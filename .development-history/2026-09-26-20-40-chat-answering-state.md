# Chat knows when the planner is answering, wherever the message came from

## Summary

Code review found that the desktop Chat never listened to `chat-changed` and kept "the planner is answering" only in the page. Leaving Chat during a turn and coming back showed the question with no answer and no spinner, Send worked again (a second planner turn could run in the same chat), and turns or card changes from the phone never showed until another chat was opened. The whole thread was also a live region, so opening a chat read every message aloud.

## Changes

- `src-tauri/src/actions.rs`: one `Claims` type for card starts and chat turns; a second message in a chat that is being answered is refused; `answering()` lists those chats; `chat_send` tells listeners when the question is stored. Test.
- `src-tauri/src/lib.rs`, `companion.rs`: `chat_answering` command; the desktop hears `chat-changed` for its own turns too; the phone API returns `answering` for the list and for a chat.
- `src/routes/chat/+page.svelte`: listens to `chat-changed` (quiet reload of the list and the open chat), shows "still answering" from the backend and holds Send and Delete, returns focus to the composer only when it was there, and announces answers through one status node. Test.
- `src/routes/m/chat/*`: the same answering state and announcement on the phone; `phone.svelte.ts` handles each live update in `receive`. Test.

## Verification

- `cargo test`: 78 unit + 14 integration passed; clippy clean. Vitest: 120 passed; `bun run check`: 0/0.

## Limitations

- none

## Follow-up

- none
