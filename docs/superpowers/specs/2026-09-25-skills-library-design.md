# Skills library: design

Approved in chat on 2026-09-25.

## Goal

Compare the skills installed for each coding CLI, so the owner can see which skill is missing from a CLI or has different content, and get a command to copy it. OpenCompanion reads the folders and never writes to them.

## Scope

- Sources: the user skill folders only. Plugin caches and project-level skill folders are out of scope.

| Root id | Path | Label |
|---|---|---|
| `claude` | `~/.claude/skills` | Claude Code |
| `codex` | `~/.codex/skills` | Codex CLI |
| `agents` | `~/.agents/skills` | Shared |
| `opencode` | `~/.config/opencode/skills` | OpenCode |
| `gemini` | `~/.gemini/skills` | Gemini CLI |

- Columns are folders, not CLIs: which CLI reads which folder depends on the CLI version, so the app does not guess. Environment overrides such as `CODEX_HOME` are not followed.
- Desktop only. No database, no settings, nothing on the phone.

## Backend (`src-tauri/src/skills.rs`, command `scan_skills`)

- Every direct subfolder of a root is a skill candidate, keyed by folder name (case-insensitive). Folders starting with `.` are skipped (for example Codex's bundled `.system`).
- A candidate with `SKILL.md` gets its `description` from the frontmatter (inline, quoted, and `>`/`|` block values), and a SHA-256 over every file in the folder: sorted relative paths plus contents. Links and junctions are followed; `.git` and `node_modules` are skipped. Over 5,000 files or 50 MB: `tooLarge`, no hash.
- Problems: `noSkillMd`, `brokenLink`, `unreadable`, `tooLarge`.
- Each distinct hash in a row gets a variant letter; `A` is the most recently modified copy. `modifiedAt` is the newest file time in the folder, in ms.
- `linkTarget` is set when the skill folder itself is a symlink or junction.
- Result: `{ shell: "powershell" | "sh", roots: [{ id, label, cli, path, exists }], skills: [{ name, description, variants, entries: [{ rootId, path, hash, variant, modifiedAt, linkTarget, problem }] }] }`, rows sorted by name.

## Frontend

- Nav item "Skills" after CLIs, Phosphor `books` icon. Route `/skills`, state local to the page, scanned on open and on Rescan.
- Summary counts, filter All / Different / Problems, search by name or description.
- Matrix table: skill column (name as a button, one-line description, "2 versions"), one column per root. Cells: "Same" or "Installed" with `check-circle`, "Version A/B" outline box plus date, "Missing", or an error chip for problems. Status is never color alone.
- Detail dialog: every root with its state, a "Copy from" select (default Version A), and for each root that lacks the skill or holds another version a command plus "Copy command".
  - Missing: `Copy-Item -LiteralPath '<src>' -Destination '<dest>' -Recurse`, prefixed with `New-Item -ItemType Directory -Force '<root>' | Out-Null;` when the root folder does not exist.
  - Different: `Remove-Item -LiteralPath '<dest>' -Recurse -Force; Copy-Item ...`, with a warning that files only in that copy are deleted.
  - No command when the destination is a link or holds one anywhere inside (`containsLinks`): `Remove-Item -Recurse` in Windows PowerShell 5.1 follows junctions and can delete the files they point to.
  - `sh`: `cp -R` and `rm -rf ... && cp -R ...`.
- States: loading, error with Try again, empty with the five paths, filter and search empty messages.

## Tests

- Rust: frontmatter parser, hash stability and change detection, skipped folders, variant order, problems, missing root.
- TS: command builder quoting and cases (`src/lib/skills.test.ts`).
- Component: matrix, filters, search, dialog commands, states (`src/routes/skills/skills.test.ts`).
