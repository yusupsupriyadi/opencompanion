# Cross-platform parity: design

Approved in chat on 2026-09-28.

## Goal

OpenCompanion works the same on Windows, Linux and macOS, and a CI run proves it. Until now only Windows 11 was tested. A Linux run in Docker (2026-09-28, commit 441ab82) passed an 18-step end-to-end test on the installed `.deb`, and an audit of the code found the gaps below.

## Decisions

- Target: feature parity plus CI. No release uploads, no code signing, no notarization.
- CI runs on CircleCI and only when triggered by hand. GitHub Actions minutes for the private repo are used up; the repo will become public later, so CI steps live in plain scripts that any CI can call.
- The owner has no Mac. macOS is proven by CI (build, unit and integration tests) only, and README says so.
- macOS gets native traffic lights. Windows and Linux keep the custom title bar.
- Windows behavior stays as it is: autostart through the registry, toasts with click-to-open, interactive sessions through PowerShell.
- The identifier stays `dev.opencompanion.app`. Changing it would move every existing data folder; Tauri only warns about the `.app` ending on macOS.

## 1. Environment and processes (`src-tauri`)

**Login shell environment.** A GUI app started from Finder or a Linux launcher gets a minimal PATH (`/usr/bin:/bin:/usr/sbin:/sbin` on macOS). CLIs are then not found, npm CLIs fail with `env: node: No such file`, and variables from `~/.zshrc` or `~/.bashrc` (API keys, proxies) never reach them.

- New module `shell_env.rs`, Unix only. At startup, on a background thread, it runs the user's `$SHELL` (fallback `/bin/sh`) as an interactive login shell, `-ilc`, printing the environment between two markers, with a 5 s timeout and stdin closed. The parsed map is kept in a `OnceLock`.
- The environment is not written into the app's own process with `set_var`. It is applied where children start: `proc::command`, the PTY `CommandBuilder` in `pty.rs`, and plain terminals in `terminal.rs`. CLI resolution in `cli.rs` searches the captured PATH.
- Code that needs the environment (CLI detection, starting a session) waits for the capture, at most until its timeout.
- If the capture fails or times out, PATH is the inherited PATH plus the existing folders in a fallback list: `/opt/homebrew/bin`, `/usr/local/bin`, `~/.local/bin`, `~/.npm-global/bin`, `~/.volta/bin`, `~/.bun/bin`, `~/.cargo/bin`, `~/.claude/local`. The same list extends `cli::extra_dirs` on macOS and Linux.
- Windows keeps its current discovery and launch path; nothing changes there.

**Stopping a process tree.** `proc::kill_tree` does nothing off Windows, so Stop, app exit and timeouts leave a CLI's children (dev servers, MCP servers) running.

- On Unix, headless children start in their own process group (`CommandExt::process_group(0)`). A PTY child is already a session leader, so its pid is its group id.
- `kill_tree` sends SIGTERM to the group, waits up to 2 s, then sends SIGKILL.

**Terminal type.** Interactive sessions on Unix get `TERM=xterm-256color` and `COLORTERM=truecolor` when those are unset, as plain terminals already do.

## 2. OS integration

- **Start at login.** On macOS a LaunchAgent (`~/Library/LaunchAgents/dev.opencompanion.app.plist`) and on Linux an XDG autostart entry (`~/.config/autostart/opencompanion.desktop`), both with the existing `--hidden` argument. `autostart.rs` writes them itself, the same files `tauri-plugin-autostart` would write, so there is no new dependency and the entries can be unit tested. On Windows the current `reg.exe` code stays. `autostart::supported()` becomes true on all three, so the Settings checkbox shows everywhere.
- **Notification click.** On Linux, notifications go through `notify-rust` with a default action; activating it opens the session, as on Windows. On macOS, notify-rust's click path needs the app's bundle id and the main run loop, which cannot be tried without a Mac, so macOS keeps the notification plugin (which handles both) without a click, and README says so.
- **Tray.** A tray that cannot be created no longer stops the app. Setup logs the error and records that there is no tray. Without a tray, closing the window quits the app instead of hiding it, so the window can never become unreachable. On GNOME without an AppIndicator extension, the tray can be "created" but stay invisible; README explains this.
- **Dock.** On macOS, `RunEvent::Reopen` shows the main window again, as clicking the tray icon does.
- **Window chrome on macOS.** `src-tauri/tauri.macos.conf.json` turns decorations on with `titleBarStyle: "Overlay"` and a hidden title, so the traffic lights sit on top of the meadow. `TitleBar.svelte` hides its minimize, maximize and close buttons on macOS and leaves room on the left for the traffic lights. The drag region stays.
- **Knowing the platform in the webview.** A small helper reads `navigator.userAgent` (`Macintosh`, `Windows`, `Linux`), so the title bar is right on the first paint with no IPC round trip.

## 3. Paths and frontend

- `projects::norm` compares paths case-sensitively on Linux, and case-insensitively on Windows and macOS (both default to case-insensitive file systems). Folder rules such as auto-run (`db.rs`) follow the same rule.
- `decode_claude_dir` also decodes the Unix form. Claude Code names `/home/u/proj` `-home-u-proj`; decoding starts at `/`.
- Keyboard hints show `⌘` on macOS and `Ctrl` elsewhere. Paste in a session and in a terminal: `⌘V` on macOS, `Ctrl+Shift+V` on Linux (`Ctrl+V` goes to the CLI, as in Linux terminals), `Ctrl+V` on Windows as now. Image paste follows the same keys.
- The folder field placeholder follows the OS: `C:\Users\you\Project\my-app`, `/Users/you/Projects/my-app`, `/home/you/projects/my-app`.
- `format.shortPath` shortens `/Users/<name>` and `/home/<name>` to `~`, as it does for `C:\Users\<name>`.
- Text that names Windows or PowerShell where it applies to every OS is reworded in both English and Indonesian (`settings.ts`, `backend.ts`, `shell.ts`). Text that is only true on Windows says so.
- Tests: the four unit tests with `C:\` fixtures (`orchestrator.rs`, `session.rs`, and two in `projects.rs`) use paths built for the OS they run on. The three non-Windows clippy warnings are fixed (`lib.rs` `open_session`, `proc.rs` `mut cmd`, `autostart.rs` test import).

## 4. CI and documentation

- `scripts/ci/checks.sh` runs, in order: `bun install --frozen-lockfile`, `bun run check`, `bun run test`, `cargo clippy --all-targets -- -D warnings`, `cargo test`. It runs under bash on all three OSes (Git Bash on Windows) and needs Node.js for Vitest.
- `e2e/linux/` holds the Linux end-to-end harness from the 2026-09-28 run:
  - a `Dockerfile` with the Tauri prerequisites, WebKitWebDriver, Xvfb, Node, Bun, Rust and `tauri-driver`;
  - `run.sh`, which builds the image and runs the test in a container on a copy of the working tree;
  - the in-container script, which builds and installs the `.deb`, puts fake `claude` and `opencode` wrappers around `fake-cli` on PATH, and starts Xvfb, D-Bus and `tauri-driver`;
  - `e2e.py`, 18 steps over the W3C WebDriver protocol with no Python dependencies.
  It runs locally with Docker, including Docker Desktop on Windows, and in CI.
- `.circleci/config.yml` has four jobs: `linux-checks`, `linux-e2e`, `windows-checks` and `macos-checks`. They are gated by boolean pipeline parameters that default to false, so an ordinary push runs no job and uses no credits. A run is started from "Trigger Pipeline" in CircleCI with, for example, `macos: true`. Cargo and Bun caches are keyed on the lockfiles.
- macOS on the CircleCI Free plan is not confirmed: the pricing table and the plan docs disagree. The first macOS run settles it. If the plan refuses the job, the job stays in the config and README says macOS is not yet proven.
- The owner connects the repo to CircleCI once (sign in with GitHub, set up the project). This needs their account.
- README: a support table per OS; prerequisites per OS (Node.js for tests; `xdg-utils` to build an AppImage; the Linux AppIndicator library for the tray); data folders; which features differ (notification click, tray on GNOME); "macOS: built and tested in CI, not yet tried by hand". CONTRIBUTING: Node.js in the setup list, the checks script, and how to run the Linux E2E with Docker. CHANGELOG: lines under Unreleased.

## Out of scope

- Release workflows, installers uploaded anywhere, code signing and notarization.
- A monochrome template tray icon for the macOS menu bar (a new asset).
- UI end-to-end tests on Windows and macOS (WKWebView has no WebDriver).
- Detecting whether GNOME shows the tray icon.
- Changing the app identifier.

## Verification

- Linux in Docker: `scripts/ci/checks.sh` passes; `e2e/linux/run.sh` passes all steps.
- Windows (this machine): the same checks pass, and `bun run tauri build` still builds.
- CircleCI: `linux-checks`, `linux-e2e` and `windows-checks` pass on a triggered run. `macos-checks` passes, or the plan limit is recorded as above.
- The shell-environment parser, the Unix `decode_claude_dir`, the per-OS `norm`, `shortPath`, and the platform key choices have unit tests. Process-group stop is covered by an integration test that starts a child with a grandchild and checks that both are gone after Stop (Unix only).
