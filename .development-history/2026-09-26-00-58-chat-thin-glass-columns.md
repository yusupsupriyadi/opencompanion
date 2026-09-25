# Thinner glass on the Chat history and live rail

## Summary

The owner found the Chat history panel read as a second sidebar (a tall glass card next to the sidebar) and asked for it to be more transparent. The history panel and the docked live rail now use a new, thinner glass tint.

## Changes

- `src/app.css`: new `--glass-thin` token, Day `#FFFAE68F` (56%) and Dusk `#242A224D` (30%).
- `src/routes/chat/+page.svelte`: `.history` and the docked `.rail` set `--surface: var(--glass-thin)` and `--ink-2: var(--ink)`. The pop-up rail keeps `glass-pop`.
- `DESIGN.md`: token row, a rule note, and two contrast rows.

## Decisions

- Thin glass only passes with `ink`, so secondary text in these columns uses `ink` (as on the plates); `ink-2` would need the old 82% / 45%.
- The docked rail gets the same tint because it is the other full-height column on the page.

## Verification

- Pixel simulation (blur 24 + saturate 1.4, cover and contain backgrounds, 360 to 2560 wide), calibrated against the recorded 82% numbers (4.70 vs 4.67): `ink` worst 4.98 Day / 6.86 Dusk, focus ring 3.19 / 4.09.
- `bun run check`: 0 errors, 0 warnings. `bun run test`: 105 passed. `bun run build`: ok.

## Limitations

- Not viewed in the running app (no smoke test requested). The current-chat row still uses `glass-2` (90%).

## Follow-up

- none
