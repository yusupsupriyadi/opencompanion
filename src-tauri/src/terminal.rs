//! Plain shell terminals: PowerShell, Command Prompt or bash in a project folder, for dev
//! servers, builds, git and the rest. Unlike sessions they run no AI CLI of their own, so there is
//! no status detection and no history in SQLite, and they end when the app does. They stay on the
//! desktop: the phone companion never sees them.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use std::time::Duration;

use serde::Serialize;

use crate::db;
use crate::pty::{PtySession, PtySpec};
use crate::session::{decode_utf8, keep_tail, OUTPUT_KEEP};

/// Everything terminals tell the webview.
pub trait TerminalEmit: Send + Sync {
    /// Terminal text. `seq` counts the terminal's chunks from 1, as for sessions.
    fn output(&self, id: &str, data: &str, seq: u64);
    fn changed(&self, info: &TerminalInfo);
}

/// A shell found on this computer.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Shell {
    pub id: String,
    pub label: String,
    pub path: String,
    #[serde(skip)]
    args: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalInfo {
    pub id: String,
    pub cwd: String,
    /// The `Shell::id` it runs.
    pub shell: String,
    pub shell_label: String,
    pub pid: Option<u32>,
    pub running: bool,
    pub exit_code: Option<i32>,
    pub started_at: i64,
}

fn shell(id: &str, label: &str, path: PathBuf, args: &[&str]) -> Shell {
    Shell {
        id: id.into(),
        label: label.into(),
        path: path.display().to_string(),
        args: args.iter().map(|a| a.to_string()).collect(),
    }
}

/// Shells installed here, the default first: PowerShell 7 when it is installed, then Windows
/// PowerShell, Command Prompt and Git Bash.
#[cfg(windows)]
pub fn available_shells() -> Vec<Shell> {
    let mut out = powershells();
    let cmd = system_root().join(r"System32\cmd.exe");
    if cmd.is_file() {
        out.push(shell("cmd", "Command Prompt", cmd, &[]));
    }
    if let Some(bash) = git_bash() {
        out.push(shell("gitbash", "Git Bash", bash, &["--login", "-i"]));
    }
    out
}

#[cfg(windows)]
fn system_root() -> PathBuf {
    std::env::var_os("SystemRoot")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Windows"))
}

#[cfg(windows)]
fn powershells() -> Vec<Shell> {
    let mut out = Vec::new();
    if let Ok(pwsh) = which::which("pwsh") {
        out.push(shell("pwsh", "PowerShell", pwsh, &["-NoLogo"]));
    }
    let powershell = system_root().join(r"System32\WindowsPowerShell\v1.0\powershell.exe");
    if powershell.is_file() {
        out.push(shell("powershell", "Windows PowerShell", powershell, &["-NoLogo"]));
    }
    out
}

/// The PowerShell a session starts its CLI from: PowerShell 7 when it is installed, else Windows
/// PowerShell. Elsewhere a CLI starts on its own.
#[cfg(windows)]
pub fn powershell() -> Option<PathBuf> {
    powershells().into_iter().next().map(|s| PathBuf::from(s.path))
}

#[cfg(not(windows))]
pub fn powershell() -> Option<PathBuf> {
    None
}

/// Git for Windows' bash. Searching the PATH for `bash` would find WSL's launcher instead.
#[cfg(windows)]
fn git_bash() -> Option<PathBuf> {
    // git.exe sits in Git\cmd, Git\bin or Git\mingw64\bin; bash.exe in Git\bin.
    let near_git = which::which("git").ok().and_then(|git| {
        git.ancestors()
            .skip(1)
            .take(3)
            .map(|dir| dir.join("bin").join("bash.exe"))
            .find(|p| p.is_file())
    });
    near_git.or_else(|| {
        [("ProgramFiles", r"Git\bin\bash.exe"), ("LOCALAPPDATA", r"Programs\Git\bin\bash.exe")]
            .iter()
            .filter_map(|(var, rel)| std::env::var_os(var).map(|base| PathBuf::from(base).join(rel)))
            .find(|p| p.is_file())
    })
}

/// The login shell first, then the usual ones that exist.
#[cfg(not(windows))]
pub fn available_shells() -> Vec<Shell> {
    let mut paths: Vec<PathBuf> = std::env::var_os("SHELL").map(PathBuf::from).into_iter().collect();
    paths.extend(["/bin/zsh", "/bin/bash", "/bin/sh"].map(PathBuf::from));
    let mut out: Vec<Shell> = Vec::new();
    for path in paths {
        let Some(name) = path.file_name().map(|n| n.to_string_lossy().into_owned()) else { continue };
        if path.is_file() && !out.iter().any(|s| s.id == name) {
            out.push(shell(&name, &name, path, &["-l"]));
        }
    }
    out
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

struct Term {
    id: String,
    shell: Shell,
    info: Mutex<TerminalInfo>,
    pty: Mutex<Option<PtySession>>,
    output: Mutex<String>,
    /// Chunks added to `output` so far; changed only while `output` is locked.
    output_seq: AtomicU64,
    utf8_carry: Mutex<Vec<u8>>,
    /// Bumped on every spawn, restart and close, always while `pty` is locked. Output and a
    /// watcher from an older generation are dropped.
    generation: AtomicU64,
}

impl Term {
    fn info(&self) -> TerminalInfo {
        lock(&self.info).clone()
    }
}

pub struct Terminals {
    emit: Arc<dyn TerminalEmit>,
    /// In the order they were opened, which is the order of the tabs.
    open: Mutex<Vec<Arc<Term>>>,
}

impl Terminals {
    pub fn new(emit: Arc<dyn TerminalEmit>) -> Arc<Self> {
        Arc::new(Self {
            emit,
            open: Mutex::new(Vec::new()),
        })
    }

    pub fn list(&self) -> Vec<TerminalInfo> {
        lock(&self.open).iter().map(|t| t.info()).collect()
    }

    fn get(&self, id: &str) -> Result<Arc<Term>, String> {
        lock(&self.open)
            .iter()
            .find(|t| t.id == id)
            .cloned()
            .ok_or_else(|| "Terminal not found.".to_string())
    }

    /// Starts `shell` (the default one when `None`) in `cwd`.
    pub fn open(self: &Arc<Self>, cwd: &str, shell: Option<&str>, cols: Option<u16>, rows: Option<u16>) -> Result<TerminalInfo, String> {
        let cwd = cwd.trim();
        if cwd.is_empty() || !Path::new(cwd).is_dir() {
            return Err("This folder does not exist.".into());
        }
        let shells = available_shells();
        let shell = match shell {
            Some(id) => shells
                .into_iter()
                .find(|s| s.id == id)
                .ok_or("That shell is not installed on this computer.")?,
            None => shells.into_iter().next().ok_or("No shell was found on this computer.")?,
        };
        let id = db::new_id();
        let term = Arc::new(Term {
            info: Mutex::new(TerminalInfo {
                id: id.clone(),
                cwd: cwd.to_string(),
                shell: shell.id.clone(),
                shell_label: shell.label.clone(),
                pid: None,
                running: false,
                exit_code: None,
                started_at: db::now_ms(),
            }),
            id,
            shell,
            pty: Mutex::new(None),
            output: Mutex::new(String::new()),
            output_seq: AtomicU64::new(0),
            utf8_carry: Mutex::new(Vec::new()),
            generation: AtomicU64::new(0),
        });
        lock(&self.open).push(Arc::clone(&term));
        if let Err(e) = self.spawn(&term, cols, rows) {
            lock(&self.open).retain(|t| t.id != term.id);
            return Err(e);
        }
        let info = term.info();
        self.emit.changed(&info);
        Ok(info)
    }

    fn spawn(self: &Arc<Self>, term: &Arc<Term>, cols: Option<u16>, rows: Option<u16>) -> Result<(), String> {
        let mut pty = lock(&term.pty);
        let generation = term.generation.fetch_add(1, Ordering::SeqCst) + 1;
        let sink_term = Arc::clone(term);
        let emit = Arc::clone(&self.emit);
        let session = PtySession::spawn(
            PtySpec {
                program: PathBuf::from(&term.shell.path),
                args: term.shell.args.clone(),
                cwd: PathBuf::from(term.info().cwd),
                cols: cols.unwrap_or(120),
                rows: rows.unwrap_or(32),
                // What xterm.js understands, so tools that read these pick colors and keys that work.
                env: vec![
                    ("TERM".into(), "xterm-256color".into()),
                    ("COLORTERM".into(), "truecolor".into()),
                ],
                powershell: None,
            },
            Box::new(move |bytes| {
                if sink_term.generation.load(Ordering::SeqCst) != generation {
                    return;
                }
                let text = decode_utf8(&mut lock(&sink_term.utf8_carry), bytes);
                if text.is_empty() {
                    return;
                }
                let seq = {
                    let mut out = lock(&sink_term.output);
                    out.push_str(&text);
                    keep_tail(&mut out, OUTPUT_KEEP);
                    sink_term.output_seq.fetch_add(1, Ordering::SeqCst) + 1
                };
                emit.output(&sink_term.id, &text, seq);
            }),
        )?;
        let pid = session.pid();
        *pty = Some(session);
        drop(pty);
        {
            let mut info = lock(&term.info);
            info.pid = pid;
            info.running = true;
            info.exit_code = None;
            info.started_at = db::now_ms();
        }
        let me = Arc::clone(self);
        let watched = Arc::clone(term);
        thread::spawn(move || me.watch(watched, generation));
        Ok(())
    }

    /// Waits for the shell to exit, then says so.
    fn watch(&self, term: Arc<Term>, generation: u64) {
        loop {
            thread::sleep(Duration::from_millis(300));
            let mut pty = lock(&term.pty);
            if term.generation.load(Ordering::SeqCst) != generation {
                return;
            }
            let Some(code) = pty.as_mut().and_then(|p| p.exit_code()) else { continue };
            pty.take();
            drop(pty);
            let info = {
                let mut info = lock(&term.info);
                info.running = false;
                info.pid = None;
                info.exit_code = Some(code as i32);
                info.clone()
            };
            self.emit.changed(&info);
            return;
        }
    }

    pub fn write(&self, id: &str, data: &str) -> Result<(), String> {
        let term = self.get(id)?;
        let mut pty = lock(&term.pty);
        match pty.as_mut() {
            Some(p) => p.write(data.as_bytes()),
            None => Err("This terminal has exited.".into()),
        }
    }

    pub fn resize(&self, id: &str, cols: u16, rows: u16) -> Result<(), String> {
        let term = self.get(id)?;
        let mut pty = lock(&term.pty);
        match pty.as_mut() {
            Some(p) => p.resize(cols, rows),
            None => Ok(()),
        }
    }

    /// Terminal text for a view that attaches late, with the number of the last chunk in it.
    pub fn output_snapshot(&self, id: &str) -> (String, u64) {
        match self.get(id) {
            Ok(term) => {
                let out = lock(&term.output);
                (out.clone(), term.output_seq.load(Ordering::SeqCst))
            }
            Err(_) => (String::new(), 0),
        }
    }

    /// Starts the same shell again in the same folder, on a clean screen.
    pub fn restart(self: &Arc<Self>, id: &str, cols: Option<u16>, rows: Option<u16>) -> Result<TerminalInfo, String> {
        let term = self.get(id)?;
        end(&term);
        lock(&term.output).clear();
        lock(&term.utf8_carry).clear();
        self.spawn(&term, cols, rows)?;
        let info = term.info();
        self.emit.changed(&info);
        Ok(info)
    }

    /// Ends the shell and whatever it started, such as a dev server, and forgets the terminal.
    pub fn close(&self, id: &str) -> Result<(), String> {
        let term = {
            let mut open = lock(&self.open);
            let at = open.iter().position(|t| t.id == id).ok_or("Terminal not found.")?;
            open.remove(at)
        };
        end(&term);
        Ok(())
    }

    /// App exit: no terminal outlives the window it was opened from.
    pub fn kill_all(&self) {
        let all = std::mem::take(&mut *lock(&self.open));
        for term in all {
            end(&term);
        }
    }
}

fn end(term: &Term) {
    let mut pty = lock(&term.pty);
    term.generation.fetch_add(1, Ordering::SeqCst);
    if let Some(mut p) = pty.take() {
        p.kill();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    #[derive(Default)]
    struct Recorder {
        output: Mutex<String>,
        changed: Mutex<Vec<TerminalInfo>>,
    }

    impl TerminalEmit for Recorder {
        fn output(&self, _id: &str, data: &str, _seq: u64) {
            self.output.lock().unwrap().push_str(data);
        }
        fn changed(&self, info: &TerminalInfo) {
            self.changed.lock().unwrap().push(info.clone());
        }
    }

    fn wait_for(what: &str, mut ok: impl FnMut() -> bool) {
        let deadline = Instant::now() + Duration::from_secs(20);
        while !ok() {
            assert!(Instant::now() < deadline, "timed out waiting for {what}");
            thread::sleep(Duration::from_millis(50));
        }
    }

    /// Command Prompt on Windows starts fastest; elsewhere the first shell found.
    fn test_shell() -> String {
        let shells = available_shells();
        let pick = shells.iter().find(|s| s.id == "cmd").or(shells.first());
        pick.expect("a shell is installed").id.clone()
    }

    #[test]
    fn shells_have_unique_ids_and_exist() {
        let shells = available_shells();
        assert!(!shells.is_empty());
        for s in &shells {
            assert!(Path::new(&s.path).is_file(), "{} at {}", s.id, s.path);
            assert_eq!(shells.iter().filter(|o| o.id == s.id).count(), 1);
        }
    }

    #[test]
    fn a_missing_folder_or_shell_opens_nothing() {
        let terms = Terminals::new(Arc::new(Recorder::default()));
        let missing = std::env::temp_dir().join("air-terminal-no-such-folder");
        let err = terms.open(&missing.display().to_string(), None, None, None).unwrap_err();
        assert_eq!(err, "This folder does not exist.");
        let here = std::env::temp_dir().display().to_string();
        let err = terms.open(&here, Some("no-such-shell"), None, None).unwrap_err();
        assert_eq!(err, "That shell is not installed on this computer.");
        assert!(terms.list().is_empty());
    }

    #[test]
    fn a_shell_runs_commands_reports_its_exit_and_restarts() {
        let rec = Arc::new(Recorder::default());
        let terms = Terminals::new(rec.clone());
        let dir = std::env::temp_dir().join(format!("air-terminal-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let info = terms
            .open(&dir.display().to_string(), Some(&test_shell()), Some(100), Some(30))
            .expect("the shell starts");
        assert!(info.running);
        assert_eq!(terms.list().len(), 1);

        terms.write(&info.id, "echo oc-term-ok\r").unwrap();
        wait_for("the echo", || rec.output.lock().unwrap().contains("oc-term-ok"));
        let (snap, seq) = terms.output_snapshot(&info.id);
        assert!(snap.contains("oc-term-ok"));
        assert!(seq > 0);

        terms.write(&info.id, "exit\r").unwrap();
        wait_for("the exit", || terms.list()[0].exit_code.is_some());
        let ended = &terms.list()[0];
        assert!(!ended.running);
        assert_eq!(ended.exit_code, Some(0));
        assert!(rec.changed.lock().unwrap().iter().any(|i| !i.running));
        assert_eq!(terms.write(&info.id, "x").unwrap_err(), "This terminal has exited.");

        let again = terms.restart(&info.id, None, None).expect("the shell starts again");
        assert!(again.running);
        assert!(!terms.output_snapshot(&info.id).0.contains("oc-term-ok"));

        terms.close(&info.id).unwrap();
        assert!(terms.list().is_empty());
        assert_eq!(terms.close(&info.id).unwrap_err(), "Terminal not found.");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
