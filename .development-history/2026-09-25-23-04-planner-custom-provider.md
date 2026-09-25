# Chat planner custom provider

## Summary

Owner wanted a custom model provider for the Chat planner. Besides the CLIs, Settings now offers "Custom provider (OpenAI-compatible API)": base URL, model and optional API key. AI Remote calls `POST {base}/chat/completions` itself with the same context a CLI gets (PRD FR-29).

## Changes

- `src-tauri/src/db.rs`: `PlannerSource` (`cli`/`api`), `PlannerApi` with `problem()`, both in `Settings` (serde defaults keep old settings valid).
- `src-tauri/src/orchestrator.rs`: `run_api` over `ureq`, `completions_url`, `answer_text` (drops a `<think>` block), `api_error` (provider's own message); `json_only` shared with OpenCode.
- `src-tauri/src/lib.rs`: `chat_send` uses the provider when picked, with no read dirs. `Cargo.toml`: `ureq = "3"`.
- `src/routes/settings/+page.svelte`: provider option and form (per-field errors, Show/Hide key); folder reading is disabled for a provider.
- `src/routes/chat/+page.svelte`, `src/routes/clis/+page.svelte`, `src/lib/api.ts`: header, Send and planner label follow the provider.
- Tests in `orchestrator.rs`, `settings.test.ts`, `chat.test.ts`; `docs/PRD.md` (FR-29, G5, non-goal, FR-20), `DESIGN.md` D3/D7.

## Decisions

- OpenAI-compatible only: one format covers OpenRouter, Ollama, LM Studio and most hosted APIs. No `response_format`, since LM Studio rejects `json_object`; the schema goes in the system prompt and `extract_plan` parses it.
- No file tools for a provider (folder names only), so the planner still cannot change anything.
- The API key stays in the local settings database (not the OS keychain) and is only sent to the base URL.

## Verification

- `cargo test --lib`: 61 passed (4 new, against a local fake HTTP server). `cargo clippy --all-targets`: 0 warnings.
- `bun run check`: 0/0. `bun run test`: 70 passed. `bun run build`: ok.

## Limitations

- Not tried against a real provider or clicked through in the running app (no smoke test requested).

## Follow-up

- Optional: read-only file tools for providers that support function calling.
