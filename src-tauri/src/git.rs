//! Git for a session's folder: status, one file's diff against the last commit, branches and
//! recent commits, and a branch switch. Runs the `git` on the PATH (the login shell's on macOS and
//! Linux). Reads pass `GIT_OPTIONAL_LOCKS=0`, so they never take the index lock that a CLI's own
//! git command in the same folder may need.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Serialize;

use crate::files;
use crate::proc;

const TIMEOUT: Duration = Duration::from_secs(20);
/// Changes listed at most; an unignored `node_modules` would otherwise list every file in it.
const CHANGE_LIMIT: usize = 2000;
/// Untracked files whose lines are counted, each up to `COUNT_LIMIT` bytes.
const COUNTED_UNTRACKED: usize = 300;
const COUNT_LIMIT: u64 = 512 * 1024;
const DIFF_LIMIT: usize = 2 * 1024 * 1024;
const BRANCH_LIMIT: &str = "--count=200";
const COMMIT_LIMIT: &str = "-n30";

pub const NOT_INSTALLED: &str = "Git is not installed or not on PATH.";

#[derive(Debug, Serialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Change {
    pub path: String,
    /// Where a renamed or copied file came from.
    pub old_path: Option<String>,
    /// M, A, D, R, C, U (a merge conflict) or ? (untracked).
    pub code: String,
    pub staged: bool,
    pub unstaged: bool,
    /// Lines added and removed against HEAD; none for a binary file or one too large to count.
    pub added: Option<u32>,
    pub removed: Option<u32>,
}

#[derive(Debug, Serialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    /// False when the folder is not in a git repository; the rest is then empty.
    pub repo: bool,
    /// None on a detached HEAD.
    pub branch: Option<String>,
    /// The short commit HEAD points at; none before the first commit.
    pub head: Option<String>,
    pub upstream: Option<String>,
    pub ahead: u32,
    pub behind: u32,
    /// Only what lies inside the session folder, relative to it.
    pub changes: Vec<Change>,
    pub truncated: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Diff {
    pub patch: String,
    pub binary: bool,
    pub too_large: bool,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Branch {
    pub name: String,
    pub current: bool,
    pub upstream: Option<String>,
    pub ahead: u32,
    pub behind: u32,
    /// The upstream branch was deleted on the remote.
    pub gone: bool,
    pub subject: String,
    /// Last commit, in ms.
    pub at: i64,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Commit {
    pub hash: String,
    pub subject: String,
    pub author: String,
    pub at: i64,
}

#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Branches {
    pub repo: bool,
    pub branches: Vec<Branch>,
    pub commits: Vec<Commit>,
}

fn exe() -> Option<PathBuf> {
    let search = match crate::shell_env::path() {
        Some(p) => p.into(),
        None => std::env::var_os("PATH")?,
    };
    which::which_in("git", Some(search), std::env::current_dir().ok()?).ok()
}

struct Out {
    ok: bool,
    code: Option<i32>,
    stdout: String,
    stderr: String,
}

impl Out {
    /// Git's own words for a failure, first lines only.
    fn error(&self) -> String {
        let text: Vec<&str> = self.stderr.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with("warning:")).take(3).collect();
        if text.is_empty() {
            format!("Git failed with code {}.", self.code.unwrap_or(-1))
        } else {
            text.join(" ")
        }
    }
}

fn run(root: &Path, args: &[&str], input: Option<&str>) -> Result<Out, String> {
    let exe = exe().ok_or(NOT_INSTALLED)?;
    let mut cmd = proc::hidden(&exe);
    cmd.current_dir(root)
        .args(["-c", "core.quotepath=false", "-c", "color.ui=false"])
        .args(args)
        .envs(crate::shell_env::vars().iter().map(|(k, v)| (k, v)))
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("GIT_TERMINAL_PROMPT", "0")
        // English messages, like the rest of the backend; the screen translates what it knows.
        .env("LC_ALL", "C");
    let out = proc::run_with_input(cmd, input, TIMEOUT).map_err(|e| format!("Could not run git: {e}"))?;
    if out.timed_out {
        return Err("Git took too long to answer.".into());
    }
    Ok(Out { ok: out.status == Some(0), code: out.status, stdout: out.stdout, stderr: out.stderr })
}

/// The session folder's place in its repository (`""` at the top, `packages/web/` below it), or
/// none when the folder is not in a repository.
fn prefix(root: &Path) -> Result<Option<String>, String> {
    let out = run(root, &["rev-parse", "--show-prefix"], None)?;
    if out.ok {
        return Ok(Some(out.stdout.trim().to_string()));
    }
    if out.stderr.contains("not a git repository") {
        return Ok(None);
    }
    Err(out.error())
}

/// Entries of `paths` (relative to `root`) that git ignores. Empty outside a repository or
/// without git: the tree then shows nothing as ignored.
pub fn ignored<'a>(root: &Path, paths: impl Iterator<Item = &'a str>) -> HashSet<String> {
    let input: String = paths.flat_map(|p| [p, "\0"]).collect();
    if input.is_empty() {
        return HashSet::new();
    }
    match run(root, &["check-ignore", "-z", "--stdin"], Some(&input)) {
        // Exit code 1 means none is ignored; 128 means no repository.
        Ok(out) if out.code == Some(0) => out.stdout.split('\0').filter(|p| !p.is_empty()).map(str::to_owned).collect(),
        _ => HashSet::new(),
    }
}

/// Tracked and untracked files that are not ignored, relative to `root`; none outside a repository.
pub fn files(root: &Path) -> Option<Vec<String>> {
    let out = run(root, &["ls-files", "-z", "--cached", "--others", "--exclude-standard"], None).ok()?;
    if !out.ok {
        return None;
    }
    let mut seen = HashSet::new();
    Some(
        out.stdout
            .split('\0')
            .filter(|p| !p.is_empty() && seen.insert(*p))
            .map(str::to_owned)
            .collect(),
    )
}

fn empty_tree(root: &Path) -> Result<String, String> {
    let out = run(root, &["hash-object", "-t", "tree", "--stdin"], Some(""))?;
    if out.ok {
        Ok(out.stdout.trim().to_string())
    } else {
        Err(out.error())
    }
}

/// What the diff compares against: HEAD, or the empty tree before the first commit.
fn base(root: &Path, status: &Status) -> Result<String, String> {
    match status.head {
        Some(_) => Ok("HEAD".into()),
        None => empty_tree(root),
    }
}

/// `git status --porcelain=v2 -z --branch` output. Paths there are relative to the repository;
/// `prefix` (the session folder's place in it) is cut off.
pub fn parse_status(raw: &str, prefix: &str) -> Status {
    let mut st = Status { repo: true, ..Status::default() };
    let rel = |p: &str| p.strip_prefix(prefix).unwrap_or(p).to_string();
    let mut fields = raw.split('\0');
    while let Some(rec) = fields.next() {
        if let Some(header) = rec.strip_prefix("# ") {
            if let Some(v) = header.strip_prefix("branch.oid ") {
                st.head = (v != "(initial)").then(|| v.chars().take(7).collect());
            } else if let Some(v) = header.strip_prefix("branch.head ") {
                st.branch = (v != "(detached)").then(|| v.to_string());
            } else if let Some(v) = header.strip_prefix("branch.upstream ") {
                st.upstream = Some(v.to_string());
            } else if let Some(v) = header.strip_prefix("branch.ab ") {
                for part in v.split(' ') {
                    if let Some(n) = part.strip_prefix('+') {
                        st.ahead = n.parse().unwrap_or(0);
                    } else if let Some(n) = part.strip_prefix('-') {
                        st.behind = n.parse().unwrap_or(0);
                    }
                }
            }
            continue;
        }
        let (xy, path, old_path, code) = match rec.as_bytes().first() {
            Some(b'1') => {
                let parts: Vec<&str> = rec.splitn(9, ' ').collect();
                if parts.len() < 9 || parts[1].len() < 2 {
                    continue;
                }
                let (x, y) = (parts[1].as_bytes()[0], parts[1].as_bytes()[1]);
                let code = if x == b'A' {
                    "A"
                } else if x == b'D' || y == b'D' {
                    "D"
                } else {
                    "M"
                };
                (parts[1], parts[8], None, code)
            }
            Some(b'2') => {
                let parts: Vec<&str> = rec.splitn(10, ' ').collect();
                if parts.len() < 10 {
                    continue;
                }
                let old = fields.next().unwrap_or_default();
                let code = if parts[8].starts_with('C') { "C" } else { "R" };
                (parts[1], parts[9], Some(rel(old)), code)
            }
            Some(b'u') => {
                let parts: Vec<&str> = rec.splitn(11, ' ').collect();
                if parts.len() < 11 {
                    continue;
                }
                (parts[1], parts[10], None, "U")
            }
            Some(b'?') => ("?.", rec.get(2..).unwrap_or_default(), None, "?"),
            _ => continue,
        };
        if xy.len() < 2 {
            continue;
        }
        st.changes.push(Change {
            path: rel(path),
            old_path,
            code: code.to_string(),
            staged: code != "?" && xy.as_bytes()[0] != b'.',
            unstaged: code == "?" || xy.as_bytes()[1] != b'.',
            added: None,
            removed: None,
        });
    }
    st
}

/// `git diff --numstat -z` output: lines added and removed per path (the new path of a rename).
/// A binary file counts as none.
pub fn parse_numstat(raw: &str) -> HashMap<String, (Option<u32>, Option<u32>)> {
    let mut counts = HashMap::new();
    let mut fields = raw.split('\0');
    while let Some(field) = fields.next() {
        let field = field.trim_start_matches('\n');
        if field.is_empty() {
            continue;
        }
        let mut parts = field.splitn(3, '\t');
        let (added, removed, path) = (parts.next().unwrap_or(""), parts.next().unwrap_or(""), parts.next().unwrap_or(""));
        let path = if path.is_empty() {
            // A rename: the old and the new path follow as fields of their own.
            let _old = fields.next();
            fields.next().unwrap_or("")
        } else {
            path
        };
        counts.insert(path.to_string(), (added.parse().ok(), removed.parse().ok()));
    }
    counts
}

/// Lines in an untracked text file, or none for a binary or large one.
fn count_lines(path: &Path) -> Option<u32> {
    let meta = std::fs::symlink_metadata(path).ok()?;
    if !meta.is_file() || meta.len() > COUNT_LIMIT {
        return None;
    }
    let bytes = std::fs::read(path).ok()?;
    if files::is_binary(&bytes) {
        return None;
    }
    let newlines = bytes.iter().filter(|b| **b == b'\n').count();
    let open_last = bytes.last().is_some_and(|b| *b != b'\n');
    Some((newlines + usize::from(open_last)) as u32)
}

/// Uncommitted changes in the session folder against HEAD: staged, unstaged and untracked.
pub fn status(root: &Path) -> Result<Status, String> {
    let Some(prefix) = prefix(root)? else { return Ok(Status::default()) };
    let out = run(root, &["status", "--porcelain=v2", "-z", "--branch", "--untracked-files=all", "--", "."], None)?;
    if !out.ok {
        return Err(out.error());
    }
    let mut st = parse_status(&out.stdout, &prefix);
    st.truncated = st.changes.len() > CHANGE_LIMIT;
    st.changes.truncate(CHANGE_LIMIT);
    if st.changes.is_empty() {
        return Ok(st);
    }
    let base = base(root, &st)?;
    let num = run(root, &["diff", &base, "--numstat", "-z", "-M", "--relative", "--no-ext-diff", "--", "."], None)?;
    let counts = if num.ok { parse_numstat(&num.stdout) } else { HashMap::new() };
    let mut counted = 0;
    for c in &mut st.changes {
        if c.code == "?" {
            if counted < COUNTED_UNTRACKED {
                counted += 1;
                c.added = count_lines(&root.join(&c.path));
                c.removed = c.added.map(|_| 0);
            }
        } else if let Some((a, r)) = counts.get(&c.path) {
            (c.added, c.removed) = (*a, *r);
        }
    }
    Ok(st)
}

/// One file's changes against HEAD (staged and unstaged together), or all of an untracked file.
pub fn diff(root: &Path, path: &str, old_path: Option<&str>, untracked: bool) -> Result<Diff, String> {
    let path = files::clean(path)?;
    let out = if untracked {
        files::inside(root, &path)?;
        run(root, &["--literal-pathspecs", "diff", "--no-index", "--no-ext-diff", "--", "/dev/null", &path], None)?
    } else {
        let old = old_path.map(files::clean).transpose()?;
        let st = Status { head: head(root)?, ..Status::default() };
        let base = base(root, &st)?;
        // Literal, so a file named `*.ts` or `:x` is that file and not a pattern.
        let mut args = vec!["--literal-pathspecs", "diff", base.as_str(), "--no-ext-diff", "-M", "--relative", "--"];
        if let Some(old) = old.as_deref() {
            args.push(old);
        }
        args.push(&path);
        run(root, &args, None)?
    };
    // `--no-index` exits with 1 when the files differ, which they always do here.
    if !out.ok && out.code != Some(1) {
        return Err(out.error());
    }
    if out.stdout.len() > DIFF_LIMIT {
        return Ok(Diff { patch: String::new(), binary: false, too_large: true });
    }
    let binary = !out.stdout.contains("\n@@") && out.stdout.lines().any(|l| l.starts_with("Binary files "));
    Ok(Diff { patch: out.stdout, binary, too_large: false })
}

fn head(root: &Path) -> Result<Option<String>, String> {
    let out = run(root, &["rev-parse", "--verify", "-q", "--short", "HEAD"], None)?;
    Ok(out.ok.then(|| out.stdout.trim().to_string()))
}

/// `git for-each-ref` output in the format `branches` asks for.
pub fn parse_branches(raw: &str) -> Vec<Branch> {
    raw.lines()
        .filter_map(|line| {
            let f: Vec<&str> = line.split('\0').collect();
            let [current, name, upstream, track, at, subject] = f[..] else { return None };
            let (mut ahead, mut behind) = (0, 0);
            for part in track.split(", ") {
                if let Some(n) = part.strip_prefix("ahead ") {
                    ahead = n.parse().unwrap_or(0);
                } else if let Some(n) = part.strip_prefix("behind ") {
                    behind = n.parse().unwrap_or(0);
                }
            }
            Some(Branch {
                name: name.to_string(),
                current: current == "*",
                upstream: (!upstream.is_empty()).then(|| upstream.to_string()),
                ahead,
                behind,
                gone: track == "gone",
                subject: subject.to_string(),
                at: at.parse::<i64>().unwrap_or(0) * 1000,
            })
        })
        .collect()
}

/// `git log` output in the format `branches` asks for.
pub fn parse_commits(raw: &str) -> Vec<Commit> {
    raw.split('\x1e')
        .filter_map(|rec| {
            let f: Vec<&str> = rec.trim_start_matches('\n').split('\0').collect();
            let [hash, subject, author, at] = f[..] else { return None };
            Some(Commit { hash: hash.into(), subject: subject.into(), author: author.into(), at: at.trim().parse::<i64>().unwrap_or(0) * 1000 })
        })
        .collect()
}

/// Local branches, most recently committed first, and the last commits on HEAD.
pub fn branches(root: &Path) -> Result<Branches, String> {
    if prefix(root)?.is_none() {
        return Ok(Branches::default());
    }
    let refs = run(
        root,
        &[
            "for-each-ref",
            "--sort=-committerdate",
            BRANCH_LIMIT,
            "--format=%(HEAD)%00%(refname:short)%00%(upstream:short)%00%(upstream:track,nobracket)%00%(committerdate:unix)%00%(contents:subject)",
            "refs/heads",
        ],
        None,
    )?;
    if !refs.ok {
        return Err(refs.error());
    }
    let commits = match head(root)? {
        Some(_) => {
            let log = run(root, &["log", COMMIT_LIMIT, "--format=%h%x00%s%x00%an%x00%ct%x1e"], None)?;
            if !log.ok {
                return Err(log.error());
            }
            parse_commits(&log.stdout)
        }
        None => Vec::new(),
    };
    Ok(Branches { repo: true, branches: parse_branches(&refs.stdout), commits })
}

/// Checks out another local branch. Refused while tracked files have uncommitted changes, so a
/// switch never carries half-done work to another branch; untracked files stay where they are,
/// and git itself refuses when one would be overwritten.
pub fn switch(root: &Path, branch: &str) -> Result<Status, String> {
    let st = status(root)?;
    if !st.repo {
        return Err("This folder is not a git repository.".into());
    }
    if st.changes.iter().any(|c| c.code != "?") {
        return Err("Commit or stash the changes in this folder before switching branches.".into());
    }
    if st.branch.as_deref() == Some(branch) {
        return Ok(st);
    }
    if !branches(root)?.branches.iter().any(|b| b.name == branch) {
        return Err("That branch does not exist in this repository.".into());
    }
    let out = run(root, &["switch", "--no-guess", branch], None)?;
    if !out.ok {
        return Err(format!("Git could not switch branches: {}", out.error()));
    }
    status(root)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_reads_the_branch_and_every_kind_of_change() {
        let raw = [
            "# branch.oid 74c9a12a82a810383c5869ebf160715ca99fd6d2",
            "# branch.head main",
            "# branch.upstream origin/main",
            "# branch.ab +2 -1",
            "1 .M N... 100644 100644 100644 4286f42 4286f42 sub/app file.ts",
            "1 A. N... 000000 100644 100644 0000000 1234567 sub/new.ts",
            "1 .D N... 100644 100644 000000 1234567 1234567 sub/gone.ts",
            "2 RM N... 100644 100644 100644 7898192 7898192 R100 sub/b.txt",
            "sub/a.txt",
            "u UU N... 100644 100644 100644 100644 1111111 2222222 3333333 sub/conflict.ts",
            "? sub/untracked.md",
            "",
        ]
        .join("\0");
        let st = parse_status(&raw, "sub/");
        assert_eq!((st.branch.as_deref(), st.head.as_deref()), (Some("main"), Some("74c9a12")));
        assert_eq!((st.upstream.as_deref(), st.ahead, st.behind), (Some("origin/main"), 2, 1));
        let codes: Vec<_> = st.changes.iter().map(|c| (c.path.as_str(), c.code.as_str(), c.staged, c.unstaged)).collect();
        assert_eq!(
            codes,
            vec![
                ("app file.ts", "M", false, true),
                ("new.ts", "A", true, false),
                ("gone.ts", "D", false, true),
                ("b.txt", "R", true, true),
                ("conflict.ts", "U", true, true),
                ("untracked.md", "?", false, true),
            ]
        );
        assert_eq!(st.changes[3].old_path.as_deref(), Some("a.txt"));
    }

    #[test]
    fn status_before_the_first_commit_and_on_a_detached_head() {
        let st = parse_status("# branch.oid (initial)\0# branch.head main\0", "");
        assert_eq!((st.head, st.branch.as_deref()), (None, Some("main")));
        let st = parse_status("# branch.oid abcdef0123\0# branch.head (detached)\0", "");
        assert_eq!((st.head.as_deref(), st.branch), (Some("abcdef0"), None));
    }

    #[test]
    fn numstat_counts_lines_and_follows_renames() {
        let raw = "3\t1\tsrc/app.ts\0-\t-\tlogo.png\x001\t0\t\0a.txt\0a b.txt\0";
        let counts = parse_numstat(raw);
        assert_eq!(counts["src/app.ts"], (Some(3), Some(1)));
        assert_eq!(counts["logo.png"], (None, None));
        assert_eq!(counts["a b.txt"], (Some(1), Some(0)));
        assert!(!counts.contains_key("a.txt"));
    }

    #[test]
    fn branches_and_commits_parse() {
        let raw = "*\0main\0origin/main\0ahead 2, behind 1\x001790000000\0Fix the tray\n \0old\0origin/old\0gone\x001780000000\0Old work\n \0local\0\0\x001770000000\0Try a thing\n";
        let b = parse_branches(raw);
        assert_eq!(b.len(), 3);
        assert!(b[0].current && b[0].ahead == 2 && b[0].behind == 1);
        assert_eq!(b[0].at, 1_790_000_000_000);
        assert!(b[1].gone && !b[1].current);
        assert_eq!(b[2].upstream, None);

        let log = "abc1234\0Fix the tray\0Ana\x001790000000\x1e\ndef5678\0First\0Ana\x001780000000\x1e\n";
        let c = parse_commits(log);
        assert_eq!(c.len(), 2);
        assert_eq!((c[1].hash.as_str(), c[1].subject.as_str(), c[1].at), ("def5678", "First", 1_780_000_000_000));
    }

    /// A repository in a temporary folder, or none when git is not installed.
    fn repo(name: &str) -> Option<PathBuf> {
        exe()?;
        let dir = std::env::temp_dir().join(format!("oc-git-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        git(&dir, &["init", "-q", "-b", "main"]);
        git(&dir, &["config", "user.email", "test@example.com"]);
        git(&dir, &["config", "user.name", "Test"]);
        // The machine's own settings must not sign, convert line ends or run hooks here.
        git(&dir, &["config", "core.autocrlf", "false"]);
        git(&dir, &["config", "commit.gpgsign", "false"]);
        git(&dir, &["config", "core.hooksPath", ".git/no-hooks"]);
        Some(dir)
    }

    fn git(dir: &Path, args: &[&str]) {
        let out = run(dir, args, None).unwrap();
        assert!(out.ok, "git {args:?}: {}", out.stderr);
    }

    #[test]
    fn a_real_repository_shows_changes_diffs_ignores_and_switches() {
        let Some(root) = repo("real") else { return };
        std::fs::write(root.join(".gitignore"), "build/\n").unwrap();
        std::fs::write(root.join("a.txt"), "one\ntwo\n").unwrap();
        git(&root, &["add", "."]);
        git(&root, &["commit", "-qm", "First"]);
        git(&root, &["branch", "feature"]);

        std::fs::write(root.join("a.txt"), "one\n2\nthree\n").unwrap();
        std::fs::write(root.join("new.txt"), "fresh\n").unwrap();
        std::fs::create_dir_all(root.join("build")).unwrap();
        std::fs::write(root.join("build/out.js"), "x").unwrap();

        let st = status(&root).unwrap();
        assert!(st.repo);
        assert_eq!(st.branch.as_deref(), Some("main"));
        let a = st.changes.iter().find(|c| c.path == "a.txt").unwrap();
        assert_eq!((a.code.as_str(), a.added, a.removed), ("M", Some(2), Some(1)));
        let n = st.changes.iter().find(|c| c.path == "new.txt").unwrap();
        assert_eq!((n.code.as_str(), n.added), ("?", Some(1)));
        assert!(!st.changes.iter().any(|c| c.path.starts_with("build")));

        let d = diff(&root, "a.txt", None, false).unwrap();
        assert!(d.patch.contains("-two") && d.patch.contains("+three"));
        let d = diff(&root, "new.txt", None, true).unwrap();
        assert!(d.patch.contains("+fresh"));

        let listed = files::list(&root, "").unwrap();
        let build = listed.entries.iter().find(|e| e.name == "build").unwrap();
        assert!(build.ignored);
        assert!(!listed.entries.iter().find(|e| e.name == "a.txt").unwrap().ignored);
        let mut names = files(&root).unwrap();
        names.sort();
        assert_eq!(names, vec![".gitignore", "a.txt", "new.txt"]);

        // Tracked changes block a switch; once committed, the switch goes through.
        assert!(switch(&root, "feature").unwrap_err().starts_with("Commit or stash"));
        git(&root, &["commit", "-qam", "Second"]);
        let after = switch(&root, "feature").unwrap();
        assert_eq!(after.branch.as_deref(), Some("feature"));
        assert!(switch(&root, "missing").is_err());

        let b = branches(&root).unwrap();
        assert!(b.branches.iter().any(|x| x.name == "feature" && x.current));
        assert_eq!(b.commits[0].subject, "First");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_folder_below_the_repository_top_sees_only_its_own_changes() {
        let Some(root) = repo("sub") else { return };
        std::fs::create_dir_all(root.join("web")).unwrap();
        std::fs::write(root.join("top.txt"), "t\n").unwrap();
        std::fs::write(root.join("web/page.txt"), "p\n").unwrap();
        git(&root, &["add", "."]);
        git(&root, &["commit", "-qm", "First"]);
        std::fs::write(root.join("top.txt"), "t2\n").unwrap();
        std::fs::write(root.join("web/page.txt"), "p2\n").unwrap();

        let st = status(&root.join("web")).unwrap();
        let paths: Vec<_> = st.changes.iter().map(|c| (c.path.as_str(), c.added)).collect();
        assert_eq!(paths, vec![("page.txt", Some(1))]);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_folder_outside_any_repository_is_not_one() {
        if exe().is_none() {
            return;
        }
        let dir = std::env::temp_dir().join(format!("oc-git-none-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        // The temp folder itself could sit inside a repository on some machine; then skip.
        if prefix(&dir).unwrap().is_some() {
            return;
        }
        assert!(!status(&dir).unwrap().repo);
        assert!(!branches(&dir).unwrap().repo);
        assert!(files(&dir).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
