//! Colors shared by the terminal app and the desktop app: the built-in
//! palettes, and custom themes as JSON files in `~/.canopy/themes/`.
//!
//! A theme file names a base palette and overrides any of its colors:
//!
//! ```json
//! { "name": "midnight", "base": "canopy", "colors": { "bg": "#0d1117", "accent": "#58a6ff" } }
//! ```

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Every color a palette has, in order.
pub const FIELDS: [&str; 20] = [
    "bg",
    "fg",
    "muted",
    "border",
    "border_focus",
    "accent",
    "accent_alt",
    "selection_bg",
    "added",
    "removed",
    "modified",
    "conflict",
    "hash",
    "branch",
    "remote",
    "tag",
    "warn",
    "error",
    "added_bg",
    "removed_bg",
];

/// What each color is for (shown by editors).
pub fn describe(field: &str) -> &'static str {
    match field {
        "bg" => "Background",
        "fg" => "Text",
        "muted" => "Muted text",
        "border" => "Borders",
        "border_focus" => "Focused border",
        "accent" => "Accent",
        "accent_alt" => "Second accent",
        "selection_bg" => "Selection",
        "added" => "Added lines",
        "removed" => "Removed lines",
        "modified" => "Modified files",
        "conflict" => "Conflicts",
        "hash" => "Commit hashes",
        "branch" => "Branches",
        "remote" => "Remotes",
        "tag" => "Tags",
        "warn" => "Warnings",
        "error" => "Errors",
        "added_bg" => "Added line background",
        "removed_bg" => "Removed line background",
        _ => "",
    }
}

const BUILTIN: [(&str, &[(&str, &str)]); 5] = [
    (
        "canopy",
        &[
            ("bg", "#101612"),
            ("fg", "#d6e2d6"),
            ("muted", "#708474"),
            ("border", "#2e4032"),
            ("border_focus", "#7ac878"),
            ("accent", "#7ac878"),
            ("accent_alt", "#e6be6e"),
            ("selection_bg", "#243828"),
            ("added", "#7ac878"),
            ("removed", "#e86e64"),
            ("modified", "#e6be6e"),
            ("conflict", "#f078c8"),
            ("hash", "#a096e6"),
            ("branch", "#6ec8c8"),
            ("remote", "#78a0e6"),
            ("tag", "#e6be6e"),
            ("warn", "#e6be6e"),
            ("error", "#e86e64"),
            ("added_bg", "#18301c"),
            ("removed_bg", "#381a1a"),
        ],
    ),
    (
        "catppuccin",
        &[
            ("bg", "#1e1e2e"),
            ("fg", "#cdd6f4"),
            ("muted", "#7f849c"),
            ("border", "#45475a"),
            ("border_focus", "#cba6f7"),
            ("accent", "#cba6f7"),
            ("accent_alt", "#89b4fa"),
            ("selection_bg", "#313244"),
            ("added", "#a6e3a1"),
            ("removed", "#f38ba8"),
            ("modified", "#f9e2af"),
            ("conflict", "#f5c2e7"),
            ("hash", "#b4befe"),
            ("branch", "#94e2d5"),
            ("remote", "#89b4fa"),
            ("tag", "#fab387"),
            ("warn", "#f9e2af"),
            ("error", "#f38ba8"),
            ("added_bg", "#263830"),
            ("removed_bg", "#3e2634"),
        ],
    ),
    (
        "gruvbox",
        &[
            ("bg", "#282828"),
            ("fg", "#ebdbb2"),
            ("muted", "#928374"),
            ("border", "#504945"),
            ("border_focus", "#fabd2f"),
            ("accent", "#fabd2f"),
            ("accent_alt", "#83a598"),
            ("selection_bg", "#3c3836"),
            ("added", "#b8bb26"),
            ("removed", "#fb4934"),
            ("modified", "#fabd2f"),
            ("conflict", "#d3869b"),
            ("hash", "#d3869b"),
            ("branch", "#8ec07c"),
            ("remote", "#83a598"),
            ("tag", "#fe8019"),
            ("warn", "#fe8019"),
            ("error", "#fb4934"),
            ("added_bg", "#32361e"),
            ("removed_bg", "#422420"),
        ],
    ),
    (
        "nord",
        &[
            ("bg", "#2e3440"),
            ("fg", "#d8dee9"),
            ("muted", "#768094"),
            ("border", "#434c5e"),
            ("border_focus", "#88c0d0"),
            ("accent", "#88c0d0"),
            ("accent_alt", "#b48ead"),
            ("selection_bg", "#3b4252"),
            ("added", "#a3be8c"),
            ("removed", "#bf616a"),
            ("modified", "#ebcb8b"),
            ("conflict", "#b48ead"),
            ("hash", "#b48ead"),
            ("branch", "#8fbcbb"),
            ("remote", "#81a1c1"),
            ("tag", "#d08770"),
            ("warn", "#ebcb8b"),
            ("error", "#bf616a"),
            ("added_bg", "#36423c"),
            ("removed_bg", "#46363e"),
        ],
    ),
    (
        "light",
        &[
            ("bg", "#fafaf6"),
            ("fg", "#282c28"),
            ("muted", "#788078"),
            ("border", "#ced4cc"),
            ("border_focus", "#2e8c46"),
            ("accent", "#2e8c46"),
            ("accent_alt", "#b06e14"),
            ("selection_bg", "#deecde"),
            ("added", "#288c3c"),
            ("removed", "#c83232"),
            ("modified", "#b06e14"),
            ("conflict", "#b43296"),
            ("hash", "#6450be"),
            ("branch", "#14828c"),
            ("remote", "#285ab4"),
            ("tag", "#b06e14"),
            ("warn", "#b06e14"),
            ("error", "#c83232"),
            ("added_bg", "#dcf4de"),
            ("removed_bg", "#fadede"),
        ],
    ),
];

/// Names of the built-in palettes.
pub fn builtin_names() -> Vec<&'static str> {
    BUILTIN.iter().map(|(n, _)| *n).collect()
}

/// A full palette: every field in FIELDS as an RGB color.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Palette {
    pub name: String,
    pub colors: BTreeMap<String, [u8; 3]>,
}

impl Palette {
    pub fn builtin(name: &str) -> Option<Palette> {
        let (n, cols) = BUILTIN.iter().find(|(n, _)| *n == name)?;
        let colors =
            cols.iter().map(|(k, v)| (k.to_string(), parse_hex(v).expect("built-in colors are valid"))).collect();
        Some(Palette { name: n.to_string(), colors })
    }

    pub fn get(&self, field: &str) -> [u8; 3] {
        self.colors.get(field).copied().unwrap_or([128, 128, 128])
    }

    /// Dark when the background is darker than middle grey.
    pub fn is_dark(&self) -> bool {
        let [r, g, b] = self.get("bg");
        (0.2126 * r as f32 + 0.7152 * g as f32 + 0.0722 * b as f32) < 128.0
    }

    /// As a theme file with every color written out.
    pub fn to_file(&self, name: &str, base: &str) -> ThemeFile {
        ThemeFile {
            name: name.to_string(),
            base: base.to_string(),
            colors: self.colors.iter().map(|(k, v)| (k.clone(), to_hex(*v))).collect(),
        }
    }
}

/// A theme as saved in a JSON file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThemeFile {
    pub name: String,
    /// The built-in palette it starts from.
    #[serde(default = "default_base")]
    pub base: String,
    /// Colors that differ from the base, as "#rrggbb".
    #[serde(default)]
    pub colors: BTreeMap<String, String>,
}

fn default_base() -> String {
    "canopy".into()
}

impl ThemeFile {
    /// The full palette: the base with these colors on top. Errors name the
    /// first bad key or color.
    pub fn resolve(&self) -> Result<Palette, String> {
        let mut p = Palette::builtin(&self.base).ok_or_else(|| {
            format!("unknown base theme {:?} (use one of: {})", self.base, builtin_names().join(", "))
        })?;
        for (k, v) in &self.colors {
            if !FIELDS.contains(&k.as_str()) {
                return Err(format!("unknown color {k:?} (known: {})", FIELDS.join(", ")));
            }
            let rgb = parse_hex(v).ok_or_else(|| format!("{k}: {v:?} isn't a color like \"#1e2a22\""))?;
            p.colors.insert(k.clone(), rgb);
        }
        p.name = self.name.clone();
        Ok(p)
    }

    pub fn parse(json: &str) -> Result<ThemeFile, String> {
        let t: ThemeFile = serde_json::from_str(json).map_err(|e| format!("not a Canopy theme: {e}"))?;
        if t.name.trim().is_empty() {
            return Err("the theme needs a \"name\"".into());
        }
        t.resolve()?;
        Ok(t)
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default() + "\n"
    }
}

pub fn parse_hex(s: &str) -> Option<[u8; 3]> {
    let h = s.trim().strip_prefix('#')?;
    let h: String = if h.len() == 3 { h.chars().flat_map(|c| [c, c]).collect() } else { h.to_string() };
    // Only hex digits (from_str_radix would also take a leading "+").
    if h.len() != 6 || !h.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let v = u32::from_str_radix(&h, 16).ok()?;
    Some([(v >> 16) as u8, (v >> 8) as u8, v as u8])
}

pub fn to_hex([r, g, b]: [u8; 3]) -> String {
    format!("#{r:02x}{g:02x}{b:02x}")
}

/// `~/.canopy/themes`.
pub fn themes_dir() -> Option<PathBuf> {
    crate::config_dir().map(|d| d.join("themes"))
}

/// A safe file name for a theme: lowercase letters, digits and dashes.
pub fn slug(name: &str) -> String {
    let s: String = name
        .trim()
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    if s.is_empty() {
        "theme".into()
    } else {
        s
    }
}

/// Custom themes in `dir`, sorted by name. Broken files are skipped.
pub fn list_custom(dir: &Path) -> Vec<(PathBuf, ThemeFile)> {
    let mut out: Vec<(PathBuf, ThemeFile)> = std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|x| x == "json"))
                .filter_map(|p| {
                    let t = ThemeFile::parse(&std::fs::read_to_string(&p).ok()?).ok()?;
                    Some((p, t))
                })
                .collect()
        })
        .unwrap_or_default();
    out.sort_by_key(|a| a.1.name.to_lowercase());
    out
}

/// Save `theme` as `<dir>/<slug>.json`; returns the path. Saving a theme
/// again replaces its file, but a different theme whose name has the same
/// slug ("Café" and "Cafe") gets its own file (`cafe-2.json`) instead of
/// overwriting the other one.
pub fn save(dir: &Path, theme: &ThemeFile) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let base = slug(&theme.name);
    let same_theme = |p: &Path| {
        std::fs::read_to_string(p)
            .ok()
            .and_then(|j| serde_json::from_str::<ThemeFile>(&j).ok())
            .is_some_and(|t| t.name.trim().eq_ignore_ascii_case(theme.name.trim()))
    };
    let path = (1..)
        .map(|i| dir.join(if i == 1 { format!("{base}.json") } else { format!("{base}-{i}.json") }))
        .find(|p| !p.exists() || same_theme(p))
        .expect("an unused file name");
    std::fs::write(&path, theme.to_json())?;
    Ok(path)
}

/// A theme by name: built-in, or a custom one in `dir` (by name or file
/// name). None when there's no such theme.
pub fn find(name: &str, dir: Option<&Path>) -> Option<Palette> {
    if let Some(p) = Palette::builtin(name) {
        return Some(p);
    }
    custom_file(name, dir?).and_then(|t| t.resolve().ok())
}

/// A custom theme's file as saved (its base and only the colors it
/// changes), by name or file name.
pub fn custom_file(name: &str, dir: &Path) -> Option<ThemeFile> {
    let list = list_custom(dir);
    // A theme's own name wins over another theme's file name.
    let by_name = list.iter().position(|(_, t)| t.name.trim().eq_ignore_ascii_case(name.trim()));
    let by_file = || list.iter().position(|(path, _)| path.file_stem().is_some_and(|s| s == slug(name).as_str()));
    by_name.or_else(by_file).map(|i| list[i].1.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtins_are_complete() {
        for n in builtin_names() {
            let p = Palette::builtin(n).unwrap();
            assert_eq!(p.colors.len(), FIELDS.len(), "{n}");
        }
        assert!(Palette::builtin("canopy").unwrap().is_dark());
        assert!(!Palette::builtin("light").unwrap().is_dark());
    }

    #[test]
    fn theme_files() {
        let t = ThemeFile::parse(
            r##"{"name": "Midnight Blue", "base": "nord", "colors": {"accent": "#58a6ff", "bg": "#0d1"}}"##,
        )
        .unwrap();
        let p = t.resolve().unwrap();
        assert_eq!(p.get("accent"), [0x58, 0xa6, 0xff]);
        assert_eq!(p.get("bg"), [0x00, 0xdd, 0x11]);
        assert_eq!(p.get("fg"), Palette::builtin("nord").unwrap().get("fg"));
        assert!(ThemeFile::parse(r##"{"name": "x", "colors": {"acent": "#000000"}}"##)
            .unwrap_err()
            .contains("unknown color"));
        assert!(ThemeFile::parse(r#"{"name": "x", "colors": {"bg": "red"}}"#).unwrap_err().contains("isn't a color"));
        assert_eq!(parse_hex("#+12345"), None);
        assert_eq!(parse_hex("#-1-2-3"), None);
        assert!(ThemeFile::parse(r#"{"name": "x", "base": "dracula"}"#).unwrap_err().contains("unknown base"));
        assert!(ThemeFile::parse("nope").is_err());
        // A full export parses back to the same palette.
        let full = Palette::builtin("gruvbox").unwrap().to_file("Mine", "gruvbox");
        assert_eq!(
            ThemeFile::parse(&full.to_json()).unwrap().resolve().unwrap().colors,
            Palette::builtin("gruvbox").unwrap().colors
        );
    }

    #[test]
    fn save_list_find() {
        let dir = tempfile::TempDir::new().unwrap();
        let t = ThemeFile {
            name: "Midnight Blue".into(),
            base: "canopy".into(),
            colors: BTreeMap::from([("accent".into(), "#58a6ff".into())]),
        };
        let path = save(dir.path(), &t).unwrap();
        assert_eq!(path.file_name().unwrap(), "midnight-blue.json");
        std::fs::write(dir.path().join("broken.json"), "{").unwrap();
        let list = list_custom(dir.path());
        assert_eq!(list.len(), 1);
        assert_eq!(find("midnight blue", Some(dir.path())).unwrap().get("accent"), [0x58, 0xa6, 0xff]);
        assert_eq!(find("midnight-blue", Some(dir.path())).unwrap().name, "Midnight Blue");
        assert_eq!(find("nord", None).unwrap().name, "nord");
        assert!(find("nope", Some(dir.path())).is_none());
        assert_eq!(slug("  My Theme!! v2 "), "my-theme-v2");
    }

    #[test]
    fn saving_never_overwrites_a_different_theme() {
        let dir = tempfile::TempDir::new().unwrap();
        let theme = |name: &str, accent: &str| ThemeFile {
            name: name.into(),
            base: "canopy".into(),
            colors: BTreeMap::from([("accent".into(), accent.into())]),
        };
        let a = save(dir.path(), &theme("My Theme", "#111111")).unwrap();
        // Same slug, different theme: a file of its own.
        let b = save(dir.path(), &theme("My-Theme", "#222222")).unwrap();
        assert_eq!(a.file_name().unwrap(), "my-theme.json");
        assert_eq!(b.file_name().unwrap(), "my-theme-2.json");
        // Saving a theme again (an edit, any case) replaces its own file.
        assert_eq!(save(dir.path(), &theme("MY THEME", "#333333")).unwrap(), a);
        assert_eq!(save(dir.path(), &theme("My-Theme", "#444444")).unwrap(), b);
        assert_eq!(list_custom(dir.path()).len(), 2);
        assert_eq!(find("My-Theme", Some(dir.path())).unwrap().get("accent"), [0x44; 3]);
        assert_eq!(find("my theme", Some(dir.path())).unwrap().get("accent"), [0x33; 3]);
        assert_eq!(slug("!!!"), "theme");
    }
}
