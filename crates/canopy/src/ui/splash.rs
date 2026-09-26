//! The startup animation: the tree grows, then "canopy" types in beside it.
//! The repository loads in the background meanwhile; any key skips it.

use std::time::Duration;

use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};
use ratatui::Frame;

use crate::theme::Theme;
use crate::ui::logo;

/// How long the tree takes to grow.
const GROW: Duration = Duration::from_millis(650);
/// When the wordmark starts and finishes typing.
const TYPE_START: Duration = Duration::from_millis(450);
const TYPE_END: Duration = Duration::from_millis(850);
/// When the tagline appears.
const TAGLINE: Duration = Duration::from_millis(900);
/// When the splash hands over to the app.
pub const TOTAL: Duration = Duration::from_millis(1200);

/// Smallest terminal the splash is shown on.
pub fn fits(area: Rect) -> bool {
    area.width >= 44 && area.height >= 14
}

fn ease_out(t: f32) -> f32 {
    1.0 - (1.0 - t).powi(3)
}

pub fn draw(f: &mut Frame, area: Rect, theme: &Theme, elapsed: Duration) {
    f.render_widget(Block::default().style(theme.base()), area);
    let grow = ease_out((elapsed.as_secs_f32() / GROW.as_secs_f32()).min(1.0));
    let tree = logo::grown(2, grow);

    // "canopy", one letter at a time.
    let word = "canopy";
    let typed = if elapsed < TYPE_START {
        0
    } else {
        let t = (elapsed - TYPE_START).as_secs_f32() / (TYPE_END - TYPE_START).as_secs_f32();
        ((t * word.len() as f32).ceil() as usize).min(word.len())
    };
    let cursor = if typed < word.len() && elapsed >= TYPE_START { "▌" } else { "" };
    let mut title = vec![Span::styled(&word[..typed], theme.accent().add_modifier(Modifier::BOLD))];
    title.push(Span::styled(cursor, theme.fg(theme.accent)));

    let show_tag = elapsed >= TAGLINE;
    let tag = Line::styled(if show_tag { "a git dashboard for your terminal" } else { "" }, theme.muted());
    let version = Line::styled(
        if show_tag { format!("v{}", env!("CARGO_PKG_VERSION")) } else { String::new() },
        Style::default().fg(theme.border_focus).add_modifier(Modifier::DIM),
    );

    // Tree on the left, words on the right, vertically centred as a group.
    let tree_w = tree.first().map(|l| l.width()).unwrap_or(18) as u16;
    let text_w = 36u16;
    let gap = 4u16;
    let w = tree_w + gap + text_w;
    let h = tree.len() as u16;
    let x = area.x + area.width.saturating_sub(w) / 2;
    let y = area.y + area.height.saturating_sub(h) / 2;
    f.render_widget(Paragraph::new(tree), Rect { x, y, width: tree_w.min(area.width), height: h.min(area.height) });

    let words = vec![Line::from(title), Line::default(), tag, Line::default(), version];
    let ty = y + h.saturating_sub(words.len() as u16) / 2 + 1;
    let tx = x + tree_w + gap;
    if tx < area.x + area.width {
        let tw = (area.x + area.width - tx).min(text_w);
        f.render_widget(Paragraph::new(words), Rect { x: tx, y: ty, width: tw, height: 5.min(area.height) });
    }

    let hint = Line::styled("any key to skip", theme.muted().add_modifier(Modifier::DIM)).centered();
    if area.height > h + 4 {
        f.render_widget(Paragraph::new(hint), Rect { y: area.y + area.height - 2, height: 1, ..area });
    }
}
