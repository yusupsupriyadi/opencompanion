# A session runs only while a CLI runs in its terminals

## Summary

The owner asked to fix when a session counts as running ("perbaiki kondisi session running atau tidak"), since the session terminal became a shell and shell tabs can run CLIs of their own. A session's status used to follow its shell, so a terminal whose CLI had exited stayed Running or Idle until `exit`, and could not be deleted. The owner chose: running means an AI CLI runs in any of the session's terminals, and a terminal left at its shell prompt can be deleted, closing its shells.

## Changes

- `src-tauri/src/db.rs`: `Status::Shell` ("No CLI running"): live, but `runs_cli()` is false. Orphans left in it close as Stopped.
- `src-tauri/src/monitor.rs`: `clis_under` takes any key type, so one read tells the session's own terminal from its tabs.
- `src-tauri/src/lib.rs`: `follow_clis` reads the process list every 2 s, keeps the sidebar counts (`session_clis` now returns them), and calls `Manager::sync_clis`. `git_switch` is refused only while a CLI runs.
- `src-tauri/src/session.rs`: `sync_clis` moves shell-backed sessions between Running and Shell, after a 15 s start grace and two missed reads in a row. The watcher skips screen and quiet checks while no CLI runs in the terminal. Delete accepts a Shell session, ends its watcher, kills the shell, and a `deleted` flag stops late updates from storing it again. `cli_in_terminal`.
- `src-tauri/src/orchestrator.rs`, `actions.rs`: a Chat card is not typed into a terminal at its shell prompt, where it would run as a command.
- Frontend: `runsCli` in `format.ts`; sidebar order, Overview and phone lists, Chat's live list, Delete (menu hint "Exit its CLI first" for terminals), SessionRow and Branch panel use it. New copy in `shell.ts`, `sessions.ts`, `backend.ts`.
- Tests: `tests/manager.rs` (Shell transitions and delete), `orchestrator.rs`, `db.rs`, `Sidebar.test.ts`, `ContextMenu.test.ts`, `overview.test.ts`. `DESIGN.md`, `README.md`.

## Decisions

- A CLI running only in a shell tab keeps the session Running ("Running in a terminal tab"): its working or waiting state there is not visible.
- The status is set by the backend, so the phone, Chat cards and git switching agree with the sidebar.

## Verification

- `cargo clippy --all-targets -- -D warnings`: clean. `cargo test --no-fail-fast`: 142 unit + 5 companion + 13 manager passed.
- `bunx vitest run`: 41 files, 248 tests passed; `bun run check`: 0 errors; `bun run build`: ok (a first run failed while Rust compiled alongside it; the next two passed).

## Limitations

- Not run in the app (no smoke test was requested). `follow_clis` itself has no test; its parts (`clis_under`, `sync_clis`) do.

## Follow-up

- none
