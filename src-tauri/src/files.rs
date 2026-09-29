//! A session's folder as the side panel shows it: one folder at a time, a search by name or by
//! text, and one file's text. Paths are relative to the session folder, with `/` between parts,
//! and never lead out of it.

use std::cmp::Ordering;
use std::path::{Component, Path, PathBuf};
use std::time::{Duration, Instant};

use serde::Serialize;

use crate::git;

/// Entries listed for one folder; `node_modules` can hold tens of thousands.
const DIR_LIMIT: usize = 5000;
/// Files walked for a search in a folder that is not a git repository.
const WALK_LIMIT: usize = 20_000;
const NAME_MATCHES: usize = 200;
const TEXT_MATCHES: usize = 300;
const TEXT_BUDGET: Duration = Duration::from_secs(4);
/// Larger files are neither shown nor searched.
pub const READ_LIMIT: u64 = 1024 * 1024;

pub const OUTSIDE: &str = "That path is outside this session's folder.";
pub const GONE: &str = "This file no longer exists.";

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub name: String,
    pub path: String,
    pub dir: bool,
    /// Matched by .gitignore (or another exclude file), for a folder inside a git repository.
    pub ignored: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Listing {
    pub entries: Vec<Entry>,
    /// The folder holds more than `DIR_LIMIT` entries; only the first ones are listed.
    pub truncated: bool,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Match {
    pub path: String,
    /// The line of a text match, from 1; none for a name match.
    pub line: Option<u32>,
    pub text: Option<String>,
}

#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Found {
    pub matches: Vec<Match>,
    /// The search stopped early (match limit, time limit, or a folder too large to walk).
    pub truncated: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileText {
    pub text: Option<String>,
    pub size: u64,
    pub binary: bool,
    pub too_large: bool,
}

/// `rel` as a clean relative path, or an error when it is absolute or climbs out with `..`.
pub fn clean(rel: &str) -> Result<String, String> {
    let mut parts = Vec::new();
    for part in rel.split(['/', '\\']).filter(|p| !p.is_empty() && *p != ".") {
        // A drive or root prefix parses as something other than a normal component.
        if part == ".." || !matches!(Path::new(part).components().next(), Some(Component::Normal(_))) {
            return Err(OUTSIDE.into());
        }
        parts.push(part);
    }
    Ok(parts.join("/"))
}

/// The existing file or folder at `rel` inside `root`. A link inside the folder that leads out
/// of it is refused like `..`.
pub fn inside(root: &Path, rel: &str) -> Result<PathBuf, String> {
    let rel = clean(rel)?;
    let path = rel.split('/').filter(|p| !p.is_empty()).fold(root.to_path_buf(), |acc, p| acc.join(p));
    let real_root = root.canonicalize().map_err(|_| format!("The folder {} does not exist.", root.display()))?;
    let real = path.canonicalize().map_err(|_| GONE.to_string())?;
    if real.starts_with(&real_root) {
        Ok(path)
    } else {
        Err(OUTSIDE.into())
    }
}

fn join(dir: &str, name: &str) -> String {
    if dir.is_empty() {
        name.to_string()
    } else {
        format!("{dir}/{name}")
    }
}

/// Case-insensitive order with digit runs compared as numbers: `9`, `10`, `100`.
pub fn natural(a: &str, b: &str) -> Ordering {
    let (mut x, mut y) = (a.chars().peekable(), b.chars().peekable());
    loop {
        match (x.peek().copied(), y.peek().copied()) {
            (None, None) => return a.cmp(b),
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(c), Some(d)) if c.is_ascii_digit() && d.is_ascii_digit() => {
                let n: String = std::iter::from_fn(|| x.next_if(char::is_ascii_digit)).collect();
                let m: String = std::iter::from_fn(|| y.next_if(char::is_ascii_digit)).collect();
                let (n, m) = (n.trim_start_matches('0'), m.trim_start_matches('0'));
                let ord = n.len().cmp(&m.len()).then_with(|| n.cmp(m));
                if ord != Ordering::Equal {
                    return ord;
                }
            }
            (Some(c), Some(d)) => {
                let ord = c.to_lowercase().cmp(d.to_lowercase());
                if ord != Ordering::Equal {
                    return ord;
                }
                x.next();
                y.next();
            }
        }
    }
}

/// One folder of the tree: folders first, then files, `.git` left out.
pub fn list(root: &Path, dir: &str) -> Result<Listing, String> {
    let dir = clean(dir)?;
    let full = if dir.is_empty() { root.to_path_buf() } else { inside(root, &dir)? };
    let read = std::fs::read_dir(&full).map_err(|e| format!("Could not read this folder: {e}"))?;
    let mut entries: Vec<Entry> = read
        .flatten()
        .filter_map(|item| {
            let name = item.file_name().to_string_lossy().into_owned();
            if name == ".git" {
                return None;
            }
            // A link to a folder opens like a folder; `inside` still keeps it in the session folder.
            let is_dir = item.file_type().map(|t| t.is_dir() || (t.is_symlink() && item.path().is_dir())).unwrap_or(false);
            Some(Entry { path: join(&dir, &name), name, dir: is_dir, ignored: false })
        })
        .collect();
    entries.sort_by(|a, b| b.dir.cmp(&a.dir).then_with(|| natural(&a.name, &b.name)));
    let truncated = entries.len() > DIR_LIMIT;
    entries.truncate(DIR_LIMIT);
    let ignored = git::ignored(root, entries.iter().map(|e| e.path.as_str()));
    for e in &mut entries {
        e.ignored = ignored.contains(&e.path);
    }
    Ok(Listing { entries, truncated })
}

/// The files a search looks through: what git would list (tracked and untracked, not ignored),
/// or at most `WALK_LIMIT` files outside a repository. True when the walk stopped at the limit.
fn all_files(root: &Path) -> (Vec<String>, bool) {
    if let Some(files) = git::files(root) {
        return (files, false);
    }
    let mut out = Vec::new();
    let mut stack = vec![(root.to_path_buf(), String::new())];
    while let Some((dir, rel)) = stack.pop() {
        let Ok(read) = std::fs::read_dir(&dir) else { continue };
        for item in read.flatten() {
            let Ok(kind) = item.file_type() else { continue };
            let name = item.file_name().to_string_lossy().into_owned();
            // Links are not followed, so a walk cannot leave the folder or loop.
            if kind.is_dir() && name != ".git" {
                stack.push((item.path(), join(&rel, &name)));
            } else if kind.is_file() {
                out.push(join(&rel, &name));
                if out.len() >= WALK_LIMIT {
                    return (out, true);
                }
            }
        }
    }
    out.sort();
    (out, false)
}

/// Files whose path holds every word of `query`, ignoring case; the ones whose name holds them
/// come first, then shorter paths.
pub fn find_names(root: &Path, query: &str) -> Found {
    let words: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
    if words.is_empty() {
        return Found::default();
    }
    let (files, capped) = all_files(root);
    let mut hits: Vec<(u8, usize, String)> = files
        .into_iter()
        .filter_map(|path| {
            let lower = path.to_lowercase();
            if !words.iter().all(|w| lower.contains(w.as_str())) {
                return None;
            }
            let name = lower.rsplit('/').next().unwrap_or(&lower);
            let rank = u8::from(!words.iter().all(|w| name.contains(w.as_str())));
            Some((rank, path.len(), path))
        })
        .collect();
    hits.sort();
    let truncated = capped || hits.len() > NAME_MATCHES;
    hits.truncate(NAME_MATCHES);
    Found {
        matches: hits.into_iter().map(|(_, _, path)| Match { path, line: None, text: None }).collect(),
        truncated,
    }
}

/// Lines that hold `query` as plain text, ignoring case, in text files up to `READ_LIMIT`.
pub fn find_text(root: &Path, query: &str) -> Found {
    let needle = query.trim().to_lowercase();
    if needle.is_empty() {
        return Found::default();
    }
    let started = Instant::now();
    let (files, mut truncated) = all_files(root);
    let mut matches = Vec::new();
    'files: for path in files {
        if started.elapsed() > TEXT_BUDGET {
            truncated = true;
            break;
        }
        let full = root.join(&path);
        // Not through links, which could point out of the folder.
        let Ok(meta) = std::fs::symlink_metadata(&full) else { continue };
        if !meta.is_file() || meta.len() > READ_LIMIT {
            continue;
        }
        let Ok(bytes) = std::fs::read(&full) else { continue };
        if is_binary(&bytes) {
            continue;
        }
        for (i, line) in String::from_utf8_lossy(&bytes).lines().enumerate() {
            if line.to_lowercase().contains(&needle) {
                matches.push(Match { path: path.clone(), line: Some(i as u32 + 1), text: Some(snippet(line, &needle)) });
                if matches.len() >= TEXT_MATCHES {
                    truncated = true;
                    break 'files;
                }
            }
        }
    }
    Found { matches, truncated }
}

/// A matched line, trimmed; a long one is cut to 200 characters around the match.
fn snippet(line: &str, needle: &str) -> String {
    const WIDTH: usize = 200;
    let line = line.trim();
    let chars: Vec<char> = line.chars().collect();
    if chars.len() <= WIDTH {
        return line.to_string();
    }
    let lower = line.to_lowercase();
    let at = lower.find(needle).map(|b| lower[..b].chars().count()).unwrap_or(0);
    let start = at.saturating_sub(60).min(chars.len());
    let end = (start + WIDTH).min(chars.len());
    let mut out: String = chars[start..end].iter().collect();
    if start > 0 {
        out.insert(0, '…');
    }
    if end < chars.len() {
        out.push('…');
    }
    out
}

/// A NUL byte in the first 8000, the same test git uses.
pub fn is_binary(bytes: &[u8]) -> bool {
    bytes.iter().take(8000).any(|b| *b == 0)
}

pub fn read(root: &Path, rel: &str) -> Result<FileText, String> {
    let path = inside(root, rel)?;
    let meta = std::fs::metadata(&path).map_err(|_| GONE.to_string())?;
    if meta.is_dir() {
        return Err("This is a folder.".into());
    }
    let size = meta.len();
    if size > READ_LIMIT {
        return Ok(FileText { text: None, size, binary: false, too_large: true });
    }
    let bytes = std::fs::read(&path).map_err(|e| format!("Could not read this file: {e}"))?;
    if is_binary(&bytes) {
        return Ok(FileText { text: None, size, binary: true, too_large: false });
    }
    let text = String::from_utf8_lossy(&bytes);
    let text = text.strip_prefix('\u{feff}').unwrap_or(&text).to_string();
    Ok(FileText { text: Some(text), size, binary: false, too_large: false })
}

/// Images and PDFs are shown from their bytes, and are often larger than a text file may be.
pub const MEDIA_LIMIT: u64 = 50 * 1024 * 1024;
/// The viewer knows this one and shows it as a state rather than as a failure.
pub const MEDIA_TOO_LARGE: &str = "This file is larger than 50 MB.";

/// A file's bytes, for the viewer to draw as a picture or as pages.
pub fn read_bytes(root: &Path, rel: &str) -> Result<Vec<u8>, String> {
    let path = inside(root, rel)?;
    let meta = std::fs::metadata(&path).map_err(|_| GONE.to_string())?;
    if meta.is_dir() {
        return Err("This is a folder.".into());
    }
    if meta.len() > MEDIA_LIMIT {
        return Err(MEDIA_TOO_LARGE.into());
    }
    std::fs::read(&path).map_err(|e| format!("Could not read this file: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("oc-files-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn natural_order_ignores_case_and_reads_numbers() {
        let mut names = vec!["b10", "B9", "a", ".gitignore", "CHANGELOG.md", "bun.lock", "b100", "b09x"];
        names.sort_by(|a, b| natural(a, b));
        assert_eq!(names, vec![".gitignore", "a", "B9", "b09x", "b10", "b100", "bun.lock", "CHANGELOG.md"]);
    }

    #[test]
    fn paths_that_climb_out_or_are_absolute_are_refused() {
        assert_eq!(clean("src\\lib/./api.ts").unwrap(), "src/lib/api.ts");
        assert_eq!(clean("").unwrap(), "");
        assert!(clean("../secret").is_err());
        assert!(clean("src/../../x").is_err());
        #[cfg(windows)]
        assert!(clean("C:\\Windows").is_err());
        #[cfg(windows)]
        assert!(clean("C:x").is_err());
    }

    #[test]
    fn a_folder_lists_folders_first_without_git() {
        let root = scratch("list");
        std::fs::create_dir_all(root.join(".git")).unwrap();
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(root.join("b.txt"), "b").unwrap();
        std::fs::write(root.join("A.txt"), "a").unwrap();
        std::fs::write(root.join("src/main.rs"), "fn main() {}").unwrap();

        let top = list(&root, "").unwrap();
        let names: Vec<_> = top.entries.iter().map(|e| (e.name.as_str(), e.dir)).collect();
        assert_eq!(names, vec![("src", true), ("A.txt", false), ("b.txt", false)]);
        let sub = list(&root, "src").unwrap();
        assert_eq!(sub.entries[0].path, "src/main.rs");
        assert!(list(&root, "..").is_err());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn names_match_every_word_and_prefer_the_file_name() {
        let root = scratch("names");
        std::fs::create_dir_all(root.join("api/docs")).unwrap();
        std::fs::write(root.join("api/docs/readme.md"), "").unwrap();
        std::fs::write(root.join("api.ts"), "").unwrap();
        std::fs::write(root.join("notes.md"), "").unwrap();

        let found: Vec<_> = find_names(&root, "API").matches.into_iter().map(|m| m.path).collect();
        assert_eq!(found, vec!["api.ts", "api/docs/readme.md"]);
        let found: Vec<_> = find_names(&root, "api md").matches.into_iter().map(|m| m.path).collect();
        assert_eq!(found, vec!["api/docs/readme.md"]);
        assert!(find_names(&root, "   ").matches.is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn text_search_finds_lines_and_skips_binary_files() {
        let root = scratch("text");
        std::fs::write(root.join("a.txt"), "one\nTwo words\nthree two\n").unwrap();
        std::fs::write(root.join("b.bin"), b"two\0\0").unwrap();

        let found = find_text(&root, "two");
        let hits: Vec<_> = found.matches.iter().map(|m| (m.path.as_str(), m.line, m.text.as_deref())).collect();
        assert_eq!(hits, vec![("a.txt", Some(2), Some("Two words")), ("a.txt", Some(3), Some("three two"))]);
        assert!(!found.truncated);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_long_line_is_cut_around_the_match() {
        let line = format!("{}needle{}", "a".repeat(300), "b".repeat(300));
        let s = snippet(&line, "needle");
        assert!(s.starts_with('…') && s.ends_with('…'));
        assert!(s.contains("needle"));
        assert!(s.chars().count() <= 202);
    }

    #[test]
    fn reading_says_when_a_file_is_binary_or_too_large() {
        let root = scratch("read");
        std::fs::write(root.join("t.txt"), "\u{feff}hello").unwrap();
        std::fs::write(root.join("b.png"), b"\x89PNG\0\0").unwrap();
        std::fs::write(root.join("big.txt"), vec![b'a'; READ_LIMIT as usize + 1]).unwrap();

        assert_eq!(read(&root, "t.txt").unwrap().text.as_deref(), Some("hello"));
        assert!(read(&root, "b.png").unwrap().binary);
        let big = read(&root, "big.txt").unwrap();
        assert!(big.too_large && big.text.is_none());
        assert_eq!(read(&root, "gone.txt").unwrap_err(), GONE);
        assert_eq!(read(&root, "../t.txt").unwrap_err(), OUTSIDE);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn bytes_are_read_for_pictures_within_the_folder_and_the_media_limit() {
        let root = scratch("bytes");
        std::fs::write(root.join("b.png"), b"\x89PNG\0\0").unwrap();
        let big = std::fs::File::create(root.join("big.pdf")).unwrap();
        big.set_len(MEDIA_LIMIT + 1).unwrap();
        drop(big);
        std::fs::create_dir(root.join("dir")).unwrap();

        assert_eq!(read_bytes(&root, "b.png").unwrap(), b"\x89PNG\0\0");
        assert_eq!(read_bytes(&root, "big.pdf").unwrap_err(), MEDIA_TOO_LARGE);
        assert_eq!(read_bytes(&root, "dir").unwrap_err(), "This is a folder.");
        assert_eq!(read_bytes(&root, "gone.png").unwrap_err(), GONE);
        assert_eq!(read_bytes(&root, "../b.png").unwrap_err(), OUTSIDE);
        let _ = std::fs::remove_dir_all(&root);
    }
}
