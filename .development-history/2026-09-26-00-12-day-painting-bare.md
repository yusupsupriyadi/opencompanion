# Day painting without haze, text on plates

## Summary

The owner asked for the background painting to show its colors again, and for Day to have no overlay at all. Day now shows the painting untouched, Dusk is dimmed less, and every block of text over the painting sits on an `over-art` plate so it keeps WCAG AA.

## Changes

- `src/app.css`: Day `--haze: none`; Dusk haze 50% to 38%, filter `brightness(.42) saturate(.85)` to `brightness(.58)`, glass 84% to 65%. New plate rule for `.page-head`, `.section-head`, `.hint`, `.col-head`, `.planner`, `.activity`, `.detail-side`, `.note`, `.not-found`, `.toolbar`.
- `src/routes/board/+page.svelte`: column head padding 0/4 to 8/12 so the plate has room.
- `DESIGN.md`: section 2 tokens, plate rule, and contrast table updated; layout and illustration notes follow.

## Decisions

- Owner picked plates over a text halo or accepting sub-AA text. Day glass stays 84%: lower fails `ink-2` over the bare painting.

## Verification

- Pixel simulation of the painting (cover, 70% bottom, 1100 to 2560 wide): text on plates worst 5.85 Day / 7.03 Dusk; sidebar `ink-2` 4.64 Day / 6.13 Dusk; Dusk bare `ink` 5.14.
- `bun run check`: 0 errors. `bun run build`: ok. `bun run test`: 105 passed.

## Limitations

- Not viewed in the running app (no smoke test requested). Onboarding and phone art keep their old Dusk filter.

## Follow-up

- Owner to review the plates in the app and adjust padding if needed.
