# All sessions, with search and filters (FR-35)

## Summary

Sessions older than today were reachable only through the sidebar's short list. "All sessions" (`/history`, linked from the Overview) lists every session newest first, 50 at a time, with a search over titles and prompts and filters for CLI, folder and status.

## Changes

- `src-tauri/src/db.rs`: `SessionQuery`, `search_sessions` (LIKE with `%`, `_` and `\` escaped, case-insensitive folder, status groups, limit and offset), `session_folders`. Test.
- `src-tauri/src/lib.rs`: `search_sessions` (with horizon marks) and `session_folders` commands.
- `src/routes/history/+page.svelte` (new): toolbar, debounced search that never lets an older answer win, rows follow live status from the store and drop deleted sessions, "Show 50 more", every state. Test.
- `src/routes/+page.svelte`: "All sessions" link in the section head and in the empty hint. `api.ts` types.
- PRD FR-35, DESIGN D12, README.

## Decisions

- No new nav item: the screen hangs off the Overview, which already owns sessions (R-23, no navigation change without the owner).

## Verification

- `cargo test --lib db::tests::history`: passed. Vitest history: 2 passed; `bun run check`: 0/0.

## Limitations

- none

## Follow-up

- none
