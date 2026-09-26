# OpenCompanion

A local desktop app (Tauri 2 + Svelte 5) that starts, watches and answers AI coding CLIs: Claude Code, Codex CLI and OpenCode, with Gemini CLI detected but not yet driven headless. A phone on the same network can watch and answer sessions, start new ones, send them messages, plan in Chat and work the Board. Everything stays on your computer.

Product requirements: [`docs/PRD.md`](docs/PRD.md). Design direction: [`DESIGN.md`](DESIGN.md). What was measured against the real CLIs: [`docs/spike/M0-results.md`](docs/spike/M0-results.md).

## Run it

Needs Rust (MSVC toolchain on Windows), Bun, and at least one supported CLI on your PATH.

```sh
bun install
bun run tauri dev      # desktop app with hot reload
bun run tauri build    # installer in src-tauri/target/release/bundle
```

Data (SQLite, terminal logs, hook files) lives in the app data folder, `%APPDATA%\dev.opencompanion.app` on Windows. Settings shows the exact path.

## Checks

```sh
bun run check                          # svelte-check
bun run test                           # component tests (Vitest, mocked backend)
cd src-tauri && cargo test             # unit tests + session manager and companion integration tests
cd src-tauri && cargo clippy --all-targets
```

The integration tests drive a stand-in CLI (`src-tauri/src/bin/fake-cli.rs`) that prints the event formats captured from the real CLIs, so they need no CLI and no quota. `src-tauri/src/bin/air-spike.rs` runs the real CLIs from a terminal for spikes.

## Layout

| Path | What it holds |
|---|---|
| `src-tauri/src/session.rs` | Session manager: PTY and headless runs, status, waiting detection, Approve/Deny, stop, resume, Board card moves, notifications |
| `src-tauri/src/pty.rs`, `headless.rs` | ConPTY sessions and headless runners with per-CLI arguments |
| `src-tauri/src/events.rs`, `waiting.rs` | Event parsers per CLI, and the screen-text fallback for prompts |
| `src-tauri/src/orchestrator.rs` | Chat planner: runs a CLI headless with read-only access to project folders, validates dispatch cards |
| `src-tauri/src/models.rs` | Planner models and thinking levels per CLI (Claude Code's own model catalog cache, `codex debug models`, `opencode models --verbose`) and the flags that pass them |
| `src-tauri/src/companion.rs` | Phone companion: HTTP + WebSocket on the LAN, pairing codes, hashed device tokens |
| `src-tauri/src/actions.rs` | Chat, dispatch card and Board work shared by the desktop commands and the phone API |
| `src-tauri/src/monitor.rs`, `cli.rs`, `db.rs`, `projects.rs` | Outside-session scan and CPU/memory per session, CLI detection, SQLite store, project folder discovery |
| `src-tauri/src/transcript.rs` | Transcripts of sessions opened outside OpenCompanion, read from each CLI's own history (Claude Code and Codex JSONL, OpenCode's SQLite database opened read-only) |
| `src-tauri/src/skills.rs` | Skills screen: reads the user skill folders of each CLI (`~/.claude/skills`, `~/.codex/skills`, `~/.agents/skills`, `~/.config/opencode/skills`, `~/.gemini/skills`) and compares them by content hash. Read-only |
| `src/routes` | Desktop screens (Overview, Session, Chat, Board, CLIs, Skills, Settings, Onboarding) and the phone app under `/m` |

## Status

Built: the P0 requirements of PRD sections A to G, most P1 ones (install commands, custom CLI paths, resume, waiting detection, Approve/Deny, planner context, activity track, device list, offline screen, permission modes Ask me / Plan / Auto / Bypass, text size, planner model and thinking level, `@` folder mentions in Chat, CPU and memory per session, transcripts of sessions opened outside OpenCompanion, retention of finished sessions, notification choices per CLI and per project folder, Chat follow-ups sent to a running session, the tray icon that keeps sessions running after the window closes), and the Board (section H, including Add to board from Chat). Outside the PRD: the Skills screen, which shows which skill is missing from a CLI's folder or has different content there, with a copy command to run yourself.

Known limits:

- Approve/Deny works for Claude Code (headless through its stdio control protocol, interactive through hooks). Codex and OpenCode prompts are answered in their own terminal; OpenCode's headless `run` refuses prompts by itself.
- Codex success-path events are parsed from its documentation; on the development machine Codex could not authenticate, so only its failure path was observed.
- On Windows, clicking a notification while it is on screen opens its session (FR-40); one clicked later from the notification center only brings OpenCompanion forward. On macOS and Linux a click does not open the session yet: the Tauri notification plugin has no click action there.
- Not built yet: UI language switch (FR-63) and Web Push to the phone (FR-42).
- Windows 11 is the tested platform. macOS and Linux build from the same code but are untested.
