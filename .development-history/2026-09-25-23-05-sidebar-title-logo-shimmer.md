# Sidebar sessions: title and provider logo, rainbow shimmer while running

## Summary

Owner asked for sidebar session rows to show only the title and the provider icon, and for a running session's title to shimmer in rainbow colors.

## Changes

- `src/lib/Sidebar.svelte`: row is `CliMark bare` + title; status dot and "CLI · status" line removed; CLI and status kept as `sr-only` text and in the tooltip; `shimmer` class on the title when status is `running`; folder icon 16.
- `src/lib/CliMark.svelte`: `bare` prop (logo alone, no box).
- `src/app.css`: `--rainbow` token per theme (Day, Dusk, OS dark), `.shimmer` + keyframes, `.climark.bare`, single-line `.mini` (gap 6, dropped `.mini span`).
- `src/lib/Sidebar.test.ts`: row content, accessible name, tooltip, shimmer only on running.
- `DESIGN.md`: token table, MOTION 1 exception, Sidebar spec, status accessibility rule.

## Decisions

- Shimmer overrides MOTION 1 at the owner's request; `prefers-reduced-motion` stops it (static gradient).
- Every point of the gradient, midpoints included, is at least as contrasting as `ink-2` (Day 6.38:1, Dusk 8.38:1 on `bg`), so it holds over the glass sidebar.
- Only `running` shimmers; `starting`, `idle` and `waiting` show plain titles.

## Verification

- `bun run test`: 64 passed. `bun run check`: 0 errors, 0 warnings. `bun run build`: ok.

## Limitations

- Not viewed in the running app (no smoke test requested). Waiting/error sessions no longer have a visible marker in the row; the collapsed folder heading still shows waiting.

## Follow-up

- none
