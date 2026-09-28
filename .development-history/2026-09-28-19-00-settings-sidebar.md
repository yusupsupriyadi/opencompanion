# Settings sidebar with grouped sections

## Summary

Settings now swaps the app sidebar for its own: Back to app, a search box, and grouped items
(General, Agents, Workspace, Appearance). Each item is its own screen; the old General tab,
one long page of cards in a two-column grid, is split into one section per screen.

## Changes

- `src/lib/settings-nav.ts`: the groups and items, `?s=` section lookup, search filter.
- `src/lib/SettingsSidebar.svelte`: the sidebar (Back, search with Ctrl+F/⌘F, Enter, Escape, empty state).
- `src/lib/Shell.svelte`, `src/app.css`: show it on `/settings*`, remember the last app screen for Back, keep a 200 column below 1100.
- `src/lib/SettingsHead.svelte`: H1 is the open item's name; segmented tabs removed.
- `src/routes/settings/+page.svelte`: one section at a time, cards no longer repeat their title, 960 column.
- `src/routes/settings/{clis,skills}/+page.svelte`: drop the sr-only H2 that repeated the H1.
- `src/routes/chat/+page.svelte`: auto-run link goes to `/settings?s=planner#auto-run`.
- `DESIGN.md` D7 and icon list, `e2e/linux/e2e.py` settings steps, tests.

## Decisions

- Group labels stay sentence case: DESIGN.md section 3 forbids wide-tracked uppercase labels.
- No OPTIONAL/BETA badges or onboarding checklist from the reference: nothing here is beta or optional.

## Verification

- Run on a clean copy of HEAD plus only these changes (another agent had unfinished work in the tree):
  svelte-check 0 errors, vitest 222/222 passed, vite build succeeded.

## Limitations

- Not run in the real window (repo rule: no smoke test without approval). Linux E2E not rerun.

## Follow-up

- none
