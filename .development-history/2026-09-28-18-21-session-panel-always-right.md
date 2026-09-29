# Session panel: always on the right, hide and show at any width; Activity row removed

## Summary

The owner asked for the Files, Changes, Branch and Details panel of a session to stay on the right at every window size, and to be hidden and shown. Below 1280 it used to move under the terminal behind a "Files and details" button. It is now the right column at every width, with a `sidebar-simple` icon button at the right end of the tab row above the terminal, the same control as Chat's Live rail. The Activity row above the terminal (pixel label, Horizon, last event) is removed, work another session did in this worktree at the owner's request.

## Changes

- `src/routes/session/+page.svelte`: `sideOpen` comes from a remembered hidden flag (`oc-session-side-hidden`) on wide windows and from a temporary peek below 720. The icon button (`#btn-side-panel`, `aria-expanded`, tooltip Hide/Show) sits in `.views-bar` after the tab strip, outside the strip's own scroller. Hidden, the panel stops the 5 s git status read unless the viewer is open. The scroll to the viewer after opening a file is gone, since the viewer is always in sight. Below 720, Escape inside the panel (not in a dialog or a search that used it) or a file opened from it closes the panel and returns focus to the button. The Activity row, its Horizon marks and its styles are gone.
- `src/app.css`: `.detail-body.ws` keeps `minmax(0, 1fr) 320px` at every width, `.solo` gives the terminal the whole row, and the 1279 rule now applies only to the outside page. Below 720 the panel covers the terminal's right edge from under the tab row, on `glass-pop` with `shadow-modal`. `.activity` left the plate and glass selectors.
- `src/lib/i18n/workspace.ts`: `workspace.toggle` (the button's name), `workspace.show` now reads "Show files and details" (en and id). `src/lib/i18n/sessions.ts`: `sessions.detail.activityEmpty` removed; `sessions.detail.activity` stays for the phone.
- `src/routes/session/workspace.test.ts`: hide, show, the remembered choice and the stopped git read; the narrow peek (Escape, opened file, storage left alone). `src/routes/session/session.test.ts`: no Activity row, and the toggle ends the tab row outside the scroller.
- `DESIGN.md`: window minimum note, `sidebar-simple` usage, Horizon usage, D4 (Activity bullet removed, body).

## Decisions

- The text button became an icon button to match Chat's Live rail toggle, which already hides and shows a right column. With the session header gone on main (ed52792), it ends the tab row, where it sits right above the panel it controls.
- Below 720 the panel opens over the terminal instead of squeezing it: the desktop window cannot get that narrow (minimum 1100), so this only keeps the page usable if it ever does. It starts under the tab row, since covering the row hid the button that closes it.

## Verification

- After rebasing on main: `bunx vitest run` 38 files, 215 tests passed; `bun run check` 0 errors, 0 warnings; `bun run build` ok.
- In a browser with a mocked backend (Vite dev plus a stand-in for `__TAURI_INTERNALS__`): panel on the right at 1536, 1279, 1200, 1101 and 1100 with no horizontal scroll; hidden, the terminal runs to the right edge and the button follows it; the hidden choice holds across a reload; at 700 the panel starts closed, opens under the tab row with the button still clickable, closes with Escape (focus back on the button) and with a file opened from the tree; dark and light themes.

## Limitations

- Not run in the Tauri window itself. Between 1101 and 1279 the full sidebar and the panel leave the terminal about 405 to 583 px wide; hiding the panel gives it the rest.

## Follow-up

- none
