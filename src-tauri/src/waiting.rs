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
        CliKind::Claude => {
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
        CliKind::Opencode | CliKind::Gemini => None,
    }
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

    #[test]
    fn idle_prompt_is_not_waiting() {
        let idle = "❯\n  Opus 5.5 │ xhigh\n  ⏸ manual mode on";
        assert_eq!(detect(CliKind::Claude, idle), None);
        assert_eq!(detect(CliKind::Codex, CLAUDE_PERMISSION), None);
    }
}
