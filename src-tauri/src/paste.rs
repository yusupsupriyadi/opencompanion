//! Images pasted into a terminal. xterm pastes text only, so the webview hands the image over
//! and the terminal pastes the saved file's path: the AI CLIs attach a pasted image path as the image.

use std::path::{Path, PathBuf};

/// Writes a pasted image to `dir` and returns its path. Only the types Claude Code and Codex
/// attach from a path are taken.
pub fn save_image(dir: &Path, bytes: &[u8], mime: &str) -> Result<PathBuf, String> {
    let ext = match mime {
        "image/png" => "png",
        "image/jpeg" => "jpg",
        "image/gif" => "gif",
        "image/webp" => "webp",
        _ => return Err("Only PNG, JPEG, GIF and WebP images can be pasted.".into()),
    };
    if bytes.is_empty() {
        return Err("The pasted image is empty.".into());
    }
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let path = dir.join(format!("pasted-{}.{ext}", uuid::Uuid::new_v4().simple()));
    std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saves_each_paste_to_its_own_file() {
        let dir = std::env::temp_dir().join(format!("oc-paste-{}", std::process::id()));
        let a = save_image(&dir, b"\x89PNG one", "image/png").unwrap();
        let b = save_image(&dir, b"jpeg two", "image/jpeg").unwrap();
        assert_ne!(a, b);
        assert_eq!(a.extension().unwrap(), "png");
        assert_eq!(b.extension().unwrap(), "jpg");
        assert_eq!(std::fs::read(&a).unwrap(), b"\x89PNG one");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn refuses_other_types_and_empty_images() {
        let dir = std::env::temp_dir().join(format!("oc-paste-refused-{}", std::process::id()));
        assert!(save_image(&dir, b"<svg/>", "image/svg+xml").is_err());
        assert!(save_image(&dir, b"", "image/png").is_err());
        assert!(!dir.exists());
    }
}
