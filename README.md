<p align="center">
  <img src="design/logo.svg" width="112" height="112" alt="OpenCompanion logo: a pixel hiker on a green meadow hill">
</p>

<h1 align="center">OpenCompanion</h1>

<p align="center">A desktop app that starts, watches and answers AI coding CLIs on your computer, with a phone companion on your own network.</p>

<p align="center">
  <a href="https://github.com/yusupsupriyadi/opencompanion/releases">Download</a> ·
  <a href="docs/features.md">Features</a> ·
  <a href="docs/clis.md">Supported CLIs</a> ·
  <a href="CONTRIBUTING.md">Contributing</a>
</p>

<p align="center">
  <a href="https://www.producthunt.com/products/opencompanion?embed=true&utm_source=badge-featured&utm_medium=badge&utm_campaign=badge-opencompanion">
    <picture>
      <source media="(prefers-color-scheme: dark)" srcset="https://api.producthunt.com/widgets/embed-image/v1/featured.svg?post_id=1265167&theme=dark">
      <img src="https://api.producthunt.com/widgets/embed-image/v1/featured.svg?post_id=1265167&theme=light" width="250" height="54" alt="OpenCompanion on Product Hunt">
    </picture>
  </a>
</p>

<!-- Replace the line below with a screenshot or a short GIF, for example docs/media/overview.png. -->
> Screenshot placeholder: the Overview with one session waiting for you.

Run Claude Code, Codex CLI or OpenCode in several folders at once and each session stops now and then to ask for permission, which you only notice when you look at its terminal. OpenCompanion starts those sessions, lists them on one screen, tells you when one is waiting, and lets you answer from the desktop or your phone.

It runs on your computer, with no account, no cloud server and no telemetry. Each CLI talks to its own provider with your own login, as it does in your terminal. Windows, Linux and macOS, in English or Indonesian.

## Features

- Start a CLI in a project folder, in a real terminal or headless, and follow every session on one screen.
- See when a session waits for you. Claude Code permission prompts get Approve and Deny, and OS notifications can be set per CLI and per folder.
- Answer from your phone on the same network: watch sessions, approve, start new ones and send messages.
- Describe work in Chat. A planner turns it into one card per session, which starts when you press Run.
- Schedule sessions in Automations, from a daily time to a cron expression.
- Open shells in the session's folder beside the CLI, up to four split in one tab.
- Browse the session's folder, read its files, see the uncommitted diff and switch branches.
- Pick a permission mode per session (Ask me, Plan, Auto or Bypass), and keep sessions running from the tray.

Each one is described in full in [docs/features.md](docs/features.md).

## Supported CLIs

| | Claude Code | Codex CLI | OpenCode | Gemini CLI | CCS | Pi | omp | Cursor CLI |
|---|---|---|---|---|---|---|---|---|
| Terminal session | Yes | Yes | Yes | Untested | Yes | Yes | Yes | Untested |
| Headless session | Yes | Yes | Yes | No | Some profiles | Yes | Yes | Untested |
| Approve/Deny here and on the phone | Yes | No | No | No | Yes | No | No | No |
| Chat planner | Yes | Yes | Yes | No | Some profiles | Yes | Yes | Untested |

OpenCompanion finds each CLI on your PATH and never installs one. [docs/clis.md](docs/clis.md) has the tested versions, the reason behind each "No" and the flags each permission mode sets.

## Download

Installers are on the [Releases page](https://github.com/yusupsupriyadi/opencompanion/releases): `.msi` or `.exe` for Windows, `.deb`, `.rpm` or AppImage for Linux, and `.dmg` for macOS on Apple Silicon or Intel. They are not code-signed yet, so the first start asks once: on Windows choose More info, then Run anyway; on macOS open System Settings › Privacy & Security and choose Open Anyway.

Windows 11 is tested by hand, Linux end to end in Docker, and macOS in CI only. [docs/platforms.md](docs/platforms.md) lists what differs per OS.

## Build from source

You need Rust (stable), the [Tauri 2 prerequisites](https://tauri.app/start/prerequisites/) for your OS, [Bun](https://bun.sh), and [Node.js](https://nodejs.org) to run the tests.

```sh
git clone https://github.com/yusupsupriyadi/opencompanion.git
cd opencompanion
bun install
bun run tauri dev      # desktop app with hot reload
bun run tauri build    # installer in src-tauri/target/release/bundle
```

Install and sign in to at least one supported CLI. On first start, Onboarding lists the CLIs it found; it installs and changes nothing.

## Phone companion

Phone access stays off until you turn it on in Settings › Phone access. Then scan the QR code with a phone on the same Wi-Fi, or open the address shown and type the 6-digit code. The connection is plain HTTP on your local network, so away from home use a VPN such as Tailscale. [docs/phone.md](docs/phone.md) covers pairing, devices and the home screen app.

## Documentation

- [Features](docs/features.md), [Supported CLIs and permission modes](docs/clis.md), [Platforms](docs/platforms.md) and [Phone companion](docs/phone.md)
- [Privacy and data](docs/privacy.md): what leaves your computer and where OpenCompanion keeps its data
- [Status, known limits and roadmap](docs/status.md)
- [Project layout](docs/project-layout.md) and [CONTRIBUTING.md](CONTRIBUTING.md) for development, checks and releases
- [CHANGELOG.md](CHANGELOG.md), plus [DESIGN.md](DESIGN.md), the [PRD](docs/PRD.md) and the [M0 spike results](docs/spike/M0-results.md), which are written in Indonesian

## Contributing and security

Bug reports, fixes and ideas are welcome. Read [CONTRIBUTING.md](CONTRIBUTING.md) and the [Code of Conduct](CODE_OF_CONDUCT.md) before you open a pull request. Report security problems privately, as [SECURITY.md](SECURITY.md) explains, not in a public issue.

## License

[MIT](LICENSE) © 2026 Yusup Supriyadi.

Claude Code, Codex CLI, OpenCode, Gemini CLI, CCS, Pi, omp and Cursor CLI are products of their respective owners. OpenCompanion is an independent project and is not affiliated with or endorsed by them.
