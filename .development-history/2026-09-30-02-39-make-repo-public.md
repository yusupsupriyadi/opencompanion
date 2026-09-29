# Make the repository public

## Summary

Audited the full history before publishing, rewrote commit emails to the GitHub noreply address, made `yusupsupriyadi/opencompanion` public and turned on its security features.

## Changes

- History: all 102 commits rewritten with `git filter-branch --env-filter` (author and committer email only), force-pushed to `main`. Final tree unchanged (`72a9427`), dates kept.
- `.git/config`: `user.email` set to `57658483+yusupsupriyadi@users.noreply.github.com` so new commits keep the private address.
- GitHub: visibility public; private vulnerability reporting (used by `SECURITY.md`), Dependabot alerts, secret scanning and push protection enabled.
- `.development-history/` (3 reports) and `docs/superpowers/specs/2026-09-28-cross-platform-parity-design.md`: old short SHAs replaced with their rewritten ones.

## Decisions

- Rewrite done in a scratchpad clone, then local `main` moved with `git reset --soft` so other agents' uncommitted work stayed untouched.
- Internal notes (`.development-history/`, `docs/superpowers/`) stay public; they hold no secrets.

## Verification

- Regex scan of every added line in history: no real keys, tokens, private keys or `.env` files (only the `sk-secret-key` test fixture).
- `gh api repos/.../commits`: noreply email; anonymous API request returns 200; `security_and_analysis` shows secret scanning and push protection enabled; PVR `{"enabled":true}`; vulnerability alerts 204.

## Limitations

- GitHub may still serve the old commits by direct SHA until it garbage-collects them; GitHub Support can purge them on request.
- README still has the screenshot placeholder.

## Follow-up

- Add a screenshot, a repo description and topics.
