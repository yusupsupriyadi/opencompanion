# Move CLIs, Skills, Phone access and theme into Settings; list every session in the sidebar

## Summary

The owner asked to move the CLIs and Skills menus, the Phone access row and the Day/Dusk switch out of the sidebar into Settings, and to drop the sidebar's session limit (sessions vanished when new ones were added). Two caps caused it: the sidebar showed only pinned sessions plus the six most urgent, and the store loaded only the 80 newest.

## Changes

- `src/lib/SettingsHead.svelte`: shared Settings header, H1 plus tab links General / CLIs / Skills and a slot for the tab's action (Rescan).
- `src/routes/settings/clis`, `src/routes/settings/skills`: moved from `/clis` and `/skills`, now use the Settings header.
- `src/routes/settings/+page.svelte`: Settings header and a new Theme card (Day / Dusk radios).
- `src/lib/Sidebar.svelte`: nav without CLIs and Skills, no footer, no six-session cap.
- `src/app.css`: session list scrolls on its own so brand and nav stay put; `.seg a` tab styles; removed sidebar footer rules.
- `src/lib/store.svelte.ts`, `src-tauri/src/lib.rs`: `list_sessions` without a limit returns every session.
- `src/lib/i18n/*`: tab and theme strings, dropped unused sidebar keys, onboarding hint points at Settings › CLIs.
- `DESIGN.md`: Sidebar, D5, D7, D9, D11 updated.

## Decisions

- Tabs are links with `aria-current`, not a tablist, since each tab is its own route.
- Phone access stays the first card of the General tab; it was already there.

## Verification

- `bunx vitest run`: 36 files, 201 tests passed.
- `bun run check`: 0 errors, 0 warnings. `bun run build`: done. `cargo check`: finished.

## Limitations

- No manual run of the app (smoke test not requested). The phone companion's session list still returns 60.

## Follow-up

- none
