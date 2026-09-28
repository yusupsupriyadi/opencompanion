//! The environment of the user's login shell. Started from Finder or a desktop launcher, the app
//! gets a minimal PATH and none of the variables set in `~/.zshrc` or `~/.bashrc`, so CLIs would
//! not be found, `#!/usr/bin/env node` scripts would not find node, and API keys would be missing.
//! On Windows sessions start through PowerShell instead (`terminal::powershell`), so this is empty.

use std::path::PathBuf;
#[cfg(unix)]
use std::path::Path;
use std::sync::OnceLock;
#[cfg(unix)]
use std::time::Duration;

#[cfg(any(unix, test))]
const START: &str = "__OC_ENV_START__";
#[cfg(any(unix, test))]
const END: &str = "__OC_ENV_END__";
/// Variables that describe the capturing shell itself, not the user's setup.
#[cfg(any(unix, test))]
const SKIP: &[&str] = &["_", "PWD", "OLDPWD", "SHLVL", "TERM", "COLUMNS", "LINES"];
/// Install folders a GUI launch usually lacks, added after the shell's PATH when they exist.
pub const FALLBACK_DIRS: &[&str] = &[
    "/opt/homebrew/bin",
    "/usr/local/bin",
    "~/.local/bin",
    "~/.npm-global/bin",
    "~/.volta/bin",
    "~/.bun/bin",
    "~/.cargo/bin",
    "~/.claude/local",
];

static VARS: OnceLock<Vec<(String, String)>> = OnceLock::new();

/// Starts the capture in the background, so the first session does not wait for the shell.
pub fn start() {
    std::thread::spawn(|| {
        vars();
    });
}

/// The captured variables. The first call waits for the capture, at most its timeout.
pub fn vars() -> &'static [(String, String)] {
    VARS.get_or_init(load)
}

pub fn path() -> Option<&'static str> {
    vars().iter().find(|(k, _)| k == "PATH").map(|(_, v)| v.as_str())
}

#[cfg(windows)]
fn load() -> Vec<(String, String)> {
    Vec::new()
}

/// Asks the login shell for its environment. A shell that fails or takes over 5 s (a prompt, a
/// slow plugin manager) leaves the inherited PATH, which the fallback folders still extend.
#[cfg(unix)]
fn load() -> Vec<(String, String)> {
    let shell = std::env::var_os("SHELL")
        .map(PathBuf::from)
        .filter(|p| p.is_file())
        .unwrap_or_else(|| PathBuf::from("/bin/sh"));
    let script = format!("printf '%s\\n' {START}; /usr/bin/env; printf '%s\\n' {END}");
    let mut vars = capture_with(&shell, &["-ilc", &script], Duration::from_secs(5)).unwrap_or_default();
    let base = match vars.iter().position(|(k, _)| k == "PATH") {
        Some(i) => vars.remove(i).1,
        None => std::env::var("PATH").unwrap_or_default(),
    };
    vars.push(("PATH".into(), with_fallback(&base, &fallback_dirs())));
    vars
}

/// `FALLBACK_DIRS` with `~/` expanded.
pub fn fallback_dirs() -> Vec<PathBuf> {
    let home = crate::projects::home();
    FALLBACK_DIRS
        .iter()
        .filter_map(|d| match d.strip_prefix("~/") {
            Some(rest) => home.as_ref().map(|h| rest.split('/').fold(h.clone(), |acc, part| acc.join(part))),
            None => Some(PathBuf::from(d)),
        })
        .collect()
}

/// `base` followed by every folder of `dirs` that exists and is not in it yet.
#[cfg(any(unix, test))]
fn with_fallback(base: &str, dirs: &[PathBuf]) -> String {
    let mut parts: Vec<String> = base.split(':').filter(|p| !p.is_empty()).map(str::to_owned).collect();
    for dir in dirs.iter().filter(|d| d.is_dir()) {
        let dir = dir.display().to_string();
        if !parts.contains(&dir) {
            parts.push(dir);
        }
    }
    parts.join(":")
}

/// The variables `env` printed between the two markers. A line that starts with `NAME=` (no
/// space before the `=`) starts an entry, and any other line continues the value before it, as a
/// multi-line value prints. Entries whose name is not a variable name, such as the bash functions
/// Fedora exports (`BASH_FUNC_which%%=() {`), are dropped together with their continuation.
#[cfg(any(unix, test))]
fn parse(out: &str) -> Vec<(String, String)> {
    let mut lines = out.lines();
    if !lines.any(|l| l == START) {
        return Vec::new();
    }
    let is_name = |k: &str| {
        let mut chars = k.chars();
        chars.next().is_some_and(|c| c == '_' || c.is_ascii_alphabetic()) && chars.all(|c| c == '_' || c.is_ascii_alphanumeric())
    };
    // `None` marks a dropped entry, so its continuation lines are dropped too.
    let mut entries: Vec<Option<(String, String)>> = Vec::new();
    let mut ended = false;
    for line in lines {
        if line == END {
            ended = true;
            break;
        }
        let starts = line.split_once('=').map(|(k, _)| k).filter(|k| !k.is_empty() && !k.contains(char::is_whitespace));
        match (starts, entries.last_mut()) {
            (Some(k), _) if is_name(k) => entries.push(Some((k.to_string(), line[k.len() + 1..].to_string()))),
            (Some(_), _) => entries.push(None),
            (None, Some(Some((_, v)))) => {
                v.push('\n');
                v.push_str(line);
            }
            (None, _) => {}
        }
    }
    if !ended {
        return Vec::new();
    }
    entries
        .into_iter()
        .flatten()
        .filter(|(k, _)| !SKIP.contains(&k.as_str()) && !crate::proc::INHERITED_SESSION_VARS.contains(&k.as_str()))
        .collect()
}

/// Runs `shell args` with no terminal and stdin closed, without this module's own environment.
#[cfg(unix)]
fn capture_with(shell: &Path, args: &[&str], timeout: Duration) -> Option<Vec<(String, String)>> {
    let mut cmd = crate::proc::hidden(shell);
    cmd.args(args);
    let out = crate::proc::run_with_timeout(cmd, timeout).ok()?;
    if out.timed_out {
        return None;
    }
    Some(parse(&out.stdout))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_text_between_the_markers_counts() {
        let out = format!(
            "motd line\nPATH=/nope\n{START}\nPATH=/a:/b\nMULTI=one\ntwo\nHOME=/h\n_=/usr/bin/env\nSHLVL=2\nCLAUDECODE=1\n{END}\nbye\n"
        );
        let vars = parse(&out);
        let want: Vec<(String, String)> = [("PATH", "/a:/b"), ("MULTI", "one\ntwo"), ("HOME", "/h")]
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        assert_eq!(vars, want);
        assert!(parse("no markers at all").is_empty());
        // Fedora and RHEL export `which` as a bash function; its name and body are not a variable.
        let fedora = format!(
            "{START}\nPATH=/usr/bin:/bin\nBASH_FUNC_which%%=() {{  ( alias;\n eval ${{which_declare}} ) | /usr/bin/which --tty-only \"$@\"\n}}\nHOME=/h\n{END}\n"
        );
        let vars = parse(&fedora);
        assert_eq!(vars, [("PATH".to_string(), "/usr/bin:/bin".to_string()), ("HOME".to_string(), "/h".to_string())]);
        assert!(parse(&format!("{START}\nPATH=/a\n")).is_empty(), "a capture cut before its end marker is not trusted");
    }

    #[test]
    fn the_fallback_folders_join_the_path_once_and_only_when_they_exist() {
        let dir = std::env::temp_dir().join(format!("oc-shellenv-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("bin")).unwrap();
        let bin = dir.join("bin");
        let joined = with_fallback("/usr/bin", &[bin.clone(), dir.join("missing"), bin.clone()]);
        assert_eq!(joined, format!("/usr/bin:{}", bin.display()));
        assert_eq!(with_fallback("", std::slice::from_ref(&bin)), bin.display().to_string());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn fallback_folders_expand_the_home_folder() {
        let dirs = fallback_dirs();
        assert!(dirs.contains(&PathBuf::from("/opt/homebrew/bin")));
        if let Some(home) = crate::projects::home() {
            assert!(dirs.contains(&home.join(".npm-global").join("bin")));
        }
        assert!(dirs.iter().all(|d| !d.display().to_string().starts_with('~')));
    }

    #[cfg(unix)]
    #[test]
    fn a_shell_that_hangs_is_cut_at_the_timeout() {
        let started = std::time::Instant::now();
        assert!(capture_with(Path::new("/bin/sh"), &["-c", "sleep 30"], Duration::from_millis(300)).is_none());
        assert!(started.elapsed() < Duration::from_secs(3));
    }

    #[cfg(unix)]
    #[test]
    fn a_real_shell_reports_its_variables() {
        let script = format!("printf '%s\\n' {START}; OC_PROBE='a b' /usr/bin/env; printf '%s\\n' {END}");
        let vars = capture_with(Path::new("/bin/sh"), &["-c", &script], Duration::from_secs(5)).expect("sh answers");
        assert!(vars.iter().any(|(k, v)| k == "OC_PROBE" && v == "a b"), "{vars:?}");
        assert!(vars.iter().any(|(k, _)| k == "PATH"));
    }

    /// An interactive shell reads /dev/tty. Started from a terminal, it must answer instead of
    /// being stopped for touching a terminal it does not own (run under `script` to have one).
    #[cfg(unix)]
    #[test]
    fn an_interactive_shell_answers_when_the_app_has_a_terminal() {
        let script = format!("printf '%s\\n' {START}; /usr/bin/env; printf '%s\\n' {END}");
        let started = std::time::Instant::now();
        let vars = capture_with(Path::new("/bin/bash"), &["-ic", &script], Duration::from_secs(5)).expect("bash answers");
        assert!(vars.iter().any(|(k, _)| k == "PATH"));
        assert!(started.elapsed() < Duration::from_secs(3), "took {:?}", started.elapsed());
    }

    #[cfg(windows)]
    #[test]
    fn windows_adds_nothing() {
        assert!(vars().is_empty());
        assert_eq!(path(), None);
    }
}
