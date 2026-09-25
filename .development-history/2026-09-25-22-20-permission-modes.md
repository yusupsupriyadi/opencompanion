# Permission modes: Ask me, Plan, Auto, Bypass

## Summary

Owner asked for plan, auto and bypass modes in Settings. Sessions AI Remote starts now take a permission mode: a default in Settings, overridable per session in New session and Board runs (PRD FR-65).

## Changes

- `src-tauri/src/headless.rs`: `PermMode` and `mode_flags` per CLI (Claude `--permission-mode manual|plan|auto`, `--dangerously-skip-permissions`; Codex sandbox/`-a on-request`, `--approve-for-me`, `--dangerously-bypass-approvals-and-sandbox`, bypass only on `exec resume`; OpenCode `--agent plan`, `--auto`, `OPENCODE_PERMISSION` for bypass); env support for headless and PTY.
- `src-tauri/src/session.rs`, `db.rs`, `lib.rs`: mode from request or Settings, stored on the session, passed on Board runs.
- `src/routes/settings`, `src/lib/SessionForm.svelte`, `format.ts`, session detail: mode cards with a flag table, Bypass needs a second press, per-session select with a Bypass warning, mode label on sessions.
- `docs/PRD.md` FR-65, `README.md`.

## Decisions

- Default stays "Ask me" so prompts keep reaching Needs you.
- The chat planner ignores this setting and stays read-only.

## Verification

- `cargo test`: 49 passed, including an integration test where fake-cli echoes the flags and env it received. `cargo clippy --all-targets`: 0 warnings.
- `bun run check`: 0/0. `bun run test`: 36 passed (Bypass confirmation, Keep, per-session select).
- Real CLIs in a PTY: Claude showed "plan mode on", "auto mode on", "bypass permissions on"; OpenCode showed the Plan agent and "Build auto"; Codex accepted its flags.

## Limitations

- Codex's mode is not visible on screen behind its update offer; flags were accepted without error.
- Gemini CLI gets no mode flags until it can be tested.

## Follow-up

- none
