# Automations smoke test

## Summary

Ran a debug build with its own identifier (`dev.opencompanion.smoke`, own data folder and single-instance key) next to the owner's dev app, and drove it through Windows UI Automation and its phone API. Two small fixes came out of it.

## Changes

- `src/lib/schedule.ts`: English month names from `en-US` ("Sep", as the backend titles sessions) instead of `en-GB` ("Sept").
- `src/routes/automations/+page.svelte`: the Settings link in the start-at-sign-in note is underlined.

## Verification

- API: 401 without token; previews for weekdays, an invalid field and 30 February; save refuses no name, a missing folder, a headless prompt left empty.
- UI: Automations nav, list row, Run now (real Claude Code headless run in Plan mode answered "OK"; title "Smoke reply · 30 Sep 02:14", source `automation`).
- Missed run: next run set three days back with the app closed; one Late run on the first tick, next run back to 09:00, no second run on the next tick.
- Chat: the real planner answered an Indonesian request with an automation card `0 8 * * 1-5` and named the existing automation; Create saved it, started nothing, and Open automation opened it.
- Pause from the list switch and Delete through the dialog, both checked against the API.

## Limitations

- Phone screens not seen in a browser (BrowserOS Neo was not connected); the phone API was.
- Skipped runs and start-failure notifications were covered by unit tests only.

## Follow-up

- none
