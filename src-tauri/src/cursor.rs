//! Cursor CLI (`cursor-agent`, checked against 2026.09.28-64d2043). The `-p --output-format
//! stream-json` events, the stderr errors, the `--list-models` table and the transcripts under
//! `~/.cursor/projects/<folder>/agent-transcripts` are read from the CLI's bundled source. The
//! test machine is not signed in to Cursor, so only the failure path was run: the CLI prints
//! `Error: ...` on stderr and exits with 1 before any JSON line.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;

use crate::events::{one_line, SessionEvent};
use crate::models::{ModelList, ModelOption};

fn s(v: &Value) -> String {
    v.as_str().unwrap_or_default().to_string()
}

pub fn events(v: &Value) -> Option<Vec<SessionEvent>> {
    let ev = match (v["type"].as_str()?, v["subtype"].as_str()) {
        ("system", Some("init")) => vec![SessionEvent::Started {
            session_id: s(&v["session_id"]),
        }],
        // Task notices, the prompt echoed back, reasoning, and questions that print mode skips.
        ("system", _) | ("user", _) | ("thinking", _) | ("interaction_query", _) => vec![],
        ("assistant", _) => v["message"]["content"]
            .as_array()?
            .iter()
            .filter(|b| b["type"] == "text")
            .map(|b| SessionEvent::Message { text: s(&b["text"]) })
            .collect(),
        ("tool_call", Some("started")) => {
            let (tool, args, _) = tool_call(&v["tool_call"])?;
            vec![SessionEvent::ToolCall { summary: summary(args), tool }]
        }
        ("tool_call", Some("completed")) => {
            let (tool, args, result) = tool_call(&v["tool_call"])?;
            completed(tool, args, result)
        }
        ("tool_call", _) => vec![],
        ("retry", _) => vec![SessionEvent::Retrying {
            message: format!("Retrying, attempt {}", v["attempt"].as_u64().unwrap_or(1)),
        }],
        ("connection", Some("reconnecting")) => vec![SessionEvent::Retrying {
            message: "Reconnecting to Cursor".into(),
        }],
        ("connection", _) => vec![],
        // The only result Cursor writes is `success`; a failed turn ends on stderr instead.
        ("result", _) => vec![SessionEvent::Done {
            ok: v["is_error"] != true,
            summary: one_line(&s(&v["result"]), 200),
        }],
        _ => return None,
    };
    Some(ev)
}

/// A tool call is one `<name>ToolCall` object, such as `{"shellToolCall": {"args": .., "result": ..}}`.
fn tool_call(v: &Value) -> Option<(String, &Value, &Value)> {
    let (key, call) = v.as_object()?.iter().find(|(k, c)| k.ends_with("ToolCall") && c.is_object())?;
    let name = key.trim_end_matches("ToolCall");
    let mut chars = name.chars();
    let tool = match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => "Tool".into(),
    };
    Some((tool, &call["args"], &call["result"]))
}

fn summary(args: &Value) -> String {
    ["path", "command", "pattern", "globPattern", "query", "url", "toolName", "name"]
        .iter()
        .filter_map(|k| args[*k].as_str())
        .find(|t| !t.trim().is_empty())
        .map(|t| one_line(t, 160))
        .unwrap_or_default()
}

/// A result holds one of `success`, `failure`, `rejected`, `permissionDenied`, `timeout`, ...
fn completed(tool: String, args: &Value, result: &Value) -> Vec<SessionEvent> {
    let has = |keys: &[&str]| keys.iter().find_map(|k| result.get(*k).filter(|v| !v.is_null()));
    if has(&["rejected", "permissionDenied", "permission_denied"]).is_some() {
        return vec![SessionEvent::PermissionDenied { summary: summary(args), tool }];
    }
    if let Some(fail) = has(&["failure", "error", "timeout", "spawnError", "spawn_error"]) {
        let message = ["error", "message", "stderr"]
            .iter()
            .filter_map(|k| fail[*k].as_str())
            .find(|t| !t.trim().is_empty())
            .map(|t| one_line(t, 200))
            .unwrap_or_else(|| "failed".into());
        return vec![SessionEvent::ToolFailed { tool, message }];
    }
    let path = args["path"].as_str().unwrap_or_default();
    if has(&["success"]).is_some() && matches!(tool.as_str(), "Edit" | "Write" | "Delete") && !path.is_empty() {
        return vec![SessionEvent::FileChanged { path: path.into() }];
    }
    vec![]
}

/// Cursor writes errors as `Error: ...` on stderr, such as a missing login.
pub fn stderr_event(line: &str) -> Option<SessionEvent> {
    let message = stderr_reason(line)?;
    Some(SessionEvent::Error { message })
}

/// The first `Error: ...` line, without the prefix.
pub fn stderr_reason(stderr: &str) -> Option<String> {
    stderr
        .lines()
        .map(str::trim)
        .find_map(|l| l.strip_prefix("Error:"))
        .map(|m| one_line(m.trim(), 300))
        .filter(|m| !m.is_empty())
}

/// The text of the `result` line that `--output-format json` prints; an error when Cursor marks
/// the result as one.
pub fn final_answer(stdout: &str) -> Result<Option<String>, String> {
    let Some(v) = stdout
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l.trim()).ok())
        .find(|v| v["type"] == "result")
    else {
        return Ok(None);
    };
    let text = s(&v["result"]);
    if v["is_error"] == true {
        return Err(if text.trim().is_empty() { "Cursor CLI reported an error.".into() } else { one_line(&text, 300) });
    }
    Ok(Some(text))
}

/// `cursor-agent --list-models`: a heading, then `<id> - <name>` per model, the one in use marked
/// ` (current, default)`, then a tip. Cursor puts the thinking level in the model id itself.
pub fn parse_models(text: &str) -> ModelList {
    let models = text
        .lines()
        .map(str::trim)
        .filter_map(|l| l.split_once(" - "))
        .filter(|(id, _)| !id.is_empty() && !id.contains(char::is_whitespace))
        .map(|(id, name)| {
            let name = name.trim();
            let label = match name.rsplit_once(" (") {
                Some((label, mark)) if mark.trim_end_matches(')').split(", ").all(|w| w == "current" || w == "default") => label,
                _ => name,
            };
            ModelOption {
                id: id.to_string(),
                label: label.to_string(),
                group: None,
                efforts: vec![],
            }
        })
        .collect();
    ModelList {
        models,
        default_efforts: vec![],
    }
}

/// The folder name Cursor gives a workspace under `~/.cursor/projects`: every character that is
/// not an ASCII letter or digit becomes `-`, runs of them collapse, and the ends are trimmed.
pub fn project_folder(path: &str) -> String {
    let mut out = String::with_capacity(path.len());
    for c in path.chars() {
        let c = if c.is_ascii_alphanumeric() { c } else { '-' };
        if !(c == '-' && out.ends_with('-')) {
            out.push(c);
        }
    }
    out.trim_matches('-').to_string()
}

fn modified(path: &Path) -> SystemTime {
    fs::metadata(path).and_then(|m| m.modified()).unwrap_or(UNIX_EPOCH)
}

/// The newest chat transcript Cursor wrote for `cwd` since `since`: `<id>/<id>.jsonl`, or
/// `<id>.jsonl` from older versions.
pub fn newest_transcript(home: &Path, cwd: &Path, since: SystemTime) -> Option<PathBuf> {
    let want = project_folder(&cwd.display().to_string()).to_lowercase();
    let project = fs::read_dir(home.join(".cursor").join("projects"))
        .ok()?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .find(|p| p.is_dir() && p.file_name().is_some_and(|n| n.to_string_lossy().to_lowercase() == want))?;
    fs::read_dir(project.join("agent-transcripts"))
        .ok()?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter_map(|p| {
            if p.is_dir() {
                let id = p.file_name()?.to_string_lossy().into_owned();
                Some(p.join(format!("{id}.jsonl")))
            } else {
                p.extension().is_some_and(|x| x == "jsonl").then_some(p)
            }
        })
        .filter(|p| p.is_file())
        .map(|p| (modified(&p), p))
        .filter(|(m, _)| *m >= since)
        .max_by_key(|(m, _)| *m)
        .map(|(_, p)| p)
}

/// The chat id `--resume` takes: the transcript's file name, a UUID.
pub fn session_id(transcript: &Path) -> Option<String> {
    let id = transcript.file_stem()?.to_string_lossy().into_owned();
    let uuid = id.len() == 36 && id.chars().enumerate().all(|(i, c)| if [8, 13, 18, 23].contains(&i) { c == '-' } else { c.is_ascii_hexdigit() });
    uuid.then_some(id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::parse_line;
    use crate::cli::CliKind;
    use SessionEvent::*;

    // Built from the event writer in Cursor CLI 2026.09.28-64d2043; tool calls are protobuf JSON.
    #[test]
    fn stream_json_events() {
        let parse = |l: &str| parse_line(CliKind::Cursor, l);
        let init = r#"{"type":"system","subtype":"init","apiKeySource":"login","cwd":"C:\\w","session_id":"4f0e2c1a-1b2c-4d3e-8f90-0a1b2c3d4e5f","model":"Auto","permissionMode":"default"}"#;
        assert_eq!(parse(init), vec![Started { session_id: "4f0e2c1a-1b2c-4d3e-8f90-0a1b2c3d4e5f".into() }]);
        assert_eq!(crate::events::cli_session_id(CliKind::Cursor, init).as_deref(), Some("4f0e2c1a-1b2c-4d3e-8f90-0a1b2c3d4e5f"));
        assert_eq!(parse(r#"{"type":"user","message":{"role":"user","content":[{"type":"text","text":"hi"}]},"session_id":"s"}"#), vec![]);
        assert_eq!(
            parse(r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"Reading the file."}]},"session_id":"s"}"#),
            vec![Message { text: "Reading the file.".into() }]
        );
        let started = r#"{"type":"tool_call","subtype":"started","call_id":"c1","tool_call":{"shellToolCall":{"args":{"command":"git status","workingDirectory":""}}},"session_id":"s"}"#;
        assert_eq!(parse(started), vec![ToolCall { tool: "Shell".into(), summary: "git status".into() }]);
        let refused = r#"{"type":"tool_call","subtype":"completed","call_id":"c1","tool_call":{"shellToolCall":{"args":{"command":"rm -rf build"},"result":{"rejected":{"reason":""}}}},"session_id":"s"}"#;
        assert_eq!(parse(refused), vec![PermissionDenied { tool: "Shell".into(), summary: "rm -rf build".into() }]);
        let edited = r#"{"type":"tool_call","subtype":"completed","call_id":"c2","tool_call":{"editToolCall":{"args":{"path":"src/app.ts"},"result":{"success":{}}}},"session_id":"s"}"#;
        assert_eq!(parse(edited), vec![FileChanged { path: "src/app.ts".into() }]);
        let failed = r#"{"type":"tool_call","subtype":"completed","call_id":"c3","tool_call":{"readToolCall":{"args":{"path":"gone.txt"},"result":{"error":{"error":"File not found"}}}},"session_id":"s"}"#;
        assert_eq!(parse(failed), vec![ToolFailed { tool: "Read".into(), message: "File not found".into() }]);
        assert_eq!(parse(r#"{"type":"thinking","subtype":"delta","text":"hm"}"#), vec![]);
        let done = r#"{"type":"result","subtype":"success","duration_ms":4120,"duration_api_ms":4120,"is_error":false,"result":"Added the toggle.","session_id":"s","request_id":"r"}"#;
        assert_eq!(parse(done), vec![Done { ok: true, summary: "Added the toggle.".into() }]);
        assert!(matches!(parse(r#"{"type":"something_new"}"#)[..], [Raw { .. }]));
    }

    #[test]
    fn stderr_errors_and_the_planner_answer() {
        // Captured from 2026.09.28-64d2043 without a login.
        let stderr = "Error: Authentication required. Please run 'agent login' first, or set CURSOR_API_KEY environment variable.";
        assert_eq!(
            stderr_event(stderr),
            Some(Error { message: "Authentication required. Please run 'agent login' first, or set CURSOR_API_KEY environment variable.".into() })
        );
        assert_eq!(stderr_event("Operation cancelled"), None);
        assert_eq!(final_answer(r#"{"type":"result","subtype":"success","is_error":false,"result":"{\"summary\":\"x\"}"}"#), Ok(Some(r#"{"summary":"x"}"#.into())));
        assert_eq!(final_answer(""), Ok(None));
    }

    #[test]
    fn model_table() {
        let text = "Available models\n\nauto - Auto (current, default)\ngpt-5 - GPT-5\nsonnet-4-thinking - Claude 4 Sonnet (Thinking)\n\nTip: use --model <id> to switch.\n";
        let list = parse_models(text);
        let rows: Vec<(&str, &str)> = list.models.iter().map(|m| (m.id.as_str(), m.label.as_str())).collect();
        assert_eq!(rows, [("auto", "Auto"), ("gpt-5", "GPT-5"), ("sonnet-4-thinking", "Claude 4 Sonnet (Thinking)")]);
    }

    #[test]
    fn transcripts_are_found_by_folder_and_named_by_chat_id() {
        assert_eq!(project_folder(r"C:\Users\me\Project\my_app"), "C-Users-me-Project-my-app");
        assert_eq!(project_folder("/home/me/projects/app"), "home-me-projects-app");
        let home = std::env::temp_dir().join(format!("air-cursor-home-{}", std::process::id()));
        let _ = fs::remove_dir_all(&home);
        let id = "4f0e2c1a-1b2c-4d3e-8f90-0a1b2c3d4e5f";
        let chats = home.join(".cursor").join("projects").join("C-Users-me-Project-app").join("agent-transcripts");
        fs::create_dir_all(chats.join(id)).unwrap();
        fs::write(chats.join(id).join(format!("{id}.jsonl")), "{}\n").unwrap();
        let since = SystemTime::now() - std::time::Duration::from_secs(60);
        let found = newest_transcript(&home, Path::new(r"C:\Users\me\project\app"), since).unwrap();
        assert_eq!(session_id(&found).as_deref(), Some(id));
        assert_eq!(newest_transcript(&home, Path::new(r"C:\elsewhere"), since), None);
        assert_eq!(session_id(Path::new("notes.jsonl")), None);
        let _ = fs::remove_dir_all(&home);
    }
}
