# Shell command suggestions: design

Approved in chat on 2026-10-01. The owner asked for Warp's "remember and suggest commands" in the terminal, chose inline suggestions from history, the project's commands first, and the shells' own history files as a fallback, and approved the approach and the small decisions below.

## Goal

While typing in a shell tab (PowerShell, Git Bash, bash, zsh), the latest command that starts with what is typed shows after the cursor in dim text. Right Arrow at the end of the line accepts it. The session's own terminal (Claude Code, Codex and the rest) is left alone: those CLIs own their input and keep their own history.

## Decisions

- Suggestions come from, in this order: commands run in shell tabs of **this folder**, then of **any other folder**, then the shells' own history files (PSReadLine `ConsoleHost_history.txt`, `~/.bash_history`, `~/.zsh_history`). Within each, the most recently used first. Prefix match, case-sensitive, from the first typed character.
- The app learns what is typed and what ran through shell integration, the way Orca and VS Code do: a small script, added after the user's own profile, prints `OSC 133;B` where typing starts and `OSC 633;E;<command>` when a command runs. The typed text is read from the xterm buffer between that mark and the cursor, so arrow keys, Tab completion and editing keys all work.
- Command Prompt and `sh` get no script and no suggestions. There is no hook before a command runs in cmd.
- Not saved: empty commands, commands starting with a space (as bash and zsh do), commands over one line or over 1000 characters, and commands containing PSReadLine's sensitive words (`password`, `asplaintext`, `token`, `apikey`, `secret`, any case). The same rules filter the imported history files.
- PSReadLine 2.1+'s own inline prediction is turned off in our tabs, so only one dim suggestion shows.
- Settings › History gets "Suggest commands in shell tabs" (on by default) and "Delete saved commands". Off means shells start exactly as before, with no script, and nothing is recorded.

## Shell integration (`src-tauri/src/shell_integration.rs`)

| Shell | Start | `B` mark | Command text |
|---|---|---|---|
| PowerShell 5.1 / 7 | `-NoLogo -NoExit -EncodedCommand <script>`; profiles load first | At the start of `PSConsoleHostReadLine` | The line PSReadLine returns |
| bash (Git Bash, Linux, macOS) | `--rcfile <file> -i`; the file sources `/etc/profile`, then `~/.bash_profile`, `~/.bash_login` or `~/.profile`, as a login shell does | Appended to `PS1` by the last `PROMPT_COMMAND` entry (before bash-preexec's `__bp_interactive_mode`) | `history 1` when `HISTCMD` moved, so `HISTCONTROL`/`HISTIGNORE` apply |
| zsh | `ZDOTDIR` points at our folder, whose files source the user's own and restore `ZDOTDIR` | `zle-line-init` via `add-zle-hook-widget` | `preexec`'s first argument |

- Scripts are written under the app data folder (`shell/`) when a shell starts, and every hook is guarded so a failure never breaks the shell. PowerShell in ConstrainedLanguage mode is skipped.
- Command text is escaped as VS Code does for 633: `\` as `\\`, control characters as `\xHH`. PowerShell also escapes every byte outside printable ASCII, so the console code page cannot change it.
- Every report carries a nonce, `633;E;<nonce>;<command>`, new for each shell start. It reaches the script in `OPENCOMPANION_NONCE`, which the script removes from the environment first, so a program that prints a report (a file holding one, a remote shell) cannot plant a command. Added after review on 2026-10-01.

## Recording (`terminal.rs`, `db.rs`)

- The shell's output sink scans for `633;E` with a carry for sequences split across chunks, and calls `TerminalEmit::command(info, text)`. Recording happens in Rust, so output replayed into a reopened view is never recorded twice.
- `shell_commands(folder, command, uses, last_used)`, primary key `(folder, command)`, at most 1000 per folder (least recently used go first). The folder is the tab's folder (the session's), not the shell's current directory.
- `shell_history(folder)` returns `{ here, elsewhere, imported }`, most recent first. History files are read once per app run, the last 5000 lines of each, never written.
- The webview hears `terminal-command { folder, command }` and updates its cache.

## Webview (`command-suggest.ts`, `command-history.svelte.ts`, `Terminal.svelte`)

- `command-suggest.ts`: pure functions for the typed text (from the mark to the cursor, across wrapped rows only, nothing after the cursor) and the match.
- A suggestion shows only in a shell tab that is focused and taking input, at a prompt (after `B`, before Enter), in the normal buffer, with the cursor at the end of the typed text.
- Drawn with an xterm decoration (`allowProposedApi`), one cell per character in `term-dim`, clipped at the right edge. Right Arrow with no modifier accepts it by typing the rest into the shell.
- A suggestion shows only once the line reads what the keys sent so far make it (typed characters are added to the expected text), so other output arriving before a key's echo cannot bring back a suggestion for older text. After a key whose effect cannot be foreseen (Backspace, Tab, arrows), the line is taken as it reads once no key has been sent for 150 ms.
- Screen readers hear "Suggested command: {command}. Right Arrow accepts it." once the suggestion has stayed the same for 600 ms.

## Testing

- Rust: escaping, the split-chunk scanner, the save filters, the three history file formats (PSReadLine continuation lines, bash timestamps, zsh extended and metafied), the table's order and limit, and real shells through `Terminals` (Windows PowerShell and Git Bash on Windows, bash and zsh where installed) that must print `B` and report a typed command.
- Vitest: the pure functions, and `Terminal.svelte` showing a suggestion after `B` and typing its rest on Right Arrow.

## Build order

1. `shell_integration.rs`: scripts, escaping, scanner, filters, history file readers, with unit tests.
2. `db.rs`: table, record, query, delete, with tests; `Settings.shell_suggestions`.
3. `terminal.rs`: spawn with the scripts when on, scan output, `TerminalEmit::command`; real-shell tests.
4. `lib.rs`: record and emit, `shell_history` and `delete_shell_history` commands.
5. Webview: api, cache, pure functions, `Terminal.svelte`, Settings card, strings in English and Indonesian.
6. DESIGN.md (D13 and Settings › History), `docs/privacy.md`, `docs/features.md`, CHANGELOG.
