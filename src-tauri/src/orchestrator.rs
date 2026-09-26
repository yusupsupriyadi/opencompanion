//! Orchestrator chat (PRD section C). An installed CLI runs headless as the planner and
//! answers with a reply plus dispatch proposals. The planner can look at the user's project
//! folders (read-only: Claude Code gets Read, Glob and Grep inside those folders only; Codex a
//! read-only sandbox; OpenCode has edit and bash denied) but never runs commands or changes
//! anything. Instead of a CLI, a model behind an OpenAI-compatible endpoint can plan; it has no
//! tools, so it sees folder names only. Every card is validated here before Run is enabled (FR-23).

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::Value;

use crate::cli::{CliInstall, CliKind};
use crate::db::{self, ChatMessage, Db, DispatchCard, Mode, PlannerApi, SessionInfo};
use crate::monitor::ExternalSession;
use crate::proc;
use crate::projects::{self, ProjectFolder};

const PLAN_TIMEOUT: Duration = Duration::from_secs(300);

pub fn schema() -> Value {
    serde_json::json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "reply": { "type": "string" },
            "dispatches": {
                "type": "array",
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "properties": {
                        "cli": { "type": "string", "enum": ["claude", "codex", "opencode", "gemini"] },
                        "title": { "type": "string" },
                        "folder": { "type": "string" },
                        "prompt": { "type": "string" },
                        "mode": { "type": "string", "enum": ["interactive", "headless"] },
                        "reason": { "type": "string" }
                    },
                    "required": ["cli", "title", "folder", "prompt", "mode", "reason"]
                }
            },
            "follow_ups": {
                "type": "array",
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "properties": {
                        "session": { "type": "string" },
                        "prompt": { "type": "string" },
                        "reason": { "type": "string" }
                    },
                    "required": ["session", "prompt", "reason"]
                }
            }
        },
        "required": ["reply", "dispatches", "follow_ups"]
    })
}

const SYSTEM: &str = "You are the planner inside OpenCompanion, a desktop app that starts AI coding CLIs \
(Claude Code, Codex CLI, OpenCode, Gemini CLI) on the user's own computer. You never run commands or \
change files. Your whole answer is one JSON object: `reply` is a short message to the user in the \
language they wrote in, and `dispatches` lists the CLI sessions you propose. The user presses Run on \
each card before anything starts, so proposing a card is always safe.\n\
Folders:\n\
- The user's project folders are listed under Known folders with their real paths. When the user \
names a project, match it to a known folder by name (ignore case, dashes, underscores and spaces) and \
use that exact path. Never ask for a path that is in the list, and never write example or guessed paths.\n\
- If several known folders fit equally, pick the most likely one and say which in `reply`. Ask only \
when nothing in the list fits.\n\
- `@` before a path or a known folder's name marks the folder the user picked (a path with spaces is \
in double quotes). Use that folder for the card, even when it is not under Known folders.\n\
Deciding:\n\
- Prefer proposing a card over asking. If a detail is unclear, choose a sensible default, write it into \
the card's prompt as an assumption, and mention it in one line of `reply`.\n\
- When the user confirms or answers a question (\"yes\", \"iya\", \"ok\", \"that's right\"), stop asking \
and propose the card for the task discussed so far.\n\
- Ask at most one short question per answer, and only when a card would be wrong without the answer.\n\
Cards:\n\
- Only use a CLI from the installed list. If the user names one, use it.\n\
- `title` names the task in at most six words, in the language the user wrote in, like a to-do item \
(\"Custom title bar\", \"Perbaiki tes login\"). The session is shown under this name.\n\
- `prompt` is the complete, self-contained task for the coding CLI, in English, including everything \
the user said in earlier messages about this task. It tells the CLI to look at the code first.\n\
- Use `headless` for a well-defined task that can run unattended; `interactive` when the CLI will \
likely need to ask the user things, or the user wants to watch and type.\n\
- `reason` is one sentence on why this CLI and mode.\n\
- For a question about what is running, answer from the sessions lists with `dispatches` empty.\n\
- Propose several cards only for several independent tasks.\n\
Follow-ups:\n\
- To add to work a session in OpenCompanion is already doing (\"tell it to also...\", \"ask the Codex session \
to...\"), put a card under `follow_ups` instead of `dispatches`: `session` is the session's id exactly as listed, \
`prompt` the message to send it, in English, and `reason` one sentence. The user presses Send on it first.\n\
- Only use a session whose line says `takes a message: yes`. Never invent an id. Otherwise propose a new session.\n\
- `follow_ups` is empty when there is nothing to send to a session.";

const READ_HINT: &str = "You may use the Read, Glob and Grep tools to look inside the known folders \
when it helps you pick the right folder or write a sharper prompt (for example to see the framework \
or where a screen lives). Keep it to a few quick looks; the coding CLI does the real work.";

pub struct PlanInput<'a> {
    pub message: &'a str,
    pub history: &'a [ChatMessage],
    pub clis: &'a [CliInstall],
    pub folders: &'a [ProjectFolder],
    pub sessions: &'a [SessionInfo],
    pub outside: &'a [ExternalSession],
    /// Folders the planner may read. Empty: no file access at all.
    pub read_dirs: &'a [PathBuf],
}

pub struct Plan {
    pub reply: String,
    pub cards: Vec<DispatchCard>,
}

/// The text handed to the planner: context first (FR-24), then recent turns, then the ask.
pub fn prompt_text(input: &PlanInput) -> String {
    let mut out = String::from("## Installed CLIs\n");
    for c in input.clis.iter().filter(|c| c.path.is_some()) {
        out.push_str(&format!(
            "- {} ({}): version {}\n",
            c.kind.bin(),
            c.label,
            c.version.as_deref().unwrap_or("unknown")
        ));
    }
    out.push_str("\n## Known folders (name: path, what is in it)\n");
    if input.folders.is_empty() {
        out.push_str("- none found\n");
    }
    for f in input.folders {
        let kind = if f.markers.is_empty() { String::new() } else { format!(" ({})", f.markers.join(", ")) };
        out.push_str(&format!("- {}: {}{kind}\n", f.name, f.path));
    }
    out.push_str("\n## Sessions in OpenCompanion (newest first)\n");
    if input.sessions.is_empty() {
        out.push_str("- none\n");
    }
    for s in input.sessions.iter().take(12) {
        out.push_str(&format!(
            "- id {}: {} in {} ({}): {} · status {} · last: {} · takes a message: {}\n",
            s.id,
            s.cli.label(),
            s.cwd,
            if s.mode == Mode::Headless { "headless" } else { "interactive" },
            s.title,
            s.status.as_str(),
            s.last_event.as_deref().unwrap_or("nothing yet"),
            if session_problem(s).is_none() { "yes" } else { "no" }
        ));
    }
    if !input.outside.is_empty() {
        out.push_str("\n## CLIs open in other terminals (read-only for OpenCompanion)\n");
        for o in input.outside {
            out.push_str(&format!(
                "- {} ({:?}) in {}\n",
                o.kind.label(),
                o.mode,
                o.cwd.as_deref().map(|p| p.display().to_string()).unwrap_or_else(|| "an unknown folder".into())
            ));
        }
    }
    let recent: Vec<&ChatMessage> = input.history.iter().rev().take(10).collect();
    if !recent.is_empty() {
        out.push_str("\n## Conversation so far\n");
        for m in recent.into_iter().rev() {
            let who = if m.role == "user" { "User" } else { "Planner" };
            out.push_str(&format!("{who}: {}\n", m.text.trim()));
            for c in &m.cards {
                let kind = match &c.target {
                    Some(id) => format!("follow-up for session {id}"),
                    None => "card".to_string(),
                };
                out.push_str(&format!(
                    "  ({kind}: {} in {} · {} · {})\n",
                    c.cli.bin(),
                    c.folder,
                    c.prompt.lines().next().unwrap_or_default(),
                    c.state
                ));
            }
        }
    }
    out.push_str("\n## New message from the user\n");
    out.push_str(input.message.trim());
    out.push('\n');
    out
}

/// Existing folders the user pointed at with `@C:\path` or `@"C:\path with spaces"`. A bare
/// `@name` is left to the planner, which matches it against the known folders.
pub fn mentioned_dirs(text: &str) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    let mut rest = text;
    while let Some(at) = rest.find('@') {
        // `me@host` is not a mention: the `@` has to start a word.
        let starts_word = at == 0 || rest[..at].ends_with(char::is_whitespace);
        let after = &rest[at + 1..];
        let (raw, next) = match after.strip_prefix('"') {
            Some(quoted) => match quoted.find('"') {
                Some(end) => (&quoted[..end], &quoted[end + 1..]),
                None => (quoted, ""),
            },
            None => {
                let end = after.find(char::is_whitespace).unwrap_or(after.len());
                (&after[..end], &after[end..])
            }
        };
        rest = next;
        let path = PathBuf::from(raw.trim_end_matches([',', '.', ';', ')', '!', '?']));
        if starts_word && path.is_absolute() && path.is_dir() && !out.contains(&path) {
            out.push(path);
        }
    }
    out
}

/// Pulls the plan object out of whatever the CLI printed: a JSON reply, or JSON inside a code
/// fence. `None` means the answer is plain text (FR-23).
pub fn extract_plan(text: &str) -> Option<Value> {
    let t = text.trim();
    if let Ok(v) = serde_json::from_str::<Value>(t) {
        if v.get("reply").is_some() {
            return Some(v);
        }
    }
    let start = t.find('{')?;
    let end = t.rfind('}')?;
    (end > start)
        .then(|| serde_json::from_str::<Value>(&t[start..=end]).ok())
        .flatten()
        .filter(|v| v.get("reply").is_some())
}

/// Why a message cannot go to `s` right now (PRD FR-25), or `None` when it can: a running
/// terminal takes typed text, a running Claude Code turn takes a message, and a finished headless
/// session continues its conversation.
pub fn session_problem(s: &SessionInfo) -> Option<String> {
    let who = s.cli.label();
    match (s.mode, s.status.is_live()) {
        (_, true) if s.status == crate::db::Status::Waiting => {
            Some(format!("{who} is waiting for an answer in this session. Answer it first, then send this."))
        }
        (Mode::Interactive, true) => None,
        (Mode::Interactive, false) => Some("This terminal has closed. Resume the session, then send this card.".into()),
        (Mode::Headless, true) if s.cli == CliKind::Claude => None,
        (Mode::Headless, true) => Some(format!("{who} is still on this session's turn. Send this card when it is done.")),
        (Mode::Headless, false) if s.cli_session_id.is_some() => None,
        (Mode::Headless, false) => Some(format!("{who} did not report a session id, so this conversation cannot continue.")),
    }
}

/// A follow-up card is checked against its session: it has to exist and take a message now.
pub fn validate_target(card: &mut DispatchCard, clis: &[CliInstall], session: Option<&SessionInfo>) {
    validate(card, clis);
    if card.problem.is_some() || card.target.is_none() {
        return;
    }
    card.problem = match session {
        None => Some("This session is no longer in OpenCompanion.".into()),
        Some(s) => session_problem(s),
    };
}

/// FR-23: a card can only run when its CLI is installed and its folder exists.
pub fn validate(card: &mut DispatchCard, clis: &[CliInstall]) {
    let installed = clis.iter().any(|c| c.kind == card.cli && c.path.is_some());
    card.problem = if !installed {
        Some(format!("{} is not installed on this computer.", card.cli.label()))
    } else if card.folder.trim().is_empty() || !Path::new(card.folder.trim()).is_dir() {
        Some(format!("The folder {} does not exist.", card.folder))
    } else if card.prompt.trim().is_empty() {
        Some("The card has no prompt.".into())
    } else if card.mode == Mode::Headless && card.cli == CliKind::Gemini {
        Some("Headless mode for Gemini CLI is not supported yet.".into())
    } else {
        None
    };
}

/// Follow-ups for sessions the planner was shown. An id it made up names no session and is dropped.
fn follow_ups_from(v: &Value, clis: &[CliInstall], sessions: &[SessionInfo]) -> Vec<DispatchCard> {
    v["follow_ups"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|f| {
            let id = f["session"].as_str()?.trim();
            let s = sessions.iter().find(|s| s.id == id)?;
            let mut card = DispatchCard {
                id: db::new_id(),
                cli: s.cli,
                title: s.title.clone(),
                folder: s.cwd.clone(),
                prompt: f["prompt"].as_str().unwrap_or_default().trim().to_string(),
                mode: s.mode,
                reason: f["reason"].as_str().unwrap_or_default().trim().to_string(),
                problem: None,
                state: "proposed".into(),
                session_id: None,
                task_id: None,
                target: Some(s.id.clone()),
                auto: false,
            };
            validate_target(&mut card, clis, Some(s));
            Some(card)
        })
        .collect()
}

fn cards_from(v: &Value, clis: &[CliInstall], known: &[ProjectFolder]) -> Vec<DispatchCard> {
    v["dispatches"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|d| {
            let cli = CliKind::from_bin(d["cli"].as_str()?)?;
            let written = d["folder"].as_str().unwrap_or_default().trim().to_string();
            let mut card = DispatchCard {
                id: db::new_id(),
                cli,
                title: d["title"].as_str().unwrap_or_default().trim().chars().take(80).collect(),
                // A bare project name or a slightly wrong path still lands on the real folder.
                folder: projects::resolve(&written, known).unwrap_or(written),
                prompt: d["prompt"].as_str().unwrap_or_default().trim().to_string(),
                mode: if d["mode"] == "interactive" { Mode::Interactive } else { Mode::Headless },
                reason: d["reason"].as_str().unwrap_or_default().trim().to_string(),
                problem: None,
                state: "proposed".into(),
                session_id: None,
                task_id: None,
                target: None,
                auto: false,
            };
            validate(&mut card, clis);
            Some(card)
        })
        .collect()
}

fn plan_from(v: &Value, input: &PlanInput) -> Plan {
    let mut cards = cards_from(v, input.clis, input.folders);
    cards.extend(follow_ups_from(v, input.clis, input.sessions));
    Plan {
        reply: v["reply"].as_str().unwrap_or_default().trim().to_string(),
        cards,
    }
}

fn plan_from_text(text: &str, input: &PlanInput) -> Plan {
    match extract_plan(text) {
        Some(v) => plan_from(&v, input),
        None => Plan {
            reply: text.trim().to_string(),
            cards: vec![],
        },
    }
}

/// Everything the planner is told, gathered from the store, the disk and the process list.
pub struct Context {
    pub folders: Vec<ProjectFolder>,
    pub read_dirs: Vec<PathBuf>,
    pub sessions: Vec<SessionInfo>,
}

pub fn gather(db: &Db, outside: &[ExternalSession]) -> Result<Context, String> {
    let settings = db.settings()?;
    let recent: Vec<String> = db.projects(20)?.into_iter().map(|p| p.path).collect();
    let sessions = db.sessions(12)?;
    let mut open: Vec<String> = sessions.iter().filter(|s| s.status.is_live()).map(|s| s.cwd.clone()).collect();
    open.extend(outside.iter().filter_map(|o| o.cwd.as_ref().map(|p| p.display().to_string())));
    let folders = projects::discover(&recent, &open, &settings.project_roots, 80);
    let read_dirs = if settings.planner_can_read { read_dirs_for(&settings.project_roots, &folders) } else { vec![] };
    Ok(Context {
        folders,
        read_dirs,
        sessions,
    })
}

/// Project roots plus any known folder that lives outside them, so the planner's read access
/// covers exactly the folders it was told about.
fn read_dirs_for(roots: &[String], folders: &[ProjectFolder]) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = if roots.is_empty() {
        projects::default_roots()
    } else {
        roots.iter().map(PathBuf::from).filter(|p| p.is_dir()).collect()
    };
    for f in folders {
        let p = PathBuf::from(&f.path);
        if !dirs.iter().any(|d| p.starts_with(d)) {
            dirs.push(p);
        }
    }
    dirs.truncate(24);
    dirs
}

/// Runs the planner CLI once.
pub fn run(kind: CliKind, exe: &Path, work_dir: &Path, extra: &[String], input: &PlanInput) -> Result<Plan, String> {
    fs::create_dir_all(work_dir).map_err(|e| e.to_string())?;
    let can_read = !input.read_dirs.is_empty();
    let prompt = prompt_text(input);
    let system = if can_read { format!("{SYSTEM}\n{READ_HINT}") } else { SYSTEM.to_string() };
    let schema = schema().to_string();
    let mut cmd = proc::command(exe);
    // With read access the planner starts in the first project root; without it, in an empty
    // folder where there is nothing of the user's to find.
    let cwd = if can_read { input.read_dirs[0].clone() } else { work_dir.to_path_buf() };
    cmd.current_dir(&cwd);
    let stdin: String = match kind {
        CliKind::Claude => {
            let tools = if can_read { "Read,Glob,Grep" } else { "" };
            cmd.args([
                "-p",
                "--output-format",
                "json",
                "--restricted",
                "--strict-mcp-config",
                "--tools",
                tools,
                "--permission-prompts",
                "none",
                "--no-session-persistence",
                "--json-schema",
                &schema,
                "--append-system-prompt",
                &system,
            ]);
            for d in input.read_dirs.iter().skip(1) {
                cmd.arg("--add-dir").arg(d);
            }
            cmd.args(extra);
            prompt
        }
        CliKind::Codex => {
            let schema_file = work_dir.join("plan-schema.json");
            fs::write(&schema_file, &schema).map_err(|e| e.to_string())?;
            cmd.args(["exec", "--json", "--skip-git-repo-check", "--ephemeral", "-s", "read-only", "-C"]);
            cmd.arg(&cwd);
            cmd.arg("--output-schema").arg(&schema_file);
            cmd.args(extra);
            cmd.arg("-");
            format!("{system}\n\n{prompt}")
        }
        CliKind::Opencode => {
            // Read-only: edits, shell commands and web fetches are refused.
            cmd.env("OPENCODE_PERMISSION", r#"{"edit":"deny","bash":"deny","webfetch":"deny"}"#);
            cmd.args(["run", "--format", "json", "--dir"]);
            cmd.arg(&cwd);
            cmd.args(extra);
            cmd.arg(format!("{}\n\n{prompt}", json_only(&system, &schema)));
            String::new()
        }
        CliKind::Gemini => return Err("Gemini CLI cannot be the chat planner yet.".into()),
    };

    let out = proc::run_with_input(cmd, (!stdin.is_empty()).then_some(stdin.as_str()), PLAN_TIMEOUT)
        .map_err(|e| format!("Could not start {}: {e}", kind.label()))?;
    if out.timed_out {
        return Err(format!("{} did not answer within {} s.", kind.label(), PLAN_TIMEOUT.as_secs()));
    }

    match kind {
        CliKind::Claude => {
            let v: Value = serde_json::from_str(out.stdout.trim()).map_err(|_| {
                first_line_or(&out.stderr, &out.stdout, "Claude Code returned no result.")
            })?;
            if v["is_error"] == true {
                return Err(v["result"].as_str().unwrap_or("Claude Code reported an error.").to_string());
            }
            if v["structured_output"].is_object() {
                return Ok(plan_from(&v["structured_output"], input));
            }
            Ok(plan_from_text(v["result"].as_str().unwrap_or_default(), input))
        }
        CliKind::Codex => {
            let mut last = String::new();
            let mut failure = None;
            for line in out.stdout.lines() {
                let Ok(v) = serde_json::from_str::<Value>(line) else { continue };
                if v["type"] == "item.completed" && v["item"]["type"] == "agent_message" {
                    last = v["item"]["text"].as_str().unwrap_or_default().to_string();
                }
                if v["type"] == "turn.failed" {
                    failure = v["error"]["message"].as_str().map(str::to_owned);
                }
            }
            if let Some(f) = failure.filter(|_| last.is_empty()) {
                return Err(f);
            }
            if last.is_empty() {
                return Err(first_line_or(&out.stderr, &out.stdout, "Codex returned no answer."));
            }
            Ok(plan_from_text(&last, input))
        }
        _ => {
            let text: String = out
                .stdout
                .lines()
                .filter_map(|l| serde_json::from_str::<Value>(l).ok())
                .filter(|v| v["type"] == "text")
                .filter_map(|v| v["part"]["text"].as_str().map(str::to_owned))
                .collect::<Vec<_>>()
                .join("\n");
            if text.trim().is_empty() {
                return Err(first_line_or(&out.stderr, &out.stdout, "OpenCode returned no answer."));
            }
            Ok(plan_from_text(&text, input))
        }
    }
}

/// For a planner that cannot be handed the schema itself.
fn json_only(system: &str, schema: &str) -> String {
    format!("{system}\nAnswer with only the JSON object, matching this schema: {schema}")
}

/// Asks the model behind an OpenAI-compatible endpoint once. It gets the same context as a CLI
/// minus file access.
pub fn run_api(api: &PlannerApi, input: &PlanInput) -> Result<Plan, String> {
    if let Some(problem) = api.problem() {
        return Err(problem);
    }
    let url = completions_url(&api.base_url);
    let body = serde_json::json!({
        "model": api.model.trim(),
        "stream": false,
        "messages": [
            { "role": "system", "content": json_only(SYSTEM, &schema().to_string()) },
            { "role": "user", "content": prompt_text(input) },
        ],
    });
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(PLAN_TIMEOUT))
        .timeout_connect(Some(Duration::from_secs(10)))
        .http_status_as_error(false)
        .build()
        .into();
    let mut req = agent.post(&url).header("Content-Type", "application/json");
    let key = api.api_key.trim();
    if !key.is_empty() {
        req = req.header("Authorization", format!("Bearer {key}"));
    }
    let mut res = req.send(body.to_string()).map_err(|e| match e {
        ureq::Error::Timeout(_) => format!("The provider did not answer within {} s.", PLAN_TIMEOUT.as_secs()),
        e => format!("Could not reach {url}: {e}"),
    })?;
    let status = res.status().as_u16();
    let text = res.body_mut().read_to_string().map_err(|e| format!("Could not read the provider's answer: {e}"))?;
    let v: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
    if !(200..300).contains(&status) || !v["error"].is_null() {
        return Err(api_error(status, &v, &text));
    }
    let answer = answer_text(&v).ok_or("The provider's answer had no text.")?;
    Ok(plan_from_text(&answer, input))
}

/// `base` plus `/chat/completions`, unless the full address was pasted already.
fn completions_url(base: &str) -> String {
    let base = base.trim().trim_end_matches('/');
    if base.ends_with("/chat/completions") {
        base.to_string()
    } else {
        format!("{base}/chat/completions")
    }
}

/// The model's text, without the `<think>` block that reasoning models served by Ollama or
/// LM Studio put in front of it.
fn answer_text(v: &Value) -> Option<String> {
    let text = match &v["choices"][0]["message"]["content"] {
        Value::String(s) => s.clone(),
        Value::Array(parts) => parts.iter().filter_map(|p| p["text"].as_str()).collect::<Vec<_>>().join("\n"),
        _ => return None,
    };
    let text = text.rsplit_once("</think>").map_or(text.as_str(), |(_, after)| after).trim().to_string();
    (!text.is_empty()).then_some(text)
}

/// The provider's own words: OpenAI-style `error.message`, a bare `error` string (Ollama), or
/// the first line of the body.
fn api_error(status: u16, v: &Value, body: &str) -> String {
    let said = v["error"]["message"]
        .as_str()
        .or_else(|| v["error"].as_str())
        .or_else(|| v["message"].as_str())
        .map(str::to_owned)
        .unwrap_or_else(|| first_line_or(body, "", "no details"));
    format!("The provider answered {status}: {said}")
}

fn first_line_or(stderr: &str, stdout: &str, fallback: &str) -> String {
    stderr
        .lines()
        .chain(stdout.lines())
        .map(str::trim)
        .find(|l| !l.is_empty())
        .map(|l| l.chars().take(300).collect())
        .unwrap_or_else(|| fallback.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clis() -> Vec<CliInstall> {
        vec![CliInstall {
            kind: CliKind::Claude,
            label: "Claude Code",
            path: Some(PathBuf::from("claude.exe")),
            version: Some("2.1.282".into()),
            tested: true,
            error: None,
        }]
    }

    fn folder(path: &Path) -> ProjectFolder {
        ProjectFolder {
            path: path.display().to_string(),
            name: path.file_name().unwrap().to_string_lossy().into_owned(),
            markers: vec!["git", "package.json"],
            source: "root",
        }
    }

    fn input<'a>(clis: &'a [CliInstall], folders: &'a [ProjectFolder]) -> PlanInput<'a> {
        PlanInput {
            message: "add dark mode",
            history: &[],
            clis,
            folders,
            sessions: &[],
            outside: &[],
            read_dirs: &[],
        }
    }

    fn session(id: &str, mode: Mode, status: crate::db::Status, cwd: &Path) -> SessionInfo {
        SessionInfo {
            id: id.into(),
            cli: CliKind::Claude,
            cwd: cwd.display().to_string(),
            mode,
            title: format!("Task {id}"),
            prompt: String::new(),
            status,
            pid: None,
            cli_session_id: None,
            started_at: 1,
            ended_at: None,
            exit_code: None,
            last_event: None,
            waiting: None,
            source: "manual".into(),
            task_id: None,
            permission_mode: None,
            updated_at: 1,
        }
    }

    #[test]
    fn follow_ups_go_only_to_listed_sessions_and_say_when_one_cannot_take_them() {
        use crate::db::Status;
        let c = clis();
        let cwd = std::env::temp_dir();
        let sessions = [
            session("s-run", Mode::Headless, Status::Running, &cwd),
            session("s-term", Mode::Interactive, Status::Idle, &cwd),
            session("s-closed", Mode::Interactive, Status::Done, &cwd),
        ];
        let mut inp = input(&c, &[]);
        inp.sessions = &sessions;
        let v = serde_json::json!({
            "reply": "Sending it on.",
            "dispatches": [],
            "follow_ups": [
                { "session": "s-run", "prompt": "Also add tests.", "reason": "It is on this task." },
                { "session": "s-term", "prompt": "Run the linter", "reason": "Same folder." },
                { "session": "s-closed", "prompt": "More", "reason": "x" },
                { "session": "made-up", "prompt": "More", "reason": "x" }
            ]
        });
        let p = plan_from(&v, &inp);
        let targets: Vec<_> = p.cards.iter().map(|c| c.target.as_deref().unwrap()).collect();
        assert_eq!(targets, ["s-run", "s-term", "s-closed"]);
        let first = &p.cards[0];
        assert_eq!((first.title.as_str(), first.mode, first.prompt.as_str()), ("Task s-run", Mode::Headless, "Also add tests."));
        assert_eq!(first.folder, cwd.display().to_string());
        assert!(first.problem.is_none() && p.cards[1].problem.is_none());
        assert!(p.cards[2].problem.as_deref().unwrap().contains("terminal has closed"));

        // The planner sees each session's id and whether it takes a message now.
        let text = prompt_text(&inp);
        assert!(text.contains("- id s-run: Claude Code in"));
        assert!(text.contains("takes a message: yes") && text.contains("takes a message: no"));
        assert!(schema()["required"].as_array().unwrap().iter().any(|r| r == "follow_ups"));
    }

    #[test]
    fn a_session_takes_a_message_when_its_cli_can_hear_it() {
        use crate::db::Status;
        let cwd = std::env::temp_dir();
        let mut s = session("a", Mode::Headless, Status::Done, &cwd);
        assert!(session_problem(&s).unwrap().contains("did not report a session id"));
        s.cli_session_id = Some("c1".into());
        assert_eq!(session_problem(&s), None);
        s.status = Status::Running;
        s.cli = CliKind::Codex;
        assert!(session_problem(&s).unwrap().contains("still on this session's turn"));
        s.status = Status::Waiting;
        assert!(session_problem(&s).unwrap().contains("waiting for an answer"));
    }

    #[test]
    fn plain_text_answers_stay_text() {
        let c = clis();
        let p = plan_from_text("Sure, what folder?", &input(&c, &[]));
        assert_eq!(p.reply, "Sure, what folder?");
        assert!(p.cards.is_empty());
    }

    #[test]
    fn json_in_a_fence_becomes_cards_and_is_validated() {
        let dir = std::env::temp_dir();
        let text = format!(
            "```json\n{{\"reply\":\"ok\",\"dispatches\":[{{\"cli\":\"claude\",\"folder\":{:?},\"prompt\":\"do x\",\"mode\":\"headless\",\"reason\":\"r\"}},{{\"cli\":\"codex\",\"folder\":{:?},\"prompt\":\"do y\",\"mode\":\"headless\",\"reason\":\"r\"}}]}}\n```",
            dir.display().to_string(),
            dir.display().to_string()
        );
        let c = clis();
        let p = plan_from_text(&text, &input(&c, &[]));
        assert_eq!(p.reply, "ok");
        assert_eq!(p.cards.len(), 2);
        assert_eq!(p.cards[0].problem, None);
        assert_eq!(p.cards[1].problem.as_deref(), Some("Codex CLI is not installed on this computer."));
    }

    #[test]
    fn a_bare_project_name_lands_on_the_known_folder() {
        let base = std::env::temp_dir().join(format!("air-orch-{}", std::process::id()));
        let project = base.join("ai-remote");
        fs::create_dir_all(&project).unwrap();
        let known = vec![folder(&project)];
        let c = clis();
        let v: Value = serde_json::json!({
            "reply": "ok",
            "dispatches": [{ "cli": "claude", "title": " Custom title bar ", "folder": "ai-remote", "prompt": "custom title bar", "mode": "interactive", "reason": "r" }]
        });
        let p = plan_from(&v, &input(&c, &known));
        assert_eq!(p.cards[0].folder, project.display().to_string());
        assert_eq!(p.cards[0].title, "Custom title bar");
        assert_eq!(p.cards[0].problem, None);
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn missing_folder_blocks_run() {
        let mut card = DispatchCard {
            id: "c".into(),
            cli: CliKind::Claude,
            title: String::new(),
            folder: r"C:\definitely\not\here".into(),
            prompt: "x".into(),
            mode: Mode::Headless,
            reason: String::new(),
            problem: None,
            state: "proposed".into(),
            session_id: None,
            task_id: None,
            target: None,
            auto: false,
        };
        validate(&mut card, &clis());
        assert!(card.problem.unwrap().contains("does not exist"));
    }

    #[test]
    fn prompt_lists_folders_with_markers_before_the_request() {
        let known = vec![folder(Path::new(r"C:\p\uninote"))];
        let c = clis();
        let text = prompt_text(&input(&c, &known));
        let clis_at = text.find("## Installed CLIs").unwrap();
        let ask_at = text.find("add dark mode").unwrap();
        assert!(clis_at < ask_at);
        assert!(text.contains(r"- uninote: C:\p\uninote (git, package.json)"), "{text}");
    }

    #[test]
    fn at_mentions_name_existing_folders_only() {
        let base = std::env::temp_dir().join(format!("air-orch-at-{}", std::process::id()));
        let spaced = base.join("My Projects").join("app");
        let plain = base.join("plain");
        for d in [&spaced, &plain] {
            fs::create_dir_all(d).unwrap();
        }
        let gone = base.join("gone");
        let text = format!(
            "fix @{}, then @\"{}\" and @{}, mail me@{} or @uninote",
            plain.display(),
            spaced.display(),
            gone.display(),
            plain.display()
        );
        assert_eq!(mentioned_dirs(&text), vec![plain.clone(), spaced.clone()]);
        assert!(mentioned_dirs("no mentions here").is_empty());
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn read_access_covers_roots_and_known_folders_outside_them() {
        let base = std::env::temp_dir().join(format!("air-orch-read-{}", std::process::id()));
        let root = base.join("Work");
        let inside = root.join("a");
        let outside = base.join("elsewhere");
        for d in [&inside, &outside] {
            fs::create_dir_all(d).unwrap();
        }
        let dirs = read_dirs_for(&[root.display().to_string()], &[folder(&inside), folder(&outside)]);
        assert_eq!(dirs, vec![root.clone(), outside.clone()]);
        let _ = fs::remove_dir_all(&base);
    }

    /// Answers one request on a free local port with `status` and `body`, and hands back the
    /// request exactly as it arrived.
    fn serve_once(status: &'static str, body: String) -> (String, std::thread::JoinHandle<String>) {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}/v1/", listener.local_addr().unwrap());
        let handle = std::thread::spawn(move || {
            let (mut s, _) = listener.accept().unwrap();
            let mut got = Vec::new();
            let mut buf = [0u8; 8192];
            loop {
                let n = s.read(&mut buf).unwrap();
                if n == 0 {
                    break;
                }
                got.extend_from_slice(&buf[..n]);
                let text = String::from_utf8_lossy(&got).into_owned();
                let Some(end) = text.find("\r\n\r\n") else { continue };
                let len = text[..end]
                    .lines()
                    .find_map(|l| l.to_ascii_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse::<usize>().unwrap()))
                    .unwrap_or(0);
                if got.len() >= end + 4 + len {
                    break;
                }
            }
            let reply = format!(
                "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            s.write_all(reply.as_bytes()).unwrap();
            String::from_utf8_lossy(&got).into_owned()
        });
        (base, handle)
    }

    fn provider(base_url: String, api_key: &str) -> PlannerApi {
        PlannerApi {
            base_url,
            model: "qwen3-coder".into(),
            api_key: api_key.into(),
        }
    }

    #[test]
    fn a_custom_provider_gets_the_context_and_its_json_becomes_cards() {
        let dir = std::env::temp_dir().display().to_string();
        let content = format!(
            "<think>maybe {{not this}}</think>\n```json\n{{\"reply\":\"ok\",\"dispatches\":[{{\"cli\":\"claude\",\"title\":\"Dark mode\",\"folder\":{dir:?},\"prompt\":\"add dark mode\",\"mode\":\"headless\",\"reason\":\"r\"}}]}}\n```"
        );
        let body = serde_json::json!({ "choices": [{ "message": { "role": "assistant", "content": content } }] }).to_string();
        let (base, got) = serve_once("200 OK", body);
        let c = clis();
        let plan = run_api(&provider(base, "sk-test"), &input(&c, &[])).unwrap();
        assert_eq!(plan.reply, "ok");
        assert_eq!(plan.cards.len(), 1);
        assert_eq!(plan.cards[0].problem, None);

        let request = got.join().unwrap();
        assert!(request.starts_with("POST /v1/chat/completions "), "{request}");
        assert!(request.to_ascii_lowercase().contains("authorization: bearer sk-test"), "{request}");
        let sent: Value = serde_json::from_str(&request[request.find("\r\n\r\n").unwrap() + 4..]).unwrap();
        assert_eq!(sent["model"], "qwen3-coder");
        assert!(sent["messages"][0]["content"].as_str().unwrap().contains("matching this schema"));
        assert!(sent["messages"][1]["content"].as_str().unwrap().contains("add dark mode"));
    }

    #[test]
    fn a_provider_error_carries_its_own_words_and_no_key_sends_no_header() {
        let body = serde_json::json!({ "error": { "message": "model 'qwen3-coder' not found" } }).to_string();
        let (base, got) = serve_once("404 Not Found", body);
        let c = clis();
        let err = run_api(&provider(base, ""), &input(&c, &[])).err().unwrap();
        assert_eq!(err, "The provider answered 404: model 'qwen3-coder' not found");
        assert!(!got.join().unwrap().to_ascii_lowercase().contains("authorization:"));
    }

    #[test]
    fn an_unfinished_provider_is_refused_before_any_request() {
        let c = clis();
        let err = run_api(&provider(String::new(), ""), &input(&c, &[])).err().unwrap();
        assert!(err.contains("needs a base URL and a model"), "{err}");
        let err = run_api(&provider("localhost:11434/v1".into(), ""), &input(&c, &[])).err().unwrap();
        assert!(err.contains("http:// or https://"), "{err}");
    }

    #[test]
    fn the_completions_address_is_built_once() {
        assert_eq!(completions_url(" https://openrouter.ai/api/v1/ "), "https://openrouter.ai/api/v1/chat/completions");
        assert_eq!(completions_url("http://localhost:1234/v1/chat/completions"), "http://localhost:1234/v1/chat/completions");
    }
}
