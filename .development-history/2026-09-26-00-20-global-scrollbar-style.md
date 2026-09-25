# Global scrollbar style

## Summary

Every scrollbar was the WebView2 default. Added one global style from the design tokens: a 12px gutter with a 6px thumb (8px on hover), square ends like the status dots and horizon marks, and a transparent track. Contrast was measured, not guessed, including over the painting pixels (`static/meadow-day.webp`, cover at 70% 100%, 1100x700 to 1920x1080).

## Changes

- `src/app.css`: `--scroll-thumb` (line-strong mixed 25% toward ink-2) and `--scroll-thumb-hover`; per-surface overrides for `.term` (term-dim 60%) and `.sidebar` (ink-2); `::-webkit-scrollbar` rules for fine pointers only; `.main`, `.thread`, `.board` and `.detail-side` (no panel, straight on the painting) use an ink thumb ringed 1px in bg; `scrollbar-color` fallback for engines without `::-webkit-scrollbar`.
- `src/lib/Terminal.svelte`: xterm 6 slider colors match the `.term` thumb.

## Decisions

- line-strong alone fails 3:1 on surface-2 (2.86 Day, 2.73 Dusk), hence the mix toward ink-2.
- No single thumb color holds 3:1 over the painting (47% of the column fails in Day, 90% in Dusk); ink + 1px bg ring is the only tested pair that holds everywhere (worst 3.35 Day, 5.26 Dusk).
- Touch screens keep native overlay scrollbars (`hover: hover` and `pointer: fine`).

## Verification

- Contrast scripts: soft thumb 3.33-4.53 on bg/surface/surface-2, terminal 3.38/3.74, sidebar glass (ink-2) 4.70+, painting (ink ring) 3.35+.
- `npx vite build`: exit 0; `svelte-check`: 0 errors, 0 warnings.
- Not viewed in the running app (BrowserOS Neo not connected).

## Limitations

- Below 720px (browser only; Tauri min width is 1100) the window itself scrolls and keeps the soft thumb over the painting.

## Follow-up

- Add a Scrollbar row to DESIGN.md section 8 once the pending DESIGN.md edits are committed.
