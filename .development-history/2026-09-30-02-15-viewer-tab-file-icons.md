# Viewer tab: a colored glyph per file type

## Summary

The owner asked for the viewer tab to show what it holds (a PDF icon for a PDF, an image icon for a picture, an MD icon for Markdown), then for the icons to be colored. The tab now draws Phosphor's file-type glyph before the file name, in the color people tie to that type.

## Changes

- `src/lib/FileIcon.svelte`: `fileIcon(path)` maps about 90 extensions and a few whole names (lock files, `.env`, Dockerfile) to Phosphor file glyphs; `fileTone(path)` picks one of six colors or none.
- `src/app.css`: `--ft-*` tokens from the rainbow list (Day on light glass, Dusk on dark glass and on the shown tab over `term-bg`), `.ft-*` classes, the glyph at 18 in the viewer tab.
- `src/routes/session/+page.svelte`: the glyph in the viewer tab. `DESIGN.md`: palette, contrast rows, D4 tab spec, icon list, decision.
- Tests: `src/lib/FileIcon.test.ts`, one assertion in `src/routes/session/workspace.test.ts`.

## Decisions

- Colors reuse the rainbow list; Day has no readable bright yellow on cream, so `ft-yellow` Day is a new dark gold `#735600`, checked at least as contrasty as `ink-2`.
- Text, lock, and unknown files stay uncolored in the tab's secondary color.

## Verification

- `bunx vitest run src/lib/FileIcon.test.ts src/routes/session/workspace.test.ts`: 19 passed; `bun run check`: 0 errors; `bun run build`: ok.
- Color pairs checked with `contrast-check.py` on Day and Dusk `surface`/`bg` and on `term-bg`.

## Limitations

- Not run in the app (no smoke test was requested). The Files tree still shows the plain `file` glyph.

## Follow-up

- Offer the same glyphs in the Files tree and the Changes list.
