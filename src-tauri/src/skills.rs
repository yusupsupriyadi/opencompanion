//! User skill folders of the supported CLIs, compared by content for the Skills screen.
//! Read-only: the screen offers copy commands, OpenCompanion never writes to these folders.

use crate::cli::CliKind;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

const MAX_FILES: usize = 5_000;
const MAX_BYTES: u64 = 50 * 1024 * 1024;
/// Links are followed, so a link loop would recurse forever without a depth limit.
const MAX_DEPTH: usize = 16;
/// Tool folders, not skill content.
const SKIP_DIRS: [&str; 2] = [".git", "node_modules"];

/// Id, label, owning CLI and path under the home folder. Columns are folders, not CLIs:
/// which CLI reads which folder depends on its version, so the app does not guess.
const ROOTS: [(&str, &str, Option<CliKind>, &str); 8] = [
    ("claude", "Claude Code", Some(CliKind::Claude), ".claude/skills"),
    ("codex", "Codex CLI", Some(CliKind::Codex), ".codex/skills"),
    ("agents", "Shared", None, ".agents/skills"),
    ("opencode", "OpenCode", Some(CliKind::Opencode), ".config/opencode/skills"),
    ("gemini", "Gemini CLI", Some(CliKind::Gemini), ".gemini/skills"),
    ("pi", "Pi", Some(CliKind::Pi), ".pi/agent/skills"),
    ("omp", "omp", Some(CliKind::Omp), ".omp/agent/skills"),
    ("cursor", "Cursor CLI", Some(CliKind::Cursor), ".cursor/skills"),
];

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillRoot {
    pub id: String,
    pub label: String,
    pub cli: Option<CliKind>,
    pub path: PathBuf,
    pub exists: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Problem {
    NoSkillMd,
    BrokenLink,
    Unreadable,
    TooLarge,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillEntry {
    pub root_id: String,
    pub path: PathBuf,
    pub hash: Option<String>,
    /// "A" for the most recently changed content in the row, then "B", "C".
    pub variant: Option<String>,
    /// Newest file time in the folder, ms since the epoch.
    pub modified_at: Option<i64>,
    /// Set when the skill folder itself is a symlink or junction.
    pub link_target: Option<PathBuf>,
    /// A symlink or junction somewhere inside the folder.
    pub contains_links: bool,
    pub problem: Option<Problem>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillRow {
    pub name: String,
    pub description: Option<String>,
    /// Distinct contents among the readable copies.
    pub variants: usize,
    pub entries: Vec<SkillEntry>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillScan {
    /// Shell the copy commands are written for.
    pub shell: &'static str,
    pub roots: Vec<SkillRoot>,
    pub skills: Vec<SkillRow>,
}

pub fn scan() -> Result<SkillScan, String> {
    let home = std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
        .ok_or("Could not find your home folder: USERPROFILE and HOME are both unset.")?;
    Ok(scan_roots(default_roots(&home)))
}

pub fn default_roots(home: &Path) -> Vec<SkillRoot> {
    ROOTS
        .iter()
        .map(|&(id, label, cli, rel)| {
            let path = rel.split('/').fold(home.to_path_buf(), |p, part| p.join(part));
            SkillRoot { id: id.into(), label: label.into(), cli, exists: path.is_dir(), path }
        })
        .collect()
}

pub fn scan_roots(roots: Vec<SkillRoot>) -> SkillScan {
    // Keyed by lowercase name so `Foo` in one folder and `foo` in another share a row.
    let mut rows: BTreeMap<String, (String, Vec<Found>)> = BTreeMap::new();
    for root in roots.iter().filter(|r| r.exists) {
        for found in scan_root(root) {
            rows.entry(found.name.to_lowercase())
                .or_insert_with(|| (found.name.clone(), Vec::new()))
                .1
                .push(found);
        }
    }
    SkillScan {
        shell: if cfg!(windows) { "powershell" } else { "sh" },
        roots,
        skills: rows.into_values().map(|(name, found)| build_row(name, found)).collect(),
    }
}

struct Found {
    name: String,
    entry: SkillEntry,
    description: Option<String>,
}

fn scan_root(root: &SkillRoot) -> Vec<Found> {
    let Ok(items) = fs::read_dir(&root.path) else { return Vec::new() };
    let mut out = Vec::new();
    for item in items.flatten() {
        let name = item.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }
        let path = item.path();
        let Ok(own) = fs::symlink_metadata(&path) else { continue };
        let is_link = own.file_type().is_symlink();
        let mut entry = SkillEntry {
            root_id: root.id.clone(),
            path: path.clone(),
            hash: None,
            variant: None,
            modified_at: None,
            link_target: if is_link { fs::read_link(&path).ok().map(clean_link) } else { None },
            contains_links: false,
            problem: None,
        };
        let mut description = None;
        match fs::metadata(&path) {
            Err(_) if is_link => entry.problem = Some(Problem::BrokenLink),
            Err(_) => entry.problem = Some(Problem::Unreadable),
            Ok(m) if !m.is_dir() => continue,
            // Without the target the screen cannot tell the owner where the real folder is.
            Ok(_) if is_link && entry.link_target.is_none() => entry.problem = Some(Problem::Unreadable),
            Ok(_) => {
                let skill_md = path.join("SKILL.md");
                if !skill_md.is_file() {
                    entry.problem = Some(Problem::NoSkillMd);
                } else {
                    description = fs::read_to_string(&skill_md)
                        .ok()
                        .and_then(|text| frontmatter_field(&text, "description"));
                    match digest_dir(&path) {
                        Ok(d) => {
                            entry.hash = Some(d.hash);
                            entry.modified_at = d.modified;
                            entry.contains_links = d.contains_links;
                        }
                        Err(p) => entry.problem = Some(p),
                    }
                }
            }
        }
        out.push(Found { name, entry, description });
    }
    out
}

fn build_row(name: String, found: Vec<Found>) -> SkillRow {
    let mut groups: Vec<(String, Option<i64>)> = Vec::new();
    for f in &found {
        if let Some(hash) = &f.entry.hash {
            match groups.iter_mut().find(|g| &g.0 == hash) {
                Some(g) => g.1 = g.1.max(f.entry.modified_at),
                None => groups.push((hash.clone(), f.entry.modified_at)),
            }
        }
    }
    // Newest first; the hash only breaks ties so letters stay put between scans.
    groups.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    let newest = groups.first().map(|g| g.0.as_str());
    let description = found
        .iter()
        .find(|f| newest.is_some() && f.entry.hash.as_deref() == newest)
        .and_then(|f| f.description.clone())
        .or_else(|| found.iter().find_map(|f| f.description.clone()));
    let letter = |hash: &str| {
        groups
            .iter()
            .position(|g| g.0 == hash)
            .map(|i| char::from(b'A' + i as u8).to_string())
    };
    let entries = found
        .into_iter()
        .map(|f| {
            let mut e = f.entry;
            e.variant = e.hash.as_deref().and_then(letter);
            e
        })
        .collect();
    SkillRow { name, description, variants: groups.len(), entries }
}

struct FolderDigest {
    hash: String,
    modified: Option<i64>,
    contains_links: bool,
}

/// SHA-256 over every file's relative path, size and bytes, in path order.
fn digest_dir(dir: &Path) -> Result<FolderDigest, Problem> {
    let mut files = Vec::new();
    let mut contains_links = false;
    collect_files(dir, "", 0, &mut files, &mut contains_links)?;
    files.sort_by(|a, b| a.0.cmp(&b.0));
    let mut hasher = Sha256::new();
    let mut total = 0u64;
    let mut modified = None;
    let mut buf = vec![0u8; 64 * 1024];
    for (rel, path) in &files {
        let mut file = fs::File::open(path).map_err(|_| Problem::Unreadable)?;
        let meta = file.metadata().map_err(|_| Problem::Unreadable)?;
        total += meta.len();
        if total > MAX_BYTES {
            return Err(Problem::TooLarge);
        }
        let ms = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as i64);
        modified = modified.max(ms);
        hasher.update(rel.as_bytes());
        hasher.update([0]);
        hasher.update(meta.len().to_le_bytes());
        loop {
            let n = file.read(&mut buf).map_err(|_| Problem::Unreadable)?;
            if n == 0 {
                break;
            }
            hasher.update(&buf[..n]);
        }
    }
    let hash = hasher.finalize().iter().map(|b| format!("{b:02x}")).collect();
    Ok(FolderDigest { hash, modified, contains_links })
}

fn collect_files(
    dir: &Path,
    prefix: &str,
    depth: usize,
    out: &mut Vec<(String, PathBuf)>,
    links: &mut bool,
) -> Result<(), Problem> {
    if depth > MAX_DEPTH {
        return Err(Problem::TooLarge);
    }
    for item in fs::read_dir(dir).map_err(|_| Problem::Unreadable)? {
        let item = item.map_err(|_| Problem::Unreadable)?;
        let name = item.file_name().to_string_lossy().into_owned();
        let rel = if prefix.is_empty() { name.clone() } else { format!("{prefix}/{name}") };
        let path = item.path();
        *links |= item.file_type().is_ok_and(|t| t.is_symlink());
        let meta = fs::metadata(&path).map_err(|_| Problem::Unreadable)?;
        if meta.is_dir() {
            if !SKIP_DIRS.contains(&name.as_str()) {
                collect_files(&path, &rel, depth + 1, out, links)?;
            }
        } else {
            out.push((rel, path));
            if out.len() > MAX_FILES {
                return Err(Problem::TooLarge);
            }
        }
    }
    Ok(())
}

/// Windows reports junction targets with a `\\?\` or `\??\` prefix; people know the plain path.
fn clean_link(target: PathBuf) -> PathBuf {
    let s = target.to_string_lossy();
    match s.strip_prefix(r"\\?\").or_else(|| s.strip_prefix(r"\??\")) {
        Some(rest) => PathBuf::from(rest),
        None => target.clone(),
    }
}

/// One top-level field of a SKILL.md frontmatter: plain, quoted, or a `>`/`|` block.
/// Enough for `description`; not a YAML parser.
fn frontmatter_field(text: &str, key: &str) -> Option<String> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut lines = text.lines();
    if lines.next()?.trim_end() != "---" {
        return None;
    }
    let body: Vec<&str> = lines.take_while(|l| l.trim_end() != "---").collect();
    let prefix = format!("{key}:");
    let start = body.iter().position(|l| l.starts_with(&prefix))?;
    let first = body[start][prefix.len()..].trim();
    // Continuation lines are indented; the next key starts at column 0.
    let more: Vec<&str> = body[start + 1..]
        .iter()
        .take_while(|l| l.trim().is_empty() || l.starts_with(char::is_whitespace))
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect();
    let value = if first.starts_with(['>', '|']) {
        more.join(" ")
    } else {
        let parts: Vec<&str> = std::iter::once(first).chain(more).filter(|s| !s.is_empty()).collect();
        unquote(&parts.join(" "))
    };
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_string())
}

fn unquote(s: &str) -> String {
    let inner = |q: char| s.len() >= 2 && s.starts_with(q) && s.ends_with(q);
    if inner('"') {
        s[1..s.len() - 1].replace("\\\"", "\"").replace("\\\\", "\\")
    } else if inner('\'') {
        s[1..s.len() - 1].replace("''", "'")
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, SystemTime};

    fn base(name: &str) -> PathBuf {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join(format!("air-skills-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write(path: &Path, text: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    fn touch(path: &Path, secs_ago: u64) {
        let at = SystemTime::now() - Duration::from_secs(secs_ago);
        fs::File::options().write(true).open(path).unwrap().set_modified(at).unwrap();
    }

    fn root(base: &Path, id: &str) -> SkillRoot {
        let path = base.join(id);
        SkillRoot { id: id.into(), label: id.into(), cli: None, exists: path.is_dir(), path }
    }

    fn skill(text: &str) -> String {
        format!("---\nname: x\ndescription: {text}\n---\n# Body\n")
    }

    #[test]
    fn frontmatter_reads_inline_quoted_and_block_values() {
        let field = |t: &str| frontmatter_field(t, "description");
        assert_eq!(field("---\ndescription: Plain words\n---\n").as_deref(), Some("Plain words"));
        assert_eq!(field("\u{feff}---\ndescription: \"Say \\\"hi\\\"\"\n---\n").as_deref(), Some("Say \"hi\""));
        assert_eq!(field("---\ndescription: 'It''s fine'\n---\n").as_deref(), Some("It's fine"));
        assert_eq!(field("---\ndescription: >\n  Folded\n  lines\nname: x\n---\n").as_deref(), Some("Folded lines"));
        assert_eq!(field("---\ndescription: |-\n  One\n\n  Two\n---\n").as_deref(), Some("One Two"));
        assert_eq!(field("---\ndescription: Starts here\n  and goes on\nmetadata:\n  a: b\n---\n").as_deref(), Some("Starts here and goes on"));
        assert_eq!(field("# No frontmatter\ndescription: nope\n"), None);
        assert_eq!(field("---\nname: only\n---\ndescription: after the block\n"), None);
    }

    #[test]
    fn digest_is_stable_and_follows_names_and_contents() {
        let dir = base("digest");
        let s = dir.join("s");
        write(&s.join("SKILL.md"), &skill("d"));
        write(&s.join("ref").join("a.md"), "one");
        let first = digest_dir(&s).unwrap().hash;
        assert_eq!(digest_dir(&s).unwrap().hash, first);
        fs::rename(s.join("ref").join("a.md"), s.join("ref").join("b.md")).unwrap();
        assert_ne!(digest_dir(&s).unwrap().hash, first);
        fs::rename(s.join("ref").join("b.md"), s.join("ref").join("a.md")).unwrap();
        write(&s.join("ref").join("a.md"), "two");
        assert_ne!(digest_dir(&s).unwrap().hash, first);
        write(&s.join("ref").join("a.md"), "one");
        write(&s.join(".git").join("HEAD"), "ref: main");
        write(&s.join("node_modules").join("x.js"), "x");
        assert_eq!(digest_dir(&s).unwrap().hash, first);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn rows_merge_folders_and_letter_versions_newest_first() {
        let dir = base("rows");
        let (a, b) = (dir.join("a"), dir.join("b"));
        write(&a.join("alpha").join("SKILL.md"), &skill("Alpha"));
        write(&b.join("alpha").join("SKILL.md"), &skill("Alpha"));
        write(&a.join("beta").join("SKILL.md"), &skill("Old beta"));
        write(&b.join("beta").join("SKILL.md"), &skill("New beta"));
        touch(&a.join("beta").join("SKILL.md"), 3600);
        touch(&b.join("beta").join("SKILL.md"), 60);
        write(&a.join("Gamma").join("SKILL.md"), &skill("G"));
        write(&b.join("gamma").join("SKILL.md"), &skill("G"));
        fs::create_dir_all(a.join("empty")).unwrap();
        write(&a.join(".system").join("hidden").join("SKILL.md"), &skill("h"));
        write(&a.join("loose.md"), "not a skill");

        let scan = scan_roots(vec![root(&dir, "a"), root(&dir, "b"), root(&dir, "missing")]);
        assert!(!scan.roots[2].exists);
        let names: Vec<&str> = scan.skills.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names, ["alpha", "beta", "empty", "Gamma"]);

        let alpha = &scan.skills[0];
        assert_eq!(alpha.variants, 1);
        assert!(alpha.entries.iter().all(|e| e.variant.as_deref() == Some("A")));
        assert_eq!(alpha.description.as_deref(), Some("Alpha"));

        let beta = &scan.skills[1];
        assert_eq!(beta.variants, 2);
        let variant = |id: &str| beta.entries.iter().find(|e| e.root_id == id).unwrap().variant.clone();
        assert_eq!(variant("b").as_deref(), Some("A"));
        assert_eq!(variant("a").as_deref(), Some("B"));
        assert_eq!(beta.description.as_deref(), Some("New beta"));

        let empty = &scan.skills[2];
        assert_eq!(empty.entries[0].problem, Some(Problem::NoSkillMd));
        assert_eq!(empty.variants, 0);
        assert_eq!(scan.skills[3].entries.len(), 2);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn default_roots_sit_under_home_with_native_separators() {
        let home = PathBuf::from("home");
        let roots = default_roots(&home);
        let ids: Vec<&str> = roots.iter().map(|r| r.id.as_str()).collect();
        assert_eq!(ids, ["claude", "codex", "agents", "opencode", "gemini", "pi", "omp", "cursor"]);
        assert_eq!(roots[7].path, home.join(".cursor").join("skills"));
        assert_eq!(roots[3].path, home.join(".config").join("opencode").join("skills"));
        assert_eq!(roots[2].cli, None);
    }

    #[cfg(windows)]
    #[test]
    fn junctions_report_their_target_and_break_cleanly() {
        let dir = base("junction");
        let target = dir.join("real").join("neo");
        write(&target.join("SKILL.md"), &skill("Neo"));
        let a = dir.join("a");
        fs::create_dir_all(&a).unwrap();
        let junction = |link: PathBuf, to: &Path| {
            let ok = std::process::Command::new("cmd")
                .args(["/C", "mklink", "/J"])
                .arg(link)
                .arg(to)
                .output()
                .unwrap()
                .status
                .success();
            assert!(ok, "mklink /J failed");
        };
        junction(a.join("neo"), &target);
        junction(a.join("gone"), &dir.join("real").join("missing"));
        write(&a.join("outer").join("SKILL.md"), &skill("Outer"));
        junction(a.join("outer").join("shared"), &dir.join("real"));

        let scan = scan_roots(vec![root(&dir, "a")]);
        let entry = |name: &str| &scan.skills.iter().find(|r| r.name == name).unwrap().entries[0];
        let neo = entry("neo");
        assert_eq!(neo.link_target.as_deref(), Some(target.as_path()));
        assert!(neo.hash.is_some() && neo.problem.is_none() && !neo.contains_links);
        assert_eq!(entry("gone").problem, Some(Problem::BrokenLink));
        let outer = entry("outer");
        assert!(outer.contains_links && outer.link_target.is_none() && outer.hash.is_some());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn junction_prefixes_are_dropped() {
        assert_eq!(clean_link(PathBuf::from(r"\\?\C:\x\y")), PathBuf::from(r"C:\x\y"));
        assert_eq!(clean_link(PathBuf::from(r"\??\C:\x")), PathBuf::from(r"C:\x"));
        assert_eq!(clean_link(PathBuf::from("/opt/x")), PathBuf::from("/opt/x"));
    }
}
