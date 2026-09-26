pub mod diff;
pub mod graph;
pub mod modal;
pub mod screens;
pub mod util;

use canopy_git::RepoState;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};
use ratatui::Frame;

use crate::app::{App, Level};
use crate::input::{contexts, state_word};
use crate::keymap::{pretty_key, Screen};
use util::key_hint;

const SPINNER: [&str; 8] = ["⣾", "⣽", "⣻", "⢿", "⡿", "⣟", "⣯", "⣷"];

pub fn draw(f: &mut Frame, app: &mut App) {
    let area = f.area();
    f.render_widget(Block::default().style(app.theme.base()), area);
    let teach = app.config.teach_mode && app.git.is_some();
    let [header, tabs, body, teach_area, footer] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Fill(1),
        Constraint::Length(teach as u16),
        Constraint::Length(1),
    ])
    .areas(area);

    draw_header(f, header, app);
    draw_tabs(f, tabs, app);
    screens::draw(f, body, app);
    if teach {
        draw_teach(f, teach_area, app);
    }
    draw_footer(f, footer, app);
    modal::draw(f, app);
}

fn draw_header(f: &mut Frame, area: Rect, app: &App) {
    let t = &app.theme;
    let b = &app.data.status.branch;
    let branch_glyph = if app.config.nerd_font { "\u{e725} " } else { "⎇ " };
    let mut spans = vec![
        Span::styled(" 🌳 canopy ", Style::default().fg(t.bg).bg(t.accent).add_modifier(Modifier::BOLD)),
        Span::raw("  "),
        Span::styled(app.repo_name(), Style::default().fg(t.fg).add_modifier(Modifier::BOLD)),
    ];
    if app.git.is_some() {
        let name = b.head.clone().unwrap_or_else(|| "detached".into());
        spans.push(Span::styled(format!("  {branch_glyph}{name}"), t.fg(t.branch)));
        if b.upstream.is_some() {
            if b.ahead > 0 {
                spans.push(Span::styled(format!(" ↑{}", b.ahead), t.fg(t.warn)));
            }
            if b.behind > 0 {
                spans.push(Span::styled(format!(" ↓{}", b.behind), t.fg(t.warn)));
            }
        }
        if let Some(state) = app.data.state.filter(|s| *s != RepoState::Clean) {
            spans.push(Span::raw("  "));
            spans.push(Span::styled(
                format!(" {} IN PROGRESS ", state_word(state).to_uppercase()),
                Style::default().fg(t.bg).bg(t.conflict).add_modifier(Modifier::BOLD),
            ));
        }
    }
    f.render_widget(Paragraph::new(Line::from(spans)), area);

    // Right side: busy spinner / progress.
    if let Some(label) = &app.busy {
        let spin = SPINNER[(app.tick as usize) % SPINNER.len()];
        let mut text = format!("{spin} {label}");
        if let Some(p) = &app.progress {
            text.push_str(&format!(" · {}", util::trunc(p, 50)));
        }
        text.push(' ');
        let w = text.chars().count() as u16;
        if area.width > w + 30 {
            let r = Rect { x: area.x + area.width - w, width: w, ..area };
            f.render_widget(Paragraph::new(Span::styled(text, t.fg(t.accent_alt))), r);
        }
    }
}

fn draw_tabs(f: &mut Frame, area: Rect, app: &App) {
    let t = &app.theme;
    let mut spans = vec![Span::raw(" ")];
    for (i, s) in Screen::ALL.iter().enumerate() {
        let badge = match s {
            Screen::Status => {
                let n = app.data.status.files.iter().filter(|f| f.kind != canopy_git::FileKind::Ignored).count();
                (n > 0).then(|| n.to_string())
            }
            Screen::Stash => (!app.data.stashes.is_empty()).then(|| app.data.stashes.len().to_string()),
            _ => None,
        };
        let active = *s == app.screen;
        let style = if active {
            Style::default().fg(t.bg).bg(t.accent).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(t.muted)
        };
        spans.push(Span::styled(format!(" {} {} ", i + 1, s.title()), style));
        if let Some(b) = badge {
            spans.push(Span::styled(format!("{b} "), if active { style } else { t.fg(t.accent_alt) }));
        }
        spans.push(Span::raw(" "));
    }
    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn draw_teach(f: &mut Frame, area: Rect, app: &App) {
    let t = &app.theme;
    let line = match app.history.last() {
        Some(cmd) => Line::from(vec![
            Span::styled(" $ ", t.fg(t.accent).add_modifier(Modifier::BOLD)),
            Span::styled(cmd.clone(), t.fg(t.accent_alt)),
            Span::styled("   ← what that just ran", t.muted()),
        ]),
        None => Line::from(vec![
            Span::styled(" $ ", t.fg(t.accent).add_modifier(Modifier::BOLD)),
            Span::styled("teach mode: the git command behind each action appears here (T to hide)", t.muted()),
        ]),
    };
    f.render_widget(Paragraph::new(line), area);
}

fn draw_footer(f: &mut Frame, area: Rect, app: &App) {
    let t = &app.theme;
    if let Some(toast) = &app.toast {
        let color = match toast.level {
            Level::Info => t.accent,
            Level::Success => t.added,
            Level::Warn => t.warn,
            Level::Error => t.error,
        };
        let line = Line::from(vec![Span::styled(
            format!(" {} ", toast.text),
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        )]);
        f.render_widget(Paragraph::new(line), area);
        return;
    }
    let mut spans = vec![Span::raw(" ")];
    let mut seen = Vec::new();
    for ctx in contexts(app) {
        for b in app.keymap.bindings(ctx).iter().filter(|b| b.hint && !b.keys.is_empty()) {
            if seen.contains(&b.action) {
                continue;
            }
            seen.push(b.action);
            spans.extend(key_hint(t, &pretty_key(&b.keys[0]), b.action.short()));
        }
    }
    f.render_widget(Paragraph::new(Line::from(spans)), area);
}
