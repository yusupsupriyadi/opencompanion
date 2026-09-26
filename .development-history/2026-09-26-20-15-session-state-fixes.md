# Session state: ordered writes, released finished sessions, hook Idle, one spawn at a time

## Summary

Fixes from the backend code review, all in the session manager.

## Changes

- `src-tauri/src/session.rs`:
  - `update` stores and announces under the session's lock, so a reader thread's older copy cannot land after a finish and show the session running again.
  - `finish` drops the session from memory (its output is in the log; Resume and follow-ups register it again), and `own_pids` counts only live sessions, so a reused PID no longer hides a CLI from the outside list.
  - Claude Code's Stop hook keeps the session Idle until the next hook report, instead of terminal redraws flipping it back to Running within the same poll.
  - Resume and follow-ups claim the session while they spawn, so a phone and a desktop press at once start one process.
  - The hook command quotes a `'` in the path for Git Bash.
- Tests for the finished-session release, the spawn claim and the quoted path.

## Verification

- `cargo test --lib session::`: 8 passed on the staged snapshot; full `cargo test`: 77 unit + 14 integration passed; clippy clean.

## Limitations

- none

## Follow-up

- none
