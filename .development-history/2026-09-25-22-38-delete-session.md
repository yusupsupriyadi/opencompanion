# Delete a session from the session lists

## Summary

Finished sessions (Done, Error, Stopped) can be deleted from the Overview list and the sidebar Sessions list, after a confirmation dialog.

## Changes

- `src-tauri/src/db.rs`: `delete_session` removes the row and its events in one transaction and unlinks the board card that ran it.
- `src-tauri/src/session.rs`: `Manager::delete` refuses live sessions, drops the finished handle (closes the log so Windows can remove it), deletes `sessions/{id}.log` and `hooks/{id}.jsonl`; `hook_path` helper.
- `src-tauri/src/lib.rs`: `delete_session` command; emits `session-deleted` and `tasks-changed`, sends `resync` to paired phones.
- `src/lib/DeleteSessionDialog.svelte` (new, rendered once in `Shell.svelte`), `store.svelte.ts` (`deleteSession`, `askDelete`, `pendingDelete`, `session-deleted` listener), `api.ts`.
- `src/lib/SessionRow.svelte`: trash button beside the row link (not inside it); every row keeps the space so horizons stay aligned; 44 px under 720.
- `src/lib/Sidebar.svelte`: trash button per finished item, shown on hover or keyboard focus.

## Decisions

- Live sessions have no Delete; the backend also refuses them ("Stop this session before deleting it.").
- Project files the CLI changed are never touched; the dialog says so.
- Deleting the session that is open returns to Overview.

## Verification

- `cargo test --lib`: 46 passed (2 new); `cargo clippy --all-targets`: 0 warnings.
- `bun run check`: 0 errors; `bun run build`: ok; new `DeleteSessionDialog.test.ts`: 5 passed.

## Limitations

- The app itself was not run (no smoke test requested).
- `Sidebar.test.ts` "waiting sessions come first" fails after a parallel change that shows titles instead of folder names in the sidebar; not part of this task.

## Follow-up

- none
