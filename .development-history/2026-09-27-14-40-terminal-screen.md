# Terminal screen for plain shells

## Summary

Sessions are only for AI coding CLIs, so running a project (dev server, build, tests, git) had no place in the app. A new Terminal screen opens plain shells in project folders, in tabs, separate from sessions: no status detection, no SQLite history, desktop only.

## Changes

- `src-tauri/src/terminal.rs`: shell detection (PowerShell 7, Windows PowerShell, Command Prompt, Git Bash; `$SHELL`, zsh, bash, sh elsewhere) and a `Terminals` manager: open, write, resize, output snapshot with chunk numbers, restart, close (kills the process tree), kill all on exit. Unit tests run a real shell.
- `src-tauri/src/lib.rs`: `terminal_*` commands, `terminal-output`/`terminal-changed` events (webview only, never the companion), terminals killed on app exit.
- `src-tauri/src/pty.rs`: `PtySession::kill` split out of `stop`. `src-tauri/src/session.rs`: `keep_tail`, `decode_utf8` and `OUTPUT_KEEP` shared.
- `src/routes/terminal/+page.svelte`, `src/lib/TerminalForm.svelte`: tabs, New terminal dialog (folder + shell, last shell remembered), exited state with Restart and Close, loading/error/empty states. Tests.
- `src/lib/Terminal.svelte`: `kind` prop picks the session or terminal I/O; terminals get Ctrl+C copy with a selection and Ctrl+V paste.
- `src/lib/Sidebar.svelte`, `src/lib/api.ts`, `src/lib/i18n/*`: nav item, API calls, English/Indonesian copy and backend messages. `DESIGN.md`: D13 Terminal, icon and nav order.

## Decisions

- Owner chose a Terminal page with tabs (not a bottom dock) and desktop only, so a raw shell is never reachable over the LAN.
- Closing a tab ends the shell and everything it started, without a confirmation.

## Verification

- `cargo test`: 89 unit (3 new terminal tests) + 12 manager + 5 companion passed; `cargo clippy --all-targets`: clean.
- Vitest: 33 files, 189 tests passed (7 new); `bun run check`: 0 errors; `bun run build`: ok.
- Vitest must run from `C:\Users\yusup\Project\...` (capital P, the real folder name); from the lower-case path the setup file loads as a second module and jest-dom matchers are missing.

## Limitations

- Not exercised in the running app (no smoke test was requested): Ctrl+V paste and Git Bash startup are checked by code, not by hand.

## Follow-up

- none
