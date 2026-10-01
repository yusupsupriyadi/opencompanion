# Contributing to OpenCompanion

Bug reports, fixes, translations and ideas are all welcome. This page covers how to set up the project, which checks to run, and what a pull request needs.

By taking part you agree to follow the [Code of Conduct](CODE_OF_CONDUCT.md). Security problems go through [SECURITY.md](SECURITY.md), never through a public issue.

## Before you write code

- For a bug, open an issue with the bug report form, including your OS, the CLI and its version, and what you saw.
- For a new feature or a larger change, open a feature request first, so the shape is agreed before you spend time on it.
- Small fixes (typos, a clear bug with an obvious fix) can go straight to a pull request.

## Set up

You need Rust (stable, with the MSVC toolchain on Windows), the [Tauri 2 prerequisites](https://tauri.app/start/prerequisites/) for your platform, [Bun](https://bun.sh), and [Node.js](https://nodejs.org) (LTS). Vitest runs on Node: with Bun alone, `bun run test` cannot start its test workers.

```sh
git clone https://github.com/yusupsupriyadi/opencompanion.git
cd opencompanion
bun install
bun run tauri dev
```

`bun run tauri dev` starts Vite on port 1420 and opens the desktop window with hot reload. `bun run tauri build` writes an installer to `src-tauri/target/release/bundle`.

You can work on most screens and run every test without any AI CLI installed. To try real sessions, install at least one of Claude Code, Codex CLI or OpenCode and sign in to it. Real sessions use your own account and quota.

Windows 11 is tested by hand, Linux end to end in Docker (see below), and macOS only through CI. If you run OpenCompanion on macOS or on a Linux desktop, say so in your issue or pull request; reports from those platforms are especially useful.

## Checks

Run all four before you open a pull request:

```sh
bun run check                                     # svelte-check: types in .svelte and .ts files
bun run test                                      # Vitest + Testing Library in jsdom, backend calls mocked
cd src-tauri
cargo test --features dev-tools                   # Rust unit tests and the integration tests in src-tauri/tests
cargo clippy --all-targets --features dev-tools   # keep it at zero warnings
```

`bash scripts/ci/checks.sh` runs the four in order (with `clippy -- -D warnings`), on any OS; on Windows run it from Git Bash.

`--features dev-tools` builds `fake-cli` and `air-spike` (in `src-tauri/src/bin`) and the integration tests that run `fake-cli`. `tauri build` leaves the feature off, so neither tool ends up in an installer.

`bun run test` needs no Tauri runtime: the component tests mock the backend. `cargo test` runs the unit tests in each module plus `src-tauri/tests/manager.rs` (the session manager) and `src-tauri/tests/companion.rs` (the phone API and WebSocket).

### Tests that need no real CLI

The integration tests start `fake-cli` (`src-tauri/src/bin/fake-cli.rs`) in place of a real CLI. It prints the event formats captured from Claude Code, Codex CLI and OpenCode during the M0 spike:

| Invocation | Behaves like |
|---|---|
| `fake-cli run --format json --dir D PROMPT` | an OpenCode headless turn |
| `fake-cli -p ...` | a Claude Code stream-json turn with one permission request |
| `fake-cli [PROMPT]` | an interactive CLI: prints a prompt, echoes one line (also as the terminal title), exits |

So `cargo test` needs no CLI, no login and no quota, and it gives the same result on every machine. If you change how a CLI's output is parsed in `events.rs`, update `fake-cli` so the tests cover the new format.

### Linux end to end, in Docker

`e2e/linux/` builds an Ubuntu 24.04 image with the Tauri prerequisites, WebKitWebDriver and a virtual display, copies your working tree into a container, and tests there. It needs only Docker (Docker Desktop on Windows, from Git Bash):

```sh
bash e2e/linux/run.sh checks   # scripts/ci/checks.sh on Linux
bash e2e/linux/run.sh e2e      # build and install the .deb, then drive the app
bash e2e/linux/run.sh all      # both
```

The end-to-end run starts the installed app under Xvfb and drives it through `tauri-driver` with `e2e/linux/e2e.py` (standard library only). `claude` and `opencode` are wrappers around `fake-cli` that only the login shell's PATH holds, so the run also proves the app picks up that PATH. It covers onboarding, headless and interactive sessions, Approve, a shell tab in a session with Ctrl+Shift+V paste, All sessions, language and theme, start at login, phone pairing, a click on a notification (through `dunst`), a restart, and a start with no tray library. Screenshots, `results.json` and logs land in `e2e/linux/out/`.

### CI

GitHub Actions runs the same scripts (`.github/workflows/ci.yml`) on every push to `main` and on every pull request: `scripts/ci/checks.sh` on Ubuntu 24.04, Windows and macOS, and the Linux end-to-end run in Docker. The end-to-end screenshots, `results.json` and logs are attached to the run as the `linux-e2e` artifact. To run CI on another branch, open Actions, choose CI and press Run workflow, or from a terminal:

```sh
gh workflow run ci.yml --ref my-branch
```

### Checking against the real CLIs

`src-tauri/src/bin/air-spike.rs` exercises the Rust core from a terminal, without the webview. It uses real CLIs, so it uses your quota:

```sh
cd src-tauri
cargo run --features dev-tools --bin air-spike -- detect
cargo run --features dev-tools --bin air-spike -- headless claude --cwd <scratch folder> --prompt "Create hello.txt" --out run.jsonl --answer deny
cargo run --features dev-tools --bin air-spike -- replay claude run.jsonl
```

The comment at the top of `air-spike.rs` lists every subcommand (`detect`, `scan`, `pty`, `headless`, `replay`, `waiting`, `plan`). Use an empty scratch folder outside the repository, and do not commit captured output that contains your paths, prompts or account details.

## Code style

- Match the code around you. The Rust code is not run through rustfmt yet, and the frontend has no formatter set up, so do not reformat files you are not otherwise changing.
- Svelte 5 with runes and TypeScript on the frontend; Rust 2021 edition in `src-tauri`.
- Comments explain why something is done, not what the next line does.
- Every string a person reads lives in `src/lib/i18n/*.ts`, in both English and Indonesian. Backend messages are written in English in Rust and translated by pattern in `src/lib/i18n/backend.ts`; add a pattern there when you add a new message.
- Copy follows the voice in `DESIGN.md` section 13: name the CLI and the folder ("Codex CLI is waiting for you in ai-remote"), let buttons say their action ("Run in uninote"), and use no em dashes, buzzwords or decorative emoji.
- UI follows `DESIGN.md`: its colour tokens in both the Day and Dusk themes, Phosphor icons, a visible focus ring, controls reachable by keyboard, and status that is never shown by colour alone.

A few product rules hold everywhere, and changes that break them will not be merged:

- OpenCompanion never installs a CLI. It shows the command for the user to run.
- It never writes to a CLI's own folders (settings, history, skills). Claude Code hooks are passed per session with `--settings` from the app data folder.
- It never answers a CLI's opening dialogs, such as a folder trust question or an update offer, by itself.
- Chat cards that start without Run never use Bypass mode.
- The phone companion stays off until the user turns it on.

## Commits and pull requests

- Write commit messages in English, as [Conventional Commits](https://www.conventionalcommits.org/): `feat(phone): ...`, `fix(session): ...`, `docs(readme): ...`, `test(...)`, `refactor(...)`, `chore(...)`.
- Keep one logical change per pull request, and leave unrelated refactors for a separate one.
- Add or update tests when behavior changes.
- In the pull request, say which checks you ran and on which OS.
- For UI changes, add screenshots in the Day and Dusk themes. If the phone page changes, add one at phone width (390 px). Check both languages.
- Update `README.md` when a feature, limit or requirement changes, and add a line under Unreleased in `CHANGELOG.md`.

By contributing, you agree that your contribution is licensed under the [MIT License](LICENSE).

## Releasing

GitHub Actions builds each release from a version tag (`.github/workflows/release.yml`). To cut one:

1. Set the new version in `package.json`, `src-tauri/tauri.conf.json` and `src-tauri/Cargo.toml`, then run `cargo check` in `src-tauri` so `Cargo.lock` follows. The workflow stops if any of the three differs from the tag.
2. Move the Unreleased entries in `CHANGELOG.md` under the new version, and commit.
3. Tag that commit and push the tag:

```sh
git tag v0.2.0
git push origin v0.2.0
```

The workflow runs CI first. Then it builds on Windows (`.msi`, `.exe`), Ubuntu 22.04 (`.deb`, `.rpm`, AppImage) and macOS (`.dmg`, for Apple Silicon and for Intel), and uploads everything to a draft release. Check the assets, then publish the draft from the Releases page. Installed apps see the new version only after you publish it, because the update check reads `releases/latest/download/latest.json`.

The in-app updater only installs files signed with the updater key. The release build adds `--config src-tauri/tauri.updater.conf.json`, which turns on the signed updater files: `latest.json`, which holds every signature, and a `.app.tar.gz` for each macOS build. The key and its password come from the `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` repository secrets. The public key is `plugins.updater.pubkey` in `tauri.conf.json`. Keep a backup of the private key: if it is lost, installed apps cannot accept another update, and every user has to download a build signed with a new key by hand. Local builds and CI leave the updater files off, so they need no key.

The builds are not code-signed, so Windows SmartScreen and macOS Gatekeeper ask before the first start. The macOS builds are ad-hoc signed (`signingIdentity` in `src-tauri/tauri.macos.conf.json`); without that, macOS on Apple Silicon reports a downloaded app as damaged.
