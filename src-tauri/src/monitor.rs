use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::{Duration, Instant};

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

/// A CLI process that OpenCompanion did not start. Read-only by design (PRD FR-31).
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
        if joined.contains("@kaitranntt/ccs") {
            return Some(CliKind::Ccs);
        }
        // omp's Bun shim starts `bun .../@oh-my-pi/pi-coding-agent/dist/cli.js`. It is a Pi fork
        // with the same package name, so it is checked before Pi.
        if joined.contains("/@oh-my-pi/") {
            return Some(CliKind::Omp);
        }
        // Pi's own installer starts `node ~/.pi/agent/bin/pi-launcher.js`, which runs the package.
        if joined.contains("/pi-coding-agent/") || joined.contains("/.pi/agent/bin/pi-launcher.js") {
            return Some(CliKind::Pi);
        }
    }
    None
}

/// Reads only the flags that decide the mode. The full command line is never stored: it can hold
/// the prompt text or tokens.
pub fn run_mode(kind: CliKind, args: &[String]) -> RunMode {
    let has = |flag: &str| args.iter().skip(1).any(|a| a == flag);
    let has_mode = |mode: &str| args.windows(2).any(|w| w[0] == "--mode" && w[1] == mode);
    let first = args
        .iter()
        .skip(1)
        .find(|a| !a.starts_with('-'))
        .map(String::as_str);
    match kind {
        CliKind::Claude | CliKind::Ccs | CliKind::Gemini if has("-p") || has("--print") => RunMode::Headless,
        CliKind::Codex if matches!(first, Some("exec") | Some("e")) => RunMode::Headless,
        CliKind::Codex if matches!(first, Some("app-server") | Some("mcp-server")) => {
            RunMode::Server
        }
        CliKind::Opencode if first == Some("run") => RunMode::Headless,
        CliKind::Opencode if matches!(first, Some("serve") | Some("web")) => RunMode::Server,
        CliKind::Pi if has_mode("rpc") => RunMode::Server,
        CliKind::Omp if has_mode("rpc") || has_mode("rpc-ui") || has_mode("acp") || first == Some("acp") => RunMode::Server,
        CliKind::Pi | CliKind::Omp if has("-p") || has("--print") || has_mode("json") => RunMode::Headless,
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
    /// When `usage` last refreshed the process list.
    measured: Option<Instant>,
}

impl Default for Monitor {
    fn default() -> Self {
        Self::new()
    }
}

impl Monitor {
    pub fn new() -> Self {
        Self {
            sys: System::new(),
            measured: None,
        }
    }

    /// Lists CLI processes that are not descendants of `own_pids` (sessions OpenCompanion started).
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
        self.measured = Some(Instant::now());
        let procs = self.sys.processes();
        let kind_of = |p: &Process| classify(&p.name().to_string_lossy(), &arg_strings(p));

        let cpus = cpu_count();
        let mut found = Vec::new();
        for (pid, p) in procs {
            let Some(kind) = kind_of(p) else { continue };

            // Walk up once: skip children of the same CLI (OpenCode's wrapper exe starts the
            // platform exe), Claude Code started by CCS (through cmd.exe for a `.cmd` install; the
            // CCS row stands for it) and anything OpenCompanion spawned.
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
                if depth <= 1 && kind == CliKind::Claude && kind_of(parent) == Some(CliKind::Ccs) {
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
                cpu_percent: share(p.cpu_usage(), cpus),
                memory_bytes: p.memory() + child_memory(procs, *pid),
            });
        }
        found.sort_by_key(|s| s.started_at);
        found
    }
}

/// CPU and memory of a session's process and everything it started (PRD FR-34).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Usage {
    /// Share of the whole machine, as Task Manager counts it.
    pub cpu_percent: f32,
    pub memory_bytes: u64,
    /// Processes under the CLI: shells, test runners, language servers.
    pub children: usize,
}

impl Monitor {
    /// Measures `pid` and its descendants. CPU needs two measurements, so the first call for a
    /// process reports 0; a call soon after the last one reuses it rather than measuring noise.
    pub fn usage(&mut self, pid: u32) -> Option<Usage> {
        if self.measured.is_none_or(|t| t.elapsed() >= Duration::from_millis(500)) {
            self.sys.refresh_processes_specifics(
                ProcessesToUpdate::All,
                true,
                ProcessRefreshKind::nothing().with_cpu().with_memory(),
            );
            self.measured = Some(Instant::now());
        }
        let procs = self.sys.processes();
        let root = procs.get(&Pid::from_u32(pid))?;
        let tree = descendants(procs, Pid::from_u32(pid));
        let cpu = tree.iter().chain([&root]).map(|p| p.cpu_usage()).sum::<f32>();
        Some(Usage {
            cpu_percent: share(cpu, cpu_count()),
            memory_bytes: tree.iter().chain([&root]).map(|p| p.memory()).sum(),
            children: tree.len(),
        })
    }
}

impl Monitor {
    /// The AI CLIs running under each root, keyed by what the root belongs to: a session's own
    /// terminal and the shells in its tabs share the session's key. Keys with none are left out.
    pub fn clis_under(&mut self, roots: &[(String, u32)]) -> HashMap<String, Vec<CliKind>> {
        if roots.is_empty() {
            return HashMap::new();
        }
        self.sys.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing().with_cmd(UpdateKind::OnlyIfNotSet),
        );
        let procs = self.sys.processes();
        let mut children: HashMap<u32, Vec<u32>> = HashMap::new();
        for p in procs.values() {
            let Some(parent) = p.parent().and_then(|pid| procs.get(&pid)) else { continue };
            // Windows reuses PIDs: a process older than its "parent" was started by an earlier
            // process with the same PID.
            if p.start_time() >= parent.start_time() {
                children.entry(parent.pid().as_u32()).or_default().push(p.pid().as_u32());
            }
        }
        let kind_of = |pid: u32| {
            let p = procs.get(&Pid::from_u32(pid))?;
            classify(&p.name().to_string_lossy(), &arg_strings(p))
        };
        let mut out: HashMap<String, Vec<CliKind>> = HashMap::new();
        for (key, pid) in roots {
            if !procs.contains_key(&Pid::from_u32(*pid)) {
                continue;
            }
            let found = outer_clis(&children, kind_of, *pid);
            if !found.is_empty() {
                out.entry(key.clone()).or_default().extend(found);
            }
        }
        out
    }
}

/// The CLIs at or below `root`. A CLI started by another one (OpenCode's wrapper exe, CCS starting
/// Claude Code, a CLI running another as a tool) is part of the outer one and is not counted again.
fn outer_clis(children: &HashMap<u32, Vec<u32>>, kind_of: impl Fn(u32) -> Option<CliKind>, root: u32) -> Vec<CliKind> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    let mut queue = vec![root];
    while let Some(next) = queue.pop() {
        // A reused PID can point a process at its own child; stop instead of looping.
        if !seen.insert(next) || seen.len() > 4096 {
            continue;
        }
        match kind_of(next) {
            Some(kind) => out.push(kind),
            None => queue.extend(children.get(&next).into_iter().flatten()),
        }
    }
    out
}

fn cpu_count() -> f32 {
    std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1) as f32
}

/// sysinfo counts one busy core as 100%; Task Manager counts the whole machine as 100%.
fn share(per_core: f32, cpus: f32) -> f32 {
    (per_core / cpus * 10.0).round() / 10.0
}

/// Every process below `pid`, however deep.
fn descendants(procs: &HashMap<Pid, Process>, pid: Pid) -> Vec<&Process> {
    let mut children: HashMap<Pid, Vec<&Process>> = HashMap::new();
    for p in procs.values() {
        if let Some(parent) = p.parent() {
            children.entry(parent).or_default().push(p);
        }
    }
    let mut out = Vec::new();
    let mut queue = vec![pid];
    while let Some(next) = queue.pop() {
        for c in children.get(&next).into_iter().flatten() {
            // A reused PID can point a process at its own child; stop instead of looping.
            if out.len() > 4096 || c.pid() == pid {
                return out;
            }
            out.push(*c);
            queue.push(c.pid());
        }
    }
    out
}

/// The CLI process `pid`, when it still runs, found without scanning every process.
pub fn find(pid: u32) -> Option<ExternalSession> {
    let mut sys = System::new();
    let target = Pid::from_u32(pid);
    sys.refresh_processes_specifics(
        ProcessesToUpdate::Some(&[target]),
        true,
        ProcessRefreshKind::nothing()
            .with_memory()
            .with_cmd(UpdateKind::OnlyIfNotSet)
            .with_cwd(UpdateKind::OnlyIfNotSet),
    );
    let p = sys.process(target)?;
    let args = arg_strings(p);
    let kind = classify(&p.name().to_string_lossy(), &args)?;
    Some(ExternalSession {
        pid,
        kind,
        mode: run_mode(kind, &args),
        cwd: p.cwd().map(|c| c.to_path_buf()),
        started_at: p.start_time(),
        cpu_percent: 0.0,
        memory_bytes: p.memory(),
    })
}

/// Memory of direct children (shells and tools the CLI started), so the number means something.
fn child_memory(procs: &HashMap<Pid, Process>, pid: Pid) -> u64 {
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
        let ccs = args(&["node", r"C:\nvm4w\nodejs\node_modules\@kaitranntt\ccs\dist\ccs.js", "work", "-p"]);
        assert_eq!(classify("node.exe", &ccs), Some(CliKind::Ccs));
        assert_eq!(run_mode(CliKind::Ccs, &ccs), RunMode::Headless);
        let launcher = args(&["node", r"C:\Users\me\.pi\agent\bin\pi-launcher.js", "--mode", "json"]);
        assert_eq!(classify("node.exe", &launcher), Some(CliKind::Pi));
        assert_eq!(run_mode(CliKind::Pi, &launcher), RunMode::Headless);
        let pi = args(&["node", r"C:\npm\node_modules\@earendil-works\pi-coding-agent\dist\bundle\cli.js"]);
        assert_eq!(classify("node.exe", &pi), Some(CliKind::Pi));
        assert_eq!(run_mode(CliKind::Pi, &pi), RunMode::Interactive);
        assert_eq!(run_mode(CliKind::Pi, &args(&["pi", "--mode", "rpc"])), RunMode::Server);
        assert_eq!(classify("pi.exe", &[]), Some(CliKind::Pi));
        let omp = args(&["bun", r"C:\Users\me\.bun\install\global\node_modules\@oh-my-pi\pi-coding-agent\dist\cli.js", "--mode", "json"]);
        assert_eq!(classify("bun.exe", &omp), Some(CliKind::Omp));
        assert_eq!(run_mode(CliKind::Omp, &omp), RunMode::Headless);
        assert_eq!(classify("omp.exe", &[]), Some(CliKind::Omp));
        assert_eq!(run_mode(CliKind::Omp, &args(&["omp", "acp"])), RunMode::Server);
        assert_eq!(run_mode(CliKind::Omp, &args(&["omp", "--mode", "rpc-ui"])), RunMode::Server);
        assert_eq!(run_mode(CliKind::Omp, &args(&["omp", "--approval-mode", "write"])), RunMode::Interactive);
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
        let found = find(child.id());
        let usage = monitor.usage(std::process::id());
        let under_self = monitor.clis_under(&[("s1".into(), std::process::id()), ("s2".into(), child.id())]);
        let _ = child.kill();
        let _ = child.wait();
        let _ = std::fs::remove_dir_all(&dir);

        let pid = child.id();
        assert!(without_self.iter().any(|s| s.pid == pid && s.kind == CliKind::Opencode));
        assert!(!with_self.iter().any(|s| s.pid == pid));
        assert_eq!(found.map(|s| s.kind), Some(CliKind::Opencode));
        // The helper runs under this test process, so it counts toward its usage.
        let usage = usage.unwrap();
        assert!(usage.children >= 1 && usage.memory_bytes > 0);
        // Found under the test process, and under its own PID as the root.
        assert!(under_self["s1"].contains(&CliKind::Opencode));
        assert_eq!(under_self["s2"], [CliKind::Opencode]);
        assert!(find(pid).is_none());
    }

    #[test]
    #[ignore = "helper process for own_descendants_are_not_outside_sessions"]
    fn sleep_helper() {
        if std::env::var_os("AIR_SLEEP_HELPER").is_some() {
            std::thread::sleep(std::time::Duration::from_secs(10));
        }
    }

    #[test]
    fn counts_each_cli_once_under_a_root_however_deep() {
        // 1 is the session's shell: Claude Code in it, and CCS under a nested shell starting its
        // own Claude Code. 2 is a tab's shell running Codex, which runs Claude Code as a tool.
        let tree = [(1, 10), (1, 11), (11, 12), (12, 13), (1, 14), (2, 20), (20, 21), (9, 90)];
        let mut children: HashMap<u32, Vec<u32>> = HashMap::new();
        for (parent, child) in tree {
            children.entry(parent).or_default().push(child);
        }
        let kind_of = |pid: u32| match pid {
            10 | 13 | 21 | 90 => Some(CliKind::Claude),
            12 => Some(CliKind::Ccs),
            20 => Some(CliKind::Codex),
            _ => None,
        };
        let mut found = outer_clis(&children, kind_of, 1);
        found.sort_by_key(|k| k.bin());
        assert_eq!(found, [CliKind::Ccs, CliKind::Claude]);
        assert_eq!(outer_clis(&children, kind_of, 2), [CliKind::Codex]);
        // A headless session's root is the CLI itself.
        assert_eq!(outer_clis(&children, kind_of, 90), [CliKind::Claude]);
        assert_eq!(outer_clis(&children, kind_of, 11), [CliKind::Ccs]);
        assert!(outer_clis(&children, kind_of, 3).is_empty());
    }

    #[test]
    fn a_pid_loop_ends_the_walk() {
        let children = HashMap::from([(1, vec![2]), (2, vec![1, 3])]);
        let found = outer_clis(&children, |pid| (pid == 3).then_some(CliKind::Claude), 1);
        assert_eq!(found, [CliKind::Claude]);
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
