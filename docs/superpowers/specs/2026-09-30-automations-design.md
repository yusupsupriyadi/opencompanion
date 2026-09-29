# Automations: design

Approved in chat on 2026-09-30. The owner asked for scheduled automations with their own place in the sidebar, and chose every option below.

## Goal

The owner saves a CLI, a folder, a prompt and a schedule once, and OpenCompanion starts that session by itself at the chosen times, while it is open or in the tray. The owner can make, change, pause, run and delete automations from the desktop and from the phone.

## Decisions

- An automation starts a **new session**: CLI, project folder, mode (terminal or headless), prompt and permission mode, the same fields as New session. It never sends input to an existing session and never runs a plain shell command.
- Every permission mode is allowed, Bypass included, with the same Bypass warning New session shows.
- Schedules are stored as a **5-field cron expression** in the computer's local time zone. The form offers four modes: Every day at a time, Some days (weekday toggles) at a time, Every N minutes (5, 10, 15, 20, 30) or hours (1, 2, 3, 4, 6, 8, 12), and Cron. The first three are written as cron; a stored expression that matches one of them opens in that mode, anything else opens in Cron.
- The scheduler is a thread in the backend. It checks every 30 s while the app runs, the tray included. Nothing runs while OpenCompanion is closed.
- A run that was missed (app closed, computer asleep) runs **once** when the app runs again, marked **Late**, however many times it was missed. The next run is counted from now.
- When the session from the previous run of the same automation still runs a CLI, the new run is **Skipped**.
- The phone can manage automations fully: list, create, edit, pause, run now, delete.

## Data (`db.rs`)

- `automations`: `id`, `name`, `cli`, `cwd`, `mode`, `prompt`, `permission_mode`, `schedule` (cron), `enabled`, `next_run_at` (ms, null while paused), `created_at`, `updated_at`.
- `automation_runs`: `id`, `automation_id`, `due_at`, `ran_at`, `outcome` (`started`, `late`, `skipped`, `failed`), `session_id`, `error`. At most 50 rows per automation; older rows are removed. Deleting an automation removes its runs, not its sessions.

## Scheduler (`automations.rs`)

- Every 30 s: every enabled automation with `next_run_at <= now` runs.
- A run: if the automation's last started session still runs a CLI, record `skipped`. Otherwise start the session through `Manager::start` with `source: "automation"` and title `{name} · {30 Sep 09:00}`. The outcome is `late` when the start is more than 2 minutes after `due_at`, otherwise `started`. A start error records `failed` with its message and shows an OS notification ("{name} did not start: {error}"). A failed run does not pause the automation.
- After each run, `next_run_at` becomes the next cron time after now.
- **Run now** starts the session the same way (outcome `started`, `due_at` = now) and leaves the schedule as it is.
- Saving recomputes `next_run_at` from now. Pausing sets it to null; resuming computes it from now, so resuming never runs a missed schedule.
- Validation on save: a name, a folder that exists, an installed CLI, a prompt for headless mode, Gemini CLI not in headless mode, a valid cron expression. The same messages as New session where the field is the same.
- Every change (save, delete, pause, a run) emits `automations` to the webview and to the phone socket.

## Desktop

- Sidebar nav: Overview, Chat, **Automations** (`calendar-check`), Settings.
- `/automations`: H1 "Automations", sub "Starts a session in a folder at the times you choose, while OpenCompanion is open or in the tray.", primary "New automation". When Start at sign-in is off, one line says "Start at sign-in is off, so automations wait until you open OpenCompanion." with a link to Settings.
- Rows in the SessionRow style: CliMark, name, meta "{CLI} · {folder} · {schedule in words}", "Next run: {when}" or "Paused", the last outcome chip, and an Active switch. Clicking a row opens its detail. Right-click (Shift+F10, Menu key) opens Run now, Edit, Pause or Resume, and Delete in red at the bottom.
- `/automations?id=…` and `/automations?new`: the form on the left (Name, the New session fields, Schedule with its four modes and a "Next runs" preview of three times computed by the backend), a 300 px "Recent runs" panel on the right (due time, run time, outcome chip, error, Open session while that session exists). The header holds the name, the Active switch, Run now and Delete (with confirmation).
- States: "Reading your automations…", an error with Try again, and the empty state "No automations yet. Make one to start a session at a set time, for example a review every weekday at 09:00." with New automation. A detail with no runs says "It has not run yet. The first run is {when}." An unknown id says the automation is gone, with a link back.

## Phone

- A third tab, Automations (`calendar-check`), after Sessions and Chat.
- `/m/automations`: H1, `plus` "New automation", compact cards (CliMark 24, name, "{schedule} · {folder}", next run or Paused, last outcome chip, Active switch with a 44 px target). The same states as desktop.
- `/m/automations/edit?id=…` or `?new`: back to automations, a one-column form like New session on the phone (text 16 px), the Next runs preview, Run now and Delete above a Recent runs list, and "Save automation" in the bottom action bar. Delete asks in a bottom sheet.
- Companion endpoints, behind the paired-device token: `GET/POST /api/automations`, `GET/PUT/DELETE /api/automations/{id}`, `POST /api/automations/{id}/run`, `POST /api/automations/preview`.

## Testing

- Rust: next-run computation, a missed schedule runs once and is Late, Skipped while the previous session runs, runs trimmed to 50, validation, cron parsing errors.
- Vitest: the Automations nav item, the list states, the form validation and the simple-form ↔ cron conversion, the phone list and form.
