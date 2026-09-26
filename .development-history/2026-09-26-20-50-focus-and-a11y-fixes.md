# Focus, names and announcements from the frontend review

## Summary

The remaining accessibility and state findings of the frontend code review.

## Changes

- `src/lib/focus.ts` (new): `refocus` puts focus on the first control left in a card or on the page's main region when the pressed button went away. Used by `Dialog` (opener deleted), `NeedsYou` (Approve/Deny) and `DispatchCard` (Run, Discard, Undo, Save). `app.css`: no ring on the main region itself.
- `src/app.css`, `Sidebar.svelte`: below 1100 the sidebar labels are visually hidden instead of removed, so Day and Dusk keep their names; the buttons get tooltips like the nav links.
- `src/routes/settings/+page.svelte`: the pairing countdown is no longer a live region (it was read every second); the expiry is announced once. A save that fails puts checkboxes, selects and the permission mode back to the stored value.
- `src/routes/onboarding/+page.svelte`: the planner picker shows the first CLI that can plan instead of an empty select.
- `src/routes/m/board/+page.svelte`: a card deleted on the computer while its sheet is open closes the sheet with a note.
- Tests for onboarding (new file), Settings rollback and the phone board sheet. DESIGN section 12.

## Verification

- Vitest: 123 passed; `bun run check`: 0/0.

## Limitations

- Focus moves are checked by reading the code, not with a screen reader (no smoke test requested).

## Follow-up

- none
