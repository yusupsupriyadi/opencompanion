# Right-click menus on session and folder rows

## Summary

Right-clicking a session row (sidebar, Overview, History) or a folder row in the sidebar now opens an in-app menu instead of the webview's Back/Refresh/Print menu.

## Changes

- `src/lib/context-menu.svelte.ts`: shared menu state, `openMenu`/`closeMenu`, and the session and folder item builders (open, new session in folder, pin, copy path, show in file manager, delete).
- `src/lib/ContextMenu.svelte`: the menu, rendered once by the shell; keeps inside the window and below the title bar, WAI-ARIA menu keys, closes on outside click, scroll, resize and window blur.
- `src/lib/Sidebar.svelte`, `src/lib/SessionRow.svelte`: `contextmenu` handlers; the row stays lit while its menu is open.
- `src/lib/Shell.svelte`, `src/app.css`: mount the menu and give it the pop-up glass.
- `src/lib/i18n/shell.ts`: English and Indonesian strings.
- `src/test/setup.ts`, `src/lib/ContextMenu.test.ts`: opener mock and 8 tests.
- `DESIGN.md`: Context menu component row.

## Decisions

- Custom menu over the native Tauri menu: the frameless window and Day/Dusk theme would clash with an OS menu.
- The module is `context-menu.svelte.ts`, not `contextmenu.svelte.ts`: on Windows the latter resolves to `ContextMenu.svelte`.
- A live session's Delete stays listed but disabled with "Stop it first", matching the backend's refusal.
- Only the Delete icon turns red; red label text drops just under 4.5:1 over the Dusk glass.

## Verification

- `bunx vitest run`: 40 files, 230 tests passed.
- `bun run check`: 0 errors, 0 warnings. `bun run build`: done.

## Limitations

- Not clicked through in the running app (no smoke test was requested).
- Elsewhere in the app the webview's default menu still appears.

## Follow-up

- Optionally suppress the webview menu outside text fields, and add menus to chat threads or terminal tabs.
