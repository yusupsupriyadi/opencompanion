//! Normalizes each CLI's headless output into one event shape for the timeline and status.
//! Shapes below were observed on 2026-09-25 (see docs/spike/M0-results.md); anything unknown
//! becomes `Raw` so the UI can still show it as plain text.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::cli::CliKind;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SessionEvent {
    Started { session_id: String },
    Message { text: String },
    ToolCall { tool: String, summary: String },
    ToolFailed { tool: String, message: String },
    FileChanged { path: String },
    /// The CLI is waiting for an answer (Claude `can_use_tool`).
    PermissionRequest { request_id: String, tool: String, summary: String },
    /// The CLI refused a tool on its own because nobody could answer.
    PermissionDenied { tool: String, summary: String },
    Retrying { message: String },
    Done { ok: bool, summary: String },
    Error { message: String },
    Raw { line: String },
}

pub fn parse_line(kind: CliKind, line: &str) -> Vec<SessionEvent> {
    let line = line.trim();
    if line.is_empty() {
        return vec![];
    }
    let Ok(v) = serde_json::from_str::<Value>(line) else {
        return vec![SessionEvent::Raw { line: line.to_string() }];
    };
    let events = match kind {
        CliKind::Claude | CliKind::Ccs => claude(&v),
        CliKind::Codex => codex(&v),
        CliKind::Opencode => opencode(&v),
        CliKind::Gemini => None,
        CliKind::Pi => crate::pi::events(&v),
    };
    events.unwrap_or_else(|| vec![SessionEvent::Raw { line: line.to_string() }])
}

/// The CLI's own session id, needed to resume a finished headless session with a follow-up.
pub fn cli_session_id(kind: CliKind, line: &str) -> Option<String> {
    let v: Value = serde_json::from_str(line.trim()).ok()?;
    let id = match kind {
        CliKind::Claude | CliKind::Ccs => v["session_id"].as_str(),
        CliKind::Codex if v["type"] == "thread.started" => v["thread_id"].as_str(),
        CliKind::Opencode => v["sessionID"].as_str(),
        CliKind::Pi => crate::pi::cli_session_id(&v),
        _ => None,
    }?;
    (!id.is_empty()).then(|| id.to_string())
}

fn s(v: &Value) -> String {
    v.as_str().unwrap_or_default().to_string()
}

/// A short, single-line description of a tool input: a path or command when there is one.
pub(crate) fn input_summary(input: &Value) -> String {
    for key in ["file_path", "filePath", "path", "command", "pattern", "url", "description"] {
        if let Some(text) = input.get(key).and_then(Value::as_str) {
            return one_line(text, 160);
        }
    }
    one_line(&input.to_string(), 160)
}

pub(crate) fn one_line(text: &str, max: usize) -> String {
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.chars().count() <= max {
        return flat;
    }
    let cut: String = flat.chars().take(max).collect();
    format!("{cut}…")
}

fn claude(v: &Value) -> Option<Vec<SessionEvent>> {
    let ev = match (v["type"].as_str()?, v["subtype"].as_str()) {
        ("system", Some("init")) => vec![SessionEvent::Started {
            session_id: s(&v["session_id"]),
        }],
        ("system", Some("permission_denied")) => vec![SessionEvent::PermissionDenied {
            tool: s(&v["tool_name"]),
            summary: input_summary(&v["tool_input"]),
        }],
        // Hook progress, thinking token counts and similar status lines.
        ("system", _) | ("rate_limit_event", _) => vec![],
        ("assistant", _) => {
            let mut out = Vec::new();
            for block in v["message"]["content"].as_array()? {
                match block["type"].as_str() {
                    Some("text") => out.push(SessionEvent::Message {
                        text: s(&block["text"]),
                    }),
                    Some("tool_use") => out.push(SessionEvent::ToolCall {
                        tool: s(&block["name"]),
                        summary: input_summary(&block["input"]),
                    }),
                    _ => {}
                }
            }
            out
        }
        ("user", _) => {
            let mut out = Vec::new();
            for block in v["message"]["content"].as_array().into_iter().flatten() {
                if block["type"] == "tool_result" && block["is_error"] == true {
                    out.push(SessionEvent::ToolFailed {
                        tool: String::new(),
                        message: one_line(&tool_result_text(&block["content"]), 200),
                    });
                }
            }
            let file = &v["tool_use_result"]["filePath"];
            if file.is_string() {
                out.push(SessionEvent::FileChanged { path: s(file) });
            }
            out
        }
        ("control_request", _) if v["request"]["subtype"] == "can_use_tool" => {
            let r = &v["request"];
            vec![SessionEvent::PermissionRequest {
                request_id: s(&v["request_id"]),
                tool: s(&r["tool_name"]),
                summary: input_summary(&r["input"]),
            }]
        }
        ("result", sub) => {
            let ok = sub == Some("success") && v["is_error"] != true;
            vec![SessionEvent::Done {
                ok,
                summary: one_line(&s(&v["result"]), 200),
            }]
        }
        _ => return None,
    };
    Some(ev)
}

fn tool_result_text(content: &Value) -> String {
    match content {
        Value::String(t) => t.clone(),
        Value::Array(parts) => parts
            .iter()
            .filter_map(|p| p["text"].as_str())
            .collect::<Vec<_>>()
            .join(" "),
        other => other.to_string(),
    }
}

/// Codex `exec --json`. Only the failure path was observed (auth error on this machine);
/// the item types follow the Codex docs and stay unverified until a successful run.
fn codex(v: &Value) -> Option<Vec<SessionEvent>> {
    let ev = match v["type"].as_str()? {
        "thread.started" => vec![SessionEvent::Started {
            session_id: s(&v["thread_id"]),
        }],
        "turn.started" | "item.started" | "item.updated" => vec![],
        "item.completed" => codex_item(&v["item"]),
        "turn.completed" => vec![SessionEvent::Done {
            ok: true,
            summary: String::new(),
        }],
        "turn.failed" => vec![SessionEvent::Done {
            ok: false,
            summary: one_line(&s(&v["error"]["message"]), 200),
        }],
        "error" => {
            let message = s(&v["message"]);
            if message.starts_with("Reconnecting") {
                vec![SessionEvent::Retrying { message: one_line(&message, 200) }]
            } else {
                vec![SessionEvent::Error { message: one_line(&message, 200) }]
            }
        }
        _ => return None,
    };
    Some(ev)
}

fn codex_item(item: &Value) -> Vec<SessionEvent> {
    match item["type"].as_str().unwrap_or_default() {
        "agent_message" => vec![SessionEvent::Message { text: s(&item["text"]) }],
        "command_execution" => vec![SessionEvent::ToolCall {
            tool: "shell".into(),
            summary: one_line(&s(&item["command"]), 160),
        }],
        "file_change" => item["changes"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|c| SessionEvent::FileChanged { path: s(&c["path"]) })
            .collect(),
        "mcp_tool_call" => vec![SessionEvent::ToolCall {
            tool: format!("{}.{}", s(&item["server"]), s(&item["tool"])),
            summary: String::new(),
        }],
        // Config warnings such as a malformed agent role are not errors of the task.
        "error" => vec![SessionEvent::Raw { line: s(&item["message"]) }],
        _ => vec![],
    }
}

fn opencode(v: &Value) -> Option<Vec<SessionEvent>> {
    let part = &v["part"];
    let ev = match v["type"].as_str()? {
        "step_start" | "step_finish" => vec![],
        "text" => vec![SessionEvent::Message { text: s(&part["text"]) }],
        "tool_use" => {
            let tool = s(&part["tool"]);
            let state = &part["state"];
            if state["status"] == "error" {
                vec![SessionEvent::ToolFailed {
                    tool,
                    message: one_line(&s(&state["error"]), 200),
                }]
            } else {
                let mut out = vec![SessionEvent::ToolCall {
                    summary: input_summary(&state["input"]),
                    tool,
                }];
                if let Some(files) = state["metadata"]["files"].as_array() {
                    for f in files {
                        let path = f["filePath"].as_str().or(f.as_str()).unwrap_or_default();
                        if !path.is_empty() {
                            out.push(SessionEvent::FileChanged { path: path.into() });
                        }
                    }
                }
                out
            }
        }
        "error" => vec![SessionEvent::Error {
            message: one_line(&v["error"].to_string(), 200),
        }],
        _ => return None,
    };
    Some(ev)
}

/// OpenCode `run` reports permission requests on stderr and rejects them itself.
pub fn opencode_stderr(line: &str) -> Option<SessionEvent> {
    let plain = strip_ansi(line);
    let rest = plain.split("permission requested:").nth(1)?;
    let (what, _) = rest.split_once(';').unwrap_or((rest, ""));
    let what = what.trim();
    let (tool, target) = what.split_once(' ').unwrap_or((what, ""));
    Some(SessionEvent::PermissionDenied {
        tool: tool.to_string(),
        summary: target.trim_matches(|c| c == '(' || c == ')').to_string(),
    })
}

fn strip_ansi(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' && chars.peek() == Some(&'[') {
            chars.next();
            for c in chars.by_ref() {
                if c.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use SessionEvent::*;

    // Trimmed copies of lines captured in the M0 spike.
    #[test]
    fn claude_permission_request_and_tool_call() {
        let req = r#"{"type":"control_request","request_id":"3ed5","request":{"subtype":"can_use_tool","tool_name":"Write","input":{"file_path":"C:\\w\\hello.txt","content":"hi"},"tool_use_id":"toolu_1"}}"#;
        assert_eq!(
            parse_line(CliKind::Claude, req),
            vec![PermissionRequest {
                request_id: "3ed5".into(),
                tool: "Write".into(),
                summary: r"C:\w\hello.txt".into()
            }]
        );
        let call = r#"{"type":"assistant","message":{"content":[{"type":"tool_use","id":"t","name":"Write","input":{"file_path":"a.txt","content":"hi"}}]}}"#;
        assert_eq!(
            parse_line(CliKind::Claude, call),
            vec![ToolCall { tool: "Write".into(), summary: "a.txt".into() }]
        );
    }

    #[test]
    fn claude_denial_status_lines_and_result() {
        let denied = r#"{"type":"system","subtype":"permission_denied","tool_name":"Write","tool_input":{"file_path":"a.txt"}}"#;
        assert_eq!(
            parse_line(CliKind::Claude, denied),
            vec![PermissionDenied { tool: "Write".into(), summary: "a.txt".into() }]
        );
        assert!(parse_line(CliKind::Claude, r#"{"type":"system","subtype":"hook_started"}"#).is_empty());
        let result = r#"{"type":"result","subtype":"success","is_error":false,"result":"done"}"#;
        assert_eq!(
            parse_line(CliKind::Claude, result),
            vec![Done { ok: true, summary: "done".into() }]
        );
    }

    #[test]
    fn codex_failure_path() {
        let retry = r#"{"type":"error","message":"Reconnecting... 1/5 (unexpected status 401 Unauthorized)"}"#;
        assert!(matches!(parse_line(CliKind::Codex, retry)[..], [Retrying { .. }]));
        let failed = r#"{"type":"turn.failed","error":{"message":"unexpected status 401 Unauthorized"}}"#;
        assert_eq!(
            parse_line(CliKind::Codex, failed),
            vec![Done { ok: false, summary: "unexpected status 401 Unauthorized".into() }]
        );
        let warn = r#"{"type":"item.completed","item":{"id":"i0","type":"error","message":"Ignoring malformed agent role definition"}}"#;
        assert!(matches!(parse_line(CliKind::Codex, warn)[..], [Raw { .. }]));
    }

    #[test]
    fn opencode_tool_use_text_and_rejection() {
        let tool = r#"{"type":"tool_use","part":{"tool":"apply_patch","state":{"status":"completed","input":{"patchText":"*** Add File: hello.txt"}}}}"#;
        assert!(matches!(&parse_line(CliKind::Opencode, tool)[..], [ToolCall { tool, .. }] if tool == "apply_patch"));
        let err = r#"{"type":"tool_use","part":{"tool":"apply_patch","state":{"status":"error","error":"The user rejected permission to use this specific tool call."}}}"#;
        assert!(matches!(&parse_line(CliKind::Opencode, err)[..], [ToolFailed { .. }]));
        let text = r#"{"type":"text","part":{"text":"done"}}"#;
        assert_eq!(parse_line(CliKind::Opencode, text), vec![Message { text: "done".into() }]);
        let stderr = "\x1b[93m\x1b[1m! \x1b[0mpermission requested: edit (C:/w/hello.txt); auto-rejecting";
        assert_eq!(
            opencode_stderr(stderr),
            Some(PermissionDenied { tool: "edit".into(), summary: "C:/w/hello.txt".into() })
        );
    }

    #[test]
    fn non_json_becomes_raw() {
        assert_eq!(
            parse_line(CliKind::Codex, "plain text"),
            vec![Raw { line: "plain text".into() }]
        );
    }
}
