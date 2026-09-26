# Cards that start without Run in folders the owner picks (FR-26)

## Summary

Settings › Chat planner gets "Run cards without asking". A valid new-session card for one of those folders (or a folder inside it) starts as soon as the planner answers. Guardrails: adding a folder takes a second, explicit press; the session uses the default permission mode but never Bypass (it falls back to Ask me); follow-ups and cards with a problem still wait; and Chat says plainly which folders run by themselves.

## Changes

- `src-tauri/src/db.rs`: `auto_run_folders`, `Settings::auto_runs`, `DispatchCard.auto`. `projects.rs`: `contains` (also used by the notification rules).
- `src-tauri/src/actions.rs`: `auto_run` after each planner turn; `chat_send` now takes the manager (db, data folder and the "asked" signal come from it).
- `src-tauri/src/companion.rs`: the phone's chat list and chat carry `autoRun`.
- `src/routes/settings/+page.svelte`: folder list, two-step add, remove; distinct names for the two Add and Browse buttons.
- `src/routes/chat/+page.svelte`, `src/routes/m/chat/thread/+page.svelte`: the composer note; the phone no longer says "Nothing starts until you press Run" when that is not true.
- `DispatchCard.svelte`, `PhoneDispatchCard.svelte`: "Started by itself: …".
- Tests: `tests/manager.rs` (auto folder starts with Ask me instead of Bypass, others wait), Settings, Chat, DispatchCard.
- PRD FR-26, DESIGN D7, README.

## Verification

- `cargo test`: 83 unit + 15 integration passed; clippy clean. Vitest: 133 passed; `bun run check`: 0/0.

## Limitations

- Not tried with a real planner turn (no smoke test requested).

## Follow-up

- none
