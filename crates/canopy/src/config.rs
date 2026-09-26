use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::Deserialize;

/// `~/.config/canopy/config.toml`
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Config {
    /// canopy | catppuccin | gruvbox | nord | light
    pub theme: String,
    /// Use Nerd Font glyphs. Set false for plain ASCII/Unicode.
    pub nerd_font: bool,
    /// Show the git command behind every action.
    pub teach_mode: bool,
    /// Ask before destructive actions (discard, hard reset, force delete...).
    pub confirm_destructive: bool,
    /// Directories scanned for repositories in the Workspace view.
    pub workspace_dirs: Vec<String>,
    /// How deep to look for repositories under each workspace dir.
    pub workspace_depth: usize,
    /// User-defined commands bound to keys.
    pub custom_commands: Vec<CustomCommand>,
    /// Play the short tree animation when Canopy starts.
    pub splash: bool,
    /// Tighter layout: less padding and no gaps between panels.
    pub compact: bool,
    /// Show modifier keys as ⌃ ⌥ ⇧ (default on macOS) instead of ctrl-/alt-.
    pub mac_key_symbols: Option<bool>,
    /// Path to the GitHub CLI, if not `gh` on PATH.
    pub gh_program: Option<String>,
    /// Key remaps: action name -> key or list of keys.
    pub keys: BTreeMap<String, KeySpec>,
    /// Number of commits loaded per log page.
    pub log_page_size: usize,
}

/// `commit = "C"` or `commit = ["C", "ctrl-s"]`
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum KeySpec {
    One(String),
    Many(Vec<String>),
}

#[derive(Debug, Clone, Deserialize)]
pub struct CustomCommand {
    /// Single character key, e.g. "X".
    pub key: String,
    /// Shell command; runs in the repository root via `sh -c`.
    pub cmd: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub confirm: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            theme: "canopy".into(),
            nerd_font: false,
            teach_mode: true,
            confirm_destructive: true,
            workspace_dirs: Vec::new(),
            workspace_depth: 3,
            custom_commands: Vec::new(),
            keys: BTreeMap::new(),
            gh_program: None,
            mac_key_symbols: None,
            compact: false,
            splash: true,
            log_page_size: 300,
        }
    }
}

impl Config {
    pub fn key_overrides(&self) -> BTreeMap<String, Vec<String>> {
        self.keys
            .iter()
            .map(|(k, v)| {
                let keys = match v {
                    KeySpec::One(s) => vec![s.clone()],
                    KeySpec::Many(v) => v.clone(),
                };
                (k.clone(), keys)
            })
            .collect()
    }

    pub fn path() -> Option<PathBuf> {
        if let Ok(p) = std::env::var("CANOPY_CONFIG") {
            return Some(PathBuf::from(p));
        }
        let home = directories::BaseDirs::new()?.home_dir().to_path_buf();
        // Prefer XDG-style ~/.config on every platform: that's what terminal users expect.
        let xdg = std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from).unwrap_or_else(|| home.join(".config"));
        Some(xdg.join("canopy").join("config.toml"))
    }

    /// Load the config, returning a warning message if it was invalid.
    pub fn load() -> (Config, Option<String>) {
        let Some(path) = Self::path() else { return (Config::default(), None) };
        match std::fs::read_to_string(&path) {
            Ok(text) => match toml::from_str(&text) {
                Ok(c) => (c, None),
                Err(e) => (Config::default(), Some(format!("{}: {e}", path.display()))),
            },
            Err(_) => (Config::default(), None),
        }
    }

    pub fn first_run() -> bool {
        Self::path().map(|p| !p.exists()).unwrap_or(false)
    }

    pub const EXAMPLE: &'static str = r#"# Canopy configuration
theme = "canopy"            # canopy | catppuccin | gruvbox | nord | light
nerd_font = false           # true if your terminal font has Nerd Font glyphs
teach_mode = true           # show the git command behind every action
confirm_destructive = true
compact = false             # true: tighter layout, no gaps between panels
splash = true               # the tree animation when Canopy starts
workspace_dirs = ["~/code"] # scanned by the Workspace view (tab 6)
workspace_depth = 3

# Remap any action. Names are snake_case: toggle_stage, commit, push,
# goto_history, stage_line, ... (press ? in Canopy to see actions).
# [keys]
# toggle_stage = "s"
# commit = ["c", "ctrl-s"]

# [[custom_commands]]
# key = "X"
# cmd = "git push --force-with-lease"
# description = "force push (safely)"
# confirm = true
"#;
}

pub fn expand_tilde(p: &str) -> PathBuf {
    if let Some(rest) = p.strip_prefix("~/") {
        if let Some(b) = directories::BaseDirs::new() {
            return b.home_dir().join(rest);
        }
    }
    PathBuf::from(p)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn example_config_parses() {
        let c: Config = toml::from_str(Config::EXAMPLE).unwrap();
        assert_eq!(c.theme, "canopy");
        assert_eq!(c.workspace_dirs, vec!["~/code"]);
    }

    #[test]
    fn custom_commands_parse() {
        let c: Config = toml::from_str("[[custom_commands]]\nkey = \"X\"\ncmd = \"echo hi\"\n").unwrap();
        assert_eq!(c.custom_commands[0].key, "X");
        let c: Config = toml::from_str("[keys]\ncommit = \"C\"\npush = [\"P\", \"ctrl-p\"]\n").unwrap();
        let o = c.key_overrides();
        assert_eq!(o["commit"], vec!["C"]);
        assert_eq!(o["push"], vec!["P", "ctrl-p"]);
        assert!(c.teach_mode);
    }
}
