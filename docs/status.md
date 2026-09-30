# Status

FR numbers refer to the product requirements in [PRD.md](PRD.md).

Built: the P0 requirements of PRD sections A to G, most P1 ones (install commands, custom CLI paths, resume, waiting detection, Approve/Deny, planner context, activity track, device list, offline screen, permission modes Ask me / Plan / Auto / Bypass, text size, planner model and thinking level, `@` folder mentions in Chat, CPU and memory per session, transcripts of sessions opened outside OpenCompanion, retention of finished sessions, notification choices per CLI and per project folder, Chat follow-ups sent to a running session, the tray icon that keeps sessions running after the window closes, starting in the tray at sign-in, All sessions with search and filters, cards that start without Run in folders the owner picks, never with Bypass, an English or Indonesian UI). Section H (the Board) was removed on 2026-09-28. Outside the PRD: Settings › Skills, which shows which skill is missing from a CLI's folder or has different content there, with a copy command to run yourself; the installable phone page; shell tabs in a session's folder; the Files, Changes and Branch tabs beside a session's terminal; Mark done for an idle terminal session; and image paste and Shift+Enter in terminals.

## Known limits

- Approve/Deny works for Claude Code (headless through its stdio control protocol, interactive through hooks). Codex and OpenCode prompts are answered in their own terminal; OpenCode's headless `run` refuses prompts by itself.
- Codex success-path events are parsed from its documentation; on the development machine Codex could not authenticate, so only its failure path was observed.
- On Windows and Linux, clicking a notification while it is on screen opens its session (FR-40); one clicked later from the notification center only brings OpenCompanion forward. On macOS a click does not open the session: macOS notifications go through the Tauri notification plugin, which reports no click there.
- Not built yet: Web Push to the phone (FR-42). It needs HTTPS and a push service; on the plain-HTTP LAN the phone shows notifications while its page is open.
- The phone page can be added to the home screen (web app manifest, Apple tags). On the plain-HTTP LAN, Chrome shows no install prompt and no service worker runs, so the installed app opens only while the desktop answers; over HTTPS or on localhost the service worker keeps the app shell and the offline screen. On iPhone the Home Screen app keeps its own storage, apart from Safari, so it is paired once more by typing the code.
- The desktop sidebar lists every session, but the phone lists the 60 newest.
- OpenCompanion does not delete pasted images from the temp folder. A paste that cannot be saved pastes nothing, without a message.
- Windows 11 is tested by hand, Linux end to end in Docker, and macOS in GitHub Actions (build and tests, no window). Nobody has run OpenCompanion by hand on a Mac or on a Linux desktop (GNOME, KDE) yet.

## Roadmap

- Web Push to the phone (FR-42), which needs HTTPS and a push service.
- Running OpenCompanion by hand on macOS and on Linux desktops (GNOME, KDE), including the tray, notification clicks and start at sign-in.
- A successful Codex CLI run observed end to end, and Approve/Deny for Codex CLI and OpenCode. The M0 spike names the likely routes: `codex app-server` and `opencode serve`.
- Gemini CLI support once it can be tested, then more CLIs such as Aider and Qwen Code.
- A native mobile app (FR-58), after the web companion.
