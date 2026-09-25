use ratatui::style::{Color, Modifier, Style};

/// Semantic colour palette. Views never use raw colours directly.
#[derive(Debug, Clone)]
pub struct Theme {
    pub name: &'static str,
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
    pub fn by_name(name: &str) -> Theme {
        match name {
            "catppuccin" => Self::catppuccin(),
            "gruvbox" => Self::gruvbox(),
            "nord" => Self::nord(),
            "light" => Self::light(),
            _ => Self::canopy(),
        }
    }

    pub const NAMES: [&'static str; 5] = ["canopy", "catppuccin", "gruvbox", "nord", "light"];

    /// Default: deep forest greens with warm highlights.
    pub fn canopy() -> Theme {
        Theme {
            name: "canopy",
            bg: Color::Rgb(16, 22, 18),
            fg: Color::Rgb(214, 226, 214),
            muted: Color::Rgb(112, 132, 116),
            border: Color::Rgb(46, 64, 50),
            border_focus: Color::Rgb(122, 200, 120),
            accent: Color::Rgb(122, 200, 120),
            accent_alt: Color::Rgb(230, 190, 110),
            selection_bg: Color::Rgb(36, 56, 40),
            added: Color::Rgb(122, 200, 120),
            removed: Color::Rgb(232, 110, 100),
            modified: Color::Rgb(230, 190, 110),
            conflict: Color::Rgb(240, 120, 200),
            hash: Color::Rgb(160, 150, 230),
            branch: Color::Rgb(110, 200, 200),
            remote: Color::Rgb(120, 160, 230),
            tag: Color::Rgb(230, 190, 110),
            warn: Color::Rgb(230, 190, 110),
            error: Color::Rgb(232, 110, 100),
            added_bg: Color::Rgb(24, 48, 28),
            removed_bg: Color::Rgb(56, 26, 26),
        }
    }

    pub fn catppuccin() -> Theme {
        Theme {
            name: "catppuccin",
            bg: Color::Rgb(30, 30, 46),
            fg: Color::Rgb(205, 214, 244),
            muted: Color::Rgb(127, 132, 156),
            border: Color::Rgb(69, 71, 90),
            border_focus: Color::Rgb(203, 166, 247),
            accent: Color::Rgb(203, 166, 247),
            accent_alt: Color::Rgb(137, 180, 250),
            selection_bg: Color::Rgb(49, 50, 68),
            added: Color::Rgb(166, 227, 161),
            removed: Color::Rgb(243, 139, 168),
            modified: Color::Rgb(249, 226, 175),
            conflict: Color::Rgb(245, 194, 231),
            hash: Color::Rgb(180, 190, 254),
            branch: Color::Rgb(148, 226, 213),
            remote: Color::Rgb(137, 180, 250),
            tag: Color::Rgb(250, 179, 135),
            warn: Color::Rgb(249, 226, 175),
            error: Color::Rgb(243, 139, 168),
            added_bg: Color::Rgb(38, 56, 48),
            removed_bg: Color::Rgb(62, 38, 52),
        }
    }

    pub fn gruvbox() -> Theme {
        Theme {
            name: "gruvbox",
            bg: Color::Rgb(40, 40, 40),
            fg: Color::Rgb(235, 219, 178),
            muted: Color::Rgb(146, 131, 116),
            border: Color::Rgb(80, 73, 69),
            border_focus: Color::Rgb(250, 189, 47),
            accent: Color::Rgb(250, 189, 47),
            accent_alt: Color::Rgb(131, 165, 152),
            selection_bg: Color::Rgb(60, 56, 54),
            added: Color::Rgb(184, 187, 38),
            removed: Color::Rgb(251, 73, 52),
            modified: Color::Rgb(250, 189, 47),
            conflict: Color::Rgb(211, 134, 155),
            hash: Color::Rgb(211, 134, 155),
            branch: Color::Rgb(142, 192, 124),
            remote: Color::Rgb(131, 165, 152),
            tag: Color::Rgb(254, 128, 25),
            warn: Color::Rgb(254, 128, 25),
            error: Color::Rgb(251, 73, 52),
            added_bg: Color::Rgb(50, 54, 30),
            removed_bg: Color::Rgb(66, 36, 32),
        }
    }

    pub fn nord() -> Theme {
        Theme {
            name: "nord",
            bg: Color::Rgb(46, 52, 64),
            fg: Color::Rgb(216, 222, 233),
            muted: Color::Rgb(118, 128, 148),
            border: Color::Rgb(67, 76, 94),
            border_focus: Color::Rgb(136, 192, 208),
            accent: Color::Rgb(136, 192, 208),
            accent_alt: Color::Rgb(180, 142, 173),
            selection_bg: Color::Rgb(59, 66, 82),
            added: Color::Rgb(163, 190, 140),
            removed: Color::Rgb(191, 97, 106),
            modified: Color::Rgb(235, 203, 139),
            conflict: Color::Rgb(180, 142, 173),
            hash: Color::Rgb(180, 142, 173),
            branch: Color::Rgb(143, 188, 187),
            remote: Color::Rgb(129, 161, 193),
            tag: Color::Rgb(208, 135, 112),
            warn: Color::Rgb(235, 203, 139),
            error: Color::Rgb(191, 97, 106),
            added_bg: Color::Rgb(54, 66, 60),
            removed_bg: Color::Rgb(70, 54, 62),
        }
    }

    pub fn light() -> Theme {
        Theme {
            name: "light",
            bg: Color::Rgb(250, 250, 246),
            fg: Color::Rgb(40, 44, 40),
            muted: Color::Rgb(120, 128, 120),
            border: Color::Rgb(206, 212, 204),
            border_focus: Color::Rgb(46, 140, 70),
            accent: Color::Rgb(46, 140, 70),
            accent_alt: Color::Rgb(176, 110, 20),
            selection_bg: Color::Rgb(222, 236, 222),
            added: Color::Rgb(40, 140, 60),
            removed: Color::Rgb(200, 50, 50),
            modified: Color::Rgb(176, 110, 20),
            conflict: Color::Rgb(180, 50, 150),
            hash: Color::Rgb(100, 80, 190),
            branch: Color::Rgb(20, 130, 140),
            remote: Color::Rgb(40, 90, 180),
            tag: Color::Rgb(176, 110, 20),
            warn: Color::Rgb(176, 110, 20),
            error: Color::Rgb(200, 50, 50),
            added_bg: Color::Rgb(220, 244, 222),
            removed_bg: Color::Rgb(250, 222, 222),
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
