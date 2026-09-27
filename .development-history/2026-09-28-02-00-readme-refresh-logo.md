# README refresh with the app logo

## Summary

The README was written before the logo, the Terminal screen, Mark done, PowerShell sessions, image paste and the Settings tabs landed. It now shows the app logo at the top and describes version 0.1.1 as the code stands.

## Changes

- `README.md`: centered header with `design/logo.svg`; version 0.1.1; features for Mark done, Shift+Enter and image paste, the Terminal screen, the folder-grouped sidebar and the Day/Dusk theme; CLIs and Skills now under Settings; PowerShell note for interactive sessions on Windows; privacy lines for pasted images and Terminal shells; retention wording fixed to the real options (Forever, 90, 30, 7, 1 day); layout rows for `terminal.rs`, `paste.rs` and `design/`; status and known limits updated.

## Decisions

- The icon is the existing logo (`design/logo.svg`), not a new asset. No emojis in headings.

## Verification

- Every relative link and the logo path exist; both in-page anchors match headings; no em or en dashes.
- Facts checked against the code: i18n labels, `KEEP = [0, 90, 30, 7, 1]`, `sessions(60)` in `companion.rs`, `opencompanion-paste` in `lib.rs`, shell list in `terminal.rs`.

## Limitations

- Not previewed on GitHub. `CHANGELOG.md` still says Unreleased, planned as 0.1.0, while the app is 0.1.1.

## Follow-up

- Replace the screenshot placeholder; bring `CHANGELOG.md` up to 0.1.1.
