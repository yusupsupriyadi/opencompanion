# Chat follow-ups for a session that already runs (FR-25)

## Summary

The planner can now answer with a follow-up card for an existing session instead of a new one. Send types the message into the session's terminal and presses Enter, or starts the next headless turn; nothing new starts. The same confirmation applies as for any card.

## Changes

- `src-tauri/src/orchestrator.rs`: `follow_ups` in the plan schema (required, empty when unused); the sessions list gives each id and "takes a message: yes/no"; `session_problem` and `validate_target`; follow-ups for ids the planner was not shown are dropped. Tests.
- `src-tauri/src/db.rs`: `DispatchCard.target`.
- `src-tauri/src/actions.rs`: Run on a follow-up sends to its session; a follow-up cannot become a Board card.
- `src-tauri/src/session.rs`: `send_message` (terminal: text, then Enter on its own; headless: next turn), now also behind the phone's input endpoint.
- `src-tauri/src/lib.rs`: editing a follow-up changes only its message.
- `src/lib/DispatchCard.svelte`, `PhoneDispatchCard.svelte`: "Follow-up for …", "Send to session", message-only edit, no Add to board, "Sent". Tests.
- `tests/companion.rs`: a follow-up run from the phone continues a finished headless session and starts no new one.
- PRD FR-25, DESIGN dispatch card, README.

## Verification

- `cargo test`: 81 unit + 14 integration passed; clippy clean. Vitest: DispatchCard 10 passed; `bun run check`: 0/0.

## Limitations

- Not tried with a real planner turn (no smoke test requested); the schema change keeps Codex's strict output valid because every property is required.

## Follow-up

- none
