//! Recently opened repositories, kept in `~/.config/canopy/desktop.json`
//! (next to the TUI's config.toml).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

const MAX: usize = 12;

#[derive(Debug, Default, Serialize, Deserialize)]
struct Store {
    #[serde(default)]
    recent: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RecentRepo {
    pub path: String,
    /// `path` with the home folder shortened to `~`.
    pub display: String,
    pub name: String,
    /// False when the folder has been moved or deleted since.
    pub exists: bool,
}

pub fn store_path() -> Option<PathBuf> {
    let home = home()?;
    let base = std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from).unwrap_or_else(|| home.join(".config"));
    Some(base.join("canopy").join("desktop.json"))
}

fn read(file: &Path) -> Store {
    std::fs::read_to_string(file).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

fn home() -> Option<PathBuf> {
    std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(PathBuf::from)
}

/// `/Users/ada/code/x` → `~/code/x`.
pub fn tilde(path: &str, home: Option<&Path>) -> String {
    match home.and_then(|h| Path::new(path).strip_prefix(h).ok()) {
        Some(rest) if rest.as_os_str().is_empty() => "~".into(),
        Some(rest) => format!("~{}{}", std::path::MAIN_SEPARATOR, rest.display()),
        None => path.to_string(),
    }
}

pub fn list(file: &Path) -> Vec<RecentRepo> {
    let home = home();
    read(file)
        .recent
        .into_iter()
        .map(|path| {
            let p = Path::new(&path);
            RecentRepo {
                display: tilde(&path, home.as_deref()),
                name: p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| path.clone()),
                exists: p.is_dir(),
                path,
            }
        })
        .collect()
}

/// Move `path` to the front of the list.
pub fn add(file: &Path, path: &str) -> std::io::Result<()> {
    let mut store = read(file);
    store.recent.retain(|p| p != path);
    store.recent.insert(0, path.to_string());
    store.recent.truncate(MAX);
    write(file, &store)
}

pub fn remove(file: &Path, path: &str) -> std::io::Result<()> {
    let mut store = read(file);
    store.recent.retain(|p| p != path);
    write(file, &store)
}

fn write(file: &Path, store: &Store) -> std::io::Result<()> {
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(file, serde_json::to_string_pretty(store)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tilde_paths() {
        let home = Path::new("/Users/ada");
        assert_eq!(tilde("/Users/ada/code/x", Some(home)), format!("~{}code/x", std::path::MAIN_SEPARATOR));
        assert_eq!(tilde("/Users/ada", Some(home)), "~");
        assert_eq!(tilde("/srv/x", Some(home)), "/srv/x");
        assert_eq!(tilde("/Users/adam/x", Some(home)), "/Users/adam/x");
    }

    #[test]
    fn most_recent_first_without_duplicates() {
        let dir = tempfile::TempDir::new().unwrap();
        let file = dir.path().join("sub/desktop.json");
        assert!(list(&file).is_empty());
        let real = dir.path().display().to_string();
        add(&file, "/nope/a").unwrap();
        add(&file, &real).unwrap();
        add(&file, "/nope/a").unwrap();
        let got = list(&file);
        assert_eq!(got.iter().map(|r| r.path.as_str()).collect::<Vec<_>>(), ["/nope/a", real.as_str()]);
        assert!(!got[0].exists && got[1].exists);
        assert_eq!(got[0].name, "a");
        remove(&file, "/nope/a").unwrap();
        assert_eq!(list(&file).len(), 1);
        for i in 0..20 {
            add(&file, &format!("/r/{i}")).unwrap();
        }
        assert_eq!(list(&file).len(), MAX);
    }
}
