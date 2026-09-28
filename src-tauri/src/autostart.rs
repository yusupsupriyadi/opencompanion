//! Start at sign-in (PRD FR-64). The app starts with `--hidden`, so it opens straight into the tray.
//! Windows: a value under the user's `Run` key, written with `reg.exe`, never by hand.
//! macOS: a LaunchAgent. Linux: an XDG autostart entry, which GNOME, KDE and most desktops read.

#[cfg(any(unix, test))]
use std::path::Path;
#[cfg(windows)]
use std::path::PathBuf;
#[cfg(windows)]
use std::process::Stdio;

/// The argument a sign-in start carries: the window stays hidden and the tray icon is the way in.
pub const HIDDEN_ARG: &str = "--hidden";

#[cfg(windows)]
const RUN_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
#[cfg(windows)]
const VALUE: &str = "OpenCompanion";

#[cfg(windows)]
fn reg() -> std::process::Command {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let exe = std::env::var_os("SystemRoot")
        .map(|root| PathBuf::from(root).join("System32").join("reg.exe"))
        .unwrap_or_else(|| PathBuf::from("reg.exe"));
    let mut cmd = std::process::Command::new(exe);
    cmd.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::piped());
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd
}

/// Whether this platform can start the app at sign-in.
pub fn supported() -> bool {
    cfg!(any(windows, target_os = "macos", target_os = "linux"))
}

/// Whether `value` exists under `key`.
#[cfg(windows)]
fn present(key: &str, value: &str) -> bool {
    reg().args(["query", key, "/v", value]).status().is_ok_and(|s| s.success())
}

/// Writes or removes `value` under `key`, pointing at `exe` with the hidden flag.
#[cfg(windows)]
fn write(key: &str, value: &str, exe: &std::path::Path, on: bool) -> Result<(), String> {
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
#[cfg(windows)]
pub fn enabled() -> bool {
    present(RUN_KEY, VALUE)
}

/// Turns the start at sign-in on or off for this user.
#[cfg(windows)]
pub fn set(on: bool) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    write(RUN_KEY, VALUE, &exe, on)
}

/// Whether the login item exists now; it can be removed outside the app.
#[cfg(any(target_os = "macos", target_os = "linux"))]
pub fn enabled() -> bool {
    entry_path().is_some_and(|p| p.is_file())
}

/// Writes or removes the login item for this user.
#[cfg(any(target_os = "macos", target_os = "linux"))]
pub fn set(on: bool) -> Result<(), String> {
    let path = entry_path().ok_or("Your home folder could not be found, so the login item could not be written.")?;
    let exe = launch_target().map_err(|e| format!("Could not add OpenCompanion to the login items: {e}"))?;
    write_entry(&path, &entry_text(&exe), on)
}

#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
pub fn enabled() -> bool {
    false
}

#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
pub fn set(_on: bool) -> Result<(), String> {
    Err("Starting at sign-in is not available on this system.".into())
}

#[cfg(target_os = "linux")]
fn entry_path() -> Option<std::path::PathBuf> {
    let config = std::env::var_os("XDG_CONFIG_HOME")
        .map(std::path::PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| crate::projects::home().map(|h| h.join(".config")))?;
    Some(config.join("autostart").join("opencompanion.desktop"))
}

#[cfg(target_os = "macos")]
fn entry_path() -> Option<std::path::PathBuf> {
    crate::projects::home().map(|h| h.join("Library").join("LaunchAgents").join("dev.opencompanion.app.plist"))
}

/// What the login item starts: an AppImage runs from a temporary mount, so its own file is used.
#[cfg(any(target_os = "macos", target_os = "linux"))]
fn launch_target() -> std::io::Result<std::path::PathBuf> {
    if let Some(image) = std::env::var_os("APPIMAGE").filter(|_| cfg!(target_os = "linux")) {
        return Ok(image.into());
    }
    std::env::current_exe()
}

#[cfg(target_os = "linux")]
fn entry_text(exe: &Path) -> String {
    desktop_entry(exe)
}

#[cfg(target_os = "macos")]
fn entry_text(exe: &Path) -> String {
    launch_agent(exe)
}

/// An XDG autostart entry. In a quoted `Exec` argument `"`, `` ` `` and `$` take a backslash,
/// a literal backslash is written four times, and `%` is doubled so it is not a field code.
#[cfg(any(target_os = "linux", test))]
fn desktop_entry(exe: &Path) -> String {
    let mut quoted = String::new();
    for c in exe.display().to_string().chars() {
        match c {
            '\\' => quoted.push_str(r"\\\\"),
            '"' | '`' | '$' => {
                quoted.push('\\');
                quoted.push(c);
            }
            '%' => quoted.push_str("%%"),
            _ => quoted.push(c),
        }
    }
    format!(
        "[Desktop Entry]\nType=Application\nName=OpenCompanion\nIcon=opencompanion\nExec=\"{quoted}\" {HIDDEN_ARG}\nTerminal=false\nX-GNOME-Autostart-enabled=true\n"
    )
}

/// A LaunchAgent that starts the app once at login.
#[cfg(any(target_os = "macos", test))]
fn launch_agent(exe: &Path) -> String {
    let xml = |s: &str| s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
         <plist version=\"1.0\">\n\
         <dict>\n  \
         <key>Label</key>\n  <string>dev.opencompanion.app</string>\n  \
         <key>ProgramArguments</key>\n  <array>\n    <string>{}</string>\n    <string>{HIDDEN_ARG}</string>\n  </array>\n  \
         <key>RunAtLoad</key>\n  <true/>\n\
         </dict>\n\
         </plist>\n",
        xml(&exe.display().to_string())
    )
}

/// Writes `text` to `path`, creating its folder, or removes it; removing what is not there is fine.
#[cfg(any(unix, test))]
fn write_entry(path: &Path, text: &str, on: bool) -> Result<(), String> {
    if on {
        let add = |e: std::io::Error| format!("Could not add OpenCompanion to the login items: {e}");
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(add)?;
        }
        std::fs::write(path, text).map_err(add)
    } else if path.exists() {
        std::fs::remove_file(path).map_err(|e| format!("Could not remove OpenCompanion from the login items: {e}"))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_desktop_entry_quotes_the_path_and_starts_hidden() {
        let entry = desktop_entry(Path::new("/opt/Open Companion/o\"c$`\\%"));
        assert!(entry.contains("Exec=\"/opt/Open Companion/o\\\"c\\$\\`\\\\\\\\%%\" --hidden\n"), "{entry}");
        assert!(entry.starts_with("[Desktop Entry]\nType=Application\nName=OpenCompanion\n"), "{entry}");
    }

    #[test]
    fn the_launch_agent_escapes_the_path_and_runs_at_load() {
        let plist = launch_agent(Path::new("/Applications/A&B <x>.app/Contents/MacOS/opencompanion"));
        assert!(plist.contains("<string>/Applications/A&amp;B &lt;x&gt;.app/Contents/MacOS/opencompanion</string>"), "{plist}");
        assert!(plist.contains("<string>--hidden</string>"), "{plist}");
        assert!(plist.contains("<key>RunAtLoad</key>\n  <true/>"), "{plist}");
        assert!(plist.contains("<string>dev.opencompanion.app</string>"), "{plist}");
    }

    #[test]
    fn the_entry_is_written_and_removed_again() {
        let dir = std::env::temp_dir().join(format!("oc-autostart-{}", std::process::id()));
        let path = dir.join("autostart").join("opencompanion.desktop");
        write_entry(&path, "entry", true).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "entry");
        write_entry(&path, "", false).unwrap();
        assert!(!path.exists());
        // Turning off what is already off is not an error.
        write_entry(&path, "", false).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn every_desktop_system_can_start_at_sign_in() {
        assert_eq!(supported(), cfg!(any(windows, target_os = "macos", target_os = "linux")));
    }

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
