//! Work the desktop window and the phone companion both do: planning in Chat, dispatch cards,
//! and moving or running Board cards. Each caller tells its own listeners what changed.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

use crate::cli::{self, CliInstall, CliKind};
use crate::db::{self, ChatMessage, ChatThread, Db, DispatchCard, Mode, PlannerSource, SessionInfo, Settings, Task};
use crate::monitor;
use crate::models;
use crate::orchestrator;
use crate::session::{Manager, StartRequest};

type Res<T> = Result<T, String>;

pub const COLUMNS: [&str; 4] = ["pending", "todo", "progress", "done"];

/// Installed CLIs, with the custom paths from Settings taking the place of PATH lookups.
pub fn detect_with_settings(db: &Db) -> Vec<CliInstall> {
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

/// The extra arguments Settings keeps for `kind`.
pub fn cli_args(settings: &Settings, kind: CliKind) -> Vec<String> {
    settings
        .cli_args
        .get(kind.bin())
        .map(|a| a.split_whitespace().map(str::to_owned).collect())
        .unwrap_or_default()
}

/// The CLI that answers in Chat: the one chosen in Settings when it is installed, else the first usable one.
fn chat_cli(settings: &Settings, clis: &[CliInstall]) -> Option<CliKind> {
    settings
        .chat_cli
        .filter(|k| clis.iter().any(|c| c.kind == *k && c.path.is_some()))
        .or_else(|| clis.iter().find(|c| c.path.is_some() && c.kind != CliKind::Gemini).map(|c| c.kind))
}

/// Who answers in Chat, as the phone names it. `None` when nothing can plan yet.
pub fn planner_name(db: &Db, clis: &[CliInstall]) -> Res<Option<String>> {
    let settings = db.settings()?;
    if settings.planner_source == PlannerSource::Api {
        let api = &settings.planner_api;
        let url = api.base_url.trim().to_ascii_lowercase();
        let ready = !api.model.trim().is_empty() && (url.starts_with("http://") || url.starts_with("https://"));
        return Ok(ready.then(|| api.model.trim().to_string()));
    }
    Ok(chat_cli(&settings, clis).map(|k| k.label().to_string()))
}

#[derive(Serialize)]
pub struct ChatTurn {
    pub thread: ChatThread,
    pub user: ChatMessage,
    pub reply: ChatMessage,
}

/// One planner turn. Without `thread_id` the message starts a new thread, named after the message.
/// One turn per thread at a time; `asked` hears the thread id once the question is stored, so
/// other screens can show that the planner is answering.
pub fn chat_send(
    db: &Db,
    data_dir: &Path,
    own: &HashSet<u32>,
    thread_id: Option<String>,
    message: &str,
    asked: &dyn Fn(&str),
) -> Res<ChatTurn> {
    let text = message.trim().to_string();
    if text.is_empty() {
        return Err("Write a message first.".into());
    }
    let mut thread = match thread_id {
        Some(id) => db.thread(&id)?.ok_or("This chat was deleted. Start a new chat.")?,
        None => db.create_thread(&db::thread_title(&text))?,
    };
    let _answering = ANSWERING.claim(&thread.id, "The planner is still answering in this chat. Send again when it is done.")?;
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
    asked(&thread.id);

    let clis = detect_with_settings(db);
    let settings = db.settings()?;
    let reply = match chat_cli(&settings, &clis) {
        // A custom provider plans without any CLI, and without file access.
        _ if settings.planner_source == PlannerSource::Api => {
            let api = &settings.planner_api;
            let outside = monitor::Monitor::new().scan(own);
            let ctx = orchestrator::gather(db, &outside)?;
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
            let outside = monitor::Monitor::new().scan(own);
            let mut ctx = orchestrator::gather(db, &outside)?;
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
}

pub fn with_card(db: &Db, message_id: &str, card_id: &str, f: impl FnOnce(&mut DispatchCard) -> Res<()>) -> Res<ChatMessage> {
    let mut msg = db.chat_message(message_id)?.ok_or("Message not found.")?;
    let card = msg.cards.iter_mut().find(|c| c.id == card_id).ok_or("Card not found.")?;
    f(card)?;
    db.add_chat(&msg)?;
    Ok(msg)
}

/// `undo` brings a discarded card back.
pub fn discard_card(db: &Db, message_id: &str, card_id: &str, undo: bool) -> Res<ChatMessage> {
    with_card(db, message_id, card_id, |c| {
        c.state = if undo { "proposed".into() } else { "discarded".into() };
        Ok(())
    })
}

/// Work in progress by id, so the same work cannot run twice when the phone and the desktop ask
/// at the same moment.
struct Claims(Mutex<Vec<String>>);

/// Held while the work runs; dropping it frees the id.
struct Claim {
    claims: &'static Claims,
    id: String,
}

impl Claims {
    const fn new() -> Self {
        Self(Mutex::new(Vec::new()))
    }

    fn claim(&'static self, id: &str, busy: &str) -> Res<Claim> {
        let mut list = self.0.lock().map_err(|e| e.to_string())?;
        if list.iter().any(|c| c == id) {
            return Err(busy.into());
        }
        list.push(id.to_string());
        Ok(Claim { claims: self, id: id.to_string() })
    }

    fn ids(&self) -> Vec<String> {
        self.0.lock().map(|l| l.clone()).unwrap_or_default()
    }
}

impl Drop for Claim {
    fn drop(&mut self) {
        if let Ok(mut list) = self.claims.0.lock() {
            list.retain(|c| *c != self.id);
        }
    }
}

/// Cards whose session is being started.
static STARTING: Claims = Claims::new();
/// Chat threads whose planner turn is running.
static ANSWERING: Claims = Claims::new();

/// Threads where the planner is answering right now, so every screen can say so and hold Send.
pub fn answering() -> Vec<String> {
    ANSWERING.ids()
}

/// Starts the session a card proposes. Nothing runs from Chat without this.
pub fn run_card(db: &Db, manager: &Arc<Manager>, message_id: &str, card_id: &str) -> Res<ChatMessage> {
    let _claim = STARTING.claim(card_id, "This card is starting its session already.")?;
    let clis = detect_with_settings(db);
    let msg = db.chat_message(message_id)?.ok_or("Message not found.")?;
    let mut card = msg.cards.iter().find(|c| c.id == card_id).cloned().ok_or("Card not found.")?;
    if card.state != "proposed" {
        return Err("This card already ran or was discarded.".into());
    }
    // A follow-up goes to its session (PRD FR-25) instead of starting one.
    if let Some(target) = card.target.clone() {
        let session = db.session(&target)?;
        orchestrator::validate_target(&mut card, &clis, session.as_ref());
        if let Some(problem) = card.problem {
            return Err(problem);
        }
        manager.send_message(&target, &card.prompt)?;
        return with_card(db, message_id, card_id, |c| {
            c.state = "started".into();
            c.session_id = Some(target);
            Ok(())
        });
    }
    orchestrator::validate(&mut card, &clis);
    if let Some(problem) = card.problem {
        return Err(problem);
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
    with_card(db, message_id, card_id, |c| {
        c.state = "started".into();
        c.session_id = Some(session.id.clone());
        Ok(())
    })
}

/// A card from Chat becomes a Todo card on the Board, once: the chat card remembers it.
/// Returns the Board card and the chat message that now links to it.
pub fn card_to_board(db: &Db, message_id: &str, card_id: &str) -> Res<(Task, ChatMessage)> {
    let msg = db.chat_message(message_id)?.ok_or("Message not found.")?;
    let card = msg.cards.iter().find(|c| c.id == card_id).ok_or("Card not found.")?;
    if card.target.is_some() {
        return Err("A follow-up goes to its session; it cannot become a Board card.".into());
    }
    if card.task_id.as_deref().is_some_and(|t| db.task(t).ok().flatten().is_some()) {
        return Err("This card is on the Board already.".into());
    }
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
        position: db.next_position("todo")?,
        session_id: None,
        created_at: now,
        updated_at: now,
    };
    db.save_task(&task)?;
    let message = with_card(db, message_id, card_id, |c| {
        c.task_id = Some(task.id.clone());
        Ok(())
    })?;
    Ok((task, message))
}

/// Moves a card to `column`, before the card `before` when it is in that column, else to the end.
pub fn move_task(db: &Db, id: &str, column: &str, before: Option<String>) -> Res<Task> {
    if !COLUMNS.contains(&column) {
        return Err("Unknown column.".into());
    }
    let mut task = db.task(id)?.ok_or("Card not found.")?;
    task.position = match before.and_then(|b| db.task(&b).ok().flatten()) {
        Some(b) if b.column == column => {
            let prev = db
                .tasks()?
                .into_iter()
                .filter(|t| t.column == column && t.position < b.position && t.id != task.id)
                .map(|t| t.position)
                .fold(b.position - 1.0, f64::max);
            (prev + b.position) / 2.0
        }
        _ => db.next_position(column)?,
    };
    task.column = column.to_string();
    task.updated_at = db::now_ms();
    db.save_task(&task)?;
    Ok(task)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunTaskInput {
    pub id: String,
    pub cli: CliKind,
    pub cwd: String,
    pub mode: Mode,
    pub prompt: String,
    pub permission_mode: Option<String>,
}

/// Runs a Board card: the session starts and the card moves to In progress with it.
pub fn run_task(db: &Db, manager: &Arc<Manager>, input: RunTaskInput) -> Res<SessionInfo> {
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
    Ok(session)
}

#[cfg(test)]
mod tests {
    use super::*;

    static TEST_CLAIMS: Claims = Claims::new();

    #[test]
    fn work_by_id_runs_once_at_a_time_and_frees_its_id() {
        let first = TEST_CLAIMS.claim("t1", "busy").unwrap();
        assert_eq!(TEST_CLAIMS.claim("t1", "busy").err().as_deref(), Some("busy"));
        let other = TEST_CLAIMS.claim("t2", "busy").unwrap();
        assert_eq!(TEST_CLAIMS.ids(), ["t1", "t2"]);
        drop(first);
        assert_eq!(TEST_CLAIMS.ids(), ["t2"]);
        assert!(TEST_CLAIMS.claim("t1", "busy").is_ok());
        drop(other);
    }
}
