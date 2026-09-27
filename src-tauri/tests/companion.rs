//! Phone companion over real HTTP on localhost: pairing, device tokens, the session API,
//! Chat and Board from the phone, the SPA fallback and the files that install the phone app
//! (PRD FR-50 to FR-57, FR-77, section 11).

use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::Arc;
use std::time::{Duration, Instant};

use opencompanion_lib::cli::CliKind;
use opencompanion_lib::companion::{hash_token, AssetLookup, Companion};
use opencompanion_lib::db::{ChatMessage, Db, DispatchCard, EventRow, Mode, SessionInfo, Settings, Status};
use opencompanion_lib::session::{Emit, Manager};
use serde_json::Value;

struct Quiet;
impl Emit for Quiet {
    fn session(&self, _: &SessionInfo) {}
    fn output(&self, _: &str, _: &str, _: u64) {}
    fn event(&self, _: &EventRow) {}
    fn tasks_changed(&self) {}
    fn notify(&self, _: &str, _: &str, _: &str) {}
}

fn http(port: u16, method: &str, path: &str, token: Option<&str>, body: Option<&str>) -> (u16, String) {
    let raw = exchange(port, method, path, token, body);
    let status: u16 = raw.split_whitespace().nth(1).unwrap().parse().unwrap();
    let body = raw.split_once("\r\n\r\n").map(|(_, b)| b.to_string()).unwrap_or_default();
    (status, body)
}

/// A response header's value for a plain GET, or "" when it is not sent.
fn header(port: u16, path: &str, name: &str) -> String {
    let raw = exchange(port, "GET", path, None, None);
    let head = raw.split_once("\r\n\r\n").map(|(h, _)| h).unwrap_or(&raw);
    head.lines()
        .filter_map(|line| line.split_once(':'))
        .find(|(k, _)| k.eq_ignore_ascii_case(name))
        .map(|(_, v)| v.trim().to_string())
        .unwrap_or_default()
}

/// The whole raw response to one request.
fn exchange(port: u16, method: &str, path: &str, token: Option<&str>, body: Option<&str>) -> String {
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
    raw
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
        // What Tauri's resolver guesses for these: it does not know `.webmanifest`.
        "m/manifest.webmanifest" => Some((br#"{"name":"OpenCompanion"}"#.to_vec(), "text/html".into())),
        "service-worker.js" => Some((b"self.addEventListener('fetch', () => {})".to_vec(), "application/octet-stream".into())),
        _ => None,
    });
    let companion = Companion::new(Arc::clone(&db), manager, assets);
    // Port 0: the system picks a free one, so a port taken on this machine cannot fail the test.
    let status = tauri::async_runtime::block_on(companion.start(0));
    let port = status.port;
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

    // The phone can install the app and register its service worker, which sits at the root so its
    // scope may cover `/m` without a Service-Worker-Allowed header.
    assert_eq!(header(port, "/m/manifest.webmanifest", "content-type"), "application/manifest+json");
    assert_eq!(header(port, "/service-worker.js", "content-type"), "text/javascript");
    assert_eq!(header(port, "/m/pair", "content-type"), "text/html");

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
    let status = tauri::async_runtime::block_on(companion.start(0));
    assert!(status.running);
    let port = status.port;
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

fn paired(port: u16, companion: &Companion) -> String {
    let pairing = companion.start_pairing().expect("pairing code");
    let (code, body) = http(port, "POST", "/api/pair", None, Some(&format!(r#"{{"code":"{}","name":"Test phone"}}"#, pairing.code)));
    assert_eq!(code, 200, "{body}");
    json(&body)["token"].as_str().unwrap().to_string()
}

fn wait_for(db: &Db, id: &str, what: &str, pred: impl Fn(&SessionInfo) -> bool) -> SessionInfo {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        let s = db.session(id).unwrap().unwrap();
        if pred(&s) {
            return s;
        }
        assert!(Instant::now() < deadline, "timed out waiting for {what}; last status {:?}", s.status);
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// New session, messages, Chat cards and the Board from the phone (PRD FR-12, FR-57, FR-77).
#[test]
fn the_phone_starts_sessions_sends_messages_and_works_the_board() {
    let fake = env!("CARGO_BIN_EXE_fake-cli").to_string();
    let db = Arc::new(Db::open_in_memory().unwrap());
    let mut settings = Settings::default();
    for kind in ["claude", "codex", "opencode"] {
        settings.cli_paths.insert(kind.into(), fake.clone());
    }
    db.save_settings(&settings).unwrap();
    let base = std::env::temp_dir().join(format!("air-companion-c-{}", std::process::id()));
    let work = base.join("project");
    std::fs::create_dir_all(&work).unwrap();
    let cwd = work.display().to_string().replace('\\', "\\\\");
    let manager = Manager::new(Arc::clone(&db), Arc::new(Quiet), base.join("data"));
    let companion = Companion::new(Arc::clone(&db), Arc::clone(&manager), Arc::new(|_: &str| None));
    let status = tauri::async_runtime::block_on(companion.start(0));
    assert!(status.running);
    let port = status.port;

    // Every new endpoint needs a paired phone.
    let start = r#"{"cli":"opencode","cwd":".","mode":"headless"}"#;
    for (method, path, body) in [
        ("GET", "/api/options", None),
        ("POST", "/api/sessions", Some(start)),
        ("POST", "/api/sessions/x/input", Some("{}")),
        ("GET", "/api/chat", None),
        ("POST", "/api/chat", Some(r#"{"message":"hi"}"#)),
        ("POST", "/api/chat/cards/run", Some(r#"{"messageId":"a","cardId":"b"}"#)),
        ("GET", "/api/tasks", None),
        ("POST", "/api/tasks/x/move", Some(r#"{"column":"todo"}"#)),
        ("POST", "/api/tasks/x/run", Some(start)),
    ] {
        assert_eq!(http(port, method, path, None, body).0, 401, "{method} {path}");
    }
    let token = paired(port, &companion);
    let t = Some(token.as_str());

    let (code, body) = http(port, "GET", "/api/options", t, None);
    assert_eq!(code, 200, "{body}");
    let options = json(&body);
    assert!(options["clis"].as_array().unwrap().iter().any(|c| c["kind"] == "opencode" && c["path"].is_string()));
    assert_eq!(options["permissionMode"], "ask");

    // A headless session from the phone, then a follow-up turn in the same conversation.
    let bad = r#"{"cli":"opencode","cwd":"Z:\\no\\such\\folder","mode":"headless","prompt":"x"}"#;
    let (code, body) = http(port, "POST", "/api/sessions", t, Some(bad));
    assert_eq!((code, json(&body)["error"].as_str()), (409, Some("This folder does not exist.")));
    let req = format!(r#"{{"cli":"opencode","cwd":"{cwd}","mode":"headless","prompt":"add hello","permissionMode":"plan"}}"#);
    let (code, body) = http(port, "POST", "/api/sessions", t, Some(&req));
    assert_eq!(code, 200, "{body}");
    let id = json(&body)["session"]["id"].as_str().unwrap().to_string();
    assert_eq!(json(&body)["session"]["permissionMode"], "plan");
    wait_for(&db, &id, "first turn", |s| s.status == Status::Done);
    let (code, body) = http(port, "POST", &format!("/api/sessions/{id}/input"), t, Some(r#"{"text":"   "}"#));
    assert_eq!((code, json(&body)["error"].as_str()), (409, Some("Write a message first.")));
    let (code, body) = http(port, "POST", &format!("/api/sessions/{id}/input"), t, Some(r#"{"key":"esc"}"#));
    assert_eq!(code, 409, "{body}");
    let (code, body) = http(port, "POST", &format!("/api/sessions/{id}/input"), t, Some(r#"{"text":"again"}"#));
    assert_eq!(code, 200, "{body}");
    let said = |text: &str| {
        db.events(&id, 200)
            .unwrap()
            .iter()
            .any(|e| serde_json::to_value(&e.event).unwrap()["text"] == text)
    };
    wait_for(&db, &id, "second turn", |_| said("done: again"));
    assert_eq!(http(port, "POST", "/api/sessions/nope/input", t, Some(r#"{"text":"hi"}"#)).0, 404);

    // A Chat follow-up for that session (PRD FR-25): Send continues it instead of starting one.
    wait_for(&db, &id, "second turn to end", |s| !s.status.is_live());
    let chat = db.create_thread("More on that").unwrap();
    let follow = DispatchCard {
        id: "card-f".into(),
        cli: CliKind::Opencode,
        title: "add hello".into(),
        folder: work.display().to_string(),
        prompt: "also say bye".into(),
        mode: Mode::Headless,
        reason: String::new(),
        problem: None,
        state: "proposed".into(),
        session_id: None,
        task_id: None,
        target: Some(id.clone()),
        auto: false,
    };
    db.add_chat(&ChatMessage {
        id: "msg-f".into(),
        thread_id: chat.id.clone(),
        role: "planner".into(),
        text: "One follow-up.".into(),
        cards: vec![follow],
        created_at: 1,
    })
    .unwrap();
    let sessions_before = db.sessions(100).unwrap().len();
    let (code, body) = http(port, "POST", "/api/chat/cards/board", t, Some(r#"{"messageId":"msg-f","cardId":"card-f"}"#));
    assert_eq!(code, 409, "{body}");
    let (code, body) = http(port, "POST", "/api/chat/cards/run", t, Some(r#"{"messageId":"msg-f","cardId":"card-f"}"#));
    assert_eq!(code, 200, "{body}");
    assert_eq!(json(&body)["message"]["cards"][0]["sessionId"], id.as_str());
    wait_for(&db, &id, "follow-up turn", |_| said("done: also say bye"));
    assert_eq!(db.sessions(100).unwrap().len(), sessions_before, "a follow-up starts no new session");
    db.delete_thread(&chat.id).unwrap();

    // In a terminal the text is typed, then entered.
    let req = format!(r#"{{"cli":"opencode","cwd":"{cwd}","mode":"interactive"}}"#);
    let (code, body) = http(port, "POST", "/api/sessions", t, Some(&req));
    assert_eq!(code, 200, "{body}");
    let pty = json(&body)["session"]["id"].as_str().unwrap().to_string();
    let deadline = Instant::now() + Duration::from_secs(20);
    while !manager.output(&pty).contains("fake ready") {
        assert!(Instant::now() < deadline, "the fake terminal never got ready");
        std::thread::sleep(Duration::from_millis(50));
    }
    let (code, body) = http(port, "POST", &format!("/api/sessions/{pty}/input"), t, Some(r#"{"key":"f5"}"#));
    assert_eq!((code, json(&body)["error"].as_str()), (409, Some("Unknown key.")));
    let (code, body) = http(port, "POST", &format!("/api/sessions/{pty}/input"), t, Some(r#"{"text":"hello"}"#));
    assert_eq!(code, 200, "{body}");
    wait_for(&db, &pty, "terminal exit", |s| !s.status.is_live());
    assert!(manager.output(&pty).contains("got: hello"));

    // Chat: a new thread needs words, a missing thread is gone, and cards go to the Board or away.
    let (code, body) = http(port, "GET", "/api/chat", t, None);
    assert_eq!(code, 200, "{body}");
    assert_eq!(json(&body)["threads"], Value::Array(vec![]));
    let (code, body) = http(port, "POST", "/api/chat", t, Some(r#"{"message":"  "}"#));
    assert_eq!((code, json(&body)["error"].as_str()), (409, Some("Write a message first.")));
    assert_eq!(http(port, "GET", "/api/chat/nope", t, None).0, 404);

    let thread = db.create_thread("Fix tests").unwrap();
    let card = DispatchCard {
        id: "card-a".into(),
        cli: CliKind::Opencode,
        title: "Fix the tests".into(),
        folder: work.display().to_string(),
        prompt: "fix the failing tests".into(),
        mode: Mode::Headless,
        reason: String::new(),
        problem: None,
        state: "proposed".into(),
        session_id: None,
        task_id: None,
        target: None,
        auto: false,
    };
    let reply = ChatMessage {
        id: "msg-a".into(),
        thread_id: thread.id.clone(),
        role: "planner".into(),
        text: "One card.".into(),
        cards: vec![card],
        created_at: 1,
    };
    db.add_chat(&reply).unwrap();
    let (code, body) = http(port, "GET", &format!("/api/chat/{}", thread.id), t, None);
    assert_eq!(code, 200, "{body}");
    assert_eq!(json(&body)["messages"][0]["cards"][0]["id"], "card-a");

    let pick = r#"{"messageId":"msg-a","cardId":"card-a"}"#;
    let (code, body) = http(port, "POST", "/api/chat/cards/discard", t, Some(pick));
    assert_eq!(code, 200, "{body}");
    assert_eq!(json(&body)["message"]["cards"][0]["state"], "discarded");
    let (code, body) = http(port, "POST", "/api/chat/cards/run", t, Some(pick));
    assert_eq!((code, json(&body)["error"].as_str()), (409, Some("This card already ran or was discarded.")));
    let (code, body) = http(port, "POST", "/api/chat/cards/discard", t, Some(r#"{"messageId":"msg-a","cardId":"card-a","undo":true}"#));
    assert_eq!(code, 200, "{body}");
    assert_eq!(json(&body)["message"]["cards"][0]["state"], "proposed");
    let (code, body) = http(port, "POST", "/api/chat/cards/board", t, Some(pick));
    assert_eq!(code, 200, "{body}");
    let task_id = json(&body)["task"]["id"].as_str().unwrap().to_string();
    assert_eq!(json(&body)["task"]["column"], "todo");
    let (code, body) = http(port, "POST", "/api/chat/cards/run", t, Some(pick));
    assert_eq!(code, 200, "{body}");
    let started = json(&body);
    assert_eq!(started["message"]["cards"][0]["state"], "started");
    let from_card = started["message"]["cards"][0]["sessionId"].as_str().unwrap().to_string();
    assert_eq!(db.session(&from_card).unwrap().unwrap().source, "chat");
    wait_for(&db, &from_card, "card session", |s| !s.status.is_live());

    // Board: move a card, refuse an unknown column, then run it.
    let (code, body) = http(port, "GET", "/api/tasks", t, None);
    assert_eq!(code, 200, "{body}");
    assert_eq!(json(&body)["tasks"][0]["id"], task_id.as_str());
    let (code, body) = http(port, "POST", &format!("/api/tasks/{task_id}/move"), t, Some(r#"{"column":"pending"}"#));
    assert_eq!(code, 200, "{body}");
    assert_eq!(db.task(&task_id).unwrap().unwrap().column, "pending");
    let (code, body) = http(port, "POST", &format!("/api/tasks/{task_id}/move"), t, Some(r#"{"column":"later"}"#));
    assert_eq!((code, json(&body)["error"].as_str()), (409, Some("Unknown column.")));
    let req = format!(r#"{{"cli":"opencode","cwd":"{cwd}","mode":"headless","prompt":"from the board"}}"#);
    let (code, body) = http(port, "POST", &format!("/api/tasks/{task_id}/run"), t, Some(&req));
    assert_eq!(code, 200, "{body}");
    let run = json(&body)["session"]["id"].as_str().unwrap().to_string();
    let task = db.task(&task_id).unwrap().unwrap();
    assert_eq!(task.session_id.as_deref(), Some(run.as_str()));
    assert!(task.column == "progress" || task.column == "done", "{}", task.column);
    wait_for(&db, &run, "board session", |s| !s.status.is_live());
    // The phone's Session screen gets what the desktop shows beside the output.
    let (code, body) = http(port, "GET", &format!("/api/sessions/{run}"), t, None);
    assert_eq!(code, 200, "{body}");
    let detail = json(&body);
    assert_eq!(detail["task"]["id"], task_id.as_str());
    assert!(detail["files"].is_array(), "{body}");
    assert!(detail["usage"].is_null(), "a finished session has no CPU or memory: {body}");

    manager.kill_all();
    companion.stop();
}

/// Opens the live connection the phone keeps, with a plain WebSocket handshake.
fn ws_connect(port: u16, token: &str) -> TcpStream {
    let mut s = TcpStream::connect(("127.0.0.1", port)).expect("server is listening");
    s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    let req = format!(
        "GET /api/ws?token={token} HTTP/1.1\r\nHost: localhost\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\n\r\n"
    );
    s.write_all(req.as_bytes()).unwrap();
    // Byte by byte, so no frame after the headers is swallowed.
    let mut head = Vec::new();
    let mut byte = [0u8; 1];
    while !head.ends_with(b"\r\n\r\n") {
        s.read_exact(&mut byte).unwrap();
        head.push(byte[0]);
    }
    assert!(String::from_utf8_lossy(&head).starts_with("HTTP/1.1 101"), "{}", String::from_utf8_lossy(&head));
    s
}

/// One unmasked frame from the server: its opcode and payload.
fn ws_frame(s: &mut TcpStream) -> (u8, Vec<u8>) {
    let mut h = [0u8; 2];
    s.read_exact(&mut h).unwrap();
    let len = match h[1] & 0x7F {
        126 => {
            let mut b = [0u8; 2];
            s.read_exact(&mut b).unwrap();
            u16::from_be_bytes(b) as usize
        }
        127 => {
            let mut b = [0u8; 8];
            s.read_exact(&mut b).unwrap();
            u64::from_be_bytes(b) as usize
        }
        n => n as usize,
    };
    let mut payload = vec![0u8; len];
    s.read_exact(&mut payload).unwrap();
    (h[0] & 0x0F, payload)
}

/// The close code of the next close frame, skipping any updates still on their way.
fn close_code(s: &mut TcpStream) -> u16 {
    loop {
        let (op, payload) = ws_frame(s);
        if op == 0x8 {
            return u16::from_be_bytes([payload[0], payload[1]]);
        }
    }
}

/// PRD FR-55: removing a phone on the desktop ends its live updates at once, and turning phone
/// access off ends every phone's.
#[test]
fn a_removed_phone_loses_its_live_updates() {
    let db = Arc::new(Db::open_in_memory().unwrap());
    let data = std::env::temp_dir().join(format!("air-companion-d-{}", std::process::id()));
    let manager = Manager::new(Arc::clone(&db), Arc::new(Quiet), data);
    let companion = Companion::new(Arc::clone(&db), manager, Arc::new(|_: &str| None));
    let status = tauri::async_runtime::block_on(companion.start(0));
    assert!(status.running);
    let port = status.port;

    let first = paired(port, &companion);
    let second = paired(port, &companion);
    let mut a = ws_connect(port, &first);
    let mut b = ws_connect(port, &second);
    std::thread::sleep(Duration::from_millis(200));
    companion.broadcast(serde_json::json!({ "type": "tasks" }));
    assert_eq!(ws_frame(&mut a), (0x1, br#"{"type":"tasks"}"#.to_vec()));
    assert_eq!(ws_frame(&mut b).0, 0x1);

    let removed = db.devices().unwrap().into_iter().find(|d| d.token_hash == hash_token(&first)).unwrap();
    db.remove_device(&removed.id).unwrap();
    companion.revoke(Some(&removed.id));
    assert_eq!(close_code(&mut a), opencompanion_lib::companion::CLOSE_REMOVED);

    // The other phone still hears updates, until phone access is turned off.
    companion.broadcast(serde_json::json!({ "type": "tasks" }));
    assert_eq!(ws_frame(&mut b).0, 0x1);
    companion.stop();
    assert_eq!(close_code(&mut b), opencompanion_lib::companion::CLOSE_OFF);
}

/// Remembers what the phone's changes told the desktop, so a test can check the live updates.
#[derive(Default)]
struct Heard {
    chats: std::sync::Mutex<Vec<String>>,
    settings: std::sync::atomic::AtomicUsize,
}
impl Emit for Heard {
    fn session(&self, _: &SessionInfo) {}
    fn output(&self, _: &str, _: &str, _: u64) {}
    fn event(&self, _: &EventRow) {}
    fn tasks_changed(&self) {}
    fn notify(&self, _: &str, _: &str, _: &str) {}
    fn chat_changed(&self, thread_id: &str) {
        self.chats.lock().unwrap().push(thread_id.to_string());
    }
    fn settings_changed(&self) {
        self.settings.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }
}

/// Chat from the phone as on the desktop (PRD FR-24a, FR-25, FR-28, FR-57): the chat list, the
/// planner and its model, editing a card before Run, deleting a chat, folders for `@` and the
/// live sessions' horizon marks.
#[test]
fn the_phone_chat_matches_the_desktop() {
    let fake = env!("CARGO_BIN_EXE_fake-cli").to_string();
    let db = Arc::new(Db::open_in_memory().unwrap());
    let mut settings = Settings::default();
    for kind in ["claude", "codex", "opencode"] {
        settings.cli_paths.insert(kind.into(), fake.clone());
    }
    settings.chat_cli = Some(CliKind::Claude);
    settings.planner_api.api_key = "sk-secret-key".into();
    db.save_settings(&settings).unwrap();
    let base = std::env::temp_dir().join(format!("air-companion-e-{}", std::process::id()));
    let work = base.join("project");
    std::fs::create_dir_all(&work).unwrap();
    let folder = work.display().to_string();
    let heard = Arc::new(Heard::default());
    let manager = Manager::new(Arc::clone(&db), heard.clone(), base.join("data"));
    let companion = Companion::new(Arc::clone(&db), Arc::clone(&manager), Arc::new(|_: &str| None));
    let status = tauri::async_runtime::block_on(companion.start(0));
    assert!(status.running);
    let port = status.port;

    let edit = r#"{"messageId":"a","card":{"id":"b","cli":"claude","folder":".","prompt":"x","mode":"headless","reason":"","problem":null,"state":"proposed","sessionId":null}}"#;
    for (method, path, body) in [
        ("GET", "/api/folders", None),
        ("GET", "/api/chat/threads", None),
        ("GET", "/api/chat/setup", None),
        ("GET", "/api/chat/models/claude", None),
        ("POST", "/api/chat/model", Some(r#"{"cli":"claude","model":"opus"}"#)),
        ("POST", "/api/chat/cards/edit", Some(edit)),
        ("DELETE", "/api/chat/x", None),
    ] {
        assert_eq!(http(port, method, path, None, body).0, 401, "{method} {path}");
    }
    let token = paired(port, &companion);
    let t = Some(token.as_str());

    // The planner as the composer needs it, without the custom provider's key.
    let (code, body) = http(port, "GET", "/api/chat/setup", t, None);
    assert_eq!(code, 200, "{body}");
    let setup = json(&body);
    assert_eq!(setup["chatCli"], "claude");
    assert_eq!(setup["plannerSource"], "cli");
    assert_eq!(setup["provider"], Value::Null);
    assert!(setup["clis"].as_array().unwrap().iter().any(|c| c["kind"] == "claude" && c["path"].is_string()));
    assert!(!body.contains("sk-secret-key"));

    // The model picker: Claude Code's list, then a choice kept for Claude Code only.
    let (code, body) = http(port, "GET", "/api/chat/models/claude", t, None);
    assert_eq!(code, 200, "{body}");
    assert!(!json(&body)["models"].as_array().unwrap().is_empty());
    assert_eq!(http(port, "GET", "/api/chat/models/nope", t, None).0, 400);
    let (code, body) = http(port, "POST", "/api/chat/model", t, Some(r#"{"cli":"claude","model":"opus","effort":"high"}"#));
    assert_eq!(code, 200, "{body}");
    assert_eq!(json(&body)["chatModels"]["claude"]["model"], "opus");
    assert!(!body.contains("sk-secret-key"));
    assert_eq!(db.settings().unwrap().chat_models["claude"].effort, "high");
    assert_eq!(db.settings().unwrap().planner_api.api_key, "sk-secret-key", "only the model choice changes");
    assert_eq!(heard.settings.load(std::sync::atomic::Ordering::SeqCst), 1, "the desktop hears about it");

    // A custom provider names its model and whether it is ready, still without the key.
    let mut api = db.settings().unwrap();
    api.planner_source = opencompanion_lib::db::PlannerSource::Api;
    api.planner_api.model = "qwen3".into();
    db.save_settings(&api).unwrap();
    let (_, body) = http(port, "GET", "/api/chat/setup", t, None);
    assert_eq!(json(&body)["provider"], serde_json::json!({ "model": "qwen3", "ready": false }));
    assert!(!body.contains("sk-secret-key"));

    // Editing a card before Run: the same checks as the planner's own cards.
    let thread = db.create_thread("Docs").unwrap();
    let card = |id: &str, target: Option<String>| DispatchCard {
        id: id.into(),
        cli: CliKind::Claude,
        title: "Write docs".into(),
        folder: folder.clone(),
        prompt: "write the docs".into(),
        mode: Mode::Headless,
        reason: String::new(),
        problem: None,
        state: "proposed".into(),
        session_id: None,
        task_id: None,
        target,
        auto: false,
    };
    db.add_chat(&ChatMessage {
        id: "msg-e".into(),
        thread_id: thread.id.clone(),
        role: "planner".into(),
        text: "Two cards.".into(),
        cards: vec![card("card-e", None), card("card-g", Some("no-such-session".into()))],
        created_at: 1,
    })
    .unwrap();
    let mut edited = serde_json::to_value(card("card-e", None)).unwrap();
    edited["cli"] = "opencode".into();
    edited["title"] = "  API docs ".into();
    edited["mode"] = "interactive".into();
    edited["prompt"] = " document the API ".into();
    let req = serde_json::json!({ "messageId": "msg-e", "card": edited }).to_string();
    let (code, body) = http(port, "POST", "/api/chat/cards/edit", t, Some(&req));
    assert_eq!(code, 200, "{body}");
    let got = &json(&body)["message"]["cards"][0];
    assert_eq!(
        (got["cli"].as_str(), got["title"].as_str(), got["mode"].as_str()),
        (Some("opencode"), Some("API docs"), Some("interactive"))
    );
    assert_eq!(got["prompt"], "document the API");
    assert_eq!(got["problem"], Value::Null);
    edited["folder"] = r"Z:\no\such\folder".into();
    let req = serde_json::json!({ "messageId": "msg-e", "card": edited }).to_string();
    let (_, body) = http(port, "POST", "/api/chat/cards/edit", t, Some(&req));
    assert_eq!(json(&body)["message"]["cards"][0]["problem"], r"The folder Z:\no\such\folder does not exist.");

    // A follow-up keeps its session's CLI and folder; only the message changes.
    let mut follow = serde_json::to_value(card("card-g", None)).unwrap();
    follow["cli"] = "codex".into();
    follow["prompt"] = "and the changelog".into();
    let req = serde_json::json!({ "messageId": "msg-e", "card": follow }).to_string();
    let (code, body) = http(port, "POST", "/api/chat/cards/edit", t, Some(&req));
    assert_eq!(code, 200, "{body}");
    let got = &json(&body)["message"]["cards"][1];
    assert_eq!((got["cli"].as_str(), got["prompt"].as_str()), (Some("claude"), Some("and the changelog")));
    assert!(got["problem"].is_string(), "a follow-up for a session that is gone says why it cannot go");

    // A card that ran or was discarded keeps what it had.
    let mut discarded = card("card-e", None);
    discarded.state = "discarded".into();
    db.add_chat(&ChatMessage {
        id: "msg-e".into(),
        thread_id: thread.id.clone(),
        role: "planner".into(),
        text: "Two cards.".into(),
        cards: vec![discarded],
        created_at: 1,
    })
    .unwrap();
    let req = serde_json::json!({ "messageId": "msg-e", "card": card("card-e", None) }).to_string();
    let (code, body) = http(port, "POST", "/api/chat/cards/edit", t, Some(&req));
    assert_eq!((code, json(&body)["error"].as_str()), (409, Some("This card already ran or was discarded.")));

    // The chat list without asking the CLIs, then deleting a chat.
    let (code, body) = http(port, "GET", "/api/chat/threads", t, None);
    assert_eq!(code, 200, "{body}");
    assert_eq!(json(&body)["threads"][0]["id"], thread.id.as_str());
    assert_eq!(json(&body)["answering"], Value::Array(vec![]));
    heard.chats.lock().unwrap().clear();
    let (code, body) = http(port, "DELETE", &format!("/api/chat/{}", thread.id), t, None);
    assert_eq!(code, 200, "{body}");
    assert!(db.thread(&thread.id).unwrap().is_none());
    assert!(db.chat_message("msg-e").unwrap().is_none());
    assert_eq!(*heard.chats.lock().unwrap(), [thread.id.as_str()], "open chat screens hear about it");
    assert_eq!(http(port, "DELETE", &format!("/api/chat/{}", thread.id), t, None).0, 404);

    // Folders for `@`, and horizon marks for the live sessions only.
    let (code, body) = http(port, "GET", "/api/folders", t, None);
    assert_eq!(code, 200, "{body}");
    assert!(json(&body)["folders"].is_array());
    let (code, body) = http(port, "GET", "/api/sessions", t, None);
    assert_eq!(code, 200, "{body}");
    assert_eq!(json(&body)["marks"], serde_json::json!({}));

    companion.stop();
}
