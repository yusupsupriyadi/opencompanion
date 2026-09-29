# Drag and drop for session terminals

## Summary

While testing the split, the owner found that dragging did not work and asked for every kind: the border between split terminals, reordering tabs, dragging a tab onto another tab or beside a terminal to split, and moving a split terminal along its tab or out to its own tab. All of these now work by pointer, and the same moves are in menus for the keyboard and a single click.

## Changes

- `src/lib/panes.svelte.ts`: `PaneDrop`, `moveTab`, `canMove`, `move` (a terminal keeps its share in its own tab and gets an equal one elsewhere; an emptied tab goes away). `src/lib/panes.test.ts`: six new tests.
- `src/routes/session/+page.svelte`: one keyed list of terminals in tab order (moving never restarts xterm, and Tab follows the visual order); drags follow the pointer on the window with a 5 px threshold, a name label, drop marks and Escape to cancel; the border is a 1 px line with a 9 px grip; move menus on tabs (right-click) and on split terminal names (click).
- `src/lib/i18n/terminal.ts`: drag hint and menu strings. `src/routes/session/shells.test.ts`: five new tests. `DESIGN.md` (D13, icons, decision), `README.md`.

## Decisions

- Pointer events rather than HTML drag and drop: on Windows, Tauri takes the webview's drag and drop for dropped files.
- The first border drag used pointer capture on a 7 px grip inside the right terminal. It now listens on the window, so the drag keeps going whatever the pointer passes over.

## Verification

- `bun run check`: 0 errors, 0 warnings. `bun run build`: ok. `bunx vitest run`: 41 files, 259 tests passed.

## Limitations

- Not run in the app by me; jsdom has no layout, so the tests fake the element under the pointer and its size.

## Follow-up

- none
