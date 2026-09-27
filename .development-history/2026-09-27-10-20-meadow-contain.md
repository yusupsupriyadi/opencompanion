# Meadow painting fits instead of cropping

## Summary

The owner wants `meadow-day.png` never cropped. The desktop shell already used contain with a blurred cover copy; onboarding and the phone header still used `object-fit: cover`.

## Changes

- `src/routes/onboarding/+page.svelte`: `.scene-art` is now `contain`, bottom-anchored at 70% like `app.css`; a new `.scene::before` paints the same painting cover-scaled and blurred 32px behind it to fill the band. The dark dim moved into a `--scene-dim` variable so both layers share it.
- `src/lib/phone.css`: `.art` drops the fixed 280px height for `aspect-ratio: 1672 / 941` with `height: auto` and `contain`, so the header shows the whole painting at full width. Rounded bottom corners, the full-bleed margins and the dark filter are unchanged.

## Decisions

- The phone header needs no blurred copy: its box takes the painting's own ratio, so contain leaves no band. If the file's ratio ever changes, the band shows the page's `--bg`.
- `app.css` left as is: it already fits the painting whole.
- `landing/` is out of scope: it uses separate parallax layers, not this file.

## Verification

- `bunx vitest run`: 30 files, 152 tests passed.
- `bun run check`: 0 errors, 0 warnings.
- `bun run build`: succeeded; built CSS carries the new rules and resolves the painting to the same path as `app.css`.

## Limitations

- Not viewed in a browser or the Tauri window; no smoke test was requested.
- The phone header is now about 219px tall on a 390px phone (270px at the 480px cap) instead of 280px.

## Follow-up

- none
