# Chat header without Change planner or plate

## Summary

The owner asked to drop the Change planner button from the chat page and then to remove the plate behind the header controls. The header now keeps only its remaining controls, right-aligned, with no plate.

## Changes

- `src/routes/chat/+page.svelte`: removed the `/settings#chat` link, moved the sr-only `h1` out of its `.grow` wrapper, and turned off the global `.page-head` plate on this page (no background, padding, rim or blur). The header hugs its controls at the right, and each control carries its own glass fill.

## Decisions

- Controls keep their own glass fill: ink over the bare Day sky drops to about 1.9:1, below the 3:1 icon and 4.5:1 text minimums.

## Verification

- `bun run check`: 0 errors, 0 warnings. `bun run test src/routes/chat`: 16 passed. `bun run build`: ok.

## Limitations

- Not viewed in the running app (no smoke test requested). `design/ai-remote.pen` still shows the old button.

## Follow-up

- none
