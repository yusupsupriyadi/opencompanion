# More in the terminal tab menu: rename, close right, duplicate, clear, restart, keys, folder

## Summary

The owner took all seven suggestions for the tab menu. A tab can now be renamed in place (menu or double-click), tabs to its right closed, a shell duplicated into the next tab, a terminal cleared or restarted (also from the terminal's own menu), the session folder's path copied or shown, and tabs switched with Ctrl+PageUp/PageDown and closed with Ctrl+Shift+W, from inside a terminal too.

## Changes

- `src-tauri/src/terminal.rs`: `clear_output` forgets a shell's output and keeps the count; covered in the existing shell test. `src-tauri/src/lib.rs`: `terminal_clear`. `src/lib/api.ts`: `terminalClear`.
- `src/lib/Terminal.svelte`: `clearTerminal(id)` for a mounted terminal, `tabKey` so xterm leaves the tab keys to the page.
- `src/lib/panes.svelte.ts`: `PaneTab.title`, `rename`, `insertTab`; titles survive moves.
- `src/routes/session/+page.svelte`: the menu items, the in-place name field, `duplicate`, `clearTerm`, the window shortcuts, tab names used across menus and dialogs.
- `src/lib/i18n/terminal.ts`: strings. Tests: five page tests, one model test, menu lists updated. `DESIGN.md` D13, `README.md`.

## Decisions

- The name belongs to the tab, not a shell, so it stays when the tab's first terminal closes.
- Clearing the session's own terminal clears the view only: its output also feeds the session log.

## Verification

- `cargo test --lib terminal::`: 4 passed; `cargo clippy --all-targets -- -D warnings`: clean.
- `bun run check`: 0 errors, 0 warnings. `bun run build`: ok. `bunx vitest run`: 47 files, 311 tests passed.

## Limitations

- Not run in the app by me. Ctrl+Shift+W and Ctrl+PageUp/PageDown are unverified in WebView2 itself.

## Follow-up

- none
