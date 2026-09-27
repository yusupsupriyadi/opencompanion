# Security policy

## Reporting a vulnerability

Please report security problems privately, not in a public issue, pull request or discussion.

Use GitHub's private vulnerability reporting: open the repository's **Security** tab and press **Report a vulnerability**, or go straight to <https://github.com/yusupsupriyadi/opencompanion/security/advisories/new>. Only the maintainer can read the report.

A useful report includes:

- the OpenCompanion version or commit, and your OS;
- the steps to reproduce, and what an attacker gains;
- where the attacker has to be (same Wi-Fi, same computer, a paired phone);
- a proof of concept, if you have one.

You will get an answer in the advisory thread. Please keep the details private until a fix is released, and say if you would like to be credited in the advisory.

## Supported versions

OpenCompanion is at 0.x and has no stable release line yet. Security fixes go into the `main` branch and the next release.

## What OpenCompanion is

OpenCompanion is a desktop app that starts AI coding CLIs (Claude Code, Codex CLI, OpenCode) under your own user account. Those CLIs can read and change files and run commands, as far as the permission mode you pick allows. OpenCompanion's job is to make sure that only you, at the computer or on a phone you paired, can start them, answer them or type into them.

## The phone companion

The phone companion is the part of OpenCompanion that listens on the network, so most of the threat model is about it.

How it works:

- It is off by default. Nothing listens on the network until you press Turn on phone access in Settings.
- When on, it serves HTTP and a WebSocket on all network interfaces, on port 8765 unless you pick another one.
- A phone pairs with a one-time 6-digit code shown on the desktop. The code expires after 2 minutes, and 5 wrong tries throw it away, so a new code has to be shown.
- A paired phone gets a random 64-character device token. The desktop stores only its SHA-256 hash. The phone keeps the token in the page's local storage and sends it as a `Bearer` header, and as a query parameter when it opens the WebSocket.
- Without a token, the server answers only the phone page's static files, `/api/hello` (the app name and its UI language) and `/api/pair`. Everything else returns 401.
- Removing a device in Settings › Phone access closes its connection and makes its token useless. Turning phone access off stops the server and closes every connection.

What a paired phone can do: everything the desktop can do with sessions. It can start any supported CLI in any folder on the computer, in any permission mode including Bypass, type into a running terminal, answer permission prompts, stop and resume sessions, and use Chat and the Board. Treat a paired phone like your keyboard.

What it protects against:

- Someone on your network who has never paired: they cannot read sessions or act on them without a device token, and guessing a pairing code is limited to 5 tries per code while a code is on screen.
- A lost or retired phone: remove it in Settings and its token stops working.

What it does not protect against:

- Anyone who can read your network traffic. The connection is plain HTTP, so on a shared or untrusted network someone who can see the packets can read session content and capture a pairing code or a device token, and then act as your phone. Use phone access only on a network you trust, or reach your computer through a VPN with HTTPS such as Tailscale. Turn phone access off when you do not need it.
- Anyone who can unlock the paired phone and open its browser.

## Out of scope

- Someone with access to your user account on the computer. They can already read the app data folder (`opencompanion.db`, terminal logs, and the API key of a custom Chat planner endpoint, which is stored unencrypted) and each CLI's own credentials.
- What a CLI does with the permissions you give it, including prompt injection through files it reads. Report those to the CLI's vendor. A bug where OpenCompanion starts a CLI with more permission than you picked is in scope.
- The providers the CLIs talk to, and a custom Chat planner endpoint you configure yourself.

## In scope, for example

- Reaching any phone API or the WebSocket without a valid device token.
- Getting a device token without the pairing code, or getting around the attempt limit.
- A removed device still getting in.
- OpenCompanion writing to a CLI's own folders, installing something, or answering a CLI dialog without the user.
- A session starting in a more permissive mode than the one chosen, or a Board card starting in Bypass mode on its own.
- Script injection in the desktop window or the phone page from CLI output, file names or transcripts.
