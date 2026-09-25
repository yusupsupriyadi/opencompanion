use std::ffi::OsStr;
use std::io::Read;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

/// Markers a running Claude Code session sets for its own children. Inherited by a CLI that
/// AI Remote starts (for example when AI Remote itself was launched from a Claude Code
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
pub fn command(program: impl AsRef<OsStr>) -> Command {
    let mut cmd = Command::new(program);
    for var in INHERITED_SESSION_VARS {
        cmd.env_remove(var);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
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
