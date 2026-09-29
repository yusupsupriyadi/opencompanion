# Type into a closed session terminal to start it again

## Summary

The owner asked to drop the Open terminal button of a closed session terminal, which read like New terminal and Split, and reported that nothing could be typed there. A closed terminal now has no button: typing into it starts it again (the same resume as before) at the terminal's size, and the keys after that reach the CLI.

## Changes

- `src/lib/Terminal.svelte`: `onwake(cols, rows)` fires once on a typed key or paste while closed (cursor keys and Ctrl+C do not), and the process gets the terminal's size when it starts.
- `src/routes/session/+page.svelte`: the button and its row are gone; `reopen` resumes the session, keeps it live until its status next changes, and focuses the terminal.
- `src/lib/i18n/sessions.ts`: new closed note and "Starting {cli} again…"; the Open terminal strings are removed.
- Tests: `src/routes/session/session.test.ts` (typing wakes it, then keys go through), `src/lib/Terminal.test.ts` (what wakes and what does not). `DESIGN.md` D4, `README.md`.

## Decisions

- The key that wakes the terminal is not sent, since the CLI is not there yet to read it.

## Verification

- `bun run check`: 0 errors, 0 warnings. `bun run build`: ok. `bunx vitest run`: 46 files, 296 tests passed.

## Limitations

- Not run in the app by me. `api.resumeSession` and the backend command stay; the phone keeps its own Resume.

## Follow-up

- none
