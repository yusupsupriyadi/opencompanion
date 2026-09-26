pub mod actions;
pub mod cli;
pub mod companion;
pub mod db;
pub mod events;
pub mod headless;
pub mod models;
pub mod monitor;
pub mod orchestrator;
pub mod proc;
pub mod projects;
pub mod pty;
pub mod session;
pub mod skills;
pub mod transcript;
pub mod waiting;

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};

use serde::{Deserialize, Serialize};
use serde_json::json;
use tauri::{AppHandle, Emitter, Manager as _, RunEvent, State};
use tauri_plugin_notification::NotificationExt;

use crate::actions::{cli_args, detect_with_settings, ChatTurn, RunTaskInput};
use crate::cli::{CliInstall, CliKind};
use crate::companion::{Companion, CompanionStatus, Pairing};
use crate::db::{ChatMessage, ChatModel, ChatThread, Db, DispatchCard, EventRow, Mode, Project, SessionInfo, Settings, Task};
use crate::session::{Emit, StartRequest};

struct AppState {
    db: Arc<Db>,
    manager: Arc<session::Manager>,
    companion: Arc<Companion>,
    monitor: Arc<Mutex<monitor::Monitor>>,
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
    fn output(&self, id: &str, data: &str) {
        let _ = self.app.emit("session-output", json!({ "id": id, "data": data }));
    }
    fn event(&self, row: &EventRow) {
        let _ = self.app.emit("session-event", row);
        if let Some(c) = self.companion.get() {
            c.broadcast(json!({ "type": "event", "event": row }));
        }
    }
    fn tasks_changed(&self) {
        let _ = self.app.emit("tasks-changed", ());
        if let Some(c) = self.companion.get() {
            c.broadcast(json!({ "type": "tasks" }));
        }
    }
    fn chat_changed(&self, thread_id: &str) {
        let _ = self.app.emit("chat-changed", thread_id);
        if let Some(c) = self.companion.get() {
            c.broadcast(json!({ "type": "chat", "threadId": thread_id }));
        }
    }
    fn notify(&self, title: &str, body: &str, session_id: &str) {
        let _ = self
            .app
            .notification()
            .builder()
            .title(title)
            .body(body)
            .show();
        if let Some(c) = self.companion.get() {
            c.broadcast(json!({ "type": "notify", "title": title, "body": body, "sessionId": session_id }));
        }
    }
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
    let sessions = state.db.sessions(limit.unwrap_or(80))?;
    Ok(sessions
        .into_iter()
        .map(|info| {
            let marks = state.db.event_marks(&info.id, 40).unwrap_or_default();
            SessionView { info, marks }
        })
        .collect())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SessionDetail {
    session: SessionInfo,
    events: Vec<EventRow>,
    output: String,
    task: Option<Task>,
}

#[tauri::command]
fn get_session(state: State<'_, AppState>, id: String) -> Res<SessionDetail> {
    let session = state.db.session(&id)?.ok_or("Session not found.")?;
    let events = state.db.events(&id, 400)?;
    let output = if session.mode == Mode::Interactive {
        state.manager.output(&id)
    } else {
        String::new()
    };
    let task = state.db.task_for_session(&id)?;
    Ok(SessionDetail {
        session,
        events,
        output,
        task,
    })
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
async fn answer_session(state: State<'_, AppState>, id: String, allow: bool) -> Res<SessionInfo> {
    let manager = Arc::clone(&state.manager);
    blocking(move || manager.answer(&id, allow)).await
}

#[tauri::command]
async fn resume_session(state: State<'_, AppState>, id: String, cols: Option<u16>, rows: Option<u16>) -> Res<SessionInfo> {
    let manager = Arc::clone(&state.manager);
    blocking(move || manager.resume(&id, cols, rows)).await
}

/// Tells every screen and the phone that sessions are gone, and that Board cards lost their link.
fn announce_deleted(app: &AppHandle, manager: &session::Manager, companion: &Companion, ids: &[String]) {
    if ids.is_empty() {
        return;
    }
    for id in ids {
        let _ = app.emit("session-deleted", id);
    }
    manager.tasks_changed();
    companion.broadcast(json!({ "type": "resync" }));
}

/// Deletes every finished session with its events, terminal log and hook files.
#[tauri::command]
async fn delete_history(app: AppHandle, state: State<'_, AppState>) -> Res<()> {
    let manager = Arc::clone(&state.manager);
    let (gone, problems) = blocking(move || Ok(manager.delete_finished())).await?;
    announce_deleted(&app, &state.manager, &state.companion, &gone);
    match problems.first() {
        Some(first) => Err(first.clone()),
        None => Ok(()),
    }
}

#[tauri::command]
fn delete_session(app: AppHandle, state: State<'_, AppState>, id: String) -> Res<()> {
    state.manager.delete(&id)?;
    announce_deleted(&app, &state.manager, &state.companion, &[id]);
    Ok(())
}

/// Retention (PRD FR-62), run at start, every hour, and when Settings shortens it.
fn prune(app: &AppHandle, manager: &session::Manager, companion: &Companion, db: &Db) {
    let keep = db.settings().map(|s| s.keep_days).unwrap_or(0);
    let gone = manager.prune(keep);
    announce_deleted(app, manager, companion, &gone);
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
    let db = Arc::clone(&state.db);
    let data_dir = state.data_dir.clone();
    let own: HashSet<u32> = state.manager.own_pids().into_iter().collect();
    let turn = blocking(move || actions::chat_send(&db, &data_dir, &own, thread_id, &message)).await?;
    state.companion.broadcast(json!({ "type": "chat", "threadId": turn.thread.id }));
    Ok(turn)
}

/// Models and thinking levels the chat planner can use with `cli`.
#[tauri::command]
async fn chat_models(state: State<'_, AppState>, cli: CliKind) -> Res<models::ModelList> {
    let db = Arc::clone(&state.db);
    let work_dir = state.data_dir.join("planner");
    blocking(move || {
        let install = detect_with_settings(&db)
            .into_iter()
            .find(|c| c.kind == cli)
            .ok_or(format!("{} is not installed.", cli.label()))?;
        models::list(&install, &work_dir, &cli_args(&db.settings()?, cli))
    })
    .await
}

/// Keeps the planner's model and thinking level for `cli`. Only this entry of Settings changes.
#[tauri::command]
fn chat_set_model(state: State<'_, AppState>, cli: CliKind, model: String, effort: String) -> Res<Settings> {
    let mut settings = state.db.settings()?;
    let choice = ChatModel {
        model: model.trim().to_string(),
        effort: effort.trim().to_string(),
    };
    if choice == ChatModel::default() {
        settings.chat_models.remove(cli.bin());
    } else {
        settings.chat_models.insert(cli.bin().to_string(), choice);
    }
    state.db.save_settings(&settings)?;
    Ok(settings)
}

#[tauri::command]
async fn chat_update_card(state: State<'_, AppState>, message_id: String, card: DispatchCard) -> Res<ChatMessage> {
    let db = Arc::clone(&state.db);
    blocking(move || {
        let clis = detect_with_settings(&db);
        actions::with_card(&db, &message_id, &card.id.clone(), |c| {
            if c.state != "proposed" {
                return Err("This card already ran or was discarded.".into());
            }
            c.cli = card.cli;
            c.title = card.title.trim().to_string();
            c.folder = card.folder.trim().to_string();
            c.prompt = card.prompt.trim().to_string();
            c.mode = card.mode;
            orchestrator::validate(c, &clis);
            Ok(())
        })
    })
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

/// Returns the chat message, whose card now names the Board card it became.
#[tauri::command]
fn chat_card_to_board(state: State<'_, AppState>, message_id: String, card_id: String) -> Res<ChatMessage> {
    let (_, message) = actions::card_to_board(&state.db, &message_id, &card_id)?;
    state.manager.tasks_changed();
    chat_changed(&state, &message);
    Ok(message)
}

#[tauri::command]
fn chat_delete_thread(state: State<'_, AppState>, thread_id: String) -> Res<()> {
    state.db.delete_thread(&thread_id)
}

// Board

#[tauri::command]
fn list_tasks(state: State<'_, AppState>) -> Res<Vec<Task>> {
    state.db.tasks()
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TaskInput {
    id: Option<String>,
    title: String,
    notes: String,
    project: String,
    cli: Option<CliKind>,
    column: String,
}

#[tauri::command]
fn save_task(state: State<'_, AppState>, task: TaskInput) -> Res<Task> {
    let title = task.title.trim();
    if title.is_empty() {
        return Err("Give the card a title.".into());
    }
    if !actions::COLUMNS.contains(&task.column.as_str()) {
        return Err("Unknown column.".into());
    }
    let now = db::now_ms();
    let existing = match &task.id {
        Some(id) => state.db.task(id)?,
        None => None,
    };
    let position = match &existing {
        Some(e) if e.column == task.column => e.position,
        _ => state.db.next_position(&task.column)?,
    };
    let saved = Task {
        id: existing.as_ref().map(|e| e.id.clone()).unwrap_or_else(db::new_id),
        title: title.to_string(),
        notes: task.notes.trim().to_string(),
        project: task.project.trim().to_string(),
        cli: task.cli,
        column: task.column,
        position,
        session_id: existing.as_ref().and_then(|e| e.session_id.clone()),
        created_at: existing.as_ref().map(|e| e.created_at).unwrap_or(now),
        updated_at: now,
    };
    state.db.save_task(&saved)?;
    state.manager.tasks_changed();
    Ok(saved)
}

#[tauri::command]
fn move_task(state: State<'_, AppState>, id: String, column: String, before: Option<String>) -> Res<Task> {
    let task = actions::move_task(&state.db, &id, &column, before)?;
    state.manager.tasks_changed();
    Ok(task)
}

#[tauri::command]
fn delete_task(state: State<'_, AppState>, id: String) -> Res<()> {
    state.db.delete_task(&id)?;
    state.manager.tasks_changed();
    Ok(())
}

#[tauri::command]
async fn run_task(state: State<'_, AppState>, input: RunTaskInput) -> Res<SessionInfo> {
    let db = Arc::clone(&state.db);
    let manager = Arc::clone(&state.manager);
    let session = blocking(move || actions::run_task(&db, &manager, input)).await?;
    state.manager.tasks_changed();
    Ok(session)
}

// Settings and phone access

#[tauri::command]
fn get_settings(state: State<'_, AppState>) -> Res<Settings> {
    state.db.settings()
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
    state.db.save_settings(&settings)?;
    if before.text_size != settings.text_size {
        apply_text_size(&app, &settings);
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
    })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut builder = tauri::Builder::default();
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.unminimize();
                let _ = w.show();
                let _ = w.set_focus();
            }
        }));
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
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            let db = Arc::new(Db::open(&data_dir.join("opencompanion.db"))?);
            db.close_orphans()?;
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
                data_dir,
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            detect_clis,
            scan_external,
            outside_detail,
            scan_skills,
            list_sessions,
            get_session,
            session_usage,
            start_session,
            send_input,
            resize_session,
            stop_session,
            answer_session,
            resume_session,
            delete_history,
            delete_session,
            recent_projects,
            project_folders,
            default_project_roots,
            folder_exists,
            chat_threads,
            chat_history,
            chat_send,
            chat_update_card,
            chat_discard_card,
            chat_run_card,
            chat_card_to_board,
            chat_delete_thread,
            chat_models,
            chat_set_model,
            list_tasks,
            save_task,
            move_task,
            delete_task,
            run_task,
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
        if let RunEvent::Exit = event {
            if let Some(state) = handle.try_state::<AppState>() {
                state.manager.kill_all();
                state.companion.stop();
            }
        }
    });
}
