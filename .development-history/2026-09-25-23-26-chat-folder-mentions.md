# @ folder mentions in Chat

## Summary

Owner asked whether Chat can take an "@ folder path". Typing `@` in the composer now lists the known project folders; picking one writes its full path into the message, and the planner uses that folder for its cards (PRD FR-24a).

## Changes

- `src/lib/FolderMention.svelte` (new): listbox above the composer, filter by name then path, Up/Down, Enter/Tab, Esc, click, "Browse for a folder…" via the folder dialog; paths with spaces quoted.
- `src/routes/chat/+page.svelte`: mention wired into the textarea keydown, placeholder mentions `@`; `src/routes/chat/mention.test.ts` (new, 4 tests).
- `src-tauri/src/orchestrator.rs`: planner rule for `@`, `mentioned_dirs` (existing absolute folders only, `me@host` ignored) + test. `lib.rs`: mentioned folders join the planner's read-only folders when reading is on.
- `docs/PRD.md` FR-24a, `DESIGN.md` Folder mention + placeholder, `README.md`.

## Decisions

- Full path, not the bare name: exact even when two projects share a name.
- FR-24a rather than a new number: section C (FR-20..29) is full and this extends FR-24 (context).
- Focus stays in the textarea (`aria-activedescendant`), so typing and Enter-to-send keep working.

## Verification

- `cargo test`: 64 unit + 12 integration passed. `cargo clippy --all-targets`: 0 warnings.
- `bun run check`: 0/0. `bun run test`: 89 passed. `bun run build`: ok.

## Limitations

- No real planner turn with an `@` path yet; not clicked through in the running app (no smoke test requested).
- The API planner (custom provider) gets the path in the text but no file access, as before.

## Follow-up

- none
