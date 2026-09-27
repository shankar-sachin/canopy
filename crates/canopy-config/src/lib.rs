//! Settings and themes shared by the Canopy terminal app and Canopy Desktop.
//!
//! Both apps keep their own settings (`config.toml` for the terminal app,
//! `desktop.json` for the desktop app, both in `~/.canopy/`), and trade the
//! settings they have in common through [`settings::Shared`]. Custom themes
//! are JSON files in `~/.canopy/themes/` that both read.

pub mod settings;
pub mod theme;

use std::path::{Path, PathBuf};
use std::sync::Once;

fn home() -> Option<PathBuf> {
    std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(PathBuf::from)
}

/// Canopy's folder: `~/.canopy` on every platform, or `$CANOPY_HOME`.
///
/// Before 1.0.7 it was `~/.canopy` (or `$XDG_CONFIG_HOME/canopy`);
/// the first time this runs, whatever is there is copied over (and the old
/// folder is left alone).
pub fn config_dir() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("CANOPY_HOME").filter(|p| !p.is_empty()) {
        return Some(PathBuf::from(p));
    }
    let home = home()?;
    let dir = home.join(".canopy");
    static MOVE: Once = Once::new();
    MOVE.call_once(|| {
        let old = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".config"))
            .join("canopy");
        let _ = move_old(&old, &dir);
    });
    Some(dir)
}

/// Copy the pre-1.0.7 folder to the new place, once (only when the new one
/// doesn't exist yet).
fn move_old(old: &Path, new: &Path) -> std::io::Result<bool> {
    if new.exists() || !old.is_dir() {
        return Ok(false);
    }
    copy_dir(old, new)?;
    Ok(true)
}

fn copy_dir(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)?.flatten() {
        let (src, dst) = (entry.path(), to.join(entry.file_name()));
        if src.is_dir() {
            copy_dir(&src, &dst)?;
        } else {
            std::fs::copy(&src, &dst)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_folder_is_copied_once() {
        let t = tempfile::tempdir().unwrap();
        let (old, new) = (t.path().join(".config/canopy"), t.path().join(".canopy"));
        std::fs::create_dir_all(old.join("themes")).unwrap();
        std::fs::write(old.join("config.toml"), "theme = \"nord\"\n").unwrap();
        std::fs::write(old.join("themes/ocean.json"), "{}").unwrap();
        assert!(move_old(&old, &new).unwrap());
        assert_eq!(std::fs::read_to_string(new.join("config.toml")).unwrap(), "theme = \"nord\"\n");
        assert!(new.join("themes/ocean.json").is_file());
        assert!(old.join("config.toml").is_file(), "the old folder stays");
        // Never again, even if the old one changes.
        std::fs::write(old.join("config.toml"), "changed").unwrap();
        assert!(!move_old(&old, &new).unwrap());
        assert_ne!(std::fs::read_to_string(new.join("config.toml")).unwrap(), "changed");
    }
}
