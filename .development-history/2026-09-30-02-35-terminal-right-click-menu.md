# Right-click menu in a terminal

## Summary

The owner asked for a right-click menu with a new terminal tab and splits to the right, left and bottom. Right-clicking inside the session's terminal or a shell now opens Copy, Paste, New terminal tab, Split right, Split left and Split down; a split puts the new shell on that side of the terminal right-clicked.

## Changes

- `src/lib/Terminal.svelte`: optional `onmenu` takes the right-click with the selection, whether the terminal runs, and a paste function.
- `src/lib/panes.svelte.ts`: `split(pane, id, side)` places a new shell beside `pane`.
- `src/routes/session/+page.svelte`: `termMenu` builds the menu; the New terminal dialog remembers the pane and side it was opened for.
- `src/lib/i18n/terminal.ts`: menu strings and the paste fallback. `src/routes/session/shells.test.ts`: two tests. `DESIGN.md` (D13, icons), `README.md`.

## Decisions

- Copy and Paste stay in the menu because it replaces the webview's own, which offered them over xterm.
- Paste reads the clipboard through the web API; Tauri leaves clipboard access off, so WebView2 may ask the first time or refuse, and a refusal shows a toast pointing to the paste key.

## Verification

- `bun run check`: 0 errors, 0 warnings. `bun run build`: ok. `bunx vitest run`: 43 files, 275 tests passed.

## Limitations

- Paste in the real WebView2 is unverified; it depends on the clipboard permission.

## Follow-up

- none
