# PRD and design spec for AI Remote

## Summary

Wrote the product requirements and the design direction for AI Remote, a local Tauri 2 + Svelte desktop app that runs, dispatches tasks to, and monitors AI coding CLIs, with a LAN web companion for phones. Started the Pencil design; the owner will finish the remaining screens from DESIGN.md.

## Changes

- `docs/PRD.md`: goals, CLI adapter table verified from local `--help` (Claude Code 2.1.282, Codex CLI 0.153.4, OpenCode 1.18.30), FR tables with acceptance criteria, architecture, security, milestones, risks.
- `DESIGN.md`: palette with verified contrast, typography (Nunito, IBM Plex Mono, Pixelify Sans), components, horizon motif, illustration prompts, specs for D1-D9 and M1-M4.
- `design/ai-remote.pen`: tokens, components, screens D1-D6. `design/meadow-day.png`: generated illustration.

## Decisions

- Chat planner runs an installed CLI headless; no API keys, no cloud backend.
- External CLI sessions are shown read-only (process list + transcript files).

## Verification

- `contrast-check.py` on every text pairing: all pass AA (lowest 5.28:1).
- Pencil CLI headless export of D1-D6 reviewed visually; dash scan on both docs: none.

## Limitations

- Pencil MCP screenshot renderer did not draw newly created nodes; verification used `pen interactive` headless export instead.
- D2/D6 session rows lost two status chip overrides (psikotes, sepulangkerja.id); documented in DESIGN.md section 9.

## Follow-up

- Build D7-D9, M1-M4 and two illustrations in Pencil; run M0 spike from the PRD.
