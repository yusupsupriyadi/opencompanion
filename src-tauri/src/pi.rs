//! Pi (`@earendil-works/pi-coding-agent`, checked against 0.87.1). The event, session file and
//! `--list-models` shapes below were captured on 2026-09-28 from Pi runs against a local
//! OpenAI-compatible stub, since the test machine has no provider account.

use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;

use crate::events::{input_summary, one_line, SessionEvent};
use crate::models::{ModelList, ModelOption};

/// Pi has no permission prompts. Plan mode and the read-only planner get only these tools.
pub const READ_ONLY_TOOLS: &str = "read,grep,find,ls";

/// `--thinking` levels. Pi clamps each one to what the chosen model supports.
pub const THINKING: [&str; 6] = ["minimal", "low", "medium", "high", "xhigh", "max"];

/// `~/.pi/agent`: sessions, user skills and settings.
pub fn agent_dir(home: &Path) -> PathBuf {
    home.join(".pi").join("agent")
}

/// The folder under `sessions` that Pi files a working folder's sessions in: `C:\w\app` becomes
/// `--C--w-app--`.
pub fn session_folder(cwd: &str) -> String {
    let path = cwd.trim_end_matches(['\\', '/']);
    let path = path.strip_prefix(['\\', '/']).unwrap_or(path);
    format!("--{}--", path.replace(['\\', '/', ':'], "-"))
}

fn modified(path: &Path) -> SystemTime {
    fs::metadata(path).and_then(|m| m.modified()).unwrap_or(UNIX_EPOCH)
}

/// The newest session file Pi wrote for `cwd` since `since`.
pub fn newest_session(agent_dir: &Path, cwd: &Path, since: SystemTime) -> Option<PathBuf> {
    let want = session_folder(&cwd.display().to_string()).to_lowercase();
    let dir = fs::read_dir(agent_dir.join("sessions"))
        .ok()?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .find(|p| p.is_dir() && p.file_name().is_some_and(|n| n.to_string_lossy().to_lowercase() == want))?;
    fs::read_dir(dir)
        .ok()?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "jsonl"))
        .map(|p| (modified(&p), p))
        .filter(|(m, _)| *m >= since)
        .max_by_key(|(m, _)| *m)
        .map(|(_, p)| p)
}

/// The session id in a session file's header line.
pub fn session_id(file: &Path) -> Option<String> {
    let mut buf = vec![0u8; 64 * 1024];
    let n = File::open(file).ok()?.read(&mut buf).ok()?;
    let text = String::from_utf8_lossy(&buf[..n]);
    let header: Value = serde_json::from_str(text.lines().next()?).ok()?;
    let id = header["id"].as_str().filter(|_| header["type"] == "session")?;
    (!id.is_empty()).then(|| id.to_string())
}

fn s(v: &Value) -> String {
    v.as_str().unwrap_or_default().to_string()
}

fn failed(message: &Value) -> bool {
    matches!(message["stopReason"].as_str(), Some("error") | Some("aborted"))
}

fn failure_text(message: &Value) -> String {
    let said = s(&message["errorMessage"]);
    if said.is_empty() {
        format!("Request {}", s(&message["stopReason"]))
    } else {
        one_line(&said, 200)
    }
}

/// Text blocks of an assistant message.
fn texts(message: &Value) -> Vec<String> {
    message["content"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|b| b["type"] == "text")
        .filter_map(|b| b["text"].as_str())
        .filter(|t| !t.trim().is_empty())
        .map(str::to_owned)
        .collect()
}

/// `--mode json`. Tool calls come from `tool_execution_start`, which carries the arguments; the
/// assistant's `toolCall` blocks repeat them. `message_update` deltas are skipped for the final
/// `message_end`.
pub fn events(v: &Value) -> Option<Vec<SessionEvent>> {
    let ev = match v["type"].as_str()? {
        "session" => vec![SessionEvent::Started { session_id: s(&v["id"]) }],
        "message_end" if v["message"]["role"] == "assistant" => {
            texts(&v["message"]).into_iter().map(|text| SessionEvent::Message { text }).collect()
        }
        "tool_execution_start" => vec![SessionEvent::ToolCall {
            tool: s(&v["toolName"]),
            summary: input_summary(&v["args"]),
        }],
        "tool_execution_end" if v["isError"] == true => vec![SessionEvent::ToolFailed {
            tool: s(&v["toolName"]),
            message: one_line(&texts(&v["result"]).join(" "), 200),
        }],
        "auto_retry_start" => vec![SessionEvent::Retrying {
            message: one_line(&s(&v["errorMessage"]), 200),
        }],
        // `willRetry` means Pi starts the run again by itself.
        "agent_end" if v["willRetry"] != true => {
            let last = v["messages"].as_array().into_iter().flatten().rev().find(|m| m["role"] == "assistant");
            let ok = !last.is_some_and(failed);
            vec![SessionEvent::Done {
                ok,
                summary: last.filter(|_| !ok).map(failure_text).unwrap_or_default(),
            }]
        }
        "agent_start" | "agent_end" | "agent_settled" | "turn_start" | "turn_end" | "message_start" | "message_end"
        | "message_update" | "tool_execution_update" | "tool_execution_end" | "auto_retry_end" | "queue_update" => vec![],
        _ => return None,
    };
    Some(ev)
}

/// The session id from the header line `--mode json` prints first.
pub fn cli_session_id(v: &Value) -> Option<&str> {
    v["id"].as_str().filter(|_| v["type"] == "session")
}

/// Pi prints why it could not run (no API key, an unknown flag) on stderr and exits with 1.
/// Warnings and the indented hints under an error are left out.
pub fn stderr_event(line: &str) -> Option<SessionEvent> {
    let text = line.trim_end();
    if text.trim().is_empty() || text.starts_with(char::is_whitespace) || text.starts_with("Warning:") {
        return None;
    }
    Some(SessionEvent::Error {
        message: one_line(text.strip_prefix("Error: ").unwrap_or(text), 200),
    })
}

/// The text of the last assistant message in a `--mode json` run, or why the run failed.
/// `Ok(None)` when Pi wrote no answer at all.
pub fn final_answer(stdout: &str) -> Result<Option<String>, String> {
    let last = stdout
        .lines()
        .rev()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .find(|v| v["type"] == "message_end" && v["message"]["role"] == "assistant");
    let Some(v) = last else { return Ok(None) };
    if failed(&v["message"]) {
        return Err(failure_text(&v["message"]));
    }
    let text = texts(&v["message"]).join("\n");
    Ok((!text.trim().is_empty()).then_some(text))
}

/// Reads `pi --list-models`: a table of provider, model, context, max-out, thinking and images.
pub fn parse_models(text: &str) -> ModelList {
    let models = text
        .lines()
        .filter_map(|line| {
            let cols: Vec<&str> = line.split_whitespace().collect();
            let [provider, model, _, _, thinking, _] = cols[..] else { return None };
            if provider == "provider" && model == "model" {
                return None;
            }
            Some(ModelOption {
                id: format!("{provider}/{model}"),
                label: model.to_string(),
                group: Some(provider.to_string()),
                efforts: if thinking == "yes" { THINKING.map(String::from).to_vec() } else { vec![] },
            })
        })
        .collect();
    // Pi clamps a level the default model lacks, so every level is safe with it.
    ModelList {
        models,
        default_efforts: THINKING.map(String::from).to_vec(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use SessionEvent::*;

    fn parse(line: &str) -> Vec<SessionEvent> {
        events(&serde_json::from_str(line).unwrap()).unwrap()
    }

    #[test]
    fn session_folders_match_the_names_pi_writes() {
        assert_eq!(session_folder(r"C:\Users\yusup\project\opencompanion"), "--C--Users-yusup-project-opencompanion--");
        assert_eq!(session_folder(r"C:\w\app\"), "--C--w-app--");
        assert_eq!(session_folder("/home/me/app"), "--home-me-app--");
    }

    // Trimmed from a `pi --mode json` run that wrote hello.txt.
    #[test]
    fn a_run_becomes_start_tool_call_messages_and_done() {
        let header = r#"{"type":"session","version":3,"id":"01a0e43a-c50e-7040-9420-cea0cdbc02a5","timestamp":"2026-09-27T18:57:31.919Z","cwd":"C:\\w"}"#;
        assert_eq!(parse(header), vec![Started { session_id: "01a0e43a-c50e-7040-9420-cea0cdbc02a5".into() }]);
        assert_eq!(cli_session_id(&serde_json::from_str(header).unwrap()), Some("01a0e43a-c50e-7040-9420-cea0cdbc02a5"));
        let said = r#"{"type":"message_end","message":{"role":"assistant","content":[{"type":"text","text":"Writing the file."},{"type":"toolCall","id":"call_1","name":"write","arguments":{"path":"hello.txt","content":"hi"}}],"stopReason":"toolUse"}}"#;
        assert_eq!(parse(said), vec![Message { text: "Writing the file.".into() }]);
        let tool = r#"{"type":"tool_execution_start","toolCallId":"call_1","toolName":"write","args":{"path":"hello.txt","content":"hi"}}"#;
        assert_eq!(parse(tool), vec![ToolCall { tool: "write".into(), summary: "hello.txt".into() }]);
        let ok = r#"{"type":"tool_execution_end","toolCallId":"call_1","toolName":"write","result":{"content":[{"type":"text","text":"Successfully wrote to hello.txt"}]},"isError":false}"#;
        assert!(parse(ok).is_empty());
        let user = r#"{"type":"message_end","message":{"role":"user","content":[{"type":"text","text":"Create hello.txt"}]}}"#;
        assert!(parse(user).is_empty());
        let end = r#"{"type":"agent_end","messages":[{"role":"user","content":[]},{"role":"assistant","content":[{"type":"text","text":"Wrote hello.txt."}],"stopReason":"stop"}],"willRetry":false}"#;
        assert_eq!(parse(end), vec![Done { ok: true, summary: String::new() }]);
        let delta = r#"{"type":"message_update","assistantMessageEvent":{"type":"text_delta","contentIndex":0,"delta":"Hel"}}"#;
        assert!(parse(delta).is_empty());
        assert!(events(&serde_json::json!({ "type": "something_new" })).is_none());
    }

    #[test]
    fn failures_carry_pi_s_own_words() {
        let failed_tool = r#"{"type":"tool_execution_end","toolName":"bash","result":{"content":[{"type":"text","text":"exit code 1"}]},"isError":true}"#;
        assert_eq!(parse(failed_tool), vec![ToolFailed { tool: "bash".into(), message: "exit code 1".into() }]);
        let end = r#"{"type":"agent_end","messages":[{"role":"assistant","content":[],"stopReason":"error","errorMessage":"401 Unauthorized"}],"willRetry":false}"#;
        assert_eq!(parse(end), vec![Done { ok: false, summary: "401 Unauthorized".into() }]);
        let retrying = r#"{"type":"agent_end","messages":[{"role":"assistant","content":[],"stopReason":"error"}],"willRetry":true}"#;
        assert!(parse(retrying).is_empty());
        let retry = r#"{"type":"auto_retry_start","attempt":1,"maxAttempts":3,"delayMs":2000,"errorMessage":"529 overloaded"}"#;
        assert_eq!(parse(retry), vec![Retrying { message: "529 overloaded".into() }]);
    }

    // Captured from `pi --mode json` with no provider signed in.
    #[test]
    fn stderr_keeps_the_reason_and_drops_hints_and_warnings() {
        assert_eq!(stderr_event("No API key found for the selected model."), Some(Error { message: "No API key found for the selected model.".into() }));
        assert_eq!(stderr_event("Error: --name requires a non-empty value"), Some(Error { message: "--name requires a non-empty value".into() }));
        assert_eq!(stderr_event(r"  C:\Users\me\.pi\agent\docs\providers.md"), None);
        assert_eq!(stderr_event("Warning: No project session found with id 'x'"), None);
        assert_eq!(stderr_event(""), None);
    }

    #[test]
    fn the_planner_answer_is_the_last_assistant_text() {
        let run = [
            r#"{"type":"session","id":"s1"}"#,
            r#"{"type":"message_end","message":{"role":"assistant","content":[{"type":"text","text":"first"}],"stopReason":"toolUse"}}"#,
            r#"{"type":"message_end","message":{"role":"assistant","content":[{"type":"thinking","thinking":"hm"},{"type":"text","text":"{\"reply\":\"ok\"}"}],"stopReason":"stop"}}"#,
        ]
        .join("\n");
        assert_eq!(final_answer(&run), Ok(Some(r#"{"reply":"ok"}"#.to_string())));
        let failed = r#"{"type":"message_end","message":{"role":"assistant","content":[],"stopReason":"error","errorMessage":"No credits"}}"#;
        assert_eq!(final_answer(failed), Err("No credits".to_string()));
        assert_eq!(final_answer(r#"{"type":"session","id":"s1"}"#), Ok(None));
    }

    #[test]
    fn models_come_from_the_list_table() {
        let text = "provider   model              context  max-out  thinking  images\n\
                    anthropic  claude-sonnet-4-5  200K     64K      yes       yes\n\
                    mock       mock-model         128K     16.4K    no        no\n";
        let list = parse_models(text);
        let ids: Vec<&str> = list.models.iter().map(|m| m.id.as_str()).collect();
        assert_eq!(ids, ["anthropic/claude-sonnet-4-5", "mock/mock-model"]);
        assert_eq!(list.models[0].group.as_deref(), Some("anthropic"));
        assert_eq!(list.models[0].efforts.len(), THINKING.len());
        assert!(list.models[1].efforts.is_empty());
        let none = "No models available. Use /login to log into a provider via OAuth or API key. See:\n  C:\\docs\\providers.md\n";
        assert!(parse_models(none).models.is_empty());
    }

    #[test]
    fn the_newest_session_for_a_folder_gives_its_id() {
        let agent = std::env::temp_dir().join(format!("oc-pi-sessions-{}", std::process::id()));
        let _ = fs::remove_dir_all(&agent);
        let dir = agent.join("sessions").join("--C--w-app--");
        fs::create_dir_all(&dir).unwrap();
        fs::create_dir_all(agent.join("sessions").join("--C--w-other--")).unwrap();
        let file = dir.join("2026-09-27T18-57-31-919Z_abc.jsonl");
        fs::write(&file, "{\"type\":\"session\",\"version\":3,\"id\":\"abc\",\"cwd\":\"C:\\\\w\\\\app\"}\n").unwrap();
        let found = newest_session(&agent, Path::new(r"c:\W\app"), UNIX_EPOCH).unwrap();
        assert_eq!(found, file);
        assert_eq!(session_id(&found).as_deref(), Some("abc"));
        assert!(newest_session(&agent, Path::new(r"C:\w\other"), UNIX_EPOCH).is_none());
        let _ = fs::remove_dir_all(&agent);
    }
}
