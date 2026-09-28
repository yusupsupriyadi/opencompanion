# Sidebar counts the CLIs running in each session

## Summary

The owner asked for the sidebar to show how many Claude instances run in a session, since a session can open several shell tabs and start `claude` in each ("ketahuan di session ada berapa claude yang sedang jalan"). Each session row now shows "×2" when two AI CLIs run in its terminals, and the tooltip and screen reader name them ("Claude Code × 2, Codex CLI running").

## Changes

- `src-tauri/src/monitor.rs`: `Monitor::clis_under` walks the process tree under each root and counts the outermost AI CLIs (`outer_clis`); a child older than its parent is ignored (Windows PID reuse). Unit tests plus a real-process check in the Windows test.
- `src-tauri/src/session.rs`: `Manager::live_pids` (session id, pid); `own_pids` uses it.
- `src-tauri/src/lib.rs`: `session_clis` command: roots are live sessions' own processes and every running shell tab.
- `src/lib/api.ts`, `src/lib/store.svelte.ts`: `app.sessionClis`, read every 3 s while the window is visible; a failed read clears the counts.
- `src/lib/Sidebar.svelte`, `src/lib/i18n/shell.ts`: count before the pin mark, tooltip and screen-reader text. `Sidebar.test.ts`: two tests.
- `DESIGN.md` (Sidebar, accessibility), `README.md`.

## Decisions

- Count every AI CLI kind, not only Claude Code, and count a CLI started by another CLI (CCS starting Claude Code, a tool call) once, as the outer one.
- Show the count from 1: with the terminal running in a shell, a Running session can have no CLI left in it, and no number says so.

## Verification

- `cargo clippy --all-targets -- -D warnings`: clean. `cargo test --no-fail-fast`: 142 unit + 5 companion + 12 manager passed (separate target dir, because the running app locks `target\debug\opencompanion.exe`).
- `bunx vitest run`: 233 tests passed; `bun run check`: 0 errors; `bun run build`: ok.

## Limitations

- Not run in the app (no smoke test was requested).

## Follow-up

- none
