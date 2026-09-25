//! Local SQLite store (PRD FR-60, FR-70): sessions, their events, chat, board tasks,
//! recent projects, paired devices and settings. One connection behind a mutex; every
//! statement is short, so contention is not a concern at this scale.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};

use crate::cli::CliKind;
use crate::events::SessionEvent;

pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or_default()
}

pub fn new_id() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Starting,
    Running,
    Waiting,
    Idle,
    Done,
    Error,
    Stopped,
}

impl Status {
    pub fn as_str(self) -> &'static str {
        match self {
            Status::Starting => "starting",
            Status::Running => "running",
            Status::Waiting => "waiting",
            Status::Idle => "idle",
            Status::Done => "done",
            Status::Error => "error",
            Status::Stopped => "stopped",
        }
    }
    fn parse(s: &str) -> Status {
        match s {
            "starting" => Status::Starting,
            "running" => Status::Running,
            "waiting" => Status::Waiting,
            "idle" => Status::Idle,
            "done" => Status::Done,
            "error" => Status::Error,
            _ => Status::Stopped,
        }
    }
    pub fn is_live(self) -> bool {
        matches!(self, Status::Starting | Status::Running | Status::Waiting | Status::Idle)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Interactive,
    Headless,
}

/// What the CLI is waiting for, and whether AI Remote can answer it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Waiting {
    /// `permission`, `trust_folder`, `update_offer`.
    pub reason: String,
    pub tool: Option<String>,
    pub detail: String,
    /// Claude stdio control request id, for headless answers.
    pub request_id: Option<String>,
    /// Approve/Deny buttons are shown only when true.
    pub can_answer: bool,
    /// `stdio`, `hook`, or `screen` (PRD FR-16: the method is shown to the user).
    pub method: String,
    pub since: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionInfo {
    pub id: String,
    pub cli: CliKind,
    pub cwd: String,
    pub mode: Mode,
    pub title: String,
    pub prompt: String,
    pub status: Status,
    pub pid: Option<u32>,
    /// The CLI's own id, used to resume (Claude session id, Codex thread id, OpenCode session).
    pub cli_session_id: Option<String>,
    pub started_at: i64,
    pub ended_at: Option<i64>,
    pub exit_code: Option<i32>,
    pub last_event: Option<String>,
    pub waiting: Option<Waiting>,
    /// `manual`, `chat`, or `board`.
    pub source: String,
    pub task_id: Option<String>,
    pub permission_mode: Option<String>,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EventRow {
    pub id: i64,
    pub session_id: String,
    pub at: i64,
    pub event: SessionEvent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DispatchCard {
    pub id: String,
    pub cli: CliKind,
    /// The task's name in a few words; the session started from the card is called this.
    /// Cards saved before titles existed have none.
    #[serde(default)]
    pub title: String,
    pub folder: String,
    pub prompt: String,
    pub mode: Mode,
    pub reason: String,
    /// Filled by validation (PRD FR-23). Run is disabled while this is set.
    pub problem: Option<String>,
    /// `proposed`, `started`, `discarded`.
    pub state: String,
    pub session_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessage {
    pub id: String,
    pub thread_id: String,
    /// `user`, `planner`, `error`.
    pub role: String,
    pub text: String,
    pub cards: Vec<DispatchCard>,
    pub created_at: i64,
}

/// One conversation with the planner. The planner only sees the messages of its own thread.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatThread {
    pub id: String,
    pub title: String,
    pub created_at: i64,
    pub updated_at: i64,
}

/// Names a thread after the first line of its first message, as the owner wrote it.
pub fn thread_title(message: &str) -> String {
    let line = message.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or_default();
    if line.is_empty() {
        return "Untitled chat".into();
    }
    line.chars().take(80).collect()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    pub id: String,
    pub title: String,
    pub notes: String,
    pub project: String,
    pub cli: Option<CliKind>,
    /// `pending`, `todo`, `progress`, `done`.
    pub column: String,
    pub position: f64,
    pub session_id: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub path: String,
    pub last_used: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Device {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing)]
    pub token_hash: String,
    pub created_at: i64,
    pub last_seen: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub onboarded: bool,
    pub chat_cli: Option<CliKind>,
    pub notify_waiting: bool,
    pub notify_done: bool,
    pub notify_error: bool,
    pub companion_enabled: bool,
    pub companion_port: u16,
    /// Per-CLI executable override (PRD FR-04), keyed by `CliKind::bin()`.
    pub cli_paths: HashMap<String, String>,
    /// Per-CLI extra arguments, for example `--pure` for an OpenCode setup whose plugin fails.
    pub cli_args: HashMap<String, String>,
    /// Seconds between outside-session scans.
    pub scan_seconds: u64,
    /// The chat planner may read (never change) files in the project folders.
    pub planner_can_read: bool,
    /// Folders that hold projects. Empty means the usual ones such as `~/Project`.
    pub project_roots: Vec<String>,
    /// Default permission mode for new sessions: `ask`, `plan`, `auto` or `bypass`.
    pub permission_mode: String,
    /// Text size of the desktop window in percent, one of `TEXT_SIZES`.
    pub text_size: u16,
    /// The chat planner's model and thinking level, keyed by `CliKind::bin()`.
    pub chat_models: HashMap<String, ChatModel>,
    /// What answers in Chat: the chosen CLI, or the model in `planner_api`.
    pub planner_source: PlannerSource,
    pub planner_api: PlannerApi,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PlannerSource {
    #[default]
    Cli,
    Api,
}

/// A model behind an OpenAI-compatible chat completions endpoint (OpenRouter, Ollama,
/// LM Studio and the like) that plans instead of a CLI.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PlannerApi {
    /// The address before `/chat/completions`, such as `http://localhost:11434/v1`.
    pub base_url: String,
    pub model: String,
    /// Empty for local servers that need none.
    pub api_key: String,
}

impl PlannerApi {
    /// Why Chat cannot use it yet, in words for the owner.
    pub fn problem(&self) -> Option<String> {
        let url = self.base_url.trim();
        if url.is_empty() || self.model.trim().is_empty() {
            Some("The custom provider needs a base URL and a model. Add them in Settings, under Chat planner.".into())
        } else if !(url.starts_with("http://") || url.starts_with("https://")) {
            Some("The custom provider's base URL must start with http:// or https://.".into())
        } else {
            None
        }
    }
}

/// Empty values keep the CLI's own default.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ChatModel {
    pub model: String,
    pub effort: String,
}

/// Text sizes Settings offers, in percent of the sizes in DESIGN.md.
pub const TEXT_SIZES: [u16; 5] = [90, 100, 110, 125, 150];

impl Settings {
    /// Text size as a webview zoom factor, so text and the controls around it scale together.
    /// A value Settings does not offer falls back to 100%.
    pub fn zoom(&self) -> f64 {
        let pct = if TEXT_SIZES.contains(&self.text_size) { self.text_size } else { 100 };
        f64::from(pct) / 100.0
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            onboarded: false,
            chat_cli: None,
            notify_waiting: true,
            notify_done: true,
            notify_error: true,
            companion_enabled: false,
            companion_port: 8765,
            cli_paths: HashMap::new(),
            cli_args: HashMap::new(),
            scan_seconds: 10,
            planner_can_read: true,
            project_roots: Vec::new(),
            permission_mode: "ask".into(),
            text_size: 100,
            chat_models: HashMap::new(),
            planner_source: PlannerSource::Cli,
            planner_api: PlannerApi::default(),
        }
    }
}

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS sessions (
  id TEXT PRIMARY KEY,
  cli TEXT NOT NULL,
  cwd TEXT NOT NULL,
  mode TEXT NOT NULL,
  title TEXT NOT NULL,
  prompt TEXT NOT NULL,
  status TEXT NOT NULL,
  pid INTEGER,
  cli_session_id TEXT,
  started_at INTEGER NOT NULL,
  ended_at INTEGER,
  exit_code INTEGER,
  last_event TEXT,
  waiting TEXT,
  source TEXT NOT NULL,
  task_id TEXT,
  permission_mode TEXT,
  updated_at INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS session_events (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  session_id TEXT NOT NULL,
  at INTEGER NOT NULL,
  kind TEXT NOT NULL,
  payload TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS session_events_by_session ON session_events(session_id, id);
CREATE TABLE IF NOT EXISTS chat_messages (
  id TEXT PRIMARY KEY,
  role TEXT NOT NULL,
  text TEXT NOT NULL,
  cards TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  thread_id TEXT
);
CREATE TABLE IF NOT EXISTS chat_threads (
  id TEXT PRIMARY KEY,
  title TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS tasks (
  id TEXT PRIMARY KEY,
  title TEXT NOT NULL,
  notes TEXT NOT NULL,
  project TEXT NOT NULL,
  cli TEXT,
  col TEXT NOT NULL,
  position REAL NOT NULL,
  session_id TEXT,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS projects (
  path TEXT PRIMARY KEY,
  last_used INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS devices (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  token_hash TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  last_seen INTEGER
);
CREATE TABLE IF NOT EXISTS settings (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL
);
";

pub struct Db {
    conn: Mutex<Connection>,
}

type R<T> = Result<T, String>;

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

fn kind_from(s: &str) -> CliKind {
    CliKind::from_bin(s).unwrap_or(CliKind::Claude)
}

fn session_from(row: &Row) -> rusqlite::Result<SessionInfo> {
    let waiting: Option<String> = row.get("waiting")?;
    let mode: String = row.get("mode")?;
    Ok(SessionInfo {
        id: row.get("id")?,
        cli: kind_from(&row.get::<_, String>("cli")?),
        cwd: row.get("cwd")?,
        mode: if mode == "headless" { Mode::Headless } else { Mode::Interactive },
        title: row.get("title")?,
        prompt: row.get("prompt")?,
        status: Status::parse(&row.get::<_, String>("status")?),
        pid: row.get("pid")?,
        cli_session_id: row.get("cli_session_id")?,
        started_at: row.get("started_at")?,
        ended_at: row.get("ended_at")?,
        exit_code: row.get("exit_code")?,
        last_event: row.get("last_event")?,
        waiting: waiting.and_then(|w| serde_json::from_str(&w).ok()),
        source: row.get("source")?,
        task_id: row.get("task_id")?,
        permission_mode: row.get("permission_mode")?,
        updated_at: row.get("updated_at")?,
    })
}

fn chat_from(row: &Row) -> rusqlite::Result<ChatMessage> {
    let cards: String = row.get("cards")?;
    Ok(ChatMessage {
        id: row.get("id")?,
        thread_id: row.get::<_, Option<String>>("thread_id")?.unwrap_or_default(),
        role: row.get("role")?,
        text: row.get("text")?,
        cards: serde_json::from_str(&cards).unwrap_or_default(),
        created_at: row.get("created_at")?,
    })
}

fn thread_from(row: &Row) -> rusqlite::Result<ChatThread> {
    Ok(ChatThread {
        id: row.get("id")?,
        title: row.get("title")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

/// Chat used to be one endless conversation. A database from then gets the thread column,
/// and its messages become one thread so nothing the owner wrote disappears.
fn migrate_chat(conn: &Connection) -> rusqlite::Result<()> {
    let has_column = conn
        .prepare("SELECT 1 FROM pragma_table_info('chat_messages') WHERE name = 'thread_id'")?
        .exists([])?;
    if !has_column {
        conn.execute_batch("ALTER TABLE chat_messages ADD COLUMN thread_id TEXT")?;
    }
    conn.execute_batch("CREATE INDEX IF NOT EXISTS chat_messages_by_thread ON chat_messages(thread_id, created_at)")?;
    let span: Option<(i64, i64)> = conn.query_row(
        "SELECT MIN(created_at), MAX(created_at) FROM chat_messages WHERE thread_id IS NULL",
        [],
        |r| Ok(r.get::<_, Option<i64>>(0)?.zip(r.get::<_, Option<i64>>(1)?)),
    )?;
    let Some((first, last)) = span else { return Ok(()) };
    let opener: Option<String> = conn
        .query_row(
            "SELECT text FROM chat_messages WHERE thread_id IS NULL AND role = 'user' ORDER BY created_at, rowid LIMIT 1",
            [],
            |r| r.get(0),
        )
        .optional()?;
    let id = new_id();
    let title = opener.as_deref().map(thread_title).unwrap_or_else(|| "Earlier chat".into());
    conn.execute(
        "INSERT INTO chat_threads (id, title, created_at, updated_at) VALUES (?1, ?2, ?3, ?4)",
        params![id, title, first, last],
    )?;
    conn.execute("UPDATE chat_messages SET thread_id = ?1 WHERE thread_id IS NULL", [&id])?;
    Ok(())
}

fn task_from(row: &Row) -> rusqlite::Result<Task> {
    let cli: Option<String> = row.get("cli")?;
    Ok(Task {
        id: row.get("id")?,
        title: row.get("title")?,
        notes: row.get("notes")?,
        project: row.get("project")?,
        cli: cli.as_deref().and_then(CliKind::from_bin),
        column: row.get("col")?,
        position: row.get("position")?,
        session_id: row.get("session_id")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

impl Db {
    pub fn open(path: &Path) -> R<Self> {
        let conn = Connection::open(path).map_err(err)?;
        Self::init(conn)
    }

    pub fn open_in_memory() -> R<Self> {
        Self::init(Connection::open_in_memory().map_err(err)?)
    }

    fn init(conn: Connection) -> R<Self> {
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")
            .map_err(err)?;
        conn.execute_batch(SCHEMA).map_err(err)?;
        migrate_chat(&conn).map_err(err)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    fn with<T>(&self, f: impl FnOnce(&Connection) -> rusqlite::Result<T>) -> R<T> {
        let conn = self.conn.lock().map_err(err)?;
        f(&conn).map_err(err)
    }

    // Sessions

    pub fn upsert_session(&self, s: &SessionInfo) -> R<()> {
        let waiting = s.waiting.as_ref().and_then(|w| serde_json::to_string(w).ok());
        self.with(|c| {
            c.execute(
                "INSERT INTO sessions (id, cli, cwd, mode, title, prompt, status, pid, cli_session_id,
                   started_at, ended_at, exit_code, last_event, waiting, source, task_id, permission_mode, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)
                 ON CONFLICT(id) DO UPDATE SET
                   status = excluded.status, pid = excluded.pid, cli_session_id = excluded.cli_session_id,
                   ended_at = excluded.ended_at, exit_code = excluded.exit_code, last_event = excluded.last_event,
                   waiting = excluded.waiting, title = excluded.title, prompt = excluded.prompt,
                   task_id = excluded.task_id, updated_at = excluded.updated_at",
                params![
                    s.id,
                    s.cli.bin(),
                    s.cwd,
                    if s.mode == Mode::Headless { "headless" } else { "interactive" },
                    s.title,
                    s.prompt,
                    s.status.as_str(),
                    s.pid,
                    s.cli_session_id,
                    s.started_at,
                    s.ended_at,
                    s.exit_code,
                    s.last_event,
                    waiting,
                    s.source,
                    s.task_id,
                    s.permission_mode,
                    s.updated_at,
                ],
            )
            .map(|_| ())
        })
    }

    pub fn session(&self, id: &str) -> R<Option<SessionInfo>> {
        self.with(|c| {
            c.query_row("SELECT * FROM sessions WHERE id = ?1", [id], session_from)
                .optional()
        })
    }

    /// Newest first. `limit` keeps the Overview light; history screens can ask for more.
    pub fn sessions(&self, limit: u32) -> R<Vec<SessionInfo>> {
        self.with(|c| {
            let mut st = c.prepare("SELECT * FROM sessions ORDER BY started_at DESC LIMIT ?1")?;
            let rows = st.query_map([limit], session_from)?;
            rows.collect()
        })
    }

    /// Sessions left live by a previous run of the app are no longer attached to anything.
    pub fn close_orphans(&self) -> R<usize> {
        let now = now_ms();
        self.with(|c| {
            c.execute(
                "UPDATE sessions SET status = 'stopped', waiting = NULL, ended_at = COALESCE(ended_at, ?1),
                   last_event = 'AI Remote was closed while this session ran', updated_at = ?1
                 WHERE status IN ('starting', 'running', 'waiting', 'idle')",
                [now],
            )
        })
    }

    pub fn delete_history(&self) -> R<()> {
        self.with(|c| {
            c.execute_batch(
                "DELETE FROM session_events WHERE session_id IN
                   (SELECT id FROM sessions WHERE status IN ('done', 'error', 'stopped'));
                 DELETE FROM sessions WHERE status IN ('done', 'error', 'stopped');",
            )
        })
    }

    /// Removes one session and its events. A board card that ran it stays, without the link.
    pub fn delete_session(&self, id: &str) -> R<()> {
        self.with(|c| {
            let tx = c.unchecked_transaction()?;
            tx.execute("DELETE FROM session_events WHERE session_id = ?1", [id])?;
            tx.execute("DELETE FROM sessions WHERE id = ?1", [id])?;
            tx.execute("UPDATE tasks SET session_id = NULL WHERE session_id = ?1", [id])?;
            tx.commit()
        })
    }

    pub fn add_event(&self, session_id: &str, at: i64, ev: &SessionEvent) -> R<i64> {
        let payload = serde_json::to_string(ev).map_err(err)?;
        let kind = serde_json::to_value(ev)
            .ok()
            .and_then(|v| v["kind"].as_str().map(str::to_owned))
            .unwrap_or_default();
        self.with(|c| {
            c.execute(
                "INSERT INTO session_events (session_id, at, kind, payload) VALUES (?1, ?2, ?3, ?4)",
                params![session_id, at, kind, payload],
            )?;
            Ok(c.last_insert_rowid())
        })
    }

    /// The most recent `limit` events, oldest first.
    pub fn events(&self, session_id: &str, limit: u32) -> R<Vec<EventRow>> {
        let mut rows = self.with(|c| {
            let mut st = c.prepare(
                "SELECT id, session_id, at, payload FROM session_events
                 WHERE session_id = ?1 ORDER BY id DESC LIMIT ?2",
            )?;
            let rows = st.query_map(params![session_id, limit], |r| {
                let payload: String = r.get(3)?;
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, payload))
            })?;
            rows.collect::<rusqlite::Result<Vec<(i64, String, i64, String)>>>()
        })?;
        rows.reverse();
        Ok(rows
            .into_iter()
            .filter_map(|(id, session_id, at, payload)| {
                Some(EventRow {
                    id,
                    session_id,
                    at,
                    event: serde_json::from_str(&payload).ok()?,
                })
            })
            .collect())
    }

    /// Event times per session, for the horizon track.
    pub fn event_marks(&self, session_id: &str, limit: u32) -> R<Vec<(i64, String)>> {
        self.with(|c| {
            let mut st = c.prepare(
                "SELECT at, kind FROM session_events WHERE session_id = ?1
                   AND kind IN ('tool_call', 'file_changed', 'permission_request', 'permission_denied', 'tool_failed', 'error')
                 ORDER BY id DESC LIMIT ?2",
            )?;
            let rows = st.query_map(params![session_id, limit], |r| Ok((r.get(0)?, r.get(1)?)))?;
            rows.collect()
        })
    }

    // Chat

    pub fn add_chat(&self, m: &ChatMessage) -> R<()> {
        let cards = serde_json::to_string(&m.cards).map_err(err)?;
        self.with(|c| {
            c.execute(
                "INSERT OR REPLACE INTO chat_messages (id, thread_id, role, text, cards, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![m.id, m.thread_id, m.role, m.text, cards, m.created_at],
            )
            .map(|_| ())
        })
    }

    /// The most recent `limit` messages of one thread, oldest first.
    pub fn chat(&self, thread_id: &str, limit: u32) -> R<Vec<ChatMessage>> {
        let mut rows = self.with(|c| {
            let mut st = c.prepare(
                "SELECT * FROM chat_messages WHERE thread_id = ?1 ORDER BY created_at DESC, rowid DESC LIMIT ?2",
            )?;
            let rows = st.query_map(params![thread_id, limit], chat_from)?;
            rows.collect::<rusqlite::Result<Vec<_>>>()
        })?;
        rows.reverse();
        Ok(rows)
    }

    pub fn chat_message(&self, id: &str) -> R<Option<ChatMessage>> {
        self.with(|c| c.query_row("SELECT * FROM chat_messages WHERE id = ?1", [id], chat_from).optional())
    }

    pub fn create_thread(&self, title: &str) -> R<ChatThread> {
        let now = now_ms();
        let t = ChatThread {
            id: new_id(),
            title: title.into(),
            created_at: now,
            updated_at: now,
        };
        self.with(|c| {
            c.execute(
                "INSERT INTO chat_threads (id, title, created_at, updated_at) VALUES (?1, ?2, ?3, ?4)",
                params![t.id, t.title, t.created_at, t.updated_at],
            )
            .map(|_| ())
        })?;
        Ok(t)
    }

    /// Threads with the latest message first.
    pub fn threads(&self) -> R<Vec<ChatThread>> {
        self.with(|c| {
            let mut st = c.prepare("SELECT * FROM chat_threads ORDER BY updated_at DESC, rowid DESC")?;
            let rows = st.query_map([], thread_from)?;
            rows.collect()
        })
    }

    pub fn thread(&self, id: &str) -> R<Option<ChatThread>> {
        self.with(|c| c.query_row("SELECT * FROM chat_threads WHERE id = ?1", [id], thread_from).optional())
    }

    pub fn touch_thread(&self, id: &str, at: i64) -> R<()> {
        self.with(|c| c.execute("UPDATE chat_threads SET updated_at = ?2 WHERE id = ?1", params![id, at]).map(|_| ()))
    }

    /// Removes the conversation only. Sessions started from its cards keep running.
    pub fn delete_thread(&self, id: &str) -> R<()> {
        self.with(|c| {
            let tx = c.unchecked_transaction()?;
            tx.execute("DELETE FROM chat_messages WHERE thread_id = ?1", [id])?;
            tx.execute("DELETE FROM chat_threads WHERE id = ?1", [id])?;
            tx.commit()
        })
    }

    // Tasks

    pub fn tasks(&self) -> R<Vec<Task>> {
        self.with(|c| {
            let mut st = c.prepare("SELECT * FROM tasks ORDER BY col, position")?;
            let rows = st.query_map([], task_from)?;
            rows.collect()
        })
    }

    pub fn task(&self, id: &str) -> R<Option<Task>> {
        self.with(|c| c.query_row("SELECT * FROM tasks WHERE id = ?1", [id], task_from).optional())
    }

    pub fn task_for_session(&self, session_id: &str) -> R<Option<Task>> {
        self.with(|c| {
            c.query_row("SELECT * FROM tasks WHERE session_id = ?1", [session_id], task_from)
                .optional()
        })
    }

    pub fn save_task(&self, t: &Task) -> R<()> {
        self.with(|c| {
            c.execute(
                "INSERT INTO tasks (id, title, notes, project, cli, col, position, session_id, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
                 ON CONFLICT(id) DO UPDATE SET title = excluded.title, notes = excluded.notes,
                   project = excluded.project, cli = excluded.cli, col = excluded.col,
                   position = excluded.position, session_id = excluded.session_id, updated_at = excluded.updated_at",
                params![
                    t.id,
                    t.title,
                    t.notes,
                    t.project,
                    t.cli.map(CliKind::bin),
                    t.column,
                    t.position,
                    t.session_id,
                    t.created_at,
                    t.updated_at
                ],
            )
            .map(|_| ())
        })
    }

    pub fn delete_task(&self, id: &str) -> R<()> {
        self.with(|c| c.execute("DELETE FROM tasks WHERE id = ?1", [id]).map(|_| ()))
    }

    /// Position after the last card of a column, so new and moved cards land at the end.
    pub fn next_position(&self, column: &str) -> R<f64> {
        self.with(|c| {
            c.query_row(
                "SELECT COALESCE(MAX(position), 0) + 1 FROM tasks WHERE col = ?1",
                [column],
                |r| r.get(0),
            )
        })
    }

    // Projects

    pub fn touch_project(&self, path: &str) -> R<()> {
        self.with(|c| {
            c.execute(
                "INSERT INTO projects (path, last_used) VALUES (?1, ?2)
                 ON CONFLICT(path) DO UPDATE SET last_used = excluded.last_used",
                params![path, now_ms()],
            )
            .map(|_| ())
        })
    }

    pub fn projects(&self, limit: u32) -> R<Vec<Project>> {
        self.with(|c| {
            let mut st = c.prepare("SELECT path, last_used FROM projects ORDER BY last_used DESC LIMIT ?1")?;
            let rows = st.query_map([limit], |r| {
                Ok(Project {
                    path: r.get(0)?,
                    last_used: r.get(1)?,
                })
            })?;
            rows.collect()
        })
    }

    // Devices

    pub fn add_device(&self, d: &Device) -> R<()> {
        self.with(|c| {
            c.execute(
                "INSERT INTO devices (id, name, token_hash, created_at, last_seen) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![d.id, d.name, d.token_hash, d.created_at, d.last_seen],
            )
            .map(|_| ())
        })
    }

    pub fn devices(&self) -> R<Vec<Device>> {
        self.with(|c| {
            let mut st = c.prepare(
                "SELECT id, name, token_hash, created_at, last_seen FROM devices ORDER BY created_at DESC",
            )?;
            let rows = st.query_map([], |r| {
                Ok(Device {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    token_hash: r.get(2)?,
                    created_at: r.get(3)?,
                    last_seen: r.get(4)?,
                })
            })?;
            rows.collect()
        })
    }

    pub fn device_by_hash(&self, hash: &str) -> R<Option<Device>> {
        Ok(self.devices()?.into_iter().find(|d| d.token_hash == hash))
    }

    pub fn touch_device(&self, id: &str) -> R<()> {
        self.with(|c| {
            c.execute("UPDATE devices SET last_seen = ?2 WHERE id = ?1", params![id, now_ms()])
                .map(|_| ())
        })
    }

    pub fn remove_device(&self, id: &str) -> R<()> {
        self.with(|c| c.execute("DELETE FROM devices WHERE id = ?1", [id]).map(|_| ()))
    }

    // Settings

    pub fn settings(&self) -> R<Settings> {
        let raw: Option<String> = self.with(|c| {
            c.query_row("SELECT value FROM settings WHERE key = 'settings'", [], |r| r.get(0))
                .optional()
        })?;
        Ok(raw
            .and_then(|r| serde_json::from_str(&r).ok())
            .unwrap_or_default())
    }

    pub fn save_settings(&self, s: &Settings) -> R<()> {
        let value = serde_json::to_string(s).map_err(err)?;
        self.with(|c| {
            c.execute(
                "INSERT INTO settings (key, value) VALUES ('settings', ?1)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                [value],
            )
            .map(|_| ())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn said(thread: &str, id: &str, role: &str, text: &str, at: i64) -> ChatMessage {
        ChatMessage {
            id: id.into(),
            thread_id: thread.into(),
            role: role.into(),
            text: text.into(),
            cards: vec![],
            created_at: at,
        }
    }

    #[test]
    fn each_chat_thread_keeps_its_own_messages() {
        let db = Db::open_in_memory().unwrap();
        let a = db.create_thread("Fix the tests").unwrap();
        let b = db.create_thread("Write the docs").unwrap();
        db.add_chat(&said(&a.id, "a1", "user", "fix", 1)).unwrap();
        db.add_chat(&said(&b.id, "b1", "user", "docs", 2)).unwrap();
        db.add_chat(&said(&a.id, "a2", "planner", "ok", 3)).unwrap();
        let texts = |id: &str| db.chat(id, 10).unwrap().into_iter().map(|m| m.text).collect::<Vec<_>>();
        assert_eq!(texts(&a.id), ["fix", "ok"]);
        assert_eq!(texts(&b.id), ["docs"]);
        assert_eq!(db.chat_message("a2").unwrap().unwrap().thread_id, a.id);

        db.touch_thread(&a.id, now_ms() + 1000).unwrap();
        let order: Vec<String> = db.threads().unwrap().into_iter().map(|t| t.title).collect();
        assert_eq!(order, ["Fix the tests", "Write the docs"]);

        db.delete_thread(&a.id).unwrap();
        assert!(db.thread(&a.id).unwrap().is_none());
        assert!(db.chat_message("a1").unwrap().is_none());
        assert_eq!(texts(&b.id), ["docs"]);
    }

    #[test]
    fn the_old_single_chat_becomes_one_thread() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE chat_messages (id TEXT PRIMARY KEY, role TEXT NOT NULL, text TEXT NOT NULL,
               cards TEXT NOT NULL, created_at INTEGER NOT NULL);
             INSERT INTO chat_messages VALUES ('m1', 'user', '  \nCodex: fix the failing tests\nin ai-remote', '[]', 10);
             INSERT INTO chat_messages VALUES ('m2', 'planner', 'One card.', '[]', 20);",
        )
        .unwrap();
        let db = Db::init(conn).unwrap();
        let threads = db.threads().unwrap();
        assert_eq!(threads.len(), 1);
        assert_eq!(threads[0].title, "Codex: fix the failing tests");
        assert_eq!((threads[0].created_at, threads[0].updated_at), (10, 20));
        assert_eq!(db.chat(&threads[0].id, 10).unwrap().len(), 2);
    }

    #[test]
    fn thread_titles_come_from_the_first_line() {
        assert_eq!(thread_title("\n  Add a dark mode toggle  \nto uninote"), "Add a dark mode toggle");
        assert_eq!(thread_title(&"x".repeat(200)).chars().count(), 80);
        assert_eq!(thread_title("   "), "Untitled chat");
    }

    fn session(id: &str, status: Status) -> SessionInfo {
        SessionInfo {
            id: id.into(),
            cli: CliKind::Claude,
            cwd: "C:/w".into(),
            mode: Mode::Headless,
            title: "t".into(),
            prompt: "p".into(),
            status,
            pid: Some(1),
            cli_session_id: None,
            started_at: 1,
            ended_at: None,
            exit_code: None,
            last_event: None,
            waiting: None,
            source: "manual".into(),
            task_id: None,
            permission_mode: None,
            updated_at: 1,
        }
    }

    #[test]
    fn sessions_round_trip_and_orphans_close() {
        let db = Db::open_in_memory().unwrap();
        let mut s = session("a", Status::Running);
        s.waiting = Some(Waiting {
            reason: "permission".into(),
            tool: Some("Write".into()),
            detail: "a.txt".into(),
            request_id: Some("r1".into()),
            can_answer: true,
            method: "stdio".into(),
            since: 5,
        });
        db.upsert_session(&s).unwrap();
        db.upsert_session(&session("b", Status::Done)).unwrap();
        let got = db.session("a").unwrap().unwrap();
        assert_eq!(got.waiting.unwrap().request_id.as_deref(), Some("r1"));
        assert_eq!(db.close_orphans().unwrap(), 1);
        assert_eq!(db.session("a").unwrap().unwrap().status, Status::Stopped);
        assert_eq!(db.session("b").unwrap().unwrap().status, Status::Done);
    }

    #[test]
    fn events_are_returned_oldest_first() {
        let db = Db::open_in_memory().unwrap();
        for i in 0..5 {
            db.add_event("a", i, &SessionEvent::Message { text: format!("m{i}") })
                .unwrap();
        }
        let evs = db.events("a", 3).unwrap();
        let texts: Vec<_> = evs
            .iter()
            .map(|e| match &e.event {
                SessionEvent::Message { text } => text.clone(),
                _ => String::new(),
            })
            .collect();
        assert_eq!(texts, ["m2", "m3", "m4"]);
    }

    #[test]
    fn tasks_positions_and_settings_defaults() {
        let db = Db::open_in_memory().unwrap();
        assert_eq!(db.next_position("todo").unwrap(), 1.0);
        let t = Task {
            id: "t1".into(),
            title: "x".into(),
            notes: String::new(),
            project: "C:/w".into(),
            cli: Some(CliKind::Codex),
            column: "todo".into(),
            position: 1.0,
            session_id: None,
            created_at: 1,
            updated_at: 1,
        };
        db.save_task(&t).unwrap();
        assert_eq!(db.next_position("todo").unwrap(), 2.0);
        assert_eq!(db.task("t1").unwrap().unwrap().cli, Some(CliKind::Codex));
        let s = db.settings().unwrap();
        assert!(s.notify_waiting && !s.companion_enabled);
    }

    #[test]
    fn text_size_defaults_to_100_and_ignores_sizes_settings_does_not_offer() {
        // Settings saved before text size existed have no `textSize` key.
        let old: Settings = serde_json::from_str(r#"{"onboarded":true,"permissionMode":"plan"}"#).unwrap();
        assert_eq!(old.text_size, 100);
        assert_eq!(old.zoom(), 1.0);

        let db = Db::open_in_memory().unwrap();
        db.save_settings(&Settings { text_size: 125, ..Settings::default() }).unwrap();
        assert_eq!(db.settings().unwrap().zoom(), 1.25);

        assert_eq!(Settings { text_size: 400, ..Settings::default() }.zoom(), 1.0);
    }

    #[test]
    fn deleting_a_session_keeps_others_and_unlinks_its_card() {
        let db = Db::open_in_memory().unwrap();
        db.upsert_session(&session("a", Status::Done)).unwrap();
        db.upsert_session(&session("b", Status::Done)).unwrap();
        db.add_event("a", 1, &SessionEvent::Message { text: "a".into() }).unwrap();
        db.add_event("b", 1, &SessionEvent::Message { text: "b".into() }).unwrap();
        db.save_task(&Task {
            id: "t1".into(),
            title: "x".into(),
            notes: String::new(),
            project: "C:/w".into(),
            cli: None,
            column: "done".into(),
            position: 1.0,
            session_id: Some("a".into()),
            created_at: 1,
            updated_at: 1,
        })
        .unwrap();
        db.delete_session("a").unwrap();
        assert!(db.session("a").unwrap().is_none());
        assert!(db.events("a", 10).unwrap().is_empty());
        assert_eq!(db.events("b", 10).unwrap().len(), 1);
        assert!(db.session("b").unwrap().is_some());
        assert_eq!(db.task("t1").unwrap().unwrap().session_id, None);
    }
}
