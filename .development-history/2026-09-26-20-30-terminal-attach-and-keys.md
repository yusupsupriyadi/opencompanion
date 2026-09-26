# Terminal: no output lost on attach, no keyboard trap, events kept during load

## Summary

Code review found three problems in the session detail. Output emitted between the page's snapshot and the terminal's listener was lost (a TUI could show a broken screen until it redrew). xterm took Tab, so a keyboard user could not leave a live terminal, and a closed one still trapped Tab. Events that arrived while the page loaded were overwritten by the stored list.

## Changes

- `src-tauri/src/session.rs`: every output chunk gets a number (`output_seq`, changed under the output lock); `output_snapshot` returns the text with its last chunk number. `Emit::output` carries the number.
- `src-tauri/src/lib.rs`: `session_output` command; `get_session` no longer reads the terminal buffer; `session-output` events carry `seq`.
- `src/lib/Terminal.svelte`: listens first, reads the snapshot second, writes only newer chunks; Ctrl+Tab and Ctrl+Shift+Tab leave a live terminal, a closed one lets Tab through; `screenReaderMode` on, as DESIGN.md section 12 says.
- `src/routes/session/+page.svelte`: stored and early events merged by id; a late failure for a previous session no longer replaces the current one; the note names Ctrl+Tab. Test.
- `tests/manager.rs`: chunk numbers count from 1 and the snapshot ends at the last one. DESIGN section 12.

## Verification

- `cargo test`: 77 unit + 14 integration passed; clippy clean. Vitest: 118 passed; `bun run check`: 0/0.

## Limitations

- The terminal itself is not covered by component tests (xterm needs canvas, which jsdom lacks).

## Follow-up

- none
