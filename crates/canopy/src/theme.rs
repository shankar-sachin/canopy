use ratatui::style::{Color, Modifier, Style};

/// Semantic colour palette. Views never use raw colours directly.
/// Semantic colour palette. Views never use raw colours directly.
///
/// The colors come from canopy-config, which the desktop app shares: the
/// built-in palettes, and custom themes as JSON files in
/// `~/.canopy/themes/`.
#[derive(Debug, Clone)]
pub struct Theme {
    pub name: String,
    pub bg: Color,
    pub fg: Color,
    pub muted: Color,
    pub border: Color,
    pub border_focus: Color,
    pub accent: Color,
    pub accent_alt: Color,
    pub selection_bg: Color,
    pub added: Color,
    pub removed: Color,
    pub modified: Color,
    pub conflict: Color,
    pub hash: Color,
    pub branch: Color,
    pub remote: Color,
    pub tag: Color,
    pub warn: Color,
    pub error: Color,
    pub added_bg: Color,
    pub removed_bg: Color,
}

impl Theme {
    /// A built-in theme, or a custom one from the themes folder; the default
    /// when there's no theme by that name.
    pub fn by_name(name: &str) -> Theme {
        Self::find(name).unwrap_or_else(Self::canopy)
    }

    pub fn find(name: &str) -> Option<Theme> {
        let dir = canopy_config::theme::themes_dir();
        canopy_config::theme::find(name, dir.as_deref()).map(|p| Self::from_palette(&p))
    }

    /// The built-in theme names.
    pub const NAMES: [&'static str; 5] = ["canopy", "catppuccin", "gruvbox", "nord", "light"];

    /// Built-ins, then custom themes, for cycling with ctrl-t.
    pub fn all_names() -> Vec<String> {
        let mut names: Vec<String> = Self::NAMES.iter().map(|n| n.to_string()).collect();
        if let Some(dir) = canopy_config::theme::themes_dir() {
            names.extend(canopy_config::theme::list_custom(&dir).into_iter().map(|(_, t)| t.name));
        }
        names
    }

    /// Default: deep forest greens with warm highlights.
    pub fn canopy() -> Theme {
        Self::from_palette(&canopy_config::theme::Palette::builtin("canopy").expect("built in"))
    }

    pub fn from_palette(p: &canopy_config::theme::Palette) -> Theme {
        let c = |f: &str| {
            let [r, g, b] = p.get(f);
            Color::Rgb(r, g, b)
        };
        Theme {
            name: p.name.clone(),
            bg: c("bg"),
            fg: c("fg"),
            muted: c("muted"),
            border: c("border"),
            border_focus: c("border_focus"),
            accent: c("accent"),
            accent_alt: c("accent_alt"),
            selection_bg: c("selection_bg"),
            added: c("added"),
            removed: c("removed"),
            modified: c("modified"),
            conflict: c("conflict"),
            hash: c("hash"),
            branch: c("branch"),
            remote: c("remote"),
            tag: c("tag"),
            warn: c("warn"),
            error: c("error"),
            added_bg: c("added_bg"),
            removed_bg: c("removed_bg"),
        }
    }

    pub fn base(&self) -> Style {
        Style::default().fg(self.fg).bg(self.bg)
    }
    pub fn muted(&self) -> Style {
        Style::default().fg(self.muted)
    }
    pub fn accent(&self) -> Style {
        Style::default().fg(self.accent).add_modifier(Modifier::BOLD)
    }
    pub fn selected(&self) -> Style {
        Style::default().bg(self.selection_bg).add_modifier(Modifier::BOLD)
    }
    pub fn fg(&self, c: Color) -> Style {
        Style::default().fg(c)
    }
    pub fn key(&self) -> Style {
        Style::default().fg(self.bg).bg(self.accent).add_modifier(Modifier::BOLD)
    }
}
