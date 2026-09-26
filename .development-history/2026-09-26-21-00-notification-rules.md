# Notification choices per CLI and per project folder (FR-41)

## Summary

Settings › Notifications keeps its three switches and adds a "By CLI" table and a "By project folder" table, each with Waiting, Done and Error. A notification goes out only when the switch, the CLI's rule and every folder rule holding the session's folder allow it; the phone follows the same rules because they decide whether the notice is sent at all.

## Changes

- `src-tauri/src/db.rs`: `NotifyRule`, `Notice`, `Settings.notify_clis` / `notify_projects` (serde default, so older settings load), `Settings::notifies` (folder rules cover subfolders, case and slash insensitive). Test.
- `src-tauri/src/session.rs`: status notifications ask `notifies`.
- `src/routes/settings/+page.svelte`: the two tables, add from known project folders or Browse, Remove; a CLI rule that sends everything is dropped. `api.ts` types. Test.
- PRD FR-41, DESIGN D7, README.

## Decisions

- A new folder rule starts with all three on, so adding it changes nothing until a box is cleared.

## Verification

- `cargo test --lib db::`: 10 passed. Vitest: settings 10 passed; `bun run check`: 0/0.

## Limitations

- none

## Follow-up

- none
