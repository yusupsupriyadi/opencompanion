# A click on a Windows notification opens its session (FR-40)

## Summary

FR-40 (P0) says a click on a notification opens the session's detail. The Tauri notification plugin has no click event on desktop, but the Windows toast underneath it (notify-rust over tauri-winrt-notification) reports one. Windows notifications now come straight from notify-rust and a click brings the window forward on that session.

## Changes

- `src-tauri/Cargo.toml`: `notify-rust` for Windows (already in the lock file through the plugin, 4.18.0).
- `src-tauri/src/lib.rs`: `show_notification` (Windows: toast with the same app id rule as the plugin, a thread waits for click or dismissal; elsewhere the plugin as before) and `open_session` (unminimize, show, focus, emit `open-session`).
- `src/lib/Shell.svelte`: `open-session` navigates to the session.
- README known limits.

## Decisions

- One short-lived thread per toast; it ends when the toast is clicked or leaves the screen.

## Verification

- `cargo test`: 81 unit + 14 integration passed; clippy clean. `bun run check`: 0/0.

## Limitations

- Not clicked in the running app (no smoke test requested). A toast clicked later from the notification center only focuses the app. macOS and Linux keep the plugin without a click action.

## Follow-up

- none
