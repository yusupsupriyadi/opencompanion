# M0 Spike results

| | |
|---|---|
| Date | 2026-09-25 |
| Machine | Windows 11 Home 10.0.26200, Rust 1.96 (MSVC), WebView2 153 |
| CLI versions | Claude Code 2.1.282, Codex CLI 0.153.4, OpenCode 1.18.30, Gemini CLI not installed |
| Test tool | `src-tauri/src/bin/air-spike.rs` (subcommands `detect`, `scan`, `pty`, `headless`, `replay`, `waiting`) |
| Test folder | an empty scratch folder outside the repo, one per CLI |

M0's done criteria in the PRD: all three CLIs can run interactive and headless from a Rust prototype, and the permission detection results are recorded per CLI. The status per criterion is in the summary table; the parts that did not pass are named plainly below it.

## Summary

| | Claude Code | Codex CLI | OpenCode |
|---|---|---|---|
| Detection + version | pass | pass | pass (npm shim redirected to the native exe) |
| Interactive PTY (ConPTY) | pass | pass | pass with `--pure`; without it, it crashes because of a plugin in the user's config |
| Headless + event parsing | pass | failure path only: 401 from the configured provider | pass with `--pure` |
| Headless "waiting for permission" signal | pass: `control_request` / `can_use_tool`, answered by the host | not tested (auth) | none: `run` rejects by itself, the path is through `opencode serve` |
| PTY "waiting for permission" signal | pass: `PermissionRequest` and `Notification` hooks, plus text patterns | text patterns for the update offer only | not tested |
| External process detection | pass, with working folder | pass, with working folder | pass, with working folder; the wrapper exe and the platform exe count as one session |

## Findings per area

### CLI detection (FR-01, FR-02)

- `which` + common install folders (`~/.local/bin`, `%APPDATA%/npm`, `~/.bun/bin`, `~/.opencode/bin`) found all three CLIs in 731 ms, in parallel.
- An npm `.cmd` shim that only forwards to a native exe (OpenCode) is replaced by that exe. The prompt does not go through `cmd.exe`, so batch quoting rules do not apply.
- Codex on this machine was installed through the native installer (`%LOCALAPPDATA%/Programs/OpenAI/Codex/bin/codex.exe`), not npm.

### PTY (FR-10)

- **ConPTY holds back all output until the cursor position query (`ESC[6n`) is answered.** Without an answer, every CLI emits only 4 bytes and then goes quiet. The Rust core now answers it from the `vt100` emulator's cursor position, so sessions with no terminal view keep running. Decision for M1: xterm.js in the webview must not pass its own CPR answer to the PTY.
- First output arrives about 270 ms after spawn. Resize and Ctrl+C work: Claude Code exits with code 1, Codex and OpenCode with code 0.
- Each CLI's first screen can be a dialog waiting for the user:
  - Claude Code: the trust dialog for a new folder. The default choice is "No, exit".
  - Codex: an update offer. The default choice is "Update now", which runs an install script. **OpenCompanion must never send Enter to this screen automatically.**
- OpenCode without `--pure` exits within 4 seconds with "Unexpected server error". The OpenCode log records `plugin config hook failed` and then a null `n.provider`; the same error has appeared since 2026-09-10, so this is a problem with a plugin in the user's config, not the PTY.
- `vt100` `contents()` joins lines that carry the wrap flag. Screen text is now read line by line.

### Headless and events (FR-11, FR-14)

- **Claude Code**: `-p --output-format stream-json --input-format stream-json --verbose --permission-prompts host --permission-prompt-tool stdio --permission-mode manual`, with the prompt sent as a `user` message on stdin. Events observed: `system/init`, `system/hook_*`, `assistant` (text, tool_use, thinking), `user` (tool_result, `tool_use_result.filePath` for Write), `control_request`, `rate_limit_event`, `result`.
- The user's hooks, `CLAUDE.md`, MCP servers and skills also load in headless mode (84 tools on the test machine). One small run is worth 0.32 to 0.34 USD according to `total_cost_usd`; the account is a subscription (`apiKeySource: none`), so this uses quota, not API billing. For the orchestrator chat (FR-20) the context needs to be limited, for example with `--setting-sources`, `--strict-mcp-config` and `--tools`.
- **Codex**: `exec --json --skip-git-repo-check -s workspace-write -C <dir> -`, with the prompt on stdin. The custom provider set in `~/.codex/config.toml` on the test machine had neither `env_key` nor `requires_openai_auth`, so every request was rejected with 401. Events observed: `thread.started`, `turn.started`, `item.completed` (type `error` for config warnings), `error` ("Reconnecting... n/5"), `turn.failed`. The item types for a successful run (`agent_message`, `command_execution`, `file_change`) come from the Codex docs and have not been observed.
- **OpenCode**: `run --format json --dir <dir> --pure <prompt>`. Events: `step_start`, `tool_use` (`part.tool`, `part.state.status/input/output`, `metadata.files` for changed files), `text`, `step_finish` (tokens and cost). There is no "finished" event; the Done status comes from the process exit. `--pure` turns plugins off, but MCP servers still load.
- The `events.rs` parser normalizes all three into `SessionEvent` and is checked again against real recordings through `air-spike replay`. Unknown lines become `Raw`.

### "Waiting for permission" signal (FR-16, FR-17)

- **Claude Code headless**: `--permission-prompts host` alone is not enough; permissions are denied right away (`system/permission_denied` and `result.permission_denials`). With `--permission-prompt-tool stdio` added, the request arrives as a `control_request` of subtype `can_use_tool` (fields `request_id`, `tool_name`, `input`, `permission_suggestions`, `tool_use_id`) 11.7 seconds after start. A `control_response` answer with `behavior: allow` makes the file actually get created. This is the main Approve/Deny path for M1.
- **Claude Code PTY**: hooks injected through `--settings <file>` fire when the permission dialog appears. `PermissionRequest` carries `tool_name` and `tool_input`; `Notification` carries `notification_type: permission_prompt`. The fallback text pattern ("Do you want to …?" + "Esc to cancel") detects the dialog on screen 29 of 30 and gives no false detection on the other 28 screens. Esc denies the permission.
- **OpenCode**: with the `ask` permission, `opencode run` writes `permission requested: edit (…); auto-rejecting` to stderr and the tool call fails. Approve/Deny for OpenCode in M1 has to go through `opencode serve` and its HTTP API.
- **Codex**: not tested because of auth. Candidate paths: `codex app-server` (JSON-RPC, experimental) and Codex hooks.

### External processes (FR-31)

- `sysinfo` 0.39 reads the name, arguments, working folder, start time, CPU and memory. The first scan takes 32 to 53 ms, later ones 13 to 25 ms.
- Test with three CLIs open at once (Claude Code, Codex, OpenCode, each in a separate terminal): each appears exactly once with the right working folder.
- Process names on this machine: `claude.exe`, `codex.exe`, `opencode.exe`. Helpers such as `codex-windows-sandbox-service.exe` and `codex-code-mode-host.exe` do not count as sessions. Children of the same CLI (the OpenCode wrapper exe) and processes spawned by OpenCompanion are skipped.
- The mode is read from flags (`-p`, `exec`, `run`, `serve`), without storing the full command line, because it can contain a prompt or token.

### Environment

- A Claude Code session passes 10 marker variables (including `CLAUDE_CODE_MESSAGING_TOKEN`) down to child processes. As a result, a CLI started from inside it turns off transcript saving. OpenCompanion now drops these variables from every CLI it starts (`proc::INHERITED_SESSION_VARS`); the "Transcript saving is off" warning is gone after that.

## Decisions for M1

1. Headless Claude Code uses the stdio control protocol (`can_use_tool`) for Approve/Deny.
2. Interactive Claude Code uses the `PermissionRequest` and `Notification` hooks through `--settings` as the main signal, text patterns as the fallback, and the method used is shown in the session detail.
3. OpenCode runs through `opencode serve` so permissions can be answered; the next spike measures that API.
4. The Rust core answers `ESC[6n`; the terminal in the webview does not pass on CPR answers.
5. CLI opening dialogs (folder trust, update offer) are shown as "Waiting for you" and are never answered automatically.
6. The orchestrator chat uses a limited CLI context so quota is not spent loading tools and skills.

## Not done yet

- Codex: a successful run and the permission signal, on a machine where Codex can sign in to its provider.
- OpenCode: PTY and runs without `--pure` after the failing plugin is fixed; the permission signal through `opencode serve`.
- Gemini CLI: not installed yet.
