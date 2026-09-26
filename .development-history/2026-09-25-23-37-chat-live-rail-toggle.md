# Hide and show the Live sessions rail in Chat

## Summary

Owner asked for the Live sessions rail in Chat to be hideable. A panel icon button at the end of the Chat header now hides and shows it at every window width; below 1280 the rail, which used to be unreachable, opens as a floating panel (DESIGN.md already promised a button there).

## Changes

- `src/routes/chat/+page.svelte`: `sidebar-simple` toggle (`aria-expanded`, filled when open); `MediaQuery("min-width: 1280px")` picks docked column (hidden state remembered in `air-chat-rail-hidden`) or a floating panel under the header (not remembered, Escape closes, focus returns to the button); rail content moved into a `live()` snippet; grid gets a third column only when docked.
- `src/test/setup.ts`: `matchMedia` stub for jsdom (every query false).
- `src/routes/chat/chat.test.ts`: wide window hide/show survives a remount; narrow window opens, tabs into, and Escapes the panel.
- `DESIGN.md`: D3 Chat rail toggle spec, icon list.

## Decisions

- Icon button rather than a text button: the header already holds Delete chat and Change planner, and at 1440 a third label squeezed the planner line.
- Narrow panel hangs from the header so it never covers its own toggle.

## Verification

- `bun run test`: 91 passed. `bun run check`: 0 errors, 0 warnings. `bun run build`: ok.

## Limitations

- Not clicked through in the running app (no smoke test requested).

## Follow-up

- none
