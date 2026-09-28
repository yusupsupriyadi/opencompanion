<p align="center">
  <img src="design/logo.svg" width="112" height="112" alt="OpenCompanion logo: a pixel hiker on a green meadow hill">
</p>

<h1 align="center">OpenCompanion</h1>

<p align="center">A desktop app that starts, watches and answers AI coding CLIs on your computer, with a phone companion on your own network.</p>

<!-- Replace the line below with a screenshot or a short GIF, for example docs/media/overview.png. -->
> Screenshot placeholder: the Overview with one session waiting for you.

## What it is

When you run Claude Code, Codex CLI or OpenCode in several folders at once, each session stops now and then to ask for permission, and you only find out when you look at its terminal. OpenCompanion starts those sessions for you, lists them on one screen, tells you when one is waiting, and lets you answer from the desktop or from your phone.

It is a Tauri 2 + Svelte 5 app that runs on your computer. There is no account, no cloud server and no telemetry. Each CLI keeps talking to its own provider with your own login, as it does in your terminal.

The UI is available in English and Indonesian (Settings › Language). The current version is 0.1.1. It is made for Windows, Linux and macOS; [Platforms](#platforms) says how far each one was tested (macOS so far only in CI). There are no prebuilt releases yet, so you [build it from source](#build-from-source).

## Features

- Start Claude Code, Codex CLI or OpenCode in a project folder, in a real terminal (ConPTY on Windows) or headless, where the CLI's JSON output becomes a list of events. A terminal session runs in a shell (PowerShell on Windows, bash on Linux and macOS), like a terminal of your own: when the CLI exits, typing its name starts it again with the session's flags and hooks, `exit` closes the terminal, and Open terminal brings a closed one back with the CLI's own history. Stop a headless session or send it a follow-up message.
- Type into a session's terminal as you would in your own. Shift+Enter adds a line, and Ctrl+V pastes text or an image; an image is saved and pasted as its file path, which Claude Code and Codex CLI attach as the image.
- See every session on one line in the Overview: status, latest event and an activity track. The sidebar lists every session under its project folder, with pinned folders and sessions first. All sessions adds search and filters, and each session shows its CPU and memory use.
- Beside a session's terminal, browse its folder (ignored files in italics, a search by name or by text), read a file, see the uncommitted changes with their diff, and see or switch the branch. It is all read-only except the switch, which waits until the session has stopped and the tracked files are committed. Desktop only.
- Sessions you opened in your own terminal show up too, read-only, with a transcript read from the CLI's own history.
- Know when a session is waiting for you. Claude Code permission prompts get Approve and Deny buttons. A CLI's opening dialogs, such as a folder trust question or an update offer, show as Waiting for you and are never answered for you.
- Get OS notifications, with choices per CLI and per project folder. On Windows, clicking one opens its session.
- Describe work in Chat, in plain words. A planner turns it into one card per session; you read each prompt, then press Run. The planner is a CLI running headless with read-only access to your project folders, or any OpenAI-compatible endpoint such as Ollama, LM Studio or OpenRouter. Type `@` to name a folder, and pick the planner's model and thinking level. In folders you choose, cards may start without Run, but never in Bypass mode.
- Run the project itself from its session: New terminal (the `+` after the session's tabs) opens a plain shell such as PowerShell, Command Prompt, Git Bash or bash in the session's folder, one tab each, for a dev server, tests or git. Shells keep no history, end when you close their tab or delete the session, and are reachable from the desktop only, never from the phone.
- Compare the skill folders of each CLI in Settings › Skills: which skill is missing from a CLI's folder or has different content there, with a command to copy and run yourself.
- Keep sessions running from the tray after the window closes, and start in the tray when you sign in to Windows.
- Answer from your phone on the same network: watch sessions, Approve or Deny, start new sessions, mark them done, send messages and plan in Chat. The phone page can go on the home screen.
- Pick a [permission mode](#permission-modes) per session: Ask me, Plan, Auto or Bypass.
- Settings also covers the Day or Dusk theme, text size, retention of finished sessions, custom CLI paths and the UI language.

## Supported CLIs

OpenCompanion looks for these on your PATH and in common install folders, and never installs one for you. Settings › CLIs shows each install command for you to copy and run in your own terminal.

| | Claude Code | Codex CLI | OpenCode | Gemini CLI | CCS | Pi | omp |
|---|---|---|---|---|---|---|---|
| Detected, with version | Yes | Yes | Yes | Yes | Yes | Yes | Yes |
| Version lines checked | 2.1 | 0.153 | 1.18 | None | 8.10 | 0.87 | 18.3 |
| Interactive session | Yes | Yes | Yes | Starts, never tested | Yes (see note) | Yes (see note) | Yes (see note) |
| Headless session | Yes | Yes (see note) | Yes (see note) | No | Default and account profiles | Yes (see note) | Yes (see note) |
| Approve/Deny in OpenCompanion and on the phone | Yes | No, answer in its terminal | No, answer in its terminal | No | Yes | No, Pi never asks | No, answer in its terminal |
| Resume | Yes | Yes | Yes | No | Default and account profiles | Yes | Yes |
| Chat planner | Yes | Yes | Yes | No | Default and account profiles | Yes | Yes |
| Opened outside OpenCompanion | Listed, with transcript | Listed, with transcript | Listed, with transcript | Listed, no transcript | Listed, with transcript | Listed, with transcript | Listed, with transcript |

Notes:

- Claude Code headless prompts are answered through its stdio control protocol. Interactive prompts come in through hooks that OpenCompanion passes to that one session with `--settings`, with a screen-text check as a fallback.
- Codex CLI's headless success events are parsed from its documentation. On the development machine Codex could not authenticate, so only its failure path was observed.
- OpenCode's headless `opencode run` rejects permission prompts by itself, so in Ask me mode a tool call that needs permission fails. Interactive OpenCode sessions ask in their terminal as usual.
- Gemini CLI was not installed on the test machine. It is detected and listed, and an interactive start is wired up, but that path has never been run against the real CLI.
- CCS starts Claude Code with a profile (`ccs [profile] [claude args]`), so OpenCompanion gives it Claude Code's flags and reads its output, hooks and transcripts the same way (transcripts from `~/.claude` and each account in `~/.ccs/instances`). Its extra arguments in Settings › CLIs go right after `ccs`, so the profile comes first, for example `work --effort high`. API and CLIProxy profiles pass their own `--settings`, and API profiles send `-p` through CCS's delegation, so OpenCompanion runs those profiles in a terminal only, without hooks (the screen-text check still works). The argument order is covered by unit tests; a full session through CCS has not been run from OpenCompanion yet.
- Pi (`@earendil-works/pi-coding-agent`) has no permission prompts: it runs its tools without asking, in every mode except Plan, which gives it only `read`, `grep`, `find` and `ls`. Headless sessions use `pi --mode json` with the prompt on stdin, and follow-ups continue with `--session`. Transcripts and the session id for Resume come from `~/.pi/agent/sessions`. Pi's own installer puts it in `~/.pi/agent/bin`, which OpenCompanion also searches. The development machine has no provider account for Pi, so its headless runs and the Chat planner were checked against a local OpenAI-compatible stub; interactive sessions and Resume are covered by unit tests of their arguments only.
- omp (`@oh-my-pi/pi-coding-agent`) is a Pi fork that runs on Bun, so OpenCompanion reads it with the Pi adapter. Each mode sets its `--approval-mode`. In a terminal, omp asks before a tool that the mode does not allow, and OpenCompanion does not detect that prompt yet. A headless run has nobody to ask, so the tool fails and the refusal appears in the output. Plan gives it only `read`, `grep` and `glob`: omp has no `ls`, and its `find` is off unless a judge model is set up. Transcripts and the session id for Resume come from `~/.omp/agent/sessions`, which names a folder under your home folder by its relative path (`-project-app`). The models come from `omp models --json`. omp 18.3.5 was checked only with runs that sent no prompt: every mode's flags, the planner's flags, the session header and the model list. No session that sends a prompt has been run from OpenCompanion yet.
- On Windows, an interactive session starts PowerShell 7 (or Windows PowerShell when PowerShell 7 is missing), which loads your profile and then starts the CLI. The session ends when the CLI exits. Headless sessions start the CLI directly.

## Platforms

| | Windows 11 | Linux | macOS |
|---|---|---|---|
| How it was tested | By hand, every day | End to end in Docker on Ubuntu 24.04: the installed `.deb`, driven through WebDriver ([e2e/linux](CONTRIBUTING.md#linux-end-to-end-in-docker)) | Built and tested in CircleCI (macOS 26, Apple M4 Pro): svelte-check, Vitest, clippy and every Rust test pass. Not run by hand yet |
| Packages from `bun run tauri build` | `.msi`, `.exe` | `.deb`, `.rpm`, AppImage | `.app`, `.dmg` |
| Window controls | OpenCompanion's own | OpenCompanion's own | Native traffic lights |
| Paste in a terminal | Ctrl+V | Ctrl+Shift+V (Ctrl+V goes to the CLI) | ⌘V |
| A click on a notification | Opens the session | Opens the session when the notification server supports actions (tried with dunst; GNOME and KDE support actions but were not tried) | Does not open the session |
| Start in the tray at sign-in | Run key in the registry | `~/.config/autostart/opencompanion.desktop` | `~/Library/LaunchAgents/dev.opencompanion.app.plist` |
| Tray icon | Taskbar corner | Needs AppIndicator support; GNOME shows it only with the AppIndicator extension | Menu bar |

The rest of the macOS column says what the code does: the app compiles and its tests pass on macOS, but nobody has opened the window on a Mac yet.

- On macOS and Linux, OpenCompanion reads your login shell's environment when it starts (`$SHELL -ilc env`, at most 5 seconds). So a CLI installed with Homebrew, npm, nvm, volta or bun is found, `#!/usr/bin/env node` scripts find node, and variables you export in `~/.zshrc` or `~/.bashrc` (API keys, proxies) reach the CLI, even when OpenCompanion starts from Finder or a desktop launcher. If the shell does not answer, it adds the usual install folders to the PATH instead.
- On macOS and Linux, Stop ends the CLI's whole process group, so the dev servers and tools it started stop with it. A child that starts a session of its own (a detached process) is not reached, where Windows would still end it.
- If Linux has no AppIndicator library for the tray icon, OpenCompanion still starts, and closing the window quits it, so the window cannot end up hidden with no way back.
- On Linux, folder rules (auto-run folders, notification rules) compare folder names with case, as the file system does. On Windows and macOS they ignore case.

## Requirements

- Windows 11, a Linux desktop with WebKitGTK 4.1, or macOS (tested in CI only, see [Platforms](#platforms)).
- Rust (stable), with the MSVC toolchain on Windows.
- The Tauri 2 system prerequisites for your platform, see [tauri.app/start/prerequisites](https://tauri.app/start/prerequisites/): on Windows, Microsoft C++ Build Tools and WebView2 (WebView2 ships with Windows 11); on Linux, WebKitGTK 4.1, libayatana-appindicator and the other listed packages, plus `xdg-utils` to bundle an AppImage; on macOS, the Xcode Command Line Tools.
- [Bun](https://bun.sh), and [Node.js](https://nodejs.org) (LTS) to run the tests.
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

1. On the desktop, open Settings › Phone access and press Turn on phone access. If Windows Firewall asks, allow OpenCompanion on private networks; if macOS asks whether to accept incoming connections, allow them.
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
  - macOS: `~/Library/Application Support/dev.opencompanion.app`
  - Linux: `~/.local/share/dev.opencompanion.app`
- That folder holds `opencompanion.db` (SQLite: sessions, events, Chat, settings and paired devices), terminal logs under `sessions/`, the hook files for Claude Code sessions under `hooks/`, and the planner's working folder `planner/`.
- Images you paste into a terminal are saved in the system temp folder, under `opencompanion-paste` (`%TEMP%\opencompanion-paste` on Windows). OpenCompanion does not delete them; they stay until you or the system clear the temp folder.
- The Files, Changes and Branch tabs of a session read its folder and run your own `git` there (status, diff, branch list and log, with `GIT_OPTIONAL_LOCKS=0` so they never lock the index). Only Switch branch changes anything, and file contents never reach the phone.
- Shell tabs are not saved: their output lives in memory while the tab is open and is gone when you close it, delete its session or quit. Retention skips a session while a shell is open in it.
- Paired phones are stored as SHA-256 hashes of their device tokens, not the tokens themselves.
- The API key of a custom Chat planner endpoint is saved with the settings in `opencompanion.db`, unencrypted.
- OpenCompanion reads the CLIs' own files and never writes to them: session history for transcripts (OpenCode's database is opened read-only) and skill folders. Claude Code hooks are passed per session with `--settings`, so your own Claude Code settings stay as they are.
- Settings › History keeps finished sessions Forever by default, or for 90, 30, 7 or 1 day, after which they are deleted with their events, logs and hook files. Delete finished sessions clears them at once.

## Permission modes

Each session starts in one of four modes. OpenCompanion turns the mode into each CLI's own flags:

| Mode | What it means | Claude Code | Codex CLI | OpenCode | Pi | omp |
|---|---|---|---|---|---|---|
| Ask me | Every permission prompt comes to you as Waiting for you | `--permission-mode manual` | `-s workspace-write` (plus `-a on-request` in a terminal) | The CLI's defaults | Runs every tool without asking | `--approval-mode always-ask` |
| Plan | Read and plan only, no changes | `--permission-mode plan` | `-s read-only` (plus `-a on-request` in a terminal) | `--agent plan` | `--tools read,grep,find,ls` | `--tools read,grep,glob --approval-mode always-ask` |
| Auto | The CLI approves routine actions itself | `--permission-mode auto` | `--approve-for-me` | `--auto` | Runs every tool without asking | `--approval-mode write` |
| Bypass | No permission checks at all | `--dangerously-skip-permissions` | `--dangerously-bypass-approvals-and-sandbox` | `--auto` with `OPENCODE_PERMISSION={"*":"allow"}` | Runs every tool without asking | `--approval-mode yolo` |

The flags were checked against each CLI's `--help` on 2026-09-25. A resumed headless Codex run keeps the sandbox it started with. Gemini CLI gets no mode flags. CCS gets Claude Code's flags. Pi was checked against 0.87.1, and omp against 18.3.5 on 2026-09-28. Chat cards that start without Run never use Bypass.

## Development

```sh
bun run check                          # svelte-check
bun run test                           # component tests (Vitest, mocked backend)
cd src-tauri && cargo test             # unit tests + session manager and companion integration tests
cd src-tauri && cargo clippy --all-targets
bash scripts/ci/checks.sh              # all of the above, clippy with -D warnings (Git Bash on Windows)
bash e2e/linux/run.sh all              # the checks and an end-to-end run on Ubuntu 24.04, in Docker
```

The tests need Node.js: Vitest runs on Node, not on Bun. CI runs on CircleCI and only when triggered, one job per OS; [CONTRIBUTING](CONTRIBUTING.md#ci) shows how.

The integration tests drive a stand-in CLI (`src-tauri/src/bin/fake-cli.rs`) that prints the event formats captured from the real CLIs, so they need no CLI and no quota. `src-tauri/src/bin/air-spike.rs` runs the real CLIs from a terminal for spikes.

See [CONTRIBUTING.md](CONTRIBUTING.md) for setup, code style and pull requests.

## Project layout

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
| `src-tauri/src/files.rs`, `git.rs` | A session's folder for its side panel: one folder at a time with ignored entries, search by name or text, a file's text, git status, a file's diff against HEAD, branches, recent commits and the branch switch |
| `src-tauri/src/skills.rs` | Settings › Skills: reads the user skill folders of each CLI (`~/.claude/skills`, `~/.codex/skills`, `~/.agents/skills`, `~/.config/opencode/skills`, `~/.gemini/skills`, `~/.pi/agent/skills`, `~/.omp/agent/skills`) and compares them by content hash. Read-only |
| `src-tauri/src/autostart.rs`, `proc.rs`, `shell_env.rs` | Start at sign-in on each OS, process launch and tree kill, and the login shell's environment for CLIs on macOS and Linux |
| `src-tauri/src/bin/` | `fake-cli` for the integration tests, `air-spike` for spikes against the real CLIs |
| `src-tauri/tests/` | Integration tests for the session manager and the phone companion |
| `src/routes` | Desktop screens (Overview, Session, All sessions, Chat, Settings with its General, CLIs and Skills tabs, Onboarding) and the phone app under `/m` |
| `src/lib` | Shared components, the store, and the English and Indonesian strings in `src/lib/i18n` |
| `src/service-worker.ts`, `static/m/` | The phone page's service worker, web app manifest and icons |
| `landing/` | The scroll-driven landing page: one `index.html` that opens by double-click, plus the meadow painting split into sky, clouds and ground |
| `design/` | The logo sources (`logo.svg`, `logo-full.svg`), the meadow painting and the Pencil design file (`ai-remote.pen`) |
| `docs/` | Product requirements, the M0 spike results and feature specs |

## Status

Built: the P0 requirements of PRD sections A to G, most P1 ones (install commands, custom CLI paths, resume, waiting detection, Approve/Deny, planner context, activity track, device list, offline screen, permission modes Ask me / Plan / Auto / Bypass, text size, planner model and thinking level, `@` folder mentions in Chat, CPU and memory per session, transcripts of sessions opened outside OpenCompanion, retention of finished sessions, notification choices per CLI and per project folder, Chat follow-ups sent to a running session, the tray icon that keeps sessions running after the window closes, starting in the tray at sign-in, All sessions with search and filters, cards that start without Run in folders the owner picks, never with Bypass, an English or Indonesian UI). Section H (the Board) was removed on 2026-09-28. Outside the PRD: Settings › Skills, which shows which skill is missing from a CLI's folder or has different content there, with a copy command to run yourself; the installable phone page; shell tabs in a session's folder; the Files, Changes and Branch tabs beside a session's terminal; Mark done for an idle terminal session; and image paste and Shift+Enter in terminals.

### Known limits

- Approve/Deny works for Claude Code (headless through its stdio control protocol, interactive through hooks). Codex and OpenCode prompts are answered in their own terminal; OpenCode's headless `run` refuses prompts by itself.
- Codex success-path events are parsed from its documentation; on the development machine Codex could not authenticate, so only its failure path was observed.
- On Windows and Linux, clicking a notification while it is on screen opens its session (FR-40); one clicked later from the notification center only brings OpenCompanion forward. On macOS a click does not open the session: macOS notifications go through the Tauri notification plugin, which reports no click there.
- Not built yet: Web Push to the phone (FR-42). It needs HTTPS and a push service; on the plain-HTTP LAN the phone shows notifications while its page is open.
- The phone page can be added to the home screen (web app manifest, Apple tags). On the plain-HTTP LAN, Chrome shows no install prompt and no service worker runs, so the installed app opens only while the desktop answers; over HTTPS or on localhost the service worker keeps the app shell and the offline screen. On iPhone the Home Screen app keeps its own storage, apart from Safari, so it is paired once more by typing the code.
- The desktop sidebar lists every session, but the phone lists the 60 newest.
- OpenCompanion does not delete pasted images from the temp folder. A paste that cannot be saved pastes nothing, without a message.
- Windows 11 is tested by hand, Linux end to end in Docker, and macOS in CircleCI (build and tests, no window). Nobody has run OpenCompanion by hand on a Mac or on a Linux desktop (GNOME, KDE) yet.

### Roadmap

- Web Push to the phone (FR-42), which needs HTTPS and a push service.
- Running OpenCompanion by hand on macOS and on Linux desktops (GNOME, KDE), including the tray, notification clicks and start at sign-in.
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

Claude Code, Codex CLI, OpenCode, Gemini CLI, CCS, Pi and omp are products of their respective owners. OpenCompanion is an independent project and is not affiliated with or endorsed by them.
