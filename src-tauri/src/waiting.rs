//! Text-pattern fallback for "Waiting for you" in interactive (PTY) sessions (PRD FR-16).
//! Claude Code has a reliable signal (hooks, see docs/spike/M0-results.md); these patterns are
//! for when hooks are unavailable. Only screens observed in the M0 spike are matched.

use serde::Serialize;

use crate::cli::CliKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WaitReason {
    Permission,
    TrustFolder,
    /// The CLI offers to update itself before starting. Enter picks "Update now" in Codex.
    UpdateOffer,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WaitingPrompt {
    pub reason: WaitReason,
    pub question: String,
}

pub fn detect(kind: CliKind, screen: &str) -> Option<WaitingPrompt> {
    let lines: Vec<&str> = screen.lines().map(str::trim).collect();
    let has = |needle: &str| lines.iter().any(|l| l.contains(needle));
    let found = |reason, question: &str| {
        Some(WaitingPrompt {
            reason,
            question: question.to_string(),
        })
    };
    match kind {
        CliKind::Claude | CliKind::Ccs => {
            const TRUST: &str = "Is this a project you created or one you trust?";
            if lines.iter().any(|l| l.contains(TRUST)) {
                return found(WaitReason::TrustFolder, TRUST);
            }
            if has("Esc to cancel") {
                if let Some(q) = lines.iter().rev().find(|l| l.starts_with("Do you want to")) {
                    return found(WaitReason::Permission, q);
                }
            }
            None
        }
        CliKind::Codex => {
            if has("Update available!") && has("Press enter to continue") {
                let q = lines.iter().find(|l| l.contains("Update available!")).unwrap_or(&"");
                return found(WaitReason::UpdateOffer, q.trim_start_matches('✨').trim());
            }
            None
        }
        CliKind::Cursor => cursor(&lines),
        CliKind::Opencode | CliKind::Gemini | CliKind::Pi | CliKind::Omp | CliKind::Terminal => None,
    }
}

/// The questions Cursor CLI's approval menu asks (`decision-logic.ts` in 2026.09.28-64d2043).
const CURSOR_QUESTIONS: [&str; 9] = [
    "Run this command?",
    "Run this command outside the sandbox?",
    "Run this MCP tool?",
    "Delete this file?",
    "Write to this file?",
    "Read this file?",
    "Allow this web search?",
    "Allow this web fetch?",
    "Proceed with this edit?",
];

/// Cursor CLI's workspace trust and MCP dialogs and its approval menu. The shell command menu is
/// matched against a screen Orca (stablyai/orca) recorded of cursor-agent 2026.08.11
/// (`cursor-agent-approval-prompt.txt`); the other questions and the dialogs are the same
/// components with the text from Cursor's source.
fn cursor(lines: &[&str]) -> Option<WaitingPrompt> {
    let has = |needle: &str| lines.iter().any(|l| l.contains(needle));
    const TRUST: &str = "Do you trust the contents of this directory?";
    if has("Workspace Trust Required") && has(TRUST) {
        return Some(WaitingPrompt { reason: WaitReason::TrustFolder, question: TRUST.into() });
    }
    const MCP: &str = "MCP Server Approval Required";
    if has(MCP) && has("Approve all servers") {
        return Some(WaitingPrompt { reason: WaitReason::Permission, question: MCP.into() });
    }
    // The menu owns the bottom of the screen while it waits, and is gone once answered. Each
    // choice ends with its key, such as `Run (once) (y)` or `Skip ... (esc or n)`.
    let shown: Vec<&str> = lines.iter().copied().filter(|l| !l.is_empty()).collect();
    let tail = &shown[shown.len().saturating_sub(8)..];
    let at = tail.iter().rposition(|l| CURSOR_QUESTIONS.contains(l))?;
    let choices = tail[at + 1..].iter().filter(|l| key_hint(l)).count();
    if choices < 2 || !tail.last().is_some_and(|l| key_hint(l)) {
        return None;
    }
    let question = tail[at];
    // A command sits above its question: `$  git status --porcelain in .`
    let command = tail[..at].iter().rev().find_map(|l| l.strip_prefix('$')).map(str::trim);
    let question = match command {
        Some(c) if !c.is_empty() && question.starts_with("Run this command") => format!("{question} {c}"),
        _ => question.to_string(),
    };
    Some(WaitingPrompt { reason: WaitReason::Permission, question })
}

/// A menu line that ends with the key that picks it: `(y)`, `(tab)`, `(shift+tab)`, `(esc or n)`.
fn key_hint(line: &str) -> bool {
    let Some(inner) = line.strip_suffix(')').and_then(|l| l.rsplit_once('(')).map(|(_, k)| k) else { return false };
    inner.split(" or ").all(|k| {
        ["tab", "shift+tab", "esc", "enter", "space"].contains(&k) || (k.len() == 1 && k.bytes().all(|b| b.is_ascii_lowercase()))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // Trimmed from screens captured in the M0 spike.
    const CLAUDE_PERMISSION: &str = "\
❯ Create a file named hello.txt containing the single word hi.
● Write(hello.txt)
 Create file
 hello.txt
  1 hi
 Do you want to create hello.txt?
 ❯ 1. Yes
   2. Yes, and switch to accept edits (auto-approve file edits and common file commands) for this session (shift+tab)
   3. No
 Esc to cancel · Tab to amend";

    const CLAUDE_TRUST: &str = "\
 Accessing workspace:
 C:\\work
 Quick safety check: Is this a project you created or one you trust? (Like your own code, a well-known open source
 ❯ No, exit
   Yes, I trust this folder
 Enter to confirm · Esc to cancel";

    const CODEX_UPDATE: &str = "\
  ✨ Update available! 0.153.4 -> 0.154.0
› 1. Update now
  2. Skip
  3. Skip until next version
  Press enter to continue";

    #[test]
    fn claude_permission_dialog() {
        let p = detect(CliKind::Claude, CLAUDE_PERMISSION).unwrap();
        assert_eq!(p.reason, WaitReason::Permission);
        assert_eq!(p.question, "Do you want to create hello.txt?");
    }

    #[test]
    fn claude_trust_dialog() {
        assert_eq!(detect(CliKind::Claude, CLAUDE_TRUST).unwrap().reason, WaitReason::TrustFolder);
    }

    #[test]
    fn codex_update_offer() {
        let p = detect(CliKind::Codex, CODEX_UPDATE).unwrap();
        assert_eq!(p.reason, WaitReason::UpdateOffer);
        assert_eq!(p.question, "Update available! 0.153.4 -> 0.154.0");
    }

    // Recorded by Orca (stablyai/orca, MIT) from cursor-agent 2026.08.11, trimmed.
    const CURSOR_APPROVAL: &str = "\
  I'll run git status --porcelain in the workspace now.

  $ git status --porcelain Waiting for approval...

────────────────────────────────────────────────────────────
 $  git status --porcelain in .

 Run this command?
 Not in allowlist: git status
  → Run (once) (y)
    Add Shell(git status) to allowlist? (tab)
    Run Everything (shift+tab)
    Skip & tell the agent what to do instead (esc or n)";

    const CURSOR_IDLE: &str = "\
  $ sleep 60 1m 14s

  sleep 60 finished successfully (exit code 0) after about 60 seconds.




  → Add a follow-up


  Auto · 6.3%
  /private/tmp/sta4513/live-cursor-trust";

    const CURSOR_TRUST: &str = "\
  ⚠ Workspace Trust Required
  Do you trust the contents of this directory?
  ▶ [a] Trust this workspace
    [q] Quit";

    #[test]
    fn cursor_approval_menu_and_trust_dialog() {
        let p = detect(CliKind::Cursor, CURSOR_APPROVAL).unwrap();
        assert_eq!(p.reason, WaitReason::Permission);
        assert_eq!(p.question, "Run this command? git status --porcelain in .");
        assert_eq!(detect(CliKind::Cursor, CURSOR_TRUST).unwrap().reason, WaitReason::TrustFolder);
        assert_eq!(detect(CliKind::Cursor, CURSOR_IDLE), None);
        // The agent quoting the menu in its reply is not the menu.
        let quoted = format!("{CURSOR_APPROVAL}\n\n  I answered it.\n\n  → Add a follow-up");
        assert_eq!(detect(CliKind::Cursor, &quoted), None);
        let write = " Write to this file?\n src/app.ts\n  → Proceed (y)\n    Reject & propose changes (esc or n or p)\n    Add Write(src/app.ts) to allowlist? (tab)";
        assert_eq!(detect(CliKind::Cursor, write).unwrap().question, "Write to this file?");
        let mcp = "  MCP Server Approval Required\n  The following MCP servers need to be approved:\n  ▶ [a] Approve all servers\n    [c] Continue without approval\n    [q] Quit";
        assert_eq!(detect(CliKind::Cursor, mcp).unwrap().question, "MCP Server Approval Required");
        assert!(!key_hint("Run it (once)") && key_hint("Run Everything (shift+tab)"));
    }

    #[test]
    fn idle_prompt_is_not_waiting() {
        let idle = "❯\n  Opus 5.5 │ xhigh\n  ⏸ manual mode on";
        assert_eq!(detect(CliKind::Claude, idle), None);
        assert_eq!(detect(CliKind::Codex, CLAUDE_PERMISSION), None);
    }
}
