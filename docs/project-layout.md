# Project layout

| Path | What it holds |
|---|---|
| `src-tauri/src/session.rs` | Session manager: PTY and headless runs, status, waiting detection, Approve/Deny, stop, resume, notifications |
| `src-tauri/src/pty.rs`, `headless.rs` | ConPTY sessions and headless runners with per-CLI arguments and permission mode flags |
| `src-tauri/src/events.rs`, `waiting.rs` | Event parsers per CLI, and the screen-text fallback for prompts |
| `src-tauri/src/orchestrator.rs` | Chat planner: runs a CLI headless with read-only access to project folders, or calls an OpenAI-compatible endpoint, and validates dispatch cards |
| `src-tauri/src/models.rs` | Planner models and thinking levels per CLI (Claude Code's own model catalog cache, `codex debug models`, `opencode models --verbose`, `pi --list-models`, `omp models --json`) and the flags that pass them |
| `src-tauri/src/companion.rs` | Phone companion: HTTP + WebSocket on the LAN, pairing codes, hashed device tokens |
| `src-tauri/src/actions.rs` | Chat and dispatch card work shared by the desktop commands and the phone API |
| `src-tauri/src/monitor.rs`, `cli.rs`, `db.rs`, `projects.rs` | Outside-session scan and CPU/memory per session, CLI detection, SQLite store, project folder discovery |
| `src-tauri/src/ccs.rs` | CCS profiles: which profile the extra arguments name, and the Claude Code config folders CCS uses |
| `src-tauri/src/pi.rs` | Pi and omp: their JSON events, stderr errors, model lists, planner answer and session files |
| `src-tauri/src/transcript.rs` | Transcripts of sessions opened outside OpenCompanion, read from each CLI's own history (Claude Code, Codex, Pi and omp JSONL, OpenCode's SQLite database opened read-only) |
| `src-tauri/src/terminal.rs`, `paste.rs` | Shell tabs of a session: shell detection and plain shells in the session's folder. Pasted images saved for the terminal to paste as a path |
| `src-tauri/src/files.rs`, `git.rs` | A session's folder for its side panel: one folder at a time with ignored entries, search by name or text, a file's text or its bytes for an image or a PDF, git status, a file's diff against HEAD, branches, recent commits and the branch switch |
| `src-tauri/src/skills.rs` | Settings › Skills: reads the user skill folders of each CLI (`~/.claude/skills`, `~/.codex/skills`, `~/.agents/skills`, `~/.config/opencode/skills`, `~/.gemini/skills`, `~/.pi/agent/skills`, `~/.omp/agent/skills`) and compares them by content hash. Read-only |
| `src-tauri/src/autostart.rs`, `proc.rs`, `shell_env.rs` | Start at sign-in on each OS, process launch and tree kill, and the login shell's environment for CLIs on macOS and Linux |
| `src-tauri/src/bin/` | `fake-cli` for the integration tests, `air-spike` for spikes against the real CLIs |
| `src-tauri/tests/` | Integration tests for the session manager and the phone companion |
| `src/routes` | Desktop screens (Overview, Session, All sessions, Chat, Settings with its General, CLIs and Skills tabs, Onboarding) and the phone app under `/m` |
| `src/lib` | Shared components, the store, and the English and Indonesian strings in `src/lib/i18n` |
| `src/service-worker.ts`, `static/m/` | The phone page's service worker, web app manifest and icons |
| `landing/` | The landing page: one `index.html` that opens by double-click, plus the meadow painting split into sky, clouds and ground. `.github/workflows/pages.yml` publishes it to [yusupsupriyadi.github.io/opencompanion](https://yusupsupriyadi.github.io/opencompanion/) whenever it changes on `main` |
| `design/` | The logo sources (`logo.svg`, `logo-full.svg`), the meadow painting and the Pencil design file (`ai-remote.pen`) |
| `docs/` | Product requirements, the M0 spike results and feature specs |

The integration tests drive a stand-in CLI (`src-tauri/src/bin/fake-cli.rs`) that prints the event formats captured from the real CLIs, so they need no CLI and no quota. `src-tauri/src/bin/air-spike.rs` runs the real CLIs from a terminal for spikes.
