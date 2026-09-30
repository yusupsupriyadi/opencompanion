## What this changes

<!-- One or two sentences. Link the issue: "Closes #123". -->

## Why

## How I tested

- [ ] `bun run check`
- [ ] `bun run test`
- [ ] `cargo test --features dev-tools` (in `src-tauri`)
- [ ] `cargo clippy --all-targets --features dev-tools` with no warnings
- [ ] Tried it in the running app (`bun run tauri dev`)

OS:
CLIs and versions used, if any:

## Screenshots

<!-- For UI changes: Day and Dusk themes, phone width (390 px) if the phone page changed, and both languages. Delete this section otherwise. -->

## Checklist

- [ ] One logical change, with no unrelated refactors
- [ ] Tests added or updated for changed behavior
- [ ] New UI text is in `src/lib/i18n`, in English and Indonesian
- [ ] `README.md` and the Unreleased section of `CHANGELOG.md` are updated if a feature, limit or requirement changed
- [ ] No private paths, prompts, tokens or account details in code, tests or captured CLI output
