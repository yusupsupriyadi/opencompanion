//! Phone companion over real HTTP on localhost: pairing, device tokens, the session API,
//! and the SPA fallback (PRD FR-50 to FR-54, section 11).

use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::Arc;
use std::time::Duration;

use ai_remote_lib::companion::{hash_token, AssetLookup, Companion};
use ai_remote_lib::db::{Db, EventRow, SessionInfo};
use ai_remote_lib::session::{Emit, Manager};
use serde_json::Value;

struct Quiet;
impl Emit for Quiet {
    fn session(&self, _: &SessionInfo) {}
    fn output(&self, _: &str, _: &str) {}
    fn event(&self, _: &EventRow) {}
    fn tasks_changed(&self) {}
    fn notify(&self, _: &str, _: &str, _: &str) {}
}

fn http(port: u16, method: &str, path: &str, token: Option<&str>, body: Option<&str>) -> (u16, String) {
    let mut s = TcpStream::connect(("127.0.0.1", port)).expect("server is listening");
    s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    let body = body.unwrap_or("");
    let auth = token.map(|t| format!("Authorization: Bearer {t}\r\n")).unwrap_or_default();
    let req = format!(
        "{method} {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\nContent-Type: application/json\r\n{auth}Content-Length: {}\r\n\r\n{body}",
        body.len()
    );
    s.write_all(req.as_bytes()).unwrap();
    let mut raw = String::new();
    s.read_to_string(&mut raw).unwrap();
    let status: u16 = raw.split_whitespace().nth(1).unwrap().parse().unwrap();
    let body = raw.split_once("\r\n\r\n").map(|(_, b)| b.to_string()).unwrap_or_default();
    (status, body)
}

fn json(body: &str) -> Value {
    serde_json::from_str(body).unwrap_or(Value::Null)
}

#[test]
fn pairing_tokens_and_the_session_api() {
    let db = Arc::new(Db::open_in_memory().unwrap());
    let data = std::env::temp_dir().join(format!("air-companion-{}", std::process::id()));
    let manager = Manager::new(Arc::clone(&db), Arc::new(Quiet), data);
    let assets: AssetLookup = Arc::new(|path: &str| match path {
        "index.html" => Some((b"<!doctype html><title>shell</title>".to_vec(), "text/html".into())),
        "_app/app.js" => Some((b"console.log(1)".to_vec(), "text/javascript".into())),
        _ => None,
    });
    let companion = Companion::new(Arc::clone(&db), manager, assets);
    let port = 40_000 + (std::process::id() % 20_000) as u16;
    let status = tauri::async_runtime::block_on(companion.start(port));
    assert!(status.running, "server did not start: {:?}", status.error);

    // Nothing without a device token.
    assert_eq!(http(port, "GET", "/api/sessions", None, None).0, 401);
    assert_eq!(http(port, "GET", "/api/sessions", Some("made-up"), None).0, 401);

    let pairing = companion.start_pairing().expect("pairing code");
    assert!(pairing.url.ends_with(&format!("/m/pair?code={}", pairing.code)));
    let wrong = if pairing.code == "000000" { "111111" } else { "000000" };
    let (code, body) = http(port, "POST", "/api/pair", None, Some(&format!(r#"{{"code":"{wrong}","name":"Test phone"}}"#)));
    assert_eq!(code, 401, "{body}");

    let (code, body) = http(port, "POST", "/api/pair", None, Some(&format!(r#"{{"code":"{}","name":"Test phone"}}"#, pairing.code)));
    assert_eq!(code, 200, "{body}");
    let token = json(&body)["token"].as_str().unwrap().to_string();
    assert_eq!(token.len(), 64);

    // Only the hash is stored, and the code cannot be used twice.
    let devices = db.devices().unwrap();
    assert_eq!(devices.len(), 1);
    assert_eq!(devices[0].name, "Test phone");
    assert_eq!(devices[0].token_hash, hash_token(&token));
    assert!(!devices[0].token_hash.contains(&token));
    let (code, _) = http(port, "POST", "/api/pair", None, Some(&format!(r#"{{"code":"{}"}}"#, pairing.code)));
    assert_eq!(code, 410);

    let (code, body) = http(port, "GET", "/api/sessions", Some(&token), None);
    assert_eq!(code, 200);
    assert_eq!(json(&body)["sessions"], Value::Array(vec![]));
    assert_eq!(http(port, "GET", "/api/sessions/nope", Some(&token), None).0, 404);

    // The phone app is the SPA shell; missing scripts stay missing.
    let (code, body) = http(port, "GET", "/m/pair?code=1", None, None);
    assert_eq!(code, 200);
    assert!(body.contains("<title>shell</title>"));
    assert_eq!(http(port, "GET", "/_app/app.js", None, None).0, 200);
    assert_eq!(http(port, "GET", "/_app/gone.js", None, None).0, 404);

    // A removed device is locked out at once.
    db.remove_device(&devices[0].id).unwrap();
    assert_eq!(http(port, "GET", "/api/sessions", Some(&token), None).0, 401);

    companion.stop();
    std::thread::sleep(Duration::from_millis(200));
    assert!(TcpStream::connect(("127.0.0.1", port)).is_err(), "server should stop listening");
}

#[test]
fn too_many_wrong_codes_end_the_pairing() {
    let db = Arc::new(Db::open_in_memory().unwrap());
    let data = std::env::temp_dir().join(format!("air-companion-b-{}", std::process::id()));
    let manager = Manager::new(Arc::clone(&db), Arc::new(Quiet), data);
    let companion = Companion::new(Arc::clone(&db), manager, Arc::new(|_: &str| None));
    let port = 40_000 + ((std::process::id() + 7) % 20_000) as u16;
    assert!(tauri::async_runtime::block_on(companion.start(port)).running);
    let pairing = companion.start_pairing().unwrap();
    let wrong = if pairing.code == "000000" { "111111" } else { "000000" };
    let mut last = 0;
    for _ in 0..5 {
        last = http(port, "POST", "/api/pair", None, Some(&format!(r#"{{"code":"{wrong}"}}"#))).0;
    }
    assert_eq!(last, 429);
    // Even the right code is refused now; a new code has to be shown.
    let (code, _) = http(port, "POST", "/api/pair", None, Some(&format!(r#"{{"code":"{}"}}"#, pairing.code)));
    assert_eq!(code, 410);
    companion.stop();
}
