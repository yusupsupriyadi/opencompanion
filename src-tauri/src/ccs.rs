//! CCS (`@kaitranntt/ccs`, checked against 8.10.0) starts Claude Code with a profile:
//! `ccs [profile] [claude args]`. Like CCS itself, the first argument that is not a flag is the
//! profile, so the extra arguments from Settings, which name it, go right after `ccs`.

use std::fs;
use std::path::{Path, PathBuf};

/// The profile the extra arguments name. `None` is CCS's default profile.
pub fn profile(extra: &[String]) -> Option<&str> {
    extra.first().map(String::as_str).filter(|a| !a.starts_with('-'))
}

/// True when CCS hands Claude Code the arguments as given: the default profile and account
/// profiles (one folder each in `~/.ccs/instances`). API and CLIProxy profiles add their own
/// `--settings`, which a second `--settings` replaces, and API profiles run `-p` through CCS's
/// delegation, which reads the argument after `-p` as the prompt.
pub fn plain_profile(home: &Path, profile: Option<&str>) -> bool {
    match profile {
        None | Some("default") => true,
        Some(name) => {
            !name.starts_with('.')
                && !name.contains(['/', '\\'])
                && home.join(".ccs").join("instances").join(name).is_dir()
        }
    }
}

/// `plain_profile` for the profile the extra arguments name, in the real home folder.
pub fn plain(extra: &[String]) -> bool {
    let profile = profile(extra);
    match crate::projects::home() {
        Some(home) => plain_profile(&home, profile),
        None => profile.is_none(),
    }
}

/// Claude Code config folders CCS starts it with: `~/.claude` for the default profile, then one
/// per account profile.
pub fn claude_dirs(home: &Path) -> Vec<PathBuf> {
    let mut dirs = vec![home.join(".claude")];
    if let Ok(entries) = fs::read_dir(home.join(".ccs").join("instances")) {
        let mut accounts: Vec<PathBuf> = entries
            .filter_map(|e| e.ok())
            .filter(|e| !e.file_name().to_string_lossy().starts_with('.'))
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .collect();
        accounts.sort();
        dirs.extend(accounts);
    }
    dirs
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    fn temp_home(name: &str) -> PathBuf {
        let home = std::env::temp_dir().join(format!("oc-ccs-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&home);
        fs::create_dir_all(home.join(".ccs").join("instances").join("work")).unwrap();
        fs::create_dir_all(home.join(".ccs").join("instances").join(".locks")).unwrap();
        home
    }

    #[test]
    fn the_first_argument_that_is_not_a_flag_is_the_profile() {
        assert_eq!(profile(&args(&["work", "--effort", "high"])), Some("work"));
        assert_eq!(profile(&args(&["--effort", "high"])), None);
        assert_eq!(profile(&[]), None);
    }

    #[test]
    fn only_default_and_account_profiles_are_plain() {
        let home = temp_home("plain");
        assert!(plain_profile(&home, None));
        assert!(plain_profile(&home, Some("default")));
        assert!(plain_profile(&home, Some("work")));
        assert!(!plain_profile(&home, Some("glm")));
        assert!(!plain_profile(&home, Some(".locks")));
        assert!(!plain_profile(&home, Some("../work")));
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn claude_dirs_are_the_default_then_each_account() {
        let home = temp_home("dirs");
        let dirs = claude_dirs(&home);
        assert_eq!(dirs, [home.join(".claude"), home.join(".ccs").join("instances").join("work")]);
        let _ = fs::remove_dir_all(&home);
    }
}
