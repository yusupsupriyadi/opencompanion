# Outside session transcripts (FR-32) and session CPU and memory (FR-34)

## Summary

Rows under "Opened outside OpenCompanion" now open `/outside?pid=`, which shows the last messages the CLI wrote to its own history, read-only. The session detail's Process panel shows CPU, memory and child processes while a session runs.

## Changes

- `src-tauri/src/transcript.rs` (new): newest history for the process folder written since the process started. Claude Code `~/.claude/projects/<encoded>/*.jsonl` (skips meta, sidechain and slash-command wrappers), Codex `~/.codex/sessions/Y/M/D/rollout-*.jsonl` matched by `session_meta.cwd`, OpenCode `opencode*.db` opened `SQLITE_OPEN_READ_ONLY`. Gemini: "Transcript not available for this CLI." 5 tests with temp homes.
- `src-tauri/src/monitor.rs`: `find(pid)` for one process, `Monitor::usage(pid)` over the whole process tree, CPU as a share of the machine (also in the outside scan).
- `src-tauri/src/lib.rs`: `outside_detail`, `session_usage`; `scan_external` moved off the main thread.
- `src/routes/outside/+page.svelte` (new) + test; `src/routes/session/+page.svelte` usage polling + test; shared detail layout moved to `src/app.css`.
- PRD FR-32/FR-34, DESIGN D4, README.

## Decisions

- Formats were checked against real history files on this machine (structure only) before writing the parsers.
- The transcript view follows new lines only when the reader is already at the end.

## Verification

- `cargo test`: 69 unit + 13 integration passed; `cargo clippy --all-targets`: clean.
- `bun run check`: 0/0; targeted Vitest: 9 passed.
- Temporary ignored test against the real Claude Code, Codex and OpenCode histories: lines parsed as expected (removed after).

## Limitations

- Two CLI processes of the same kind in one folder share the newest history file.
- Not clicked through in the running app (no smoke test requested).

## Follow-up

- none
