# Phone companion: New session, messages, Chat and Board

## Summary

The phone could only list, open, approve and stop sessions. It now also starts sessions, sends messages to them (follow-ups, terminal text and keys, Resume), plans in Chat with Run / Add to board / Discard, and works the Board, behind a bottom tab bar (Sessions, Chat, Board).

## Changes

- `src-tauri/src/actions.rs`: Chat turn, card and Board logic moved out of `lib.rs`, shared by desktop commands and the phone API.
- `src-tauri/src/companion.rs`: `/api/options`, `POST /api/sessions`, `/input`, `/resume`, `/api/chat*`, `/api/tasks*`; all need a device token.
- `src-tauri/src/lib.rs`, `session.rs`: commands call `actions`; Board and chat changes reach the phone over the WebSocket (`tasks`, `chat`).
- `src/routes/m/*`, `src/lib/PhoneDispatchCard.svelte`, `phone.css`, `phone.svelte.ts`: tab bar, New session (`/m/new`, `?task=` runs a card), Chat list and thread, Board, session composer.
- `DESIGN.md` section 10 + decision, `docs/PRD.md` FR-59 and FR-77, `README.md`.

## Decisions

- Owner chose all four features and a bottom tab bar (2026-09-25).
- Terminal text is written, then Enter 150 ms later, so a TUI does not take it as a paste.
- New session on the phone defaults to Headless; card editing and new Board cards stay on the desktop.

## Verification

- `cargo test`: all pass (companion 3, incl. new end-to-end phone test). `cargo clippy --all-targets`: 0 warnings.
- `bun run test`: 104 passed (16 phone). `bun run check`: 0 errors. `bun run build`: ok.

## Limitations

- No smoke test on a real phone (not requested). The desktop Chat screen does not yet listen to `chat-changed`, so it shows phone turns after reopening.

## Follow-up

- Owner click-through on a phone; desktop Chat listener once the Chat screen work in progress lands.
