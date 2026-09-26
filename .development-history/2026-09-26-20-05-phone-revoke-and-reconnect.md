# A removed phone loses its live updates, and the phone reconnects reliably

## Summary

Code review found that the phone's WebSocket was checked only when it opened: after Remove on the desktop, or after phone access was turned off, an open socket kept receiving sessions, prompts and notifications. The phone client could also stop reconnecting for good, went offline on any single failed request, and replaced the screen (and a half-written form) with the offline page.

## Changes

- `src-tauri/src/companion.rs`: a revoke channel; the socket pump closes with 4401 when its device is removed and 4403 when phone access stops (`stop()` revokes all). `lib.rs` `remove_device` revokes that device.
- `src-tauri/tests/companion.rs`: raw WebSocket handshake test for both close codes and for the other phone staying connected.
- `src/lib/phone.svelte.ts`: 4401 goes back to pairing; retries back off (3, 6, 10 s) and always reschedule, with a request first so a 401 ends in pairing; a stale socket's late close is ignored; a failed request while the socket is open stays online. `phone.test.ts` (new, fake socket, 4 tests).
- `src/routes/m/+layout.svelte`, `phone.css`: offline page is an overlay over the inert screen, which stays mounted; the toast region is always in the page.

## Verification

- `cargo test`: 77 unit + 14 integration passed. `bun run check`: 0/0. Vitest: 117 passed.

## Limitations

- Not tried on a real phone (no smoke test requested).

## Follow-up

- none
