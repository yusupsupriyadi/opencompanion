# Phone chat on par with the desktop Chat (FR-57)

## Summary

The phone's chat thread now has what the desktop Chat has: chat history with "answering" per chat and switching in place, Delete chat with the second press, the live sessions (as a bottom sheet), `@` folders, the planner's model picker, the desktop's composer notes, and card editing before Run. The composer rides above the on-screen keyboard, and Enter only sends from a hardware keyboard.

## Changes

- `src-tauri/src/companion.rs`: `GET /api/chat/threads`, `GET /api/chat/setup` (never the provider's URL or key), `GET /api/chat/models/{cli}`, `POST /api/chat/model` (returns only `chatModels`), `POST /api/chat/cards/edit`, `DELETE /api/chat/{id}`, `GET /api/folders`; `/api/sessions` adds horizon marks for live sessions.
- `src-tauri/src/actions.rs`: `update_card`, `delete_thread` (refused while the planner answers), `planner_models`, `set_chat_model`, shared by the desktop commands and the phone.
- `src-tauri/src/lib.rs`, `session.rs`: `Emit::settings_changed` ("settings-changed" to the desktop, `{type:"settings"}` to phones); desktop chat delete and Settings save tell phones.
- `src/routes/m/chat/thread/+page.svelte`: the new thread screen; `src/lib/PhoneDispatchCard.svelte`: Edit, "Started …" lines, focus after swaps.
- `src/lib/PlannerModel.svelte` (`source` prop), `FolderMention.svelte` (`list`, `canBrowse`), `Dialog.svelte` (`sheet`), `keyboard.svelte.ts`, `store.svelte.ts`, `phone.css` (appended block), i18n en/id, DESIGN M6.

## Decisions

- No "Browse for a folder…" on the phone: it would open a dialog on the computer.

## Verification

- `cargo test`: 86 unit + 16 integration passed; clippy clean. Vitest 171 passed (thread tests from 4 to 18). `bun run build` passed. `bun run check`: my files clean; 3 errors are in another agent's untracked PhoneScroll/PhoneStatus.

## Limitations

- Not tried on a real phone (no smoke test requested); the keyboard detection is a heuristic that falls back to Enter adding a line.

## Follow-up

- none
