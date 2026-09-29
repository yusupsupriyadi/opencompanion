# Chat proposes automations

## Summary

The Chat planner turns a request that should repeat into an automation card with a cron schedule. Create saves the automation; the card then links to it. The planner also sees the existing automations and the local time.

## Changes

- `src-tauri/src/orchestrator.rs`: `automations` in the planner schema and prompt, the Automations list and Now in the context, `validate_automation`, automation cards parsed from the answer.
- `src-tauri/src/actions.rs`: auto-run skips automation cards, Edit keeps and checks the schedule, Create saves the automation (default permission mode from Settings, like Run).
- `src-tauri/src/db.rs`: `DispatchCard.schedule` and `automation_id`, state `created`.
- `src/lib/DispatchCard.svelte`, `PhoneDispatchCard.svelte`: schedule line, Create automation, Schedule in Edit, Open automation.
- `README.md`, `DESIGN.md`: Chat bullet, Dispatch card variant, decision.

## Decisions

- Nothing is scheduled without Create, also in folders whose session cards run without Run.

## Verification

- `cargo clippy --all-targets -- -D warnings`: clean; `cargo test` (separate target dir): 153 lib, 5 companion, 14 manager passed.
- Clean HEAD worktree with these changes: `vitest` 302 passed, `svelte-check` 0 errors.

## Limitations

- Not tried against a real planner CLI; the prompt and schema are covered by unit tests only.

## Follow-up

- none
