pub mod actions;
pub mod automations;
pub mod autostart;
pub mod ccs;
pub mod cli;
pub mod companion;
pub mod db;
pub mod events;
pub mod files;
pub mod git;
pub mod headless;
pub mod models;
pub mod monitor;
pub mod orchestrator;
pub mod paste;
pub mod pi;
pub mod proc;
pub mod projects;
pub mod pty;
pub mod session;
pub mod shell_env;
pub mod skills;
pub mod terminal;
pub mod transcript;
pub mod waiting;

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use serde::Serialize;
use serde_json::json;
use tauri::{AppHandle, Emitter, Manager as _, RunEvent, State};
#[cfg(target_os = "macos")]
use tauri_plugin_notification::NotificationExt;

use crate::actions::{detect_with_settings, ChatTurn};
use crate::cli::{CliInstall, CliKind};
use crate::companion::{Companion, CompanionStatus, Pairing};
use crate::db::{ChatMessage, ChatThread, Db, DispatchCard, EventRow, Project, SessionInfo, Settings};
use crate::session::{Emit, StartRequest};

struct AppState {
    db: Arc<Db>,
    manager: Arc<session::Manager>,
    companion: Arc<Companion>,
    monitor: Arc<Mutex<monitor::Monitor>>,
    terminals: Arc<terminal::Terminals>,
    /// The AI CLIs in each session's terminals, from the last process list read (`follow_clis`).
    clis: Arc<Mutex<HashMap<String, Vec<CliKind>>>>,
    data_dir: PathBuf,
}

/// Forwards manager output to the webview, the phone companion and OS notifications.
struct TauriEmit {
    app: AppHandle,
    companion: OnceLock<Arc<Companion>>,
}

impl Emit for TauriEmit {
    fn session(&self, info: &SessionInfo) {
        let _ = self.app.emit("session-updated", info);
        if let Some(c) = self.companion.get() {
            c.broadcast(json!({ "type": "session", "session": info }));
        }
    }
    fn output(&self, id: &str, data: &str, seq: u64) {
        let _ = self.app.emit("session-output", json!({ "id": id, "data": data, "seq": seq }));
    }
    fn event(&self, row: &EventRow) {
        let _ = self.app.emit("session-event", row);
        if let Some(c) = self.companion.get() {
            c.broadcast(json!({ "type": "event", "event": row }));
        }
    }
    fn chat_changed(&self, thread_id: &str) {
        let _ = self.app.emit("chat-changed", thread_id);
        if let Some(c) = self.companion.get() {
            c.broadcast(json!({ "type": "chat", "threadId": thread_id }));
        }
    }
    fn settings_changed(&self) {
        let _ = self.app.emit("settings-changed", ());
        if let Some(c) = self.companion.get() {
            c.broadcast(json!({ "type": "settings" }));
        }
    }
    fn automations_changed(&self) {
        let _ = self.app.emit("automations-changed", ());
        if let Some(c) = self.companion.get() {
            c.broadcast(json!({ "type": "automations" }));
        }
    }
    fn notify(&self, title: &str, body: &str, session_id: &str) {
        show_notification(&self.app, title, body, session_id);
        if let Some(c) = self.companion.get() {
            c.broadcast(json!({ "type": "notify", "title": title, "body": body, "sessionId": session_id }));
        }
    }
}

/// Forwards terminal output and state to the webview only: terminals never reach the phone.
struct TerminalEvents {
    app: AppHandle,
}

impl terminal::TerminalEmit for TerminalEvents {
    fn output(&self, id: &str, data: &str, seq: u64) {
        let _ = self.app.emit("terminal-output", json!({ "id": id, "data": data, "seq": seq }));
    }
    fn changed(&self, info: &terminal::TerminalInfo) {
        let _ = self.app.emit("terminal-changed", info);
    }
}

/// Shows the window again, from the tray, a second launch or a notification.
fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

/// Brings the window forward on the session a notification was about.
#[cfg(not(target_os = "macos"))]
fn open_session(app: &AppHandle, id: &str) {
    show_main(app);
    if !id.is_empty() {
        let _ = app.emit("open-session", id);
    }
}

/// The tray's menu in the UI language: open the window, or quit and stop the sessions.
fn tray_menu<R: tauri::Runtime, M: tauri::Manager<R>>(app: &M, lang: &str) -> tauri::Result<tauri::menu::Menu<R>> {
    use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
    let (open, quit) = if lang == "id" {
        ("Buka OpenCompanion", "Keluar dari OpenCompanion dan hentikan sesinya")
    } else {
        ("Open OpenCompanion", "Quit OpenCompanion and stop its sessions")
    };
    let open = MenuItem::with_id(app, "open", open, true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", quit, true, None::<&str>)?;
    Menu::with_items(app, &[&open, &PredefinedMenuItem::separator(app)?, &quit])
}

/// Whether the tray icon was created. A Linux desktop without an AppIndicator library has none.
static HAS_TRAY: AtomicBool = AtomicBool::new(false);

/// Whether a tray icon can be made here. On Linux the tray loads an AppIndicator library the
/// first time it is used and aborts the app when none is installed, so that is checked first,
/// with the names it tries.
#[cfg(target_os = "linux")]
fn tray_possible() -> bool {
    ["libayatana-appindicator3.so.1", "libappindicator3.so.1", "libayatana-appindicator3.so", "libappindicator3.so"]
        .iter()
        .any(|name| {
            let Ok(name) = std::ffi::CString::new(*name) else { return false };
            // SAFETY: dlopen only maps the library, and the handle is closed right away.
            let handle = unsafe { libc::dlopen(name.as_ptr(), libc::RTLD_LAZY) };
            if handle.is_null() {
                return false;
            }
            unsafe { libc::dlclose(handle) };
            true
        })
}

#[cfg(not(target_os = "linux"))]
fn tray_possible() -> bool {
    true
}

/// The tray icon (PRD FR-18): a click opens the window; its menu opens it or quits, and quitting
/// is what stops the sessions.
fn tray(app: &tauri::App, lang: &str) -> tauri::Result<()> {
    use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
    let menu = tray_menu(app, lang)?;
    let mut icon = TrayIconBuilder::with_id("main")
        .tooltip("OpenCompanion")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "open" => show_main(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main(tray.app_handle());
            }
        });
    if let Some(image) = app.default_window_icon() {
        icon = icon.icon(image.clone());
    }
    icon.build(app)?;
    Ok(())
}

/// Closing the window hides it while Settings keeps the app in the tray, so sessions and the phone
/// connection carry on. The first time, a notification says where the app went.
fn on_close(window: &tauri::Window, api: &tauri::CloseRequestApi) {
    let app = window.app_handle();
    let Some(state) = app.try_state::<AppState>() else { return };
    let Ok(mut settings) = state.db.settings() else { return };
    // Without a tray icon a hidden window could not be opened again, so closing quits.
    if window.label() != "main" || !settings.close_to_tray || !HAS_TRAY.load(Ordering::SeqCst) {
        return;
    }
    api.prevent_close();
    let _ = window.hide();
    if !settings.tray_hint_shown {
        settings.tray_hint_shown = true;
        let _ = state.db.save_settings(&settings);
        let (title, body) = if settings.language == "id" {
            ("OpenCompanion masih berjalan", "Sesi tetap berjalan di tray. Klik ikonnya untuk membuka jendela, atau keluar dari menunya.")
        } else {
            ("OpenCompanion is still running", "Sessions keep going in the tray. Click its icon to open the window, or quit from its menu.")
        };
        show_notification(app, title, body, "");
    }
}

/// An OS notification whose click opens its session (PRD FR-40). A Windows toast and a Linux
/// notification (D-Bus) report the click while they are on screen, so they come straight from
/// notify-rust. On macOS the notification plugin shows them, without a click.
#[cfg(not(target_os = "macos"))]
fn show_notification(app: &AppHandle, title: &str, body: &str, session_id: &str) {
    use notify_rust::{Notification, NotificationResponse};
    let mut toast = Notification::new();
    toast.summary(title).body(body).auto_icon();
    #[cfg(windows)]
    {
        // As the plugin does: only the installed app's id is registered with Windows. A build run
        // from `target` shows its toasts under PowerShell's id instead.
        let from_build = std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(|d| d.to_path_buf()))
            .is_some_and(|dir| dir.ends_with("target/debug") || dir.ends_with("target/release"));
        if !from_build {
            toast.app_id(&app.config().identifier);
        }
    }
    #[cfg(not(windows))]
    {
        // A notification server reports a click on the body only as the "default" action.
        let lang = app.try_state::<AppState>().and_then(|s| s.db.settings().ok()).map(|s| s.language);
        let open = if lang.as_deref() == Some("id") { "Buka" } else { "Open" };
        toast.appname("OpenCompanion").icon("opencompanion").action("default", open);
    }
    let (app, id) = (app.clone(), session_id.to_string());
    std::thread::spawn(move || {
        let Ok(shown) = toast.show() else { return };
        // Waits until the toast is clicked or leaves the screen; a toast that already left has
        // nothing more to report.
        let _ = shown.wait_for_response(|response: &NotificationResponse| {
            if matches!(response, NotificationResponse::Default | NotificationResponse::Action(_)) {
                open_session(&app, &id);
            }
        });
    });
}

#[cfg(target_os = "macos")]
fn show_notification(app: &AppHandle, title: &str, body: &str, _session_id: &str) {
    let _ = app.notification().builder().title(title).body(body).show();
}

type Res<T> = Result<T, String>;

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> Res<T> + Send + 'static) -> Res<T> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| e.to_string())?
}

// CLIs and monitoring

#[tauri::command]
async fn detect_clis(state: State<'_, AppState>) -> Res<Vec<CliInstall>> {
    let db = Arc::clone(&state.db);
    blocking(move || Ok(detect_with_settings(&db))).await
}

#[tauri::command]
async fn scan_external(state: State<'_, AppState>) -> Res<Vec<monitor::ExternalSession>> {
    let own: HashSet<u32> = state.manager.own_pids().into_iter().collect();
    let monitor = Arc::clone(&state.monitor);
    blocking(move || Ok(monitor.lock().map_err(|e| e.to_string())?.scan(&own))).await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct OutsideDetail {
    session: monitor::ExternalSession,
    transcript: transcript::Transcript,
}

/// A CLI opened outside OpenCompanion, with the last messages of the history it keeps (PRD FR-32).
#[tauri::command]
async fn outside_detail(pid: u32) -> Res<OutsideDetail> {
    blocking(move || {
        let session = monitor::find(pid).ok_or("This process has ended.")?;
        let transcript = transcript::read(session.kind, session.cwd.as_deref(), session.started_at);
        Ok(OutsideDetail { session, transcript })
    })
    .await
}

#[tauri::command]
async fn scan_skills() -> Res<skills::SkillScan> {
    blocking(skills::scan).await
}

// Sessions

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SessionView {
    #[serde(flatten)]
    info: SessionInfo,
    /// `[at, kind]` pairs for the horizon track, newest first.
    marks: Vec<(i64, String)>,
}

#[tauri::command]
fn list_sessions(state: State<'_, AppState>, limit: Option<u32>) -> Res<Vec<SessionView>> {
    // Without a limit it returns every session: the sidebar lists them all.
    let sessions = state.db.sessions(limit.unwrap_or(u32::MAX))?;
    Ok(sessions
        .into_iter()
        .map(|info| {
            let marks = state.db.event_marks(&info.id, 40).unwrap_or_default();
            SessionView { info, marks }
        })
        .collect())
}

/// PRD FR-35: sessions matching the history screen's search, with their horizon marks.
#[tauri::command]
fn search_sessions(state: State<'_, AppState>, query: db::SessionQuery) -> Res<Vec<SessionView>> {
    Ok(state
        .db
        .search_sessions(&query)?
        .into_iter()
        .map(|info| {
            let marks = state.db.event_marks(&info.id, 40).unwrap_or_default();
            SessionView { info, marks }
        })
        .collect())
}

#[tauri::command]
fn session_folders(state: State<'_, AppState>) -> Res<Vec<String>> {
    state.db.session_folders()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SessionDetail {
    session: SessionInfo,
    events: Vec<EventRow>,
}

#[tauri::command]
fn get_session(state: State<'_, AppState>, id: String) -> Res<SessionDetail> {
    let session = state.db.session(&id)?.ok_or("Session not found.")?;
    let events = state.db.events(&id, 400)?;
    Ok(SessionDetail { session, events })
}

#[derive(Serialize)]
struct OutputSnapshot {
    data: String,
    seq: u64,
}

/// What the terminal view writes before live output: read after it starts listening, so no
/// chunk falls between the two (see `Manager::output_snapshot`).
#[tauri::command]
fn session_output(state: State<'_, AppState>, id: String) -> OutputSnapshot {
    let (data, seq) = state.manager.output_snapshot(&id);
    OutputSnapshot { data, seq }
}

/// CPU and memory of a running session's CLI and the processes it started (PRD FR-34).
#[tauri::command]
async fn session_usage(state: State<'_, AppState>, id: String) -> Res<Option<monitor::Usage>> {
    let Some(pid) = state.db.session(&id)?.filter(|s| s.status.is_live()).and_then(|s| s.pid) else {
        return Ok(None);
    };
    let monitor = Arc::clone(&state.monitor);
    blocking(move || Ok(monitor.lock().map_err(|e| e.to_string())?.usage(pid))).await
}

/// The AI CLIs running in each session's terminals: its own and the shells in its tabs. Sessions
/// with none are left out.
#[tauri::command]
fn session_clis(state: State<'_, AppState>) -> Res<HashMap<String, Vec<CliKind>>> {
    Ok(state.clis.lock().map_err(|e| e.to_string())?.clone())
}

/// How often the process list is read for the CLIs in sessions' terminals.
const CLIS_EVERY: std::time::Duration = std::time::Duration::from_secs(2);

/// Reads which AI CLIs run in each session's terminals, keeps the counts for the sidebar, and
/// lets the manager mark a session whose terminals run none as being at its shell prompt.
fn follow_clis(state: &AppState) {
    let mut roots: Vec<((String, bool), u32)> = state.manager.live_pids().into_iter().map(|(id, pid)| ((id, true), pid)).collect();
    roots.extend(state.terminals.list().into_iter().filter_map(|t| Some(((t.session_id, false), t.pid?))));
    let Ok(found) = state.monitor.lock().map(|mut m| m.clis_under(&roots)) else { return };
    let (mut own, mut any) = (HashSet::new(), HashSet::new());
    let mut counts: HashMap<String, Vec<CliKind>> = HashMap::new();
    for ((id, is_own), kinds) in found {
        if is_own {
            own.insert(id.clone());
        }
        any.insert(id.clone());
        counts.entry(id).or_default().extend(kinds);
    }
    if let Ok(mut c) = state.clis.lock() {
        *c = counts;
    }
    state.manager.sync_clis(&own, &any);
}

#[tauri::command]
async fn start_session(state: State<'_, AppState>, req: StartRequest) -> Res<SessionInfo> {
    let manager = Arc::clone(&state.manager);
    blocking(move || manager.start(req)).await
}

#[tauri::command]
async fn send_input(state: State<'_, AppState>, id: String, text: String) -> Res<SessionInfo> {
    let manager = Arc::clone(&state.manager);
    blocking(move || manager.send_input(&id, &text)).await
}

#[tauri::command]
fn resize_session(state: State<'_, AppState>, id: String, cols: u16, rows: u16) -> Res<()> {
    state.manager.resize(&id, cols, rows)
}

#[tauri::command]
fn stop_session(state: State<'_, AppState>, id: String) -> Res<()> {
    state.manager.stop(&id)
}

#[tauri::command]
fn mark_session_done(state: State<'_, AppState>, id: String) -> Res<()> {
    state.manager.mark_done(&id)
}

#[tauri::command]
async fn answer_session(state: State<'_, AppState>, id: String, allow: bool) -> Res<SessionInfo> {
    let manager = Arc::clone(&state.manager);
    blocking(move || manager.answer(&id, allow)).await
}

#[tauri::command]
async fn resume_session(state: State<'_, AppState>, id: String, cols: Option<u16>, rows: Option<u16>) -> Res<SessionInfo> {
    let manager = Arc::clone(&state.manager);
    blocking(move || manager.resume(&id, cols, rows)).await
}

/// Tells every screen and the phone that sessions are gone.
/// The shells opened from them end too, off this thread: killing a process tree takes a moment.
fn announce_deleted(app: &AppHandle, companion: &Companion, ids: &[String]) {
    if ids.is_empty() {
        return;
    }
    for id in ids {
        let _ = app.emit("session-deleted", id);
    }
    companion.broadcast(json!({ "type": "resync" }));
    if let Some(state) = app.try_state::<AppState>() {
        let (terminals, ids) = (Arc::clone(&state.terminals), ids.to_vec());
        std::thread::spawn(move || ids.iter().for_each(|id| terminals.close_session(id)));
    }
}

/// Deletes every finished session with its events, terminal log and hook files.
#[tauri::command]
async fn delete_history(app: AppHandle, state: State<'_, AppState>) -> Res<()> {
    let manager = Arc::clone(&state.manager);
    let (gone, problems) = blocking(move || Ok(manager.delete_finished())).await?;
    announce_deleted(&app, &state.companion, &gone);
    match problems.first() {
        Some(first) => Err(first.clone()),
        None => Ok(()),
    }
}

#[tauri::command]
fn delete_session(app: AppHandle, state: State<'_, AppState>, id: String) -> Res<()> {
    state.manager.delete(&id)?;
    announce_deleted(&app, &state.companion, &[id]);
    Ok(())
}

/// Retention (PRD FR-62), run at start, every hour, and when Settings shortens it.
fn prune(app: &AppHandle, manager: &session::Manager, companion: &Companion, db: &Db) {
    let keep = db.settings().map(|s| s.keep_days).unwrap_or(0);
    // The first run comes before the app state exists, and so before any shell was opened.
    let in_use = app.try_state::<AppState>().map(|s| s.terminals.sessions()).unwrap_or_default();
    let gone = manager.prune(keep, &in_use);
    announce_deleted(app, companion, &gone);
}

#[tauri::command]
fn recent_projects(state: State<'_, AppState>) -> Res<Vec<Project>> {
    state.db.projects(12)
}

/// Project folders found on this computer (recent, open, Claude Code history, project roots).
#[tauri::command]
async fn project_folders(state: State<'_, AppState>) -> Res<Vec<projects::ProjectFolder>> {
    let db = Arc::clone(&state.db);
    let own: HashSet<u32> = state.manager.own_pids().into_iter().collect();
    blocking(move || {
        let outside = monitor::Monitor::new().scan(&own);
        Ok(orchestrator::gather(&db, &outside)?.folders)
    })
    .await
}

/// The roots scanned for projects when Settings lists none.
#[tauri::command]
fn default_project_roots() -> Vec<String> {
    projects::default_roots().into_iter().map(|p| p.display().to_string()).collect()
}

#[tauri::command]
fn folder_exists(path: String) -> bool {
    !path.trim().is_empty() && std::path::Path::new(path.trim()).is_dir()
}

// A session's folder: files and git for the side panel. Desktop only; the phone never reads files.

/// The folder a session runs in, which every path below is relative to.
fn session_folder(state: &AppState, id: &str) -> Res<PathBuf> {
    let session = state.db.session(id)?.ok_or("Session not found.")?;
    let dir = PathBuf::from(&session.cwd);
    if !dir.is_dir() {
        return Err(format!("The folder {} does not exist.", session.cwd));
    }
    Ok(dir)
}

#[tauri::command]
async fn folder_list(state: State<'_, AppState>, id: String, dir: String) -> Res<files::Listing> {
    let root = session_folder(&state, &id)?;
    blocking(move || files::list(&root, &dir)).await
}

/// Files whose path holds the query, or with `contents` the lines that hold it.
#[tauri::command]
async fn folder_find(state: State<'_, AppState>, id: String, query: String, contents: bool) -> Res<files::Found> {
    let root = session_folder(&state, &id)?;
    blocking(move || Ok(if contents { files::find_text(&root, &query) } else { files::find_names(&root, &query) })).await
}

#[tauri::command]
async fn folder_read(state: State<'_, AppState>, id: String, path: String) -> Res<files::FileText> {
    let root = session_folder(&state, &id)?;
    blocking(move || files::read(&root, &path)).await
}

/// Raw bytes, not JSON: an image or a PDF reaches the viewer without being encoded on the way.
#[tauri::command]
async fn folder_read_bytes(state: State<'_, AppState>, id: String, path: String) -> Res<tauri::ipc::Response> {
    let root = session_folder(&state, &id)?;
    blocking(move || files::read_bytes(&root, &path)).await.map(tauri::ipc::Response::new)
}

#[tauri::command]
async fn git_status(state: State<'_, AppState>, id: String) -> Res<git::Status> {
    let root = session_folder(&state, &id)?;
    blocking(move || git::status(&root)).await
}

#[tauri::command]
async fn git_diff(state: State<'_, AppState>, id: String, path: String, old_path: Option<String>, untracked: bool) -> Res<git::Diff> {
    let root = session_folder(&state, &id)?;
    blocking(move || git::diff(&root, &path, old_path.as_deref(), untracked)).await
}

#[tauri::command]
async fn git_branches(state: State<'_, AppState>, id: String) -> Res<git::Branches> {
    let root = session_folder(&state, &id)?;
    blocking(move || git::branches(&root)).await
}

/// Refused while any OpenCompanion session in the folder runs its CLI: the CLI holds the files
/// as they are on this branch.
#[tauri::command]
async fn git_switch(state: State<'_, AppState>, id: String, branch: String) -> Res<git::Status> {
    let root = session_folder(&state, &id)?;
    let folder = projects::norm(&root.display().to_string());
    let busy = state
        .db
        .sessions(u32::MAX)?
        .into_iter()
        .any(|s| s.status.runs_cli() && projects::norm(&s.cwd) == folder);
    if busy {
        return Err("Stop or finish the sessions in this folder before switching branches.".into());
    }
    blocking(move || git::switch(&root, &branch)).await
}

// Terminals: plain shells in a session's folder, in tabs beside its own terminal.

#[tauri::command]
async fn terminal_shells() -> Res<Vec<terminal::Shell>> {
    blocking(|| Ok(terminal::available_shells())).await
}

/// One session's shells, in tab order.
#[tauri::command]
fn terminal_list(state: State<'_, AppState>, session_id: String) -> Vec<terminal::TerminalInfo> {
    state.terminals.list().into_iter().filter(|t| t.session_id == session_id).collect()
}

#[tauri::command]
async fn terminal_open(state: State<'_, AppState>, session_id: String, shell: Option<String>, cols: Option<u16>, rows: Option<u16>) -> Res<terminal::TerminalInfo> {
    let cwd = session_folder(&state, &session_id)?.display().to_string();
    let terminals = Arc::clone(&state.terminals);
    blocking(move || terminals.open(&session_id, &cwd, shell.as_deref(), cols, rows)).await
}

#[tauri::command]
fn terminal_write(state: State<'_, AppState>, id: String, data: String) -> Res<()> {
    state.terminals.write(&id, &data)
}

#[tauri::command]
fn terminal_resize(state: State<'_, AppState>, id: String, cols: u16, rows: u16) -> Res<()> {
    state.terminals.resize(&id, cols, rows)
}

#[tauri::command]
fn terminal_output(state: State<'_, AppState>, id: String) -> OutputSnapshot {
    let (data, seq) = state.terminals.output_snapshot(&id);
    OutputSnapshot { data, seq }
}

#[tauri::command]
async fn terminal_restart(state: State<'_, AppState>, id: String, cols: Option<u16>, rows: Option<u16>) -> Res<terminal::TerminalInfo> {
    let terminals = Arc::clone(&state.terminals);
    blocking(move || terminals.restart(&id, cols, rows)).await
}

#[tauri::command]
async fn terminal_close(state: State<'_, AppState>, id: String) -> Res<()> {
    let terminals = Arc::clone(&state.terminals);
    blocking(move || terminals.close(&id)).await
}

/// An image pasted into a session or terminal, as the raw body with its type in `x-image-type`.
/// Returns the saved file's path for the terminal to paste.
#[tauri::command]
fn save_pasted_image(request: tauri::ipc::Request<'_>) -> Res<String> {
    let tauri::ipc::InvokeBody::Raw(bytes) = request.body() else {
        return Err("The pasted image is empty.".into());
    };
    let mime = request.headers().get("x-image-type").and_then(|v| v.to_str().ok()).unwrap_or_default();
    let dir = std::env::temp_dir().join("opencompanion-paste");
    paste::save_image(&dir, bytes, mime).map(|p| p.display().to_string())
}

// Chat

#[tauri::command]
fn chat_threads(state: State<'_, AppState>) -> Res<Vec<ChatThread>> {
    state.db.threads()
}

#[tauri::command]
fn chat_history(state: State<'_, AppState>, thread_id: String) -> Res<Vec<ChatMessage>> {
    state.db.chat(&thread_id, 200)
}

/// Without `thread_id` the message starts a new thread, named after the message.
#[tauri::command]
async fn chat_send(state: State<'_, AppState>, thread_id: Option<String>, message: String) -> Res<ChatTurn> {
    let own: HashSet<u32> = state.manager.own_pids().into_iter().collect();
    let manager = Arc::clone(&state.manager);
    let turn = blocking(move || actions::chat_send(&manager, &own, thread_id, &message)).await?;
    state.manager.chat_changed(&turn.thread.id);
    Ok(turn)
}

/// Threads where the planner is answering now, from this window or from the phone.
#[tauri::command]
fn chat_answering() -> Vec<String> {
    actions::answering()
}

/// Models and thinking levels the chat planner can use with `cli`.
#[tauri::command]
async fn chat_models(state: State<'_, AppState>, cli: CliKind) -> Res<models::ModelList> {
    let db = Arc::clone(&state.db);
    let work_dir = state.data_dir.join("planner");
    blocking(move || actions::planner_models(&db, &work_dir, cli)).await
}

/// Keeps the planner's model and thinking level for `cli`. Only this entry of Settings changes.
#[tauri::command]
fn chat_set_model(state: State<'_, AppState>, cli: CliKind, model: String, effort: String) -> Res<Settings> {
    let settings = actions::set_chat_model(&state.db, cli, &model, &effort)?;
    // An open phone chat shows the same picker.
    state.companion.broadcast(json!({ "type": "settings" }));
    Ok(settings)
}

#[tauri::command]
async fn chat_update_card(state: State<'_, AppState>, message_id: String, card: DispatchCard) -> Res<ChatMessage> {
    let db = Arc::clone(&state.db);
    blocking(move || actions::update_card(&db, &message_id, card))
        .await
        .inspect(|m| chat_changed(&state, m))
}

/// The phone shows the same chats, so it hears about every change made here.
fn chat_changed(state: &AppState, message: &ChatMessage) {
    state.companion.broadcast(json!({ "type": "chat", "threadId": message.thread_id }));
}

#[tauri::command]
fn chat_discard_card(state: State<'_, AppState>, message_id: String, card_id: String, undo: Option<bool>) -> Res<ChatMessage> {
    actions::discard_card(&state.db, &message_id, &card_id, undo.unwrap_or(false)).inspect(|m| chat_changed(&state, m))
}

#[tauri::command]
async fn chat_run_card(state: State<'_, AppState>, message_id: String, card_id: String) -> Res<ChatMessage> {
    let db = Arc::clone(&state.db);
    let manager = Arc::clone(&state.manager);
    blocking(move || actions::run_card(&db, &manager, &message_id, &card_id))
        .await
        .inspect(|m| chat_changed(&state, m))
}

#[tauri::command]
fn chat_delete_thread(state: State<'_, AppState>, thread_id: String) -> Res<()> {
    actions::delete_thread(&state.db, &thread_id)?;
    state.companion.broadcast(json!({ "type": "chat", "threadId": thread_id }));
    Ok(())
}

// Automations

#[tauri::command]
fn list_automations(state: State<'_, AppState>) -> Res<Vec<automations::View>> {
    automations::list(&state.db)
}

#[tauri::command]
fn get_automation(state: State<'_, AppState>, id: String) -> Res<automations::Detail> {
    automations::detail(&state.db, &id)
}

#[tauri::command]
async fn save_automation(state: State<'_, AppState>, id: Option<String>, draft: automations::Draft) -> Res<db::Automation> {
    let manager = Arc::clone(&state.manager);
    blocking(move || {
        let saved = automations::save(manager.db(), id.as_deref(), draft, |kind| manager.resolve_exe(kind).map(|_| ()))?;
        manager.automations_changed();
        Ok(saved)
    })
    .await
}

#[tauri::command]
fn set_automation_enabled(state: State<'_, AppState>, id: String, enabled: bool) -> Res<db::Automation> {
    let saved = automations::set_enabled(&state.db, &id, enabled)?;
    state.manager.automations_changed();
    Ok(saved)
}

#[tauri::command]
fn delete_automation(state: State<'_, AppState>, id: String) -> Res<()> {
    automations::delete(&state.db, &id)?;
    state.manager.automations_changed();
    Ok(())
}

#[tauri::command]
async fn run_automation(state: State<'_, AppState>, id: String) -> Res<db::AutomationRun> {
    let manager = Arc::clone(&state.manager);
    blocking(move || automations::run_now(&manager, &id)).await
}

#[tauri::command]
fn preview_schedule(schedule: String) -> Res<Vec<i64>> {
    automations::preview(&schedule, db::now_ms())
}

// Settings and phone access

/// Settings, with the start at sign-in read from the system, where it can be changed outside the app.
#[tauri::command]
fn get_settings(state: State<'_, AppState>) -> Res<Settings> {
    let mut settings = state.db.settings()?;
    settings.start_at_login = autostart::enabled();
    Ok(settings)
}

/// Settings' text size is the webview zoom (PRD FR-66). The phone page is untouched: it follows the phone.
fn apply_text_size(app: &AppHandle, settings: &Settings) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.set_zoom(settings.zoom());
    }
}

#[tauri::command]
async fn save_settings(app: AppHandle, state: State<'_, AppState>, settings: Settings) -> Res<Settings> {
    let before = state.db.settings()?;
    // The system is asked first, so a refused change is never stored as done.
    if settings.start_at_login != autostart::enabled() {
        let on = settings.start_at_login;
        blocking(move || autostart::set(on)).await?;
    }
    state.db.save_settings(&settings)?;
    if before.text_size != settings.text_size {
        apply_text_size(&app, &settings);
    }
    if before.language != settings.language {
        if let Some(tray) = app.tray_by_id("main") {
            if let Ok(menu) = tray_menu(&app, &settings.language) {
                let _ = tray.set_menu(Some(menu));
            }
        }
    }
    if before.keep_days != settings.keep_days {
        let (handle, manager, companion, db) = (app.clone(), Arc::clone(&state.manager), Arc::clone(&state.companion), Arc::clone(&state.db));
        tauri::async_runtime::spawn_blocking(move || prune(&handle, &manager, &companion, &db));
    }
    let companion = Arc::clone(&state.companion);
    let restart = settings.companion_enabled
        && (!before.companion_enabled || before.companion_port != settings.companion_port || !companion.status().running);
    if restart {
        companion.start(settings.companion_port).await;
    } else if !settings.companion_enabled && before.companion_enabled {
        companion.stop();
    }
    // A phone chat shows the planner, its model and the auto-run folders from here.
    companion.broadcast(json!({ "type": "settings" }));
    Ok(settings)
}

#[tauri::command]
fn companion_status(state: State<'_, AppState>) -> CompanionStatus {
    state.companion.status()
}

#[tauri::command]
fn start_pairing(state: State<'_, AppState>) -> Res<Pairing> {
    state.companion.start_pairing()
}

#[tauri::command]
fn list_devices(state: State<'_, AppState>) -> Res<Vec<db::Device>> {
    state.db.devices()
}

/// The phone's live connection closes at once, not only its next request.
#[tauri::command]
fn remove_device(state: State<'_, AppState>, id: String) -> Res<()> {
    state.db.remove_device(&id)?;
    state.companion.revoke(Some(&id));
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AppInfo {
    version: String,
    data_dir: String,
    counts: HashMap<String, usize>,
    /// Whether Settings can offer the start at sign-in on this platform.
    can_start_at_login: bool,
}

#[tauri::command]
fn app_info(app: AppHandle, state: State<'_, AppState>) -> Res<AppInfo> {
    let mut counts = HashMap::new();
    counts.insert("sessions".into(), state.db.sessions(100_000)?.len());
    counts.insert("devices".into(), state.db.devices()?.len());
    Ok(AppInfo {
        version: app.package_info().version.to_string(),
        data_dir: state.data_dir.display().to_string(),
        counts,
        can_start_at_login: autostart::supported(),
    })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut builder = tauri::Builder::default();
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| show_main(app)));
    }
    let app = builder
        // The window draws its own title bar (`decorations: false`); a saved state from before that would turn the native frame back on.
        .plugin(
            tauri_plugin_window_state::Builder::default()
                .with_state_flags(tauri_plugin_window_state::StateFlags::all() & !tauri_plugin_window_state::StateFlags::DECORATIONS)
                .build(),
        )
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_dialog::init())
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                on_close(window, api);
            }
        })
        .setup(|app| {
            // Read the login shell's PATH now, so CLI detection and the first session find it ready.
            shell_env::start();
            // Started at sign-in: straight into the tray, without a window in the way.
            if std::env::args().any(|a| a == autostart::HIDDEN_ARG) {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.hide();
                }
            }
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            let db = Arc::new(Db::open(&data_dir.join("opencompanion.db"))?);
            db.close_orphans()?;
            if !tray_possible() {
                eprintln!("OpenCompanion runs without a tray icon: no AppIndicator library is installed");
            } else {
                match tray(app, &db.settings()?.language) {
                    Ok(()) => HAS_TRAY.store(true, Ordering::SeqCst),
                    Err(e) => eprintln!("OpenCompanion runs without a tray icon: {e}"),
                }
            }
            let emit = Arc::new(TauriEmit {
                app: app.handle().clone(),
                companion: OnceLock::new(),
            });
            let manager = session::Manager::new(Arc::clone(&db), emit.clone(), data_dir.clone());
            let resolver = app.handle().clone();
            let assets: companion::AssetLookup = Arc::new(move |path: &str| {
                resolver
                    .asset_resolver()
                    .get(path.to_string())
                    .map(|a| (a.bytes().to_vec(), a.mime_type().to_string()))
            });
            let companion = Companion::new(Arc::clone(&db), Arc::clone(&manager), assets);
            let _ = emit.companion.set(Arc::clone(&companion));
            if db.settings()?.companion_enabled {
                let c = Arc::clone(&companion);
                let port = db.settings()?.companion_port;
                tauri::async_runtime::spawn(async move {
                    c.start(port).await;
                });
            }
            apply_text_size(app.handle(), &db.settings()?);
            {
                let (handle, manager, companion, db) = (app.handle().clone(), Arc::clone(&manager), Arc::clone(&companion), Arc::clone(&db));
                std::thread::spawn(move || loop {
                    prune(&handle, &manager, &companion, &db);
                    std::thread::sleep(std::time::Duration::from_secs(3600));
                });
            }
            app.manage(AppState {
                db,
                manager,
                companion,
                monitor: Arc::new(Mutex::new(monitor::Monitor::new())),
                terminals: terminal::Terminals::new(Arc::new(TerminalEvents { app: app.handle().clone() })),
                clis: Arc::new(Mutex::new(HashMap::new())),
                data_dir,
            });
            {
                // The first pass waits one tick, so the login shell's PATH and the orphan cleanup
                // above are done before a missed automation starts.
                let manager = Arc::clone(&app.state::<AppState>().manager);
                std::thread::spawn(move || loop {
                    std::thread::sleep(std::time::Duration::from_secs(automations::TICK_SECS));
                    automations::tick(&manager, db::now_ms());
                });
            }
            {
                let handle = app.handle().clone();
                std::thread::spawn(move || loop {
                    std::thread::sleep(CLIS_EVERY);
                    if let Some(state) = handle.try_state::<AppState>() {
                        follow_clis(&state);
                    }
                });
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            detect_clis,
            scan_external,
            outside_detail,
            scan_skills,
            list_sessions,
            search_sessions,
            session_folders,
            get_session,
            session_output,
            session_usage,
            session_clis,
            start_session,
            send_input,
            resize_session,
            stop_session,
            mark_session_done,
            answer_session,
            resume_session,
            delete_history,
            delete_session,
            recent_projects,
            project_folders,
            default_project_roots,
            folder_exists,
            folder_list,
            folder_find,
            folder_read,
            folder_read_bytes,
            git_status,
            git_diff,
            git_branches,
            git_switch,
            terminal_shells,
            terminal_list,
            terminal_open,
            terminal_write,
            terminal_resize,
            terminal_output,
            terminal_restart,
            terminal_close,
            save_pasted_image,
            chat_threads,
            chat_history,
            chat_send,
            chat_answering,
            chat_update_card,
            chat_discard_card,
            chat_run_card,
            chat_delete_thread,
            chat_models,
            chat_set_model,
            list_automations,
            get_automation,
            save_automation,
            set_automation_enabled,
            delete_automation,
            run_automation,
            preview_schedule,
            get_settings,
            save_settings,
            companion_status,
            start_pairing,
            list_devices,
            remove_device,
            app_info,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|handle, event| {
        // Clicking the Dock icon brings a window hidden in the menu bar back.
        #[cfg(target_os = "macos")]
        if let RunEvent::Reopen { .. } = event {
            show_main(handle);
        }
        if let RunEvent::Exit = event {
            if let Some(state) = handle.try_state::<AppState>() {
                state.manager.kill_all();
                state.terminals.kill_all();
                state.companion.stop();
            }
        }
    });
}
