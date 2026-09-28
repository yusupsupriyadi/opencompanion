# Changelog

Notable changes to OpenCompanion are listed here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow [Semantic Versioning](https://semver.org/). While the version is 0.x, any release may change behavior.

## [Unreleased]

Planned as 0.1.0, the first public release. Windows 11 is tested by hand, Linux end to end in Docker, and macOS is ready for its first CI run.

### Added

- Desktop app (Tauri 2 + Svelte 5) that finds Claude Code, Codex CLI, OpenCode and Gemini CLI on the PATH and in common install folders, shows their versions, and offers install commands to copy. Custom CLI paths in Settings.
- Interactive (ConPTY) and headless sessions for Claude Code, Codex CLI and OpenCode, with status, waiting detection, stop, resume, follow-up messages, and CPU and memory per session.
- Approve/Deny for Claude Code permission prompts: headless through its stdio control protocol, interactive through hooks passed with `--settings`.
- Permission modes Ask me, Plan, Auto and Bypass, turned into each CLI's own flags.
- Overview and All sessions with search and filters. Sessions opened in your own terminal are listed read-only, with transcripts from each CLI's history.
- Chat planner that turns a request into one dispatch card per session. It runs a CLI headless with read-only access to your project folders, or calls an OpenAI-compatible endpoint. Model and thinking level, `@` folder mentions, follow-ups to a running session, Add to board.
- Board with Pending, Todo, In progress and Done. Cards start a CLI on Run and follow their session to Done; in folders you pick they may start without Run, never with Bypass.
- Skills screen that compares the skill folders of each CLI by content and offers copy commands. Read-only.
- OS notifications with choices per CLI and per project folder. On Windows and Linux a click opens the session.
- Tray icon that keeps sessions running after the window closes, and start in the tray at sign-in.
- Phone companion over HTTP and WebSocket on the local network, off by default: one-time pairing codes, SHA-256 hashed device tokens, a device list, and an offline screen. From the phone: sessions, Approve/Deny, new sessions, messages, Chat and the Board. The phone page can be added to the home screen.
- English and Indonesian UI, including backend messages, OS notifications and the tray menu.
- CCS (`@kaitranntt/ccs`) as a CLI. It runs Claude Code with Claude Code's flags, output, hooks and transcripts. Its extra arguments in Settings go right after `ccs`, so the profile comes first.
- Pi (`@earendil-works/pi-coding-agent`) as a CLI: interactive and headless sessions (`pi --mode json`), follow-ups and Resume, the Chat planner with its models and thinking levels, outside sessions with transcripts, and its skill folder. Plan mode limits Pi to read-only tools.
- omp (`@oh-my-pi/pi-coding-agent`), a Pi fork that runs on Bun, as a CLI through the Pi adapter: interactive and headless sessions (`omp --mode json`), follow-ups and Resume, the Chat planner with `omp models --json`, outside sessions with transcripts from `~/.omp/agent/sessions`, and its skill folder. Each permission mode sets omp's `--approval-mode`; Plan also limits it to `read`, `grep` and `glob`.
- Text size and retention of finished sessions in Settings.
- Linux and macOS support: CLIs started from a GUI launch get the login shell's PATH and environment; Stop ends the CLI's whole process group; start in the tray at sign-in (XDG autostart entry, LaunchAgent); a click on a Linux notification opens its session; native traffic lights on macOS; each OS's paste key, folder example and home-folder shortening; folder rules follow the file system's case rules; Claude Code history is read as a project source on Unix.
- `scripts/ci/checks.sh`, a Linux end-to-end test in Docker (`e2e/linux`), and CircleCI jobs for Linux, Windows and macOS that run only when triggered.
- Scroll-driven landing page in `landing/`.
- README, CONTRIBUTING, SECURITY, CODE_OF_CONDUCT, the MIT LICENSE, and GitHub issue and pull request templates.

### Known limits

- Approve/Deny covers Claude Code only. Codex CLI and OpenCode prompts are answered in their own terminal.
- Codex CLI's successful headless run has not been observed yet; its events are parsed from its documentation.
- Gemini CLI is detected but untested: no headless mode and no Chat planner.
- CCS API and CLIProxy profiles run in a terminal only, without hooks, because they pass their own `--settings` and API profiles route `-p` through CCS's delegation. A full session through CCS has not been run from OpenCompanion yet.
- Pi never asks for permission, so Ask me, Auto and Bypass all let it run every tool; only Plan limits it. Its headless runs and the Chat planner were checked against a local stub, not a real provider, and interactive sessions only by unit tests.
- omp's approval prompts in a terminal are not detected as Waiting for you yet, and a headless omp run refuses any tool that needs approval. omp 18.3.5 was checked only with runs that sent no prompt.
- No Web Push to the phone yet; the phone shows notifications while its page is open.
- macOS has not been built or run yet. On macOS a click on a notification does not open its session.
- On a GNOME desktop the tray icon shows only with the AppIndicator extension.
- On macOS and Linux, a child that detaches into a session of its own outlives Stop.

[Unreleased]: https://github.com/yusupsupriyadi/opencompanion/commits/main
