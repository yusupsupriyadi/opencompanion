//! Stand-in CLI for the session manager tests (`tests/manager.rs`). It speaks the event
//! formats captured from the real CLIs in the M0 spike, so no real CLI or quota is needed.
//!
//!   fake-cli run --format json --dir D PROMPT   OpenCode-style headless turn
//!   fake-cli -p ...                             Claude-style stream-json turn with one permission request
//!   fake-cli [PROMPT]                           interactive: prints a prompt, echoes one line (also as
//!                                               the terminal title), exits

use std::io::{self, BufRead, Write};

fn out(line: &str) {
    let mut o = io::stdout().lock();
    let _ = writeln!(o, "{line}");
    let _ = o.flush();
}

fn opencode_turn(args: &[String]) -> i32 {
    let prompt = args.last().map(String::as_str).unwrap_or("");
    out(r#"{"type":"step_start","sessionID":"ses_fake1","part":{"type":"step-start"}}"#);
    // Lets tests see which flags and permission rules reached the process.
    let seen = format!(
        "args: {} | env: {}",
        args.join(" "),
        std::env::var("OPENCODE_PERMISSION").unwrap_or_default()
    );
    out(&serde_json::json!({ "type": "text", "sessionID": "ses_fake1", "part": { "text": seen } }).to_string());
    out(r#"{"type":"tool_use","sessionID":"ses_fake1","part":{"tool":"apply_patch","state":{"status":"completed","input":{"patchText":"*** Add File: hello.txt"},"metadata":{"files":[{"filePath":"hello.txt"}]}}}}"#);
    out(&format!(
        r#"{{"type":"text","sessionID":"ses_fake1","part":{{"text":"done: {}"}}}}"#,
        prompt.replace('"', "'")
    ));
    if prompt.contains("fail") {
        eprintln!("fake failure requested");
        return 3;
    }
    0
}

fn claude_turn() -> i32 {
    let stdin = io::stdin();
    let mut lines = stdin.lock().lines();
    let first = lines.next().and_then(Result::ok).unwrap_or_default();
    if !first.contains("\"user\"") {
        eprintln!("expected a user message on stdin");
        return 2;
    }
    out(r#"{"type":"system","subtype":"init","session_id":"sess-fake-1","cwd":"x"}"#);
    out(r#"{"type":"assistant","message":{"content":[{"type":"tool_use","id":"t1","name":"Write","input":{"file_path":"hello.txt","content":"hi"}}]}}"#);
    out(r#"{"type":"control_request","request_id":"req-1","request":{"subtype":"can_use_tool","tool_name":"Write","input":{"file_path":"hello.txt","content":"hi"},"tool_use_id":"t1"}}"#);
    let answer = lines.next().and_then(Result::ok).unwrap_or_default();
    let allowed = answer.contains("\"allow\"") && answer.contains("req-1");
    let text = if allowed { "allowed" } else { "denied" };
    out(&format!(
        r#"{{"type":"assistant","message":{{"content":[{{"type":"text","text":"{text}"}}]}}}}"#
    ));
    out(&format!(
        r#"{{"type":"result","subtype":"success","is_error":false,"result":"{text}","session_id":"sess-fake-1"}}"#
    ));
    // Real Claude exits once its stdin closes after the result.
    for _ in lines.by_ref() {}
    0
}

fn interactive(prompt: Option<&str>) -> i32 {
    if let Some(p) = prompt {
        print!("first message: {p}\r\n");
    }
    // Terminal titles as Claude Code sets them: its own name first, then the task's.
    print!("\x1b]0;✳ Claude Code\x07fake ready\r\n> ");
    let _ = io::stdout().flush();
    let mut line = String::new();
    let _ = io::stdin().read_line(&mut line);
    print!("got: {}\r\n\x1b]0;✳ {}\x07", line.trim(), line.trim());
    let _ = io::stdout().flush();
    0
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = match args.first().map(String::as_str) {
        Some("run") => opencode_turn(&args),
        Some("-p") => claude_turn(),
        other => interactive(other.filter(|a| !a.starts_with('-'))),
    };
    std::process::exit(code);
}
