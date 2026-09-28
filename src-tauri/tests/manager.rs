//! Session manager end to end, against `fake-cli` (src/bin/fake-cli.rs), which prints the
//! event formats captured from the real CLIs in the M0 spike.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use opencompanion_lib::actions::auto_run;
use opencompanion_lib::cli::CliKind;
use opencompanion_lib::db::{Db, DispatchCard, EventRow, Mode, SessionInfo, Settings, Status};
use opencompanion_lib::events::SessionEvent;
use opencompanion_lib::session::{Emit, Manager, StartRequest};

#[derive(Default)]
struct Recorder {
    output: Mutex<String>,
    seqs: Mutex<Vec<u64>>,
    events: Mutex<Vec<EventRow>>,
    notes: Mutex<Vec<String>>,
}

impl Emit for Recorder {
    fn session(&self, _info: &SessionInfo) {}
    fn output(&self, _id: &str, data: &str, seq: u64) {
        self.output.lock().unwrap().push_str(data);
        self.seqs.lock().unwrap().push(seq);
    }
    fn event(&self, row: &EventRow) {
        self.events.lock().unwrap().push(row.clone());
    }
    fn notify(&self, title: &str, _body: &str, _session_id: &str) {
        self.notes.lock().unwrap().push(title.to_string());
    }
}

struct Rig {
    manager: Arc<Manager>,
    db: Arc<Db>,
    rec: Arc<Recorder>,
    work: PathBuf,
}

fn rig(name: &str) -> Rig {
    let fake = env!("CARGO_BIN_EXE_fake-cli").to_string();
    let db = Arc::new(Db::open_in_memory().unwrap());
    let mut settings = Settings::default();
    for kind in ["claude", "codex", "opencode"] {
        settings.cli_paths.insert(kind.into(), fake.clone());
    }
    db.save_settings(&settings).unwrap();
    let base = std::env::temp_dir().join(format!("air-manager-{}-{name}", std::process::id()));
    let work = base.join("project");
    std::fs::create_dir_all(&work).unwrap();
    let rec = Arc::new(Recorder::default());
    let manager = Manager::new(Arc::clone(&db), rec.clone(), base.join("data"));
    Rig { manager, db, rec, work }
}

fn start(r: &Rig, cli: CliKind, mode: Mode, prompt: &str) -> SessionInfo {
    r.manager
        .start(StartRequest {
            cli,
            cwd: r.work.display().to_string(),
            mode,
            prompt: prompt.into(),
            title: None,
            permission_mode: None,
            source: None,
            cols: Some(100),
            rows: Some(30),
        })
        .expect("session starts")
}

fn wait_until(r: &Rig, id: &str, what: &str, pred: impl Fn(&SessionInfo) -> bool) -> SessionInfo {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        let s = r.db.session(id).unwrap().unwrap();
        if pred(&s) {
            return s;
        }
        assert!(Instant::now() < deadline, "timed out waiting for {what}; last status {:?}", s.status);
        thread::sleep(Duration::from_millis(50));
    }
}

fn messages(r: &Rig, id: &str) -> Vec<String> {
    r.db.events(id, 200)
        .unwrap()
        .into_iter()
        .filter_map(|e| match e.event {
            SessionEvent::Message { text } => Some(text),
            _ => None,
        })
        .collect()
}

#[test]
fn headless_turn_records_events_and_finishes() {
    let r = rig("headless");
    let s = start(&r, CliKind::Opencode, Mode::Headless, "add hello");
    let done = wait_until(&r, &s.id, "done", |s| s.status == Status::Done);
    assert_eq!(done.cli_session_id.as_deref(), Some("ses_fake1"));
    assert_eq!(done.exit_code, Some(0));

    let kinds: Vec<String> = r
        .db
        .events(&s.id, 50)
        .unwrap()
        .iter()
        .map(|e| serde_json::to_value(&e.event).unwrap()["kind"].as_str().unwrap().to_string())
        .collect();
    assert!(kinds.contains(&"tool_call".into()) && kinds.contains(&"file_changed".into()), "{kinds:?}");
    assert!(messages(&r, &s.id).iter().any(|m| m == "done: add hello"));
    assert!(r.rec.notes.lock().unwrap().iter().any(|n| n == "OpenCode finished"));

    // A follow-up resumes the CLI's own session in a new turn (PRD FR-12, FR-15).
    r.manager.send_input(&s.id, "again").unwrap();
    wait_until(&r, &s.id, "second turn", |_| messages(&r, &s.id).iter().any(|m| m == "done: again"));
    wait_until(&r, &s.id, "done again", |s| s.status == Status::Done);
}

#[test]
fn failing_turn_ends_in_error() {
    let r = rig("fail");
    let s = start(&r, CliKind::Opencode, Mode::Headless, "please fail");
    let s = wait_until(&r, &s.id, "error", |s| s.status == Status::Error);
    assert_eq!(s.exit_code, Some(3));
    assert_eq!(s.last_event.as_deref(), Some("Exited with code 3"));
}

#[test]
fn claude_permission_request_waits_then_approve_continues() {
    let r = rig("approve");
    let s = start(&r, CliKind::Claude, Mode::Headless, "write hello");
    let w = wait_until(&r, &s.id, "waiting", |s| s.status == Status::Waiting);
    let waiting = w.waiting.unwrap();
    assert_eq!(waiting.request_id.as_deref(), Some("req-1"));
    assert_eq!(waiting.method, "stdio");
    assert!(waiting.can_answer);

    r.manager.answer(&s.id, true).unwrap();
    let done = wait_until(&r, &s.id, "done", |s| s.status == Status::Done);
    assert_eq!(done.cli_session_id.as_deref(), Some("sess-fake-1"));
    let msgs = messages(&r, &s.id);
    assert!(msgs.iter().any(|m| m == "allowed"), "{msgs:?}");
    assert!(msgs.iter().any(|m| m.starts_with("Approved Write")), "{msgs:?}");
}

#[test]
fn deny_is_passed_to_the_cli() {
    let r = rig("deny");
    let s = start(&r, CliKind::Claude, Mode::Headless, "write hello");
    wait_until(&r, &s.id, "waiting", |s| s.status == Status::Waiting);
    r.manager.answer(&s.id, false).unwrap();
    wait_until(&r, &s.id, "done", |s| s.status == Status::Done);
    assert!(messages(&r, &s.id).iter().any(|m| m == "denied"));
}

#[test]
fn stop_ends_a_waiting_session_as_stopped() {
    let r = rig("stop");
    let s = start(&r, CliKind::Claude, Mode::Headless, "write hello");
    wait_until(&r, &s.id, "waiting", |s| s.status == Status::Waiting);
    r.manager.stop(&s.id).unwrap();
    let s = wait_until(&r, &s.id, "stopped", |s| s.status == Status::Stopped);
    assert!(s.waiting.is_none());
}

#[test]
fn done_ends_an_idle_terminal_as_done() {
    let r = rig("done");
    let s = start(&r, CliKind::Claude, Mode::Interactive, "");
    wait_until(&r, &s.id, "prompt on screen", |_| r.rec.output.lock().unwrap().contains("fake ready"));

    // Claude Code's Stop hook: the turn is over and the terminal waits for the next message.
    let hooks = r.work.parent().unwrap().join("data").join("hooks").join(format!("{}.jsonl", s.id));
    std::fs::write(&hooks, "{\"hook_event_name\":\"Stop\"}\n").unwrap();
    wait_until(&r, &s.id, "idle", |s| s.status == Status::Idle);

    r.manager.mark_done(&s.id).unwrap();
    let s = wait_until(&r, &s.id, "done", |s| !s.status.is_live());
    assert_eq!(s.status, Status::Done);
    assert_eq!(s.last_event.as_deref(), Some("Marked done by you"));
    // The user ended it, so no "finished" notification tells them what they just did.
    assert!(r.rec.notes.lock().unwrap().is_empty(), "{:?}", r.rec.notes.lock().unwrap());
}

#[test]
fn interactive_session_streams_output_and_takes_input() {
    let r = rig("pty");
    let s = start(&r, CliKind::Opencode, Mode::Interactive, "");
    wait_until(&r, &s.id, "prompt on screen", |_| r.rec.output.lock().unwrap().contains("fake ready"));

    // A late view's snapshot ends at the last chunk sent so far, and chunks count up from 1.
    thread::sleep(Duration::from_millis(300));
    let (text, seq) = r.manager.output_snapshot(&s.id);
    let seqs = r.rec.seqs.lock().unwrap().clone();
    assert!(text.contains("fake ready"));
    assert_eq!(seqs, (1..=seqs.len() as u64).collect::<Vec<_>>());
    assert_eq!(seq, *seqs.last().unwrap());

    r.manager.send_input(&s.id, "hello\r").unwrap();
    wait_until(&r, &s.id, "the CLI's reply", |_| r.manager.output(&s.id).contains("got: hello"));

    // The CLI exited, but its shell stays, as in a terminal of your own: typing the CLI's name
    // starts it again, and `exit` closes the terminal.
    thread::sleep(Duration::from_millis(1500));
    assert!(r.db.session(&s.id).unwrap().unwrap().status.is_live(), "the session ended with its CLI");
    r.manager.send_input(&s.id, "opencode\r").unwrap();
    wait_until(&r, &s.id, "the CLI again", |_| r.manager.output(&s.id).matches("fake ready").count() == 2);
    r.manager.send_input(&s.id, "bye\r").unwrap();
    wait_until(&r, &s.id, "the second reply", |_| r.manager.output(&s.id).contains("got: bye"));
    thread::sleep(Duration::from_millis(1500));
    r.manager.send_input(&s.id, "exit\r").unwrap();
    let done = wait_until(&r, &s.id, "the terminal closed", |s| !s.status.is_live());
    assert_eq!(done.status, Status::Done, "last: {:?}", done.last_event);
    assert_eq!(done.last_event.as_deref(), Some("Terminal closed"));
    // You closed it yourself, so no "finished" notification says so.
    assert!(r.rec.notes.lock().unwrap().is_empty(), "{:?}", r.rec.notes.lock().unwrap());
}

#[test]
fn an_empty_claude_session_is_named_by_its_first_message_then_by_claude() {
    let r = rig("names");
    let s = start(&r, CliKind::Claude, Mode::Interactive, "");
    assert_eq!(s.title, "Claude Code in project");
    wait_until(&r, &s.id, "prompt on screen", |_| r.rec.output.lock().unwrap().contains("fake ready"));

    let hooks = r.work.parent().unwrap().join("data").join("hooks").join(format!("{}.jsonl", s.id));
    let submit = serde_json::json!({ "hook_event_name": "UserPromptSubmit", "prompt": "Fix the login bug\nit fails on Enter" });
    std::fs::write(&hooks, format!("{submit}\n")).unwrap();
    let named = wait_until(&r, &s.id, "title from the message", |s| s.title == "Fix the login bug");
    assert_eq!(named.prompt, "Fix the login bug\nit fails on Enter");

    // fake-cli names the task in its terminal title the way Claude Code does.
    r.manager.send_input(&s.id, "Login bug fix\r").unwrap();
    wait_until(&r, &s.id, "title from the terminal", |s| s.title == "Login bug fix");
    // The shell outlives the CLI; end it with the test.
    r.manager.stop(&s.id).unwrap();
}

#[test]
fn a_card_name_outlives_the_cli_title() {
    let r = rig("card-name");
    let s = r
        .manager
        .start(StartRequest {
            cli: CliKind::Claude,
            cwd: r.work.display().to_string(),
            mode: Mode::Interactive,
            prompt: "Write docs for the export API".into(),
            title: Some("Export API docs".into()),
            permission_mode: None,
            source: Some("chat".into()),
            cols: None,
            rows: None,
        })
        .unwrap();
    assert_eq!(s.title, "Export API docs");
    wait_until(&r, &s.id, "prompt on screen", |_| r.rec.output.lock().unwrap().contains("fake ready"));
    r.manager.send_input(&s.id, "Something else\r").unwrap();
    let named = wait_until(&r, &s.id, "the CLI's reply", |_| r.manager.output(&s.id).contains("got: Something else"));
    // The terminal title arrives with the reply, so give it a moment to be read.
    thread::sleep(Duration::from_millis(500));
    assert_eq!(r.db.session(&named.id).unwrap().unwrap().title, "Export API docs");
    r.manager.stop(&s.id).unwrap();
}

#[test]
fn start_rejects_missing_folders_and_empty_headless_prompts() {
    let r = rig("reject");
    let bad = r.manager.start(StartRequest {
        cli: CliKind::Codex,
        cwd: r"C:\definitely\missing\folder".into(),
        mode: Mode::Interactive,
        prompt: String::new(),
        title: None,
        permission_mode: None,
        source: None,
        cols: None,
        rows: None,
    });
    assert_eq!(bad.unwrap_err(), "This folder does not exist.");
    let empty = r.manager.start(StartRequest {
        cli: CliKind::Codex,
        cwd: r.work.display().to_string(),
        mode: Mode::Headless,
        prompt: "  ".into(),
        title: None,
        permission_mode: None,
        source: None,
        cols: None,
        rows: None,
    });
    assert_eq!(empty.unwrap_err(), "Headless sessions need a prompt.");
}

#[test]
fn permission_mode_comes_from_settings_or_the_session_and_reaches_the_cli() {
    let r = rig("mode");
    let mut settings = r.db.settings().unwrap();
    settings.permission_mode = "plan".into();
    r.db.save_settings(&settings).unwrap();

    let a = start(&r, CliKind::Opencode, Mode::Headless, "look around");
    let a = wait_until(&r, &a.id, "done", |s| s.status == Status::Done);
    assert_eq!(a.permission_mode.as_deref(), Some("plan"));
    let seen = messages(&r, &a.id).into_iter().find(|m| m.starts_with("args:")).unwrap();
    assert!(seen.contains("--agent plan"), "{seen}");

    let b = r
        .manager
        .start(StartRequest {
            cli: CliKind::Opencode,
            cwd: r.work.display().to_string(),
            mode: Mode::Headless,
            prompt: "just do it".into(),
            title: None,
            permission_mode: Some("bypass".into()),
            source: None,
            cols: None,
            rows: None,
        })
        .unwrap();
    let b = wait_until(&r, &b.id, "done", |s| s.status == Status::Done);
    assert_eq!(b.permission_mode.as_deref(), Some("bypass"));
    let seen = messages(&r, &b.id).into_iter().find(|m| m.starts_with("args:")).unwrap();
    assert!(seen.contains("--auto") && seen.contains(r#"env: {"*":"allow"}"#), "{seen}");
}

fn card(id: &str, folder: &str) -> DispatchCard {
    DispatchCard {
        id: id.into(),
        cli: CliKind::Opencode,
        title: format!("Task {id}"),
        folder: folder.into(),
        prompt: format!("do {id}"),
        mode: Mode::Headless,
        reason: String::new(),
        problem: None,
        state: "proposed".into(),
        session_id: None,
        target: None,
        auto: false,
    }
}

/// PRD FR-26: cards for a folder that runs without asking start at once, never with Bypass;
/// every other card still waits for Run.
#[test]
fn cards_for_an_auto_run_folder_start_by_themselves_without_bypass() {
    let r = rig("autorun");
    let other = r.work.parent().unwrap().join("other");
    std::fs::create_dir_all(&other).unwrap();
    let mut settings = r.db.settings().unwrap();
    settings.permission_mode = "bypass".into();
    // Windows and macOS ignore case in folder rules; on Linux the case must match.
    let rule = r.work.display().to_string();
    settings.auto_run_folders = vec![if cfg!(any(windows, target_os = "macos")) { rule.to_uppercase() } else { rule }];
    r.db.save_settings(&settings).unwrap();

    let mut broken = card("broken", &r.work.display().to_string());
    broken.problem = Some("The card has no prompt.".into());
    let mut cards = vec![card("here", &r.work.display().to_string()), card("elsewhere", &other.display().to_string()), broken];
    auto_run(&r.manager, &settings, &mut cards);

    assert!(cards[0].auto && cards[0].state == "started");
    let id = cards[0].session_id.clone().unwrap();
    let s = wait_until(&r, &id, "auto session", |s| s.status == Status::Done);
    assert_eq!((s.permission_mode.as_deref(), s.source.as_str(), s.title.as_str()), (Some("ask"), "chat", "Task here"));
    assert!(!cards[1].auto && cards[1].state == "proposed" && cards[1].session_id.is_none());
    assert!(!cards[2].auto && cards[2].state == "proposed");
}
