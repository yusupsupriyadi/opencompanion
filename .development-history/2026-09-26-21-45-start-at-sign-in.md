# Start in the tray at Windows sign-in (FR-64)

## Summary

Settings › "Window and sign-in" can start OpenCompanion when the user signs in to Windows. The start carries `--hidden`, so the app opens straight into the tray. No new dependency: the `Run` value is written and removed with `reg.exe`.

## Changes

- `src-tauri/src/autostart.rs` (new): `supported`, `enabled`, `set`; the value is `"<exe>" --hidden` under `HKCU\...\CurrentVersion\Run`. Test against a throwaway key under `HKCU\Software`, removed at the end (checked: no key left).
- `src-tauri/src/lib.rs`: `get_settings` reads the real state from Windows; `save_settings` asks Windows first, so a refused change is not stored; `AppInfo.can_start_at_login`; a `--hidden` start hides the window in setup.
- `src-tauri/src/db.rs`: `start_at_login`.
- `src/routes/settings/+page.svelte`: the checkbox, shown only where the platform has it. `api.ts`. Test.
- PRD FR-64, DESIGN D7, README.

## Decisions

- The window is hidden in setup rather than created hidden, so a failure can never leave the app without a window; the cost is a brief flash at sign-in.

## Verification

- `cargo test`: 82 unit + 14 integration passed; clippy clean. Vitest settings: 12 passed; `bun run check`: 0/0.

## Limitations

- Windows only. Not tried with a real sign-in (no smoke test requested).

## Follow-up

- none
