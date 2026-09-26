//! Start at sign-in (PRD FR-64), Windows only: a value under the user's `Run` key starts the app
//! with `--hidden`, so it opens straight into the tray. Written with `reg.exe`, never by hand.

use std::path::{Path, PathBuf};
use std::process::Stdio;

/// The argument a sign-in start carries: the window stays hidden and the tray icon is the way in.
pub const HIDDEN_ARG: &str = "--hidden";

const RUN_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
const VALUE: &str = "OpenCompanion";

fn reg() -> std::process::Command {
    let exe = std::env::var_os("SystemRoot")
        .map(|root| PathBuf::from(root).join("System32").join("reg.exe"))
        .unwrap_or_else(|| PathBuf::from("reg.exe"));
    let mut cmd = std::process::Command::new(exe);
    cmd.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

/// Whether this platform can start the app at sign-in.
pub fn supported() -> bool {
    cfg!(windows)
}

/// Whether `value` exists under `key`.
fn present(key: &str, value: &str) -> bool {
    reg().args(["query", key, "/v", value]).status().is_ok_and(|s| s.success())
}

/// Writes or removes `value` under `key`, pointing at `exe` with the hidden flag.
fn write(key: &str, value: &str, exe: &Path, on: bool) -> Result<(), String> {
    let mut cmd = reg();
    if on {
        let data = format!("\"{}\" {HIDDEN_ARG}", exe.display());
        cmd.args(["add", key, "/v", value, "/t", "REG_SZ", "/d", &data, "/f"]);
    } else if present(key, value) {
        cmd.args(["delete", key, "/v", value, "/f"]);
    } else {
        return Ok(());
    }
    let out = cmd.output().map_err(|e| format!("Windows did not run reg.exe: {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        let why = String::from_utf8_lossy(&out.stderr).trim().to_string();
        Err(format!("Windows refused to change the sign-in list: {why}"))
    }
}

/// Whether OpenCompanion starts at sign-in now, as Windows has it.
pub fn enabled() -> bool {
    supported() && present(RUN_KEY, VALUE)
}

/// Turns the start at sign-in on or off for this user.
pub fn set(on: bool) -> Result<(), String> {
    if !supported() {
        return Err("Starting at sign-in is only available on Windows for now.".into());
    }
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    write(RUN_KEY, VALUE, &exe, on)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Writes a throwaway key under the user's Software key, never the real Run key.
    #[cfg(windows)]
    #[test]
    fn the_value_is_written_with_the_quoted_path_and_removed_again() {
        let key = format!(r"HKCU\Software\OpenCompanionTest{}", std::process::id());
        let exe = Path::new(r"C:\Program Files\OpenCompanion\opencompanion.exe");
        write(&key, VALUE, exe, true).unwrap();
        assert!(present(&key, VALUE));
        let out = reg().args(["query", &key, "/v", VALUE]).stdout(Stdio::piped()).output().unwrap();
        let text = String::from_utf8_lossy(&out.stdout);
        assert!(text.contains(r#""C:\Program Files\OpenCompanion\opencompanion.exe" --hidden"#), "{text}");
        write(&key, VALUE, exe, false).unwrap();
        assert!(!present(&key, VALUE));
        // Turning off what is already off is not an error.
        write(&key, VALUE, exe, false).unwrap();
        let _ = reg().args(["delete", &key, "/f"]).status();
    }
}
