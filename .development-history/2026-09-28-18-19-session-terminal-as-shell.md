# A session's terminal is a shell, and the session screen has no header

## Summary

The owner wants the session terminal to work like a terminal of their own ("bisa exit claude code dan start"), without Resume, Stop and Mark done on top, and the header card removed ("di sidebar aja sudah cukup"). Interactive sessions now run in a shell that stays after the CLI exits; typing the CLI's name starts it again with the session's flags and hooks, and `exit` closes the terminal.

## Changes

- `src-tauri/src/pty.rs`: `PtySpec::stay` (`Stay { name, again, rc }`). On Windows PowerShell runs `-NoExit` with a global function named after the CLI; elsewhere bash runs with a generated `--rcfile` (the user's `.bashrc`, the function, then the CLI, all single-quoted). `PtySession::stays()`. New real-shell test.
- `src-tauri/src/session.rs`: interactive sessions pass `Stay` (hooks, permission flags, extra args, no prompt or resume). The shell's own exit ends the session as Done, "Terminal closed", with no notification. The rc file is deleted with the session.
- `src/routes/session/+page.svelte`: the header card is gone (screen-reader H1 only), and its meta moved to a "Session" group in Details. A closed terminal gets Open terminal in its note, and a running headless session gets Stop in its output bar. `src/lib/i18n/sessions.ts`: new copy; the Mark done and Resume strings are removed.
- Tests: `tests/manager.rs` (relaunch by name, `exit`), `tests/companion.rs` (the phone sends `exit`), `session.test.ts` rewritten. `DESIGN.md` D4 and decisions, `README.md`.

## Decisions

- Typed relaunches keep the session's hooks and permission mode through the function, so Waiting for you and Approve/Deny still work after a restart of the CLI.
- Windows PowerShell's `ConvertFrom-Json` returns an array as one object, so the script unwraps it with `foreach` before splatting.
- Headless sessions keep Stop, moved into their output bar: they have no terminal to exit from. The phone keeps its Stop, Done and Resume.

## Verification

- `cargo test --no-fail-fast`: 140 unit + 5 companion + 12 manager passed; `cargo clippy --all-targets -- -D warnings`: clean.
- `bunx vitest run`: 38 files, 212 tests passed; `bun run check`: 0 errors; `bun run build`: ok.

## Limitations

- Not run in the app (no smoke test was requested). The bash path was only compiled on Windows; the Linux and macOS runs of its test are unverified until CI or the Docker e2e runs.

## Follow-up

- none
