//! What a CLI says about its own state outside Claude Code's hooks: Codex's OSC 9 notifications,
//! Gemini CLI's window title and OpenCode's plugin events, each read from the CLI's source
//! (Codex 0.153.4, Gemini CLI 0.62.0, OpenCode 1.18.32). Orca (stablyai/orca) reads Gemini CLI's
//! title and OpenCode's events the same way; for Codex it installs hooks, which Codex runs only
//! once you trust them, so the notifications Codex already sends are read here instead.

use serde_json::Value;

use crate::cli::CliKind;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Said {
    /// It asks you something. `reason` as in `db::Waiting::reason`.
    Asking { reason: &'static str, detail: String },
    Working,
    /// Its turn is over and it waits for the next message.
    Done,
}

/// The longest OSC body kept waiting for its end; a longer one is dropped.
const OSC_MAX: usize = 4096;

/// The bodies of OSC sequences (`ESC ] body BEL` or `ESC ] body ESC \`) in terminal output. A
/// sequence cut off at the end of `chunk` waits in `carry` for the next one.
pub fn osc_bodies(carry: &mut String, chunk: &str) -> Vec<String> {
    let joined;
    let text = if carry.is_empty() {
        chunk
    } else {
        joined = std::mem::take(carry) + chunk;
        joined.as_str()
    };
    let mut bodies = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find("\x1b]") {
        let body = &rest[start + 2..];
        let Some(end) = body.find(['\x07', '\x1b']) else {
            if body.len() <= OSC_MAX {
                *carry = rest[start..].to_string();
            }
            break;
        };
        bodies.push(body[..end].to_string());
        rest = &body[end + 1..];
    }
    bodies
}

/// Codex's config for OSC 9 notifications on its turn's end and on its questions, whatever the
/// terminal. Bare values: Codex reads a value that is not TOML as a string, and Windows
/// PowerShell drops double quotes from the arguments of a program it starts again.
pub const CODEX_NOTICES: [&str; 6] = [
    "-c",
    "tui.notifications=true",
    "-c",
    "tui.notification_method=osc9",
    "-c",
    "tui.notification_condition=always",
];

/// The CLI says when it starts working, so its turn ends only after that. Codex tells only
/// questions and the end of a turn.
pub fn says_working(kind: CliKind) -> bool {
    matches!(kind, CliKind::Gemini | CliKind::Opencode)
}

/// What an OSC body from a CLI's terminal says, and how it was said.
pub fn from_osc(kind: CliKind, body: &str) -> Option<(Said, &'static str)> {
    match kind {
        CliKind::Codex => codex_notice(body.strip_prefix("9;")?).map(|said| (said, "osc9")),
        CliKind::Gemini => {
            let title = body.strip_prefix("0;").or_else(|| body.strip_prefix("2;"))?;
            gemini_title(title).map(|said| (said, "title"))
        }
        _ => None,
    }
}

/// Codex's notification text (`tui/src/chatwidget/notifications.rs`): its questions start with
/// fixed words, and the end of a turn shows the start of its reply.
fn codex_notice(message: &str) -> Option<Said> {
    // ConEmu's OSC 9 subcommands, such as `4;1;50` for progress, start with a number.
    if message.split_once(';').is_some_and(|(n, _)| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit())) {
        return None;
    }
    let message = message.trim();
    let asking = |reason| Some(Said::Asking { reason, detail: message.to_string() });
    if message.starts_with("Approval requested") || message.starts_with("Codex wants to edit ") {
        asking("permission")
    } else if message.starts_with("Plan mode prompt:") {
        asking("question")
    } else {
        Some(Said::Done)
    }
}

/// Gemini CLI's dynamic title (`cli/src/utils/windowTitle.ts`): a glyph for its state, padded
/// to 80 characters.
fn gemini_title(title: &str) -> Option<Said> {
    let mut chars = title.trim().chars();
    let glyph = chars.next()?;
    match glyph {
        '✋' => Some(Said::Asking { reason: "permission", detail: chars.as_str().trim().to_string() }),
        '◇' => Some(Said::Done),
        '✦' | '⏲' => Some(Said::Working),
        _ => None,
    }
}

/// A line OpenCode's plugin (`OPENCODE_PLUGIN`) wrote to the session's hook file.
pub fn from_opencode(line: &str) -> Option<Said> {
    let v: Value = serde_json::from_str(line).ok()?;
    match v["said"].as_str()? {
        "asking" => Some(Said::Asking {
            reason: if v["reason"] == "question" { "question" } else { "permission" },
            detail: v["detail"].as_str().unwrap_or_default().to_string(),
        }),
        "working" => Some(Said::Working),
        "done" => Some(Said::Done),
        _ => None,
    }
}

/// OpenCode loads this from `plugins/` in `OPENCODE_CONFIG_DIR`, a folder read after the
/// user's own. It appends what the session does to `OPENCOMPANION_HOOK_FILE`, one JSON line
/// each; subagents' sessions finishing are left out.
pub const OPENCODE_PLUGIN: &str = r#"// Written by OpenCompanion, which reads what this OpenCode session does from the lines below.
import { appendFileSync } from "node:fs";

export const OpenCompanion = async () => {
  const file = process.env.OPENCOMPANION_HOOK_FILE;
  if (!file) return {};
  const subagents = new Set();
  const say = (line) => {
    try {
      appendFileSync(file, JSON.stringify(line) + "\n");
    } catch {}
  };
  return {
    event: async ({ event }) => {
      const p = event.properties ?? {};
      switch (event.type) {
        case "session.created":
        case "session.updated":
          if (p.info?.parentID) subagents.add(p.info.id);
          break;
        case "permission.asked": {
          const patterns = Array.isArray(p.patterns) ? p.patterns.join(", ") : "";
          say({ said: "asking", reason: "permission", detail: [p.permission, patterns].filter(Boolean).join(": ") });
          break;
        }
        case "question.asked":
          say({ said: "asking", reason: "question", detail: p.questions?.[0]?.question ?? "" });
          break;
        case "permission.replied":
        case "question.replied":
        case "question.rejected":
          say({ said: "working" });
          break;
        case "session.status":
          if (p.status?.type === "busy") say({ said: "working" });
          break;
        case "session.idle":
          if (!subagents.has(p.sessionID)) say({ said: "done" });
          break;
      }
    },
  };
};
"#;

#[cfg(test)]
mod tests {
    use super::*;

    fn asking(reason: &'static str, detail: &str) -> Said {
        Said::Asking { reason, detail: detail.into() }
    }

    #[test]
    fn osc_bodies_end_with_bel_or_st_and_a_cut_one_waits_for_the_next_chunk() {
        let mut carry = String::new();
        let got = osc_bodies(&mut carry, "a\x1b]9;Agent turn complete\x07b\x1b]0;◇  Ready (w)\x1b\\c\x1b]9;Appro");
        assert_eq!(got, ["9;Agent turn complete", "0;◇  Ready (w)"]);
        assert_eq!(osc_bodies(&mut carry, "val requested: npm test\x07"), ["9;Approval requested: npm test"]);
        assert!(carry.is_empty());
        // A sequence that never ends is not kept forever.
        osc_bodies(&mut carry, &format!("\x1b]9;{}", "x".repeat(10_000)));
        assert!(carry.is_empty());
    }

    #[test]
    fn codex_notifications_tell_a_question_from_the_end_of_a_turn() {
        let codex = |body: &str| from_osc(CliKind::Codex, body);
        assert_eq!(codex("9;Approval requested: npm test"), Some((asking("permission", "Approval requested: npm test"), "osc9")));
        assert_eq!(codex("9;Codex wants to edit src/app.ts"), Some((asking("permission", "Codex wants to edit src/app.ts"), "osc9")));
        assert_eq!(codex("9;Approval requested by github"), Some((asking("permission", "Approval requested by github"), "osc9")));
        assert_eq!(codex("9;Plan mode prompt: Pick a plan"), Some((asking("question", "Plan mode prompt: Pick a plan"), "osc9")));
        // Anything else is the preview of Codex's reply.
        assert_eq!(codex("9;Done. The tests pass now."), Some((Said::Done, "osc9")));
        // ConEmu's OSC 9 subcommands, such as progress, and titles are not notifications.
        assert_eq!(codex("9;4;1;50"), None);
        assert_eq!(codex("0;codex"), None);
    }

    #[test]
    fn gemini_titles_say_whether_it_works_waits_or_asks() {
        let gemini = |body: &str| from_osc(CliKind::Gemini, body);
        let pad = |t: &str| format!("0;{t:<80}");
        assert_eq!(gemini(&pad("✋  Action Required (uninote)")), Some((asking("permission", "Action Required (uninote)"), "title")));
        assert_eq!(gemini(&pad("◇  Ready (uninote)")), Some((Said::Done, "title")));
        assert_eq!(gemini(&pad("✦  Reading the tests (uninote)")), Some((Said::Working, "title")));
        assert_eq!(gemini(&pad("⏲  Working… (uninote)")), Some((Said::Working, "title")));
        // With dynamic titles off, the title says nothing about the state.
        assert_eq!(gemini(&pad("Gemini CLI (uninote)")), None);
        assert_eq!(gemini("9;Agent turn complete"), None);
        assert_eq!(from_osc(CliKind::Claude, &pad("◇  Ready (uninote)")), None);
    }

    #[test]
    fn opencode_plugin_lines_become_what_the_session_does() {
        assert_eq!(from_opencode(r#"{"said":"asking","reason":"permission","detail":"bash: npm test"}"#), Some(asking("permission", "bash: npm test")));
        assert_eq!(from_opencode(r#"{"said":"asking","reason":"question","detail":"Which database?"}"#), Some(asking("question", "Which database?")));
        assert_eq!(from_opencode(r#"{"said":"asking","reason":"anything","detail":""}"#), Some(asking("permission", "")));
        assert_eq!(from_opencode(r#"{"said":"working"}"#), Some(Said::Working));
        assert_eq!(from_opencode(r#"{"said":"done"}"#), Some(Said::Done));
        assert_eq!(from_opencode(r#"{"said":"later"}"#), None);
        assert_eq!(from_opencode("not json"), None);
    }

    #[test]
    fn the_opencode_plugin_reads_its_file_from_the_environment_and_skips_subagents() {
        assert!(OPENCODE_PLUGIN.contains("process.env.OPENCOMPANION_HOOK_FILE"));
        for event in ["permission.asked", "question.asked", "permission.replied", "question.replied", "question.rejected", "session.status", "session.idle", "parentID"] {
            assert!(OPENCODE_PLUGIN.contains(event), "{event}");
        }
    }
}
