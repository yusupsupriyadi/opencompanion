//! Pi (`@earendil-works/pi-coding-agent`, checked against 0.87.1). The event, session file and
//! `--list-models` shapes below were captured on 2026-09-28 from Pi runs against a local
//! OpenAI-compatible stub, since the test machine has no provider account.
//!
//! omp (`@oh-my-pi/pi-coding-agent`, checked against 18.3.5) is a Pi fork with the same print
//! mode, events and session files. Where it differs (its agent dir, session folder names, tool
//! names, `omp models --json` and a few extra events) the omp shapes come from its bundled
//! source and from runs that sent no prompt.

use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;

use crate::cli::CliKind;
use crate::events::{input_summary, one_line, SessionEvent};
use crate::models::{ModelList, ModelOption};

/// Tools for Plan mode and the read-only planner. omp has no `ls`, and its `find` is a semantic
/// search that is off unless a judge model is set up, so it gets `glob` instead.
pub fn read_only_tools(kind: CliKind) -> &'static str {
    match kind {
        CliKind::Omp => "read,grep,glob",
        _ => "read,grep,find,ls",
    }
}

/// `--thinking` levels. Pi and omp clamp each one to what the chosen model supports.
pub const THINKING: [&str; 6] = ["minimal", "low", "medium", "high", "xhigh", "max"];

/// `~/.pi/agent` or `~/.omp/agent`: sessions, user skills and settings.
pub fn agent_dir(kind: CliKind, home: &Path) -> PathBuf {
    let dot = if kind == CliKind::Omp { ".omp" } else { ".pi" };
    home.join(dot).join("agent")
}

/// The folder under `sessions` that Pi files a working folder's sessions in: `C:\w\app` becomes
/// `--C--w-app--`.
pub fn session_folder(cwd: &str) -> String {
    let path = cwd.trim_end_matches(['\\', '/']);
    let path = path.strip_prefix(['\\', '/']).unwrap_or(path);
    format!("--{}--", path.replace(['\\', '/', ':'], "-"))
}

/// `path` relative to `base`, with `/` separators, or `None` when it is not inside `base`.
/// Letter case is ignored, as Windows does.
fn relative_to(path: &str, base: &str) -> Option<String> {
    let norm = |s: &str| s.replace('\\', "/").trim_end_matches('/').to_string();
    let (path, base) = (norm(path), norm(base));
    if base.is_empty() || !path.get(..base.len())?.eq_ignore_ascii_case(&base) {
        return None;
    }
    match &path[base.len()..] {
        "" => Some(String::new()),
        rest => rest.strip_prefix('/').map(str::to_owned),
    }
}

/// The folder omp files a working folder's sessions in. Under the home folder the name comes
/// from the relative path (`~\project\app` becomes `-project-app`, home itself `-`), under the
/// temp folder the same after `-tmp`, and anywhere else it is Pi's name.
pub fn omp_session_folder(cwd: &str, home: &Path, tmp: &Path) -> String {
    for (prefix, base) in [("-", home), ("-tmp", tmp)] {
        if let Some(rel) = relative_to(cwd, &base.display().to_string()) {
            let rel = rel.replace(['/', ':'], "-");
            return match (rel.is_empty(), prefix.ends_with('-')) {
                (true, _) => prefix.to_string(),
                (false, true) => format!("{prefix}{rel}"),
                (false, false) => format!("{prefix}-{rel}"),
            };
        }
    }
    session_folder(cwd)
}

fn modified(path: &Path) -> SystemTime {
    fs::metadata(path).and_then(|m| m.modified()).unwrap_or(UNIX_EPOCH)
}

/// The newest session file `kind` wrote for `cwd` since `since`.
pub fn newest_session(kind: CliKind, home: &Path, cwd: &Path, since: SystemTime) -> Option<PathBuf> {
    let cwd_text = cwd.display().to_string();
    let want = match kind {
        CliKind::Omp => omp_session_folder(&cwd_text, home, &std::env::temp_dir()),
        _ => session_folder(&cwd_text),
    }
    .to_lowercase();
    let dir = fs::read_dir(agent_dir(kind, home).join("sessions"))
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

/// The session id in a session file's header line. omp writes a `title` line above it.
pub fn session_id(file: &Path) -> Option<String> {
    let mut buf = vec![0u8; 64 * 1024];
    let n = File::open(file).ok()?.read(&mut buf).ok()?;
    let text = String::from_utf8_lossy(&buf[..n]);
    let header = text
        .lines()
        .take(3)
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .find(|v| v["type"] == "session")?;
    let id = header["id"].as_str()?;
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
        // Pi's `willRetry` and omp's `yielded: false` mean the run goes on by itself (a retry,
        // compaction, a reminder).
        "agent_end" if v["willRetry"] != true && v["yielded"] != false => {
            let last = v["messages"].as_array().into_iter().flatten().rev().find(|m| m["role"] == "assistant");
            let ok = !last.is_some_and(failed);
            vec![SessionEvent::Done {
                ok,
                summary: last.filter(|_| !ok).map(failure_text).unwrap_or_default(),
            }]
        }
        // omp: info and warning notices are about its setup, such as MCP tools it mounted.
        "notice" if v["level"] == "error" => vec![SessionEvent::Error {
            message: one_line(&s(&v["message"]), 200),
        }],
        "agent_start" | "agent_end" | "agent_settled" | "turn_start" | "turn_end" | "message_start" | "message_end"
        | "message_update" | "tool_execution_update" | "tool_execution_end" | "auto_retry_end" | "queue_update" => vec![],
        "notice" | "tool_stream_update" | "auto_compaction_start" | "auto_compaction_end" | "retry_fallback_applied"
        | "retry_fallback_succeeded" | "model_changed" | "config_warnings_changed" | "advisor_cost_changed"
        | "advisor_yielded" | "ttsr_triggered" | "todo_reminder" | "todo_auto_clear" | "irc_message"
        | "thinking_level_changed" | "goal_updated" => vec![],
        _ => return None,
    };
    Some(ev)
}

/// The session id from the header line `--mode json` prints first.
pub fn cli_session_id(v: &Value) -> Option<&str> {
    v["id"].as_str().filter(|_| v["type"] == "session")
}

/// Pi and omp print why they could not run (no API key, an unknown flag) on stderr and exit
/// with 1. Warnings, notes and the indented hints under an error are left out. So are two omp
/// lines that are not the reason: the source excerpt Bun prints above an uncaught error
/// (`233 | ...`), and the line omp 18.3's startup watchdog prints even when a run succeeds.
pub fn stderr_event(line: &str) -> Option<SessionEvent> {
    let text = line.trim_end();
    let excerpt = text
        .split_once(" |")
        .is_some_and(|(n, rest)| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()) && (rest.is_empty() || rest.starts_with(' ')));
    let watchdog = text.starts_with("omp: `") && text.contains("ended before completing");
    if text.trim().is_empty()
        || text.starts_with(char::is_whitespace)
        || text.starts_with("Warning:")
        || text.starts_with("Note:")
        || excerpt
        || watchdog
    {
        return None;
    }
    Some(SessionEvent::Error {
        message: one_line(text.strip_prefix("Error: ").unwrap_or(text), 200),
    })
}

/// The first line of `stderr` that says why the run failed.
pub fn stderr_reason(stderr: &str) -> Option<String> {
    stderr.lines().find_map(stderr_event).and_then(|e| match e {
        SessionEvent::Error { message } => Some(message),
        _ => None,
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

/// True when `omp models --json` wrote the whole list. omp 18.3 exits with 1 after printing it.
pub fn omp_models_complete(stdout: &str) -> bool {
    serde_json::from_str::<Value>(stdout.trim()).is_ok_and(|v| v["models"].is_array())
}

/// Reads `omp models --json`: chat models with a `provider/id` selector, a display name and
/// the thinking levels each one takes (`null` for none).
pub fn parse_omp_models(text: &str) -> ModelList {
    let v: Value = serde_json::from_str(text.trim()).unwrap_or_default();
    let models = v["models"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|m| m["kind"].as_str().is_none_or(|k| k == "chat"))
        .filter_map(|m| {
            let id = m["selector"].as_str().filter(|s| !s.is_empty())?;
            Some(ModelOption {
                id: id.to_string(),
                label: m["name"].as_str().or(m["id"].as_str()).unwrap_or(id).to_string(),
                group: m["provider"].as_str().map(str::to_owned),
                efforts: m["thinking"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .filter(|e| THINKING.contains(e))
                    .map(str::to_owned)
                    .collect(),
            })
        })
        .collect();
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
        let home = std::env::temp_dir().join(format!("oc-pi-sessions-{}", std::process::id()));
        let _ = fs::remove_dir_all(&home);
        let sessions = agent_dir(CliKind::Pi, &home).join("sessions");
        let dir = sessions.join("--C--w-app--");
        fs::create_dir_all(&dir).unwrap();
        fs::create_dir_all(sessions.join("--C--w-other--")).unwrap();
        let file = dir.join("2026-09-27T18-57-31-919Z_abc.jsonl");
        fs::write(&file, "{\"type\":\"session\",\"version\":3,\"id\":\"abc\",\"cwd\":\"C:\\\\w\\\\app\"}\n").unwrap();
        let found = newest_session(CliKind::Pi, &home, Path::new(r"c:\W\app"), UNIX_EPOCH).unwrap();
        assert_eq!(found, file);
        assert_eq!(session_id(&found).as_deref(), Some("abc"));
        assert!(newest_session(CliKind::Pi, &home, Path::new(r"C:\w\other"), UNIX_EPOCH).is_none());
        assert!(newest_session(CliKind::Omp, &home, Path::new(r"c:\W\app"), UNIX_EPOCH).is_none());
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn omp_names_session_folders_from_the_home_relative_path() {
        let home = Path::new(r"C:\Users\yusup");
        let tmp = Path::new("/tmp");
        assert_eq!(omp_session_folder(r"C:\Users\yusup\project\opencompanion", home, tmp), "-project-opencompanion");
        assert_eq!(omp_session_folder(r"c:\users\YUSUP\Project\app\", home, tmp), "-Project-app");
        assert_eq!(omp_session_folder(r"C:\Users\yusup", home, tmp), "-");
        assert_eq!(omp_session_folder(r"C:\Users\yusupx\app", home, tmp), "--C--Users-yusupx-app--");
        assert_eq!(omp_session_folder(r"D:\work\app", home, tmp), "--D--work-app--");
        assert_eq!(omp_session_folder("/tmp/scratch/a", Path::new("/home/me"), tmp), "-tmp-scratch-a");
        assert_eq!(omp_session_folder("/tmp", Path::new("/home/me"), tmp), "-tmp");
        assert_eq!(read_only_tools(CliKind::Omp), "read,grep,glob");
        assert_eq!(read_only_tools(CliKind::Pi), "read,grep,find,ls");
    }

    // Trimmed from an omp 18.3.5 session file: a padded title line comes before the header.
    #[test]
    fn omp_session_files_start_with_a_title_line() {
        let home = std::env::temp_dir().join(format!("oc-omp-sessions-{}", std::process::id()));
        let _ = fs::remove_dir_all(&home);
        let dir = agent_dir(CliKind::Omp, &home).join("sessions").join("-project-app");
        fs::create_dir_all(&dir).unwrap();
        let file = dir.join("2026-09-27T18-51-57-379Z_01a0e435.jsonl");
        let lines = [
            r#"{"type":"title","v":1,"title":"","updatedAt":"2026-09-27T18:51:57.379Z","pad":"          "}"#,
            r#"{"type":"session","version":3,"id":"01a0e435-aa43-7314-b8f4-36a8762d4b76","timestamp":"2026-09-27T18:51:57.379Z","cwd":"C:\\Users\\me\\project\\app"}"#,
        ];
        fs::write(&file, lines.join("\n")).unwrap();
        let found = newest_session(CliKind::Omp, &home, &home.join("project").join("app"), UNIX_EPOCH).unwrap();
        assert_eq!(found, file);
        assert_eq!(session_id(&found).as_deref(), Some("01a0e435-aa43-7314-b8f4-36a8762d4b76"));
        let _ = fs::remove_dir_all(&home);
    }

    // omp's `agent_end` carries `yielded` instead of `willRetry`, and it adds notices.
    #[test]
    fn omp_events_end_only_when_the_agent_yields() {
        let resumes = r#"{"type":"agent_end","messages":[{"role":"assistant","content":[],"stopReason":"error"}],"isTerminal":false,"yielded":false}"#;
        assert!(parse(resumes).is_empty());
        let done = r#"{"type":"agent_end","messages":[{"role":"assistant","content":[{"type":"text","text":"Done."}],"stopReason":"stop"}],"isTerminal":true,"yielded":true}"#;
        assert_eq!(parse(done), vec![Done { ok: true, summary: String::new() }]);
        let mounted = r#"{"type":"notice","level":"info","message":"xd://: mounted mcp__context7_query_docs","source":"xdev"}"#;
        assert!(parse(mounted).is_empty());
        let failed = r#"{"type":"notice","level":"error","message":"Signed out of anthropic"}"#;
        assert_eq!(parse(failed), vec![Error { message: "Signed out of anthropic".into() }]);
        assert!(parse(r#"{"type":"tool_stream_update","toolCallId":"c1","toolName":"bash"}"#).is_empty());
        assert!(parse(r#"{"type":"thinking_level_changed","thinkingLevel":"high"}"#).is_empty());
    }

    // Captured from `omp --mode json --tools read,ls` with nothing on stdin.
    #[test]
    fn omp_stderr_keeps_the_error_and_drops_bun_s_excerpt_and_the_watchdog() {
        let stderr = "omp: `omp launch` ended before completing: the event loop drained while it was still pending (rerun with PI_DEBUG_STARTUP=1 to see the last phase reached)\n\
                      233 | \n\
                      234 | ${G.bold(\"Plugin Options:\")}\n\
                      \n\
                      CliUsageError: Unknown tool in --tools: ls. Valid tools: read, write.\n      \
                      at OYe (C:\\omp\\dist\\cli.js:238:3880)\n\
                      Warning: MCP server \"vercel\" failed to connect: HTTP 401; its tools are unavailable for this run.\n";
        let kept: Vec<SessionEvent> = stderr.lines().filter_map(stderr_event).collect();
        assert_eq!(kept, vec![Error { message: "CliUsageError: Unknown tool in --tools: ls. Valid tools: read, write.".into() }]);
        assert_eq!(stderr_reason(stderr).as_deref(), Some("CliUsageError: Unknown tool in --tools: ls. Valid tools: read, write."));
        assert_eq!(stderr_event("Note: plan.defaultOnStartup is ignored in print mode"), None);
        assert_eq!(stderr_event("2 | 3 errors"), None);
        assert_eq!(stderr_event("404 |not a table"), Some(Error { message: "404 |not a table".into() }));
    }

    // Trimmed from `omp models --json` (18.3.5).
    #[test]
    fn omp_models_come_from_its_json_list() {
        let text = r#"{"models":[
            {"provider":"amazon-bedrock","kind":"chat","id":"anthropic.claude-opus-4-6-v1","selector":"amazon-bedrock/anthropic.claude-opus-4-6-v1","name":"Claude Opus 4.6","reasoning":true,"thinking":["low","medium","high","max"]},
            {"provider":"amazon-bedrock","kind":"chat","id":"anthropic.claude-3-haiku-20240307-v1:0","selector":"amazon-bedrock/anthropic.claude-3-haiku-20240307-v1:0","name":"Claude Haiku 3","reasoning":false,"thinking":null},
            {"provider":"openai","kind":"embedding","id":"text-embedding-3-small","selector":"openai/text-embedding-3-small","name":"Embedding"}
        ]}"#;
        assert!(omp_models_complete(text));
        assert!(!omp_models_complete("omp: `omp models` ended before completing"));
        let list = parse_omp_models(text);
        let ids: Vec<&str> = list.models.iter().map(|m| m.id.as_str()).collect();
        assert_eq!(ids, ["amazon-bedrock/anthropic.claude-opus-4-6-v1", "amazon-bedrock/anthropic.claude-3-haiku-20240307-v1:0"]);
        assert_eq!(list.models[0].label, "Claude Opus 4.6");
        assert_eq!(list.models[0].group.as_deref(), Some("amazon-bedrock"));
        assert_eq!(list.models[0].efforts, ["low", "medium", "high", "max"]);
        assert!(list.models[1].efforts.is_empty());
        assert_eq!(list.default_efforts.len(), THINKING.len());
        assert!(parse_omp_models("").models.is_empty());
    }
}
