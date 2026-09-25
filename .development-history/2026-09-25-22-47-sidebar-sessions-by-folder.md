# Sidebar sessions grouped by folder

## Summary

Owner asked for the sessions list to be grouped by folder. The sidebar "Sessions" list now shows a folder heading with its sessions underneath, the folder holding the most urgent session first.

## Changes

- `src/lib/Sidebar.svelte`: `groups` derived from the same six sessions (waiting, live, then recent finished), grouped by normalized `cwd`; folder heading (`folder-simple` 14 + mono name, full path as tooltip) labels a `role="group"`; item line is now "CLI · status".
- `src/app.css`: `.folder-group`, `.folder-head`; gap between folders 10.
- `src/lib/Sidebar.test.ts`: grouping test (order, case/trailing-slash merge, same name in another path kept apart).
- `DESIGN.md`: Sidebar component spec.

## Decisions

- Kept the six-session cap; grouping happens after selection so urgency order still decides what is shown.
- Folders compare case-insensitively with slashes normalized (Windows paths).

## Verification

- `bun run test`: 52 passed. `bun run check`: 0 errors, 0 warnings. `bun run build`: ok.

## Limitations

- Not viewed in the running app (no smoke test requested). Overview and phone lists are not grouped.

## Follow-up

- none
