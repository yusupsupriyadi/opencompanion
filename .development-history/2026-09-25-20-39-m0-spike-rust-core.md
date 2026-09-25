# M0 spike: Tauri 2 scaffold and Rust core

## Summary

Scaffolded the Tauri 2 + SvelteKit (Svelte 5, Bun) app and built the M0 Rust core: CLI detection, ConPTY sessions, headless runners with a normalized event parser, external process scan, and text-pattern prompt detection. Ran the spike against Claude Code, Codex CLI and OpenCode; results are in `docs/spike/M0-results.md`.

## Changes

- `src-tauri/src/{cli,proc,pty,headless,events,monitor,waiting}.rs`: Rust core modules with 16 unit tests built from captured output.
- `src-tauri/src/bin/air-spike.rs`: terminal runner (`detect`, `scan`, `pty`, `headless`, `replay`, `waiting`).
- `src-tauri/src/lib.rs`: Tauri commands `detect_clis` and `scan_external`.
- `src/`: M0 page (CLIs table, outside sessions list, loading/empty/error states), DESIGN.md tokens, meadow background with haze, local fonts, CLI logos.
- `docs/spike/M0-results.md`, `docs/PRD.md` section 6 notes, `DESIGN.md` contrast row for `st-err` text.

## Decisions

- Bun is the package manager (owner request); Tauri hooks use `bun run`.
- Claude Code headless answers permissions through `--permission-prompt-tool stdio` control requests; interactive sessions use injected `PermissionRequest`/`Notification` hooks.
- Rust core answers ConPTY's `ESC[6n` cursor query; without it every CLI stalls after 4 bytes.
- Claude Code session marker env vars are stripped from every spawned CLI.

## Verification

- `cargo test --lib`: 16 passed. `cargo clippy --all-targets`: 0 warnings. `cargo build`: ok.
- `bun run check`: 0 errors, 0 warnings. `bun run build`: ok.
- Spike runs: see results doc (detect, scan, PTY x3, headless x3 plus two permission runs).

- First run by the owner: buttons stuck on "Checking…"/"Scanning…". Root cause: a Svelte `$effect` tracked the in-flight flags and restarted both loads forever; moved to `onMount`. The scan also listed the app's own `opencode --version` probe; `scan_external` now excludes descendants of the app process (regression test `own_descendants_are_not_outside_sessions`). After the fix no child processes were spawned over 4 s.

## Limitations

- App window not run (no smoke test without approval); UI verified by svelte-check, build and code inspection.
- Codex success path untested (401 from the machine's custom provider); OpenCode needs `--pure` because a user plugin crashes its server.

## Follow-up

- M1: session manager on top of `pty` and `headless`, OpenCode via `opencode serve`, port the prototype shell.
