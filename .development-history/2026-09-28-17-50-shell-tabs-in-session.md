# Shell tabs in the session screen

## Summary

The owner asked to fold the Terminal screen into the session ("di session bisa nambah tab"). Plain shells now open as tabs on a session's screen, always in the session's folder, next to the CLI's own tab and the file viewer. The Terminal nav item and the `/terminal` route are gone.

## Changes

- `src-tauri/src/terminal.rs`: `TerminalInfo.session_id`, `open` takes the session, `sessions()` and `close_session()`; new unit test.
- `src-tauri/src/lib.rs`: `terminal_list`/`terminal_open` take `session_id` (the folder comes from the session); deleting a session ends its shells off the calling thread; retention passes the sessions with open shells.
- `src-tauri/src/session.rs`: `Manager::prune` skips sessions in `in_use`; test extended.
- `src/lib/shells.svelte.ts` (new): one session's shells, events, numbering. `src/routes/session/+page.svelte`: the tab strip is always shown (CLI tab, shells, viewer, `+` New terminal), shell panels with Exited/Restart/Close, load error with Try again.
- `src/app.css`: New terminal sits on a tab-shaped glass plate (38 × 38, `ink` icon) so it shows over the painting, as the owner asked.
- `src/lib/TerminalForm.svelte`: the folder is fixed and shown read-only; only the shell is picked. `src/lib/api.ts`, `Sidebar.svelte`, i18n `terminal`/`workspace`/`shell`: API and copy updated, nav item removed.
- Tests: `src/routes/session/shells.test.ts` replaces `routes/terminal/terminal.test.ts`; session, workspace and sidebar tests updated. `e2e/linux/e2e.py` opens the shell tab from the interactive session.
- `DESIGN.md` (D4 viewer, D13 rewritten, nav, icons, decisions), `README.md`, `CONTRIBUTING.md`.

## Decisions

- New terminal keeps its dialog (shell choice with its path) instead of opening the default shell at once: no menu component exists, and the dialog keeps Git Bash and Command Prompt one choice away.
- Retention skips a session with an open shell rather than killing a dev server an hour later; an explicit delete ends the shells.

## Verification

- Exact commit tree (HEAD + only these hunks, exported to a temp folder, because another agent's unfinished Board removal did not compile in the shared tree): `cargo test --no-fail-fast` 135 unit + 5 companion + 12 manager passed; `cargo clippy --all-targets -- -D warnings` clean; `bun run check` 0 errors; `bun run build` ok. Three `projects.rs` tests were skipped there because discovery ignores folders under `%TEMP%` by design.
- Shared working tree: `bunx vitest run` 40 files, 223 tests passed (7 new).

## Limitations

- Not run in the app (no smoke test was requested); the Linux e2e script was updated but not run.

## Follow-up

- none
