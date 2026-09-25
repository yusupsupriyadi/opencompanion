# Planner model and thinking level in Chat

## Summary

Owner asked for a model picker and a thinking level in Chat. The composer now has Model and Thinking selects for the CLI planner, saved per CLI in Settings and passed as each CLI's own flags (PRD FR-28).

## Changes

- `src-tauri/src/models.rs` (new): model lists per CLI (Claude Code's `/model` catalog cache `~/.claude/cache/model-catalog/*-cc.json` with versioned names and per-model efforts, aliases as fallback; `codex debug models`, `opencode models --verbose` with a `--pure` retry), `args()` for `--model/--effort`, `-m` + `-c model_reasoning_effort=`, `-m` + `--variant`; flag-like values refused.
- `src-tauri/src/db.rs`: `ChatModel`, `Settings.chat_models`. `lib.rs`: `cli_args` helper, `chat_models`, `chat_set_model`, flags appended in `chat_send`'s CLI branch.
- `src/lib/PlannerModel.svelte` (new) + test, `api.ts`, `format.ts` `effortLabel`, `store.svelte.ts` list cache, `src/routes/chat/+page.svelte` composer bar, `chat.test.ts`, `settings.test.ts` fixture.
- `docs/PRD.md` FR-28, `DESIGN.md` Composer and D3, `README.md`.

## Decisions

- "CLI default" passes no flag, so nothing changes until the owner picks something.
- Owner follow-up: labels need versions ("Opus 5.5", "Fable 5.1"), so Claude models come from its own catalog and pass full ids; older ones sit under "More models"; models needing a newer Claude Code are hidden. A saved id the list lacks shows as "{id} (saved)".
- A level the new model lacks resets to the default; Codex's default model gets only the levels all listed models share; OpenCode's default model gets none.
- Hidden when the planner is a custom provider (parallel work by another session).

## Verification

- `cargo test`: 63 unit + 12 integration passed (7 new in models). `cargo clippy --all-targets`: 0 warnings.
- `bun run check`: 0/0. `bun run test`: 80 passed (8 new for this feature). `bun run build`: ok.
- Real CLIs: lists parsed from Claude Code 2.1.282, Codex 0.153.4 (6 models), OpenCode 1.18.32 (28 models, plugin failure fell back to `--pure`); `claude -p --model haiku --effort max` and `--model claude-haiku-4-5-20251001` answered; the real catalog gives Opus 5.5, Fable 5.1, Sonnet 5, Haiku 4.5 (no efforts) plus 6 under "More models".

## Limitations

- No real planner turn with Codex `-c model_reasoning_effort` or OpenCode `--variant`; flags are from each CLI's `--help`.
- The Claude catalog is Claude Code's internal cache, not a documented API; if its shape changes the aliases come back.
- Not clicked through in the running app (no smoke test was requested).

## Follow-up

- none
