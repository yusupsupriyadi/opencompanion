use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Stdio};
use std::thread;

use crate::cli::CliKind;
use crate::proc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stream {
    Stdout,
    Stderr,
}

pub type LineSink = Box<dyn FnMut(Stream, &str) + Send>;

/// How a prompt reaches the CLI. Claude Code and Codex read it from stdin, which avoids
/// command-line quoting for long or multi-line prompts.
pub struct Invocation {
    pub args: Vec<String>,
    /// Extra environment for the CLI process (OpenCode takes its permission rules this way).
    pub env: Vec<(String, String)>,
    /// Written to stdin right after spawn.
    pub stdin_first: Option<String>,
    /// Keep stdin open for follow-up messages and permission answers (Claude stream-json).
    pub keep_stdin: bool,
}

/// Permission mode for sessions OpenCompanion starts (Settings, New session).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermMode {
    /// Every permission prompt comes to you ("Waiting for you").
    Ask,
    /// Read and plan only, no changes.
    Plan,
    /// The CLI approves routine actions itself.
    Auto,
    /// No permission checks at all.
    Bypass,
}

impl PermMode {
    pub fn parse(s: &str) -> PermMode {
        match s {
            "plan" => PermMode::Plan,
            "auto" => PermMode::Auto,
            "bypass" => PermMode::Bypass,
            _ => PermMode::Ask,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            PermMode::Ask => "ask",
            PermMode::Plan => "plan",
            PermMode::Auto => "auto",
            PermMode::Bypass => "bypass",
        }
    }
}

/// Flags and environment per CLI for a permission mode, checked against each CLI's `--help`
/// on 2026-09-25 (Claude Code 2.1.282, Codex CLI 0.153.4, OpenCode 1.18.32). Gemini CLI is not
/// installed on the test machine, so it gets none. Pi (0.87.1) never asks: only Plan changes
/// anything, by giving it read-only tools.
///
/// `headless_resume` covers `codex exec resume`, which accepts only the bypass flag: the
/// resumed thread keeps the sandbox it started with.
pub fn mode_flags(kind: CliKind, mode: PermMode, interactive: bool, headless_resume: bool) -> (Vec<String>, Vec<(String, String)>) {
    let none = Vec::new();
    let args = match kind {
        CliKind::Claude | CliKind::Ccs => match mode {
            PermMode::Ask => strings(&["--permission-mode", "manual"]),
            PermMode::Plan => strings(&["--permission-mode", "plan"]),
            PermMode::Auto => strings(&["--permission-mode", "auto"]),
            PermMode::Bypass => strings(&["--dangerously-skip-permissions"]),
        },
        CliKind::Codex if headless_resume => match mode {
            PermMode::Bypass => strings(&["--dangerously-bypass-approvals-and-sandbox"]),
            _ => vec![],
        },
        CliKind::Codex => match (mode, interactive) {
            (PermMode::Ask, true) => strings(&["-s", "workspace-write", "-a", "on-request"]),
            (PermMode::Ask, false) => strings(&["-s", "workspace-write"]),
            (PermMode::Plan, true) => strings(&["-s", "read-only", "-a", "on-request"]),
            (PermMode::Plan, false) => strings(&["-s", "read-only"]),
            (PermMode::Auto, _) => strings(&["--approve-for-me"]),
            (PermMode::Bypass, _) => strings(&["--dangerously-bypass-approvals-and-sandbox"]),
        },
        CliKind::Opencode => match mode {
            PermMode::Ask => vec![],
            PermMode::Plan => strings(&["--agent", "plan"]),
            PermMode::Auto | PermMode::Bypass => strings(&["--auto"]),
        },
        CliKind::Gemini => vec![],
        CliKind::Pi => match mode {
            PermMode::Plan => strings(&["--tools", crate::pi::READ_ONLY_TOOLS]),
            _ => vec![],
        },
    };
    let env = if kind == CliKind::Opencode && mode == PermMode::Bypass {
        vec![("OPENCODE_PERMISSION".to_string(), r#"{"*":"allow"}"#.to_string())]
    } else {
        none
    };
    (args, env)
}

/// Per-turn choices. `resume` continues the CLI's own session for a follow-up message.
pub struct TurnOptions<'a> {
    pub mode: PermMode,
    pub resume: Option<&'a str>,
    /// User-configured extra flags from Settings, placed before the prompt argument.
    pub extra: &'a [String],
}

impl Default for TurnOptions<'_> {
    fn default() -> Self {
        Self {
            mode: PermMode::Ask,
            resume: None,
            extra: &[],
        }
    }
}

fn strings(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| s.to_string()).collect()
}

/// Claude Code `-p` with stream-json in and out. `--permission-prompt-tool stdio` routes
/// permission requests to us as `control_request` / `can_use_tool` lines instead of denying
/// them (verified in M0).
pub fn claude_invocation(prompt: &str, opts: &TurnOptions) -> Invocation {
    let mut args = strings(&[
        "-p",
        "--output-format",
        "stream-json",
        "--input-format",
        "stream-json",
        "--verbose",
        "--permission-prompts",
        "host",
        "--permission-prompt-tool",
        "stdio",
    ]);
    let (flags, env) = mode_flags(CliKind::Claude, opts.mode, false, false);
    args.extend(flags);
    if let Some(id) = opts.resume {
        args.extend(strings(&["--resume", id]));
    }
    args.extend(opts.extra.iter().cloned());
    Invocation {
        args,
        env,
        stdin_first: Some(claude_user_message(prompt)),
        keep_stdin: true,
    }
}

pub fn claude_user_message(text: &str) -> String {
    serde_json::json!({
        "type": "user",
        "message": { "role": "user", "content": text },
    })
    .to_string()
}

/// Answer to a `can_use_tool` control request.
pub fn claude_permission_answer(request_id: &str, allow: bool, input: &serde_json::Value) -> String {
    let response = if allow {
        serde_json::json!({ "behavior": "allow", "updatedInput": input })
    } else {
        serde_json::json!({ "behavior": "deny", "message": "Denied in OpenCompanion" })
    };
    serde_json::json!({
        "type": "control_response",
        "response": { "subtype": "success", "request_id": request_id, "response": response },
    })
    .to_string()
}

/// `codex exec` reads the prompt from stdin (`-`). `exec resume` takes no `-s`/`-C`: the
/// resumed thread keeps its own folder and sandbox.
pub fn codex_invocation(cwd: &Path, prompt: &str, opts: &TurnOptions) -> Invocation {
    let resuming = opts.resume.is_some();
    let (flags, env) = mode_flags(CliKind::Codex, opts.mode, false, resuming);
    let mut args = if resuming {
        strings(&["exec", "resume", "--json", "--skip-git-repo-check"])
    } else {
        strings(&["exec", "--json", "--skip-git-repo-check", "-C"])
    };
    if !resuming {
        args.push(cwd.display().to_string());
    }
    args.extend(flags);
    args.extend(opts.extra.iter().cloned());
    if let Some(id) = opts.resume {
        args.push(id.to_string());
    }
    args.push("-".into());
    Invocation {
        args,
        env,
        stdin_first: Some(prompt.to_string()),
        keep_stdin: false,
    }
}

pub fn opencode_invocation(cwd: &Path, prompt: &str, opts: &TurnOptions) -> Invocation {
    let mut args = strings(&["run", "--format", "json", "--dir"]);
    args.push(cwd.display().to_string());
    if let Some(id) = opts.resume {
        args.extend(strings(&["--session", id]));
    }
    let (flags, env) = mode_flags(CliKind::Opencode, opts.mode, false, false);
    args.extend(flags);
    args.extend(opts.extra.iter().cloned());
    args.push(prompt.into());
    Invocation {
        args,
        env,
        stdin_first: None,
        keep_stdin: false,
    }
}

/// `pi --mode json` reads the prompt from stdin when no message argument is given, so a prompt
/// that starts with `@` is not taken for a file to attach.
pub fn pi_invocation(prompt: &str, opts: &TurnOptions) -> Invocation {
    let mut args = strings(&["--mode", "json"]);
    if let Some(id) = opts.resume {
        args.extend(strings(&["--session", id]));
    }
    let (flags, env) = mode_flags(CliKind::Pi, opts.mode, false, false);
    args.extend(flags);
    args.extend(opts.extra.iter().cloned());
    Invocation {
        args,
        env,
        stdin_first: Some(prompt.to_string()),
        keep_stdin: false,
    }
}

/// Splits the extra arguments from Settings into those that go right after the program and those
/// that go before the prompt. CCS reads its profile from its first argument, so for CCS they lead.
pub fn split_extra(kind: CliKind, extra: &[String]) -> (&[String], &[String]) {
    match kind {
        CliKind::Ccs => (extra, &[]),
        _ => (&[], extra),
    }
}

pub fn invocation(kind: CliKind, cwd: &Path, prompt: &str, opts: &TurnOptions) -> Option<Invocation> {
    match kind {
        CliKind::Claude | CliKind::Ccs => Some(claude_invocation(prompt, opts)),
        CliKind::Codex => Some(codex_invocation(cwd, prompt, opts)),
        CliKind::Opencode => Some(opencode_invocation(cwd, prompt, opts)),
        CliKind::Gemini => None,
        CliKind::Pi => Some(pi_invocation(prompt, opts)),
    }
}

/// Arguments and environment for an interactive (PTY) start. The first message goes in as the
/// CLI's own prompt argument, so nothing has to be typed into a TUI that may not be ready yet.
pub fn interactive_args(
    kind: CliKind,
    prompt: &str,
    resume: Option<&str>,
    mode: PermMode,
    extra: &[String],
) -> (Vec<String>, Vec<(String, String)>) {
    let (flags, env) = mode_flags(kind, mode, true, false);
    let mut args: Vec<String> = Vec::new();
    match kind {
        CliKind::Claude | CliKind::Ccs => {
            args.extend(flags);
            if let Some(id) = resume {
                args.extend(strings(&["--resume", id]));
            }
        }
        // `codex resume [OPTIONS] [SESSION_ID]`: options go between the subcommand and the id.
        CliKind::Codex => match resume {
            Some(id) => {
                args.push("resume".into());
                args.extend(flags);
                args.push(id.to_string());
            }
            None => args.extend(flags),
        },
        CliKind::Opencode => {
            if let Some(id) = resume {
                args.extend(strings(&["--session", id]));
            }
            args.extend(flags);
        }
        CliKind::Gemini => args.extend(flags),
        CliKind::Pi => {
            if let Some(id) = resume {
                args.extend(strings(&["--session", id]));
            }
            args.extend(flags);
        }
    }
    args.extend(extra.iter().cloned());
    let prompt = prompt.trim();
    if !prompt.is_empty() {
        match kind {
            CliKind::Opencode => args.extend(["--prompt".to_string(), prompt.to_string()]),
            // Unverified: Gemini CLI is not installed on the test machine.
            CliKind::Gemini => args.extend(["-i".to_string(), prompt.to_string()]),
            // Pi reads an argument that starts with `@` as a file to attach, even after `--`.
            CliKind::Pi if prompt.starts_with('@') => args.extend(["--".to_string(), format!(" {prompt}")]),
            CliKind::Pi => args.extend(["--".to_string(), prompt.to_string()]),
            _ => args.push(prompt.to_string()),
        }
    }
    (args, env)
}

pub struct HeadlessRun {
    child: Child,
    stdin: Option<ChildStdin>,
    readers: Vec<thread::JoinHandle<()>>,
}

impl HeadlessRun {
    /// `sink` is called from reader threads, one call per output line.
    pub fn spawn(
        program: &PathBuf,
        cwd: &Path,
        inv: Invocation,
        sink: LineSink,
    ) -> Result<Self, String> {
        let mut cmd = proc::command(program);
        cmd.args(&inv.args)
            .envs(inv.env.iter().map(|(k, v)| (k.as_str(), v.as_str())))
            .current_dir(cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = cmd.spawn().map_err(|e| format!("spawn {}: {e}", program.display()))?;

        let mut stdin = child.stdin.take();
        if let (Some(first), Some(pipe)) = (&inv.stdin_first, stdin.as_mut()) {
            writeln!(pipe, "{first}").map_err(|e| e.to_string())?;
            pipe.flush().map_err(|e| e.to_string())?;
        }
        if !inv.keep_stdin {
            stdin = None;
        }

        let sink = std::sync::Arc::new(std::sync::Mutex::new(sink));
        let mut readers = Vec::new();
        let out = child.stdout.take().expect("piped stdout");
        let err = child.stderr.take().expect("piped stderr");
        for (stream, pipe) in [
            (Stream::Stdout, Box::new(out) as Box<dyn std::io::Read + Send>),
            (Stream::Stderr, Box::new(err)),
        ] {
            let sink = std::sync::Arc::clone(&sink);
            readers.push(thread::spawn(move || {
                for line in BufReader::new(pipe).lines() {
                    let Ok(line) = line else { break };
                    if let Ok(mut s) = sink.lock() {
                        s(stream, &line);
                    }
                }
            }));
        }

        Ok(Self {
            child,
            stdin,
            readers,
        })
    }

    pub fn pid(&self) -> u32 {
        self.child.id()
    }

    /// Sends one JSON line; only valid while stdin is kept open.
    pub fn send_line(&mut self, line: &str) -> Result<(), String> {
        let pipe = self.stdin.as_mut().ok_or("stdin is closed for this run")?;
        writeln!(pipe, "{line}")
            .and_then(|_| pipe.flush())
            .map_err(|e| e.to_string())
    }

    /// Ends the conversation for CLIs that wait on stdin (Claude stream-json).
    pub fn close_stdin(&mut self) {
        self.stdin = None;
    }

    pub fn try_exit_code(&mut self) -> Option<i32> {
        self.child.try_wait().ok().flatten().map(|s| s.code().unwrap_or(-1))
    }

    /// Ends the CLI and everything it started, then reaps it.
    pub fn kill(&mut self) {
        crate::proc::kill_tree(self.child.id());
        let _ = self.child.kill();
        let _ = self.child.wait();
    }

    /// Waits for the process and for both reader threads to drain.
    pub fn wait(mut self) -> Option<i32> {
        self.stdin = None;
        let code = self.child.wait().ok().and_then(|s| s.code());
        for r in self.readers.drain(..) {
            let _ = r.join();
        }
        code
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codex_resume_takes_only_the_bypass_flag() {
        let opts = TurnOptions { resume: Some("t-1"), ..Default::default() };
        let inv = codex_invocation(Path::new("C:/w"), "next", &opts);
        assert_eq!(inv.args, ["exec", "resume", "--json", "--skip-git-repo-check", "t-1", "-"]);
        let bypass = TurnOptions { resume: Some("t-1"), mode: PermMode::Bypass, ..Default::default() };
        let inv = codex_invocation(Path::new("C:/w"), "next", &bypass);
        assert!(inv.args.contains(&"--dangerously-bypass-approvals-and-sandbox".to_string()));
        let fresh = codex_invocation(Path::new("C:/w"), "go", &TurnOptions::default());
        assert_eq!(fresh.args, ["exec", "--json", "--skip-git-repo-check", "-C", "C:/w", "-s", "workspace-write", "-"]);
    }

    #[test]
    fn each_mode_maps_to_the_flags_each_cli_documents() {
        let flags = |k, m, i| mode_flags(k, m, i, false).0.join(" ");
        assert_eq!(flags(CliKind::Claude, PermMode::Ask, false), "--permission-mode manual");
        assert_eq!(flags(CliKind::Claude, PermMode::Plan, true), "--permission-mode plan");
        assert_eq!(flags(CliKind::Claude, PermMode::Auto, true), "--permission-mode auto");
        assert_eq!(flags(CliKind::Claude, PermMode::Bypass, false), "--dangerously-skip-permissions");
        assert_eq!(flags(CliKind::Codex, PermMode::Ask, true), "-s workspace-write -a on-request");
        assert_eq!(flags(CliKind::Codex, PermMode::Plan, false), "-s read-only");
        assert_eq!(flags(CliKind::Codex, PermMode::Auto, false), "--approve-for-me");
        assert_eq!(flags(CliKind::Codex, PermMode::Bypass, true), "--dangerously-bypass-approvals-and-sandbox");
        assert_eq!(flags(CliKind::Opencode, PermMode::Ask, true), "");
        assert_eq!(flags(CliKind::Opencode, PermMode::Plan, true), "--agent plan");
        assert_eq!(flags(CliKind::Opencode, PermMode::Auto, false), "--auto");
        let (_, env) = mode_flags(CliKind::Opencode, PermMode::Bypass, true, false);
        assert_eq!(env, [("OPENCODE_PERMISSION".to_string(), r#"{"*":"allow"}"#.to_string())]);
        assert_eq!(PermMode::parse("bypass"), PermMode::Bypass);
        assert_eq!(PermMode::parse("anything else"), PermMode::Ask);
    }

    #[test]
    fn extra_flags_and_modes_go_before_the_prompt() {
        let extra = vec!["--pure".to_string()];
        let opts = TurnOptions { extra: &extra, mode: PermMode::Plan, ..Default::default() };
        let inv = opencode_invocation(Path::new("C:/w"), "hi", &opts);
        assert_eq!(inv.args.last().unwrap(), "hi");
        assert!(inv.args.contains(&"--pure".to_string()) && inv.args.contains(&"plan".to_string()));
        let (args, _) = interactive_args(CliKind::Opencode, "hi", None, PermMode::Ask, &extra);
        assert_eq!(args, ["--pure", "--prompt", "hi"]);
        let (args, _) = interactive_args(CliKind::Claude, "", Some("s1"), PermMode::Auto, &[]);
        assert_eq!(args, ["--permission-mode", "auto", "--resume", "s1"]);
        let (args, _) = interactive_args(CliKind::Codex, "", Some("t9"), PermMode::Plan, &[]);
        assert_eq!(args, ["resume", "-s", "read-only", "-a", "on-request", "t9"]);
    }

    #[test]
    fn ccs_gets_its_profile_first_then_claude_code_arguments() {
        let extra = strings(&["work", "--effort", "high"]);
        let none: &[String] = &[];
        assert_eq!(split_extra(CliKind::Ccs, &extra), (&extra[..], none));
        assert_eq!(split_extra(CliKind::Claude, &extra), (none, &extra[..]));
        let ccs = invocation(CliKind::Ccs, Path::new("C:/w"), "hi", &TurnOptions::default()).unwrap();
        assert_eq!(ccs.args, claude_invocation("hi", &TurnOptions::default()).args);
        assert!(ccs.keep_stdin);
        let (args, _) = interactive_args(CliKind::Ccs, "hi", Some("s1"), PermMode::Ask, none);
        assert_eq!(args, ["--permission-mode", "manual", "--resume", "s1", "hi"]);
    }

    #[test]
    fn pi_takes_the_prompt_on_stdin_and_plan_as_read_only_tools() {
        let extra = strings(&["--model", "anthropic/claude-sonnet-4-5"]);
        let opts = TurnOptions { resume: Some("01a0e43a"), mode: PermMode::Plan, extra: &extra };
        let inv = invocation(CliKind::Pi, Path::new("C:/w"), "@README.md tidy it", &opts).unwrap();
        assert_eq!(inv.args, ["--mode", "json", "--session", "01a0e43a", "--tools", "read,grep,find,ls", "--model", "anthropic/claude-sonnet-4-5"]);
        assert_eq!(inv.stdin_first.as_deref(), Some("@README.md tidy it"));
        assert!(!inv.keep_stdin);
        for mode in [PermMode::Ask, PermMode::Auto, PermMode::Bypass] {
            assert!(mode_flags(CliKind::Pi, mode, true, false).0.is_empty());
        }
        let (args, _) = interactive_args(CliKind::Pi, "-v means verbose?", Some("s1"), PermMode::Ask, &[]);
        assert_eq!(args, ["--session", "s1", "--", "-v means verbose?"]);
        let (args, _) = interactive_args(CliKind::Pi, "@src/app.ts fix it", None, PermMode::Ask, &[]);
        assert_eq!(args, ["--", " @src/app.ts fix it"]);
    }
}
