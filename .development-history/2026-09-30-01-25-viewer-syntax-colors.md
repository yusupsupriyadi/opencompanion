# File viewer: syntax colors and editor touches

## Summary

The owner found the session file viewer plain: one color on the dark panel. Code now colors like an editor through Shiki (VS Code's TextMate grammars), the line numbers stay in view while a long line scrolls sideways, the bar names the language, and a toggle wraps long lines.

## Changes

- `src/lib/highlight.ts`: about 40 bundled grammars, file name to language, a theme whose colors are `--syn-*` variables, and slice-by-slice tokenizing (300 lines, a turn for the page between slices, long lines left plain).
- `src/lib/FileViewer.svelte`: plain text first, colors after; a diff colors each side as its own file; sticky gutter; language label; Wrap long lines (`aria-pressed`, remembered in `oc-viewer-wrap`).
- `src/app.css` (`--syn-*` tokens), `src/routes/+layout.svelte` (Plex Mono 400 italic for comments), `src/lib/i18n/workspace.ts`, `package.json`/`bun.lock` (`shiki` 4.4.3).
- `DESIGN.md` (syntax palette, contrast rows, viewer spec, icon, decision), `README.md`.

## Decisions

- Syntax colors reuse the dusk `rainbow` list and the terminal colors, so no new palette colors; comments on a search-hit row take `term-text` because `term-dim` falls to 4.44:1 there.
- Only common grammars ship (all of Shiki's add about 8.6 MB); Shiki loads the first time a file opens.

## Verification

- `bunx vitest run`: 42 files, 265 tests passed (4 new in `highlight.test.ts`, 2 new viewer tests); `bun run check`: 0 errors, 0 warnings; `bun run build`: ok.
- Color pairs checked with `contrast-check.py` on both panel colors and on the add, remove and hit tints.

## Limitations

- Not run in the app (no smoke test was requested), so the colors were not seen on screen.
- A later rerun, after another agent started uncommitted pane work, fails 2 split-terminal tests in `shells.test.ts` and type-checks with errors in `panes.test.ts`; none of those files are part of this change.

## Follow-up

- none
