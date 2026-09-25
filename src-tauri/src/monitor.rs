use std::collections::HashSet;
use std::path::PathBuf;

use serde::Serialize;
use sysinfo::{Pid, Process, ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

use crate::cli::CliKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum RunMode {
    Interactive,
    Headless,
    Server,
}

/// A CLI process that AI Remote did not start. Read-only by design (PRD FR-31).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalSession {
    pub pid: u32,
    pub kind: CliKind,
    pub mode: RunMode,
    /// `None` when Windows refuses to read the working folder ("Folder unknown" in the UI).
    pub cwd: Option<PathBuf>,
    pub started_at: u64,
    pub cpu_percent: f32,
    pub memory_bytes: u64,
}

/// Maps a process to a CLI. Helpers such as `codex-windows-sandbox-service.exe` are not sessions.
pub fn classify(name: &str, args: &[String]) -> Option<CliKind> {
    let stem = name.strip_suffix(".exe").unwrap_or(name).to_ascii_lowercase();
    if let Some(kind) = CliKind::from_bin(&stem) {
        return Some(kind);
    }
    // npm installs that run through node instead of a native binary.
    if stem == "node" || stem == "bun" {
        let joined = args.join(" ").replace('\\', "/").to_ascii_lowercase();
        if joined.contains("@openai/codex") {
            return Some(CliKind::Codex);
        }
        if joined.contains("@anthropic-ai/claude-code") {
            return Some(CliKind::Claude);
        }
        if joined.contains("@google/gemini-cli") {
            return Some(CliKind::Gemini);
        }
    }
    None
}

/// Reads only the flags that decide the mode. The full command line is never stored: it can hold
/// the prompt text or tokens.
pub fn run_mode(kind: CliKind, args: &[String]) -> RunMode {
    let has = |flag: &str| args.iter().skip(1).any(|a| a == flag);
    let first = args
        .iter()
        .skip(1)
        .find(|a| !a.starts_with('-'))
        .map(String::as_str);
    match kind {
        CliKind::Claude | CliKind::Gemini if has("-p") || has("--print") => RunMode::Headless,
        CliKind::Codex if matches!(first, Some("exec") | Some("e")) => RunMode::Headless,
        CliKind::Codex if matches!(first, Some("app-server") | Some("mcp-server")) => {
            RunMode::Server
        }
        CliKind::Opencode if first == Some("run") => RunMode::Headless,
        CliKind::Opencode if matches!(first, Some("serve") | Some("web")) => RunMode::Server,
        _ => RunMode::Interactive,
    }
}

fn arg_strings(p: &Process) -> Vec<String> {
    p.cmd()
        .iter()
        .map(|a| a.to_string_lossy().into_owned())
        .collect()
}

pub struct Monitor {
    sys: System,
}

impl Default for Monitor {
    fn default() -> Self {
        Self::new()
    }
}

impl Monitor {
    pub fn new() -> Self {
        Self { sys: System::new() }
    }

    /// Lists CLI processes that are not descendants of `own_pids` (sessions AI Remote started).
    /// CPU usage needs two scans; the first scan reports 0.
    pub fn scan(&mut self, own_pids: &HashSet<u32>) -> Vec<ExternalSession> {
        self.sys.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing()
                .with_cpu()
                .with_memory()
                .with_cmd(UpdateKind::OnlyIfNotSet)
                .with_cwd(UpdateKind::OnlyIfNotSet),
        );
        let procs = self.sys.processes();
        let kind_of = |p: &Process| classify(&p.name().to_string_lossy(), &arg_strings(p));

        let mut found = Vec::new();
        for (pid, p) in procs {
            let Some(kind) = kind_of(p) else { continue };

            // Walk up once: skip children of the same CLI (OpenCode's wrapper exe starts the
            // platform exe) and anything AI Remote spawned.
            let mut same_cli_parent = false;
            let mut ours = own_pids.contains(&pid.as_u32());
            let mut cursor = p.parent();
            let mut depth = 0;
            while let Some(ppid) = cursor {
                if own_pids.contains(&ppid.as_u32()) {
                    ours = true;
                    break;
                }
                let Some(parent) = procs.get(&ppid) else { break };
                if depth == 0 && kind_of(parent) == Some(kind) {
                    same_cli_parent = true;
                }
                cursor = parent.parent();
                depth += 1;
                if depth > 32 {
                    break;
                }
            }
            if ours || same_cli_parent {
                continue;
            }

            let args = arg_strings(p);
            found.push(ExternalSession {
                pid: pid.as_u32(),
                kind,
                mode: run_mode(kind, &args),
                cwd: p.cwd().map(|c| c.to_path_buf()),
                started_at: p.start_time(),
                cpu_percent: p.cpu_usage(),
                memory_bytes: p.memory() + child_memory(procs, *pid),
            });
        }
        found.sort_by_key(|s| s.started_at);
        found
    }
}

/// Memory of direct children (shells and tools the CLI started), so the number means something.
fn child_memory(procs: &std::collections::HashMap<Pid, Process>, pid: Pid) -> u64 {
    procs
        .values()
        .filter(|c| c.parent() == Some(pid))
        .map(Process::memory)
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn classifies_native_binaries_and_skips_helpers() {
        assert_eq!(classify("claude.exe", &[]), Some(CliKind::Claude));
        assert_eq!(classify("codex.exe", &[]), Some(CliKind::Codex));
        assert_eq!(classify("opencode.exe", &[]), Some(CliKind::Opencode));
        assert_eq!(classify("codex-windows-sandbox-service.exe", &[]), None);
        assert_eq!(classify("codex-code-mode-host.exe", &[]), None);
        assert_eq!(classify("node.exe", &args(&["node", "-e", "x"])), None);
    }

    #[test]
    fn classifies_npm_installs_running_under_node() {
        let a = args(&["node", r"C:\npm\node_modules\@openai\codex\bin\codex.js", "exec"]);
        assert_eq!(classify("node.exe", &a), Some(CliKind::Codex));
    }

    /// Regression: the app's own `opencode --version` probe was listed as an outside session.
    /// A copy of this test binary named `opencode.exe` stands in for the probe.
    #[cfg(windows)]
    #[test]
    fn own_descendants_are_not_outside_sessions() {
        use std::process::{Command, Stdio};
        use std::time::Duration;

        let dir = std::env::temp_dir().join(format!("air-monitor-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let fake = dir.join("opencode.exe");
        std::fs::copy(std::env::current_exe().unwrap(), &fake).unwrap();
        let mut child = Command::new(&fake)
            .args(["--exact", "monitor::tests::sleep_helper", "--ignored"])
            .env("AIR_SLEEP_HELPER", "1")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        std::thread::sleep(Duration::from_millis(700));

        let mut monitor = Monitor::new();
        let without_self = monitor.scan(&HashSet::new());
        let with_self = monitor.scan(&HashSet::from([std::process::id()]));
        let _ = child.kill();
        let _ = child.wait();
        let _ = std::fs::remove_dir_all(&dir);

        let pid = child.id();
        assert!(without_self.iter().any(|s| s.pid == pid && s.kind == CliKind::Opencode));
        assert!(!with_self.iter().any(|s| s.pid == pid));
    }

    #[test]
    #[ignore = "helper process for own_descendants_are_not_outside_sessions"]
    fn sleep_helper() {
        if std::env::var_os("AIR_SLEEP_HELPER").is_some() {
            std::thread::sleep(std::time::Duration::from_secs(10));
        }
    }

    #[test]
    fn reads_mode_from_flags() {
        let c = CliKind::Claude;
        assert_eq!(run_mode(c, &args(&["claude", "-c"])), RunMode::Interactive);
        assert_eq!(run_mode(c, &args(&["claude", "-p", "hi"])), RunMode::Headless);
        let x = CliKind::Codex;
        assert_eq!(run_mode(x, &args(&["codex", "exec", "--json"])), RunMode::Headless);
        assert_eq!(run_mode(x, &args(&["codex", "app-server"])), RunMode::Server);
        assert_eq!(run_mode(x, &args(&["codex"])), RunMode::Interactive);
        let o = CliKind::Opencode;
        assert_eq!(run_mode(o, &args(&["opencode", "run", "hi"])), RunMode::Headless);
        assert_eq!(run_mode(o, &args(&["opencode", "serve"])), RunMode::Server);
    }
}
