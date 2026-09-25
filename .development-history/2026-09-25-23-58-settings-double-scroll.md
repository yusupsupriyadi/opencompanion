# Settings double scroll, sidebar stays fixed

## Summary

Settings showed two vertical scrollbars (window + page), and scrolling the window dragged the sidebar away. Root cause: absolutely positioned elements (`.sr-only` legends/captions and the hidden radios in `.opt`) had `.app` as their containing block, so they escaped `.main`'s scroll clip and stretched the document. Settings has several of them below the fold (permission modes, text size).

## Changes

- `src/app.css`: `.main` is now `position: relative`, so absolute children scroll inside the page; `.opt` is `position: relative`, so its hidden radio sits in its option; `.app` gets `overflow: hidden`, so the shell itself never scrolls and the sidebar stays put.

## Decisions

- Fixed in the shared shell CSS rather than per page, since every page with `.sr-only` below the fold could hit the same bug.

## Verification

- `npx vitest run src/routes/settings`: 7 passed.
- `npx vite build`: exit 0.
- Visual check in the running app not done (BrowserOS Neo was not connected).

## Limitations

- At the 720px breakpoint (below the Tauri 1100px min width) the sidebar is a top bar and still scrolls with the page, as before.

## Follow-up

- none
