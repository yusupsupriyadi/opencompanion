# Desktop MVP, Board and phone companion

## Summary

Built AI Remote end to end on top of the M0 core: session manager (PTY and headless), chat planner, Board, SQLite history, notifications, and a LAN phone companion, with the prototype's screens ported to Svelte 5.

## Changes

- `src-tauri/src/{db,session,orchestrator,companion}.rs`, `lib.rs`: store, session lifecycle, planner, HTTP/WebSocket companion, 32 Tauri commands.
- `src-tauri/src/bin/fake-cli.rs`, `tests/{manager,companion}.rs`: stand-in CLI and integration tests.
- `src/lib/*`, `src/routes/*`: shell, Overview, Session (xterm), Chat, Board, CLIs, Settings, Onboarding, phone routes under `/m`; Vitest component tests.
- `README.md`, `DESIGN.md` contrast rows.

## Decisions

- Planner runs Claude Code with `--restricted --tools "" --json-schema` (about 0.035 USD equivalent per turn versus 0.33 with full context).
- Interactive Claude sessions get hooks through a per-session `--settings` file; headless Claude answers through the stdio control protocol.
- Headless follow-ups start a new turn with the CLI's own resume id.
- Companion off by default; one-time 6-digit codes, 5 attempts, SHA-256 token hashes.

## Verification

- `cargo test`: 41 passed (32 unit, 7 manager, 2 companion over real HTTP). `cargo clippy --all-targets`: 0 warnings.
- `bun run check`: 0 errors, 0 warnings. `bun run test`: 32 passed. `bun run build`: ok.
- One real planner turn: 2 valid cards in 10.4 s.

## Limitations

- Desktop window not clicked through by the agent (no smoke test approval); owner runs `tauri dev`.
- Notification click, FR-25, FR-32, FR-34 CPU, FR-41, FR-42, FR-63, FR-77 not built.

## Follow-up

- Owner click-through; then git init and first commit when approved.
