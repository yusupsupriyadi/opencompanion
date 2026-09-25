# Rename the app to OpenCompanion

## Summary

The owner renamed the app from AI Remote to OpenCompanion and asked for it to be pushed to a new private GitHub repo. Brand text, package names, crate names, the bundle identifier and the database file name now use the new name.

## Changes

- `package.json`, `bun.lock`: package `opencompanion`.
- `src-tauri/Cargo.toml`, `Cargo.lock`: crate and default binary `opencompanion`, library `opencompanion_lib` (imports updated in `main.rs`, `bin/air-spike.rs`, `tests/`).
- `src-tauri/tauri.conf.json`: product name and window title `OpenCompanion`, identifier `dev.opencompanion.app`.
- `src-tauri/src/lib.rs`: database file `opencompanion.db`.
- `src/**`, `src-tauri/src/**`, `src-tauri/tests/**`: every "AI Remote" string (UI copy, page titles, planner prompt, comments, tests).
- `src/app.css`: sidebar wordmark 20px to 18px, same as onboarding and the phone header.
- `README.md`, `DESIGN.md`, `docs/PRD.md`, `docs/spike/M0-results.md`, skills spec: new name; PRD diagram border realigned, open question 4 now asks only for the logo.

## Decisions

- Display name `OpenCompanion` (camel case, like OpenCode); lowercase `opencompanion` for package, crate and repo names.
- Wordmark 18px: measured from the Nunito 800 font file, "OpenCompanion" at 20px plus the icon needs 186.5px of the 188px available, so a sidebar scrollbar would push it past the edge. At 18px it needs 170.7px.
- Kept as is: `.development-history/`, `design/ai-remote.pen`, the OpenDesign project name "AI Remote", `ai-remote` sample folder names in tests and examples, and internal `air-` prefixes (localStorage keys, temp dirs, `air-spike`).

## Verification

- `bun run check`: 0 errors, 0 warnings. `bun run test`: 89 passed.
- `cargo clippy --all-targets`: no warnings. `cargo test` (separate target dir, a running `tauri dev` held the exe): 64 unit passed (1 ignored), 12 integration passed.

## Limitations

- The new identifier means a new app data folder: sessions, settings and paired phones in `%APPDATA%\dev.airemote.app` are not carried over (copy that folder's contents and rename `ai-remote.db` to `opencompanion.db` to keep them). Theme, collapsed folders and window position start fresh, and phones must pair again. Claude Code hook settings are written per session, so they follow the new folder.
- The local project folder is still named `ai-remote`.

## Follow-up

- Final logo (PRD open question 4).
