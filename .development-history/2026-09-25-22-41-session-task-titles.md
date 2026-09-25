# Sessions named by their task

## Summary

Owner asked for session titles to show the task, not the folder. The sidebar and the Chat live-sessions rail now lead with the session title, and titles come from a real task name where one exists.

## Changes

- `src-tauri/src/orchestrator.rs`, `db.rs`: dispatch cards get a `title` (at most six words, user's language) in the planner schema and prompt; old cards default to empty.
- `src-tauri/src/session.rs`: `StartRequest.title`; interactive Claude takes the task name Claude Code writes in the terminal title (OSC 0 behind a spinner glyph); a session started empty takes its first typed prompt from the `UserPromptSubmit` hook.
- `src-tauri/src/lib.rs`: Chat and Board runs pass the card title; card edit and Add to board keep it.
- `src/lib/Sidebar.svelte`, `src/routes/chat/+page.svelte`, `src/lib/DispatchCard.svelte`, `api.ts`: title first, folder second; card title shown and editable.
- `src-tauri/src/bin/fake-cli.rs`, tests, `docs/PRD.md` FR-21.

## Decisions

- A card name (Chat or Board) is never replaced; only titles derived from the prompt or folder are upgraded.

## Verification

- `cargo test`: 46 unit + 10 manager + 2 companion passed, incl. new terminal-title and naming tests through a real ConPTY. `cargo clippy --all-targets`: 0 warnings.
- `bun run check`: 0/0. `bun run test`: 51 passed.

## Limitations

- Codex and OpenCode interactive sessions started without a prompt keep "CLI in folder": they have no hooks and no verified terminal title.
- Existing sessions keep their old titles until resumed (Claude interactive) or never (headless).

## Follow-up

- none
