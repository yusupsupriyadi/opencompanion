# npm shims without cmd.exe, and Stop that ends the whole process tree

## Summary

Code review found that a CLI installed by npm (a `.cmd` shim that runs a node script) got its prompt through cmd.exe, where `&` ran a second command, `%VAR%` expanded and a second line was cut; and that Stop and app exit killed only the CLI's own PID, leaving its children (or, for npm installs, the CLI itself under node) running.

## Changes

- `src-tauri/src/proc.rs`: `launcher` turns an npm node shim into `node <script>` (node.exe next to the shim first, like npm's shim); `is_batch`, `cmd_unsafe`; `kill_tree` (`taskkill /PID /T /F` from System32); `command` uses the launcher; timeout kills the tree. 4 tests, one kills a real `cmd` + `ping` tree.
- `src-tauri/src/pty.rs`: PTY sessions use the launcher; any other batch file refuses text with cmd metacharacters or line breaks, with a message that says how to fix it. Hard stop kills the tree.
- `src-tauri/src/headless.rs`: `kill` ends the tree first. `cli.rs`: doc comment.

## Decisions

- The shim stays the CLI's path in the UI and for detection; only the spawn changes.
- `taskkill` runs before the parent is reaped, since `/T` finds children through the parent.

## Verification

- `cargo test`: 77 unit + 14 integration passed; `cargo clippy --all-targets`: clean.

## Limitations

- macOS/Linux still kill only the CLI's PID (untested platforms).

## Follow-up

- none
