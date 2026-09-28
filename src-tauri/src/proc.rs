use std::ffi::OsStr;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

/// Markers a running Claude Code session sets for its own children. Inherited by a CLI that
/// OpenCompanion starts (for example when OpenCompanion itself was launched from a Claude Code
/// terminal), they turn off transcript saving and leak the parent's messaging token.
/// User configuration such as `CLAUDE_CODE_USE_BEDROCK` is deliberately not listed.
pub const INHERITED_SESSION_VARS: &[&str] = &[
    "CLAUDECODE",
    "CLAUDE_CODE_CHILD_SESSION",
    "CLAUDE_CODE_ENTRYPOINT",
    "CLAUDE_CODE_EXECPATH",
    "CLAUDE_CODE_MESSAGING_SOCKET",
    "CLAUDE_CODE_MESSAGING_TOKEN",
    "CLAUDE_CODE_SESSION_ATTENDED",
    "CLAUDE_CODE_SESSION_ID",
    "CLAUDE_EFFORT",
    "CLAUDE_PID",
];

/// A `Command` that does not flash a console window when the app runs as a GUI on Windows.
/// An npm shim that runs a node script starts as `node <script>` instead (see `launcher`).
pub fn command(program: impl AsRef<Path>) -> Command {
    let (program, lead) = launcher(program.as_ref());
    let mut cmd = hidden(&program);
    cmd.args(lead);
    cmd.envs(crate::shell_env::vars().iter().map(|(k, v)| (k, v)));
    for var in INHERITED_SESSION_VARS {
        cmd.env_remove(var);
    }
    cmd
}

pub(crate) fn hidden(program: impl AsRef<OsStr>) -> Command {
    let mut cmd = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    // Its own session, and so its own process group with its pid as the id: `kill_tree` ends the
    // CLI together with what it started. With no controlling terminal, a shell or a prompt that
    // opens /dev/tty gets an error instead of being stopped when the app itself runs from a terminal.
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        // SAFETY: setsid(2) is async-signal-safe and touches no memory shared with the parent.
        unsafe {
            cmd.pre_exec(|| if libc::setsid() == -1 { Err(std::io::Error::last_os_error()) } else { Ok(()) });
        }
    }
    cmd
}

/// `.cmd` and `.bat` files, which Windows can only run through cmd.exe.
pub fn is_batch(program: &Path) -> bool {
    cfg!(windows)
        && program
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("cmd") || e.eq_ignore_ascii_case("bat"))
}

/// Text cmd.exe would act on even inside quotes (`&` starts a second command, `%` expands a
/// variable), or cut at a line break.
pub fn cmd_unsafe(arg: &str) -> bool {
    arg.contains(['&', '|', '<', '>', '^', '%', '!', '"', '\r', '\n'])
}

/// One Windows command line that the program's C runtime splits back into exactly `args`.
pub fn command_line(args: &[String]) -> String {
    let mut line = String::new();
    for arg in args {
        if !line.is_empty() {
            line.push(' ');
        }
        if !arg.is_empty() && !arg.contains([' ', '\t', '\n', '\x0b', '"']) {
            line.push_str(arg);
            continue;
        }
        // Backslashes are literal unless they precede a quote, so only those runs are doubled.
        line.push('"');
        let mut backslashes = 0;
        for c in arg.chars() {
            match c {
                '\\' => backslashes += 1,
                '"' => {
                    line.extend(std::iter::repeat_n('\\', backslashes + 1));
                    backslashes = 0;
                }
                _ => backslashes = 0,
            }
            line.push(c);
        }
        line.extend(std::iter::repeat_n('\\', backslashes));
        line.push('"');
    }
    line
}

/// How to start `program` without cmd.exe in between. npm installs Codex, Claude Code and
/// Gemini CLI as a `.cmd` shim that runs a node script; that becomes `node <script>`, so a prompt
/// never passes through cmd.exe, where `&` in it would run a command. Anything else starts as it is.
pub fn launcher(program: &Path) -> (PathBuf, Vec<String>) {
    let unchanged = (program.to_path_buf(), Vec::new());
    if !is_batch(program) {
        return unchanged;
    }
    let (Ok(text), Some(dir)) = (std::fs::read_to_string(program), program.parent()) else { return unchanged };
    let Some(script) = shim_script(&text).map(|rel| dir.join(rel)).filter(|p| p.is_file()) else { return unchanged };
    // The shim prefers a node.exe next to itself, like npm's own shim does.
    let node = Some(dir.join("node.exe")).filter(|p| p.is_file()).or_else(|| which::which("node").ok());
    match node {
        Some(node) => (node, vec![script.display().to_string()]),
        None => unchanged,
    }
}

/// The node script an npm cmd-shim runs: `"%dp0%\node_modules\@openai\codex\bin\codex.js" %*`.
/// Pi's own installer writes `node "%~dp0pi-launcher.js" %*`, with no backslash after `%~dp0`.
fn shim_script(text: &str) -> Option<PathBuf> {
    for marker in ["\"%dp0%\\", "\"%~dp0\\", "\"%~dp0"] {
        let mut rest = text;
        while let Some(at) = rest.find(marker) {
            rest = &rest[at + marker.len()..];
            let Some(end) = rest.find('"') else { break };
            let rel = rest[..end].trim_start_matches('\\');
            let lower = rel.to_ascii_lowercase();
            if [".js", ".cjs", ".mjs"].iter().any(|x| lower.ends_with(x)) {
                return Some(PathBuf::from(rel.replace('\\', std::path::MAIN_SEPARATOR_STR)));
            }
        }
    }
    None
}

/// Ends `pid` and every process it started. Killing only the CLI would leave its shells, test
/// runners and dev servers running, and for an npm install the CLI itself under node.
pub fn kill_tree(pid: u32) {
    #[cfg(windows)]
    {
        let taskkill = std::env::var_os("SystemRoot")
            .map(|root| PathBuf::from(root).join("System32").join("taskkill.exe"))
            .unwrap_or_else(|| PathBuf::from("taskkill.exe"));
        let _ = hidden(taskkill)
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    // Every CLI starts in its own process group (`hidden`, and a PTY child leads its own
    // session), so its pid is the group id: SIGTERM first, SIGKILL for what is left after 2 s.
    #[cfg(unix)]
    {
        let Some(group) = target_group(pid) else { return };
        // SAFETY: kill(2) only sends a signal; a negative pid names a process group.
        unsafe { libc::kill(group, libc::SIGTERM) };
        thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(2);
            while Instant::now() < deadline {
                // Signal 0 only asks whether the group still has a member.
                if unsafe { libc::kill(group, 0) } != 0 {
                    return;
                }
                thread::sleep(Duration::from_millis(50));
            }
            unsafe { libc::kill(group, libc::SIGKILL) };
        });
    }
}

/// The process group `kill_tree` signals for `pid`, as kill(2) names it. Never 0 (our own group)
/// or -1 (every process we may signal): neither is a CLI we started.
#[cfg(unix)]
fn target_group(pid: u32) -> Option<i32> {
    let pid = i32::try_from(pid).ok()?;
    (pid > 1).then_some(-pid)
}

pub struct Captured {
    pub status: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub timed_out: bool,
}

fn drain(mut pipe: impl Read + Send + 'static) -> mpsc::Receiver<String> {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = pipe.read_to_end(&mut buf);
        let _ = tx.send(String::from_utf8_lossy(&buf).into_owned());
    });
    rx
}

/// Runs a short-lived command and kills it if it outlives `timeout`.
pub fn run_with_timeout(cmd: Command, timeout: Duration) -> std::io::Result<Captured> {
    run_with_input(cmd, None, timeout)
}

/// Like `run_with_timeout`, writing `input` to stdin first and then closing it.
///
/// Output is collected with a grace period rather than a join: an npm `.cmd` shim can leave a
/// grandchild holding the pipes open after the shim itself is killed.
pub fn run_with_input(mut cmd: Command, input: Option<&str>, timeout: Duration) -> std::io::Result<Captured> {
    cmd.stdin(if input.is_some() { Stdio::piped() } else { Stdio::null() })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = cmd.spawn()?;
    // Readers start before the write, so a child that answers early cannot fill its pipe
    // and block while we are still writing.
    let out = drain(child.stdout.take().expect("piped stdout"));
    let err = drain(child.stderr.take().expect("piped stderr"));
    if let (Some(text), Some(mut stdin)) = (input, child.stdin.take()) {
        use std::io::Write;
        stdin.write_all(text.as_bytes())?;
    }

    let deadline = Instant::now() + timeout;
    let mut timed_out = false;
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status.code();
        }
        if Instant::now() >= deadline {
            timed_out = true;
            kill_tree(child.id());
            let _ = child.kill();
            break child.wait()?.code();
        }
        thread::sleep(Duration::from_millis(25));
    };

    let grace = Duration::from_millis(500);
    Ok(Captured {
        status,
        stdout: out.recv_timeout(grace).unwrap_or_default(),
        stderr: err.recv_timeout(grace).unwrap_or_default(),
        timed_out,
    })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    // The shim npm writes for `@openai/codex`.
    const NODE_SHIM: &str = r#"@ECHO off
GOTO start
:find_dp0
SET dp0=%~dp0
EXIT /b
:start
SETLOCAL
CALL :find_dp0

IF EXIST "%dp0%\node.exe" (
  SET "_prog=%dp0%\node.exe"
) ELSE (
  SET "_prog=node"
)

endLocal & goto #_undefined_# 2>NUL || title %COMSPEC% & "%_prog%"  "%dp0%\node_modules\@openai\codex\bin\codex.js" %*
"#;

    #[test]
    fn npm_shims_name_their_node_script() {
        let script = shim_script(NODE_SHIM).expect("script");
        assert!(script.ends_with("codex.js"));
        assert!(script.starts_with("node_modules"));
        // A shim for a native exe has no script; `cli::unwrap_shim` swaps those for the exe.
        assert_eq!(shim_script(r#""%dp0%\node_modules\opencode-ai\bin\opencode.exe" %*"#), None);
        let pi = shim_script("@ECHO off\r\nnode \"%~dp0pi-launcher.js\" %*\r\n").expect("pi script");
        assert_eq!(pi, PathBuf::from("pi-launcher.js"));
    }

    #[test]
    fn cmd_metacharacters_and_line_breaks_are_unsafe() {
        assert!(cmd_unsafe("Save & echo INJECTED"));
        assert!(cmd_unsafe("first line\nsecond"));
        assert!(cmd_unsafe("%USERNAME%"));
        assert!(!cmd_unsafe("Fix the failing tests in src/app.ts"));
    }

    #[test]
    fn a_command_line_quotes_only_what_would_split_or_vanish() {
        let line = |args: &[&str]| command_line(&args.iter().map(|a| a.to_string()).collect::<Vec<_>>());
        assert_eq!(line(&["--resume", "abc", "Fix the tests", ""]), r#"--resume abc "Fix the tests" """#);
        assert_eq!(line(&[r#"say "hi""#]), r#""say \"hi\"""#);
        assert_eq!(line(&[r#"a\"b"#]), r#""a\\\"b""#);
        assert_eq!(line(&[r"C:\dir\", r"C:\my dir\"]), r#"C:\dir\ "C:\my dir\\""#);
        assert_eq!(line(&["$HOME 'x' `n"]), r#""$HOME 'x' `n""#);
    }

    #[cfg(windows)]
    #[test]
    fn a_node_shim_launches_node_with_its_script() {
        let dir = std::env::temp_dir().join(format!("air-shim-{}", std::process::id()));
        let bin = dir.join("node_modules").join("@openai").join("codex").join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::write(bin.join("codex.js"), "").unwrap();
        std::fs::write(dir.join("node.exe"), "").unwrap();
        let shim = dir.join("codex.cmd");
        std::fs::write(&shim, NODE_SHIM).unwrap();

        let (program, lead) = launcher(&shim);
        assert_eq!(program, dir.join("node.exe"));
        assert_eq!(lead, [bin.join("codex.js").display().to_string()]);
        let plain = dir.join("node.exe");
        assert_eq!(launcher(&plain), (plain.clone(), vec![]));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Whether `pid` has ended. A killed orphan stays a zombie where no init reaps it (a
    /// container whose first process is not an init), and that counts as ended.
    #[cfg(unix)]
    pub(crate) fn gone(pid: u32) -> bool {
        use sysinfo::{ProcessRefreshKind, ProcessStatus, ProcessesToUpdate, System};
        let mut sys = System::new();
        let p = sysinfo::Pid::from_u32(pid);
        sys.refresh_processes_specifics(ProcessesToUpdate::Some(&[p]), true, ProcessRefreshKind::nothing());
        sys.process(p).is_none_or(|p| p.status() == ProcessStatus::Zombie)
    }

    #[cfg(unix)]
    #[test]
    fn kill_tree_ends_the_children_too_on_unix() {
        use std::io::BufRead;
        let mut parent = command("/bin/sh")
            .args(["-c", "sleep 30 & echo $!; wait"])
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let mut line = String::new();
        std::io::BufReader::new(parent.stdout.take().unwrap()).read_line(&mut line).unwrap();
        let grandchild: u32 = line.trim().parse().unwrap();
        assert!(!gone(grandchild));
        // As `HeadlessRun::kill` stops a CLI.
        kill_tree(parent.id());
        let _ = parent.kill();
        let _ = parent.wait();
        let deadline = Instant::now() + Duration::from_secs(4);
        while !gone(grandchild) {
            assert!(Instant::now() < deadline, "the grandchild outlived Stop");
            thread::sleep(Duration::from_millis(50));
        }
    }

    /// 0 is the caller's own group and 1 would signal every process: neither is a CLI we started.
    /// Tested on the pure function, so a broken guard fails here instead of signalling everything.
    #[cfg(unix)]
    #[test]
    fn kill_tree_never_signals_our_own_group_or_everyone() {
        assert_eq!(target_group(0), None);
        assert_eq!(target_group(1), None);
        assert_eq!(target_group(u32::MAX), None);
        assert_eq!(target_group(4242), Some(-4242));
    }

    /// A CLI leads its own session, so it has no controlling terminal: a shell or a prompt that
    /// opens /dev/tty gets an error instead of being stopped when the app runs from a terminal.
    #[cfg(unix)]
    #[test]
    fn a_cli_leads_its_own_session() {
        // getsid(2) rather than `ps`: its session column differs between Linux and macOS.
        let mut child = hidden("/bin/sleep").arg("5").stdin(Stdio::null()).spawn().unwrap();
        let pid = child.id() as i32;
        // SAFETY: getsid(2) only reads the session id of another process.
        let sid = unsafe { libc::getsid(pid) };
        let _ = child.kill();
        let _ = child.wait();
        assert_eq!(sid, pid, "the child is not a session leader");
    }

    #[cfg(windows)]
    #[test]
    fn kill_tree_ends_the_children_too() {
        use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System};
        let mut parent = hidden("cmd.exe").args(["/d", "/c", "ping -n 30 127.0.0.1 >nul"]).spawn().unwrap();
        let pid = sysinfo::Pid::from_u32(parent.id());
        let mut sys = System::new();
        let deadline = Instant::now() + Duration::from_secs(5);
        let child = loop {
            sys.refresh_processes_specifics(ProcessesToUpdate::All, true, ProcessRefreshKind::nothing());
            if let Some(c) = sys.processes().values().find(|p| p.parent() == Some(pid)) {
                break c.pid();
            }
            assert!(Instant::now() < deadline, "ping never started");
            thread::sleep(Duration::from_millis(50));
        };
        kill_tree(parent.id());
        let _ = parent.wait();
        thread::sleep(Duration::from_millis(200));
        sys.refresh_processes_specifics(ProcessesToUpdate::Some(&[child]), true, ProcessRefreshKind::nothing());
        assert!(sys.process(child).is_none(), "the child outlived its parent");
    }
}
