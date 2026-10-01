# Cross-platform parity Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** OpenCompanion works the same on Windows, Linux and macOS, with CI scripts and a CircleCI config that prove it.

**Architecture:** Platform differences stay behind small functions in the Rust core (`shell_env`, `proc::kill_tree`, `autostart`, `show_notification`) and one frontend helper (`src/lib/platform.ts`). Windows code paths are kept as they are. CI steps live in `scripts/ci/` and `e2e/linux/`, which CircleCI (and later GitHub Actions) only calls.

**Tech Stack:** Rust 2021, Tauri 2.11, portable-pty 0.9, notify-rust 4.18, libc 0.2, SvelteKit 2 / Svelte 5, Vitest 5, Bun, Docker, CircleCI 2.1.

**Spec:** `docs/superpowers/specs/2026-09-28-cross-platform-parity-design.md`

## Global Constraints

- Windows behavior does not change: registry autostart, notify-rust toasts, PowerShell host for sessions, current CLI search order.
- Identifier stays `dev.opencompanion.app`.
- No new runtime dependency except `libc` (Unix target only). notify-rust moves from a Windows-only dependency to "not macOS".
- Every user-facing string exists in English and Indonesian (`src/lib/i18n/*.ts`); backend messages get a pattern in `src/lib/i18n/backend.ts`.
- Copy follows DESIGN.md section 13: no em dashes, name the thing, no buzzwords.
- `cargo clippy --all-targets -- -D warnings` must pass on Windows and Linux; macOS-only code is kept tiny and cfg-gated so it cannot warn elsewhere.
- Commits: Conventional Commits in English, no AI attribution lines. Tests run from the checkout path in its real letter case, since a Windows working folder that differs only in case breaks path comparisons.

## Review Focus

- A login shell that prints a banner, asks a question, or hangs: capture must time out (5 s) and fall back to the inherited PATH plus fallback folders. Test: parser ignores text outside the markers; capture of a shell that sleeps is cut at the timeout.
- `kill_tree(0)` or `kill_tree(1)` on Unix would signal the app's own group or every process. Guard pid <= 1. Test pins it.
- A zombie grandchild left in a container without an init reaper must count as gone in the process-group test (sysinfo status `Zombie`).
- Paths with spaces, quotes or `$` in the autostart entry (`Exec=` and plist): quoted and escaped. Test with such a path.
- Folder rules on Linux must not match a folder that differs only in case; on Windows they still do. Test per OS.

---

### Task 1: Portable test fixtures and non-Windows clippy warnings

**Files:**
- Modify: `src-tauri/src/orchestrator.rs` (test `prompt_lists_folders_with_markers_before_the_request`)
- Modify: `src-tauri/src/session.rs` (test `titles_come_from_the_prompt_or_the_folder`)
- Modify: `src-tauri/src/projects.rs` (tests `claude_names_decode_...`, `names_resolve_only_when_one_folder_fits`) — the decode test is rewritten in Task 2
- Modify: `src-tauri/src/proc.rs` (`hidden` gets `process_group` in Task 4, which makes `mut` used; until then gate `mut`)
- Modify: `src-tauri/src/lib.rs` (`open_session` cfg), `src-tauri/src/autostart.rs` (tests module gains Unix tests in Task 5)

- [ ] Step 1: In `orchestrator.rs` build the fixture from the OS separator:

```rust
    #[test]
    fn prompt_lists_folders_with_markers_before_the_request() {
        let path = Path::new(std::path::MAIN_SEPARATOR_STR).join("p").join("uninote");
        let known = vec![folder(&path)];
        let c = clis();
        let text = prompt_text(&input(&c, &known));
        let clis_at = text.find("## Installed CLIs").unwrap();
        let ask_at = text.find("add dark mode").unwrap();
        assert!(clis_at < ask_at);
        assert!(text.contains(&format!("- uninote: {} (git, package.json)", path.display())), "{text}");
    }
```

- [ ] Step 2: In `session.rs`:

```rust
    #[test]
    fn titles_come_from_the_prompt_or_the_folder() {
        let sep = std::path::MAIN_SEPARATOR;
        let dir = format!("{sep}p{sep}uninote");
        assert_eq!(title_for(CliKind::Codex, &dir, "Fix tests\nthen more"), "Fix tests");
        assert_eq!(title_for(CliKind::Claude, &format!("{dir}{sep}"), "  "), "Claude Code in uninote");
    }
```

- [ ] Step 3: In `projects.rs` `names_resolve_only_when_one_folder_fits`, replace the Windows-only wrong path and trailing separator:

```rust
        let elsewhere = Path::new(std::path::MAIN_SEPARATOR_STR).join("Users").join("someone").join("projects").join("AI-Remote");
        assert_eq!(resolve(&elsewhere.display().to_string(), &known).as_deref(), Some(ai_remote.as_str()));
        ...
        let trailing = format!("{ai_remote}{}", std::path::MAIN_SEPARATOR);
```

- [ ] Step 4: Run in Linux container and on Windows: `cargo test --lib orchestrator:: session::tests::titles projects::tests::names` → PASS on both.
- [ ] Step 5: Commit `test(core): build path fixtures for the OS the tests run on`.

The three clippy warnings disappear through Tasks 4 (`mut cmd` becomes used), 5 (autostart tests use `super::*` on Unix) and 6 (`open_session` gated to non-macOS and used on Linux). Task 10 checks clippy with `-D warnings` on both OSes.

### Task 2: Per-OS path comparison and Unix Claude history

**Files:** Modify `src-tauri/src/projects.rs` (`norm`, `contains`, `skip`, `decode_claude_dir`, tests).

**Interfaces:** Produces `norm(&str) -> String` with separators unified to `/` on Windows only; case folded on Windows and macOS only. `contains(folder, path)` uses `/` as the joiner after `norm`.

- [ ] Step 1: Tests (fail first on Linux):

```rust
    #[test]
    fn folder_rules_follow_the_file_system_case_rules() {
        let sep = std::path::MAIN_SEPARATOR;
        let a = format!("{sep}x{sep}Work");
        let b = format!("{sep}x{sep}work{sep}app");
        assert_eq!(contains(&a, &b), cfg!(any(windows, target_os = "macos")));
        assert!(contains(&a, &format!("{a}{sep}app")));
        assert!(!contains(&a, &format!("{a}-other")));
    }

    #[test]
    fn claude_names_decode_to_real_folders_even_with_dashes_and_dots() {
        let base = tree("decode");
        let mut cache = HashMap::new();
        // Claude Code turns every character that is not a letter or digit into `-`, on every OS.
        let enc = |p: &Path| claude_encode(&p.display().to_string());
        let target = base.join("Work").join("ai-remote");
        assert_eq!(decode_claude_dir(&enc(&target), &mut cache), Some(target.clone()));
        let dotted = base.join("Work").join("my.app").join("sub");
        assert_eq!(decode_claude_dir(&enc(&dotted), &mut cache), Some(dotted));
        let hidden = base.join("Work").join(".hidden");
        assert_eq!(decode_claude_dir(&enc(&hidden), &mut cache), Some(hidden));
        assert_eq!(decode_claude_dir("C--definitely-not-here-xyz", &mut cache), None);
        assert_eq!(decode_claude_dir("-definitely-not-here-xyz", &mut cache), None);
        let _ = fs::remove_dir_all(&base);
    }
```

- [ ] Step 2: Implementation:

```rust
/// A path as it compares: no trailing separator; on Windows `\` and `/` are one separator.
/// Case is folded where the file system ignores it by default (Windows, macOS), and kept on Linux,
/// where `Work` and `work` are two folders.
pub(crate) fn norm(p: &str) -> String {
    let trimmed = p.trim_end_matches(['\\', '/']);
    if cfg!(windows) {
        trimmed.replace('\\', "/").to_lowercase()
    } else if cfg!(target_os = "macos") {
        trimmed.to_lowercase()
    } else {
        trimmed.to_string()
    }
}

pub fn contains(folder: &str, path: &str) -> bool {
    let (f, p) = (norm(folder), norm(path));
    p == f || p.starts_with(&format!("{f}/"))
}
```

`skip` uses `/appdata/`, `/node_modules`, `/.git` (after `norm`, Windows separators are `/`; the AppData check only matters on Windows, where case is folded).

`decode_claude_dir` start:

```rust
    let (mut dir, rest) = match encoded.strip_prefix('-') {
        // Unix: `/home/u/proj` is stored as `-home-u-proj`.
        Some(rest) if cfg!(unix) => (PathBuf::from("/"), rest.to_string()),
        _ => {
            let drive = encoded.chars().next().filter(char::is_ascii_alphabetic)?;
            let rest = encoded.get(1..)?.strip_prefix("--")?;
            (PathBuf::from(format!("{drive}:\\")), rest.to_string())
        }
    };
    let mut remaining = rest;
```

Dot folders: `/root/.hidden` encodes as `-root--hidden`. After `root` matches, `trim_start_matches('-')` leaves `hidden`, but `claude_encode(".hidden")` is `-hidden`, so the current comparison never matches a dot folder (on Windows either). Compare each child's encoding with its leading dashes trimmed, and advance by that trimmed length:

```rust
        let best = children
            .iter()
            .filter(|c| {
                let enc = claude_encode(c).to_lowercase();
                let enc = enc.trim_start_matches('-');
                !enc.is_empty() && (lower == enc || lower.starts_with(&format!("{enc}-")))
            })
            .max_by_key(|c| c.len())?;
        dir = dir.join(best);
        let used = claude_encode(best).trim_start_matches('-').len();
```

- [ ] Step 3: `cargo test --lib projects::` on Linux and Windows → PASS.
- [ ] Step 4: Commit `fix(projects): compare folders by the OS's case rules and read Claude history on Unix`.

### Task 3: Login shell environment for started CLIs

**Files:**
- Create: `src-tauri/src/shell_env.rs`
- Modify: `src-tauri/src/lib.rs` (`pub mod shell_env;`, `shell_env::start()` first in `setup`)
- Modify: `src-tauri/src/proc.rs` (`command` applies the environment; `hidden` becomes `pub(crate)`)
- Modify: `src-tauri/src/pty.rs` (spawn applies it, plus `TERM`/`COLORTERM` on Unix)
- Modify: `src-tauri/src/cli.rs` (`resolve` searches the captured PATH, `extra_dirs` adds the fallback folders on Unix)

**Interfaces:**
- Produces: `shell_env::start()`, `shell_env::vars() -> &'static [(String, String)]` (empty on Windows), `shell_env::path() -> Option<&'static str>`, `shell_env::FALLBACK_DIRS: &[&str]` (home-relative when starting with `~/`).

- [ ] Step 1: Tests in `shell_env.rs` (pure functions compiled on every OS under `#[cfg(any(unix, test))]`):

```rust
    #[test]
    fn only_the_text_between_the_markers_counts() {
        let out = "motd line\nPATH=/nope\n__OC_ENV_START__\nPATH=/a:/b\nMULTI=one\ntwo\nHOME=/h\n_=/usr/bin/env\nSHLVL=2\n__OC_ENV_END__\nbye\n";
        let vars = parse(out);
        assert_eq!(vars, vec![("PATH".into(), "/a:/b".into()), ("MULTI".into(), "one\ntwo".into()), ("HOME".into(), "/h".into())]);
        assert!(parse("no markers at all").is_empty());
    }

    #[test]
    fn the_fallback_folders_join_the_path_once_and_only_when_they_exist() {
        let dir = std::env::temp_dir().join(format!("oc-shellenv-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("bin")).unwrap();
        let joined = with_fallback("/usr/bin", &[dir.join("bin"), dir.join("missing"), PathBuf::from("/usr/bin")]);
        assert_eq!(joined, format!("/usr/bin:{}", dir.join("bin").display()));
        let _ = std::fs::remove_dir_all(&dir);
    }
```

Unix only: `a_shell_that_hangs_is_cut_at_the_timeout` runs `capture_with(Path::new("/bin/sh"), &["-c", "sleep 30"], Duration::from_millis(300))` and asserts `None` within 3 s; `a_real_shell_reports_its_path` runs `/bin/sh -c` printing the markers and `env`, asserts PATH present.

- [ ] Step 2: Implementation:

```rust
//! The environment of the user's login shell. Started from Finder or a desktop launcher, the app
//! gets a minimal PATH and none of the variables set in `~/.zshrc` or `~/.bashrc`, so CLIs would
//! not be found, `#!/usr/bin/env node` scripts would not find node, and API keys would be missing.
//! On Windows sessions start through PowerShell instead (`terminal::powershell`), so this is empty.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Duration;

const START: &str = "__OC_ENV_START__";
const END: &str = "__OC_ENV_END__";
/// Variables that describe the capturing shell itself, not the user's setup.
const SKIP: &[&str] = &["_", "PWD", "OLDPWD", "SHLVL", "TERM", "COLUMNS", "LINES"];
/// Install folders a GUI launch usually lacks, added after the shell's PATH when they exist.
pub const FALLBACK_DIRS: &[&str] = &[
    "/opt/homebrew/bin", "/usr/local/bin", "~/.local/bin", "~/.npm-global/bin",
    "~/.volta/bin", "~/.bun/bin", "~/.cargo/bin", "~/.claude/local",
];

static VARS: OnceLock<Vec<(String, String)>> = OnceLock::new();

/// Starts the capture in the background, so the first session does not wait for the shell.
pub fn start() {
    std::thread::spawn(|| {
        vars();
    });
}

/// The captured variables; the first call waits for the capture (at most its timeout).
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

#[cfg(unix)]
fn load() -> Vec<(String, String)> {
    let shell = std::env::var_os("SHELL").map(PathBuf::from).filter(|p| p.is_file()).unwrap_or_else(|| PathBuf::from("/bin/sh"));
    let script = format!("printf '%s\\n' {START}; /usr/bin/env; printf '%s\\n' {END}");
    let mut vars = capture_with(&shell, &["-ilc", &script], Duration::from_secs(5)).unwrap_or_default();
    let inherited = std::env::var("PATH").unwrap_or_default();
    let base = vars.iter().find(|(k, _)| k == "PATH").map(|(_, v)| v.clone()).unwrap_or(inherited);
    let path = with_fallback(&base, &fallback_dirs());
    vars.retain(|(k, _)| k != "PATH");
    vars.push(("PATH".into(), path));
    vars
}
```

`capture_with` runs `proc::hidden(shell).args(args)` (no shell environment applied, so no recursion) through `proc::run_with_timeout`, returns `None` on spawn error or timeout, else `Some(parse(&out.stdout))`. `parse` takes the text between the marker lines; a line starting with a name (`[A-Za-z_][A-Za-z0-9_]*=`) starts a variable, any other line continues the previous value with `\n`; names in `SKIP` and `proc::INHERITED_SESSION_VARS` are dropped. `fallback_dirs` expands `~/` with `projects::home()`. `with_fallback(base, dirs)` appends each existing dir not already in `base`, joined with `:`.

- [ ] Step 3: Apply at spawn points.

`proc.rs`:

```rust
pub fn command(program: impl AsRef<Path>) -> Command {
    let (program, lead) = launcher(program.as_ref());
    let mut cmd = hidden(&program);
    cmd.args(lead);
    cmd.envs(crate::shell_env::vars().iter().map(|(k, v)| (k, v)));
    for var in INHERITED_SESSION_VARS {
        cmd.env_remove(var);
    }
    cmd
}
```

`pty.rs` before `INHERITED_SESSION_VARS` removal:

```rust
        for (k, v) in crate::shell_env::vars() {
            cmd.env(k, v);
        }
        // What xterm.js understands; a GUI launch has no TERM at all.
        #[cfg(unix)]
        {
            cmd.env("TERM", "xterm-256color");
            cmd.env("COLORTERM", "truecolor");
        }
```

`cli.rs` `resolve`:

```rust
pub fn resolve(kind: CliKind) -> Option<PathBuf> {
    let mut dirs: Vec<PathBuf> = match crate::shell_env::path() {
        Some(p) => std::env::split_paths(p).collect(),
        None => std::env::var_os("PATH").map(|p| std::env::split_paths(&p).collect()).unwrap_or_default(),
    };
    dirs.extend(extra_dirs().into_iter().filter(|d| d.is_dir()));
    let search = std::env::join_paths(dirs).ok()?;
    let cwd = std::env::current_dir().ok()?;
    which::which_in(kind.bin(), Some(search), cwd).ok().map(unwrap_shim)
}
```

`extra_dirs` on Unix also pushes every expanded `shell_env::FALLBACK_DIRS` entry (a shared `shell_env::fallback_dirs()`).

- [ ] Step 4: `lib.rs`: `pub mod shell_env;` and `shell_env::start();` at the top of `setup`.
- [ ] Step 5: Linux container: `cargo test --lib shell_env:: cli::` and the full `cargo test`; Windows: full `cargo test`. Both PASS.
- [ ] Step 6: Commit `feat(core): give CLIs the login shell's PATH and environment on macOS and Linux`.

### Task 4: Stop the whole process tree on Unix

**Files:** Modify `src-tauri/Cargo.toml` (`[target.'cfg(unix)'.dependencies] libc = "0.2"`), `src-tauri/src/proc.rs` (`hidden`, `kill_tree`, tests), `src-tauri/src/pty.rs` (test).

- [ ] Step 1: Test in `proc.rs`:

```rust
    #[cfg(unix)]
    fn gone(pid: u32) -> bool {
        use sysinfo::{ProcessRefreshKind, ProcessStatus, ProcessesToUpdate, System};
        let mut sys = System::new();
        let p = sysinfo::Pid::from_u32(pid);
        sys.refresh_processes_specifics(ProcessesToUpdate::Some(&[p]), true, ProcessRefreshKind::nothing());
        // In a container without an init that reaps, a killed orphan stays a zombie.
        sys.process(p).is_none_or(|p| p.status() == ProcessStatus::Zombie)
    }

    #[cfg(unix)]
    #[test]
    fn kill_tree_ends_the_children_too_on_unix() {
        let mut parent = command("/bin/sh").args(["-c", "sleep 30 & echo $!; wait"]).stdout(Stdio::piped()).spawn().unwrap();
        let mut line = String::new();
        std::io::BufRead::read_line(&mut std::io::BufReader::new(parent.stdout.take().unwrap()), &mut line).unwrap();
        let grandchild: u32 = line.trim().parse().unwrap();
        kill_tree(parent.id());
        let _ = parent.wait();
        let deadline = Instant::now() + Duration::from_secs(4);
        while !gone(grandchild) {
            assert!(Instant::now() < deadline, "the grandchild outlived Stop");
            thread::sleep(Duration::from_millis(50));
        }
    }

    #[cfg(unix)]
    #[test]
    fn kill_tree_never_signals_our_own_group_or_everyone() {
        kill_tree(0);
        kill_tree(1);
    }
```

`pty.rs` Unix test: spawn `/bin/sh -c 'sleep 30 & echo "child $!"; wait'`, read the pid from `screen_text()`, call `kill()`, assert `gone`.

- [ ] Step 2: Implementation:

```rust
pub(crate) fn hidden(program: impl AsRef<OsStr>) -> Command {
    let mut cmd = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    // Its own process group, so `kill_tree` can end the CLI together with what it started.
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    cmd
}
```

`kill_tree` Unix branch:

```rust
    #[cfg(unix)]
    {
        // 0 is our own group and 1 would mean every process: never a CLI we started.
        if pid <= 1 {
            return;
        }
        let group = -(pid as i32);
        // SAFETY: kill(2) only sends a signal; a negative pid names the process group.
        unsafe { libc::kill(group, libc::SIGTERM) };
        thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(2);
            while Instant::now() < deadline {
                // Signal 0 only asks whether the group still has a member.
                if unsafe { libc::kill(group, 0) } != 0 {
                    return;
                }
                thread::sleep(Duration::from_millis(50));
            }
            unsafe { libc::kill(group, libc::SIGKILL) };
        });
    }
```

A PTY child is a session leader (portable-pty calls `setsid`), so its pid is its group id and the same call works.

- [ ] Step 3: Linux: `cargo test --lib proc:: pty::` PASS; Windows: `cargo test --lib proc::` PASS.
- [ ] Step 4: Commit `fix(core): stop a CLI's whole process group on macOS and Linux`.

### Task 5: Start at login on macOS and Linux

**Files:** Modify `src-tauri/src/autostart.rs`, `src/lib/i18n/backend.ts`, `src/lib/i18n/settings.ts`.

**Interfaces:** `autostart::supported()` true on Windows, macOS and Linux; `enabled()`, `set(on)` unchanged signatures.

- [ ] Step 1: Tests (pure builders compiled `#[cfg(any(target_os = "linux", test))]` / `#[cfg(any(target_os = "macos", test))]`):

```rust
    #[test]
    fn the_desktop_entry_quotes_the_path_and_starts_hidden() {
        let entry = desktop_entry(Path::new("/opt/Open Companion/o\"c$`\\"));
        assert!(entry.contains("Exec=\"/opt/Open Companion/o\\\"c\\$\\`\\\\\" --hidden\n"), "{entry}");
        assert!(entry.starts_with("[Desktop Entry]\nType=Application\nName=OpenCompanion\n"));
    }

    #[test]
    fn the_launch_agent_escapes_the_path_and_runs_at_load() {
        let plist = launch_agent(Path::new("/Applications/A&B <x>.app/Contents/MacOS/opencompanion"));
        assert!(plist.contains("<string>/Applications/A&amp;B &lt;x&gt;.app/Contents/MacOS/opencompanion</string>"));
        assert!(plist.contains("<string>--hidden</string>") && plist.contains("<key>RunAtLoad</key>\n  <true/>"));
    }
```

Unix only: `the_entry_is_written_and_removed_again` calls `write_entry(&tmp_path, exe, true)`, checks the file, `write_entry(.., false)` twice (second is a no-op).

- [ ] Step 2: Implementation: `entry_path()` is `$XDG_CONFIG_HOME/autostart/opencompanion.desktop` (fallback `~/.config`) on Linux and `~/Library/LaunchAgents/dev.opencompanion.app.plist` on macOS. `launch_target()` is `$APPIMAGE` when set (Linux AppImage), else `current_exe()`. `enabled()` = entry file exists. `set(on)` writes or removes it. Errors: `Could not add OpenCompanion to the login items: {e}` and `Could not remove OpenCompanion from the login items: {e}`. Windows code is untouched.
- [ ] Step 3: `backend.ts` patterns for the two new messages. `settings.ts`: `settings.window.login` becomes "Start in the tray when I sign in" / "Mulai di tray saat saya masuk ke komputer"; Indonesian `settings.window.title` "Jendela dan masuk ke komputer", `loginOnSaved`/`loginOffSaved` drop "Windows" ("...saat Anda masuk ke komputer.").
- [ ] Step 4: Tests on Linux and Windows PASS; Vitest settings tests PASS.
- [ ] Step 5: Commit `feat(settings): start at login on macOS and Linux`.

### Task 6: Notification click on Linux, tray fallback, Dock reopen

**Files:** Modify `src-tauri/Cargo.toml` (notify-rust target `cfg(not(any(target_os = "macos", target_os = "android", target_os = "ios")))`), `src-tauri/src/lib.rs`.

- [ ] Step 1: `show_notification` for `#[cfg(not(target_os = "macos"))]` is the current Windows function, with the Windows-only `app_id` block under `#[cfg(windows)]` and, on Linux, `toast.appname("OpenCompanion").icon("opencompanion").action("default", open_label)` so a click on the body reports `Default`. The macOS variant is the current plugin variant. `open_session` gets `#[cfg(not(target_os = "macos"))]`. `use tauri_plugin_notification::NotificationExt;` becomes `#[cfg(target_os = "macos")]`.
- [ ] Step 2: Tray: in `setup`, `let has_tray = match tray(app, lang) { Ok(()) => true, Err(e) => { eprintln!("tray unavailable: {e}"); false } };`, kept in a `static HAS_TRAY: AtomicBool`. `on_close` returns early (lets the window close and the app quit) when `!HAS_TRAY.load(..)`.
- [ ] Step 3: `app.run`: `#[cfg(target_os = "macos")] if let RunEvent::Reopen { .. } = event { show_main(handle); }`.
- [ ] Step 4: Linux container: `cargo clippy --all-targets -- -D warnings` and `cargo test` PASS; E2E still passes. Windows: same.
- [ ] Step 5: Commit `feat(desktop): open the session from a Linux notification and keep the app reachable without a tray`.

### Task 7: Platform helper and macOS traffic lights

**Files:** Create `src/lib/platform.ts`, `src/lib/platform.test.ts`, `src-tauri/tauri.macos.conf.json`. Modify `src/lib/TitleBar.svelte`, `src/lib/TitleBar.test.ts`.

**Interfaces:** `type Platform = "windows" | "macos" | "linux"`; `platformOf(ua: string): Platform`; `currentPlatform(): Platform` (reads `navigator.userAgent` on each call, so tests can stub it).

- [ ] Step 1: Tests:

```ts
import { expect, test } from "vitest";
import { platformOf } from "./platform";

test("the webview's user agent names the OS", () => {
  expect(platformOf("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Edg/140.0")).toBe("windows");
  expect(platformOf("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15")).toBe("macos");
  expect(platformOf("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/605.1.15")).toBe("linux");
  expect(platformOf("Mozilla/5.0 (linux) AppleWebKit/537.36 jsdom/27")).toBe("linux");
});
```

`TitleBar.test.ts` gains: with `vi.spyOn(navigator, "userAgent", "get").mockReturnValue(MAC_UA)`, the minimize, maximize and close buttons are absent and `#titlebar` still has `data-tauri-drag-region`.

- [ ] Step 2: `platform.ts`:

```ts
export type Platform = "windows" | "macos" | "linux";

/** WebView2 says "Windows NT", WKWebView "Macintosh", WebKitGTK "X11; Linux". */
export function platformOf(ua: string): Platform {
  if (ua.includes("Windows")) return "windows";
  if (ua.includes("Macintosh") || ua.includes("Mac OS X")) return "macos";
  return "linux";
}

export function currentPlatform(): Platform {
  return platformOf(typeof navigator === "undefined" ? "" : navigator.userAgent);
}
```

`TitleBar.svelte`: `const native = currentPlatform() === "macos";` and `{#if !native}<div class="tb-controls">…</div>{/if}`; comment explains macOS draws its own traffic lights over the bar.

`tauri.macos.conf.json` (JSON Merge Patch replaces the whole `windows` array, so every field is repeated):

```json
{
  "app": {
    "windows": [
      {
        "title": "OpenCompanion",
        "width": 1280,
        "height": 800,
        "minWidth": 1100,
        "minHeight": 700,
        "decorations": true,
        "titleBarStyle": "Overlay",
        "hiddenTitle": true
      }
    ]
  }
}
```

- [ ] Step 3: `bunx vitest run src/lib/platform.test.ts src/lib/TitleBar.test.ts` PASS; `bun run check` PASS.
- [ ] Step 4: Commit `feat(desktop): use the native traffic lights on macOS`.

### Task 8: Paste keys, hints, placeholders and wording per OS

**Files:** Modify `src/lib/Terminal.svelte`, `src/lib/Terminal.test.ts`, `src/lib/FolderField.svelte`, `src/lib/format.ts` (+ its test file if present, else `src/lib/format.test.ts`), `src/lib/i18n/sessions.ts`, `src/lib/i18n/terminal.ts`, `src/lib/i18n/settings.ts`, `src/lib/i18n/backend.ts`, `src-tauri/src/transcript.rs`, and the components that render `sessions.detail.terminalLive` / `terminal.live`.

**Interfaces:** `pasteKey(p: Platform): string` in `platform.ts` ("Ctrl+V", "⌘V", "Ctrl+Shift+V"); `folderExample(p: Platform): string`.

- [ ] Step 1: Tests: `Terminal.test.ts` adds "on Linux, Ctrl+Shift+V lets the browser paste in a session and Ctrl+V still goes to the CLI" (stub a WebKitGTK UA, key events as in the existing Windows test). `format.test.ts`: `shortPath("/Users/ana/Projects/app") === "~/Projects/app"`, `shortPath("/home/ana/app") === "~/app"`, `shortPath("/opt/app") === "/opt/app"`, the Windows case unchanged.
- [ ] Step 2: `Terminal.svelte`: replace `const windows = navigator.userAgent.includes("Windows")` with `const os = currentPlatform();`, keep Ctrl+V for terminals and for sessions on Windows, and add `if (key === "v" && e.shiftKey && os === "linux") return false;` before it. macOS pastes with ⌘V, which xterm leaves to the webview.
- [ ] Step 3: Strings: `sessions.detail.terminalLive` and `terminal.live` use `{paste}` for the paste key (EN and ID), filled with `pasteKey(currentPlatform())` where they are rendered. `settings.theme.desc`: "follows the light or dark setting of your system" / "mengikuti mode terang atau gelap di sistem Anda". `transcript.rs` note becomes "The system did not share this process's folder, so its transcript cannot be matched." with the backend pattern and the test updated.
- [ ] Step 4: `FolderField.svelte` placeholder `folderExample(currentPlatform())`: `C:\Users\you\Project\my-app`, `/Users/you/Projects/my-app`, `/home/you/projects/my-app`. `format.shortPath` also matches `^/(Users|home)/[^/]+`.
- [ ] Step 5: `bun run test`, `bun run check` PASS on Windows and Linux.
- [ ] Step 6: Commit `feat(ui): show each OS its own paste key, folder example and wording`.

### Task 9: CI scripts and the Linux E2E harness in the repo

**Files:** Create `scripts/ci/checks.sh`, `scripts/ci/linux-deps.sh`, `e2e/linux/Dockerfile`, `e2e/linux/run.sh`, `e2e/linux/inside.sh`, `e2e/linux/e2e.py`. CONTRIBUTING documents how to run them.

- [ ] Step 1: `scripts/ci/linux-deps.sh` installs the apt packages (Tauri prerequisites, `webkit2gtk-driver xvfb xauth dbus-x11 at-spi2-core x11-utils xclip xdg-utils python3 procps curl git unzip`), Node 24 (official tarball), Bun, rustup stable with clippy, and `tauri-driver`. The Dockerfile copies and runs it, so local runs and CI share one list.
- [ ] Step 2: `scripts/ci/checks.sh` (bash, `set -euo pipefail`, runs from the repo root): `bun install --frozen-lockfile`, `bun run check`, `bun run test`, then in `src-tauri`: `cargo clippy --all-targets -- -D warnings`, `cargo test`.
- [ ] Step 3: `e2e/linux/run.sh [checks|e2e|all]` builds the image `opencompanion-linux-e2e`, streams the working tree into a container with `tar --exclude=node_modules --exclude=src-tauri/target --exclude=.svelte-kit --exclude=build`, and runs `inside.sh`. Screenshots and logs land in `e2e/linux/out/` (git-ignored). `inside.sh e2e` builds `bun run tauri build --bundles deb`, installs the `.deb`, builds `fake-cli`, writes the `claude`/`opencode` wrappers, starts Xvfb, D-Bus and `tauri-driver`, and runs `e2e.py`.
- [ ] Step 4: `e2e.py` is the 18-step script from the Docker run, reading `OC_OUT` and `OC_PROJECT`, plus two steps: "settings: start at login writes and removes the autostart entry" (checkbox on, `~/.config/autostart/opencompanion.desktop` contains `--hidden`, off, file gone) and "terminal: Ctrl+Shift+V pastes on Linux" (`xclip -selection clipboard` holds `echo PASTED_$((2+3))`, the key chord goes to the terminal through the WebDriver actions API, `PASTED_5` appears).
- [ ] Step 5: Run `bash e2e/linux/run.sh all` from Git Bash on this machine → checks PASS and all E2E steps PASS.
- [ ] Step 6: Commit `test(e2e): run the checks and a Linux end-to-end test in Docker`.

### Task 10: CircleCI

**Files:** Create `.circleci/config.yml`.

- [ ] Step 1: Config: version 2.1; parameters `linux`, `linux_e2e`, `windows`, `macos` (boolean, default false); one workflow per parameter guarded by `when: << pipeline.parameters.X >>`.
  - `linux-checks` and `linux-e2e`: `machine: image: ubuntu-2404:current`, `checkout`, `bash e2e/linux/run.sh checks|e2e`, `store_artifacts: e2e/linux/out`.
  - `windows-checks`: `circleci/windows` orb executor; installs rustup, Bun and Node if missing (PowerShell), then `bash scripts/ci/checks.sh`.
  - `macos-checks`: `macos: xcode: 27.1.0`, `resource_class: m4pro.medium`; installs rustup and Bun, Node through Homebrew if missing, then `bash scripts/ci/checks.sh`.
  - Caches for `~/.cargo/registry`, `src-tauri/target` and Bun keyed on `Cargo.lock` / `bun.lock` for Windows and macOS.
- [ ] Step 2: Validate: `docker run --rm -v "$PWD":/repo -w /repo circleci/circleci-cli:latest circleci config validate .circleci/config.yml` → "Config file at .circleci/config.yml is valid."
- [ ] Step 3: Commit `ci: add manual CircleCI jobs for Linux, Windows and macOS`.

### Task 11: Documentation and history

**Files:** Modify `README.md`, `CONTRIBUTING.md`, `CHANGELOG.md`, `.gitignore` (`e2e/linux/out/`), create `.development-history/2026-09-28-HH-mm-cross-platform-parity.md`.

- [ ] Step 1: README: platform table (Windows 11 tested by hand; Linux tested end to end in Docker on Ubuntu 24.04; macOS built and tested in CI, not tried by hand), prerequisites per OS (Node.js for Vitest, `xdg-utils` to bundle an AppImage, AppIndicator for the tray and the GNOME extension note), notification click per OS, start at login per OS, data folders without "untested".
- [ ] Step 2: CONTRIBUTING: Node.js in the setup list, `bash scripts/ci/checks.sh`, `bash e2e/linux/run.sh all`, how to trigger CircleCI with parameters (UI and API `curl`).
- [ ] Step 3: CHANGELOG Unreleased lines.
- [ ] Step 4: Final verification on Windows (`bun run check`, `bun run test`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, `bun run tauri build`) and Linux (`e2e/linux/run.sh all`).
- [ ] Step 5: Commit `docs: describe Windows, Linux and macOS support` and push.
