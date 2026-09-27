//! Transcripts of CLI sessions opened outside OpenCompanion (PRD FR-32). Each CLI keeps its own
//! history on disk; this finds the newest one written for the process's folder since the process
//! started and reads its last messages. Nothing here writes to those files.

use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection, OpenFlags};
use serde::Serialize;
use serde_json::Value;

use crate::cli::CliKind;
use crate::events::{input_summary, one_line};
use crate::projects::{claude_encode, norm};

/// Lines kept from the end of a transcript.
const KEEP: usize = 40;
/// Bytes read from the end of a JSONL transcript. Tool results make Claude Code lines long, so this
/// holds a few dozen messages; older ones would be cut by `KEEP` anyway.
const TAIL_BYTES: u64 = 4 * 1024 * 1024;
const TEXT_MAX: usize = 1200;
/// A transcript written this long before the process started still counts, for clock skew.
const SLACK: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Speaker {
    You,
    Cli,
    Tool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Line {
    pub speaker: Speaker,
    pub text: String,
    /// Unix ms, when the CLI recorded a time.
    pub at: Option<i64>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Transcript {
    /// The file the lines were read from.
    pub source: Option<String>,
    pub lines: Vec<Line>,
    /// Why no lines are shown, in words for the owner.
    pub note: Option<String>,
}

fn unavailable(note: &str) -> Transcript {
    Transcript {
        note: Some(note.into()),
        ..Transcript::default()
    }
}

/// The newest transcript `kind` wrote for a process in `cwd` that started at `started_at` (Unix seconds).
pub fn read(kind: CliKind, cwd: Option<&Path>, started_at: u64) -> Transcript {
    match crate::projects::home() {
        Some(home) => read_in(&home, kind, cwd, started_at),
        None => unavailable("Your home folder could not be found, so the CLI's history could not be either."),
    }
}

/// `read` with the home folder given, so tests can point it at a temporary one.
pub fn read_in(home: &Path, kind: CliKind, cwd: Option<&Path>, started_at: u64) -> Transcript {
    let Some(cwd) = cwd else {
        return unavailable("Windows did not share this process's folder, so its transcript cannot be matched.");
    };
    let since = (UNIX_EPOCH + Duration::from_secs(started_at)).checked_sub(SLACK).unwrap_or(UNIX_EPOCH);
    let found = match kind {
        CliKind::Claude => claude(&[home.join(".claude")], cwd, since),
        CliKind::Ccs => claude(&crate::ccs::claude_dirs(home), cwd, since),
        CliKind::Codex => codex(home, cwd, since),
        CliKind::Opencode => opencode(home, cwd, since),
        CliKind::Gemini => return unavailable("Transcript not available for this CLI."),
        CliKind::Pi | CliKind::Omp => pi(kind, home, cwd, since),
    };
    match found {
        Ok(Some(mut t)) => {
            if t.lines.len() > KEEP {
                t.lines.drain(..t.lines.len() - KEEP);
            }
            if t.lines.is_empty() {
                t.note = Some("No messages in this session yet.".into());
            }
            t
        }
        Ok(None) => unavailable(&format!("{} has not written a transcript for this folder since this process started.", kind.label())),
        Err(e) => unavailable(&format!("The transcript could not be read: {e}")),
    }
}

fn same_dir(a: &str, b: &Path) -> bool {
    norm(a) == norm(&b.display().to_string())
}

fn modified(path: &Path) -> SystemTime {
    fs::metadata(path).and_then(|m| m.modified()).unwrap_or(UNIX_EPOCH)
}

/// Complete lines from the last `TAIL_BYTES` of a file.
fn tail_lines(path: &Path) -> std::io::Result<Vec<String>> {
    let mut f = File::open(path)?;
    let len = f.metadata()?.len();
    let start = len.saturating_sub(TAIL_BYTES);
    f.seek(SeekFrom::Start(start))?;
    let mut buf = Vec::new();
    f.read_to_end(&mut buf)?;
    let text = String::from_utf8_lossy(&buf);
    let mut lines: Vec<String> = text.lines().map(str::to_owned).collect();
    // Reading from the middle of the file starts inside a line.
    if start > 0 && !lines.is_empty() {
        lines.remove(0);
    }
    Ok(lines)
}

fn first_line(path: &Path) -> Option<String> {
    let mut f = File::open(path).ok()?;
    let mut buf = vec![0u8; 64 * 1024];
    let n = f.read(&mut buf).ok()?;
    let text = String::from_utf8_lossy(&buf[..n]);
    text.lines().next().map(str::to_owned)
}

fn say(speaker: Speaker, text: &str, at: Option<i64>) -> Option<Line> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    let text = if text.chars().count() > TEXT_MAX {
        format!("{}…", text.chars().take(TEXT_MAX).collect::<String>())
    } else {
        text.to_string()
    };
    Some(Line { speaker, text, at })
}

fn tool(name: &str, input: &Value, at: Option<i64>) -> Option<Line> {
    let summary = match input {
        Value::Null => String::new(),
        // Codex passes tool arguments as JSON text.
        Value::String(s) => match serde_json::from_str::<Value>(s) {
            Ok(v @ Value::Object(_)) => input_summary(&v),
            // A patch opens with "*** Begin Patch", then names the file: "*** Update File: a.md".
            _ => s
                .lines()
                .map(|l| l.trim().trim_start_matches('*').trim())
                .find(|l| !l.is_empty() && *l != "Begin Patch")
                .map(|l| one_line(l, 160))
                .unwrap_or_default(),
        },
        v => input_summary(v),
    };
    say(Speaker::Tool, format!("{name} {summary}").trim_end(), at)
}

/// Unix ms from an RFC 3339 time such as `2026-07-24T10:28:44.858Z`.
fn parse_time(s: &str) -> Option<i64> {
    let (date, rest) = s.split_once('T')?;
    let mut d = date.split('-').map(|p| p.parse::<i64>().ok());
    let (y, m, day) = (d.next()??, d.next()??, d.next()??);
    let (clock, offset_min) = if let Some(c) = rest.strip_suffix('Z') {
        (c, 0)
    } else {
        let at = rest.rfind(['+', '-'])?;
        let (c, off) = rest.split_at(at);
        let sign = if off.starts_with('-') { -1 } else { 1 };
        let (oh, om) = off[1..].split_once(':')?;
        (c, sign * (oh.parse::<i64>().ok()? * 60 + om.parse::<i64>().ok()?))
    };
    let mut t = clock.split(':');
    let (h, min) = (t.next()?.parse::<i64>().ok()?, t.next()?.parse::<i64>().ok()?);
    let sec: f64 = t.next()?.parse().ok()?;
    // Days from 1970-01-01 to the date (Howard Hinnant's days_from_civil).
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * ((m + 9) % 12) + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    let secs = days * 86_400 + h * 3600 + min * 60 - offset_min * 60;
    Some(secs * 1000 + (sec * 1000.0).round() as i64)
}

// Claude Code: ~/.claude/projects/<folder with every non-alphanumeric character as "-">/<session>.jsonl

/// Text Claude Code adds to the conversation itself, such as slash command wrappers.
fn claude_harness_text(text: &str) -> bool {
    let t = text.trim_start();
    ["<command-", "<local-command-", "<task-notification", "<system-reminder"].iter().any(|p| t.starts_with(p))
}

fn claude_lines(v: &Value) -> Vec<Line> {
    if v["isMeta"] == true || v["isSidechain"] == true {
        return vec![];
    }
    let at = v["timestamp"].as_str().and_then(parse_time);
    let content = &v["message"]["content"];
    match v["type"].as_str() {
        Some("user") => match content {
            Value::String(s) if !claude_harness_text(s) => say(Speaker::You, s, at).into_iter().collect(),
            Value::Array(blocks) => blocks
                .iter()
                .filter(|b| b["type"] == "text")
                .filter_map(|b| b["text"].as_str())
                .filter(|t| !claude_harness_text(t))
                .filter_map(|t| say(Speaker::You, t, at))
                .collect(),
            _ => vec![],
        },
        Some("assistant") => content
            .as_array()
            .map(|blocks| {
                blocks
                    .iter()
                    .filter_map(|b| match b["type"].as_str() {
                        Some("text") => say(Speaker::Cli, b["text"].as_str().unwrap_or_default(), at),
                        Some("tool_use") => tool(b["name"].as_str().unwrap_or("Tool"), &b["input"], at),
                        _ => None,
                    })
                    .collect()
            })
            .unwrap_or_default(),
        _ => vec![],
    }
}

/// The newest transcript for `cwd` in any of these Claude Code config folders: one for Claude
/// Code, one per profile for CCS.
fn claude(config_dirs: &[PathBuf], cwd: &Path, since: SystemTime) -> Result<Option<Transcript>, String> {
    let want = claude_encode(cwd.display().to_string().trim_end_matches(['\\', '/'])).to_lowercase();
    let mut files = Vec::new();
    for projects in config_dirs.iter().map(|d| d.join("projects")) {
        let Ok(entries) = fs::read_dir(&projects) else { continue };
        let Some(dir) = entries
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .find(|p| p.is_dir() && p.file_name().is_some_and(|n| n.to_string_lossy().to_lowercase() == want))
        else {
            continue;
        };
        files.extend(
            fs::read_dir(&dir)
                .map_err(|e| e.to_string())?
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|x| x == "jsonl")),
        );
    }
    let newest = files
        .into_iter()
        .map(|p| (modified(&p), p))
        .filter(|(m, _)| *m >= since)
        .max_by_key(|(m, _)| *m);
    let Some((_, file)) = newest else { return Ok(None) };
    let lines = tail_lines(&file).map_err(|e| e.to_string())?;
    Ok(Some(Transcript {
        source: Some(file.display().to_string()),
        lines: lines
            .iter()
            .filter_map(|l| serde_json::from_str::<Value>(l).ok())
            .flat_map(|v| claude_lines(&v))
            .collect(),
        note: None,
    }))
}

// Codex CLI: ~/.codex/sessions/YYYY/MM/DD/rollout-*.jsonl, whose first line names the folder.

fn codex_lines(v: &Value) -> Option<Line> {
    let at = v["timestamp"].as_str().and_then(parse_time);
    let p = &v["payload"];
    match (v["type"].as_str()?, p["type"].as_str()?) {
        ("event_msg", "user_message") => say(Speaker::You, p["message"].as_str()?, at),
        ("event_msg", "agent_message") => say(Speaker::Cli, p["message"].as_str()?, at),
        ("response_item", "function_call") => tool(p["name"].as_str()?, &p["arguments"], at),
        ("response_item", "custom_tool_call") => tool(p["name"].as_str()?, &p["input"], at),
        _ => None,
    }
}

/// Sub-folders of `dir`, newest name first (the names are years, months or days).
fn dirs_desc(dir: &Path) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = fs::read_dir(dir)
        .map(|rd| rd.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.is_dir()).collect())
        .unwrap_or_default();
    out.sort();
    out.reverse();
    out
}

fn codex(home: &Path, cwd: &Path, since: SystemTime) -> Result<Option<Transcript>, String> {
    let root = home.join(".codex").join("sessions");
    let days: Vec<PathBuf> = dirs_desc(&root)
        .iter()
        .flat_map(|y| dirs_desc(y))
        .flat_map(|m| dirs_desc(&m))
        .take(14)
        .collect();
    let mut files: Vec<(SystemTime, PathBuf)> = days
        .iter()
        .filter_map(|d| fs::read_dir(d).ok())
        .flatten()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "jsonl"))
        .map(|p| (modified(&p), p))
        .filter(|(m, _)| *m >= since)
        .collect();
    files.sort_by_key(|(m, _)| std::cmp::Reverse(*m));
    for (_, file) in files.into_iter().take(40) {
        let meta = first_line(&file).and_then(|l| serde_json::from_str::<Value>(&l).ok());
        let folder = meta.as_ref().and_then(|m| m["payload"]["cwd"].as_str());
        if !folder.is_some_and(|f| same_dir(f, cwd)) {
            continue;
        }
        let lines = tail_lines(&file).map_err(|e| e.to_string())?;
        return Ok(Some(Transcript {
            source: Some(file.display().to_string()),
            lines: lines
                .iter()
                .filter_map(|l| serde_json::from_str::<Value>(l).ok())
                .filter_map(|v| codex_lines(&v))
                .collect(),
            note: None,
        }));
    }
    Ok(None)
}

// OpenCode: a SQLite database in ~/.local/share/opencode, opened read-only.

fn opencode(home: &Path, cwd: &Path, since: SystemTime) -> Result<Option<Transcript>, String> {
    let dir = home.join(".local").join("share").join("opencode");
    let mut dbs: Vec<(SystemTime, PathBuf)> = fs::read_dir(&dir)
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| {
                    p.file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .is_some_and(|n| n.starts_with("opencode") && n.ends_with(".db"))
                })
                .map(|p| (modified(&p), p))
                .collect()
        })
        .unwrap_or_default();
    dbs.sort_by_key(|(m, _)| std::cmp::Reverse(*m));
    let since_ms = since.duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0);
    for (_, db) in dbs {
        if let Some(t) = opencode_db(&db, cwd, since_ms).map_err(|e| e.to_string())? {
            return Ok(Some(t));
        }
    }
    Ok(None)
}

fn opencode_db(db: &Path, cwd: &Path, since_ms: i64) -> rusqlite::Result<Option<Transcript>> {
    let conn = Connection::open_with_flags(db, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX)?;
    conn.busy_timeout(Duration::from_millis(500))?;
    let session: Option<String> = {
        let mut st = conn.prepare(
            "SELECT id, directory FROM session WHERE parent_id IS NULL AND time_updated >= ?1
             ORDER BY time_updated DESC LIMIT 60",
        )?;
        let rows = st.query_map([since_ms], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
        let mut found = None;
        for row in rows {
            let (id, directory) = row?;
            if same_dir(&directory, cwd) {
                found = Some(id);
                break;
            }
        }
        found
    };
    let Some(session) = session else { return Ok(None) };
    let mut st = conn.prepare(
        "SELECT m.data, p.data, p.time_created FROM part p JOIN message m ON m.id = p.message_id
         WHERE p.session_id = ?1 ORDER BY p.time_created DESC, p.id DESC LIMIT 400",
    )?;
    let rows = st.query_map(params![session], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, i64>(2)?)))?;
    let mut lines = Vec::new();
    for row in rows {
        let (message, part, at) = row?;
        let (Ok(message), Ok(part)) = (serde_json::from_str::<Value>(&message), serde_json::from_str::<Value>(&part)) else { continue };
        let line = match part["type"].as_str() {
            Some("text") if part["synthetic"] != true => {
                let who = if message["role"] == "user" { Speaker::You } else { Speaker::Cli };
                say(who, part["text"].as_str().unwrap_or_default(), Some(at))
            }
            Some("tool") => tool(part["tool"].as_str().unwrap_or("tool"), &part["state"]["input"], Some(at)),
            _ => None,
        };
        lines.extend(line);
    }
    lines.reverse();
    Ok(Some(Transcript {
        source: Some(db.display().to_string()),
        lines,
        note: None,
    }))
}

// Pi and omp: <agent dir>/sessions/<folder>/<time>_<session>.jsonl, one entry per line.

fn pi_lines(v: &Value) -> Vec<Line> {
    if v["type"] != "message" {
        return vec![];
    }
    let at = v["timestamp"].as_str().and_then(parse_time);
    let m = &v["message"];
    let blocks = m["content"].as_array().map(Vec::as_slice).unwrap_or_default();
    match m["role"].as_str() {
        Some("user") => match &m["content"] {
            Value::String(s) => say(Speaker::You, s, at).into_iter().collect(),
            _ => blocks
                .iter()
                .filter(|b| b["type"] == "text")
                .filter_map(|b| say(Speaker::You, b["text"].as_str().unwrap_or_default(), at))
                .collect(),
        },
        Some("assistant") => blocks
            .iter()
            .filter_map(|b| match b["type"].as_str() {
                Some("text") => say(Speaker::Cli, b["text"].as_str().unwrap_or_default(), at),
                Some("toolCall") => tool(b["name"].as_str().unwrap_or("tool"), &b["arguments"], at),
                _ => None,
            })
            .collect(),
        _ => vec![],
    }
}

fn pi(kind: CliKind, home: &Path, cwd: &Path, since: SystemTime) -> Result<Option<Transcript>, String> {
    let Some(file) = crate::pi::newest_session(kind, home, cwd, since) else { return Ok(None) };
    let lines = tail_lines(&file).map_err(|e| e.to_string())?;
    Ok(Some(Transcript {
        source: Some(file.display().to_string()),
        lines: lines
            .iter()
            .filter_map(|l| serde_json::from_str::<Value>(l).ok())
            .flat_map(|v| pi_lines(&v))
            .collect(),
        note: None,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_home(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("air-transcript-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn now_secs() -> u64 {
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()
    }

    #[test]
    fn times_parse_as_unix_ms() {
        assert_eq!(parse_time("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(parse_time("2026-07-24T10:28:44.858Z"), Some(1_784_888_924_858));
        assert_eq!(parse_time("2026-07-24T17:28:44.858+07:00"), Some(1_784_888_924_858));
        assert_eq!(parse_time("yesterday"), None);
    }

    #[test]
    fn claude_transcript_skips_meta_and_harness_text_and_names_tools() {
        let home = temp_home("claude");
        let cwd = Path::new(r"C:\Users\me\Project\ai-remote");
        let dir = home.join(".claude").join("projects").join("C--Users-me-Project-ai-remote");
        fs::create_dir_all(&dir).unwrap();
        let lines = [
            r#"{"type":"user","isMeta":true,"message":{"role":"user","content":"caveat"},"timestamp":"2026-07-24T10:28:44Z"}"#,
            r#"{"type":"user","message":{"role":"user","content":"<command-name>/model</command-name>"},"timestamp":"2026-07-24T10:28:45Z"}"#,
            r#"{"type":"user","message":{"role":"user","content":"Fix the failing tests"},"timestamp":"2026-07-24T10:28:46Z"}"#,
            r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"thinking","thinking":"hm"},{"type":"text","text":"Running them first."},{"type":"tool_use","name":"Bash","input":{"command":"bun run test"}}]},"timestamp":"2026-07-24T10:28:47Z"}"#,
            r#"{"type":"user","message":{"role":"user","content":[{"type":"tool_result","content":"ok"}]},"timestamp":"2026-07-24T10:28:48Z"}"#,
            r#"{"type":"assistant","isSidechain":true,"message":{"role":"assistant","content":[{"type":"text","text":"subagent"}]}}"#,
            "not json",
        ];
        fs::write(dir.join("a.jsonl"), lines.join("\n")).unwrap();

        let t = read_in(&home, CliKind::Claude, Some(cwd), now_secs());
        let got: Vec<(Speaker, &str)> = t.lines.iter().map(|l| (l.speaker, l.text.as_str())).collect();
        assert_eq!(
            got,
            [(Speaker::You, "Fix the failing tests"), (Speaker::Cli, "Running them first."), (Speaker::Tool, "Bash bun run test")]
        );
        assert!(t.source.unwrap().ends_with("a.jsonl"));
        assert_eq!(t.lines[0].at, parse_time("2026-07-24T10:28:46Z"));

        // A transcript last written before the process started belongs to an earlier session.
        let t = read_in(&home, CliKind::Claude, Some(cwd), now_secs() + 3600);
        assert!(t.lines.is_empty() && t.note.unwrap().contains("has not written a transcript"));
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn ccs_transcript_can_come_from_an_account_profile() {
        let home = temp_home("ccs");
        let cwd = Path::new(r"C:\w\app");
        let dir = home.join(".ccs").join("instances").join("work").join("projects").join("C--w-app");
        fs::create_dir_all(&dir).unwrap();
        let line = r#"{"type":"user","message":{"role":"user","content":"Ship it"},"timestamp":"2026-09-28T10:00:00Z"}"#;
        fs::write(dir.join("s.jsonl"), line).unwrap();

        let t = read_in(&home, CliKind::Ccs, Some(cwd), now_secs());
        assert_eq!(t.lines.iter().map(|l| l.text.as_str()).collect::<Vec<_>>(), ["Ship it"]);
        // Claude Code itself reads only ~/.claude.
        assert!(read_in(&home, CliKind::Claude, Some(cwd), now_secs()).lines.is_empty());
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn codex_transcript_is_the_newest_rollout_for_the_folder() {
        let home = temp_home("codex");
        let day = home.join(".codex").join("sessions").join("2026").join("09").join("25");
        fs::create_dir_all(&day).unwrap();
        let meta = |cwd: &str| format!(r#"{{"timestamp":"2026-09-25T13:00:00Z","type":"session_meta","payload":{{"cwd":{}}}}}"#, serde_json::to_string(cwd).unwrap());
        let other = [meta(r"C:\elsewhere"), r#"{"type":"event_msg","payload":{"type":"user_message","message":"not this one"}}"#.into()];
        fs::write(day.join("rollout-b.jsonl"), other.join("\n")).unwrap();
        let mine = [
            meta("C:/Users/me/Project/uninote/"),
            r#"{"timestamp":"2026-09-25T13:00:01Z","type":"response_item","payload":{"type":"message","role":"developer","content":[]}}"#.into(),
            r#"{"timestamp":"2026-09-25T13:00:02Z","type":"event_msg","payload":{"type":"user_message","message":"Write the API docs"}}"#.into(),
            r#"{"timestamp":"2026-09-25T13:00:03Z","type":"response_item","payload":{"type":"function_call","name":"shell_command","arguments":"{\"command\":\"ls docs\"}"}}"#.into(),
            r#"{"timestamp":"2026-09-25T13:00:04Z","type":"response_item","payload":{"type":"custom_tool_call","name":"apply_patch","input":"*** Begin Patch\n*** Update File: docs/api.md\n"}}"#.into(),
            r#"{"timestamp":"2026-09-25T13:00:05Z","type":"event_msg","payload":{"type":"agent_message","message":"Docs are written."}}"#.into(),
        ];
        fs::write(day.join("rollout-a.jsonl"), mine.join("\n")).unwrap();

        let t = read_in(&home, CliKind::Codex, Some(Path::new(r"C:\Users\me\Project\uninote")), now_secs());
        let got: Vec<&str> = t.lines.iter().map(|l| l.text.as_str()).collect();
        assert_eq!(got, ["Write the API docs", "shell_command ls docs", "apply_patch Update File: docs/api.md", "Docs are written."]);
        assert!(t.source.unwrap().ends_with("rollout-a.jsonl"));
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn opencode_transcript_comes_from_its_database_read_only() {
        let home = temp_home("opencode");
        let dir = home.join(".local").join("share").join("opencode");
        fs::create_dir_all(&dir).unwrap();
        let db = dir.join("opencode.db");
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as i64;
        {
            let c = Connection::open(&db).unwrap();
            c.execute_batch(
                "CREATE TABLE session (id TEXT PRIMARY KEY, parent_id TEXT, directory TEXT NOT NULL, time_updated INTEGER NOT NULL);
                 CREATE TABLE message (id TEXT PRIMARY KEY, session_id TEXT NOT NULL, data TEXT NOT NULL);
                 CREATE TABLE part (id TEXT PRIMARY KEY, message_id TEXT NOT NULL, session_id TEXT NOT NULL, time_created INTEGER NOT NULL, data TEXT NOT NULL);",
            )
            .unwrap();
            c.execute("INSERT INTO session VALUES ('s1', NULL, 'C:/w/app', ?1)", [now]).unwrap();
            c.execute("INSERT INTO session VALUES ('child', 's1', 'C:/w/app', ?1)", [now + 5]).unwrap();
            c.execute(r#"INSERT INTO message VALUES ('m1', 's1', '{"role":"user"}')"#, []).unwrap();
            c.execute(r#"INSERT INTO message VALUES ('m2', 's1', '{"role":"assistant"}')"#, []).unwrap();
            let part = |id: &str, m: &str, at: i64, data: &str| {
                c.execute("INSERT INTO part VALUES (?1, ?2, 's1', ?3, ?4)", params![id, m, at, data]).unwrap();
            };
            part("p1", "m1", now, r#"{"type":"text","text":"Add a README"}"#);
            part("p2", "m1", now + 1, r#"{"type":"text","text":"context","synthetic":true}"#);
            part("p3", "m2", now + 2, r#"{"type":"step-start"}"#);
            part("p4", "m2", now + 3, r#"{"type":"tool","tool":"write","state":{"input":{"filePath":"C:/w/app/README.md"}}}"#);
            part("p5", "m2", now + 4, r#"{"type":"text","text":"Added it."}"#);
        }
        let t = read_in(&home, CliKind::Opencode, Some(Path::new(r"C:\w\app")), now_secs());
        let got: Vec<(Speaker, &str)> = t.lines.iter().map(|l| (l.speaker, l.text.as_str())).collect();
        assert_eq!(got, [(Speaker::You, "Add a README"), (Speaker::Tool, "write C:/w/app/README.md"), (Speaker::Cli, "Added it.")]);
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn unknown_folders_and_gemini_explain_themselves() {
        let home = temp_home("none");
        assert!(read_in(&home, CliKind::Claude, None, 0).note.unwrap().contains("did not share this process's folder"));
        assert_eq!(read_in(&home, CliKind::Gemini, Some(Path::new("C:/w")), 0).note.as_deref(), Some("Transcript not available for this CLI."));
        assert!(read_in(&home, CliKind::Codex, Some(Path::new("C:/w")), 0).note.unwrap().starts_with("Codex CLI has not written"));
        let _ = fs::remove_dir_all(&home);
    }

    // Trimmed from the session file of a Pi run that wrote hello.txt.
    #[test]
    fn pi_transcript_is_the_newest_session_file_for_the_folder() {
        let home = temp_home("pi");
        let dir = home.join(".pi").join("agent").join("sessions").join("--C--Users-me-Project-app--");
        fs::create_dir_all(&dir).unwrap();
        let lines = [
            r#"{"type":"session","version":3,"id":"01a0e43a","timestamp":"2026-09-27T18:57:31.919Z","cwd":"C:\\Users\\me\\Project\\app"}"#,
            r#"{"type":"model_change","id":"2388c714","parentId":null,"timestamp":"2026-09-27T18:57:32.176Z","provider":"mock","modelId":"mock-model"}"#,
            r#"{"type":"message","id":"679d7380","timestamp":"2026-09-27T18:57:32.180Z","message":{"role":"system","content":"","sections":{"preamble":"You are an expert coding assistant"}}}"#,
            r#"{"type":"message","id":"b9cedcd4","timestamp":"2026-09-27T18:57:32.190Z","message":{"role":"user","content":[{"type":"text","text":"Create hello.txt"}]}}"#,
            r#"{"type":"message","id":"8e838c0b","timestamp":"2026-09-27T18:57:32.411Z","message":{"role":"assistant","content":[{"type":"text","text":"Writing the file."},{"type":"toolCall","id":"call_1","name":"write","arguments":{"path":"hello.txt","content":"hi"}}]}}"#,
            r#"{"type":"message","id":"d15fb19a","timestamp":"2026-09-27T18:57:32.417Z","message":{"role":"toolResult","toolName":"write","content":[{"type":"text","text":"Successfully wrote to hello.txt"}]}}"#,
            r#"{"type":"message","id":"ec9b1a80","timestamp":"2026-09-27T18:57:32.427Z","message":{"role":"assistant","content":[{"type":"text","text":"Wrote hello.txt."}]}}"#,
        ];
        fs::write(dir.join("2026-09-27T18-57-31-919Z_01a0e43a.jsonl"), lines.join("\n")).unwrap();

        let t = read_in(&home, CliKind::Pi, Some(Path::new(r"C:\Users\me\Project\app")), now_secs());
        let got: Vec<(Speaker, &str)> = t.lines.iter().map(|l| (l.speaker, l.text.as_str())).collect();
        assert_eq!(
            got,
            [(Speaker::You, "Create hello.txt"), (Speaker::Cli, "Writing the file."), (Speaker::Tool, "write hello.txt"), (Speaker::Cli, "Wrote hello.txt.")]
        );
        assert_eq!(t.lines[0].at, parse_time("2026-09-27T18:57:32.190Z"));
        assert!(read_in(&home, CliKind::Pi, Some(Path::new(r"C:\elsewhere")), now_secs()).note.unwrap().starts_with("Pi has not written"));
        let _ = fs::remove_dir_all(&home);
    }

    // Trimmed from an omp 18.3.5 session file in a folder under the home folder.
    #[test]
    fn omp_transcript_is_read_from_the_home_relative_folder() {
        let home = temp_home("omp");
        let dir = home.join(".omp").join("agent").join("sessions").join("-project-app");
        fs::create_dir_all(&dir).unwrap();
        let lines = [
            r#"{"type":"title","v":1,"title":"","updatedAt":"2026-09-27T18:51:57.379Z","pad":"    "}"#,
            r#"{"type":"session","version":3,"id":"01a0e435","timestamp":"2026-09-27T18:51:57.379Z","cwd":"C:\\Users\\me\\project\\app"}"#,
            r#"{"type":"model_change","id":"3402269f","parentId":null,"timestamp":"2026-09-27T18:51:57.673Z","model":"amazon-bedrock/us.anthropic.claude-opus-5-5"}"#,
            r#"{"type":"message","id":"b7b2e910","parentId":"6010ae09","timestamp":"2026-09-27T18:52:59.645Z","message":{"role":"user","content":[{"type":"text","text":"hi"}],"attribution":"user"}}"#,
            r#"{"type":"message","id":"7c3405e8","parentId":"b7b2e910","timestamp":"2026-09-27T18:53:02.586Z","message":{"role":"assistant","content":[{"type":"text","text":"Hai! Mau dikerjakan apa?"}],"stopReason":"stop"}}"#,
            r#"{"type":"custom","customType":"session_exit","data":{"reason":"dispose","kind":"normal"},"id":"0c641df5","timestamp":"2026-09-27T18:53:09.647Z"}"#,
        ];
        fs::write(dir.join("2026-09-27T18-51-57-379Z_01a0e435.jsonl"), lines.join("\n")).unwrap();

        let t = read_in(&home, CliKind::Omp, Some(&home.join("project").join("app")), now_secs());
        let got: Vec<(Speaker, &str)> = t.lines.iter().map(|l| (l.speaker, l.text.as_str())).collect();
        assert_eq!(got, [(Speaker::You, "hi"), (Speaker::Cli, "Hai! Mau dikerjakan apa?")]);
        assert!(read_in(&home, CliKind::Omp, Some(Path::new(r"C:\elsewhere")), now_secs()).note.unwrap().starts_with("omp has not written"));
        let _ = fs::remove_dir_all(&home);
    }
}
