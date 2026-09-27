use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};

pub struct PtySpec {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    pub cols: u16,
    pub rows: u16,
    pub env: Vec<(String, String)>,
    /// Starts the program from this PowerShell, as if typed at its prompt, so the user's
    /// profile (PATH, environment) applies to it.
    pub powershell: Option<PathBuf>,
}

/// Runs the program with the environment the profile left and exits with its code. The program and
/// its arguments arrive as environment variables: PowerShell would parse `$` or quotes in a prompt.
const POWERSHELL_HOST: &str = "$ErrorActionPreference = 'Stop'; \
    $start = New-Object System.Diagnostics.ProcessStartInfo; \
    $start.FileName = $env:OPENCOMPANION_PROGRAM; \
    $start.Arguments = $env:OPENCOMPANION_ARGS; \
    $start.WorkingDirectory = $env:OPENCOMPANION_CWD; \
    $start.UseShellExecute = $false; \
    foreach ($name in 'OPENCOMPANION_PROGRAM', 'OPENCOMPANION_ARGS', 'OPENCOMPANION_CWD') { [Environment]::SetEnvironmentVariable($name, $null) }; \
    $cli = [System.Diagnostics.Process]::Start($start); \
    $cli.WaitForExit(); \
    exit $cli.ExitCode";

pub type OutputSink = Box<dyn FnMut(&[u8]) + Send>;

type SharedWriter = Arc<Mutex<Box<dyn Write + Send>>>;

/// An interactive CLI running in a pseudo terminal (ConPTY on Windows).
pub struct PtySession {
    master: Box<dyn MasterPty + Send>,
    writer: SharedWriter,
    child: Box<dyn Child + Send + Sync>,
    screen: Arc<Mutex<vt100::Parser>>,
    eof: Arc<AtomicBool>,
}

const CURSOR_QUERY: &[u8] = b"\x1b[6n";

/// Counts cursor position queries (`ESC[6n`), including one split across two reads.
fn count_cursor_queries(carry: &mut Vec<u8>, chunk: &[u8]) -> usize {
    carry.extend_from_slice(chunk);
    let count = carry
        .windows(CURSOR_QUERY.len())
        .filter(|w| *w == CURSOR_QUERY)
        .count();
    let keep = carry.len().min(CURSOR_QUERY.len() - 1);
    carry.drain(..carry.len() - keep);
    count
}

impl PtySession {
    pub fn spawn(spec: PtySpec, mut sink: OutputSink) -> Result<Self, String> {
        let size = PtySize {
            rows: spec.rows,
            cols: spec.cols,
            pixel_width: 0,
            pixel_height: 0,
        };
        let pair = native_pty_system()
            .openpty(size)
            .map_err(|e| format!("openpty: {e}"))?;

        // An npm shim runs as `node <script>`. Any other batch file needs cmd.exe as its host,
        // which reads `&`, `%` and quotes in the arguments as its own syntax.
        let (program, lead) = crate::proc::launcher(&spec.program);
        let batch = crate::proc::is_batch(&program);
        if batch && spec.args.iter().any(|a| crate::proc::cmd_unsafe(a)) {
            return Err(format!(
                "{} is a batch file, so cmd.exe would run parts of this text as commands or cut it at a line break.                      Set the CLI's .exe or .js file as its path on the CLIs screen, or leave out & | < > ^ % ! and quotes.",
                program.display()
            ));
        }
        let mut cmd = if let Some(powershell) = &spec.powershell {
            let mut c = CommandBuilder::new(powershell);
            c.args(["-NoLogo", "-Command", POWERSHELL_HOST]);
            let args: Vec<String> = lead.into_iter().chain(spec.args.iter().cloned()).collect();
            c.env("OPENCOMPANION_PROGRAM", &program);
            c.env("OPENCOMPANION_ARGS", crate::proc::command_line(&args));
            c.env("OPENCOMPANION_CWD", &spec.cwd);
            c
        } else {
            let mut c = if batch {
                let mut c = CommandBuilder::new("cmd.exe");
                c.args(["/d", "/c"]);
                c.arg(&program);
                c
            } else {
                CommandBuilder::new(&program)
            };
            c.args(&lead);
            c.args(&spec.args);
            c
        };
        cmd.cwd(&spec.cwd);
        for var in crate::proc::INHERITED_SESSION_VARS {
            cmd.env_remove(var);
        }
        for (k, v) in &spec.env {
            cmd.env(k, v);
        }

        let child = pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| format!("spawn: {e}"))?;
        // The slave handle must be dropped, or reads never see EOF after the child exits.
        drop(pair.slave);

        let mut reader = pair
            .master
            .try_clone_reader()
            .map_err(|e| format!("reader: {e}"))?;
        let writer: SharedWriter = Arc::new(Mutex::new(
            pair.master
                .take_writer()
                .map_err(|e| format!("writer: {e}"))?,
        ));

        let screen = Arc::new(Mutex::new(vt100::Parser::new(spec.rows, spec.cols, 0)));
        let eof = Arc::new(AtomicBool::new(false));
        {
            let screen = Arc::clone(&screen);
            let writer = Arc::clone(&writer);
            let eof = Arc::clone(&eof);
            thread::spawn(move || {
                let mut buf = [0u8; 8192];
                let mut carry = Vec::new();
                loop {
                    match reader.read(&mut buf) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => {
                            let chunk = &buf[..n];
                            let mut cursor = (0, 0);
                            if let Ok(mut s) = screen.lock() {
                                s.process(chunk);
                                cursor = s.screen().cursor_position();
                            }
                            // ConPTY asks for the cursor position at start and blocks all
                            // output until it gets an answer. Answer here, so a session with
                            // no terminal view attached still runs; the webview terminal must
                            // not forward its own answer.
                            for _ in 0..count_cursor_queries(&mut carry, chunk) {
                                let reply = format!("\x1b[{};{}R", cursor.0 + 1, cursor.1 + 1);
                                if let Ok(mut w) = writer.lock() {
                                    let _ = w.write_all(reply.as_bytes());
                                    let _ = w.flush();
                                }
                            }
                            sink(chunk);
                        }
                    }
                }
                eof.store(true, Ordering::SeqCst);
            });
        }

        Ok(Self {
            master: pair.master,
            writer,
            child,
            screen,
            eof,
        })
    }

    pub fn pid(&self) -> Option<u32> {
        self.child.process_id()
    }

    pub fn write(&mut self, bytes: &[u8]) -> Result<(), String> {
        let mut w = self.writer.lock().map_err(|e| e.to_string())?;
        w.write_all(bytes)
            .and_then(|_| w.flush())
            .map_err(|e| e.to_string())
    }

    pub fn resize(&mut self, cols: u16, rows: u16) -> Result<(), String> {
        self.master
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| e.to_string())?;
        if let Ok(mut s) = self.screen.lock() {
            s.screen_mut().set_size(rows, cols);
        }
        Ok(())
    }

    /// Visible screen as plain text, one line per terminal row, the input for per-CLI prompt
    /// detection. `contents()` is not used: it joins soft-wrapped rows, and full-width TUIs
    /// such as OpenCode then read as one long line.
    pub fn screen_text(&self) -> String {
        self.screen
            .lock()
            .map(|s| {
                let screen = s.screen();
                let (_, cols) = screen.size();
                screen
                    .rows(0, cols)
                    .map(|r| r.trim_end().to_string())
                    .collect::<Vec<_>>()
                    .join("\n")
                    .trim_end()
                    .to_string()
            })
            .unwrap_or_default()
    }

    pub fn exit_code(&mut self) -> Option<u32> {
        match self.child.try_wait() {
            Ok(Some(status)) => Some(status.exit_code()),
            _ => None,
        }
    }

    pub fn output_closed(&self) -> bool {
        self.eof.load(Ordering::SeqCst)
    }

    /// Polite stop first (Ctrl+C, twice for TUIs that ask to confirm), then a hard kill.
    pub fn stop(&mut self, grace: Duration) -> Option<u32> {
        let _ = self.write(b"\x03");
        thread::sleep(Duration::from_millis(150));
        let _ = self.write(b"\x03");
        let deadline = Instant::now() + grace;
        while Instant::now() < deadline {
            if let Some(code) = self.exit_code() {
                return Some(code);
            }
            thread::sleep(Duration::from_millis(50));
        }
        self.kill();
        None
    }

    /// Ends the process and everything it started, without asking first.
    pub fn kill(&mut self) {
        // The whole tree: the CLI's own children would outlive it otherwise.
        if let Some(pid) = self.child.process_id() {
            crate::proc::kill_tree(pid);
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_cursor_queries_across_reads() {
        let mut carry = Vec::new();
        assert_eq!(count_cursor_queries(&mut carry, b"\x1b[6n"), 1);
        assert_eq!(count_cursor_queries(&mut carry, b"text \x1b["), 0);
        assert_eq!(count_cursor_queries(&mut carry, b"6n and \x1b[6n"), 2);
        assert_eq!(count_cursor_queries(&mut carry, b"\x1b[6m"), 0);
    }

    #[cfg(windows)]
    #[test]
    fn a_program_started_from_powershell_keeps_its_arguments_folder_environment_and_exit_code() {
        let root = std::env::var_os("SystemRoot").map(PathBuf::from).expect("SystemRoot");
        let dir = std::env::temp_dir().join(format!("oc host {}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let text = Arc::new(Mutex::new(String::new()));
        let sink = Arc::clone(&text);
        // PowerShell would expand $HOME and drop the quotes if the arguments went through its parser.
        let args = ["/d", "/c", "echo", "$HOME", "'x'", "%OC_TEST%", "&", "cd", "&", "set", "OPENCOMPANION", "&", "exit", "3"];
        let mut pty = PtySession::spawn(
            PtySpec {
                program: root.join(r"System32\cmd.exe"),
                args: args.iter().map(|a| a.to_string()).collect(),
                cwd: dir.clone(),
                cols: 160,
                rows: 30,
                env: vec![("OC_TEST".into(), "reached".into())],
                powershell: Some(crate::terminal::powershell().expect("PowerShell is installed")),
            },
            Box::new(move |bytes| sink.lock().unwrap().push_str(&String::from_utf8_lossy(bytes))),
        )
        .expect("PowerShell starts");
        let pid = sysinfo::Pid::from_u32(pty.pid().expect("pid"));
        let mut sys = sysinfo::System::new();
        sys.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[pid]), true);
        let host = sys.process(pid).map(|p| p.name().to_string_lossy().to_lowercase()).unwrap_or_default();
        assert!(host == "powershell.exe" || host == "pwsh.exe", "started {host}");

        let deadline = Instant::now() + Duration::from_secs(60);
        let code = loop {
            if let Some(code) = pty.exit_code() {
                break code;
            }
            assert!(Instant::now() < deadline, "never exited: {}", pty.screen_text());
            thread::sleep(Duration::from_millis(50));
        };
        while !pty.screen_text().contains(&dir.display().to_string()) && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(50));
        }
        let screen = pty.screen_text();
        assert_eq!(code, 3, "{screen}");
        assert!(screen.contains("$HOME 'x' reached"), "{screen}");
        assert!(screen.contains(&dir.display().to_string()), "{screen}");
        assert!(!text.lock().unwrap().contains("OPENCOMPANION_"), "{screen}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
