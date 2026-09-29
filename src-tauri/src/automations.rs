//! Automations: sessions OpenCompanion starts by itself on a cron schedule, in the computer's local
//! time, while the app runs (the tray included). A schedule that came due while the app was closed
//! runs once when it runs again, marked late; the next run is counted from then.

use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;

use chrono::{Local, TimeZone};
use croner::Cron;
use serde::{Deserialize, Serialize};

use crate::cli::CliKind;
use crate::db::{self, Automation, AutomationRun, Db, Mode, SessionInfo, RUNS_KEPT};
use crate::headless::PermMode;
use crate::session::{Manager, StartRequest};

/// How often the scheduler looks for automations that came due.
pub const TICK_SECS: u64 = 30;
/// A start later than this after its time counts as late.
const LATE_AFTER_MS: i64 = 120_000;
/// Times the form previews.
const PREVIEW_COUNT: usize = 3;
/// croner's own messages ("Component error: Number out of bounds.") name no field, so the ranges do.
const NOT_VALID: &str =
    "This schedule is not valid. Minutes go from 0 to 59, hours 0 to 23, days 1 to 31, months 1 to 12 and weekdays 0 to 7 (0 and 7 are Sunday).";

type Res<T> = Result<T, String>;

/// What the form sends to create or change an automation.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Draft {
    pub name: String,
    pub cli: CliKind,
    pub cwd: String,
    pub mode: Mode,
    #[serde(default)]
    pub prompt: String,
    pub permission_mode: String,
    pub schedule: String,
    pub enabled: bool,
}

/// A row of the list: the automation and how its last run went.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct View {
    #[serde(flatten)]
    pub automation: Automation,
    pub last_run: Option<AutomationRun>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Detail {
    pub automation: Automation,
    pub runs: Vec<AutomationRun>,
}

fn parse(schedule: &str) -> Res<Cron> {
    let schedule = schedule.trim();
    if schedule.is_empty() {
        return Err("Choose when it runs.".into());
    }
    if schedule.split_whitespace().count() != 5 {
        return Err("Use five fields: minute, hour, day of month, month and day of week.".into());
    }
    Cron::from_str(schedule).map_err(|_| NOT_VALID.to_string())
}

fn local(ms: i64) -> Res<chrono::DateTime<Local>> {
    Local.timestamp_millis_opt(ms).earliest().ok_or_else(|| "This time does not exist here.".to_string())
}

fn next_of(cron: &Cron, after_ms: i64) -> Res<i64> {
    cron.find_next_occurrence(&local(after_ms)?, false)
        .map(|t| t.timestamp_millis())
        .map_err(|_| "This schedule never comes round.".to_string())
}

/// The first time after `after_ms` the schedule comes round.
pub fn next_after(schedule: &str, after_ms: i64) -> Res<i64> {
    next_of(&parse(schedule)?, after_ms)
}

/// The next few times after `from_ms`, for the form.
pub fn preview(schedule: &str, from_ms: i64) -> Res<Vec<i64>> {
    let cron = parse(schedule)?;
    let mut times = Vec::with_capacity(PREVIEW_COUNT);
    let mut after = from_ms;
    for _ in 0..PREVIEW_COUNT {
        after = next_of(&cron, after)?;
        times.push(after);
    }
    Ok(times)
}

/// "30 Sep 09:00" in local time, for the session title.
fn stamp(ms: i64) -> String {
    local(ms).map(|t| t.format("%-d %b %H:%M").to_string()).unwrap_or_default()
}

/// Checks the draft like New session does and stores it. `installed` says whether a CLI can start.
pub fn save(db: &Db, id: Option<&str>, draft: Draft, installed: impl Fn(CliKind) -> Res<()>) -> Res<Automation> {
    let name = draft.name.trim().to_string();
    if name.is_empty() {
        return Err("Give the automation a name.".into());
    }
    let cwd = draft.cwd.trim().to_string();
    if cwd.is_empty() || !Path::new(&cwd).is_dir() {
        return Err("This folder does not exist.".into());
    }
    let prompt = draft.prompt.trim().to_string();
    if draft.mode == Mode::Headless && prompt.is_empty() {
        return Err("Headless sessions need a prompt.".into());
    }
    if draft.mode == Mode::Headless && draft.cli == CliKind::Gemini {
        return Err("Headless mode for Gemini CLI is not supported yet.".into());
    }
    installed(draft.cli)?;
    let schedule = draft.schedule.split_whitespace().collect::<Vec<_>>().join(" ");
    let now = db::now_ms();
    // Checked even while paused, so a schedule that never comes round is never stored.
    let next = next_after(&schedule, now)?;
    let before = match id {
        Some(id) => Some(db.automation(id)?.ok_or("This automation was deleted.")?),
        None => None,
    };
    let automation = Automation {
        id: before.as_ref().map_or_else(db::new_id, |b| b.id.clone()),
        name,
        cli: draft.cli,
        cwd,
        mode: draft.mode,
        prompt,
        permission_mode: PermMode::parse(&draft.permission_mode).as_str().to_string(),
        schedule,
        enabled: draft.enabled,
        next_run_at: draft.enabled.then_some(next),
        created_at: before.as_ref().map_or(now, |b| b.created_at),
        updated_at: now,
    };
    db.upsert_automation(&automation)?;
    Ok(automation)
}

/// Pausing clears the next run; resuming counts it from now, so a time that passed while paused
/// does not run.
pub fn set_enabled(db: &Db, id: &str, enabled: bool) -> Res<Automation> {
    let mut a = db.automation(id)?.ok_or("This automation was deleted.")?;
    a.enabled = enabled;
    a.next_run_at = if enabled { Some(next_after(&a.schedule, db::now_ms())?) } else { None };
    a.updated_at = db::now_ms();
    db.upsert_automation(&a)?;
    Ok(a)
}

pub fn delete(db: &Db, id: &str) -> Res<()> {
    db.automation(id)?.ok_or("This automation was deleted.")?;
    db.delete_automation(id)
}

pub fn list(db: &Db) -> Res<Vec<View>> {
    db.automations()?
        .into_iter()
        .map(|automation| {
            let last_run = db.last_run(&automation.id)?;
            Ok(View { automation, last_run })
        })
        .collect()
}

pub fn detail(db: &Db, id: &str) -> Res<Detail> {
    let automation = db.automation(id)?.ok_or("This automation was deleted.")?;
    let runs = db.runs(id, RUNS_KEPT)?;
    Ok(Detail { automation, runs })
}

/// Starts the automation's session for the time `due_at`, unless the session it started last
/// still runs a CLI. The run is stored either way.
fn launch(db: &Db, a: &Automation, due_at: i64, now: i64, start: &mut dyn FnMut(StartRequest) -> Res<SessionInfo>) -> Res<AutomationRun> {
    let mut run = AutomationRun {
        id: db::new_id(),
        automation_id: a.id.clone(),
        due_at,
        ran_at: now,
        outcome: "started".into(),
        session_id: None,
        error: None,
    };
    let busy = match db.last_session(&a.id)? {
        Some(id) => db.session(&id)?.is_some_and(|s| s.status.runs_cli()),
        None => false,
    };
    if busy {
        run.outcome = "skipped".into();
    } else {
        let started = start(StartRequest {
            cli: a.cli,
            cwd: a.cwd.clone(),
            mode: a.mode,
            prompt: a.prompt.clone(),
            title: Some(format!("{} · {}", a.name, stamp(due_at))),
            permission_mode: Some(a.permission_mode.clone()),
            source: Some("automation".into()),
            cols: None,
            rows: None,
        });
        match started {
            Ok(s) => {
                run.session_id = Some(s.id);
                if now - due_at > LATE_AFTER_MS {
                    run.outcome = "late".into();
                }
            }
            Err(e) => {
                run.outcome = "failed".into();
                run.error = Some(e);
            }
        }
    }
    db.add_run(&run)?;
    Ok(run)
}

/// One scheduler pass: every enabled automation whose time came runs once, and its next run is
/// counted from `now`, so a schedule missed many times runs only once.
pub fn run_due(db: &Db, now: i64, start: &mut dyn FnMut(StartRequest) -> Res<SessionInfo>) -> Vec<(Automation, AutomationRun)> {
    let Ok(due) = db.due_automations(now) else { return Vec::new() };
    let mut done = Vec::new();
    for a in due {
        let Some(due_at) = a.next_run_at else { continue };
        // A schedule that stopped coming round waits, paused in effect, until it is saved again.
        let _ = db.set_next_run(&a.id, next_after(&a.schedule, now).ok());
        if let Ok(run) = launch(db, &a, due_at, now, start) {
            done.push((a, run));
        }
    }
    done
}

fn tell_failure(manager: &Manager, a: &Automation, run: &AutomationRun) {
    let Some(error) = run.error.as_deref() else { return };
    let Ok(settings) = manager.db().settings() else { return };
    if !settings.notify_error {
        return;
    }
    let title = if settings.language == "id" {
        format!("{} tidak dimulai", a.name)
    } else {
        format!("{} did not start", a.name)
    };
    manager.notify(&title, error);
}

/// The scheduler's pass with real sessions.
pub fn tick(manager: &Arc<Manager>, now: i64) {
    let db = Arc::clone(manager.db());
    let done = run_due(&db, now, &mut |req| manager.start(req));
    for (a, run) in &done {
        tell_failure(manager, a, run);
    }
    if !done.is_empty() {
        manager.automations_changed();
    }
}

/// Run now: starts the session at once and leaves the schedule as it is.
pub fn run_now(manager: &Arc<Manager>, id: &str) -> Res<AutomationRun> {
    let db = Arc::clone(manager.db());
    let a = db.automation(id)?.ok_or("This automation was deleted.")?;
    let now = db::now_ms();
    let run = launch(&db, &a, now, now, &mut |req| manager.start(req))?;
    manager.automations_changed();
    Ok(run)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Status;
    use chrono::NaiveDate;

    fn at(y: i32, m: u32, d: u32, h: u32, min: u32) -> i64 {
        let naive = NaiveDate::from_ymd_opt(y, m, d).unwrap().and_hms_opt(h, min, 0).unwrap();
        Local.from_local_datetime(&naive).earliest().unwrap().timestamp_millis()
    }

    fn folder() -> String {
        std::env::temp_dir().to_string_lossy().into_owned()
    }

    fn draft(schedule: &str) -> Draft {
        Draft {
            name: "Morning review".into(),
            cli: CliKind::Claude,
            cwd: folder(),
            mode: Mode::Headless,
            prompt: "review the open pull requests".into(),
            permission_mode: "bypass".into(),
            schedule: schedule.into(),
            enabled: true,
        }
    }

    fn ok(_: CliKind) -> Res<()> {
        Ok(())
    }

    /// Stands in for `Manager::start`: stores the session with `status` and records the request.
    fn starter<'a>(db: &'a Db, status: Status, asked: &'a mut Vec<StartRequest>) -> impl FnMut(StartRequest) -> Res<SessionInfo> + 'a {
        move |req: StartRequest| {
            let s = SessionInfo {
                id: db::new_id(),
                cli: req.cli,
                cwd: req.cwd.clone(),
                mode: req.mode,
                title: req.title.clone().unwrap_or_default(),
                prompt: req.prompt.clone(),
                status,
                pid: None,
                cli_session_id: None,
                started_at: 0,
                ended_at: None,
                exit_code: None,
                last_event: None,
                waiting: None,
                source: req.source.clone().unwrap_or_default(),
                permission_mode: req.permission_mode.clone(),
                updated_at: 0,
            };
            db.upsert_session(&s)?;
            asked.push(req);
            Ok(s)
        }
    }

    #[test]
    fn next_run_is_the_next_cron_time_after_now() {
        assert_eq!(next_after("0 9 * * *", at(2026, 9, 30, 8, 0)).unwrap(), at(2026, 9, 30, 9, 0));
        assert_eq!(next_after("0 9 * * *", at(2026, 9, 30, 9, 0)).unwrap(), at(2026, 10, 1, 9, 0));
        // 30 September 2026 is a Wednesday: the weekday schedule skips to Monday after Friday.
        assert_eq!(next_after("0 9 * * 1-5", at(2026, 10, 2, 10, 0)).unwrap(), at(2026, 10, 5, 9, 0));
        assert_eq!(next_after("*/15 * * * *", at(2026, 9, 30, 8, 7)).unwrap(), at(2026, 9, 30, 8, 15));
        assert_eq!(
            preview("0 */6 * * *", at(2026, 9, 30, 7, 0)).unwrap(),
            [at(2026, 9, 30, 12, 0), at(2026, 9, 30, 18, 0), at(2026, 10, 1, 0, 0)]
        );
    }

    #[test]
    fn schedules_need_five_fields_and_a_time_that_comes() {
        assert_eq!(next_after("", 0).unwrap_err(), "Choose when it runs.");
        assert_eq!(
            next_after("0 0 9 * * *", 0).unwrap_err(),
            "Use five fields: minute, hour, day of month, month and day of week."
        );
        assert_eq!(next_after("0 25 * * *", 0).unwrap_err(), NOT_VALID);
        assert_eq!(next_after("abc d e f g", 0).unwrap_err(), NOT_VALID);
        assert!(next_after("0 9 * * MON-FRI", 0).is_ok());
        assert_eq!(next_after("0 0 30 2 *", at(2026, 9, 30, 8, 0)).unwrap_err(), "This schedule never comes round.");
    }

    #[test]
    fn saving_checks_the_fields_and_pausing_clears_the_next_run() {
        let db = Db::open_in_memory().unwrap();
        let refuse = |mut d: Draft, change: fn(&mut Draft)| {
            change(&mut d);
            save(&db, None, d, ok).unwrap_err()
        };
        assert_eq!(refuse(draft("0 9 * * *"), |d| d.name = "  ".into()), "Give the automation a name.");
        assert_eq!(refuse(draft("0 9 * * *"), |d| d.cwd = "Z:/no/such/folder".into()), "This folder does not exist.");
        assert_eq!(refuse(draft("0 9 * * *"), |d| d.prompt = " ".into()), "Headless sessions need a prompt.");
        assert_eq!(refuse(draft("0 9 * * *"), |d| d.cli = CliKind::Gemini), "Headless mode for Gemini CLI is not supported yet.");
        assert_eq!(refuse(draft("0 0 30 2 *"), |d| d.enabled = false), "This schedule never comes round.");
        assert_eq!(
            save(&db, None, draft("0 9 * * *"), |_| Err("Codex CLI is not installed or not on PATH.".into())).unwrap_err(),
            "Codex CLI is not installed or not on PATH."
        );
        assert!(db.automations().unwrap().is_empty());

        let mut spaced = draft("0  9 * *   1-5");
        spaced.name = "  Morning review ".into();
        let a = save(&db, None, spaced, ok).unwrap();
        assert_eq!((a.name.as_str(), a.schedule.as_str(), a.permission_mode.as_str()), ("Morning review", "0 9 * * 1-5", "bypass"));
        assert!(a.next_run_at.unwrap() > db::now_ms());

        let paused = set_enabled(&db, &a.id, false).unwrap();
        assert_eq!(paused.next_run_at, None);
        // Resuming never runs a time that passed while paused.
        let resumed = set_enabled(&db, &a.id, true).unwrap();
        assert!(resumed.next_run_at.unwrap() > db::now_ms());

        let mut edit = draft("30 7 * * *");
        edit.enabled = false;
        let changed = save(&db, Some(&a.id), edit, ok).unwrap();
        assert_eq!((changed.id.as_str(), changed.created_at, changed.next_run_at), (a.id.as_str(), a.created_at, None));
        assert_eq!(db.automations().unwrap().len(), 1);

        delete(&db, &a.id).unwrap();
        assert_eq!(delete(&db, &a.id).unwrap_err(), "This automation was deleted.");
        assert_eq!(save(&db, Some(&a.id), draft("0 9 * * *"), ok).unwrap_err(), "This automation was deleted.");
    }

    #[test]
    fn a_missed_schedule_runs_once_and_is_late() {
        let db = Db::open_in_memory().unwrap();
        let mut a = save(&db, None, draft("0 9 * * *"), ok).unwrap();
        let now = at(2026, 9, 30, 12, 0);
        // The app was closed from Sunday: three 09:00 runs were missed.
        a.next_run_at = Some(at(2026, 9, 27, 9, 0));
        db.upsert_automation(&a).unwrap();

        let mut asked = Vec::new();
        let done = run_due(&db, now, &mut starter(&db, Status::Done, &mut asked));
        assert_eq!(done.len(), 1);
        assert_eq!(asked.len(), 1);
        assert_eq!(done[0].1.outcome, "late");
        assert_eq!(db.automation(&a.id).unwrap().unwrap().next_run_at, Some(at(2026, 10, 1, 9, 0)));
        assert!(run_due(&db, now + 30_000, &mut starter(&db, Status::Done, &mut asked)).is_empty());
        assert_eq!(asked.len(), 1);
    }

    #[test]
    fn a_run_on_time_is_started_with_the_automation_title_and_source() {
        let db = Db::open_in_memory().unwrap();
        let mut a = save(&db, None, draft("0 9 * * *"), ok).unwrap();
        let due = at(2026, 9, 30, 9, 0);
        a.next_run_at = Some(due);
        db.upsert_automation(&a).unwrap();

        let mut asked = Vec::new();
        let done = run_due(&db, due + 20_000, &mut starter(&db, Status::Running, &mut asked));
        let run = &done[0].1;
        assert_eq!(run.outcome, "started");
        assert_eq!(run.due_at, due);
        let req = &asked[0];
        assert_eq!(req.title.as_deref(), Some("Morning review · 30 Sep 09:00"));
        assert_eq!(req.source.as_deref(), Some("automation"));
        assert_eq!(req.permission_mode.as_deref(), Some("bypass"));
        assert_eq!((req.mode, req.prompt.as_str()), (Mode::Headless, "review the open pull requests"));
        let stored = db.session(run.session_id.as_deref().unwrap()).unwrap().unwrap();
        assert_eq!(stored.title, "Morning review · 30 Sep 09:00");
        assert_eq!(db.runs(&a.id, 10).unwrap(), vec![run.clone()]);
    }

    #[test]
    fn a_run_is_skipped_while_the_last_session_runs_a_cli() {
        let db = Db::open_in_memory().unwrap();
        let mut a = save(&db, None, draft("0 * * * *"), ok).unwrap();
        let mut asked = Vec::new();
        a.next_run_at = Some(at(2026, 9, 30, 9, 0));
        db.upsert_automation(&a).unwrap();
        run_due(&db, at(2026, 9, 30, 9, 0), &mut starter(&db, Status::Waiting, &mut asked));

        a.next_run_at = Some(at(2026, 9, 30, 10, 0));
        db.upsert_automation(&a).unwrap();
        let done = run_due(&db, at(2026, 9, 30, 10, 0), &mut starter(&db, Status::Running, &mut asked));
        assert_eq!(done[0].1.outcome, "skipped");
        assert_eq!(done[0].1.session_id, None);
        assert_eq!(asked.len(), 1);

        // A terminal left at its shell prompt runs no CLI, so the next one starts.
        let last = db.last_session(&a.id).unwrap().unwrap();
        let mut s = db.session(&last).unwrap().unwrap();
        s.status = Status::Shell;
        db.upsert_session(&s).unwrap();
        a.next_run_at = Some(at(2026, 9, 30, 11, 0));
        db.upsert_automation(&a).unwrap();
        let done = run_due(&db, at(2026, 9, 30, 11, 0), &mut starter(&db, Status::Running, &mut asked));
        assert_eq!(done[0].1.outcome, "started");
        assert_eq!(asked.len(), 2);
    }

    #[test]
    fn a_start_error_is_recorded_and_the_automation_stays_on() {
        let db = Db::open_in_memory().unwrap();
        let mut a = save(&db, None, draft("0 9 * * *"), ok).unwrap();
        a.next_run_at = Some(at(2026, 9, 30, 9, 0));
        db.upsert_automation(&a).unwrap();
        let done = run_due(&db, at(2026, 9, 30, 9, 0), &mut |_| Err("Claude Code is not installed or not on PATH.".into()));
        let run = &done[0].1;
        assert_eq!((run.outcome.as_str(), run.error.as_deref()), ("failed", Some("Claude Code is not installed or not on PATH.")));
        let after = db.automation(&a.id).unwrap().unwrap();
        assert!(after.enabled);
        assert_eq!(after.next_run_at, Some(at(2026, 10, 1, 9, 0)));
        assert_eq!(list(&db).unwrap()[0].last_run.as_ref(), Some(run));
    }
}
