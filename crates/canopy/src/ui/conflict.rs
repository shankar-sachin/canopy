use canopy_git::parse::conflict::Segment;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::app::App;
use crate::keymap::{pretty_key, Action, Ctx, Focus};
use crate::theme::Theme;
use crate::ui::util::panel;

/// Unchanged text longer than this between conflicts is folded.
const FOLD: usize = 7;

fn text_lines<'a>(text: &str, theme: &Theme, first: bool, last: bool, out: &mut Vec<Line<'a>>) {
    let lines: Vec<&str> = text.lines().collect();
    let plain = |l: &str| Line::styled(format!("  {}", l.replace('\t', "    ")), theme.muted());
    if lines.len() <= FOLD {
        out.extend(lines.iter().map(|l| plain(l)));
        return;
    }
    let (head, tail) = match (first, last) {
        (true, true) => (0, 0),
        (true, false) => (0, 3),
        (false, true) => (3, 0),
        (false, false) => (3, 3),
    };
    out.extend(lines[..head].iter().map(|l| plain(l)));
    let hidden = lines.len() - head - tail;
    out.push(Line::styled(format!("  … {hidden} unchanged lines …"), theme.muted().add_modifier(Modifier::ITALIC)));
    out.extend(lines[lines.len() - tail..].iter().map(|l| plain(l)));
}

fn side<'a>(label: String, body: &str, color: Color, theme: &Theme, dim: bool, out: &mut Vec<Line<'a>>) {
    let st = |s: Style| if dim { s.add_modifier(Modifier::DIM) } else { s };
    out.push(Line::styled(format!("  ▌{label}"), st(Style::default().fg(color).add_modifier(Modifier::BOLD))));
    if body.is_empty() {
        out.push(Line::styled("  ▌  (nothing)", st(theme.muted().add_modifier(Modifier::ITALIC))));
    }
    for l in body.lines() {
        out.push(Line::from(vec![
            Span::styled("  ▌ ", st(Style::default().fg(color))),
            Span::styled(l.replace('\t', "    "), st(Style::default().fg(theme.fg))),
        ]));
    }
}

pub fn draw(f: &mut Frame, area: Rect, app: &mut App) {
    let theme = app.theme.clone();
    let focused = app.focus == Focus::Conflict;
    let key = |a| pretty_key(app.keymap.key_for(&[Ctx::Conflict], a).unwrap_or("?"));
    let hint = format!(
        " {} ours · {} theirs · {} both · {} restore ",
        key(Action::KeepOurs),
        key(Action::KeepTheirs),
        key(Action::KeepBoth),
        key(Action::RestoreConflict)
    );
    let enter = pretty_key(
        app.keymap
            .key_for(&[crate::keymap::Ctx::Screen(crate::keymap::Screen::Status)], Action::Enter)
            .unwrap_or("enter"),
    );
    let Some(view) = app.conflict.as_mut() else { return };
    let total = view.count();

    let mut lines: Vec<Line> = Vec::new();
    let mut header_at = 0;
    let mut n = 0;
    let last_i = view.segments.len().saturating_sub(1);
    for (i, seg) in view.segments.iter().enumerate() {
        match seg {
            Segment::Text(t) => text_lines(t, &theme, i == 0, i == last_i, &mut lines),
            Segment::Conflict(c) => {
                let current = n == view.current;
                if current {
                    header_at = lines.len();
                }
                let head = format!(" Conflict {} of {total} ", n + 1);
                lines.push(if current {
                    Line::from(vec![
                        Span::styled(
                            head,
                            Style::default().fg(theme.bg).bg(theme.conflict).add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(if focused { hint.clone() } else { String::new() }, theme.muted()),
                    ])
                } else {
                    Line::styled(head, Style::default().fg(theme.conflict).add_modifier(Modifier::DIM))
                });
                let ours = if c.ours_label.is_empty() { "ours".into() } else { format!("ours · {}", c.ours_label) };
                side(ours, &c.ours, theme.branch, &theme, !current, &mut lines);
                if let Some(base) = &c.base {
                    side("base (before both changes)".into(), base, theme.muted, &theme, !current, &mut lines);
                }
                let theirs =
                    if c.theirs_label.is_empty() { "theirs".into() } else { format!("theirs · {}", c.theirs_label) };
                side(theirs, &c.theirs, theme.remote, &theme, !current, &mut lines);
                n += 1;
            }
        }
    }

    let title = Line::from(vec![
        Span::raw(" "),
        Span::raw(view.path.clone()),
        Span::styled(format!(" · {total} conflict(s) "), Style::default().fg(theme.conflict)),
    ]);
    let block = panel(&theme, title, focused);
    let inner = block.inner(area);
    f.render_widget(block, area);

    let h = inner.height as usize;
    // Keep the current conflict's header about a quarter of the way down.
    view.scroll = header_at.saturating_sub(h / 4).min(lines.len().saturating_sub(h));
    let shown: Vec<Line> = lines.into_iter().skip(view.scroll).take(h).collect();
    f.render_widget(Paragraph::new(shown), inner);

    if !focused && area.height > 2 {
        let tip = format!(" {enter} to resolve conflicts one by one ");
        let w = tip.chars().count() as u16;
        if area.width > w + 4 {
            let r = Rect { x: area.x + area.width - w - 2, y: area.y + area.height - 1, width: w, height: 1 };
            f.render_widget(Paragraph::new(Span::styled(tip, theme.fg(theme.accent_alt))), r);
        }
    }
}
