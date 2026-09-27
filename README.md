# OpenCompanion

A desktop app that starts, watches and answers AI coding CLIs on your computer, with a phone companion on your own network.

<!-- Replace the line below with a screenshot or a short GIF, for example docs/media/overview.png. -->
> Screenshot placeholder: the Overview with one session waiting for you.

## What it is

When you run Claude Code, Codex CLI or OpenCode in several folders at once, each session stops now and then to ask for permission, and you only find out when you look at its terminal. OpenCompanion starts those sessions for you, lists them on one screen, tells you when one is waiting, and lets you answer from the desktop or from your phone.

It is a Tauri 2 + Svelte 5 app that runs on your computer. There is no account, no cloud server and no telemetry. Each CLI keeps talking to its own provider with your own login, as it does in your terminal.

The UI is available in English and Indonesian (Settings › Language). The current version is 0.1.0, and Windows 11 is the tested platform. There are no prebuilt releases yet, so you [build it from source](#build-from-source).

## Features

- Start Claude Code, Codex CLI or OpenCode in a project folder, in a real terminal (ConPTY on Windows) or headless, where the CLI's JSON output becomes a list of events. Stop a session, resume it, or send it a follow-up message.
- See every session on one line in the Overview: status, latest event and an activity track. All sessions adds search and filters, and each session shows its CPU and memory use.
- Sessions you opened in your own terminal show up too, read-only, with a transcript read from the CLI's own history.
- Know when a session is waiting for you. Claude Code permission prompts get Approve and Deny buttons. A CLI's opening dialogs, such as a folder trust question or an update offer, show as Waiting for you and are never answered for you.
- Get OS notifications, with choices per CLI and per project folder. On Windows, clicking one opens its session.
- Describe work in Chat, in plain words. A planner turns it into one card per session; you read each prompt, then press Run. The planner is a CLI running headless with read-only access to your project folders, or any OpenAI-compatible endpoint such as Ollama, LM Studio or OpenRouter. Type `@` to name a folder, and pick the planner's model and thinking level.
- Line up work on the Board: Pending, Todo, In progress and Done. A card starts its CLI when you press Run and follows the session to Done. In folders you choose, cards may start without Run, but never in Bypass mode.
- Compare the skill folders of each CLI on the Skills screen: which skill is missing from a CLI's folder or has different content there, with a command to copy and run yourself.
- Keep sessions running from the tray after the window closes, and start in the tray when you sign in to Windows.
- Answer from your phone on the same network: watch sessions, Approve or Deny, start new sessions, send messages, plan in Chat and work the Board. The phone page can go on the home screen.
- Pick a [permission mode](#permission-modes) per session: Ask me, Plan, Auto or Bypass.
- Settings also covers text size, retention of finished sessions, custom CLI paths and the UI language.

## Supported CLIs

OpenCompanion looks for these on your PATH and in common install folders, and never installs one for you. The CLIs screen shows each install command for you to copy and run in your own terminal.

| | Claude Code | Codex CLI | OpenCode | Gemini CLI |
|---|---|---|---|---|
| Detected, with version | Yes | Yes | Yes | Yes |
| Version lines checked | 2.1 | 0.153 | 1.18 | None |
| Interactive session | Yes | Yes | Yes | Starts, never tested |
| Headless session | Yes | Yes (see note) | Yes (see note) | No |
| Approve/Deny in OpenCompanion and on the phone | Yes | No, answer in its terminal | No, answer in its terminal | No |
| Resume | Yes | Yes | Yes | No |
| Chat planner | Yes | Yes | Yes | No |
| Opened outside OpenCompanion | Listed, with transcript | Listed, with transcript | Listed, with transcript | Listed, no transcript |

Notes:

- Claude Code headless prompts are answered through its stdio control protocol. Interactive prompts come in through hooks that OpenCompanion passes to that one session with `--settings`, with a screen-text check as a fallback.
- Codex CLI's headless success events are parsed from its documentation. On the development machine Codex could not authenticate, so only its failure path was observed.
- OpenCode's headless `opencode run` rejects permission prompts by itself, so in Ask me mode a tool call that needs permission fails. Interactive OpenCode sessions ask in their terminal as usual.
- Gemini CLI was not installed on the test machine. It is detected and listed, and an interactive start is wired up, but that path has never been run against the real CLI.

## Requirements

- Windows 11. macOS and Linux build from the same code but are untested.
- Rust (stable), with the MSVC toolchain on Windows.
- The Tauri 2 system prerequisites for your platform: on Windows, Microsoft C++ Build Tools and WebView2 (WebView2 ships with Windows 11). See [tauri.app/start/prerequisites](https://tauri.app/start/prerequisites/).
- [Bun](https://bun.sh).
- At least one supported CLI, installed and signed in. OpenCompanion runs it with your own account and quota.

## Build from source

```sh
git clone https://github.com/yusupsupriyadi/opencompanion.git
cd opencompanion
bun install
bun run tauri dev      # desktop app with hot reload
bun run tauri build    # installer in src-tauri/target/release/bundle
```

On first start, Onboarding lists the CLIs it found. It installs and changes nothing.

## Phone companion

Phone access is off until you turn it on. Then OpenCompanion serves the phone page over HTTP and WebSocket on your local network, on port 8765 unless you pick another one in Settings.

1. On the desktop, open Settings › Phone access and press Turn on phone access. If Windows Firewall asks, allow OpenCompanion on private networks.
2. On the phone, join the same Wi-Fi and scan the QR code with the camera, or open the address shown and type the 6-digit pairing code. A code works once, expires after 2 minutes and allows 5 wrong tries.
3. Give the phone a name. It then shows your sessions.

Settings › Phone access lists the paired devices. Remove one and it has to pair again. Turning phone access off disconnects every phone.

Good to know:

- The connection is plain HTTP. On a network you do not trust, or away from home, reach your computer through a VPN with HTTPS, such as Tailscale.
- The page has a web app manifest and Apple tags, so you can add it to the home screen. On the plain-HTTP LAN, Chrome shows no install prompt and no service worker runs, so the installed app opens only while the desktop answers. Over HTTPS or on localhost, the service worker keeps the app shell and the offline screen.
- On iPhone, the Home Screen app keeps its own storage, apart from Safari, so pair it once more by typing the code.
- The phone shows notifications only while its page is open. Web Push is not built yet.

## Privacy and data

- OpenCompanion makes network connections of its own in two cases only: the phone companion, on your local network and only while phone access is on, and a custom Chat planner endpoint, if you set one.
- The CLIs you run talk to their own providers with your own login and quota, as in your terminal. A Chat planner that runs a CLI headless uses that CLI's quota.
- The Chat planner receives your message with its context: recent Chat turns, the installed CLIs, your project folder paths and the current sessions. A CLI planner passes that to the CLI's provider; a custom endpoint receives it directly.
- Data lives in the app data folder. Settings shows the exact path.
  - Windows: `%APPDATA%\dev.opencompanion.app`
  - macOS (untested): `~/Library/Application Support/dev.opencompanion.app`
  - Linux (untested): `~/.local/share/dev.opencompanion.app`
- That folder holds `opencompanion.db` (SQLite: sessions, events, Chat, Board, settings and paired devices), terminal logs under `sessions/`, the hook files for Claude Code sessions under `hooks/`, and the planner's working folder `planner/`.
- Paired phones are stored as SHA-256 hashes of their device tokens, not the tokens themselves.
- The API key of a custom Chat planner endpoint is saved with the settings in `opencompanion.db`, unencrypted.
- OpenCompanion reads the CLIs' own files and never writes to them: session history for transcripts (OpenCode's database is opened read-only) and skill folders. Claude Code hooks are passed per session with `--settings`, so your own Claude Code settings stay as they are.
- Finished sessions and their logs can be deleted after a number of days you choose in Settings. 0 keeps them.

## Permission modes

Each session starts in one of four modes. OpenCompanion turns the mode into each CLI's own flags:

| Mode | What it means | Claude Code | Codex CLI | OpenCode |
|---|---|---|---|---|
| Ask me | Every permission prompt comes to you as Waiting for you | `--permission-mode manual` | `-s workspace-write` (plus `-a on-request` in a terminal) | The CLI's defaults |
| Plan | Read and plan only, no changes | `--permission-mode plan` | `-s read-only` (plus `-a on-request` in a terminal) | `--agent plan` |
| Auto | The CLI approves routine actions itself | `--permission-mode auto` | `--approve-for-me` | `--auto` |
| Bypass | No permission checks at all | `--dangerously-skip-permissions` | `--dangerously-bypass-approvals-and-sandbox` | `--auto` with `OPENCODE_PERMISSION={"*":"allow"}` |

The flags were checked against each CLI's `--help` on 2026-09-25. A resumed headless Codex run keeps the sandbox it started with. Gemini CLI gets no mode flags. Board cards that start without Run never use Bypass.

## Development

```sh
bun run check                          # svelte-check
bun run test                           # component tests (Vitest, mocked backend)
cd src-tauri && cargo test             # unit tests + session manager and companion integration tests
cd src-tauri && cargo clippy --all-targets
```

The integration tests drive a stand-in CLI (`src-tauri/src/bin/fake-cli.rs`) that prints the event formats captured from the real CLIs, so they need no CLI and no quota. `src-tauri/src/bin/air-spike.rs` runs the real CLIs from a terminal for spikes.

See [CONTRIBUTING.md](CONTRIBUTING.md) for setup, code style and pull requests.

## Project layout

| Path | What it holds |
|---|---|
| `src-tauri/src/session.rs` | Session manager: PTY and headless runs, status, waiting detection, Approve/Deny, stop, resume, Board card moves, notifications |
| `src-tauri/src/pty.rs`, `headless.rs` | ConPTY sessions and headless runners with per-CLI arguments and permission mode flags |
| `src-tauri/src/events.rs`, `waiting.rs` | Event parsers per CLI, and the screen-text fallback for prompts |
| `src-tauri/src/orchestrator.rs` | Chat planner: runs a CLI headless with read-only access to project folders, or calls an OpenAI-compatible endpoint, and validates dispatch cards |
| `src-tauri/src/models.rs` | Planner models and thinking levels per CLI (Claude Code's own model catalog cache, `codex debug models`, `opencode models --verbose`) and the flags that pass them |
| `src-tauri/src/companion.rs` | Phone companion: HTTP + WebSocket on the LAN, pairing codes, hashed device tokens |
| `src-tauri/src/actions.rs` | Chat, dispatch card and Board work shared by the desktop commands and the phone API |
| `src-tauri/src/monitor.rs`, `cli.rs`, `db.rs`, `projects.rs` | Outside-session scan and CPU/memory per session, CLI detection, SQLite store, project folder discovery |
| `src-tauri/src/transcript.rs` | Transcripts of sessions opened outside OpenCompanion, read from each CLI's own history (Claude Code and Codex JSONL, OpenCode's SQLite database opened read-only) |
| `src-tauri/src/skills.rs` | Skills screen: reads the user skill folders of each CLI (`~/.claude/skills`, `~/.codex/skills`, `~/.agents/skills`, `~/.config/opencode/skills`, `~/.gemini/skills`) and compares them by content hash. Read-only |
| `src-tauri/src/autostart.rs`, `proc.rs` | Start at Windows sign-in, and process launch and tree kill |
| `src-tauri/src/bin/` | `fake-cli` for the integration tests, `air-spike` for spikes against the real CLIs |
| `src-tauri/tests/` | Integration tests for the session manager and the phone companion |
| `src/routes` | Desktop screens (Overview, Session, Chat, Board, CLIs, Skills, Settings, Onboarding) and the phone app under `/m` |
| `src/lib` | Shared components, the store, and the English and Indonesian strings in `src/lib/i18n` |
| `src/service-worker.ts`, `static/m/` | The phone page's service worker, web app manifest and icons |
| `landing/` | The scroll-driven landing page: one `index.html` that opens by double-click, plus the meadow painting split into sky, clouds and ground |
| `docs/` | Product requirements, the M0 spike results and feature specs |

## Status

Built: the P0 requirements of PRD sections A to G, most P1 ones (install commands, custom CLI paths, resume, waiting detection, Approve/Deny, planner context, activity track, device list, offline screen, permission modes Ask me / Plan / Auto / Bypass, text size, planner model and thinking level, `@` folder mentions in Chat, CPU and memory per session, transcripts of sessions opened outside OpenCompanion, retention of finished sessions, notification choices per CLI and per project folder, Chat follow-ups sent to a running session, the tray icon that keeps sessions running after the window closes, starting in the tray at Windows sign-in, All sessions with search and filters, cards that start without Run in folders the owner picks, never with Bypass, an English or Indonesian UI), and the Board (section H, including Add to board from Chat). Outside the PRD: the Skills screen, which shows which skill is missing from a CLI's folder or has different content there, with a copy command to run yourself, and the installable phone page.

### Known limits

- Approve/Deny works for Claude Code (headless through its stdio control protocol, interactive through hooks). Codex and OpenCode prompts are answered in their own terminal; OpenCode's headless `run` refuses prompts by itself.
- Codex success-path events are parsed from its documentation; on the development machine Codex could not authenticate, so only its failure path was observed.
- On Windows, clicking a notification while it is on screen opens its session (FR-40); one clicked later from the notification center only brings OpenCompanion forward. On macOS and Linux a click does not open the session yet: the Tauri notification plugin has no click action there.
- Not built yet: Web Push to the phone (FR-42). It needs HTTPS and a push service; on the plain-HTTP LAN the phone shows notifications while its page is open.
- The phone page can be added to the home screen (web app manifest, Apple tags). On the plain-HTTP LAN, Chrome shows no install prompt and no service worker runs, so the installed app opens only while the desktop answers; over HTTPS or on localhost the service worker keeps the app shell and the offline screen. On iPhone the Home Screen app keeps its own storage, apart from Safari, so it is paired once more by typing the code.
- Windows 11 is the tested platform. macOS and Linux build from the same code but are untested.

### Roadmap

- Web Push to the phone (FR-42), which needs HTTPS and a push service.
- Testing on macOS and Linux, including notification clicks there.
- A successful Codex CLI run observed end to end, and Approve/Deny for Codex CLI and OpenCode. The M0 spike names the likely routes: `codex app-server` and `opencode serve`.
- Gemini CLI support once it can be tested, then more CLIs such as Aider and Qwen Code.
- A native mobile app (FR-58), after the web companion.

## Documentation

- [`docs/PRD.md`](docs/PRD.md): product requirements. The FR numbers above refer to it.
- [`DESIGN.md`](DESIGN.md): design direction, tokens and screens.
- [`docs/spike/M0-results.md`](docs/spike/M0-results.md): what was measured against the real CLIs.
- [`CHANGELOG.md`](CHANGELOG.md): what changed in each version.

The PRD, DESIGN.md and the M0 results are written in Indonesian.

## Contributing

Bug reports, fixes and ideas are welcome. Read [CONTRIBUTING.md](CONTRIBUTING.md) before you open a pull request, and follow the [Code of Conduct](CODE_OF_CONDUCT.md).

## Security

Please do not report security problems in public issues. [SECURITY.md](SECURITY.md) explains how to report one privately and what the phone companion does and does not protect against.

## License

[MIT](LICENSE) © 2026 Yusup Supriyadi.

Claude Code, Codex CLI, OpenCode and Gemini CLI are products of their respective owners. OpenCompanion is an independent project and is not affiliated with or endorsed by them.
