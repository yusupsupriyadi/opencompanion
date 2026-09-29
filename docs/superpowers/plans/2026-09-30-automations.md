# Automations Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Scheduled automations that start a CLI session in a folder at cron times, managed from a new Automations screen in the desktop sidebar and from the phone.

**Architecture:** Two SQLite tables in `db.rs`, the rules in a new `automations.rs` (cron through the `croner` crate in local time, validation, the 30 s scheduler tick with an injectable starter so it is testable without spawning CLIs), Tauri commands and companion endpoints that both call `automations.rs`, and an `automations-changed` event (Tauri) / `automations` message (phone socket). The desktop gets `/automations` (list, detail, form) and a nav item; the phone gets a third tab with a list and an edit screen. The session fields of both New session forms are extracted into shared components so the automation forms reuse them.

**Tech Stack:** Rust (Tauri 2, rusqlite, axum, croner 4, chrono 0.4), Svelte 5 + SvelteKit static, Vitest + Testing Library.

**Spec:** `docs/superpowers/specs/2026-09-30-automations-design.md`

## Global Constraints

- Schedules are 5-field cron in the computer's local time zone; the form's simple modes are written as cron.
- Every-N options: minutes 5, 10, 15, 20, 30; hours 1, 2, 3, 4, 6, 8, 12.
- Scheduler checks every 30 s; a start more than 2 minutes after `due_at` is `late`.
- A missed schedule runs once; the next run is counted from now.
- At most 50 runs per automation.
- Session `source` is `"automation"`; title `{name} · {d Mon HH:MM}`.
- Every permission mode, Bypass included, with the New session Bypass warning.
- UI copy in English and Indonesian, no em dash, no emoji, buttons name their action.
- Commits: Conventional Commits, no AI attribution lines; stage only this task's files (other agents share the tree).
- Run Vitest and cargo test from `C:\Users\yusup\Project` (capital P) or path-case failures appear.

## Review Focus

1. A schedule that never comes round (`0 0 30 2 *`): saving must refuse it with a message, not store an automation that never runs. Test in Task 2.
2. The app was closed for three days over a daily schedule: exactly one Late run on the first tick, not three. Test in Task 2.
3. Pausing and resuming an automation whose time passed while paused: resuming must not run it at once. Test in Task 2.
4. A run's session was deleted by retention: the run stays in Recent runs without an Open session link. Test in Task 1 (db) and Task 4 (UI).
5. Some days with no day picked: the form must say so instead of saving an empty schedule. Test in Task 3.

---

### Task 1: Tables and queries (`db.rs`)

**Files:**
- Modify: `src-tauri/src/db.rs` (types after `Device`, SCHEMA, an "Automations" section in `impl Db`, tests)

**Interfaces:**
- Produces:
  - `pub struct Automation { id, name, cli: CliKind, cwd, mode: Mode, prompt, permission_mode: String, schedule: String, enabled: bool, next_run_at: Option<i64>, created_at: i64, updated_at: i64 }` (serde camelCase)
  - `pub struct AutomationRun { id, automation_id, due_at: i64, ran_at: i64, outcome: String, session_id: Option<String>, error: Option<String> }` (serde camelCase)
  - `Db::upsert_automation(&Automation)`, `automation(&str) -> Option<Automation>`, `automations() -> Vec<Automation>` (oldest first), `due_automations(now) -> Vec<Automation>`, `set_next_run(&str, Option<i64>)`, `delete_automation(&str)`, `add_run(&AutomationRun)` (trims to `RUNS_KEPT` = 50), `runs(&str, u32) -> Vec<AutomationRun>` (newest first, `session_id` null when the session row is gone), `last_run(&str) -> Option<AutomationRun>`, `last_session(&str) -> Option<String>`

- [ ] **Step 1: Failing tests** in `db.rs` tests: `automations_keep_their_runs_and_forget_deleted_sessions` (upsert, list, due only when enabled and due, 55 runs trimmed to 50 newest, a run whose session row is missing reads `session_id: None`, `last_session` returns the newest run's session, delete removes runs).
- [ ] **Step 2:** `cargo test automations_keep` fails to compile (types missing).
- [ ] **Step 3:** Add SCHEMA:

```sql
CREATE TABLE IF NOT EXISTS automations (
  id TEXT PRIMARY KEY, name TEXT NOT NULL, cli TEXT NOT NULL, cwd TEXT NOT NULL, mode TEXT NOT NULL,
  prompt TEXT NOT NULL, permission_mode TEXT NOT NULL, schedule TEXT NOT NULL, enabled INTEGER NOT NULL,
  next_run_at INTEGER, created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS automation_runs (
  id TEXT PRIMARY KEY, automation_id TEXT NOT NULL, due_at INTEGER NOT NULL, ran_at INTEGER NOT NULL,
  outcome TEXT NOT NULL, session_id TEXT, error TEXT
);
CREATE INDEX IF NOT EXISTS automation_runs_by_automation ON automation_runs(automation_id, ran_at);
```

  Runs query: `SELECT r.id, r.automation_id, r.due_at, r.ran_at, r.outcome, CASE WHEN s.id IS NULL THEN NULL ELSE r.session_id END AS session_id, r.error FROM automation_runs r LEFT JOIN sessions s ON s.id = r.session_id WHERE r.automation_id = ?1 ORDER BY r.ran_at DESC, r.rowid DESC LIMIT ?2`. Trim: `DELETE FROM automation_runs WHERE automation_id = ?1 AND id NOT IN (SELECT id FROM automation_runs WHERE automation_id = ?1 ORDER BY ran_at DESC, rowid DESC LIMIT 50)`.
- [ ] **Step 4:** Tests pass.

### Task 2: Rules and scheduler (`automations.rs`), commands, events, endpoints

**Files:**
- Create: `src-tauri/src/automations.rs`
- Modify: `src-tauri/Cargo.toml` (`croner = "4"`, `chrono = "0.4"`), `src-tauri/src/lib.rs` (mod, `Emit::automations_changed`, commands, scheduler thread), `src-tauri/src/session.rs` (`Emit::automations_changed` default, `Manager::automations_changed`, `Manager::notify`, `resolve_exe` → `pub(crate)`), `src-tauri/src/companion.rs` (routes)

**Interfaces:**
- Consumes: Task 1 types and `Db` methods; `Manager::start(StartRequest)`.
- Produces:
  - `pub fn next_after(schedule: &str, after_ms: i64) -> Result<i64, String>`; `pub fn preview(schedule: &str, from_ms: i64) -> Result<Vec<i64>, String>` (three times)
  - `#[serde(rename_all = "camelCase")] pub struct Draft { name, cli: CliKind, cwd, mode: Mode, prompt, permission_mode: String, schedule, enabled: bool }`
  - `pub fn save(db, id: Option<&str>, draft: Draft, installed: impl Fn(CliKind) -> Result<(), String>) -> Result<Automation, String>`
  - `pub fn set_enabled(db, id, enabled) -> Result<Automation, String>`; `pub fn delete(db, id) -> Result<(), String>`
  - `pub struct View { #[serde(flatten)] automation, last_run: Option<AutomationRun> }`; `pub fn list(db) -> Result<Vec<View>, String>`
  - `pub struct Detail { automation, runs: Vec<AutomationRun> }`; `pub fn detail(db, id) -> Result<Detail, String>` (Err "This automation was deleted.")
  - `pub fn run_due(db, now, start: &mut dyn FnMut(StartRequest) -> Result<SessionInfo, String>) -> Vec<(Automation, AutomationRun)>`; `pub fn tick(manager, now)`; `pub fn run_now(manager, id) -> Result<AutomationRun, String>`
  - Tauri commands: `list_automations`, `get_automation`, `save_automation(id?, draft)`, `set_automation_enabled(id, enabled)`, `delete_automation(id)`, `run_automation(id)`, `preview_schedule(schedule)`
  - Event `automations-changed`; phone message `{ "type": "automations" }`
  - Endpoints: `GET/POST /api/automations`, `POST /api/automations/preview`, `GET/PUT/DELETE /api/automations/{id}`, `POST /api/automations/{id}/run`, `POST /api/automations/{id}/enabled`

- [ ] **Step 1: Failing tests** (`automations.rs` `#[cfg(test)]`, a fake starter that stores a `SessionInfo` with a chosen status):
  - `next_run_is_the_next_cron_time_after_now` (daily 09:00 from 08:00 → same day 09:00, from 09:00 → next day)
  - `schedules_need_five_fields_and_a_time_that_comes` (six fields, garbage, `0 0 30 2 *` refused with messages)
  - `a_missed_schedule_runs_once_and_is_late` (next_run_at three days ago, one tick → one `late` run, next_run_at > now)
  - `a_run_on_time_is_started_with_the_automation_title_and_source`
  - `a_run_is_skipped_while_the_last_session_runs_a_cli` (no start call)
  - `a_start_error_is_recorded_and_the_automation_stays_on`
  - `saving_checks_the_fields_and_pausing_clears_the_next_run` (empty name, missing folder, headless without prompt, Gemini headless, uninstalled CLI; paused → `next_run_at: None`; resume after the time passed → next_run_at in the future)
- [ ] **Step 2:** `cargo test automations::` fails to compile.
- [ ] **Step 3:** Implement. Parsing: trim, `split_whitespace().count() == 5` else "Use five fields: minute, hour, day of month, month and day of week.", `Cron::from_str` error → "This schedule is not valid: {e}", no next time → "This schedule never comes round.". Launch: `last_session` still `runs_cli()` → `skipped`; else `start(StartRequest { title: Some(format!("{} · {}", name, stamp(due))), source: Some("automation".into()), permission_mode: Some(perm), cols: None, rows: None, .. })`; outcome `late` if `now - due > 120_000` else `started`; error → `failed` + notification. `run_due` sets `next_run_at = next_after(schedule, now).ok()` for each due automation. The thread in `lib.rs` sleeps 30 s, then ticks, forever.
- [ ] **Step 4:** Tests pass; `cargo build` passes.
- [ ] **Step 5:** Commit backend (Tasks 1–2) together: `feat(automations): schedule sessions in the backend`.

### Task 3: Schedule logic, API and shared session fields (frontend)

**Files:**
- Create: `src/lib/schedule.ts`, `src/lib/schedule.test.ts`, `src/lib/ScheduleField.svelte`, `src/lib/SessionFields.svelte`, `src/lib/PhoneSessionFields.svelte`, `src/lib/i18n/automations.ts`
- Modify: `src/lib/api.ts`, `src/lib/i18n.svelte.ts`, `src/lib/i18n/backend.ts`, `src/lib/SessionForm.svelte`, `src/routes/m/new/+page.svelte`, `src/lib/phone.svelte.ts` (no change needed: `receive` forwards every type)

**Interfaces:**
- Produces:
  - `type Simple = { kind: "daily"; time: string } | { kind: "days"; days: number[]; time: string } | { kind: "every"; n: number; unit: "minutes" | "hours" }`
  - `toCron(s: Simple): string`, `fromCron(expr: string): Simple | null`, `describe(expr: string): string`, `MINUTE_STEPS`, `HOUR_STEPS`
  - `ScheduleField` props: `value = $bindable("")`, `idPrefix`, `preview: (expr: string) => Promise<number[]>`
  - `SessionFields` props (all `$bindable`): `cli, cwd, mode, prompt, permissionMode, folderError, promptError`, plus `idPrefix`, `promptEl`; module export `checkSession(v, idPrefix): Promise<boolean>`-style helper returning `{ folderError?, promptError?, failure? }`
  - `PhoneSessionFields` props: `options: PhoneOptions`, bindable `cli, folder, typed, mode, prompt, permissionMode, folderError, promptError`, `idPrefix`
  - api: `Automation`, `AutomationRun`, `AutomationView`, `AutomationDetail`, `AutomationDraft`, `api.listAutomations()`, `api.getAutomation(id)`, `api.saveAutomation(id | null, draft)`, `api.setAutomationEnabled(id, enabled)`, `api.deleteAutomation(id)`, `api.runAutomation(id)`, `api.previewSchedule(schedule)`

- [ ] **Step 1: Failing tests** `schedule.test.ts`: daily/days/every round trips (`30 9 * * *`, `0 9 * * 1-5`, `0 9 * * 1,3`, `*/15 * * * *`, `0 */2 * * *`, `0 * * * *`), all seven days → daily, unknown shapes (`0 9 1 * *`) → null, `describe` words ("Every day at 09:30", "Weekdays at 09:00", "Mon, Wed at 09:00", "Every 15 minutes", "Every 2 hours", "Every hour", "Cron: 0 9 1 * *").
- [ ] **Step 2:** Tests fail (module missing). Implement `schedule.ts`; tests pass.
- [ ] **Step 3:** Extract `SessionFields` from `SessionForm` and `PhoneSessionFields` from `m/new`; run `SessionForm.test.ts`, `NewSessionDialog.test.ts`, `m/new/new.test.ts`: must stay green unchanged.
- [ ] **Step 4:** `ScheduleField`: radios Every day | Some days | Every | Cron, time input, seven day toggles (`aria-pressed`), every-N select + unit select, cron input mono; "Pick at least one day." when Some days has none (value ""), preview "Next runs: …" debounced 250 ms, preview error shown as the field error.

### Task 4: Desktop screen and sidebar

**Files:**
- Create: `src/routes/automations/+page.svelte`, `src/lib/AutomationForm.svelte`, `src/lib/AutomationRow.svelte`, `src/routes/automations/automations.test.ts`
- Modify: `src/lib/Sidebar.svelte`, `src/lib/Sidebar.test.ts`, `src/lib/i18n/shell.ts`

- [ ] **Step 1: Failing tests**: nav hrefs `["/", "/chat", "/automations", "/settings"]`; list empty state + New automation link; a row shows name, "Weekdays at 09:00", "Next run", last outcome chip, Active switch calls `set_automation_enabled`; list error + Try again; new form: Create without name says "Give the automation a name.", Some days with none picked says "Pick at least one day.", a filled form calls `save_automation` with `schedule: "0 9 * * *"` and goes to `?id=`; detail lists runs, a run without session has no Open session link, Run now calls `run_automation`, Delete asks then calls `delete_automation`.
- [ ] **Step 2:** Implement; tests pass; `bun run check` clean.

### Task 5: Phone tab, list and edit screen

**Files:**
- Create: `src/routes/m/automations/+page.svelte`, `src/routes/m/automations/edit/+page.svelte`, `src/routes/m/automations/automations.test.ts`
- Modify: `src/routes/m/+layout.svelte`, `src/lib/i18n/phone.ts`

- [ ] **Step 1: Failing tests**: list renders cards and the switch posts `/api/automations/{id}/enabled`; empty state; edit `?new` posts the draft; edit `?id=` puts it; Delete opens the sheet and deletes; a message `{type: "automations"}` reloads the list.
- [ ] **Step 2:** Implement; tests pass.

### Task 6: Verify, document, commit

- [ ] `cargo test` (src-tauri), `bun run test`, `bun run check`, `cargo clippy` if configured; README Features line; DESIGN.md screens D14/M8/M9 and the nav line; `.development-history/2026-09-30-HH-mm-automations.md`; commit per logical scope; push.
