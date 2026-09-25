//! Phone companion (PRD section F): an HTTP + WebSocket server on the LAN, off by default.
//! Only paired devices get in: a one-time 6-digit code that expires, a limit on attempts,
//! and device tokens stored as SHA-256 hashes (PRD section 11).

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path as UrlPath, Query, State};
use axum::http::{header, HeaderMap, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tokio::sync::{broadcast, oneshot};

use crate::db::{self, Db, Device, Mode};
use crate::session::Manager;

const PAIR_TTL_MS: i64 = 120_000;
const PAIR_ATTEMPTS: u32 = 5;

pub type AssetLookup = Arc<dyn Fn(&str) -> Option<(Vec<u8>, String)> + Send + Sync>;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Pairing {
    pub code: String,
    pub url: String,
    pub expires_at: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompanionStatus {
    pub running: bool,
    pub address: Option<String>,
    pub port: u16,
    pub error: Option<String>,
}

struct PairState {
    code: String,
    expires_at: i64,
    attempts: u32,
}

pub struct Companion {
    db: Arc<Db>,
    manager: Arc<Manager>,
    tx: broadcast::Sender<String>,
    assets: AssetLookup,
    shutdown: Mutex<Option<oneshot::Sender<()>>>,
    status: Mutex<CompanionStatus>,
    pairing: Mutex<Option<PairState>>,
}

pub fn hash_token(token: &str) -> String {
    Sha256::digest(token.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn random_hex(bytes: usize) -> String {
    let mut out = String::new();
    while out.len() < bytes * 2 {
        out.push_str(&uuid::Uuid::new_v4().simple().to_string());
    }
    out.truncate(bytes * 2);
    out
}

fn random_code() -> String {
    let n = u32::from_le_bytes(uuid::Uuid::new_v4().as_bytes()[..4].try_into().unwrap_or([0; 4]));
    format!("{:06}", n % 1_000_000)
}

pub fn lan_address() -> Option<String> {
    local_ip_address::local_ip().ok().map(|ip| ip.to_string())
}

/// Last visible terminal screen, for the phone's tail view (PRD FR-53).
pub fn render_screen(raw: &str) -> String {
    let mut parser = vt100::Parser::new(40, 100, 0);
    let start = raw.len().saturating_sub(200_000);
    let mut cut = start;
    while cut < raw.len() && !raw.is_char_boundary(cut) {
        cut += 1;
    }
    parser.process(&raw.as_bytes()[cut..]);
    let screen = parser.screen();
    screen
        .rows(0, 100)
        .map(|r| r.trim_end().to_string())
        .collect::<Vec<_>>()
        .join("\n")
        .trim_matches('\n')
        .to_string()
}

impl Companion {
    pub fn new(db: Arc<Db>, manager: Arc<Manager>, assets: AssetLookup) -> Arc<Self> {
        let (tx, _) = broadcast::channel(256);
        let port = db.settings().map(|s| s.companion_port).unwrap_or(8765);
        Arc::new(Self {
            db,
            manager,
            tx,
            assets,
            shutdown: Mutex::new(None),
            status: Mutex::new(CompanionStatus {
                running: false,
                address: None,
                port,
                error: None,
            }),
            pairing: Mutex::new(None),
        })
    }

    pub fn broadcast(&self, message: Value) {
        let _ = self.tx.send(message.to_string());
    }

    pub fn status(&self) -> CompanionStatus {
        self.status.lock().map(|s| s.clone()).unwrap_or(CompanionStatus {
            running: false,
            address: None,
            port: 0,
            error: None,
        })
    }

    pub async fn start(self: &Arc<Self>, port: u16) -> CompanionStatus {
        self.stop();
        let addr = SocketAddr::from(([0, 0, 0, 0], port));
        let listener = match tokio::net::TcpListener::bind(addr).await {
            Ok(l) => l,
            Err(e) => {
                let st = CompanionStatus {
                    running: false,
                    address: None,
                    port,
                    error: Some(format!("Port {port} is not available: {e}")),
                };
                if let Ok(mut s) = self.status.lock() {
                    *s = st.clone();
                }
                return st;
            }
        };
        let (stop_tx, stop_rx) = oneshot::channel::<()>();
        if let Ok(mut s) = self.shutdown.lock() {
            *s = Some(stop_tx);
        }
        let app = router(Arc::clone(self));
        tauri::async_runtime::spawn(async move {
            let _ = axum::serve(listener, app)
                .with_graceful_shutdown(async {
                    let _ = stop_rx.await;
                })
                .await;
        });
        let st = CompanionStatus {
            running: true,
            address: lan_address(),
            port,
            error: None,
        };
        if let Ok(mut s) = self.status.lock() {
            *s = st.clone();
        }
        st
    }

    pub fn stop(&self) {
        if let Ok(mut s) = self.shutdown.lock() {
            if let Some(tx) = s.take() {
                let _ = tx.send(());
            }
        }
        if let Ok(mut s) = self.status.lock() {
            s.running = false;
            s.address = None;
        }
        if let Ok(mut p) = self.pairing.lock() {
            *p = None;
        }
    }

    /// A fresh one-time code for the QR on the desktop (PRD FR-51).
    pub fn start_pairing(&self) -> Result<Pairing, String> {
        let st = self.status();
        if !st.running {
            return Err("Turn on phone access first.".into());
        }
        let address = st.address.ok_or("No local network address was found on this computer.")?;
        let code = random_code();
        let expires_at = db::now_ms() + PAIR_TTL_MS;
        if let Ok(mut p) = self.pairing.lock() {
            *p = Some(PairState {
                code: code.clone(),
                expires_at,
                attempts: 0,
            });
        }
        Ok(Pairing {
            url: format!("http://{address}:{}/m/pair?code={code}", st.port),
            code,
            expires_at,
        })
    }

    fn redeem(&self, code: &str, name: &str) -> Result<(String, Device), (StatusCode, String)> {
        let mut guard = self
            .pairing
            .lock()
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        let Some(state) = guard.as_mut() else {
            return Err((StatusCode::GONE, "No pairing code is active. Show a new code on the desktop.".into()));
        };
        if db::now_ms() > state.expires_at {
            *guard = None;
            return Err((StatusCode::GONE, "This code has expired. Show a new code on the desktop.".into()));
        }
        if state.code != code.trim() {
            state.attempts += 1;
            if state.attempts >= PAIR_ATTEMPTS {
                *guard = None;
                return Err((
                    StatusCode::TOO_MANY_REQUESTS,
                    "Too many wrong codes. Show a new code on the desktop.".into(),
                ));
            }
            return Err((StatusCode::UNAUTHORIZED, "That code is not right.".into()));
        }
        *guard = None;
        let token = random_hex(32);
        let device = Device {
            id: db::new_id(),
            name: if name.trim().is_empty() { "Phone".into() } else { name.trim().chars().take(60).collect() },
            token_hash: hash_token(&token),
            created_at: db::now_ms(),
            last_seen: Some(db::now_ms()),
        };
        self.db
            .add_device(&device)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
        Ok((token, device))
    }

    fn device_for(&self, token: &str) -> Option<Device> {
        let device = self.db.device_by_hash(&hash_token(token)).ok().flatten()?;
        let _ = self.db.touch_device(&device.id);
        Some(device)
    }
}

type Ctx = Arc<Companion>;

fn router(ctx: Ctx) -> Router {
    Router::new()
        .route("/api/hello", get(hello))
        .route("/api/pair", post(pair))
        .route("/api/sessions", get(list_sessions))
        .route("/api/sessions/{id}", get(one_session))
        .route("/api/sessions/{id}/answer", post(answer))
        .route("/api/sessions/{id}/stop", post(stop))
        .route("/api/ws", get(ws))
        .fallback(get(static_file))
        .with_state(ctx)
}

fn fail(code: StatusCode, message: impl Into<String>) -> Response {
    (code, Json(json!({ "error": message.into() }))).into_response()
}

fn bearer(headers: &HeaderMap) -> Option<String> {
    headers
        .get(header::AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
        .map(str::to_owned)
}

// A ready-made axum response is the natural error here; boxing it only adds noise.
#[allow(clippy::result_large_err)]
fn authed(ctx: &Ctx, headers: &HeaderMap) -> Result<Device, Response> {
    bearer(headers)
        .and_then(|t| ctx.device_for(&t))
        .ok_or_else(|| fail(StatusCode::UNAUTHORIZED, "This phone is not paired, or it was removed on the desktop."))
}

async fn hello() -> Json<Value> {
    Json(json!({ "app": "OpenCompanion" }))
}

#[derive(Deserialize)]
struct PairBody {
    code: String,
    #[serde(default)]
    name: String,
}

async fn pair(State(ctx): State<Ctx>, Json(body): Json<PairBody>) -> Response {
    match ctx.redeem(&body.code, &body.name) {
        Ok((token, device)) => {
            ctx.broadcast(json!({ "type": "devices" }));
            Json(json!({ "token": token, "device": device })).into_response()
        }
        Err((code, msg)) => fail(code, msg),
    }
}

async fn list_sessions(State(ctx): State<Ctx>, headers: HeaderMap) -> Response {
    if let Err(r) = authed(&ctx, &headers) {
        return r;
    }
    match ctx.db.sessions(60) {
        Ok(s) => Json(json!({ "sessions": s })).into_response(),
        Err(e) => fail(StatusCode::INTERNAL_SERVER_ERROR, e),
    }
}

async fn one_session(State(ctx): State<Ctx>, headers: HeaderMap, UrlPath(id): UrlPath<String>) -> Response {
    if let Err(r) = authed(&ctx, &headers) {
        return r;
    }
    let Ok(Some(session)) = ctx.db.session(&id) else {
        return fail(StatusCode::NOT_FOUND, "Session not found.");
    };
    let events = ctx.db.events(&id, 60).unwrap_or_default();
    let tail = if session.mode == Mode::Interactive {
        render_screen(&ctx.manager.output(&id))
    } else {
        String::new()
    };
    Json(json!({ "session": session, "events": events, "tail": tail })).into_response()
}

#[derive(Deserialize)]
struct AnswerBody {
    allow: bool,
}

async fn answer(
    State(ctx): State<Ctx>,
    headers: HeaderMap,
    UrlPath(id): UrlPath<String>,
    Json(body): Json<AnswerBody>,
) -> Response {
    let device = match authed(&ctx, &headers) {
        Ok(d) => d,
        Err(r) => return r,
    };
    let manager = Arc::clone(&ctx.manager);
    let result = tokio::task::spawn_blocking(move || manager.answer_from(&id, body.allow, Some(&device.name))).await;
    match result {
        Ok(Ok(s)) => Json(json!({ "session": s })).into_response(),
        Ok(Err(e)) => fail(StatusCode::CONFLICT, e),
        Err(e) => fail(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    }
}

async fn stop(State(ctx): State<Ctx>, headers: HeaderMap, UrlPath(id): UrlPath<String>) -> Response {
    let device = match authed(&ctx, &headers) {
        Ok(d) => d,
        Err(r) => return r,
    };
    match ctx.manager.stop_from(&id, Some(&device.name)) {
        Ok(()) => Json(json!({ "ok": true })).into_response(),
        Err(e) => fail(StatusCode::CONFLICT, e),
    }
}

#[derive(Deserialize)]
struct WsQuery {
    token: String,
}

async fn ws(State(ctx): State<Ctx>, Query(q): Query<WsQuery>, upgrade: WebSocketUpgrade) -> Response {
    if ctx.device_for(&q.token).is_none() {
        return fail(StatusCode::UNAUTHORIZED, "This phone is not paired.");
    }
    let rx = ctx.tx.subscribe();
    upgrade.on_upgrade(move |socket| pump(socket, rx))
}

async fn pump(mut socket: WebSocket, mut rx: broadcast::Receiver<String>) {
    loop {
        tokio::select! {
            msg = rx.recv() => match msg {
                Ok(text) => {
                    if socket.send(Message::Text(text.into())).await.is_err() {
                        return;
                    }
                }
                Err(broadcast::error::RecvError::Lagged(_)) => {
                    if socket.send(Message::Text(json!({ "type": "resync" }).to_string().into())).await.is_err() {
                        return;
                    }
                }
                Err(_) => return,
            },
            incoming = socket.recv() => match incoming {
                Some(Ok(Message::Close(_))) | None | Some(Err(_)) => return,
                _ => {}
            }
        }
    }
}

/// Serves the same SvelteKit build as the desktop; unknown paths fall back to the SPA shell.
async fn static_file(State(ctx): State<Ctx>, uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    let candidates = [path.to_string(), format!("{path}.html"), "index.html".to_string()];
    for (i, candidate) in candidates.iter().enumerate() {
        if candidate.is_empty() {
            continue;
        }
        // Only the shell may stand in for a missing path; a missing script stays a 404.
        if i == 2 && path.starts_with("_app/") {
            break;
        }
        if let Some((bytes, mime)) = (ctx.assets)(candidate) {
            return ([(header::CONTENT_TYPE, mime)], bytes).into_response();
        }
    }
    fail(StatusCode::NOT_FOUND, "Not found. Build the frontend with `bun run build`.")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_are_hashed_and_codes_are_six_digits() {
        let h = hash_token("abc");
        assert_eq!(h, "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        let code = random_code();
        assert_eq!(code.len(), 6);
        assert!(code.chars().all(|c| c.is_ascii_digit()));
        assert_eq!(random_hex(32).len(), 64);
    }

    #[test]
    fn screen_render_keeps_the_last_frame() {
        let raw = "first line\r\n\x1b[2J\x1b[Hsecond frame";
        assert_eq!(render_screen(raw), "second frame");
    }
}
