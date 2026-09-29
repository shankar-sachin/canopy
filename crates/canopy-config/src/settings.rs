//! The settings both apps have, as one JSON file to move between them.

use std::path::Path;

use serde::{Deserialize, Serialize};

/// Settings canopy console and the desktop app share. Missing fields are
/// left alone when importing.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Shared {
    /// Always "canopy-settings", so a random JSON file isn't mistaken for one.
    #[serde(default)]
    pub kind: String,
    /// A theme name: built-in or custom (a file in ~/.canopy/themes).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme: Option<String>,
    /// A custom theme carried along, so it works on another computer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme_file: Option<crate::theme::ThemeFile>,
    /// The text editor command (like "code --wait" or "vim").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub editor: Option<String>,
    /// Fix with AI: auto, claude, codex, off, or a command.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ai: Option<String>,
    /// Show the git command behind each action.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub show_commands: Option<bool>,
}

pub const KIND: &str = "canopy-settings";

impl Shared {
    pub fn parse(json: &str) -> Result<Shared, String> {
        let s: Shared = serde_json::from_str(json).map_err(|e| format!("not a canopy settings file: {e}"))?;
        if s.kind != KIND {
            return Err(format!("not a canopy settings file (\"kind\" should be \"{KIND}\")"));
        }
        if let Some(t) = &s.theme_file {
            t.resolve()?;
        }
        Ok(s)
    }

    pub fn to_json(&self) -> String {
        let mut s = self.clone();
        s.kind = KIND.into();
        serde_json::to_string_pretty(&s).unwrap_or_default() + "\n"
    }

    /// The shared settings in canopy console's config.toml text.
    pub fn from_toml(text: &str) -> Result<Shared, String> {
        let v: toml::Table = toml::from_str(text).map_err(|e| format!("config.toml: {e}"))?;
        let s = |k: &str| v.get(k).and_then(|x| x.as_str()).map(String::from);
        Ok(Shared {
            kind: KIND.into(),
            theme: s("theme"),
            theme_file: None,
            editor: s("editor"),
            ai: s("ai"),
            show_commands: v.get("teach_mode").and_then(|x| x.as_bool()),
        })
    }

    /// Write these settings into config.toml text, changing only their keys
    /// and keeping everything else (comments included).
    pub fn apply_to_toml(&self, text: &str) -> String {
        let mut keys: Vec<(&str, String)> = Vec::new();
        if let Some(t) = &self.theme {
            keys.push(("theme", toml_string(t)));
        }
        if let Some(e) = &self.editor {
            keys.push(("editor", toml_string(e)));
        }
        if let Some(a) = &self.ai {
            keys.push(("ai", toml_string(a)));
        }
        if let Some(b) = self.show_commands {
            keys.push(("teach_mode", b.to_string()));
        }
        set_top_level_keys(text, &keys)
    }
}

/// `s` as a TOML basic string. Control characters are escaped too: a raw
/// newline or tab would make config.toml unreadable.
fn toml_string(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            c if c.is_control() => out.push_str(&format!("\\u{:04X}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Set top-level `key = value` lines in TOML text (before the first
/// `[table]`), replacing existing ones in place and adding new ones at the
/// end of the top-level section. Comments after a value are dropped only on
/// replaced lines.
pub fn set_top_level_keys(text: &str, keys: &[(&str, String)]) -> String {
    let mut lines: Vec<String> = text.lines().map(String::from).collect();
    let table_start = lines.iter().position(|l| l.trim_start().starts_with('[')).unwrap_or(lines.len());
    let mut missing = Vec::new();
    for (key, value) in keys {
        let found = lines[..table_start].iter().position(|l| {
            let t = l.trim_start();
            !t.starts_with('#') && t.split('=').next().is_some_and(|k| k.trim() == *key) && t.contains('=')
        });
        match found {
            Some(i) => lines[i] = format!("{key} = {value}"),
            None => missing.push(format!("{key} = {value}")),
        }
    }
    if !missing.is_empty() {
        // Before the first table (and the blank lines above it).
        let mut at = table_start;
        while at > 0 && lines[at - 1].trim().is_empty() {
            at -= 1;
        }
        for (i, l) in missing.into_iter().enumerate() {
            lines.insert(at + i, l);
        }
    }
    let mut out = lines.join("\n");
    out.push('\n');
    out
}

/// Read a shared settings file.
pub fn read(path: &Path) -> Result<Shared, String> {
    Shared::parse(&std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toml_round_trip_keeps_comments() {
        let text = "# My config\ntheme = \"nord\"   # cool\nteach_mode = true\n\n[keys]\ncommit = \"C\"\n";
        let s = Shared::from_toml(text).unwrap();
        assert_eq!(s.theme.as_deref(), Some("nord"));
        assert_eq!(s.show_commands, Some(true));
        assert_eq!(s.editor, None);
        let new = Shared {
            theme: Some("midnight".into()),
            editor: Some("code --wait".into()),
            show_commands: Some(false),
            ..Default::default()
        };
        let out = new.apply_to_toml(text);
        assert_eq!(
            out,
            "# My config\ntheme = \"midnight\"\nteach_mode = false\neditor = \"code --wait\"\n\n[keys]\ncommit = \"C\"\n"
        );
        // Nothing else changes when nothing is set; an empty file gets the keys.
        assert_eq!(Shared::default().apply_to_toml(text), text);
        assert_eq!(Shared { ai: Some("codex".into()), ..Default::default() }.apply_to_toml(""), "ai = \"codex\"\n");
        // A key inside a table isn't touched.
        let t2 = "[keys]\ntheme = \"x\"\n";
        assert_eq!(
            Shared { theme: Some("y".into()), ..Default::default() }.apply_to_toml(t2),
            "theme = \"y\"\n[keys]\ntheme = \"x\"\n"
        );
    }

    #[test]
    fn written_values_are_valid_toml() {
        let s = Shared { editor: Some("my \"ed\"\tx\\y\nz\u{7}".into()), ..Default::default() };
        let back = Shared::from_toml(&s.apply_to_toml("")).unwrap();
        assert_eq!(back.editor, s.editor);
    }

    #[test]
    fn json_files() {
        let s = Shared { theme: Some("nord".into()), ai: Some("claude".into()), ..Default::default() };
        let json = s.to_json();
        assert!(json.contains("\"kind\": \"canopy-settings\""));
        assert_eq!(Shared::parse(&json).unwrap().ai.as_deref(), Some("claude"));
        assert!(Shared::parse("{\"theme\": \"x\"}").is_err());
        assert!(Shared::parse("[]").is_err());
    }
}
