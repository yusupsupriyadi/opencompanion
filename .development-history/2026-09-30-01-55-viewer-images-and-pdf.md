# File viewer: images and PDFs

## Summary

The owner asked for PDFs and images to open in the session file viewer, where they used to show as "binary file". Pictures now show as themselves, PDFs as pages, both with zoom; SVG switches between Preview and Code.

## Changes

- `src-tauri/src/files.rs`: `read_bytes` inside the session folder, up to 50 MB (`MEDIA_TOO_LARGE` past it); `src-tauri/src/lib.rs`: `folder_read_bytes` returns raw bytes (`tauri::ipc::Response`), no JSON.
- `src/lib/media.ts`: formats by extension, zoom steps. `src/lib/PdfPages.svelte`: pdf.js (legacy build, loaded on first PDF), pages drawn only near the view and released after, canvases capped at 16.7 M pixels, a selectable text layer, reading place kept on zoom.
- `src/lib/FileViewer.svelte`: picture and PDF states, zoom group (Zoom out, Fit/percent, Zoom in), format and size in the bar, Preview | Code for SVG, checkerboard behind transparency.
- `src/lib/api.ts`, `src/lib/i18n/workspace.ts`, `package.json`/`bun.lock` (`pdfjs-dist` 6.3.289), `DESIGN.md`, `README.md`.

## Decisions

- pdf.js instead of the webview's own PDF viewer: WebKitGTK on Linux has none, and pdf.js looks the same on all three systems.
- A picture is read again on a git status change only when its own change entry moved, or on Refresh, since it can be 50 MB.

## Verification

- `cargo test --lib files::`: 8 passed (1 new). `bunx vitest run src/routes/session/workspace.test.ts src/lib/media.test.ts`: 18 passed (6 new).
- Full `bunx vitest run`: 294 of 295 passed; the failure is in `session.test.ts`, and `bun run check` reports errors only in `Terminal.svelte` and `session/+page.svelte`, all from another agent's uncommitted work. `bun run build`: ok, pdf.js worker emitted as its own asset.

## Limitations

- Not run in the app (no smoke test was requested). No text search or links inside PDFs, and no CMaps for CJK PDFs without embedded fonts.

## Follow-up

- none
