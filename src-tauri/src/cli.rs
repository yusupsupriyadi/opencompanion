use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::proc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CliKind {
    Claude,
    Codex,
    Opencode,
    Gemini,
}

impl CliKind {
    pub const ALL: [CliKind; 4] = [
        CliKind::Claude,
        CliKind::Codex,
        CliKind::Opencode,
        CliKind::Gemini,
    ];

    pub fn bin(self) -> &'static str {
        match self {
            CliKind::Claude => "claude",
            CliKind::Codex => "codex",
            CliKind::Opencode => "opencode",
            CliKind::Gemini => "gemini",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            CliKind::Claude => "Claude Code",
            CliKind::Codex => "Codex CLI",
            CliKind::Opencode => "OpenCode",
            CliKind::Gemini => "Gemini CLI",
        }
    }

    /// `major.minor` lines the adapter was checked against (PRD section 6).
    pub fn tested_lines(self) -> &'static [&'static str] {
        match self {
            CliKind::Claude => &["2.1"],
            CliKind::Codex => &["0.153"],
            CliKind::Opencode => &["1.18"],
            CliKind::Gemini => &[],
        }
    }

    pub fn from_bin(name: &str) -> Option<CliKind> {
        CliKind::ALL.into_iter().find(|k| k.bin() == name)
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CliInstall {
    pub kind: CliKind,
    pub label: &'static str,
    pub path: Option<PathBuf>,
    pub version: Option<String>,
    /// False when the version is outside `tested_lines`, or unknown.
    pub tested: bool,
    pub error: Option<String>,
}

/// Install folders that are often missing from PATH for GUI apps started from the Start menu.
fn extra_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(home) = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")) {
        let home = PathBuf::from(home);
        dirs.push(home.join(".local").join("bin"));
        dirs.push(home.join(".bun").join("bin"));
        dirs.push(home.join(".opencode").join("bin"));
    }
    if let Some(appdata) = std::env::var_os("APPDATA") {
        dirs.push(PathBuf::from(appdata).join("npm"));
    }
    dirs
}

pub fn resolve(kind: CliKind) -> Option<PathBuf> {
    if let Ok(path) = which::which(kind.bin()) {
        return Some(unwrap_shim(path));
    }
    let dirs = std::env::join_paths(extra_dirs().into_iter().filter(|d| d.is_dir())).ok()?;
    let cwd = std::env::current_dir().ok()?;
    which::which_in(kind.bin(), Some(dirs), cwd)
        .ok()
        .map(unwrap_shim)
}

/// An npm `.cmd` shim that only forwards to a native exe is replaced by that exe, so prompts
/// never pass through cmd.exe quoting. Shims that start a node script are kept as they are.
fn unwrap_shim(path: PathBuf) -> PathBuf {
    let is_cmd = path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("cmd"));
    if !is_cmd {
        return path;
    }
    let (Ok(text), Some(dir)) = (std::fs::read_to_string(&path), path.parent()) else {
        return path;
    };
    shim_target(&text)
        .map(|rel| dir.join(rel))
        .filter(|p| p.is_file())
        .unwrap_or(path)
}

/// Finds `"%dp0%\...\name.exe"` in an npm cmd-shim.
fn shim_target(text: &str) -> Option<PathBuf> {
    let start = text.find("\"%dp0%\\")? + "\"%dp0%\\".len();
    let rest = &text[start..];
    let rel = &rest[..rest.find('"')?];
    rel.to_ascii_lowercase()
        .ends_with(".exe")
        .then(|| PathBuf::from(rel.replace('\\', std::path::MAIN_SEPARATOR_STR)))
}

/// First `N.N` or `N.N.N` token, so "codex-cli 0.153.4" and "2.1.282 (Claude Code)" both work.
pub fn parse_version(raw: &str) -> Option<String> {
    raw.split(|c: char| c.is_whitespace() || c == '(' || c == ')' || c == ',')
        .map(|t| t.trim_start_matches('v'))
        .find(|t| {
            let parts: Vec<&str> = t.split('.').collect();
            parts.len() >= 2
                && parts
                    .iter()
                    .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
        })
        .map(str::to_owned)
}

pub fn is_tested(kind: CliKind, version: &str) -> bool {
    kind.tested_lines().iter().any(|line| {
        version == *line
            || version
                .strip_prefix(line)
                .is_some_and(|rest| rest.starts_with('.'))
    })
}

fn read_version(path: &Path) -> Result<String, String> {
    let mut cmd = proc::command(path);
    cmd.arg("--version");
    let out = proc::run_with_timeout(cmd, Duration::from_secs(15)).map_err(|e| e.to_string())?;
    if out.timed_out {
        return Err("`--version` did not answer within 15 s".into());
    }
    let text = format!("{}\n{}", out.stdout, out.stderr);
    parse_version(&text).ok_or_else(|| {
        let first = text.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
        format!("could not read a version from `--version`: {first}")
    })
}

pub fn detect(kind: CliKind) -> CliInstall {
    detect_at(kind, resolve(kind))
}

/// Detection for a known path, used for a custom executable set in Settings (PRD FR-04).
pub fn detect_at(kind: CliKind, path: Option<PathBuf>) -> CliInstall {
    let mut install = CliInstall {
        kind,
        label: kind.label(),
        path,
        version: None,
        tested: false,
        error: None,
    };
    if let Some(path) = &install.path {
        match read_version(path) {
            Ok(v) => {
                install.tested = is_tested(kind, &v);
                install.version = Some(v);
            }
            Err(e) => install.error = Some(e),
        }
    }
    install
}

/// Detects every supported CLI in parallel; npm shims can take a second or two each.
pub fn detect_all() -> Vec<CliInstall> {
    let handles: Vec<_> = CliKind::ALL
        .into_iter()
        .map(|k| thread::spawn(move || detect(k)))
        .collect();
    handles
        .into_iter()
        .zip(CliKind::ALL)
        .map(|(h, k)| {
            h.join().unwrap_or_else(|_| CliInstall {
                kind: k,
                label: k.label(),
                path: None,
                version: None,
                tested: false,
                error: Some("detection thread panicked".into()),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_real_version_strings() {
        assert_eq!(parse_version("2.1.282 (Claude Code)").as_deref(), Some("2.1.282"));
        assert_eq!(parse_version("codex-cli 0.153.4").as_deref(), Some("0.153.4"));
        assert_eq!(parse_version("1.18.30\n").as_deref(), Some("1.18.30"));
        assert_eq!(parse_version("gemini v0.9.0").as_deref(), Some("0.9.0"));
        assert_eq!(parse_version("command not found"), None);
        assert_eq!(parse_version("see 1. or .2"), None);
    }

    #[test]
    fn finds_native_exe_in_npm_shim_only() {
        let exe = "@SETLOCAL\r\nCALL :find_dp0\r\n\"%dp0%\\node_modules\\opencode-ai\\bin\\opencode.exe\"   %*\r\n";
        let target = shim_target(exe).expect("exe target");
        assert!(target.ends_with("opencode.exe"));
        let node = "\"%_prog%\"  \"%dp0%\\node_modules\\@openai\\codex\\bin\\codex.js\" %*\r\n";
        assert_eq!(shim_target(node), None);
    }

    #[test]
    fn tested_matches_major_minor_line_only() {
        assert!(is_tested(CliKind::Claude, "2.1.282"));
        assert!(is_tested(CliKind::Claude, "2.1"));
        assert!(!is_tested(CliKind::Claude, "2.10.0"));
        assert!(!is_tested(CliKind::Codex, "0.154.0"));
        assert!(!is_tested(CliKind::Gemini, "0.9.0"));
    }
}
