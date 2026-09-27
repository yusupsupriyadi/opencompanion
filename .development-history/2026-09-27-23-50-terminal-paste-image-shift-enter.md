# Paste images and Shift+Enter in terminals

## Summary

Images could not be pasted into session or Terminal-screen terminals, and Shift+Enter sent the prompt instead of adding a line. Root causes: xterm sends Ctrl+V as `\x16`, which Claude Code on Windows ignores (its image paste is Alt+V); xterm's paste reads `text/plain` only, so a screenshot pastes nothing; xterm sends Shift+Enter as a plain `\r`.

## Changes

- `src/lib/Terminal.svelte`: on Windows Ctrl+V is the browser paste in sessions too (as in Windows Terminal); a paste holding only an image is saved and its path pasted; Shift+Enter sends ESC CR in sessions and a win32-input-mode Shift+Enter in terminals once ConPTY sends `ESC[?9001h`.
- `src/lib/term-input.ts`: `pastedImage`, `pathForPaste`, `shiftEnter`. `src/lib/api.ts`: `savePastedImage` (raw IPC body).
- `src-tauri/src/paste.rs`, `src-tauri/src/lib.rs`: `save_pasted_image` writes PNG/JPEG/GIF/WebP to `%TEMP%\opencompanion-paste`.
- `src/lib/i18n/sessions.ts`, `src/lib/i18n/terminal.ts`, `DESIGN.md`: terminal notes name Shift+Enter and image paste.

## Decisions

- Image as a pasted file path: Claude Code and Codex attach a pasted image path as the image, and it works for a CLI started inside the Terminal screen too.
- Probed on a real ConPTY: a Node program receives ESC CR as `"\u001b\r"` (Claude's own terminal setup binds Shift+Enter to it) but loses Shift from a win32 Shift+Enter, while PowerShell needs the win32 key to run AddLine. Hence one sequence per kind.

## Verification

- Vitest: 35 files, 197 tests passed (8 new; 4 of them fail against the old component). `bun run check`: 0 errors.
- `cargo test` (from `C:\Users\yusup\Project\...`): 93 unit (2 new) + 5 companion + 12 manager passed. `cargo clippy --all-targets`: clean.

## Limitations

- Not exercised in the running app (no smoke test requested). Pasted images stay in the temp folder until Windows cleans it. A failed save pastes nothing, without a message.

## Follow-up

- none
