//! Phone companion (PRD section F): an HTTP + WebSocket server on the LAN, off by default.
//! Only paired devices get in: a one-time 6-digit code that expires, a limit on attempts,
//! and device tokens stored as SHA-256 hashes (PRD section 11).

use std::collections::HashSet;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use axum::extract::ws::{CloseFrame, Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path as UrlPath, Query, State};
use axum::http::{header, HeaderMap, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tokio::sync::{broadcast, oneshot};

use crate::actions;
use crate::automations;
use crate::cli::CliKind;
use crate::db::{self, Db, Device, DispatchCard, Mode, PlannerSource};
use crate::events::SessionEvent;
use crate::session::{Manager, StartRequest};
use crate::{monitor, orchestrator};

const PAIR_TTL_MS: i64 = 120_000;
const PAIR_ATTEMPTS: u32 = 5;
/// Close codes the phone reads: removed on the desktop (pair again), or phone access turned off.
pub const CLOSE_REMOVED: u16 = 4401;
pub const CLOSE_OFF: u16 = 4403;

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
    /// Ends open live connections: `Some(device id)` for one phone, `None` for every phone. A
    /// socket outlives the server's graceful shutdown, so stopping the server alone would not.
    revoke: broadcast::Sender<Option<String>>,
    /// CPU and memory for the phone's Session screen. Kept between requests: CPU needs two
    /// measurements, and the phone asks every few seconds while a session runs.
    monitor: Mutex<monitor::Monitor>,
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
        let (revoke, _) = broadcast::channel(16);
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
            revoke,
            monitor: Mutex::new(monitor::Monitor::new()),
        })
    }

    /// Closes the live connection of a removed phone, or of every phone (`None`).
    pub fn revoke(&self, device_id: Option<&str>) {
        let _ = self.revoke.send(device_id.map(str::to_owned));
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
        // Port 0 lets the system pick one; the status reports the port actually bound.
        let port = listener.local_addr().map(|a| a.port()).unwrap_or(port);
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
        self.revoke(None);
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
        .route("/api/options", get(options))
        .route("/api/sessions", get(list_sessions).post(start_session))
        .route("/api/sessions/{id}", get(one_session))
        .route("/api/sessions/{id}/answer", post(answer))
        .route("/api/sessions/{id}/stop", post(stop))
        .route("/api/sessions/{id}/done", post(mark_done))
        .route("/api/sessions/{id}/input", post(input))
        .route("/api/sessions/{id}/resume", post(resume))
        .route("/api/folders", get(folders))
        .route("/api/chat", get(chat_list).post(chat_send))
        .route("/api/chat/threads", get(chat_threads))
        .route("/api/chat/setup", get(chat_setup))
        .route("/api/chat/model", post(chat_set_model))
        .route("/api/chat/models/{cli}", get(chat_models))
        .route("/api/chat/{id}", get(chat_thread).delete(chat_delete))
        .route("/api/chat/cards/run", post(card_run))
        .route("/api/chat/cards/edit", post(card_edit))
        .route("/api/chat/cards/discard", post(card_discard))
        .route("/api/automations", get(automations_list).post(automation_create))
        .route("/api/automations/preview", post(automation_preview))
        .route("/api/automations/{id}", get(automation_one).put(automation_update).delete(automation_delete))
        .route("/api/automations/{id}/run", post(automation_run))
        .route("/api/automations/{id}/enabled", post(automation_enabled))
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

/// Open to any caller: the app's name and its UI language, so the pairing screen speaks it too.
async fn hello(State(ctx): State<Ctx>) -> Json<Value> {
    let language = ctx.db.settings().map(|s| s.language).unwrap_or_else(|_| "en".into());
    Json(json!({ "app": "OpenCompanion", "language": language }))
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

/// The sessions, with the horizon marks of the live ones (`[at, kind]`, newest first) for Chat's
/// live sessions list.
async fn list_sessions(State(ctx): State<Ctx>, headers: HeaderMap) -> Response {
    if let Err(r) = authed(&ctx, &headers) {
        return r;
    }
    match ctx.db.sessions(60) {
        Ok(s) => {
            let marks: serde_json::Map<String, Value> = s
                .iter()
                .filter(|info| info.status.is_live())
                .map(|info| (info.id.clone(), json!(ctx.db.event_marks(&info.id, 40).unwrap_or_default())))
                .collect();
            Json(json!({ "sessions": s, "marks": marks })).into_response()
        }
        Err(e) => fail(StatusCode::INTERNAL_SERVER_ERROR, e),
    }
}

/// Events the phone's Session screen shows, the newest ones.
const PHONE_EVENTS: usize = 150;

/// One session for the phone: its newest events, the terminal's screen, and what the desktop
/// Session screen shows beside them (files changed, CPU and memory while it runs).
async fn one_session(State(ctx): State<Ctx>, headers: HeaderMap, UrlPath(id): UrlPath<String>) -> Response {
    if let Err(r) = authed(&ctx, &headers) {
        return r;
    }
    let Ok(Some(session)) = ctx.db.session(&id) else {
        return fail(StatusCode::NOT_FOUND, "Session not found.");
    };
    // The same 400 events the desktop reads, so Files changed agrees with it; only the newest go over the network.
    let mut events = ctx.db.events(&id, 400).unwrap_or_default();
    let files = changed_files(&events);
    events.drain(..events.len().saturating_sub(PHONE_EVENTS));
    let tail = if session.mode == Mode::Interactive {
        render_screen(&ctx.manager.output(&id))
    } else {
        String::new()
    };
    let usage = match session.pid.filter(|_| session.status.is_live()) {
        Some(pid) => {
            let companion = Arc::clone(&ctx);
            tokio::task::spawn_blocking(move || companion.monitor.lock().ok()?.usage(pid))
                .await
                .ok()
                .flatten()
        }
        None => None,
    };
    Json(json!({ "session": session, "events": events, "tail": tail, "files": files, "usage": usage })).into_response()
}

/// Each file the session changed, once, with its number of edits, in the order first changed.
fn changed_files(events: &[db::EventRow]) -> Vec<(String, u32)> {
    let mut files: Vec<(String, u32)> = Vec::new();
    for row in events {
        if let SessionEvent::FileChanged { path } = &row.event {
            match files.iter_mut().find(|(p, _)| p == path) {
                Some((_, n)) => *n += 1,
                None => files.push((path.clone(), 1)),
            }
        }
    }
    files
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

async fn mark_done(State(ctx): State<Ctx>, headers: HeaderMap, UrlPath(id): UrlPath<String>) -> Response {
    let device = match authed(&ctx, &headers) {
        Ok(d) => d,
        Err(r) => return r,
    };
    match ctx.manager.mark_done_from(&id, Some(&device.name)) {
        Ok(()) => Json(json!({ "ok": true })).into_response(),
        Err(e) => fail(StatusCode::CONFLICT, e),
    }
}

/// Runs blocking work off the async runtime. An error is the desktop's own message for the phone.
async fn off_thread<T: Serialize + Send + 'static>(f: impl FnOnce() -> Result<T, String> + Send + 'static) -> Response {
    match tokio::task::spawn_blocking(f).await {
        Ok(Ok(v)) => Json(v).into_response(),
        Ok(Err(e)) => fail(StatusCode::CONFLICT, e),
        Err(e) => fail(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    }
}

/// What the New session and Run forms offer: installed CLIs, project folders and the default mode.
async fn options(State(ctx): State<Ctx>, headers: HeaderMap) -> Response {
    if let Err(r) = authed(&ctx, &headers) {
        return r;
    }
    let db = Arc::clone(&ctx.db);
    let own: HashSet<u32> = ctx.manager.own_pids().into_iter().collect();
    off_thread(move || {
        let clis = actions::detect_with_settings(&db);
        let outside = monitor::Monitor::new().scan(&own);
        let folders = orchestrator::gather(&db, &outside)?.folders;
        Ok(json!({ "clis": clis, "folders": folders, "permissionMode": db.settings()?.permission_mode }))
    })
    .await
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SessionBody {
    cli: CliKind,
    cwd: String,
    mode: Mode,
    #[serde(default)]
    prompt: String,
    permission_mode: Option<String>,
}

async fn start_session(State(ctx): State<Ctx>, headers: HeaderMap, Json(body): Json<SessionBody>) -> Response {
    if let Err(r) = authed(&ctx, &headers) {
        return r;
    }
    let manager = Arc::clone(&ctx.manager);
    off_thread(move || {
        let session = manager.start(StartRequest {
            cli: body.cli,
            cwd: body.cwd,
            mode: body.mode,
            prompt: body.prompt,
            title: None,
            permission_mode: body.permission_mode,
            source: None,
            cols: None,
            rows: None,
        })?;
        Ok(json!({ "session": session }))
    })
    .await
}

#[derive(Deserialize)]
struct InputBody {
    #[serde(default)]
    text: String,
    key: Option<String>,
}

/// The keys the phone can press in an interactive terminal.
fn key_bytes(key: &str) -> Option<&'static str> {
    Some(match key {
        "enter" => "\r",
        "esc" => "\x1b",
        "up" => "\x1b[A",
        "down" => "\x1b[B",
        "tab" => "\t",
        "ctrl_c" => "\x03",
        _ => return None,
    })
}

/// A message for the session (PRD FR-12 from the phone): typed and entered in a terminal, a
/// follow-up turn for a headless session.
async fn input(State(ctx): State<Ctx>, headers: HeaderMap, UrlPath(id): UrlPath<String>, Json(body): Json<InputBody>) -> Response {
    if let Err(r) = authed(&ctx, &headers) {
        return r;
    }
    let Ok(Some(session)) = ctx.db.session(&id) else {
        return fail(StatusCode::NOT_FOUND, "Session not found.");
    };
    let manager = Arc::clone(&ctx.manager);
    off_thread(move || {
        let text = body.text.trim();
        let info = match (session.mode, body.key.as_deref()) {
            (Mode::Interactive, Some(key)) => manager.send_input(&id, key_bytes(key).ok_or("Unknown key.")?)?,
            (Mode::Headless, Some(_)) => return Err("Keys only work in an interactive terminal.".into()),
            (_, None) if text.is_empty() => return Err("Write a message first.".into()),
            (_, None) => manager.send_message(&id, text)?,
        };
        Ok(json!({ "session": info }))
    })
    .await
}

async fn resume(State(ctx): State<Ctx>, headers: HeaderMap, UrlPath(id): UrlPath<String>) -> Response {
    if let Err(r) = authed(&ctx, &headers) {
        return r;
    }
    let manager = Arc::clone(&ctx.manager);
    off_thread(move || Ok(json!({ "session": manager.resume(&id, None, None)? }))).await
}

// Chat (PRD FR-57): the same threads and cards as the desktop.

async fn chat_list(State(ctx): State<Ctx>, headers: HeaderMap) -> Response {
    if let Err(r) = authed(&ctx, &headers) {
        return r;
    }
    let db = Arc::clone(&ctx.db);
    off_thread(move || {
        let clis = actions::detect_with_settings(&db);
        Ok(json!({
            "threads": db.threads()?,
            "planner": actions::planner_name(&db, &clis)?,
            "answering": actions::answering(),
            "autoRun": db.settings()?.auto_run_folders,
        }))
    })
    .await
}

async fn chat_thread(State(ctx): State<Ctx>, headers: HeaderMap, UrlPath(id): UrlPath<String>) -> Response {
    if let Err(r) = authed(&ctx, &headers) {
        return r;
    }
    let Ok(Some(thread)) = ctx.db.thread(&id) else {
        return fail(StatusCode::NOT_FOUND, "This chat was deleted.");
    };
    match ctx.db.chat(&id, 200) {
        Ok(messages) => {
            let answering = actions::answering().contains(&id);
            let auto_run = ctx.db.settings().map(|s| s.auto_run_folders).unwrap_or_default();
            Json(json!({ "thread": thread, "messages": messages, "answering": answering, "autoRun": auto_run })).into_response()
        }
        Err(e) => fail(StatusCode::INTERNAL_SERVER_ERROR, e),
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChatBody {
    thread_id: Option<String>,
    message: String,
}

async fn chat_send(State(ctx): State<Ctx>, headers: HeaderMap, Json(body): Json<ChatBody>) -> Response {
    if let Err(r) = authed(&ctx, &headers) {
        return r;
    }
    let manager = Arc::clone(&ctx.manager);
    off_thread(move || {
        let own: HashSet<u32> = manager.own_pids().into_iter().collect();
        let turn = actions::chat_send(&manager, &own, body.thread_id, &body.message)?;
        manager.chat_changed(&turn.thread.id);
        Ok(turn)
    })
    .await
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CardBody {
    message_id: String,
    card_id: String,
    #[serde(default)]
    undo: bool,
}

async fn card_run(State(ctx): State<Ctx>, headers: HeaderMap, Json(body): Json<CardBody>) -> Response {
    if let Err(r) = authed(&ctx, &headers) {
        return r;
    }
    let db = Arc::clone(&ctx.db);
    let manager = Arc::clone(&ctx.manager);
    off_thread(move || {
        let message = actions::run_card(&db, &manager, &body.message_id, &body.card_id)?;
        manager.chat_changed(&message.thread_id);
        Ok(json!({ "message": message }))
    })
    .await
}

async fn card_discard(State(ctx): State<Ctx>, headers: HeaderMap, Json(body): Json<CardBody>) -> Response {
    if let Err(r) = authed(&ctx, &headers) {
        return r;
    }
    match actions::discard_card(&ctx.db, &body.message_id, &body.card_id, body.undo) {
        Ok(message) => {
            ctx.manager.chat_changed(&message.thread_id);
            Json(json!({ "message": message })).into_response()
        }
        Err(e) => fail(StatusCode::CONFLICT, e),
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EditBody {
    message_id: String,
    card: DispatchCard,
}

/// The same card edit as the desktop's, checked the same way.
async fn card_edit(State(ctx): State<Ctx>, headers: HeaderMap, Json(body): Json<EditBody>) -> Response {
    if let Err(r) = authed(&ctx, &headers) {
        return r;
    }
    let db = Arc::clone(&ctx.db);
    let manager = Arc::clone(&ctx.manager);
    off_thread(move || {
        let message = actions::update_card(&db, &body.message_id, body.card)?;
        manager.chat_changed(&message.thread_id);
        Ok(json!({ "message": message }))
    })
    .await
}

/// The saved chats and the ones the planner is answering, without asking the CLIs anything.
async fn chat_threads(State(ctx): State<Ctx>, headers: HeaderMap) -> Response {
    if let Err(r) = authed(&ctx, &headers) {
        return r;
    }
    match ctx.db.threads() {
        Ok(threads) => Json(json!({ "threads": threads, "answering": actions::answering() })).into_response(),
        Err(e) => fail(StatusCode::INTERNAL_SERVER_ERROR, e),
    }
}

/// What the phone's composer needs from Settings: who plans, the model picked for each CLI and
/// the auto-run folders. The custom provider's address and key stay on the desktop.
async fn chat_setup(State(ctx): State<Ctx>, headers: HeaderMap) -> Response {
    if let Err(r) = authed(&ctx, &headers) {
        return r;
    }
    let db = Arc::clone(&ctx.db);
    off_thread(move || {
        let clis = actions::detect_with_settings(&db);
        let settings = db.settings()?;
        let provider = (settings.planner_source == PlannerSource::Api).then(|| {
            let ready = actions::planner_name(&db, &clis).ok().flatten().is_some();
            json!({ "model": settings.planner_api.model.trim(), "ready": ready })
        });
        Ok(json!({
            "clis": clis,
            "chatCli": settings.chat_cli,
            "plannerSource": settings.planner_source,
            "provider": provider,
            "chatModels": settings.chat_models,
            "autoRun": settings.auto_run_folders,
        }))
    })
    .await
}

/// A CLI named in the address, such as `claude`.
#[allow(clippy::result_large_err)]
fn cli_from(name: &str) -> Result<CliKind, Response> {
    serde_json::from_value(json!(name)).map_err(|_| fail(StatusCode::BAD_REQUEST, "Unknown CLI."))
}

async fn chat_models(State(ctx): State<Ctx>, headers: HeaderMap, UrlPath(cli): UrlPath<String>) -> Response {
    if let Err(r) = authed(&ctx, &headers) {
        return r;
    }
    let cli = match cli_from(&cli) {
        Ok(c) => c,
        Err(r) => return r,
    };
    let db = Arc::clone(&ctx.db);
    let work_dir = ctx.manager.data_dir().join("planner");
    off_thread(move || actions::planner_models(&db, &work_dir, cli)).await
}

#[derive(Deserialize)]
struct ModelBody {
    cli: CliKind,
    #[serde(default)]
    model: String,
    #[serde(default)]
    effort: String,
}

/// Keeps the planner's model and thinking level for one CLI. Only the model choices come back:
/// the rest of Settings holds the custom provider's key.
async fn chat_set_model(State(ctx): State<Ctx>, headers: HeaderMap, Json(body): Json<ModelBody>) -> Response {
    if let Err(r) = authed(&ctx, &headers) {
        return r;
    }
    match actions::set_chat_model(&ctx.db, body.cli, &body.model, &body.effort) {
        Ok(settings) => {
            ctx.manager.settings_changed();
            Json(json!({ "chatModels": settings.chat_models })).into_response()
        }
        Err(e) => fail(StatusCode::CONFLICT, e),
    }
}

async fn chat_delete(State(ctx): State<Ctx>, headers: HeaderMap, UrlPath(id): UrlPath<String>) -> Response {
    if let Err(r) = authed(&ctx, &headers) {
        return r;
    }
    if !matches!(ctx.db.thread(&id), Ok(Some(_))) {
        return fail(StatusCode::NOT_FOUND, "This chat was deleted.");
    }
    match actions::delete_thread(&ctx.db, &id) {
        Ok(()) => {
            ctx.manager.chat_changed(&id);
            Json(json!({ "ok": true })).into_response()
        }
        Err(e) => fail(StatusCode::CONFLICT, e),
    }
}

/// Project folders for `@` in the chat composer, without the slower CLI check `/api/options` does.
async fn folders(State(ctx): State<Ctx>, headers: HeaderMap) -> Response {
    if let Err(r) = authed(&ctx, &headers) {
        return r;
    }
    let db = Arc::clone(&ctx.db);
    let own: HashSet<u32> = ctx.manager.own_pids().into_iter().collect();
    off_thread(move || {
        let outside = monitor::Monitor::new().scan(&own);
        Ok(json!({ "folders": orchestrator::gather(&db, &outside)?.folders }))
    })
    .await
}

#[derive(Deserialize)]
struct WsQuery {
    token: String,
}

async fn ws(State(ctx): State<Ctx>, Query(q): Query<WsQuery>, upgrade: WebSocketUpgrade) -> Response {
    let Some(device) = ctx.device_for(&q.token) else {
        return fail(StatusCode::UNAUTHORIZED, "This phone is not paired.");
    };
    let rx = ctx.tx.subscribe();
    let revoked = ctx.revoke.subscribe();
    upgrade.on_upgrade(move |socket| pump(socket, rx, revoked, device.id))
}

async fn close(socket: &mut WebSocket, code: u16, reason: &str) {
    let frame = CloseFrame {
        code,
        reason: reason.into(),
    };
    let _ = socket.send(Message::Close(Some(frame))).await;
}

async fn pump(mut socket: WebSocket, mut rx: broadcast::Receiver<String>, mut revoked: broadcast::Receiver<Option<String>>, device: String) {
    loop {
        tokio::select! {
            gone = revoked.recv() => match gone {
                Ok(Some(id)) if id == device => {
                    return close(&mut socket, CLOSE_REMOVED, "This phone was removed on the desktop.").await;
                }
                Ok(Some(_)) => {}
                Ok(None) => return close(&mut socket, CLOSE_OFF, "Phone access is off.").await,
                // A missed message could have been this phone's removal: it reconnects and is checked again.
                Err(broadcast::error::RecvError::Lagged(_)) => return close(&mut socket, 1012, "Reconnect.").await,
                Err(broadcast::error::RecvError::Closed) => return,
            },
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
            return ([(header::CONTENT_TYPE, content_type(candidate, mime))], bytes).into_response();
        }
    }
    fail(StatusCode::NOT_FOUND, "Not found. Build the frontend with `bun run build`.")
}

/// Tauri guesses a file's type from its bytes and extension, and an extension it does not know
/// becomes `text/html`. The phone's browser needs the real types to install the app from its
/// manifest and to register the service worker.
fn content_type(path: &str, guessed: String) -> String {
    match path.rsplit_once('.').map(|(_, ext)| ext) {
        Some("webmanifest") => "application/manifest+json".into(),
        Some("js" | "mjs") => "text/javascript".into(),
        _ => guessed,
    }
}

// Automations: the phone manages them like the desktop, through the same rules.

async fn automations_list(State(ctx): State<Ctx>, headers: HeaderMap) -> Response {
    if let Err(r) = authed(&ctx, &headers) {
        return r;
    }
    match automations::list(&ctx.db) {
        Ok(list) => Json(json!({ "automations": list })).into_response(),
        Err(e) => fail(StatusCode::INTERNAL_SERVER_ERROR, e),
    }
}

async fn automation_one(State(ctx): State<Ctx>, headers: HeaderMap, UrlPath(id): UrlPath<String>) -> Response {
    if let Err(r) = authed(&ctx, &headers) {
        return r;
    }
    match automations::detail(&ctx.db, &id) {
        Ok(detail) => Json(detail).into_response(),
        Err(e) => fail(StatusCode::NOT_FOUND, e),
    }
}

async fn save_automation(ctx: &Ctx, id: Option<String>, draft: automations::Draft) -> Response {
    let manager = Arc::clone(&ctx.manager);
    off_thread(move || {
        let saved = automations::save(manager.db(), id.as_deref(), draft, |kind| manager.resolve_exe(kind).map(|_| ()))?;
        manager.automations_changed();
        Ok(json!({ "automation": saved }))
    })
    .await
}

async fn automation_create(State(ctx): State<Ctx>, headers: HeaderMap, Json(draft): Json<automations::Draft>) -> Response {
    if let Err(r) = authed(&ctx, &headers) {
        return r;
    }
    save_automation(&ctx, None, draft).await
}

async fn automation_update(State(ctx): State<Ctx>, headers: HeaderMap, UrlPath(id): UrlPath<String>, Json(draft): Json<automations::Draft>) -> Response {
    if let Err(r) = authed(&ctx, &headers) {
        return r;
    }
    save_automation(&ctx, Some(id), draft).await
}

async fn automation_delete(State(ctx): State<Ctx>, headers: HeaderMap, UrlPath(id): UrlPath<String>) -> Response {
    if let Err(r) = authed(&ctx, &headers) {
        return r;
    }
    match automations::delete(&ctx.db, &id) {
        Ok(()) => {
            ctx.manager.automations_changed();
            Json(json!({ "ok": true })).into_response()
        }
        Err(e) => fail(StatusCode::NOT_FOUND, e),
    }
}

async fn automation_run(State(ctx): State<Ctx>, headers: HeaderMap, UrlPath(id): UrlPath<String>) -> Response {
    if let Err(r) = authed(&ctx, &headers) {
        return r;
    }
    let manager = Arc::clone(&ctx.manager);
    off_thread(move || Ok(json!({ "run": automations::run_now(&manager, &id)? }))).await
}

#[derive(Deserialize)]
struct EnabledBody {
    enabled: bool,
}

async fn automation_enabled(State(ctx): State<Ctx>, headers: HeaderMap, UrlPath(id): UrlPath<String>, Json(body): Json<EnabledBody>) -> Response {
    if let Err(r) = authed(&ctx, &headers) {
        return r;
    }
    match automations::set_enabled(&ctx.db, &id, body.enabled) {
        Ok(saved) => {
            ctx.manager.automations_changed();
            Json(json!({ "automation": saved })).into_response()
        }
        Err(e) => fail(StatusCode::CONFLICT, e),
    }
}

#[derive(Deserialize)]
struct PreviewBody {
    schedule: String,
}

async fn automation_preview(State(ctx): State<Ctx>, headers: HeaderMap, Json(body): Json<PreviewBody>) -> Response {
    if let Err(r) = authed(&ctx, &headers) {
        return r;
    }
    match automations::preview(&body.schedule, db::now_ms()) {
        Ok(times) => Json(json!({ "times": times })).into_response(),
        Err(e) => fail(StatusCode::UNPROCESSABLE_ENTITY, e),
    }
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
    fn the_phone_app_files_get_the_types_browsers_need() {
        assert_eq!(content_type("m/manifest.webmanifest", "text/html".into()), "application/manifest+json");
        assert_eq!(content_type("service-worker.js", "application/octet-stream".into()), "text/javascript");
        assert_eq!(content_type("_app/immutable/entry/start.mjs", "text/html".into()), "text/javascript");
        assert_eq!(content_type("m/icons/icon-192.png", "image/png".into()), "image/png");
        assert_eq!(content_type("index.html", "text/html".into()), "text/html");
    }

    #[test]
    fn changed_files_are_listed_once_with_their_edit_count() {
        let row = |event: SessionEvent| db::EventRow { id: 0, session_id: "s".into(), at: 0, event };
        let edit = |path: &str| row(SessionEvent::FileChanged { path: path.into() });
        let events = [edit("src/b.rs"), row(SessionEvent::Message { text: "hi".into() }), edit("src/a.rs"), edit("src/b.rs")];
        assert_eq!(changed_files(&events), vec![("src/b.rs".to_string(), 2), ("src/a.rs".to_string(), 1)]);
    }

    #[test]
    fn screen_render_keeps_the_last_frame() {
        let raw = "first line\r\n\x1b[2J\x1b[Hsecond frame";
        assert_eq!(render_screen(raw), "second frame");
    }
}
