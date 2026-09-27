# Changelog

Notable changes to OpenCompanion are listed here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow [Semantic Versioning](https://semver.org/). While the version is 0.x, any release may change behavior.

## [Unreleased]

Planned as 0.1.0, the first public release. Windows 11 is the tested platform.

### Added

- Desktop app (Tauri 2 + Svelte 5) that finds Claude Code, Codex CLI, OpenCode and Gemini CLI on the PATH and in common install folders, shows their versions, and offers install commands to copy. Custom CLI paths in Settings.
- Interactive (ConPTY) and headless sessions for Claude Code, Codex CLI and OpenCode, with status, waiting detection, stop, resume, follow-up messages, and CPU and memory per session.
- Approve/Deny for Claude Code permission prompts: headless through its stdio control protocol, interactive through hooks passed with `--settings`.
- Permission modes Ask me, Plan, Auto and Bypass, turned into each CLI's own flags.
- Overview and All sessions with search and filters. Sessions opened in your own terminal are listed read-only, with transcripts from each CLI's history.
- Chat planner that turns a request into one dispatch card per session. It runs a CLI headless with read-only access to your project folders, or calls an OpenAI-compatible endpoint. Model and thinking level, `@` folder mentions, follow-ups to a running session, Add to board.
- Board with Pending, Todo, In progress and Done. Cards start a CLI on Run and follow their session to Done; in folders you pick they may start without Run, never with Bypass.
- Skills screen that compares the skill folders of each CLI by content and offers copy commands. Read-only.
- OS notifications with choices per CLI and per project folder. On Windows a click opens the session.
- Tray icon that keeps sessions running after the window closes, and start in the tray at Windows sign-in.
- Phone companion over HTTP and WebSocket on the local network, off by default: one-time pairing codes, SHA-256 hashed device tokens, a device list, and an offline screen. From the phone: sessions, Approve/Deny, new sessions, messages, Chat and the Board. The phone page can be added to the home screen.
- English and Indonesian UI, including backend messages, OS notifications and the tray menu.
- Text size and retention of finished sessions in Settings.
- Scroll-driven landing page in `landing/`.
- README, CONTRIBUTING, SECURITY, CODE_OF_CONDUCT, the MIT LICENSE, and GitHub issue and pull request templates.

### Known limits

- Approve/Deny covers Claude Code only. Codex CLI and OpenCode prompts are answered in their own terminal.
- Codex CLI's successful headless run has not been observed yet; its events are parsed from its documentation.
- Gemini CLI is detected but untested: no headless mode and no Chat planner.
- No Web Push to the phone yet; the phone shows notifications while its page is open.
- macOS and Linux build from the same code but are untested.

[Unreleased]: https://github.com/yusupsupriyadi/opencompanion/commits/main
