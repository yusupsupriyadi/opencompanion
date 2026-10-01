# PRD: OpenCompanion

| | |
|---|---|
| Status | Draft v0.1 |
| Date | 2026-09-25 |
| Platform | Desktop: Tauri 2 + Svelte 5. Companion: mobile web (PWA) served by the desktop. Native mobile comes later. |
| Design | `design/ai-remote.pen` (Pencil), visual direction in `DESIGN.md` |

## 1. Summary

OpenCompanion is a local desktop app for managing AI coding CLIs such as Claude Code, Codex CLI and OpenCode. From one window you can:

1. see which CLIs are installed and their versions,
2. run CLI sessions in any project folder,
3. give tasks through a built-in AI chat that drafts a plan and sends each task to the right CLI,
4. monitor every running session, including ones opened outside the app,
5. watch sessions and answer permission prompts from your phone through a web companion on the local network.

Everything runs on your computer. OpenCompanion has no cloud server, no accounts and no telemetry. The AI models are still run by each CLI's provider, because the CLI itself calls its API. One exception you choose yourself: the Chat planner can use a custom provider (FR-29), which OpenCompanion calls directly.

## 2. Problem

- Developers who use several AI CLIs at once have to open many terminal windows and switch between them to know which one finished, which one is stuck and which one is waiting for permission.
- Permission prompts (for example "may I run this command?") often wait a long time because nobody is looking at that terminal.
- Choosing which CLI gets which task, then copying the same context into each CLI, is repetitive manual work.
- Away from the desk, there is no way to see session status without a full remote desktop.

## 3. Goals and non-goals

### Goals

- **G1** One place to see every AI CLI session on this machine and its status.
- **G2** Run and control CLI sessions (start, send input, stop, resume) without opening a separate terminal.
- **G3** Give tasks in plain language through chat, turned into a dispatch plan you approve before it runs.
- **G4** Know within seconds when a session finishes, fails or waits for you, on the desktop and on the phone.
- **G5** Stay local: no session data leaves your machine or local network, except what the CLI itself sends to its provider, or the Chat context sent to a custom provider you set up (FR-29).

### Non-goals (v1)

- Not a CLI replacement. OpenCompanion has no coding agent of its own. Its only direct model API call is the Chat planner when you pick a custom provider (FR-29); coding sessions are always a CLI.
- No cloud sync, accounts or multiple users.
- No control over external sessions (ones opened outside the app). External sessions are watched, not controlled.
- No code editor or full diff viewer. Diffs are shown briefly; detailed work stays in your editor.
- No internet relay for mobile. Access outside the home network goes through your own VPN (for example Tailscale).

## 4. Users and scenarios

**Primary user**: a solo developer or small team running 2 or more AI coding CLIs in parallel on Windows (primary), macOS or Linux.

Scenarios:

- **S1** In the morning you open OpenCompanion and write in chat: "Codex, fix the failing tests in ai-remote; Claude Code, write the API docs in uninote." Chat drafts two dispatches, you approve them, two sessions run.
- **S2** You are at lunch. Your phone buzzes: "Claude Code is waiting for approval in uninote: run `npm install`". You open the web companion, read the command and press Approve.
- **S3** You forgot that yesterday you opened OpenCode in Windows Terminal. Overview shows it under "Opened outside OpenCompanion" with its folder and how long it has been running.
- **S4** Codex fails because it is not signed in. The session is marked Error with the CLI's own message and a suggested next step (`codex login`).

## 5. Product principles

1. **Local first.** All state is stored on the machine. The companion server is off by default.
2. **The CLI does the work.** OpenCompanion arranges, shows and relays. Coding decisions stay with the CLI.
3. **You hold the permissions.** Chat only proposes dispatches. No session starts without your confirmation, unless you turn on auto-dispatch for a project yourself.
4. **Honest about limits.** External sessions are marked read-only, and data that cannot be read is shown as "not available", not guessed.

## 6. Supported CLIs

Each CLI is connected through an **adapter** with the same contract: `detect`, `version`, `start_interactive`, `start_headless`, `parse_event`, `find_external`, `read_transcript`.

The data below was verified on 2026-09-25 from the `--help` of the CLIs installed on the developer's machine. Flags can change in other versions, so an adapter must read the version and mark versions that have not been tested.

| CLI | Verified version | Headless mode | Event format | Resume a session | Local session history |
|---|---|---|---|---|---|
| Claude Code (`claude`) | 2.1.282 | `-p` / `--print` | `--output-format text\|json\|stream-json`, input `--input-format stream-json` | `--resume <id>`, `--continue` | `~/.claude/projects/` |
| Codex CLI (`codex`) | 0.153.4 | `codex exec` | `--json` (JSONL to stdout), `-o <file>` for the last message | `codex exec resume` | `~/.codex/sessions/` |
| OpenCode (`opencode`) | 1.18.30 | `opencode run` | `--format json` | `--session <id>`, `--continue` | `~/.local/share/opencode/` (SQLite) |
| Gemini CLI (`gemini`) | not installed | not verified | not verified | not verified | not verified |

Notes per CLI:

- **Claude Code**: `--permission-mode` (choices `acceptEdits`, `auto`, `bypassPermissions`, `manual`, `dontAsk`, `plan`) and `--allowedTools` decide what may run without asking. Verified in M0: headless with `--permission-prompt-tool stdio` sends permission requests as a `control_request` the host can answer, and interactive sessions fire the `PermissionRequest` and `Notification` hooks (see `docs/spike/M0-results.md`).
- **Codex CLI**: `-C/--cd <DIR>` for the working folder, `-s/--sandbox` for the sandbox policy. Codex also has `mcp-server` and `app-server` (experimental), which could become a richer integration path later. The permission signal was not tested in M0 because provider auth failed on the test machine.
- **OpenCode**: `opencode serve` runs a headless server, and `opencode run --attach <url>` can attach to that server. `--dir` sets the working folder. Verified in M0: `opencode run` rejects permissions set to `ask` by itself, so Approve/Deny has to go through `opencode serve`.
- CLIs next after the MVP: Gemini CLI, Aider, Qwen Code, and other CLIs that have a non-interactive mode.

## 7. Functional requirements

Priority: **P0** required for the MVP, **P1** important after the MVP, **P2** later.

### A. CLI Registry

| ID | Prio | Requirement | Acceptance criteria |
|---|---|---|---|
| FR-01 | P0 | Detect installed CLIs through PATH and common install locations. | When the app opens and when Rescan is clicked, every supported CLI shows with status Installed / Not found, the executable path, and the version from `--version`. |
| FR-02 | P0 | Mark versions the adapter has not tested. | If the version is outside the tested range, the CLI row shows "Untested version" and headless features can still be tried. |
| FR-03 | P1 | Show how to install a missing CLI. | A "Not found" row shows the official install command to copy, without running it automatically. |
| FR-04 | P1 | Custom executable path. | The user can pick the executable file by hand; the version is read again after saving. |

### B. Sessions

| ID | Prio | Requirement | Acceptance criteria |
|---|---|---|---|
| FR-10 | P0 | Start an interactive session in a PTY. | Pick a CLI + project folder → the CLI runs in an embedded terminal (xterm) with correct ANSI colors and resizing. |
| FR-11 | P0 | Start a headless session with a prompt. | Pick a CLI + folder + prompt → the CLI runs in headless mode; events are parsed into a timeline (messages, tool calls, files changed, finished). |
| FR-12 | P0 | Send input to a running session. | Typing in the input bar goes to the PTY; for headless, it is sent as a follow-up turn when the CLI supports it. |
| FR-13 | P0 | Stop a session. | Stop sends a graceful stop signal first, then force-kills after a configurable delay; the status becomes Stopped. |
| FR-14 | P0 | Session status. | Each session has one status: Starting, Running, Waiting for you, Idle, Done, Error, Stopped. Status changes are recorded with a time. |
| FR-15 | P1 | Resume a session. | A Done/Stopped session can be resumed with the CLI's own resume mechanism (see the table in section 6). |
| FR-16 | P1 | Detect "Waiting for you". | Permission prompts or questions from the CLI are detected through the event stream (headless), hooks (when the CLI supports them), or text patterns per adapter (PTY). The method used is shown in the session detail. |
| FR-17 | P1 | Approve / deny from the UI. | While Waiting for you, the session detail shows the request and Approve / Deny buttons that pass the answer to the CLI. |
| FR-18 | P2 | Sessions keep running after the window closes. | Closing the window moves the app to the system tray; sessions keep running until the app really quits. Tray icon: a click opens the window, its menu has "Open OpenCompanion" and "Quit OpenCompanion and stop its sessions". The first time the window goes to the tray, one notification explains it. Settings › "Closing the window" can turn this off, so closing the window quits and stops the sessions. |

### C. Orchestrator Chat

| ID | Prio | Requirement | Acceptance criteria |
|---|---|---|---|
| FR-20 | P0 | Chat uses an installed CLI as its brain. | Settings offers a choice of CLI for chat (default: Claude Code when installed). Chat runs in that CLI's headless mode, with no extra API key. A custom provider can be chosen instead (FR-29). |
| FR-21 | P0 | Chat produces a structured dispatch plan. | For a task request, the chat answer holds one or more dispatch cards: a short task name, CLI, folder, prompt, mode (interactive/headless), and a short reason for the CLI choice. A session started from a Chat card uses that task name as its title; an interactive Claude Code session without a card name uses the task name Claude Code writes in the terminal title itself. |
| FR-22 | P0 | Confirm before dispatch. | A dispatch card has Run, Edit and Discard buttons. No session starts without Run, unless auto-dispatch is on for that project. |
| FR-23 | P0 | Plan validation. | Chat output is validated in Rust (CLI installed, folder exists). If it is not valid, the card shows the reason and the Run button is disabled. If the output is not valid JSON, the answer shows as plain text. |
| FR-24 | P1 | Chat knows the context. | Chat receives the list of installed CLIs, projects used before, and active sessions as context, so it can answer "what is Codex working on?". |
| FR-24a | P1 | Point at a folder with `@`. | Typing `@` in the Chat composer opens a list of known project folders (name + path), narrowing as you type; Up/Down arrows choose, Enter or Tab writes the full path (`@C:\path`, in quotes when it has spaces), Esc closes. The last option, "Browse for a folder…", opens a folder dialog. The planner uses the folder you point at for its card, and the planner can also read that folder (read-only) when read access is on. `name@host` does not open the list. |
| FR-25 | P1 | Follow-up tasks to an existing session. | Chat can propose "send to session X" for a session that is still running, with the same confirmation. The planner sees each session's id and whether it can take a message now (a running terminal, a running headless Claude Code turn, or a finished headless session that has a CLI id), then answers with a follow-up card. That card is named "Follow-up for {session title}", its button is "Send to session", and only its message can be edited. Send types the message and presses Enter in the terminal, or starts a headless follow-up turn; no new session. A session that cannot take a message makes its card state the reason and disables Send. |
| FR-26 | P2 | Auto-dispatch per project. | The user can allow dispatch without confirmation for chosen projects; a clear indicator shows in chat. Settings › Chat planner › "Run cards without asking": a folder is added from the list of known folders or with Browse, with a second press "Start them without asking". A valid new-session card for that folder (and folders inside it) runs right away with the default permission mode, except Bypass, which drops to Ask me; follow-up cards and cards with problems still wait. The Chat composer on the desktop and phone names the folder ("Cards for uninote start without asking; the rest wait for Run."), and a card that ran by itself says "Started by itself: … runs cards without asking." |
| FR-27 | P1 | Chat history per conversation. | Each conversation with the planner is saved as its own chat, named from the first line of its first message. The "Chats" list shows every chat (newest first) and opens its history through `/chat?id=`. "New chat" starts an empty conversation. The planner reads only messages from the open chat. Deleting a chat needs a second press and does not stop sessions started from its cards. Old conversations from before this feature become one chat. |
| FR-28 | P1 | Planner model and thinking level. | The Chat composer has Model and Thinking pickers for the planner CLI, saved per CLI in Settings and applied from the next message. "CLI default" sends no flag. The lists come from the CLI itself and show versioned names: Claude Code from its own `/model` catalog (`~/.claude/cache/model-catalog`, for example "Opus 5.5", "Fable 5.1", older models in a "More models" group, the full ID through `--model` and the level per model through `--effort`; without the catalog, the aliases `fable`, `opus`, `sonnet`, `haiku` as a fallback); Codex `codex debug models` with `-c model_reasoning_effort`; OpenCode `opencode models --verbose` with `--variant` per model, grouped by provider. A level the new model does not support falls back to the default. These pickers do not show while the planner uses a custom provider. |
| FR-29 | P1 | Custom provider for the planner. | Settings, Chat planner has a "Custom provider (OpenAI-compatible API)" choice with Base URL, Model and API key (optional; it may be empty for Ollama or LM Studio). Chat sends `POST {base}/chat/completions` directly from OpenCompanion with the same context as the CLI (FR-24), then validates the answer as in FR-23. The provider has no file access, so it sees only folder names. The API key is stored in the local database and sent only to that Base URL. Empty fields or a URL without http(s) are rejected before saving, and provider errors show in the provider's own words. |

### D. Monitoring

| ID | Prio | Requirement | Acceptance criteria |
|---|---|---|---|
| FR-30 | P0 | Overview of every session from the app. | Overview shows sessions that need you at the top, then running ones, then ones finished today. Each row: CLI, folder, status, running time, last event. |
| FR-31 | P0 | Detect external sessions. | CLI processes running outside the app are detected through the process list (executable name, PID, working folder when readable, start time) and shown under "Opened outside OpenCompanion" with a Read-only label. |
| FR-32 | P1 | External session content from the transcript. | When the CLI writes local session history (section 6), the external session detail shows the latest messages from that file. When it cannot be read, it shows "Transcript not available for this CLI". It uses the newest history for that process's folder written since the process started (Claude Code `~/.claude/projects`, Codex `~/.codex/sessions`, the OpenCode database in `~/.local/share/opencode` opened read-only), read again every few seconds. It holds your messages, the CLI's answers and the names of the tools used; OpenCompanion never writes to that file. |
| FR-33 | P1 | Activity line. | Each session shows a horizon line with a dot for each real event (tool call, file changed, permission prompt, error) within the visible time range. |
| FR-34 | P1 | Resource usage. | The session detail shows CPU and memory of the CLI process (and its child processes). CPU is counted as a share of the whole machine, like Task Manager, measured every 3 seconds while the session runs, along with the number of child processes. |
| FR-35 | P2 | Filter and search. | Filter by CLI, project, status; search titles and prompts. The "All sessions" screen (`/history`, from a link on Overview) loads every session newest first, 50 per page with "Show 50 more"; a search field for titles and prompts (case-insensitive, `%` and `_` read as text), and selects for CLI, folder (folders sessions have used) and status (Running, Waiting for you, Done, Error, Stopped). |

### E. Notifications

| ID | Prio | Requirement | Acceptance criteria |
|---|---|---|---|
| FR-40 | P0 | Desktop notifications. | An OS notification appears when a session is Waiting for you, Done or Error. It names the CLI and folder. A click opens the session detail. |
| FR-41 | P1 | Notification settings. | The user can turn off certain notification kinds per CLI or per project. Settings › Notifications: three general switches, a "By CLI" table (Waiting, Done, Error per installed CLI), and a "By project folder" table (folders picked from known project folders or through Browse, and removable). A notification goes out only when the general switch, its CLI rule, and every folder rule that contains the session's folder (subfolders included) allow it. The phone follows the same rules. |
| FR-42 | P1 | Notifications to the phone. | Paired devices receive the same notifications through the companion (Web Push when available over an HTTPS connection, otherwise shown while the page is open). |

### F. Mobile companion (web first)

| ID | Prio | Requirement | Acceptance criteria |
|---|---|---|---|
| FR-50 | P0 | The companion server can be turned on and off. | Off by default. When on, Settings shows the LAN address and port in use. |
| FR-51 | P0 | Pairing by QR. | The desktop shows a QR with the address + a one-time code that expires. The phone that scans it receives a device token. Pairing fails after the code expires. |
| FR-52 | P0 | Session list on the phone. | The phone shows the same sessions as Overview, updated in real time over WebSocket. |
| FR-53 | P0 | Session detail on the phone. | Shows the status, latest events, and the end of the output (tail). |
| FR-54 | P1 | Approve / Deny and Stop from the phone. | The same buttons as the desktop, with a confirmation for Stop. |
| FR-55 | P1 | Manage devices. | The desktop shows the list of paired devices with when each last connected; each device can be revoked. |
| FR-56 | P1 | Disconnected state. | If the desktop cannot be reached, the phone shows a "Can't reach your desktop" screen with likely causes and a retry button. |
| FR-57 | P2 | Chat from the phone. | Send messages to the orchestrator chat from the phone, with the same dispatch confirmation. |
| FR-58 | P2 | Native mobile app. | Once the web companion is stable, a native version uses the same companion API. |
| FR-59 | P1 | Start sessions and send messages from the phone. | The phone can start a new session (CLI, a project folder the desktop knows or another path, mode, prompt, permission mode) and send messages to a session: follow-ups for headless with the same rules as the desktop, text plus Enter and the Enter, Esc, arrow and Ctrl+C keys for interactive terminals, and Resume for a terminal that has closed. |

### G. History and Settings

| ID | Prio | Requirement | Acceptance criteria |
|---|---|---|---|
| FR-60 | P0 | Session and task history. | Sessions and dispatches are stored in local SQLite: prompt, CLI, folder, final status, time. |
| FR-61 | P0 | Day / Dusk theme. | The theme toggle works fully in both modes; the default follows the OS theme. |
| FR-62 | P1 | Data retention. | The user decides how long session output is kept, and can delete history. Settings › History: "Keep finished sessions for" Forever (default), 90, 30, 7 or 1 day, checked when the app opens and every hour; finished sessions older than that are deleted with their events, terminal logs and hook files. "Delete finished sessions" deletes every finished session with the same files. The Chat cards that ran them stay, without a session link. Files the CLI changed in the project are not touched. |
| FR-63 | P1 | UI language. | The UI can switch between English and Indonesian. Settings › Language picks English or Bahasa Indonesia; every desktop screen, the phone pages (which read the desktop's choice through `/api/hello`), OS notifications and the tray menu follow it without a restart. OpenCompanion's own messages from the backend are translated when shown; text written by a CLI (output, planner answers, CLI error messages) stays as it was. |
| FR-64 | P2 | Start at sign-in. | An option to run OpenCompanion when you sign in to the OS. Windows: Settings › "Window and sign-in" writes or removes the `OpenCompanion` value in `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` through `reg.exe`; the app started at sign-in carries `--hidden` and goes straight to the tray. Its state is read from Windows every time Settings opens. Not on macOS and Linux yet. |
| FR-65 | P1 | Session permission modes: Ask me, Plan, Auto, Bypass. | Settings stores the default mode for new sessions; New session can pick another mode for one session. Each mode is passed as each CLI's official flag, and the table shows in Settings. Bypass is saved only after a second confirmation, and the session form warns when Bypass is chosen. The Chat planner is not affected and stays read-only. |
| FR-66 | P1 | Text size. | Settings offers 90%, 100% (default), 110%, 125%, 150%. The choice applies right away on every desktop screen as webview zoom, so text, buttons and spacing grow together and the layout follows the existing breakpoints. It is saved in Settings and applied again when the app opens. The phone pages are not affected and follow the phone's text size. |

### H. Board (removed)

Removed 2026-09-28: Board was taken out of the product, on the desktop and on the phone. FR-70 to FR-77 no longer apply and their numbers are not reused.

## 8. Main flows

**F1. First launch**: Onboarding → scan CLIs → list of results (installed, version, missing) → pick the CLI for chat → empty Overview inviting you to write a first task.

**F2. A task through chat**: write a request → chat (headless CLI) answers with dispatch cards → validation → you press Run → the session appears on Overview → a notification when it finishes or needs you.

**F3. Approve from the phone**: the CLI asks for permission → status Waiting for you → desktop + phone notification → open the detail on the phone → read the command → Approve → the session goes back to Running.

**F4. External session**: periodic process scan → a CLI the app does not know shows as Read-only → click → the detail shows the folder, running time, and the latest messages from the transcript when there is one.

**F5. A task through Board**: removed 2026-09-28 together with Board (section H).

## 9. Screens (map to the design)

| Code | Screen | Related FRs |
|---|---|---|
| D1 | Onboarding: CLI scan | FR-01, FR-20 |
| D2 | Overview | FR-30, FR-31, FR-33 |
| D3 | Orchestrator Chat | FR-20 to FR-25, FR-27, FR-28, FR-29 |
| D4 | Session detail | FR-10 to FR-17, FR-34 |
| D5 | CLI Manager | FR-01 to FR-04 |
| D6 | New session (modal) | FR-10, FR-11 |
| D7 | Settings: Mobile access | FR-50, FR-51, FR-55 |
| D8 | Empty, loading, error states | every data view |
| D9 | Dusk theme (Overview, Session detail) | FR-61 |
| D10 | Board (removed 2026-09-28) | none |
| M1 | Pair device | FR-51 |
| M2 | Sessions | FR-52 |
| M3 | Session detail | FR-53, FR-54 |
| M4 | Can't reach your desktop | FR-56 |

## 10. Architecture

```
┌────────────────────────── OpenCompanion (one Tauri process) ──────────────────────────┐
│  Webview: Svelte 5 SPA (SvelteKit adapter-static)                                     │
│    Overview · Chat · Session (xterm.js) · CLI Manager · Settings                       │
│        │  invoke / events (Tauri IPC)                                                  │
│  Rust core                                                                             │
│    cli_registry ── adapters (claude, codex, opencode, ...)                             │
│    session_manager ── PTY sessions ── headless runner ── event parser per adapter      │
│    orchestrator ── runs the chat CLI headless, validates dispatch plans                │
│    monitor ── process scanner ── transcript watcher                                    │
│    notifier ── OS notifications + fan-out to the companion                             │
│    store ── SQLite (sessions, events, dispatches, devices)                             │
│    companion_server (off by default) ── HTTP + WebSocket, token per device             │
└──────────────┬──────────────────────────────────────────────┬─────────────────────────┘
               │ spawn / PTY                                   │ LAN (or your own VPN)
        claude · codex · opencode                        Phone browser: web companion (PWA)
```

Technical decisions:

- **Frontend**: Svelte 5 with SvelteKit `adapter-static` in SPA mode (`fallback: 'index.html'`, `ssr = false` in the root layout), following Tauri 2's official guide for SvelteKit. The web companion is a separate build from the same codebase (mobile routes), served by `companion_server`.
- **Official Tauri 2 plugins** planned: `notification`, `store`, `sql` (SQLite), `single-instance`, `window-state`, `dialog`, `opener`, then `autostart` and `updater` after the MVP.
- **Candidate crates** (not tested yet, decided in M0): `portable-pty` for cross-platform PTY (ConPTY on Windows), `sysinfo` for the process scan, `notify` for watching transcript files, `axum` for the companion's HTTP + WebSocket.
- **Core data model**: `CliInstall`, `Project`, `Session` (cli, folder, mode, status, pid, started_at, ended_at), `SessionEvent` (type, summary, payload), `Dispatch` (chat source, plan, decision), `Device` (name, token hash, last_seen). `Task` (Board card) was removed 2026-09-28; old databases keep its table unchanged.

How the orchestrator chat works:

1. Your message + context (installed CLIs, projects, active sessions) is sent to the chat CLI in headless mode with a system instruction that asks for a JSON answer holding a dispatch plan.
2. Rust parses and validates the JSON. Chat never runs a shell command itself; its only side effect is a proposed dispatch.
3. The dispatch cards show. Run calls `session_manager` with the parameters from the validated card.

## 11. Security and privacy

- No telemetry and no network calls from OpenCompanion itself, except the companion server when you turn it on.
- OpenCompanion does not store CLI credentials. Sign-in stays with each CLI.
- Companion server: off by default, accepts only paired devices, tokens stored as hashes, one-time pairing codes with a short lifetime, a limit on pairing attempts.
- HTTP on the LAN is not encrypted. Settings shows this warning and recommends a VPN with HTTPS (for example Tailscale) for use outside a trusted network.
- Dispatches and Approve/Deny from the phone are recorded in history with the device name.
- Session output can contain secrets (env, tokens). Output is stored locally only and follows the retention setting (FR-62).

## 12. Non-functional requirements

The numbers below are design targets, not yet measured.

- Session status in the UI changes within 1 second of the event arriving from the CLI.
- With no active session, the app uses no meaningful CPU (a periodic process scan with a configurable interval).
- The terminal stays responsive with long output (rendered through xterm.js, output history capped and the rest saved to disk).
- Windows 11 is fully supported in the MVP. macOS and Linux are built from the same codebase and tested after the MVP.
- Every control can be used with the keyboard, focus is visible, and text contrast meets WCAG AA in both themes.

## 13. Milestones

| Milestone | Contents | Done when |
|---|---|---|
| M0 Spike | PTY on Windows (ConPTY) with Claude Code, Codex, OpenCode; parse each CLI's headless events; detect external processes; test the "waiting for permission" signal. | All three CLIs can run interactive and headless from a Rust prototype, and the permission detection results are recorded per CLI. |
| M1 Desktop MVP | P0 FRs of sections A, B, C, D, E, G. | Flows F1, F2, F4 run end to end on Windows 11. |
| M2 Web companion | P0 FRs of section F + FR-54, FR-56. | Flow F3 runs from a phone on the same network. |
| M3 After the MVP | The remaining P1 FRs, more CLIs, macOS/Linux, then native mobile (FR-58). | Decided after M2. |

## 14. Risks

| Risk | Impact | Mitigation |
|---|---|---|
| CLI flags or event formats change on update. | Event parsing breaks, status is wrong. | The adapter records the tested versions, falls back to a raw text view, and warns "Untested version". |
| "Waiting for permission" detection in a PTY depends on TUI text patterns. | Permission prompts are missed. | Prefer the event stream and hooks; text patterns are only a fallback, and the method used is shown to the user. |
| The working folder of an external process cannot always be read on Windows. | External sessions show without a folder. | Show "Folder unknown", match against the transcript when possible. |
| PWA and Web Push need HTTPS; a plain LAN is HTTP only. | The PWA cannot be installed and push does not work on the LAN. | The companion MVP runs as a plain web page; HTTPS through a VPN (for example Tailscale) as the PWA path. Decided in M2. |
| Headless chat uses the CLI's subscription quota. | Cost or quota runs out faster. | The chat CLI can be changed; the context sent is limited. |
| Each CLI's terms of use for automation. | Some usage patterns may not be allowed by the provider. | Review each CLI's terms before M1; OpenCompanion only runs official CLIs with the user's own account. |

## 15. Open questions

1. Is Gemini CLI part of the MVP, or are Claude Code, Codex and OpenCode enough?
2. May the phone send free text to a session (not only Approve/Deny/Stop) in M2?
3. Is a "worktree per dispatch" mode needed so two CLIs do not change the same folder at once?
4. The final logo (the name is already OpenCompanion; for now it is a text wordmark).

## 16. Terms

- **Session**: one CLI process running for one folder.
- **Dispatch**: a proposal from chat to start a session or send a task to a session.
- **Headless**: a CLI running without a TUI, taking a prompt and emitting events.
- **External session**: a CLI process that OpenCompanion did not start.
- **Companion**: the web app on the phone that connects to the desktop over the local network.
