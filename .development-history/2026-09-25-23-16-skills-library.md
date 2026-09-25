# Skills library

## Summary

New Skills screen that compares the user skill folders of each CLI by content, so a missing or different skill shows up at a glance, with a copy command the owner runs by hand. Read-only.

## Changes

- `src-tauri/src/skills.rs`: scans `~/.claude/skills`, `~/.codex/skills`, `~/.agents/skills`, `~/.config/opencode/skills`, `~/.gemini/skills`; frontmatter description, SHA-256 per folder, variant letters (A = newest), problems, link target, links inside.
- `src-tauri/src/lib.rs`: `scan_skills` command.
- `src/lib/api.ts`, `src/lib/skills.ts`: types, cell states, filters, copy commands (PowerShell and sh).
- `src/routes/skills/+page.svelte`: matrix, filter, search, detail dialog; `src/lib/Sidebar.svelte`: Skills nav item.
- `DESIGN.md` (D11, icons, decisions), `README.md`, `docs/superpowers/specs/2026-09-25-skills-library-design.md`.

## Decisions

- Columns are folders, not CLIs: which CLI reads which folder depends on its version.
- No copy or replace command into a folder that is, or holds, a link: Windows PowerShell 5.1 `Remove-Item -Recurse` follows junctions.

## Verification

- `cargo test`: 63 unit + 12 integration passed; `cargo clippy --all-targets`: clean.
- `bun run check`: 0 errors; `bun run test`: 15 files, 80 tests passed.

## Limitations

- Not run in the real app (no smoke test without approval). Env overrides such as `CODEX_HOME` are not followed. Plugin and project skill folders are out of scope.

## Follow-up

- Smoke test on the real skill folders when the owner approves.
