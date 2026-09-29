# Split terminals in a session's tabs

## Summary

The owner asked for split terminals ("bisa buka beberapa terminal dalam satu tab") and picked side by side with a Stack terminals toggle, one direction per tab, no nesting. Split terminal opens a shell at the end of the shown tab, including the session's own CLI tab, up to four per tab, with draggable and keyboard-resizable borders.

## Changes

- `src/lib/panes.svelte.ts` (new): `PaneLayout`, the tabs and their terminals, direction and sizes, kept per session for the life of the window. `src/lib/panes.test.ts`: unit tests.
- `src/lib/shells.svelte.ts`: `open(shell, place)` places a split shell before it is listed, so its own tab never flashes.
- `src/routes/session/+page.svelte`: tabs from the layout, Split terminal and Stack terminals buttons, one dark panel with every terminal mounted, pane headers with Close, `role=separator` borders, one keyboard note per split tab.
- `src/lib/TerminalForm.svelte`, `src/lib/i18n/terminal.ts`: "Split terminal" title and strings, help text no longer says "its tab".
- `src/routes/session/shells.test.ts`: five page tests. `DESIGN.md` (D13, icons, decision), `README.md`.

## Decisions

- Panes stay mounted in DOM order and only their visibility changes, so a split never restarts xterm or rereads its output.
- The border line is `term-dim` at 60% (3.39:1 Day, 3.76:1 Dusk) because it is a control's edge.

## Verification

- `bun run check`: 0 errors, 0 warnings. `bun run build`: ok.
- `bunx vitest run`: 41 files, 248 tests passed (12 new).

## Limitations

- Not run in the app (no smoke test was requested); pointer dragging is covered by code only, jsdom has no layout.

## Follow-up

- none
