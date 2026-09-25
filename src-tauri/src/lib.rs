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
pub mod waiting;

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};

use serde::{Deserialize, Serialize};
use serde_json::json;
use tauri::{AppHandle, Emitter, Manager as _, RunEvent, State};
use tauri_plugin_notification::NotificationExt;

use crate::cli::{CliInstall, CliKind};
use crate::companion::{Companion, CompanionStatus, Pairing};
use crate::db::{ChatMessage, ChatModel, ChatThread, Db, DispatchCard, EventRow, Mode, PlannerSource, Project, SessionInfo, Settings, Task};
use crate::session::{Emit, StartRequest};

struct AppState {
    db: Arc<Db>,
    manager: Arc<session::Manager>,
    companion: Arc<Companion>,
    monitor: Mutex<monitor::Monitor>,
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

fn detect_with_settings(db: &Db) -> Vec<CliInstall> {
    let settings = db.settings().unwrap_or_default();
    let mut found = cli::detect_all();
    for install in &mut found {
        if let Some(custom) = settings.cli_paths.get(install.kind.bin()).filter(|p| !p.trim().is_empty()) {
            let path = PathBuf::from(custom);
            *install = if path.is_file() {
                cli::detect_at(install.kind, Some(path))
            } else {
                let mut missing = cli::detect_at(install.kind, None);
                missing.error = Some(format!("The custom path does not exist: {custom}"));
                missing
            };
        }
    }
    found
}

#[tauri::command]
async fn detect_clis(state: State<'_, AppState>) -> Res<Vec<CliInstall>> {
    let db = Arc::clone(&state.db);
    blocking(move || Ok(detect_with_settings(&db))).await
}

#[tauri::command]
fn scan_external(state: State<'_, AppState>) -> Res<Vec<monitor::ExternalSession>> {
    let own: HashSet<u32> = state.manager.own_pids().into_iter().collect();
    let mut monitor = state.monitor.lock().map_err(|e| e.to_string())?;
    Ok(monitor.scan(&own))
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

#[tauri::command]
fn delete_history(state: State<'_, AppState>) -> Res<()> {
    state.db.delete_history()
}

#[tauri::command]
fn delete_session(app: AppHandle, state: State<'_, AppState>, id: String) -> Res<()> {
    state.manager.delete(&id)?;
    let _ = app.emit("session-deleted", &id);
    let _ = app.emit("tasks-changed", ());
    state.companion.broadcast(json!({ "type": "resync" }));
    Ok(())
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

#[derive(Serialize)]
struct ChatTurn {
    thread: ChatThread,
    user: ChatMessage,
    reply: ChatMessage,
}

/// Without `thread_id` the message starts a new thread, named after the message.
#[tauri::command]
async fn chat_send(state: State<'_, AppState>, thread_id: Option<String>, message: String) -> Res<ChatTurn> {
    let text = message.trim().to_string();
    if text.is_empty() {
        return Err("Write a message first.".into());
    }
    let db = Arc::clone(&state.db);
    let data_dir = state.data_dir.clone();
    let own: HashSet<u32> = state.manager.own_pids().into_iter().collect();
    blocking(move || {
        let mut thread = match thread_id {
            Some(id) => db.thread(&id)?.ok_or("This chat was deleted. Start a new chat.")?,
            None => db.create_thread(&db::thread_title(&text))?,
        };
        let history = db.chat(&thread.id, 40)?;
        let msg = |role: &str, text: String, cards: Vec<DispatchCard>| ChatMessage {
            id: db::new_id(),
            thread_id: thread.id.clone(),
            role: role.into(),
            text,
            cards,
            created_at: db::now_ms(),
        };
        let user = msg("user", text.clone(), vec![]);
        db.add_chat(&user)?;

        let clis = detect_with_settings(&db);
        let settings = db.settings()?;
        let chat_cli = settings
            .chat_cli
            .filter(|k| clis.iter().any(|c| c.kind == *k && c.path.is_some()))
            .or_else(|| clis.iter().find(|c| c.path.is_some() && c.kind != CliKind::Gemini).map(|c| c.kind));
        let reply = match chat_cli {
            // A custom provider plans without any CLI, and without file access.
            _ if settings.planner_source == PlannerSource::Api => {
                let api = &settings.planner_api;
                let outside = monitor::Monitor::new().scan(&own);
                let ctx = orchestrator::gather(&db, &outside)?;
                let input = orchestrator::PlanInput {
                    message: &text,
                    history: &history,
                    clis: &clis,
                    folders: &ctx.folders,
                    sessions: &ctx.sessions,
                    outside: &outside,
                    read_dirs: &[],
                };
                match orchestrator::run_api(api, &input) {
                    Ok(plan) => msg("planner", plan.reply, plan.cards),
                    Err(e) => {
                        let who = if api.model.trim().is_empty() { "The custom provider" } else { api.model.trim() };
                        msg("error", format!("{who} could not answer: {e}"), vec![])
                    }
                }
            }
            None => msg(
                "error",
                "No CLI is installed that can plan tasks. Install Claude Code, Codex CLI or OpenCode, then rescan on the CLIs screen.".into(),
                vec![],
            ),
            Some(kind) => {
                let exe = clis
                    .iter()
                    .find(|c| c.kind == kind)
                    .and_then(|c| c.path.clone())
                    .ok_or("The chat CLI is not installed.")?;
                let outside = monitor::Monitor::new().scan(&own);
                let mut ctx = orchestrator::gather(&db, &outside)?;
                // A folder the owner @-mentioned is readable too, wherever it lives.
                if settings.planner_can_read {
                    for dir in orchestrator::mentioned_dirs(&text) {
                        if !ctx.read_dirs.iter().any(|r| dir.starts_with(r)) {
                            ctx.read_dirs.push(dir);
                        }
                    }
                }
                let mut extra = cli_args(&settings, kind);
                if let Some(choice) = settings.chat_models.get(kind.bin()) {
                    extra.extend(models::args(kind, &choice.model, &choice.effort));
                }
                let input = orchestrator::PlanInput {
                    message: &text,
                    history: &history,
                    clis: &clis,
                    folders: &ctx.folders,
                    sessions: &ctx.sessions,
                    outside: &outside,
                    read_dirs: &ctx.read_dirs,
                };
                match orchestrator::run(kind, &exe, &data_dir.join("planner"), &extra, &input) {
                    Ok(plan) => msg("planner", plan.reply, plan.cards),
                    Err(e) => msg("error", format!("{} could not answer: {e}", kind.label()), vec![]),
                }
            }
        };
        db.add_chat(&reply)?;
        thread.updated_at = reply.created_at;
        db.touch_thread(&thread.id, thread.updated_at)?;
        Ok(ChatTurn { thread, user, reply })
    })
    .await
}

/// The extra arguments Settings keeps for `kind`.
fn cli_args(settings: &Settings, kind: CliKind) -> Vec<String> {
    settings
        .cli_args
        .get(kind.bin())
        .map(|a| a.split_whitespace().map(str::to_owned).collect())
        .unwrap_or_default()
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

fn with_card(db: &Db, message_id: &str, card_id: &str, f: impl FnOnce(&mut DispatchCard) -> Res<()>) -> Res<ChatMessage> {
    let mut msg = db.chat_message(message_id)?.ok_or("Message not found.")?;
    let card = msg.cards.iter_mut().find(|c| c.id == card_id).ok_or("Card not found.")?;
    f(card)?;
    db.add_chat(&msg)?;
    Ok(msg)
}

#[tauri::command]
async fn chat_update_card(state: State<'_, AppState>, message_id: String, card: DispatchCard) -> Res<ChatMessage> {
    let db = Arc::clone(&state.db);
    blocking(move || {
        let clis = detect_with_settings(&db);
        with_card(&db, &message_id, &card.id.clone(), |c| {
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
}

#[tauri::command]
fn chat_discard_card(state: State<'_, AppState>, message_id: String, card_id: String, undo: Option<bool>) -> Res<ChatMessage> {
    let restore = undo.unwrap_or(false);
    with_card(&state.db, &message_id, &card_id, |c| {
        c.state = if restore { "proposed".into() } else { "discarded".into() };
        Ok(())
    })
}

#[tauri::command]
async fn chat_run_card(state: State<'_, AppState>, message_id: String, card_id: String) -> Res<ChatMessage> {
    let db = Arc::clone(&state.db);
    let manager = Arc::clone(&state.manager);
    blocking(move || {
        let clis = detect_with_settings(&db);
        let msg = db.chat_message(&message_id)?.ok_or("Message not found.")?;
        let mut card = msg.cards.iter().find(|c| c.id == card_id).cloned().ok_or("Card not found.")?;
        orchestrator::validate(&mut card, &clis);
        if let Some(problem) = card.problem {
            return Err(problem);
        }
        if card.state != "proposed" {
            return Err("This card already ran or was discarded.".into());
        }
        let session = manager.start(StartRequest {
            cli: card.cli,
            cwd: card.folder.clone(),
            mode: card.mode,
            prompt: card.prompt.clone(),
            title: Some(card.title.clone()),
            permission_mode: None,
            source: Some("chat".into()),
            task_id: None,
            cols: None,
            rows: None,
        })?;
        with_card(&db, &message_id, &card_id, |c| {
            c.state = "started".into();
            c.session_id = Some(session.id.clone());
            Ok(())
        })
    })
    .await
}

#[tauri::command]
fn chat_card_to_board(app: AppHandle, state: State<'_, AppState>, message_id: String, card_id: String) -> Res<Task> {
    let msg = state.db.chat_message(&message_id)?.ok_or("Message not found.")?;
    let card = msg.cards.iter().find(|c| c.id == card_id).ok_or("Card not found.")?;
    let now = db::now_ms();
    let task = Task {
        id: db::new_id(),
        title: Some(card.title.trim())
            .filter(|t| !t.is_empty())
            .or_else(|| card.prompt.lines().next())
            .unwrap_or("Task from chat")
            .chars()
            .take(120)
            .collect(),
        notes: card.prompt.clone(),
        project: card.folder.clone(),
        cli: Some(card.cli),
        column: "todo".into(),
        position: state.db.next_position("todo")?,
        session_id: None,
        created_at: now,
        updated_at: now,
    };
    state.db.save_task(&task)?;
    let _ = app.emit("tasks-changed", ());
    Ok(task)
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

const COLUMNS: [&str; 4] = ["pending", "todo", "progress", "done"];

#[tauri::command]
fn save_task(app: AppHandle, state: State<'_, AppState>, task: TaskInput) -> Res<Task> {
    let title = task.title.trim();
    if title.is_empty() {
        return Err("Give the card a title.".into());
    }
    if !COLUMNS.contains(&task.column.as_str()) {
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
    let _ = app.emit("tasks-changed", ());
    Ok(saved)
}

#[tauri::command]
fn move_task(app: AppHandle, state: State<'_, AppState>, id: String, column: String, before: Option<String>) -> Res<Task> {
    if !COLUMNS.contains(&column.as_str()) {
        return Err("Unknown column.".into());
    }
    let mut task = state.db.task(&id)?.ok_or("Card not found.")?;
    task.position = match before.and_then(|b| state.db.task(&b).ok().flatten()) {
        Some(b) if b.column == column => {
            let prev = state
                .db
                .tasks()?
                .into_iter()
                .filter(|t| t.column == column && t.position < b.position && t.id != task.id)
                .map(|t| t.position)
                .fold(b.position - 1.0, f64::max);
            (prev + b.position) / 2.0
        }
        _ => state.db.next_position(&column)?,
    };
    task.column = column;
    task.updated_at = db::now_ms();
    state.db.save_task(&task)?;
    let _ = app.emit("tasks-changed", ());
    Ok(task)
}

#[tauri::command]
fn delete_task(app: AppHandle, state: State<'_, AppState>, id: String) -> Res<()> {
    state.db.delete_task(&id)?;
    let _ = app.emit("tasks-changed", ());
    Ok(())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RunTaskInput {
    id: String,
    cli: CliKind,
    cwd: String,
    mode: Mode,
    prompt: String,
    permission_mode: Option<String>,
}

#[tauri::command]
async fn run_task(app: AppHandle, state: State<'_, AppState>, input: RunTaskInput) -> Res<SessionInfo> {
    let db = Arc::clone(&state.db);
    let manager = Arc::clone(&state.manager);
    blocking(move || {
        let mut task = db.task(&input.id)?.ok_or("Card not found.")?;
        let session = manager.start(StartRequest {
            cli: input.cli,
            cwd: input.cwd.clone(),
            mode: input.mode,
            prompt: input.prompt.clone(),
            title: Some(task.title.clone()),
            permission_mode: input.permission_mode.clone(),
            source: Some("board".into()),
            task_id: Some(task.id.clone()),
            cols: None,
            rows: None,
        })?;
        task.session_id = Some(session.id.clone());
        task.cli = Some(input.cli);
        task.project = input.cwd.trim().to_string();
        task.column = "progress".into();
        task.position = db.next_position("progress")?;
        task.updated_at = db::now_ms();
        db.save_task(&task)?;
        let _ = app.emit("tasks-changed", ());
        Ok(session)
    })
    .await
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

#[tauri::command]
fn remove_device(state: State<'_, AppState>, id: String) -> Res<()> {
    state.db.remove_device(&id)
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
            let db = Arc::new(Db::open(&data_dir.join("ai-remote.db"))?);
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
            app.manage(AppState {
                db,
                manager,
                companion,
                monitor: Mutex::new(monitor::Monitor::new()),
                data_dir,
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            detect_clis,
            scan_external,
            scan_skills,
            list_sessions,
            get_session,
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
