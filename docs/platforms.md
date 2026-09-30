# Platforms

| | Windows 11 | Linux | macOS |
|---|---|---|---|
| How it was tested | By hand, every day | End to end in Docker on Ubuntu 24.04: the installed `.deb`, driven through WebDriver ([e2e/linux](../CONTRIBUTING.md#linux-end-to-end-in-docker)) | Built and tested in GitHub Actions (macOS 26, Apple Silicon): svelte-check, Vitest, clippy and every Rust test pass. Not run by hand yet |
| Packages from `bun run tauri build` | `.msi`, `.exe` | `.deb`, `.rpm`, AppImage | `.app`, `.dmg` |
| Window controls | OpenCompanion's own | OpenCompanion's own | Native traffic lights |
| Paste in a terminal | Ctrl+V | Ctrl+Shift+V (Ctrl+V goes to the CLI) | ⌘V |
| A click on a notification | Opens the session | Opens the session when the notification server supports actions (tried with dunst; GNOME and KDE support actions but were not tried) | Does not open the session |
| Start in the tray at sign-in | Run key in the registry | `~/.config/autostart/opencompanion.desktop` | `~/Library/LaunchAgents/dev.opencompanion.app.plist` |
| Tray icon | Taskbar corner | Needs AppIndicator support; GNOME shows it only with the AppIndicator extension | Menu bar |

The rest of the macOS column says what the code does: the app compiles and its tests pass on macOS, but nobody has opened the window on a Mac yet.

- On macOS and Linux, OpenCompanion reads your login shell's environment when it starts (`$SHELL -ilc env`, at most 5 seconds). So a CLI installed with Homebrew, npm, nvm, volta or bun is found, `#!/usr/bin/env node` scripts find node, and variables you export in `~/.zshrc` or `~/.bashrc` (API keys, proxies) reach the CLI, even when OpenCompanion starts from Finder or a desktop launcher. If the shell does not answer, it adds the usual install folders to the PATH instead.
- On macOS and Linux, Stop ends the CLI's whole process group, so the dev servers and tools it started stop with it. A child that starts a session of its own (a detached process) is not reached, where Windows would still end it.
- If Linux has no AppIndicator library for the tray icon, OpenCompanion still starts, and closing the window quits it, so the window cannot end up hidden with no way back.
- On Linux, folder rules (auto-run folders, notification rules) compare folder names with case, as the file system does. On Windows and macOS they ignore case.

## Build requirements

- Windows 11, a Linux desktop with WebKitGTK 4.1, or macOS (tested in CI only, see the table above).
- Rust (stable), with the MSVC toolchain on Windows.
- The Tauri 2 system prerequisites for your platform, see [tauri.app/start/prerequisites](https://tauri.app/start/prerequisites/): on Windows, Microsoft C++ Build Tools and WebView2 (WebView2 ships with Windows 11); on Linux, WebKitGTK 4.1, libayatana-appindicator and the other listed packages, plus `xdg-utils` to bundle an AppImage; on macOS, the Xcode Command Line Tools.
- [Bun](https://bun.sh), and [Node.js](https://nodejs.org) (LTS) to run the tests.
- At least one supported CLI, installed and signed in. OpenCompanion runs it with your own account and quota.
