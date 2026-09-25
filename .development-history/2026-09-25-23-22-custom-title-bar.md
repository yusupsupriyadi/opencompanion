# Custom title bar instead of the native window frame

## Summary

Owner asked for the app bar (minimize, maximize, close) to be drawn by the app, not the native Windows frame.

## Changes

- `src/lib/TitleBar.svelte`: fixed 36px drag strip with Minimize, Maximize/Restore (tracks `isMaximized` via `onResized`), Close.
- `src/lib/Shell.svelte`: renders `<TitleBar />` last, for both the app shell and Onboarding.
- `src/app.css`: `--titlebar` token, `.app` top padding uses it, title bar block (46x36 hit areas, inset 34x28 hover fill, Close hover `st-err`).
- `src-tauri/tauri.conf.json`: `decorations: false`.
- `src-tauri/capabilities/default.json`: `allow-start-dragging`, `allow-minimize`, `allow-toggle-maximize`, `allow-close`.
- `src-tauri/src/lib.rs`: window-state plugin no longer saves/restores `DECORATIONS` (a saved `decorated: true` would bring the native frame back).
- `src/test/setup.ts`, `src/lib/TitleBar.test.ts`: window API mock and three tests.
- `DESIGN.md`: Title bar component spec, frame layout, window icons.

## Decisions

- No wordmark in the bar: the sidebar already carries the brand. The bar is transparent over the painting's sky.
- Buttons are last in the DOM so Tab reaches the page before the window controls.

## Verification

- `bun run test`: 85 passed. `bun run check`: 0 errors. `bun run build`: ok. `cargo check`: ok.

## Limitations

- Not viewed in the running app (no smoke test requested): drag, double-click maximize, edge resize and Win11 corner rounding are unverified by eye.
- While a modal `<dialog>` is open the page is inert, so the window buttons wait until it closes (Esc); Alt+F4 still works.

## Follow-up

- none
