# Sidebar folder groups: clearer heading and collapse

## Summary

Owner asked to improve the folder-grouped sidebar list and make groups collapsible. Each folder heading is now a toggle button; a collapsed folder keeps one line that still shows a waiting session and the session count.

## Changes

- `src/lib/Sidebar.svelte`: folder heading is a `button` with `aria-expanded`/`aria-controls`, `caret-down` + `folder-simple` + mono name; collapsed shows `hand-palm` (waiting) and count with screen-reader text; `holds-current` fill when the open session is inside; collapsed keys saved in `localStorage` (`air-collapsed-folders`, try/catch like the theme).
- `src/app.css`: heading hover and 30 px height, caret rotation, rows indented 12 so the status box sits under the folder icon and titles under the folder name; `.folder-items[hidden]`.
- `src/lib/Sidebar.test.ts`: collapse test (hidden rows, accessible name, persists across mounts).
- `DESIGN.md`: Sidebar spec.

## Verification

- `bun run test`: 53 passed. `bun run check`: 0 errors, 0 warnings. `bun run build`: ok.
- Contrast (antislop-human checker): new pairs 5.43:1 or higher in Day and Dusk.

## Limitations

- Not viewed in the running app (no smoke test requested).

## Follow-up

- none
