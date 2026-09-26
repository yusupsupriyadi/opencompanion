//! Session manager (PRD section B): starts CLIs in a PTY or headless, tracks one status per
//! session, detects "Waiting for you", answers permissions, and records everything in SQLite.

use std::collections::{HashMap, HashSet};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use serde::Deserialize;
use serde_json::Value;

use crate::cli::{self, CliKind};
use crate::db::{self, Db, EventRow, Mode, Notice, SessionInfo, Status, Waiting};
use crate::events::{self, SessionEvent};
use crate::headless::{self, HeadlessRun, PermMode, Stream, TurnOptions};
use crate::pty::{PtySession, PtySpec};
use crate::waiting;

/// Everything the manager tells the outside world. The Tauri app forwards these to the
/// webview, the phone companion and OS notifications; tests record them.
pub trait Emit: Send + Sync {
    fn session(&self, info: &SessionInfo);
    /// Terminal text. `seq` counts the session's chunks from 1, so a view that read a snapshot
    /// can skip what the snapshot already holds.
    fn output(&self, id: &str, data: &str, seq: u64);
    fn event(&self, row: &EventRow);
    fn tasks_changed(&self);
    fn notify(&self, title: &str, body: &str, session_id: &str);
    /// A chat thread changed from the phone, so an open Chat screen reloads it.
    fn chat_changed(&self, _thread_id: &str) {}
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartRequest {
    pub cli: CliKind,
    pub cwd: String,
    pub mode: Mode,
    #[serde(default)]
    pub prompt: String,
    /// The task's name from a Chat or Board card. Without one the title comes from the prompt.
    pub title: Option<String>,
    pub permission_mode: Option<String>,
    pub source: Option<String>,
    pub task_id: Option<String>,
    pub cols: Option<u16>,
    pub rows: Option<u16>,
}

enum Runner {
    None,
    Pty(PtySession),
    Headless(HeadlessRun),
}

/// Keeps the tail of a PTY stream so a terminal view can be rebuilt after navigation.
const OUTPUT_KEEP: usize = 512 * 1024;
/// Output silence after which an interactive session counts as Idle.
const IDLE_AFTER: Duration = Duration::from_secs(8);

struct Live {
    info: Mutex<SessionInfo>,
    runner: Mutex<Runner>,
    output: Mutex<String>,
    /// Chunks added to `output` so far; changed only while `output` is locked.
    output_seq: AtomicU64,
    utf8_carry: Mutex<Vec<u8>>,
    log: Mutex<Option<File>>,
    last_output: Mutex<Instant>,
    stop_requested: AtomicBool,
    /// Bumped on every spawn; a watcher whose generation is stale leaves quietly.
    generation: AtomicU64,
    /// Set while the headless turn reported a failure, so the exit maps to Error.
    turn_failed: AtomicBool,
    /// Pending Claude `can_use_tool` input, echoed back on Approve.
    pending_input: Mutex<Option<Value>>,
    hook_file: Option<PathBuf>,
    /// Claude Code's Stop hook said the turn is over. Terminal redraws do not make it Running
    /// again; the next hook report does.
    hook_idle: AtomicBool,
}

pub struct Manager {
    db: Arc<Db>,
    emit: Arc<dyn Emit>,
    data_dir: PathBuf,
    live: Mutex<HashMap<String, Arc<Live>>>,
    /// Sessions a Resume or follow-up is starting right now, so a second press from the phone
    /// or the desktop cannot start a second process.
    spawning: Mutex<HashSet<String>>,
}

/// Holds a session's place in `Manager::spawning` until the spawn is over.
struct Spawning<'a> {
    set: &'a Mutex<HashSet<String>>,
    id: String,
}

impl Drop for Spawning<'_> {
    fn drop(&mut self) {
        if let Ok(mut set) = self.set.lock() {
            set.remove(&self.id);
        }
    }
}

fn title_for(kind: CliKind, cwd: &str, prompt: &str) -> String {
    let first = prompt.lines().map(str::trim).find(|l| !l.is_empty());
    match first {
        Some(line) if line.chars().count() > 90 => {
            format!("{}…", line.chars().take(90).collect::<String>())
        }
        Some(line) => line.to_string(),
        None => format!("{} in {}", kind.label(), folder_name(cwd)),
    }
}

/// True while the title is only the prompt's first line or the folder, so a real task name
/// may replace it. Names from a card are kept.
fn title_is_derived(info: &SessionInfo) -> bool {
    info.task_id.is_none() && info.title == title_for(info.cli, &info.cwd, &info.prompt)
}

/// Claude Code names the task in the terminal title (OSC 0) behind a spinner glyph, such as
/// "✳ Fix the login bug". Returns the last such name in the chunk. A title without a glyph is
/// the program path set by the console, and "Claude Code" is its name before it has a task.
fn terminal_task_name(text: &str) -> Option<String> {
    let mut found = None;
    let mut rest = text;
    while let Some(at) = rest.find("\x1b]") {
        rest = &rest[at + 2..];
        let Some(body) = rest.strip_prefix("0;").or_else(|| rest.strip_prefix("2;")) else { continue };
        let Some(end) = body.find(['\x07', '\x1b']) else { break };
        rest = &body[end..];
        if let Some((glyph, name)) = body[..end].trim().split_once(' ') {
            let name = name.trim();
            if !glyph.chars().any(char::is_alphanumeric) && !name.is_empty() && name != "Claude Code" {
                found = Some(name.to_string());
            }
        }
    }
    found
}

pub fn folder_name(path: &str) -> String {
    Path::new(path.trim_end_matches(['\\', '/']))
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string())
}

fn split_args(raw: Option<&String>) -> Vec<String> {
    raw.map(|r| r.split_whitespace().map(str::to_owned).collect())
        .unwrap_or_default()
}

/// Decodes a byte chunk, holding back an incomplete UTF-8 sequence for the next chunk.
fn decode_utf8(carry: &mut Vec<u8>, chunk: &[u8]) -> String {
    carry.extend_from_slice(chunk);
    match std::str::from_utf8(carry) {
        Ok(s) => {
            let out = s.to_string();
            carry.clear();
            out
        }
        Err(e) if e.error_len().is_none() => {
            let valid = e.valid_up_to();
            let out = String::from_utf8_lossy(&carry[..valid]).into_owned();
            carry.drain(..valid);
            out
        }
        Err(_) => {
            let out = String::from_utf8_lossy(carry).into_owned();
            carry.clear();
            out
        }
    }
}

fn event_summary(ev: &SessionEvent) -> Option<String> {
    let text = match ev {
        SessionEvent::Message { text } => text.lines().next().unwrap_or_default().to_string(),
        SessionEvent::ToolCall { tool, summary } => format!("{tool} {summary}"),
        SessionEvent::ToolFailed { tool, message } => format!("{tool} failed: {message}"),
        SessionEvent::FileChanged { path } => format!("Edited {}", folder_name(path)),
        SessionEvent::PermissionRequest { tool, .. } => format!("Asked to use {tool}"),
        SessionEvent::PermissionDenied { tool, .. } => format!("{tool} was refused"),
        SessionEvent::Retrying { message } | SessionEvent::Error { message } => message.clone(),
        SessionEvent::Done { ok: true, .. } => "Finished".into(),
        SessionEvent::Done { ok: false, summary } => format!("Failed: {summary}"),
        SessionEvent::Started { .. } | SessionEvent::Raw { .. } => return None,
    };
    let trimmed = text.trim();
    (!trimmed.is_empty()).then(|| trimmed.chars().take(140).collect())
}

/// Settings file injected into interactive Claude Code so its hooks report state to us.
/// Hook commands run in Git Bash, which Claude Code on Windows requires.
fn claude_hook_settings(hook_file: &Path) -> String {
    // Single-quoted for Git Bash, so a quote in the path (C:\Users\O'Neil) closes and reopens it.
    let target = hook_file.display().to_string().replace('\\', "/").replace('\'', r"'\''");
    let cmd = format!("cat >> '{target}'; echo >> '{target}'");
    let hook = serde_json::json!([{ "hooks": [{ "type": "command", "command": cmd }] }]);
    serde_json::json!({
        "hooks": {
            "PermissionRequest": hook,
            "Notification": hook,
            "Stop": hook,
            "UserPromptSubmit": hook,
            "PostToolUse": hook,
        }
    })
    .to_string()
}

impl Manager {
    pub fn new(db: Arc<Db>, emit: Arc<dyn Emit>, data_dir: PathBuf) -> Arc<Self> {
        let _ = fs::create_dir_all(data_dir.join("sessions"));
        let _ = fs::create_dir_all(data_dir.join("hooks"));
        Arc::new(Self {
            db,
            emit,
            data_dir,
            live: Mutex::new(HashMap::new()),
            spawning: Mutex::new(HashSet::new()),
        })
    }

    pub fn db(&self) -> &Arc<Db> {
        &self.db
    }

    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    /// The Board changed outside a session (a card was added, moved or run).
    pub fn tasks_changed(&self) {
        self.emit.tasks_changed();
    }

    pub fn chat_changed(&self, thread_id: &str) {
        self.emit.chat_changed(thread_id);
    }

    /// This app and the CLIs it runs now. A finished session's PID is left out: Windows hands
    /// it to new processes, which would then be hidden from the outside list.
    pub fn own_pids(&self) -> Vec<u32> {
        let mut pids = vec![std::process::id()];
        if let Ok(map) = self.live.lock() {
            for live in map.values() {
                if let Ok(info) = live.info.lock() {
                    if info.status.is_live() {
                        pids.extend(info.pid);
                    }
                }
            }
        }
        pids
    }

    /// Claims `id` for a spawn; `None` while another Resume or follow-up is starting it.
    fn claim_spawn(&self, id: &str) -> Option<Spawning<'_>> {
        let mut set = self.spawning.lock().ok()?;
        set.insert(id.to_string()).then(|| Spawning {
            set: &self.spawning,
            id: id.to_string(),
        })
    }

    fn resolve_exe(&self, kind: CliKind) -> Result<PathBuf, String> {
        let settings = self.db.settings()?;
        if let Some(custom) = settings.cli_paths.get(kind.bin()).filter(|p| !p.trim().is_empty()) {
            let p = PathBuf::from(custom);
            return if p.is_file() {
                Ok(p)
            } else {
                Err(format!("The custom path for {} does not exist: {custom}", kind.label()))
            };
        }
        cli::resolve(kind).ok_or_else(|| format!("{} is not installed or not on PATH.", kind.label()))
    }

    fn extra_args(&self, kind: CliKind) -> Vec<String> {
        self.db
            .settings()
            .map(|s| split_args(s.cli_args.get(kind.bin())))
            .unwrap_or_default()
    }

    fn log_path(&self, id: &str) -> PathBuf {
        self.data_dir.join("sessions").join(format!("{id}.log"))
    }

    fn hook_path(&self, id: &str) -> PathBuf {
        self.data_dir.join("hooks").join(format!("{id}.jsonl"))
    }

    /// The `--settings` file that points Claude Code's hooks at `hook_path`.
    fn hook_settings_path(&self, id: &str) -> PathBuf {
        self.data_dir.join("hooks").join(format!("{id}.settings.json"))
    }

    // Lifecycle

    pub fn start(self: &Arc<Self>, req: StartRequest) -> Result<SessionInfo, String> {
        let cwd = req.cwd.trim().to_string();
        if cwd.is_empty() || !Path::new(&cwd).is_dir() {
            return Err("This folder does not exist.".into());
        }
        if req.mode == Mode::Headless && req.prompt.trim().is_empty() {
            return Err("Headless sessions need a prompt.".into());
        }
        if req.mode == Mode::Headless && req.cli == CliKind::Gemini {
            return Err("Headless mode for Gemini CLI is not supported yet.".into());
        }
        let exe = self.resolve_exe(req.cli)?;
        let now = db::now_ms();
        let info = SessionInfo {
            id: db::new_id(),
            cli: req.cli,
            cwd: cwd.clone(),
            mode: req.mode,
            title: title_for(
                req.cli,
                &cwd,
                req.title.as_deref().filter(|t| !t.trim().is_empty()).unwrap_or(&req.prompt),
            ),
            prompt: req.prompt.trim().to_string(),
            status: Status::Starting,
            pid: None,
            cli_session_id: None,
            started_at: now,
            ended_at: None,
            exit_code: None,
            last_event: None,
            waiting: None,
            source: req.source.clone().unwrap_or_else(|| "manual".into()),
            task_id: req.task_id.clone(),
            // The session's own choice, else the default from Settings.
            permission_mode: Some(
                PermMode::parse(
                    req.permission_mode
                        .as_deref()
                        .filter(|m| !m.is_empty())
                        .unwrap_or(&self.db.settings()?.permission_mode),
                )
                .as_str()
                .to_string(),
            ),
            updated_at: now,
        };
        self.db.upsert_session(&info)?;
        self.db.touch_project(&cwd)?;
        let live = self.register(info.clone(), req.cli == CliKind::Claude && req.mode == Mode::Interactive);

        let result = match req.mode {
            Mode::Interactive => self.spawn_pty(&live, &exe, &req.prompt, None, req.cols, req.rows),
            Mode::Headless => self.spawn_turn(&live, &exe, &req.prompt, None),
        };
        if let Err(e) = result {
            self.finish(&live, Status::Error, None, Some(format!("Could not start: {e}")));
            return Err(e);
        }
        Ok(self.info(&live))
    }

    fn register(&self, info: SessionInfo, with_hooks: bool) -> Arc<Live> {
        let hook_file = with_hooks.then(|| self.hook_path(&info.id));
        let log = OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.log_path(&info.id))
            .ok();
        let id = info.id.clone();
        let live = Arc::new(Live {
            info: Mutex::new(info),
            runner: Mutex::new(Runner::None),
            output: Mutex::new(String::new()),
            output_seq: AtomicU64::new(0),
            utf8_carry: Mutex::new(Vec::new()),
            log: Mutex::new(log),
            last_output: Mutex::new(Instant::now()),
            stop_requested: AtomicBool::new(false),
            generation: AtomicU64::new(0),
            turn_failed: AtomicBool::new(false),
            pending_input: Mutex::new(None),
            hook_file,
            hook_idle: AtomicBool::new(false),
        });
        if let Ok(mut map) = self.live.lock() {
            map.insert(id, Arc::clone(&live));
        }
        live
    }

    fn info(&self, live: &Live) -> SessionInfo {
        live.info.lock().map(|i| i.clone()).unwrap_or_else(|p| p.into_inner().clone())
    }

    /// Applies a change, persists it, and announces it. Returns the updated copy.
    fn update(&self, live: &Live, f: impl FnOnce(&mut SessionInfo)) -> SessionInfo {
        let (before, after) = {
            let mut info = live.info.lock().unwrap_or_else(|p| p.into_inner());
            let before = info.status;
            f(&mut info);
            info.updated_at = db::now_ms();
            // Stored and announced under the lock: a reader thread's copy taken just before a
            // finish can then never land after it and show the session running again.
            let _ = self.db.upsert_session(&info);
            self.emit.session(&info);
            (before, info.clone())
        };
        if before != after.status {
            self.on_status_change(before, &after);
        }
        after
    }

    fn on_status_change(&self, before: Status, info: &SessionInfo) {
        let settings = self.db.settings().unwrap_or_default();
        let place = folder_name(&info.cwd);
        let who = info.cli.label();
        match info.status {
            Status::Waiting if settings.notifies(Notice::Waiting, info.cli, &info.cwd) => {
                let what = info
                    .waiting
                    .as_ref()
                    .map(|w| match w.reason.as_str() {
                        "permission" => format!("{who} needs your permission"),
                        "trust_folder" => format!("{who} asks whether you trust this folder"),
                        "update_offer" => format!("{who} is asking about an update"),
                        _ => format!("{who} is waiting for you"),
                    })
                    .unwrap_or_else(|| format!("{who} is waiting for you"));
                self.emit.notify(&what, &format!("{place} · {}", info.title), &info.id);
            }
            Status::Done if settings.notifies(Notice::Done, info.cli, &info.cwd) && before != Status::Done => {
                self.emit.notify(&format!("{who} finished"), &format!("{place} · {}", info.title), &info.id);
            }
            Status::Error if settings.notifies(Notice::Error, info.cli, &info.cwd) => {
                let why = info.last_event.clone().unwrap_or_default();
                self.emit.notify(&format!("{who} stopped with an error"), &format!("{place} · {why}"), &info.id);
            }
            _ => {}
        }
        // Board (PRD FR-74): a finished session moves its card to Done; an error leaves it.
        // The card is found through the session's own task id first, so a CLI that finishes
        // before the card stored its session id still moves it.
        let task = match &info.task_id {
            Some(tid) => self.db.task(tid).ok().flatten(),
            None => self.db.task_for_session(&info.id).ok().flatten(),
        };
        if info.status == Status::Done {
            if let Some(mut task) = task {
                if task.column != "done" {
                    task.column = "done".into();
                    task.position = self.db.next_position("done").unwrap_or(1.0);
                    task.updated_at = db::now_ms();
                    let _ = self.db.save_task(&task);
                    self.emit.tasks_changed();
                }
            }
        } else if task.is_some() && matches!(info.status, Status::Waiting | Status::Error | Status::Running) {
            self.emit.tasks_changed();
        }
    }

    fn record(&self, live: &Live, ev: SessionEvent) {
        let id = self.info(live).id;
        let at = db::now_ms();
        if let Ok(row_id) = self.db.add_event(&id, at, &ev) {
            self.emit.event(&EventRow {
                id: row_id,
                session_id: id,
                at,
                event: ev,
            });
        }
    }

    fn finish(&self, live: &Live, status: Status, code: Option<i32>, note: Option<String>) {
        let id = self
            .update(live, |i| {
                i.status = status;
                i.exit_code = code.or(i.exit_code);
                i.ended_at = Some(db::now_ms());
                i.waiting = None;
                if let Some(n) = note {
                    i.last_event = Some(n);
                }
            })
            .id;
        if let Ok(mut r) = live.runner.lock() {
            *r = Runner::None;
        }
        // A finished session keeps nothing in memory: its terminal output is in the log, and
        // Resume or a follow-up registers it again. Only this copy leaves, not a newer one.
        if let Ok(mut map) = self.live.lock() {
            if map.get(&id).is_some_and(|l| std::ptr::eq(Arc::as_ptr(l), live)) {
                map.remove(&id);
            }
        }
    }

    // Interactive

    fn spawn_pty(
        self: &Arc<Self>,
        live: &Arc<Live>,
        exe: &Path,
        prompt: &str,
        resume: Option<&str>,
        cols: Option<u16>,
        rows: Option<u16>,
    ) -> Result<(), String> {
        let info = self.info(live);
        let mut args = Vec::new();
        if let Some(hook_file) = &live.hook_file {
            let settings_path = self.hook_settings_path(&info.id);
            fs::write(&settings_path, claude_hook_settings(hook_file)).map_err(|e| e.to_string())?;
            let _ = File::create(hook_file);
            args.extend(["--settings".to_string(), settings_path.display().to_string()]);
        }
        let mode = PermMode::parse(info.permission_mode.as_deref().unwrap_or("ask"));
        let (cli_args, env) = headless::interactive_args(info.cli, prompt, resume, mode, &self.extra_args(info.cli));
        args.extend(cli_args);

        let sink_live = Arc::clone(live);
        let sink_self = Arc::clone(self);
        let session_id = info.id.clone();
        let names_task = info.cli == CliKind::Claude;
        let session = PtySession::spawn(
            PtySpec {
                program: exe.to_path_buf(),
                args,
                cwd: PathBuf::from(&info.cwd),
                cols: cols.unwrap_or(120),
                rows: rows.unwrap_or(32),
                env,
            },
            Box::new(move |bytes| {
                let text = {
                    let mut carry = sink_live.utf8_carry.lock().unwrap_or_else(|p| p.into_inner());
                    decode_utf8(&mut carry, bytes)
                };
                if let Ok(mut log) = sink_live.log.lock() {
                    if let Some(f) = log.as_mut() {
                        let _ = f.write_all(bytes);
                    }
                }
                if let Ok(mut t) = sink_live.last_output.lock() {
                    *t = Instant::now();
                }
                if text.is_empty() {
                    return;
                }
                let seq = {
                    let mut out = sink_live.output.lock().unwrap_or_else(|p| p.into_inner());
                    out.push_str(&text);
                    if out.len() > OUTPUT_KEEP {
                        let mut cut = out.len() - OUTPUT_KEEP;
                        while !out.is_char_boundary(cut) {
                            cut += 1;
                        }
                        out.drain(..cut);
                    }
                    sink_live.output_seq.fetch_add(1, Ordering::SeqCst) + 1
                };
                sink_self.emit.output(&session_id, &text, seq);
                if let Some(name) = names_task.then(|| terminal_task_name(&text)).flatten() {
                    sink_self.adopt_task_name(&sink_live, &name);
                }
            }),
        )?;
        let pid = session.pid();
        let generation = live.generation.fetch_add(1, Ordering::SeqCst) + 1;
        if let Ok(mut r) = live.runner.lock() {
            *r = Runner::Pty(session);
        }
        self.update(live, |i| {
            i.pid = pid;
            i.status = Status::Running;
            i.ended_at = None;
            i.exit_code = None;
        });

        let me = Arc::clone(self);
        let watched = Arc::clone(live);
        thread::spawn(move || me.watch_pty(watched, generation));
        Ok(())
    }

    /// Polls exit, hook reports and the screen for one interactive session.
    fn watch_pty(&self, live: Arc<Live>, generation: u64) {
        let mut hook_offset = 0u64;
        let mut screen_waiting_misses = 0;
        let mut screen_showed_dialog = false;
        loop {
            thread::sleep(Duration::from_millis(300));
            if live.generation.load(Ordering::SeqCst) != generation {
                return;
            }
            let exit = match live.runner.lock() {
                Ok(mut r) => match &mut *r {
                    Runner::Pty(p) => p.exit_code(),
                    _ => return,
                },
                Err(_) => return,
            };
            if let Some(code) = exit {
                let stopped = live.stop_requested.load(Ordering::SeqCst);
                let status = if stopped {
                    Status::Stopped
                } else if code == 0 {
                    Status::Done
                } else {
                    Status::Error
                };
                let note = if stopped {
                    "Stopped by you".to_string()
                } else {
                    format!("Exited with code {code}")
                };
                self.finish(&live, status, Some(code as i32), Some(note));
                return;
            }

            if let Some(file) = &live.hook_file {
                for line in read_new_lines(file, &mut hook_offset) {
                    self.apply_hook(&live, &line);
                }
            }

            let info = self.info(&live);
            let screen = match live.runner.lock() {
                Ok(r) => match &*r {
                    Runner::Pty(p) => p.screen_text(),
                    _ => String::new(),
                },
                Err(_) => String::new(),
            };
            let seen = waiting::detect(info.cli, &screen);
            match (&info.waiting, seen) {
                (None, Some(p)) => {
                    let can_answer = info.cli == CliKind::Claude && p.reason == waiting::WaitReason::Permission;
                    let reason = serde_json::to_value(p.reason)
                        .ok()
                        .and_then(|v| v.as_str().map(str::to_owned))
                        .unwrap_or_default();
                    self.update(&live, |i| {
                        i.status = Status::Waiting;
                        i.waiting = Some(Waiting {
                            reason,
                            tool: None,
                            detail: p.question.clone(),
                            request_id: None,
                            can_answer,
                            method: "screen".into(),
                            since: db::now_ms(),
                        });
                        i.last_event = Some(p.question);
                    });
                    screen_waiting_misses = 0;
                    screen_showed_dialog = true;
                }
                (Some(_), Some(_)) => {
                    screen_waiting_misses = 0;
                    screen_showed_dialog = true;
                }
                (Some(w), None) => {
                    // The dialog left the screen: answered in the terminal or from here. A
                    // hook report is trusted until its dialog has been seen and then gone,
                    // since the text patterns do not know every dialog variant.
                    screen_waiting_misses += 1;
                    let seen_then_gone = w.method == "screen" || screen_showed_dialog;
                    if seen_then_gone && screen_waiting_misses >= 2 {
                        self.update(&live, |i| {
                            i.status = Status::Running;
                            i.waiting = None;
                        });
                        screen_waiting_misses = 0;
                        screen_showed_dialog = false;
                    }
                }
                (None, None) => screen_showed_dialog = false,
            }

            let quiet = live
                .last_output
                .lock()
                .map(|t| t.elapsed() > IDLE_AFTER)
                .unwrap_or(false);
            let info = self.info(&live);
            if info.waiting.is_none() {
                if quiet && info.status == Status::Running {
                    self.update(&live, |i| i.status = Status::Idle);
                } else if !quiet && info.status == Status::Idle && !live.hook_idle.load(Ordering::SeqCst) {
                    self.update(&live, |i| i.status = Status::Running);
                }
            }
        }
    }

    /// Swaps a derived title for the name Claude Code gave the task.
    fn adopt_task_name(&self, live: &Live, name: &str) {
        let info = self.info(live);
        let title = title_for(info.cli, &info.cwd, name);
        if title_is_derived(&info) && title != info.title {
            self.update(live, |i| i.title = title);
        }
    }

    /// Claude Code hook reports (PRD FR-16, verified in M0).
    fn apply_hook(&self, live: &Live, line: &str) {
        let Ok(v) = serde_json::from_str::<Value>(line) else { return };
        if let Some(sid) = v["session_id"].as_str() {
            if self.info(live).cli_session_id.as_deref() != Some(sid) {
                let sid = sid.to_string();
                self.update(live, |i| i.cli_session_id = Some(sid));
            }
        }
        let event = v["hook_event_name"].as_str();
        if event.is_some() {
            live.hook_idle.store(event == Some("Stop"), Ordering::SeqCst);
        }
        match event {
            Some("PermissionRequest") => {
                let tool = v["tool_name"].as_str().unwrap_or_default().to_string();
                let detail = hook_input_summary(&v["tool_input"]);
                self.record(
                    live,
                    SessionEvent::PermissionRequest {
                        request_id: String::new(),
                        tool: tool.clone(),
                        summary: detail.clone(),
                    },
                );
                self.update(live, |i| {
                    i.status = Status::Waiting;
                    i.waiting = Some(Waiting {
                        reason: "permission".into(),
                        tool: Some(tool.clone()),
                        detail: detail.clone(),
                        request_id: None,
                        can_answer: true,
                        method: "hook".into(),
                        since: db::now_ms(),
                    });
                    i.last_event = Some(format!("Asked to use {tool}"));
                });
            }
            Some("Notification") if v["notification_type"] == "permission_prompt" => {
                if self.info(live).waiting.is_none() {
                    self.update(live, |i| {
                        i.status = Status::Waiting;
                        i.waiting = Some(Waiting {
                            reason: "permission".into(),
                            tool: None,
                            detail: v["message"].as_str().unwrap_or_default().to_string(),
                            request_id: None,
                            can_answer: true,
                            method: "hook".into(),
                            since: db::now_ms(),
                        });
                    });
                }
            }
            Some("UserPromptSubmit") | Some("PostToolUse") => {
                if let Some(tool) = v["tool_name"].as_str() {
                    self.record(
                        live,
                        SessionEvent::ToolCall {
                            tool: tool.to_string(),
                            summary: hook_input_summary(&v["tool_input"]),
                        },
                    );
                }
                let typed = v["prompt"].as_str().map(str::trim).filter(|p| !p.is_empty() && !p.starts_with('/'));
                self.update(live, |i| {
                    i.status = Status::Running;
                    i.waiting = None;
                    if let Some(tool) = v["tool_name"].as_str() {
                        i.last_event = Some(format!("{tool} {}", hook_input_summary(&v["tool_input"])));
                    }
                    // A session started without a prompt takes its first message as the task.
                    if let Some(p) = typed.filter(|_| i.prompt.is_empty()) {
                        let derived = title_is_derived(i);
                        i.prompt = p.to_string();
                        if derived {
                            i.title = title_for(i.cli, &i.cwd, p);
                        }
                    }
                });
            }
            Some("Stop") => {
                self.update(live, |i| {
                    i.status = Status::Idle;
                    i.waiting = None;
                    i.last_event = Some("Waiting for your next message".into());
                });
            }
            _ => {}
        }
    }

    // Headless

    fn spawn_turn(self: &Arc<Self>, live: &Arc<Live>, exe: &Path, prompt: &str, resume: Option<&str>) -> Result<(), String> {
        let info = self.info(live);
        let extra = self.extra_args(info.cli);
        let opts = TurnOptions {
            mode: PermMode::parse(info.permission_mode.as_deref().unwrap_or("ask")),
            resume,
            extra: &extra,
        };
        let inv = headless::invocation(info.cli, Path::new(&info.cwd), prompt, &opts)
            .ok_or("Headless mode is not supported for this CLI.")?;
        live.turn_failed.store(false, Ordering::SeqCst);

        let sink_self = Arc::clone(self);
        let sink_live = Arc::clone(live);
        let kind = info.cli;
        let run = HeadlessRun::spawn(
            &exe.to_path_buf(),
            Path::new(&info.cwd),
            inv,
            Box::new(move |stream, line| sink_self.on_headless_line(&sink_live, kind, stream, line)),
        )?;
        let pid = run.pid();
        let generation = live.generation.fetch_add(1, Ordering::SeqCst) + 1;
        if let Ok(mut r) = live.runner.lock() {
            *r = Runner::Headless(run);
        }
        self.update(live, |i| {
            i.pid = Some(pid);
            i.status = Status::Running;
            i.ended_at = None;
            i.exit_code = None;
            i.waiting = None;
        });

        let me = Arc::clone(self);
        let watched = Arc::clone(live);
        thread::spawn(move || me.watch_headless(watched, generation));
        Ok(())
    }

    fn on_headless_line(&self, live: &Live, kind: CliKind, stream: Stream, line: &str) {
        if let Ok(mut log) = live.log.lock() {
            if let Some(f) = log.as_mut() {
                let tag = if stream == Stream::Stdout { "O" } else { "E" };
                let _ = writeln!(f, "{tag} {line}");
            }
        }
        let evs = match stream {
            Stream::Stdout => {
                if let Some(sid) = events::cli_session_id(kind, line) {
                    if self.info(live).cli_session_id.as_deref() != Some(sid.as_str()) {
                        self.update(live, |i| i.cli_session_id = Some(sid));
                    }
                }
                if kind == CliKind::Claude {
                    if let Ok(v) = serde_json::from_str::<Value>(line) {
                        if v["type"] == "control_request" && v["request"]["subtype"] == "can_use_tool" {
                            if let Ok(mut p) = live.pending_input.lock() {
                                *p = Some(v["request"]["input"].clone());
                            }
                        }
                        if v["type"] == "result" {
                            // One turn per process: closing stdin lets Claude exit.
                            if let Ok(mut r) = live.runner.lock() {
                                if let Runner::Headless(h) = &mut *r {
                                    h.close_stdin();
                                }
                            }
                        }
                    }
                }
                events::parse_line(kind, line)
            }
            Stream::Stderr if kind == CliKind::Opencode => events::opencode_stderr(line).into_iter().collect(),
            Stream::Stderr => vec![],
        };

        for ev in evs {
            if matches!(ev, SessionEvent::Raw { .. }) {
                continue;
            }
            match &ev {
                SessionEvent::PermissionRequest { request_id, tool, summary } => {
                    let (rid, tool, summary) = (request_id.clone(), tool.clone(), summary.clone());
                    self.update(live, |i| {
                        i.status = Status::Waiting;
                        i.waiting = Some(Waiting {
                            reason: "permission".into(),
                            tool: Some(tool.clone()),
                            detail: summary,
                            request_id: Some(rid),
                            can_answer: true,
                            method: "stdio".into(),
                            since: db::now_ms(),
                        });
                        i.last_event = Some(format!("Asked to use {tool}"));
                    });
                }
                SessionEvent::Done { ok: false, .. } => live.turn_failed.store(true, Ordering::SeqCst),
                _ => {}
            }
            if let Some(summary) = event_summary(&ev) {
                self.update(live, |i| i.last_event = Some(summary));
            }
            self.record(live, ev);
        }
    }

    fn watch_headless(&self, live: Arc<Live>, generation: u64) {
        loop {
            thread::sleep(Duration::from_millis(250));
            if live.generation.load(Ordering::SeqCst) != generation {
                return;
            }
            let code = match live.runner.lock() {
                Ok(mut r) => match &mut *r {
                    Runner::Headless(h) => h.try_exit_code(),
                    _ => return,
                },
                Err(_) => return,
            };
            let Some(code) = code else { continue };
            // Give the reader threads a moment to deliver the last lines.
            thread::sleep(Duration::from_millis(300));
            let status = if live.stop_requested.load(Ordering::SeqCst) {
                Status::Stopped
            } else if code != 0 || live.turn_failed.load(Ordering::SeqCst) {
                Status::Error
            } else {
                Status::Done
            };
            let note = match status {
                Status::Stopped => Some("Stopped by you".to_string()),
                Status::Error if code != 0 => Some(format!("Exited with code {code}")),
                _ => None,
            };
            self.finish(&live, status, Some(code), note);
            return;
        }
    }

    // Actions

    fn get_live(&self, id: &str) -> Result<Arc<Live>, String> {
        self.live
            .lock()
            .map_err(|e| e.to_string())?
            .get(id)
            .cloned()
            .ok_or_else(|| "This session is not running in OpenCompanion.".into())
    }

    /// Approve or Deny (PRD FR-17). Headless Claude gets a control response; interactive
    /// Claude gets the keys its dialog expects (Enter picks "Yes", Esc cancels).
    pub fn answer(&self, id: &str, allow: bool) -> Result<SessionInfo, String> {
        self.answer_from(id, allow, None)
    }

    /// `device` names the paired phone, so the history says where the answer came from.
    pub fn answer_from(&self, id: &str, allow: bool, device: Option<&str>) -> Result<SessionInfo, String> {
        let live = self.get_live(id)?;
        let info = self.info(&live);
        let waiting = info.waiting.clone().ok_or("This session is not waiting for an answer.")?;
        if !waiting.can_answer {
            return Err("Answer this one in the session's terminal.".into());
        }
        {
            let mut runner = live.runner.lock().map_err(|e| e.to_string())?;
            match &mut *runner {
                Runner::Headless(h) => {
                    let rid = waiting.request_id.clone().unwrap_or_default();
                    let input = live
                        .pending_input
                        .lock()
                        .ok()
                        .and_then(|mut p| p.take())
                        .unwrap_or(Value::Object(Default::default()));
                    h.send_line(&headless::claude_permission_answer(&rid, allow, &input))?;
                }
                Runner::Pty(p) => p.write(if allow { b"\r" } else { b"\x1b" })?,
                Runner::None => return Err("This session has ended.".into()),
            }
        }
        let verdict = if allow { "Approved" } else { "Denied" };
        let tool = waiting.tool.clone().unwrap_or_else(|| "the request".into());
        let place = device.map(|d| format!("on {d}")).unwrap_or_else(|| "in OpenCompanion".into());
        self.record(
            &live,
            SessionEvent::Message {
                text: format!("{verdict} {tool} {place}"),
            },
        );
        Ok(self.update(&live, |i| {
            i.status = Status::Running;
            i.waiting = None;
            i.last_event = Some(format!("{verdict} {tool}"));
        }))
    }

    /// Typed input (PRD FR-12). Interactive: raw bytes to the terminal. Headless: a follow-up
    /// turn that resumes the CLI's own session once the previous turn has finished.
    pub fn send_input(self: &Arc<Self>, id: &str, text: &str) -> Result<SessionInfo, String> {
        let live = match self.get_live(id) {
            Ok(l) => l,
            Err(_) => return self.follow_up(id, text),
        };
        let info = self.info(&live);
        {
            let mut runner = live.runner.lock().map_err(|e| e.to_string())?;
            match &mut *runner {
                Runner::Pty(p) => {
                    p.write(text.as_bytes())?;
                    return Ok(info);
                }
                Runner::Headless(h) if info.cli == CliKind::Claude => {
                    h.send_line(&headless::claude_user_message(text))
                        .map_err(|_| "This turn is finishing. Send your message again in a moment.".to_string())?;
                    drop(runner);
                    self.record(&live, SessionEvent::Message { text: format!("You: {text}") });
                    return Ok(info);
                }
                Runner::Headless(_) => return Err("Wait for this turn to finish, then send the next message.".into()),
                Runner::None => {}
            }
        }
        self.follow_up(id, text)
    }

    /// A message for the session as a person would send it: typed into a terminal and then
    /// entered, or as the next turn of a headless conversation.
    pub fn send_message(self: &Arc<Self>, id: &str, text: &str) -> Result<SessionInfo, String> {
        let info = self.db.session(id)?.ok_or("Session not found.")?;
        match info.mode {
            Mode::Interactive => {
                // A terminal UI can read text and Enter that arrive together as a paste, so Enter follows on its own.
                self.send_input(id, &text.replace(['\r', '\n'], " "))?;
                thread::sleep(Duration::from_millis(150));
                self.send_input(id, "\r")
            }
            Mode::Headless => self.send_input(id, text),
        }
    }

    fn follow_up(self: &Arc<Self>, id: &str, text: &str) -> Result<SessionInfo, String> {
        let info = self.db.session(id)?.ok_or("Session not found.")?;
        if info.mode != Mode::Headless {
            return Err("This terminal has closed. Use Resume to open it again.".into());
        }
        let resume = info
            .cli_session_id
            .clone()
            .ok_or("This CLI did not report a session id, so the conversation cannot continue.")?;
        let exe = self.resolve_exe(info.cli)?;
        let _claim = self
            .claim_spawn(id)
            .ok_or("The next turn is starting. Send your message again in a moment.")?;
        let live = match self.get_live(id) {
            Ok(l) => l,
            Err(_) => self.register(info.clone(), false),
        };
        if live.runner.lock().map(|r| !matches!(*r, Runner::None)).unwrap_or(true) {
            return Err("This turn is finishing. Send your message again in a moment.".into());
        }
        live.stop_requested.store(false, Ordering::SeqCst);
        self.record(&live, SessionEvent::Message { text: format!("You: {text}") });
        self.spawn_turn(&live, &exe, text, Some(&resume))?;
        Ok(self.info(&live))
    }

    /// Reopens a finished interactive session with the CLI's own resume (PRD FR-15).
    pub fn resume(self: &Arc<Self>, id: &str, cols: Option<u16>, rows: Option<u16>) -> Result<SessionInfo, String> {
        let _claim = self.claim_spawn(id).ok_or("This session is already resuming.")?;
        let info = self.db.session(id)?.ok_or("Session not found.")?;
        if info.status.is_live() {
            return Err("This session is still running.".into());
        }
        let exe = self.resolve_exe(info.cli)?;
        match info.mode {
            Mode::Headless => Err("Send a message to continue a headless session.".into()),
            Mode::Interactive => {
                let live = self.register(info.clone(), info.cli == CliKind::Claude);
                live.stop_requested.store(false, Ordering::SeqCst);
                let resume = info.cli_session_id.clone();
                self.spawn_pty(&live, &exe, "", resume.as_deref(), cols, rows)?;
                Ok(self.info(&live))
            }
        }
    }

    pub fn resize(&self, id: &str, cols: u16, rows: u16) -> Result<(), String> {
        let live = self.get_live(id)?;
        let mut runner = live.runner.lock().map_err(|e| e.to_string())?;
        match &mut *runner {
            Runner::Pty(p) => p.resize(cols.max(20), rows.max(5)),
            _ => Ok(()),
        }
    }

    /// Stop (PRD FR-13): polite first, then a hard kill after `grace`.
    pub fn stop(&self, id: &str) -> Result<(), String> {
        self.stop_from(id, None)
    }

    pub fn stop_from(&self, id: &str, device: Option<&str>) -> Result<(), String> {
        let live = self.get_live(id)?;
        if let Some(d) = device {
            self.record(&live, SessionEvent::Message { text: format!("Stopped on {d}") });
        }
        live.stop_requested.store(true, Ordering::SeqCst);
        let grace = Duration::from_secs(3);
        let handle = Arc::clone(&live);
        thread::spawn(move || {
            let mut runner = handle.runner.lock().unwrap_or_else(|p| p.into_inner());
            match &mut *runner {
                Runner::Pty(p) => {
                    p.stop(grace);
                }
                Runner::Headless(h) => {
                    h.close_stdin();
                    let deadline = Instant::now() + grace;
                    while Instant::now() < deadline && h.try_exit_code().is_none() {
                        thread::sleep(Duration::from_millis(100));
                    }
                    if h.try_exit_code().is_none() {
                        h.kill();
                    }
                }
                Runner::None => {}
            }
        });
        Ok(())
    }

    /// Forgets a finished session: its row, events, terminal log and hook file. Files the CLI
    /// changed in the project are not touched.
    pub fn delete(&self, id: &str) -> Result<(), String> {
        let info = self.db.session(id)?.ok_or("Session not found.")?;
        let mut map = self.live.lock().map_err(|e| e.to_string())?;
        let running = map.get(id).map_or(info.status, |l| self.info(l).status);
        if running.is_live() {
            return Err("Stop this session before deleting it.".into());
        }
        // Holding the map lock keeps a Resume from registering it again mid-delete. Closing
        // the log here lets Windows remove the file while a stale watcher still holds `Live`.
        if let Some(live) = map.remove(id) {
            if let Ok(mut log) = live.log.lock() {
                *log = None;
            }
        }
        drop(map);
        self.db.delete_session(id)?;
        for path in [self.log_path(id), self.hook_path(id), self.hook_settings_path(id)] {
            match fs::remove_file(&path) {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
                    return Err(format!("The session is gone, but {} could not be removed: {e}", path.display()));
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// Settings › History "Delete finished sessions": every finished session with its files.
    /// Returns the ids that are gone, and why any could not go.
    pub fn delete_finished(&self) -> (Vec<String>, Vec<String>) {
        let ids = match self.db.finished_sessions(None) {
            Ok(ids) => ids,
            Err(e) => return (vec![], vec![e]),
        };
        let (mut gone, mut problems) = (Vec::new(), Vec::new());
        for id in ids {
            match self.delete(&id) {
                Ok(()) => gone.push(id),
                Err(e) => problems.push(e),
            }
        }
        self.sweep_files();
        (gone, problems)
    }

    /// Retention (PRD FR-62): finished sessions that ended more than `keep_days` ago go, with
    /// their events, terminal logs and hook files. `0` keeps everything. Returns the ids that went.
    pub fn prune(&self, keep_days: u32) -> Vec<String> {
        let mut gone = Vec::new();
        if keep_days > 0 {
            let cutoff = db::now_ms() - i64::from(keep_days) * 86_400_000;
            for id in self.db.finished_sessions(Some(cutoff)).unwrap_or_default() {
                if self.delete(&id).is_ok() {
                    gone.push(id);
                }
            }
        }
        self.sweep_files();
        gone
    }

    /// Terminal logs and hook files whose session is no longer stored, such as those an older
    /// Delete history left behind. A log can hold secrets the CLI printed, so none stays.
    fn sweep_files(&self) {
        for dir in [self.data_dir.join("sessions"), self.data_dir.join("hooks")] {
            let Ok(entries) = fs::read_dir(&dir) else { continue };
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().into_owned();
                let id = name.split('.').next().unwrap_or_default();
                let is_id = id.len() == 32 && id.bytes().all(|b| b.is_ascii_hexdigit());
                if is_id && matches!(self.db.session(id), Ok(None)) && self.get_live(id).is_err() {
                    let _ = fs::remove_file(entry.path());
                }
            }
        }
    }

    /// Terminal text for a view that attaches late, with the number of the last chunk in it. A view
    /// that listens first and reads this second writes only the chunks after that number.
    /// A session that is not running has no more chunks coming: its log tail and 0.
    pub fn output_snapshot(&self, id: &str) -> (String, u64) {
        if let Ok(live) = self.get_live(id) {
            if let Ok(out) = live.output.lock() {
                if !out.is_empty() {
                    return (out.clone(), live.output_seq.load(Ordering::SeqCst));
                }
            }
        }
        (read_tail(&self.log_path(id), 256 * 1024), 0)
    }

    /// Terminal text for a view that attaches late: the live buffer, or the log tail.
    pub fn output(&self, id: &str) -> String {
        if let Ok(live) = self.get_live(id) {
            if let Ok(out) = live.output.lock() {
                if !out.is_empty() {
                    return out.clone();
                }
            }
        }
        read_tail(&self.log_path(id), 256 * 1024)
    }

    /// App exit: no grace period, so no CLI outlives the window it was started from.
    pub fn kill_all(&self) {
        let lives: Vec<Arc<Live>> = self
            .live
            .lock()
            .map(|m| m.values().cloned().collect())
            .unwrap_or_default();
        for live in lives {
            live.stop_requested.store(true, Ordering::SeqCst);
            if let Ok(mut r) = live.runner.lock() {
                match &mut *r {
                    Runner::Pty(p) => {
                        p.stop(Duration::from_millis(200));
                    }
                    Runner::Headless(h) => h.kill(),
                    Runner::None => continue,
                }
            }
            self.finish(&live, Status::Stopped, None, Some("OpenCompanion was closed".into()));
        }
    }

    pub fn stop_all(&self) {
        let ids: Vec<String> = self
            .live
            .lock()
            .map(|m| m.keys().cloned().collect())
            .unwrap_or_default();
        for id in ids {
            let _ = self.stop(&id);
        }
    }
}

fn hook_input_summary(input: &Value) -> String {
    for key in ["command", "file_path", "path", "url", "pattern", "description"] {
        if let Some(text) = input.get(key).and_then(Value::as_str) {
            return text.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(300).collect();
        }
    }
    if input.is_null() {
        return String::new();
    }
    input.to_string().chars().take(300).collect()
}

/// Complete lines appended to `path` since `offset`.
fn read_new_lines(path: &Path, offset: &mut u64) -> Vec<String> {
    let Ok(mut f) = File::open(path) else { return vec![] };
    let Ok(len) = f.metadata().map(|m| m.len()) else { return vec![] };
    if len <= *offset {
        return vec![];
    }
    if f.seek(SeekFrom::Start(*offset)).is_err() {
        return vec![];
    }
    let mut buf = Vec::new();
    if f.read_to_end(&mut buf).is_err() {
        return vec![];
    }
    let Some(last_newline) = buf.iter().rposition(|b| *b == b'\n') else { return vec![] };
    *offset += last_newline as u64 + 1;
    String::from_utf8_lossy(&buf[..last_newline])
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_owned)
        .collect()
}

fn read_tail(path: &Path, max: u64) -> String {
    let Ok(mut f) = File::open(path) else { return String::new() };
    let len = f.metadata().map(|m| m.len()).unwrap_or(0);
    let start = len.saturating_sub(max);
    if f.seek(SeekFrom::Start(start)).is_err() {
        return String::new();
    }
    let mut buf = Vec::new();
    let _ = f.read_to_end(&mut buf);
    String::from_utf8_lossy(&buf).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf8_split_across_chunks_is_held_back() {
        let bytes = "héllo".as_bytes();
        let mut carry = Vec::new();
        let a = decode_utf8(&mut carry, &bytes[..2]);
        let b = decode_utf8(&mut carry, &bytes[2..]);
        assert_eq!(format!("{a}{b}"), "héllo");
        assert_eq!(a, "h");
    }

    #[test]
    fn titles_come_from_the_prompt_or_the_folder() {
        assert_eq!(title_for(CliKind::Codex, r"C:\p\uninote", "Fix tests\nthen more"), "Fix tests");
        assert_eq!(title_for(CliKind::Claude, r"C:\p\uninote\", "  "), "Claude Code in uninote");
    }

    #[test]
    fn task_names_come_from_claude_terminal_titles() {
        // Sequences as Claude Code printed them in a real session log.
        let chunk = "\x1b]0;C:\\Users\\me\\.local\\bin\\claude.exe\x07\x1b]0;claude\x07\x1b]0;◐ Claude Code\x07";
        assert_eq!(terminal_task_name(chunk), None);
        let chunk = "x\x1b]0;◐ OpenCompanion custom title bar\x07y\x1b]0;✳ OpenCompanion custom title bar\x07";
        assert_eq!(terminal_task_name(chunk).as_deref(), Some("OpenCompanion custom title bar"));
        assert_eq!(terminal_task_name("\x1b]2;⠂ Fix tests\x1b\\").as_deref(), Some("Fix tests"));
        assert_eq!(terminal_task_name("\x1b]0;✳ Cut off"), None);
        assert_eq!(terminal_task_name("\x1b[31mred\x1b[0m"), None);
    }

    #[test]
    fn hook_settings_point_every_event_at_the_file() {
        let s = claude_hook_settings(Path::new(r"C:\data\hooks\a.jsonl"));
        let v: Value = serde_json::from_str(&s).unwrap();
        let cmd = v["hooks"]["PermissionRequest"][0]["hooks"][0]["command"].as_str().unwrap();
        assert!(cmd.contains("'C:/data/hooks/a.jsonl'"));
        assert!(v["hooks"]["Stop"].is_array());

        // A quote in the profile folder closes the quoted path and opens it again.
        let s = claude_hook_settings(Path::new(r"C:\Users\O'Neil\hooks\a.jsonl"));
        let v: Value = serde_json::from_str(&s).unwrap();
        let cmd = v["hooks"]["Stop"][0]["hooks"][0]["command"].as_str().unwrap();
        assert_eq!(cmd, r"cat >> 'C:/Users/O'\''Neil/hooks/a.jsonl'; echo >> 'C:/Users/O'\''Neil/hooks/a.jsonl'");
    }

    struct NoEmit;
    impl Emit for NoEmit {
        fn session(&self, _: &SessionInfo) {}
        fn output(&self, _: &str, _: &str, _: u64) {}
        fn event(&self, _: &EventRow) {}
        fn tasks_changed(&self) {}
        fn notify(&self, _: &str, _: &str, _: &str) {}
    }

    fn stored(id: &str, status: Status) -> SessionInfo {
        SessionInfo {
            id: id.into(),
            cli: CliKind::Claude,
            cwd: "C:/w".into(),
            mode: Mode::Interactive,
            title: "t".into(),
            prompt: String::new(),
            status,
            pid: None,
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
    fn delete_refuses_a_live_session_and_removes_a_finished_one_with_its_files() {
        let dir = std::env::temp_dir().join(format!("air-delete-{}", std::process::id()));
        fs::create_dir_all(dir.join("sessions")).unwrap();
        fs::create_dir_all(dir.join("hooks")).unwrap();
        let db = Arc::new(Db::open_in_memory().unwrap());
        let m = Manager::new(Arc::clone(&db), Arc::new(NoEmit), dir.clone());

        db.upsert_session(&stored("live", Status::Running)).unwrap();
        assert_eq!(m.delete("live").unwrap_err(), "Stop this session before deleting it.");
        assert!(db.session("live").unwrap().is_some());

        let done = stored("done", Status::Done);
        db.upsert_session(&done).unwrap();
        m.register(done, true);
        assert!(m.log_path("done").exists());
        fs::write(m.hook_path("done"), "{}\n").unwrap();
        m.delete("done").unwrap();
        assert!(db.session("done").unwrap().is_none());
        assert!(!m.log_path("done").exists() && !m.hook_path("done").exists());
        assert_eq!(m.delete("done").unwrap_err(), "Session not found.");
        let _ = fs::remove_dir_all(&dir);
    }

    fn temp_manager(name: &str) -> (PathBuf, Arc<Db>, Arc<Manager>) {
        let dir = std::env::temp_dir().join(format!("air-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let db = Arc::new(Db::open_in_memory().unwrap());
        let m = Manager::new(Arc::clone(&db), Arc::new(NoEmit), dir.clone());
        (dir, db, m)
    }

    #[test]
    fn retention_removes_old_finished_sessions_with_their_files_and_sweeps_leftovers() {
        let (dir, db, m) = temp_manager("prune");
        let day = 86_400_000;
        let (old_id, new_id, live_id, orphan) = ("a".repeat(32), "b".repeat(32), "c".repeat(32), "d".repeat(32));
        let mut old = stored(&old_id, Status::Done);
        old.ended_at = Some(db::now_ms() - 10 * day);
        let mut recent = stored(&new_id, Status::Error);
        recent.ended_at = Some(db::now_ms() - day / 2);
        let mut running = stored(&live_id, Status::Running);
        running.started_at = db::now_ms() - 30 * day;
        for s in [&old, &recent, &running] {
            db.upsert_session(s).unwrap();
            fs::write(m.log_path(&s.id), "output").unwrap();
        }
        fs::write(m.hook_settings_path(&old_id), "{}").unwrap();
        fs::write(m.log_path(&orphan), "left behind").unwrap();
        fs::write(m.hook_path(&orphan), "{}").unwrap();
        fs::write(dir.join("sessions").join("notes.txt"), "not ours to judge").unwrap();

        // Keeping everything still sweeps files whose session is gone.
        assert!(m.prune(0).is_empty());
        assert!(!m.log_path(&orphan).exists() && !m.hook_path(&orphan).exists());
        assert!(dir.join("sessions").join("notes.txt").exists());

        assert_eq!(m.prune(7), vec![old_id.clone()]);
        assert!(db.session(&old_id).unwrap().is_none());
        assert!(!m.log_path(&old_id).exists() && !m.hook_settings_path(&old_id).exists());
        assert!(db.session(&new_id).unwrap().is_some() && m.log_path(&new_id).exists());
        assert!(db.session(&live_id).unwrap().is_some() && m.log_path(&live_id).exists());

        // Delete history takes every finished session and leaves the running one.
        let (gone, problems) = m.delete_finished();
        assert_eq!((gone, problems), (vec![new_id.clone()], vec![]));
        assert!(!m.log_path(&new_id).exists());
        assert!(db.session(&live_id).unwrap().is_some());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_finished_session_leaves_memory_and_the_own_pids() {
        let (dir, db, m) = temp_manager("own");
        let mut running = stored("run", Status::Running);
        running.pid = Some(111);
        let mut done = stored("done", Status::Done);
        done.pid = Some(222);
        db.upsert_session(&running).unwrap();
        db.upsert_session(&done).unwrap();
        let live = m.register(running, false);
        m.register(done, false);
        let own = m.own_pids();
        assert!(own.contains(&111) && !own.contains(&222));

        m.finish(&live, Status::Stopped, None, None);
        assert!(m.get_live("run").is_err());
        assert!(!m.own_pids().contains(&111));
        assert_eq!(db.session("run").unwrap().unwrap().status, Status::Stopped);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_second_resume_or_follow_up_waits_for_the_first() {
        let (dir, _db, m) = temp_manager("claim");
        let first = m.claim_spawn("s1");
        assert!(first.is_some());
        assert!(m.claim_spawn("s1").is_none());
        assert!(m.claim_spawn("s2").is_some());
        drop(first);
        assert!(m.claim_spawn("s1").is_some());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn new_lines_are_read_once_and_partial_lines_wait() {
        let dir = std::env::temp_dir().join(format!("air-lines-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let f = dir.join("h.jsonl");
        fs::write(&f, "{\"a\":1}\n{\"b\"").unwrap();
        let mut off = 0;
        assert_eq!(read_new_lines(&f, &mut off), ["{\"a\":1}"]);
        let mut h = OpenOptions::new().append(true).open(&f).unwrap();
        h.write_all(b":2}\n").unwrap();
        assert_eq!(read_new_lines(&f, &mut off), ["{\"b\":2}"]);
        assert!(read_new_lines(&f, &mut off).is_empty());
        let _ = fs::remove_dir_all(&dir);
    }
}
