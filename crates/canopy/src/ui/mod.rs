pub mod conflict;
pub mod diff;
pub mod graph;
pub mod logo;
pub mod modal;
pub mod screens;
pub mod splash;
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

/// Single-width glyphs that every monospace font has (no braille/emoji).
pub const SPINNER: [&str; 4] = ["·", "•", "●", "•"];

pub fn draw(f: &mut Frame, app: &mut App) {
    let area = f.area();
    if let Some(start) = app.splash_start {
        if splash::fits(area) {
            splash::draw(f, area, &app.theme, start.elapsed());
            return;
        }
    }
    f.render_widget(Block::default().style(app.theme.base()), area);
    let teach = app.config.teach_mode && app.git.is_some();
    // Spacious (default) adds vertical room: a row between the title and the
    // tabs (the mini logo's second row), air around the body, and a row
    // between the teach line and the key hints. Panels use the full width;
    // text rows are indented one column so they don't touch the edge.
    let roomy = !util::compact() && area.height >= 20;
    let gutter: u16 = 1;
    let text = |r: Rect| Rect { x: r.x + gutter, width: r.width.saturating_sub(gutter * 2), ..r };
    let [header, header_gap, tabs, _, body, _, teach_area, _, footer] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(roomy as u16),
        Constraint::Length(1),
        Constraint::Length(roomy as u16),
        Constraint::Fill(1),
        Constraint::Length((roomy && teach) as u16),
        Constraint::Length(teach as u16),
        Constraint::Length(roomy as u16),
        Constraint::Length(1),
    ])
    .areas(area);
    let (header, header_gap, tabs, teach_area, footer) =
        (text(header), text(header_gap), text(tabs), text(teach_area), text(footer));

    // The mini tree in the top-right corner, across the header and the gap.
    let logo_w = 6;
    let header = if roomy && area.width >= 70 {
        let r = Rect { x: header.x + header.width - logo_w, y: header.y, width: logo_w, height: 1 + header_gap.height };
        f.render_widget(Paragraph::new(logo::mini()), r);
        Rect { width: header.width.saturating_sub(logo_w + 2), ..header }
    } else {
        header
    };

    draw_header(f, header, app);
    // The tab bar only needs the left gutter; its right edge can use the width.
    draw_tabs(f, Rect { width: tabs.width + gutter, ..tabs }, app);
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
    let branch_glyph = if app.config.nerd_font { "\u{e725} " } else { "" };
    let mut spans = vec![
        Span::styled(" canopy ", Style::default().fg(t.bg).bg(t.accent).add_modifier(Modifier::BOLD)),
        Span::raw("   "),
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
            if let Some(canopy_git::parse::bisect::BisectStep::Testing { steps_left, .. }) = &app.bisect {
                spans.push(Span::styled(format!(" ~{steps_left} steps left"), t.fg(t.conflict)));
            }
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
    let badge = |s: &Screen| match s {
        Screen::Status => {
            let n = app.data.status.files.iter().filter(|f| f.kind != canopy_git::FileKind::Ignored).count();
            (n > 0).then(|| n.to_string())
        }
        Screen::Stash => (!app.data.stashes.is_empty()).then(|| app.data.stashes.len().to_string()),
        Screen::Pulls if app.github.prs_loaded => {
            (!app.github.prs.is_empty()).then(|| app.github.prs.len().to_string())
        }
        _ => None,
    };
    // Full names if they fit, then short names, then just the numbers.
    let build = |names: &dyn Fn(Screen) -> &'static str| {
        let mut spans = vec![];
        for (i, s) in Screen::ALL.iter().enumerate() {
            let active = *s == app.screen;
            let style = if active {
                Style::default().fg(t.bg).bg(t.accent).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(t.muted)
            };
            // Keys are 1-9 then 0.
            let key = (i + 1) % 10;
            let name = names(*s);
            let label = if name.is_empty() { format!(" {key} ") } else { format!(" {key} {name} ") };
            spans.push(Span::styled(label, style));
            if let Some(b) = badge(s).filter(|_| !name.is_empty()) {
                spans.push(Span::styled(format!("{b} "), if active { style } else { t.fg(t.accent_alt) }));
            }
            spans.push(Span::raw(" "));
        }
        Line::from(spans)
    };
    let full = build(&Screen::title);
    let line = if full.width() <= area.width as usize {
        full
    } else {
        let short = build(&Screen::short_title);
        if short.width() <= area.width as usize {
            short
        } else {
            build(&|s| if s == app.screen { Screen::short_title(s) } else { "" })
        }
    };
    f.render_widget(Paragraph::new(line), area);
}

fn draw_teach(f: &mut Frame, area: Rect, app: &App) {
    let t = &app.theme;
    let line = match app.history.last() {
        Some(cmd) => Line::from(vec![
            Span::styled("$ ", t.fg(t.accent).add_modifier(Modifier::BOLD)),
            Span::styled(cmd.clone(), t.fg(t.accent_alt)),
            Span::styled("   ← what that just ran", t.muted()),
        ]),
        None => Line::from(vec![
            Span::styled("$ ", t.fg(t.accent).add_modifier(Modifier::BOLD)),
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
    // "? help" is pinned to the right so it's never cut off; the other
    // hints fill the space to its left, whole hints only.
    let help_key = app.keymap.key_for(&[crate::keymap::Ctx::Global], crate::keymap::Action::Help).unwrap_or("?");
    let help = Line::from(key_hint(t, &pretty_key(help_key), "help"));
    let help_w = help.width() as u16;
    let avail = area.width.saturating_sub(help_w + 1) as usize;
    let mut spans = vec![];
    let mut used = 0;
    let mut seen = vec![crate::keymap::Action::Help];
    for ctx in contexts(app) {
        for b in app.keymap.bindings(ctx).iter().filter(|b| b.hint && !b.keys.is_empty()) {
            if seen.contains(&b.action) {
                continue;
            }
            seen.push(b.action);
            let chip = key_hint(t, &pretty_key(&b.keys[0]), b.action.short());
            let w: usize = chip.iter().map(|s| s.width()).sum();
            if used + w > avail {
                continue;
            }
            used += w;
            spans.extend(chip);
        }
    }
    f.render_widget(Paragraph::new(Line::from(spans)), area);
    if area.width > help_w {
        let r = Rect { x: area.x + area.width - help_w, width: help_w, ..area };
        f.render_widget(Paragraph::new(help), r);
    }
}
