use std::cell::Cell;
use std::time::{SystemTime, UNIX_EPOCH};

use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::style::Style;
use ratatui::symbols::border;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Padding};

use crate::theme::Theme;

thread_local! {
    /// `compact = true` in the config restores the tight, dense layout.
    /// Per thread: the UI draws on one thread, and each test gets its own.
    static COMPACT: Cell<bool> = const { Cell::new(false) };
}

pub fn set_compact(on: bool) {
    COMPACT.with(|c| c.set(on));
}

pub fn compact() -> bool {
    COMPACT.with(Cell::get)
}

/// Space between neighbouring panels (columns side by side, rows stacked).
pub fn gap() -> u16 {
    if compact() {
        0
    } else {
        1
    }
}

/// Extra (width, height) dialogs need for their roomier padding.
pub fn modal_extra() -> (u16, u16) {
    if compact() {
        (0, 0)
    } else {
        (2, 1)
    }
}

/// Extra rows a panel needs for its top padding.
pub fn pad_top() -> u16 {
    if compact() {
        0
    } else {
        1
    }
}

/// Space between table columns.
pub fn col_gap() -> u16 {
    if compact() {
        1
    } else {
        2
    }
}

pub fn now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

/// Compact relative time: `now`, `5m`, `3h`, `2d`, `6w`, `4mo`, `2y`.
pub fn ago(ts: i64) -> String {
    if ts == 0 {
        return String::new();
    }
    let d = (now() - ts).max(0);
    match d {
        0..60 => "now".into(),
        60..3600 => format!("{}m", d / 60),
        3600..86400 => format!("{}h", d / 3600),
        86400..1209600 => format!("{}d", d / 86400),
        1209600..5184000 => format!("{}w", d / 604800),
        5184000..31536000 => format!("{}mo", d / 2592000),
        _ => format!("{}y", d / 31536000),
    }
}

pub fn panel<'a>(theme: &Theme, title: impl Into<Line<'a>>, focused: bool) -> Block<'a> {
    let color = if focused { theme.border_focus } else { theme.border };
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(color))
        .title(title.into().style(if focused { theme.accent() } else { theme.fg(theme.fg) }))
        .padding(if compact() { Padding::horizontal(1) } else { Padding::new(2, 2, 1, 0) })
}

pub fn modal_block<'a>(theme: &Theme, title: impl Into<Line<'a>>, danger: bool) -> Block<'a> {
    let color = if danger { theme.error } else { theme.border_focus };
    Block::default()
        .borders(Borders::ALL)
        .border_set(border::ROUNDED)
        .border_style(Style::default().fg(color))
        .title(title.into().style(Style::default().fg(color).add_modifier(ratatui::style::Modifier::BOLD)))
        .style(theme.base())
        .padding(if compact() { Padding::new(2, 2, 1, 0) } else { Padding::new(3, 3, 1, 1) })
}

pub fn centered(area: Rect, w: u16, h: u16) -> Rect {
    let w = w.min(area.width.saturating_sub(2));
    let h = h.min(area.height.saturating_sub(2));
    let [v] = Layout::vertical([Constraint::Length(h)]).flex(Flex::Center).areas(area);
    let [r] = Layout::horizontal([Constraint::Length(w)]).flex(Flex::Center).areas(v);
    r
}

pub fn trunc(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else if max == 0 {
        String::new()
    } else {
        let mut t: String = s.chars().take(max - 1).collect();
        t.push('…');
        t
    }
}

/// `key label` pairs rendered as highlighted chips for the hint bar.
pub fn key_hint<'a>(theme: &Theme, key: &str, label: &str) -> Vec<Span<'a>> {
    vec![Span::styled(format!(" {key} "), theme.key()), Span::styled(format!(" {label}  "), theme.muted())]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_times() {
        let n = now();
        assert_eq!(ago(n), "now");
        assert_eq!(ago(n - 120), "2m");
        assert_eq!(ago(n - 7200), "2h");
        assert_eq!(ago(n - 86400 * 3), "3d");
        assert_eq!(ago(0), "");
    }

    #[test]
    fn truncation() {
        assert_eq!(trunc("hello", 10), "hello");
        assert_eq!(trunc("hello world", 5), "hell…");
    }
}
