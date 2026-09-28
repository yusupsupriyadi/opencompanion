# Session workspace panel: files, changes and branch

## Summary

The owner asked for Orca's (onorca.dev) file explorer, git diff and branch views on the right of the session screen. The 300 px column became a 320 px tabbed panel (Files, Changes, Branch, Details), and a file or diff opens as a second dark tab above the terminal. Everything is read-only except Switch branch, the owner's choice. Desktop only.

## Changes

- `src-tauri/src/files.rs`: one folder at a time (folders first, natural order, `.git` hidden, ignored flag), name and text search, file read with 1 MB, binary and path-escape checks.
- `src-tauri/src/git.rs`: status (porcelain v2, sub-folder prefix, numstat counts), one file's diff against HEAD or the empty tree, branches, recent commits, switch refused on tracked changes. `GIT_OPTIONAL_LOCKS=0`, literal pathspecs for diffs.
- `src-tauri/src/lib.rs`: `folder_*` and `git_*` commands resolving the folder from the session id; `git_switch` refused while any live session runs in that folder.
- `src/lib/FilesPanel.svelte`, `ChangesPanel.svelte`, `BranchPanel.svelte`, `FileViewer.svelte`, `workspace.svelte.ts`, `diff.ts`, `i18n/workspace.ts`: the tabs, ARIA tree, viewer, shared git status poll (5 s while shown), English and Indonesian strings.
- `src/routes/session/+page.svelte`: ARIA side tabs (remembered), viewer tab strip, Details as a tab. `src/app.css`: tab strip styles moved here from the Terminal screen, side panel styles.
- `DESIGN.md` (D4, icons, contrast, decisions), `README.md`, spec `docs/superpowers/specs/2026-09-28-session-workspace-panel-design.md`.

## Decisions

- Ignored entries use italics plus a `prohibit` icon, and git letters stay colorless: secondary text on this plate is `ink` and status colors belong to chips.
- The phone companion does not get these views, so file contents never cross the plain-HTTP LAN link.

## Verification

- `cargo test`: 137 unit (14 new, including a real temporary repository) + 5 + 12 passed; `cargo clippy --all-targets -- -D warnings`: clean.
- `bunx vitest run`: 40 files, 224 tests passed (11 new); `bun run check`: 0 errors, 0 warnings; `bun run build`: ok.
- New color pairs checked with `contrast-check.py` (all at least 4.5:1).

## Limitations

- Not exercised in the running app (no smoke test was requested). A folder with a huge untracked, unignored tree makes each 5 s status read slow.

## Follow-up

- none
