# New session from the sidebar: per folder or with a folder picker

## Summary

Owner asked to start a session from a folder group in the sidebar, or from Sessions and pick the folder. The New session dialog is now one shared dialog opened from Overview, the Sessions label, or a folder heading (folder pre-filled).

## Changes

- `src/lib/NewSessionDialog.svelte` (new, rendered once in `Shell.svelte`): starts the session, toasts, opens it; closes via Cancel/Esc.
- `src/lib/store.svelte.ts`: `pendingNew` + `askNewSession(cwd?)`, next to `pendingDelete`.
- `src/routes/+page.svelte`: local dialog and `start` removed; buttons and `#new` call `askNewSession()`.
- `src/lib/Sidebar.svelte`: Sessions label row with `plus` "New session" (always shown, also with no sessions); folder heading wrapped in `.folder-row` with `plus` "New session in {folder}" shown on hover/focus; `.side-sessions` wrapper.
- `src/app.css`: narrow sidebar hides `.side-sessions` (label, add button and list together).
- Tests: `NewSessionDialog.test.ts` (new), `Sidebar.test.ts`, `overview.test.ts`.
- `DESIGN.md`: Sidebar spec, D6 New session.

## Decisions

- One dialog for every entry point, so the current page stays put when it opens from the sidebar.
- Folder add button reveals on hover/focus like the delete button; the Sessions one is always visible.

## Verification

- `bun run test`: 80 passed. `bun run check`: 0 errors, 0 warnings. `bun run build`: ok.

## Limitations

- Not clicked through in the running app (no smoke test requested).

## Follow-up

- none
