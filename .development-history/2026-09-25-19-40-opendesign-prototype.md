# Clickable UI prototype in OpenDesign

## Summary

Built the full AI Remote UI as a clickable HTML prototype in the OpenDesign project "AI Remote", following DESIGN.md: desktop screens D1-D9 (Dusk via a working theme toggle) and the phone companion M1-M4.

## Changes

- OpenDesign `styles/app.css`: light and dark tokens, components, reflow at 1280/1100/720 px.
- OpenDesign `scripts/app.js`, `scripts/data.js`: shared sidebar, theme toggle, horizon motif, dialogs, toasts, sample sessions.
- OpenDesign pages: `index`, `session`, `chat`, `clis`, `settings`, `onboarding`, `states`, `phone`, `mobile` (.html); `assets/meadow-day.webp`.
- `DESIGN.md` section 14: where the prototype lives.

## Decisions

- Wrote the HTML by hand instead of commissioning an OpenDesign generation run, to keep it faithful to DESIGN.md.
- One data-driven `session.html` covers running, waiting, done, stopped, read-only and not-found states.

## Verification

- jsdom click-through of every control on all 9 pages: 63/63 checks pass, no script errors.
- `node --check` on all scripts; local links and dialog targets resolve; no em or en dashes.
- New color pairs checked with `contrast-check.py`: all pass AA (lowest 5.22:1).

## Limitations

- No visual render check: BrowserOS Neo was not connected and other browsers are not allowed here.

## Follow-up

- Visual review in OpenDesign, then start the M0 spike from the PRD.
