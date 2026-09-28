# Cross-platform parity for Windows, Linux and macOS

## Summary

A Docker run on Ubuntu 24.04 found Linux-only failures and a code audit found macOS/Linux gaps. Fixed per `docs/superpowers/plans/2026-09-28-cross-platform-parity.md`: login-shell environment for CLIs, process-group stop, start at login, Linux notification click, tray fallback, macOS traffic lights and Dock reopen, per-OS paths, keys and wording. Added a Linux E2E harness and manual CircleCI jobs.

## Changes

- `src-tauri/src/shell_env.rs`, `proc.rs`, `pty.rs`, `cli.rs`: `$SHELL -ilc env` applied to every CLI start; process groups and `kill_tree` on Unix; TERM for PTYs.
- `src-tauri/src/autostart.rs`, `lib.rs`, `projects.rs`, `tauri.macos.conf.json`: LaunchAgent / XDG autostart; notify-rust on Linux; tray fallback; Reopen; per-OS `norm`, Unix `decode_claude_dir`.
- `src/lib/platform.ts`, `TitleBar.svelte`, `Terminal.svelte`, `format.ts`, i18n: platform helper, paste key, folder example, `~` for /Users and /home.
- `scripts/ci/`, `e2e/linux/`, `.circleci/config.yml`, `.gitattributes`: checks script, Docker E2E (22 steps), CircleCI jobs gated by parameters.

## Decisions

- macOS notifications stay on the Tauri plugin (no click): notify-rust's macOS click path cannot be tried without a Mac.
- Login items are written directly (no tauri-plugin-autostart); CircleCI jobs run only when triggered (credits).

## Verification

- `bash scripts/ci/checks.sh` on Windows: pass (Vitest 212, cargo 123/5/12, clippy -D warnings clean).
- `bash e2e/linux/run.sh all`: checks pass on Linux; E2E 22/22 on the installed `.deb`.
- `circleci config validate`: valid.

## Limitations

- macOS has not been built or run; CircleCI is not connected to the repo yet, so no CI job has run.

## Follow-up

- Connect CircleCI, trigger `linux`, `windows`, `macos`; fix whatever the first macOS run reports.
