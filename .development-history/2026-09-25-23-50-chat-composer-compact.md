# Compact chat composer

## Summary

The Model and Thinking pickers in the chat composer are now boxless text pickers with a caret, the Send button is icon-only, and the fixed hint under the composer is gone.

## Changes

- `src/lib/PlannerModel.svelte`: pickers drop the border and fill, size to the chosen option (`field-sizing: content`, max 240), show a `caret-down`, and Thinking leads with a `brain` icon. Labels are screen-reader only; tooltips name the picker and the full choice.
- `src/routes/chat/+page.svelte`: Send is a 36 × 36 icon button (`aria-label="Send"`, 44 × 44 under 720). The "Nothing starts until you press Run on a card" hint is removed; the "still answering in another chat" line shows only in that state.
- `DESIGN.md`: Composer spec and icon list updated.

## Decisions

- Kept native `<select>` elements so keyboard use and the option popup stay as they were.
- Shift+Enter moved into the Send tooltip; the planner intro already says nothing starts until Run.

## Verification

- `vitest run` PlannerModel and chat tests: 18 passed.
- `bun run check`: 0 errors, 0 warnings. `bun run build`: succeeded.

## Limitations

- Not checked in the running app (no smoke test was requested). Browsers without `field-sizing` fall back to the width of the longest option.

## Follow-up

- none
