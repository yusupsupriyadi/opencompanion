# Close and pin terminal tabs from their right-click menu

## Summary

The owner asked for Close all, Close others and Pin in a tab's right-click menu. The menu now starts with Close tab, Close other tabs and Close all tabs, then Pin tab or Unpin tab, before the moves. Pinned tabs stay first, show a pin where the close was, and the bulk closes leave them open. Closing more than one shell asks first.

## Changes

- `src/lib/panes.svelte.ts`: `PaneTab.pinned`, `pin()`, and every save keeps pinned tabs first so no move crosses that line; moves keep the flag. `src/lib/panes.test.ts`: one test.
- `src/routes/session/+page.svelte`: the tab menu's close and pin items, `askClose`/`confirmClose` with the "Close {n} terminals?" dialog, the pin mark in place of the tab's `x`.
- `src/lib/i18n/terminal.ts`: strings. `src/routes/session/shells.test.ts`: three tests and the menu list updated. `DESIGN.md` D13.

## Decisions

- Only shells close: the session's own terminal closes with `exit`, so its tab's Close tab is refused with that reason.
- A pinned tab hides its `x` against a stray click; its menu still closes it.

## Verification

- `bun run check`: 0 errors, 0 warnings. `bun run build`: ok. `bunx vitest run`: 47 files, 306 tests passed.

## Limitations

- Not run in the app by me.

## Follow-up

- Suggested to the owner: rename a tab, close tabs to the right, restart or clear a shell, a second shell of the same kind in one click, and keys to switch tabs.
