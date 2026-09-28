# Red Delete in the right-click menu, no action buttons on sidebar rows

## Summary

The owner asked for Delete to read red and for the sidebar session list to drop its row buttons now that the right-click menu covers them.

## Changes

- `src/lib/ContextMenu.svelte`: Delete label and icon use `st-err` at rest and lit; a refused Delete stays grey.
- `src/app.css`: new `--glass-menu` tint (92% Day, 85% Dusk) for `.ctx-menu`, so the red keeps 4.5:1.
- `src/lib/Sidebar.svelte`: removed the hover buttons (new session, pin, delete) from folder and session rows; a filled pin mark (not a button, "Pinned" for screen readers) shows pinned rows.
- `src/lib/i18n/shell.ts`: dropped the three button labels, added "Pinned" / "Disematkan".
- `src/lib/Sidebar.test.ts`, `src/lib/ContextMenu.test.ts`: tests go through the menu; one new test checks rows have no buttons.
- `DESIGN.md`: `glass-menu` token, contrast rows, sidebar and Context menu entries.

## Decisions

- Only the sidebar list loses its buttons; the Overview and History rows keep their trash button.
- Tints were computed for the worst spot (black behind in Day, white behind in Dusk) with `contrast-check.py`: 4.66 / 4.82 at rest, 4.69 / 4.78 lit.

## Verification

- `bunx vitest run`: 40 files, 231 tests passed.
- `bun run check`: 0 errors, 0 warnings. `bun run build`: done.

## Limitations

- Not clicked through in the running app (no smoke test requested).

## Follow-up

- Remove the trash button from Overview and History rows too if the owner wants the same there.
