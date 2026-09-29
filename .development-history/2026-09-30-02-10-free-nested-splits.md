# Free, nested terminal splits

## Summary

Testing the drag, the owner found that a split tab only took terminals the way it already ran ("cuman support vertical bottom, horizontal gak bisa") and asked for splits that are "fleksibel dan bebas". A tab's layout is now a tree: a terminal dropped on any side of any terminal splits that spot that way, one split inside another.

## Changes

- `src/lib/panes.svelte.ts`: `PaneNode` tree per tab, folded so a split never holds one part or a part running its way; `rects` and `borders`; `split`, `flip` (turns every split), `swap`, `dock`, `resize` by split path keeping 120 px per terminal lined up, `move` with a side.
- `src/routes/session/+page.svelte`: terminals placed absolutely from `rects` (no nesting in the page, so nothing restarts); one border per split, after the terminals before it; drops pick the nearest side; the terminal menu offers Swap with, the four edges, a new tab and Split with.
- `src/lib/i18n/terminal.ts`: menu and hint strings. Tests: `src/lib/panes.test.ts` rewritten (14), `src/routes/session/shells.test.ts` updated plus one nested drop test. `DESIGN.md` (D13, icons, decisions), `README.md`.

## Decisions

- The Stack button turns the whole tree, so a nested layout keeps its shape.
- Menu moves are Swap and Dock, since "left" and "right" have no single meaning in a nested layout.

## Verification

- `bun run check`: 0 errors, 0 warnings. `bun run build`: ok. `bunx vitest run`: 42 files, 267 tests passed.

## Limitations

- Not run in the app by me; the tests fake the element under the pointer and its size.

## Follow-up

- none
