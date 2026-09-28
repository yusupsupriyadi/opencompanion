# Session workspace panel: design

Approved in chat on 2026-09-28. The owner asked for the file explorer, git diff and branch views that Orca (onorca.dev) shows beside its terminals, on the right of the session screen. Scope chosen by the owner: everything read-only, plus switching branches.

## Goal

While a CLI works in a folder, the owner can browse that folder, read a file, see what changed since the last commit with its diff, and see or switch the branch, without leaving OpenCompanion.

## Decisions

- The 300 px column on the right of the session screen becomes a tabbed panel: **Files**, **Changes**, **Branch**, **Details**. Details is the column as it was (files the CLI reported, process, needs-you signal, Board card). The tab shown last is remembered on this computer; the first visit opens Files.
- Desktop only. The phone companion does not get these views, so file contents never travel over the plain-HTTP LAN connection.
- Read-only, except **Switch branch**. Switching is refused while the session is live (the CLI holds the files in its context) and while tracked files have uncommitted changes. Untracked files do not block it: git carries them over and refuses by itself when one would be overwritten. A confirmation dialog names the branch and the folder.
- Every call takes the session id; the backend reads the folder from the session. Relative paths are checked to stay inside that folder (no `..`, no absolute paths, symlinks that lead out are refused).
- Git runs as the `git` executable found on the PATH (the login shell's PATH on macOS and Linux), with `GIT_OPTIONAL_LOCKS=0` so a status read never takes the index lock from the CLI's own git commands. A folder that is not a repository still gets Files; Changes and Branch say it is not a repository.
- Opening a file or a change shows it as a second tab above the terminal, in the terminal's dark colors (like the tabs on the Terminal screen). One viewer tab at a time: opening another file replaces it. The terminal stays mounted underneath.

## Files tab

- Header: folder name, Refresh.
- Find field with a Names | Contents switch. Names matches every word of the query in the relative path, case-insensitively, file name matches first, at most 200. Contents searches text files up to 1 MB for the query as plain text, case-insensitively, at most 300 lines and 4 s; the list says when it stopped early. Results are the files git would list (`ls-files --cached --others --exclude-standard`), or a walk of at most 20 000 files outside a repository.
- The tree loads one folder at a time: folders first, then files, in natural order (`9` before `10`). `.git` is hidden. Ignored entries are shown in italics with a `prohibit` icon and "Ignored by .gitignore" for screen readers and in the tooltip. Changed files carry their git letter (M, A, D, R, U, C, ?) at the right, colorless, with the meaning in the tooltip.
- ARIA tree: Up/Down move, Right opens or goes to the first child, Left closes or goes to the parent, Enter opens a file, Home/End.

## Changes tab

- Uncommitted changes against HEAD: staged, unstaged and untracked together, one row each: letter, file name, folder, `+added −removed`. Clicking a row opens its diff.
- Diff viewer: unified diff with old and new line numbers, `+`/`−` in the gutter, a light green or red tint behind added and removed lines, hunk headers in `term-dim`. Binary files and diffs over 2 MB say so instead.

## Branch tab

- Current branch (or "Detached at abc1234"), its upstream with ahead/behind, local branches sorted by last commit with their subject and age, and the last 30 commits on HEAD.
- Each other branch has **Switch**; when switching is blocked, one line above the list says why.

## Refresh

- Git status is read when a tab opens, every 5 s while Files, Changes or Branch is shown and the window is visible, after a switch, and on Refresh. When the status changes, the open folders of the tree and an open diff are read again.

## States

- Files: "Reading {folder}…", error with Try again, "This folder is empty.", search "Searching…", no match, stopped early.
- Changes and Branch: "Reading git…", "Git is not installed or not on PATH.", "{folder} is not a git repository.", "No uncommitted changes.", "No commits yet."
- Viewer: loading, error with Try again, binary, too large, file gone.
