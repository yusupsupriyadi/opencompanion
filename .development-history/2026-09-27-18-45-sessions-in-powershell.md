# Terminal sessions start their CLI from PowerShell

## Summary

The owner wants CLI sessions and the Terminal screen to stand on PowerShell. On Windows, a terminal (interactive) session now starts PowerShell 7 (or Windows PowerShell), which runs the CLI with the user's profile applied; the session still ends when the CLI exits. New terminals always start in the default shell (PowerShell on Windows) instead of the shell picked last.

## Changes

- `src-tauri/src/pty.rs`: `PtySpec::powershell`; when set, PowerShell runs a fixed script that starts the CLI via `ProcessStartInfo` and exits with its code. Program, arguments and folder go through `OPENCOMPANION_*` environment variables, cleared before the CLI starts. Real-PTY test.
- `src-tauri/src/proc.rs`: `command_line`, Windows C-runtime quoting for the argument string. Unit test.
- `src-tauri/src/terminal.rs`: `powershell()` (pwsh first, then Windows PowerShell; `None` off Windows), sharing detection with `available_shells`.
- `src-tauri/src/session.rs`: interactive sessions pass `powershell()`. `air-spike.rs` and terminals pass `None`.
- `src/lib/TerminalForm.svelte`, `src/routes/terminal/terminal.test.ts`, `DESIGN.md`: the last-picked shell is no longer remembered.

## Decisions

- Prompts never enter PowerShell's parser (`$`, backticks, quotes stay literal); `-EncodedCommand` avoided so the command stays readable in process lists.
- The profile loads and the execution policy is left as the user set it.
- Headless sessions still start the CLI directly: they have no terminal.

## Verification

- `cargo test` (from `C:\Users\yusup\Project\...`): 91 unit + 5 companion + 12 manager passed (manager interactive sessions now go through PowerShell). `cargo clippy --all-targets`: clean.
- Vitest `src/routes/terminal`: 7 passed. `bun run check`: 0 errors.

## Limitations

- Not exercised in the running app (no smoke test requested): a slow or noisy PowerShell profile delays or prints before the CLI starts.

## Follow-up

- none
