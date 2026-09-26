//! Finds the user's project folders so the chat planner can match "ai-remote" to a real path
//! without asking (PRD FR-24). Sources, strongest first: folders used in OpenCompanion, folders of
//! CLIs running right now, Claude Code's own project history, and the children of common
//! project roots such as `~/Project`.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectFolder {
    pub path: String,
    pub name: String,
    /// Files that say what kind of project it is: `git`, `package.json`, `Cargo.toml`, ...
    pub markers: Vec<&'static str>,
    /// `recent`, `open`, `claude`, `root`.
    pub source: &'static str,
}

const MARKERS: [(&str, &str); 11] = [
    ("src-tauri/tauri.conf.json", "tauri"),
    ("svelte.config.js", "svelte"),
    (".git", "git"),
    ("package.json", "package.json"),
    ("Cargo.toml", "Cargo.toml"),
    ("pyproject.toml", "pyproject.toml"),
    ("requirements.txt", "requirements.txt"),
    ("go.mod", "go.mod"),
    ("composer.json", "composer.json"),
    ("pubspec.yaml", "pubspec.yaml"),
    ("tauri.conf.json", "tauri"),
];

/// Folder names under the home folder that usually hold projects.
const ROOT_NAMES: [&str; 11] = [
    "Project",
    "Projects",
    "projects",
    "code",
    "Code",
    "dev",
    "src",
    "repos",
    "workspace",
    "source/repos",
    "Documents/GitHub",
];

pub(crate) fn home() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
}

pub fn default_roots() -> Vec<PathBuf> {
    let Some(home) = home() else { return vec![] };
    let mut seen: Vec<PathBuf> = Vec::new();
    for name in ROOT_NAMES {
        let p = name.split('/').fold(home.clone(), |acc, part| acc.join(part));
        // `Project` and `projects` are the same folder on Windows.
        if p.is_dir() && !seen.iter().any(|s| same_path(s, &p)) {
            seen.push(p);
        }
    }
    seen
}

fn same_path(a: &Path, b: &Path) -> bool {
    norm(&a.display().to_string()) == norm(&b.display().to_string())
}

pub(crate) fn norm(p: &str) -> String {
    p.trim_end_matches(['\\', '/']).replace('/', "\\").to_lowercase()
}

/// Claude Code names a project folder by replacing every non-alphanumeric character with `-`.
pub(crate) fn claude_encode(name: &str) -> String {
    name.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect()
}

/// Walks the real filesystem to undo `claude_encode`, which is lossy: `ai-remote` and
/// `ai\remote` encode the same. The longest existing child that fits wins at each level.
pub fn decode_claude_dir(encoded: &str, cache: &mut HashMap<PathBuf, Vec<String>>) -> Option<PathBuf> {
    let mut chars = encoded.chars();
    let drive = chars.next().filter(char::is_ascii_alphabetic)?;
    let rest = encoded.get(1..)?.strip_prefix("--")?;
    let mut dir = PathBuf::from(format!("{drive}:\\"));
    let mut remaining = rest.to_string();
    while !remaining.is_empty() {
        let children = cache
            .entry(dir.clone())
            .or_insert_with(|| {
                fs::read_dir(&dir)
                    .map(|rd| {
                        rd.filter_map(|e| e.ok())
                            .filter(|e| e.path().is_dir())
                            .map(|e| e.file_name().to_string_lossy().into_owned())
                            .collect()
                    })
                    .unwrap_or_default()
            })
            .clone();
        let lower = remaining.to_lowercase();
        let best = children
            .iter()
            .filter(|c| {
                let enc = claude_encode(c).to_lowercase();
                lower == enc || lower.starts_with(&format!("{enc}-"))
            })
            .max_by_key(|c| c.len())?;
        dir = dir.join(best);
        let used = claude_encode(best).len();
        remaining = remaining.get(used..).unwrap_or("").trim_start_matches('-').to_string();
        // A dot-folder encodes as `--name`; the extra dash was consumed above.
    }
    Some(dir)
}

fn markers(path: &Path) -> Vec<&'static str> {
    MARKERS
        .iter()
        .filter(|(file, _)| path.join(file).exists())
        .map(|(_, label)| *label)
        .collect()
}

/// App data, the system temp folder (where CLIs leave scratch sessions) and build or VCS
/// internals are never projects.
fn skip(path: &Path) -> bool {
    let s = norm(&path.display().to_string());
    let temp = norm(&std::env::temp_dir().display().to_string());
    s.starts_with(&temp) || s.contains("\\appdata\\") || s.contains("\\node_modules") || s.contains("\\.git")
}

fn claude_history(cache: &mut HashMap<PathBuf, Vec<String>>) -> Vec<PathBuf> {
    let Some(dir) = home().map(|h| h.join(".claude").join("projects")) else { return vec![] };
    let Ok(entries) = fs::read_dir(dir) else { return vec![] };
    let mut named: Vec<(SystemTime, String)> = entries
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .map(|e| {
            let when = e.metadata().and_then(|m| m.modified()).unwrap_or(SystemTime::UNIX_EPOCH);
            (when, e.file_name().to_string_lossy().into_owned())
        })
        .collect();
    named.sort_by_key(|n| std::cmp::Reverse(n.0));
    named
        .into_iter()
        .take(120)
        .filter_map(|(_, n)| decode_claude_dir(&n, cache))
        .collect()
}

/// Everything the planner may pick from, deduplicated, strongest sources first.
pub fn discover(recent: &[String], open: &[String], roots: &[String], limit: usize) -> Vec<ProjectFolder> {
    let mut cache = HashMap::new();
    let history = claude_history(&mut cache);
    discover_with(recent, open, &history, roots, limit)
}

/// `discover` with Claude Code's history passed in, so tests do not read the real one.
pub fn discover_with(recent: &[String], open: &[String], history: &[PathBuf], roots: &[String], limit: usize) -> Vec<ProjectFolder> {
    let mut out: Vec<ProjectFolder> = Vec::new();
    let root_paths: Vec<PathBuf> = if roots.is_empty() {
        default_roots()
    } else {
        roots.iter().map(PathBuf::from).filter(|p| p.is_dir()).collect()
    };
    let home = home();
    let mut push = |path: PathBuf, source: &'static str| {
        if !path.is_dir() || skip(&path) {
            return;
        }
        // Roots and the home folder hold projects; they are not projects.
        if root_paths.iter().any(|r| same_path(r, &path)) || home.as_deref().is_some_and(|h| same_path(h, &path)) {
            return;
        }
        let key = norm(&path.display().to_string());
        if out.iter().any(|p| norm(&p.path) == key) {
            return;
        }
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.display().to_string());
        out.push(ProjectFolder {
            markers: markers(&path),
            path: tidy(&path.display().to_string()),
            name,
            source,
        });
    };

    for r in recent {
        push(PathBuf::from(r), "recent");
    }
    for o in open {
        push(PathBuf::from(o), "open");
    }
    for p in history {
        push(p.clone(), "claude");
    }
    for root in &root_paths {
        let Ok(entries) = fs::read_dir(root) else { continue };
        let mut children: Vec<PathBuf> = entries
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .filter(|p| !p.file_name().is_some_and(|n| n.to_string_lossy().starts_with('.')))
            .collect();
        children.sort();
        for c in children {
            push(c, "root");
        }
    }
    out.truncate(limit);
    out
}

/// Drops a trailing separator (`C:\p\app\` from a process working folder) but keeps a
/// drive root such as `C:\`.
pub fn tidy(path: &str) -> String {
    let trimmed = path.trim_end_matches(['\\', '/']);
    if trimmed.ends_with(':') || trimmed.is_empty() {
        path.to_string()
    } else {
        trimmed.to_string()
    }
}

fn squash(s: &str) -> String {
    s.chars().filter(|c| c.is_alphanumeric()).collect::<String>().to_lowercase()
}

/// Maps what the planner wrote to a real folder: an existing path stays; a bare name or a
/// wrong path is matched by folder name, but only when exactly one known folder fits.
pub fn resolve(folder: &str, known: &[ProjectFolder]) -> Option<String> {
    let f = folder.trim().trim_matches(['"', '\'']);
    if f.is_empty() {
        return None;
    }
    if Path::new(f).is_dir() {
        return Some(tidy(f));
    }
    let wanted = Path::new(f.trim_end_matches(['\\', '/']))
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| f.to_string());
    let exact: Vec<&ProjectFolder> = known.iter().filter(|k| k.name.eq_ignore_ascii_case(&wanted)).collect();
    if exact.len() == 1 {
        return Some(exact[0].path.clone());
    }
    let key = squash(&wanted);
    let loose: Vec<&ProjectFolder> = known.iter().filter(|k| !key.is_empty() && squash(&k.name) == key).collect();
    (loose.len() == 1).then(|| loose[0].path.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree(name: &str) -> PathBuf {
        // Not under %TEMP%: discovery skips the temp folder on purpose.
        let base = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join(format!("air-projects-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        for p in ["Work/ai-remote", "Work/ai", "Work/my.app/sub", "Work/.hidden"] {
            fs::create_dir_all(base.join(p)).unwrap();
        }
        fs::write(base.join("Work/ai-remote/package.json"), "{}").unwrap();
        base
    }

    #[test]
    fn claude_names_decode_to_real_folders_even_with_dashes_and_dots() {
        let base = tree("decode");
        let mut cache = HashMap::new();
        let enc = |p: &Path| {
            let s = p.display().to_string();
            let drive = &s[..1];
            format!("{drive}--{}", claude_encode(&s[3..]))
        };
        let target = base.join("Work").join("ai-remote");
        assert_eq!(decode_claude_dir(&enc(&target), &mut cache), Some(target.clone()));
        let dotted = base.join("Work").join("my.app").join("sub");
        assert_eq!(decode_claude_dir(&enc(&dotted), &mut cache), Some(dotted));
        assert_eq!(decode_claude_dir("C--definitely-not-here-xyz", &mut cache), None);
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn roots_list_their_children_with_markers_and_skip_hidden() {
        let base = tree("roots");
        let root = base.join("Work").display().to_string();
        let found = discover_with(&[], &[], &[], std::slice::from_ref(&root), 50);
        let names: Vec<&str> = found.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, ["ai", "ai-remote", "my.app"]);
        assert_eq!(found[1].markers, ["package.json"]);
        assert!(found.iter().all(|f| f.source == "root" && !same_path(Path::new(&f.path), Path::new(&root))));
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn names_resolve_only_when_one_folder_fits() {
        let base = tree("resolve");
        let root = base.join("Work").display().to_string();
        let known = discover_with(&[], &[], &[], &[root], 50);
        let ai_remote = base.join("Work").join("ai-remote").display().to_string();
        assert_eq!(resolve("ai-remote", &known).as_deref(), Some(ai_remote.as_str()));
        assert_eq!(resolve(r"C:\Users\someone\projects\AI-Remote", &known).as_deref(), Some(ai_remote.as_str()));
        assert_eq!(resolve("ai_remote", &known).as_deref(), Some(ai_remote.as_str()));
        assert_eq!(resolve(&ai_remote, &known).as_deref(), Some(ai_remote.as_str()));
        assert_eq!(resolve("nothing-like-it", &known), None);
        let trailing = format!("{ai_remote}\\");
        assert_eq!(resolve(&trailing, &known).as_deref(), Some(ai_remote.as_str()));
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn tidy_drops_trailing_separators_but_keeps_drive_roots() {
        assert_eq!(tidy(r"C:\p\app\"), r"C:\p\app");
        assert_eq!(tidy("C:/p/app/"), "C:/p/app");
        assert_eq!(tidy(r"C:\"), r"C:\");
    }
}
