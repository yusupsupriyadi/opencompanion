# Compact, icon-first phone remote

## Summary

The phone Session screen is now one phone tall: a one-row header (back, CLI mark, title, status chip, Stop), a Terminal | Activity | Details switch, a terminal pane that fills the height and follows new output, and a compact dock of key caps and an icon Send. Sessions, Board, New session and the tab bar got the same dense, icon-first treatment.

## Changes

- `src-tauri/src/companion.rs`: `GET /api/sessions/:id` also returns `files` (changed files over the desktop's 400 events), `task` (Board card) and `usage` (CPU, memory, children while live, from a monitor kept in `Companion`); events sent drop to the newest 150.
- `src/lib/tail.ts`: splits the terminal screen into text, rules and TUI boxes so box drawing survives a narrow screen.
- `src/lib/timeline.ts`: the desktop timeline lines, now shared by `Timeline.svelte` and the phone Activity view.
- `src/lib/PhoneScroll.svelte`, `PhoneStatus.svelte`, `PhoneTopBar.svelte`: follow-the-end pane with a jump button, status chip with a live dot, tab-screen top bar with the connection.
- `src/lib/phone.css`: compact kit (`m-icon-btn`, `m-key`, `m-status`/`m-dot`, `m-conn`, `m-views`, `m-pane`/`m-scroll`/`m-screen`, `m-press`, `m-appear`), denser cards and needs panels, tab indicator.
- `src/routes/m/**`: session, sessions, board and new-session screens; the layout adds one passive touch listener so iOS shows pressed states.
- `DESIGN.md`: owner motion exception for the phone and the new M2, M3, M5, M7 and tab bar specs.

## Decisions

- The tail has no ANSI codes (vt100 on the desktop already applies them), so only box drawing is handled; lines wrap as before.
- Status pulses are an owner-requested exception to MOTION 1, only while the state is live, off under reduced motion.
- Approve, Deny, Run, Resume and Move keep their words; icon-only only where the icon is unambiguous.

## Verification

- `npm test`: 32 files, 179 tests passed. `npm run check`: 0 errors, 0 warnings. `npm run build`: done.
- `cargo test --lib companion` and `cargo test --test companion`: passed.

## Limitations

- No smoke test on a real phone was run (needs approval). Terminal colors are not sent to the phone.

## Follow-up

- Chat screens can adopt the compact kit classes.
