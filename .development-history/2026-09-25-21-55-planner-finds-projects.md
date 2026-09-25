# Chat planner finds projects and reads them

## Summary

The planner kept asking for a folder path ("ai-remote" was unknown on a fresh database) and kept asking after the owner confirmed. It now knows the owner's project folders, may read them (read-only), and proposes a card instead of asking.

## Changes

- `src-tauri/src/projects.rs`: discovery from AI Remote history, open CLI sessions, Claude Code's `~/.claude/projects` (decoded against the real filesystem), and project roots such as `~/Project`; markers per folder; name-based folder resolution.
- `src-tauri/src/orchestrator.rs`: new rules (match names, no guessed paths, stop asking after a confirmation, state assumptions); read-only access (Claude `--restricted --tools Read,Glob,Grep --add-dir`, Codex `-s read-only`, OpenCode edit/bash denied); card folders resolved before validation.
- `src-tauri/src/db.rs`, `lib.rs`: `plannerCanRead`, `projectRoots` settings; `project_folders`, `default_project_roots` commands.
- `src/routes/settings`, `src/lib/FolderField.svelte`, Chat copy: planner read toggle, Project folders card, discovered folder suggestions.

## Decisions

- Read access stays read-only so the PRD rule "chat never acts on its own" holds; the owner can turn it off.
- Temp and AppData folders are never offered as projects.

## Verification

- `cargo test`: 47 passed; `cargo clippy --all-targets`: 0 warnings; `bun run check`: 0/0; `bun run test`: 32 passed.
- Real planner, owner's exact message: card for `C:\Users\yusup\Project\ai-remote` in 20.9 s without asking; after "iya itu": card in 17.2 s.

## Limitations

- A turn with file reads takes about 15 to 25 s instead of about 10 s.

## Follow-up

- none
