use ratatui::layout::{Constraint, Layout, Position, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::App;
use crate::input::palette_matches;
use crate::keymap::{pretty_key, Ctx, Screen};
use crate::modal::{Modal, TodoAction};
use crate::textarea::TextArea;
use crate::theme::Theme;
use crate::ui::util::{ago, centered, key_hint, modal_block, trunc};

pub fn draw(f: &mut Frame, app: &App) {
    let area = f.area();
    let t = &app.theme;
    match &app.modal {
        Modal::None => {}
        Modal::Help { scroll } => help(f, area, app, *scroll),
        Modal::Welcome => welcome(f, area, t),
        Modal::Confirm { title, lines, danger, .. } => {
            let w = lines.iter().map(|l| l.chars().count()).max().unwrap_or(20).max(title.len() + 4) as u16 + 8;
            let r = centered(area, w.max(44), lines.len() as u16 + 6);
            f.render_widget(Clear, r);
            let mut text: Vec<Line> = lines.iter().map(|l| Line::raw(l.clone())).collect();
            text.push(Line::default());
            let mut hints = key_hint(t, "y", if *danger { "yes, do it" } else { "yes" });
            hints.extend(key_hint(t, "n", "cancel"));
            text.push(Line::from(hints));
            f.render_widget(Paragraph::new(text).block(modal_block(t, format!(" {title} "), *danger)), r);
        }
        Modal::Menu { title, items, sel } => {
            let w = items.iter().map(|i| i.label.len() + i.detail.len() + 10).max().unwrap_or(30).max(title.len() + 6)
                as u16
                + 6;
            let r = centered(area, w, items.len() as u16 + 5);
            f.render_widget(Clear, r);
            let lines: Vec<Line> = items
                .iter()
                .enumerate()
                .map(|(i, it)| {
                    let color = if it.danger { t.error } else { t.accent };
                    let mut l = Line::from(vec![
                        Span::styled(
                            format!(" {} ", it.key),
                            Style::default().fg(t.bg).bg(color).add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(
                            format!("  {:<24}", it.label),
                            Style::default().fg(t.fg).add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(it.detail.clone(), t.muted()),
                    ]);
                    if i == *sel {
                        l = l.style(t.selected());
                    }
                    l
                })
                .collect();
            let mut text = lines;
            text.push(Line::default());
            text.push(Line::styled("esc cancel", t.muted()));
            f.render_widget(Paragraph::new(text).block(modal_block(t, format!(" {title} "), false)), r);
        }
        Modal::Input { title, hint, input, .. } => {
            let r = centered(area, 64, 7);
            f.render_widget(Clear, r);
            let block = modal_block(t, format!(" {title} "), false);
            let inner = block.inner(r);
            f.render_widget(block, r);
            let [line, _, h] =
                Layout::vertical([Constraint::Length(1), Constraint::Length(1), Constraint::Length(1)]).areas(inner);
            draw_text(f, line, input, t, true, "› ");
            f.render_widget(Paragraph::new(Line::styled(hint.clone(), t.muted())), h);
        }
        Modal::Commit { subject, body, on_body, amend } => commit(f, area, app, subject, body, *on_body, *amend),
        Modal::Palette { input, sel } => {
            let entries = palette_matches(app, &input.text());
            let h = (entries.len() as u16 + 5).min(area.height.saturating_sub(4)).max(8);
            let r = centered(area, 60, h);
            let r = Rect { y: area.y + area.height / 6, ..r };
            f.render_widget(Clear, r);
            let block = modal_block(t, " Command palette ", false);
            let inner = block.inner(r);
            f.render_widget(block, r);
            let [line, _, list] =
                Layout::vertical([Constraint::Length(1), Constraint::Length(1), Constraint::Fill(1)]).areas(inner);
            draw_text(f, line, input, t, true, "› ");
            let width = list.width as usize;
            let items: Vec<ListItem> = entries
                .iter()
                .map(|(a, k)| {
                    let label = match a {
                        crate::keymap::Action::Custom(i) => {
                            let c = &app.config.custom_commands[*i];
                            if c.description.is_empty() {
                                c.cmd.clone()
                            } else {
                                c.description.clone()
                            }
                        }
                        _ => a.label().to_string(),
                    };
                    let pad = width.saturating_sub(label.chars().count() + k.chars().count() + 2);
                    ListItem::new(Line::from(vec![
                        Span::raw(format!(" {label}")),
                        Span::raw(" ".repeat(pad)),
                        Span::styled(k.clone(), t.fg(t.accent)),
                    ]))
                })
                .collect();
            let mut st = ListState::default().with_selected(Some(*sel));
            f.render_stateful_widget(List::new(items).highlight_style(t.selected()), list, &mut st);
        }
        Modal::Output { title, lines, scroll } => {
            let r = centered(area, area.width * 4 / 5, area.height * 4 / 5);
            f.render_widget(Clear, r);
            let text: Vec<Line> = lines.iter().map(|l| Line::raw(l.clone())).collect();
            let block = modal_block(t, format!(" {title} "), false)
                .title_bottom(Line::styled(" j/k scroll · esc close ", t.muted()));
            f.render_widget(Paragraph::new(text).scroll((*scroll, 0)).block(block), r);
        }
        Modal::Blame(v) => blame(f, area, t, v),
        Modal::Rebase { items, sel, .. } => {
            let r = centered(area, 90, items.len() as u16 + 9);
            f.render_widget(Clear, r);
            let mut lines =
                vec![Line::styled("Oldest first: git replays these top to bottom.", t.muted()), Line::default()];
            for (i, it) in items.iter().enumerate() {
                let color = match it.action {
                    TodoAction::Pick => t.accent,
                    TodoAction::Edit => t.accent_alt,
                    TodoAction::Squash | TodoAction::Fixup => t.hash,
                    TodoAction::Drop => t.error,
                };
                let dropped = it.action == TodoAction::Drop;
                let subj_style =
                    if dropped { t.muted().add_modifier(Modifier::CROSSED_OUT) } else { Style::default().fg(t.fg) };
                let mut l = Line::from(vec![
                    Span::styled(
                        format!(" {:<7}", it.action.word()),
                        Style::default().fg(color).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(format!("{} ", it.commit.short), t.fg(t.hash)),
                    Span::styled(it.commit.subject.clone(), subj_style),
                    Span::styled(format!("  {}", ago(it.commit.time)), t.muted()),
                ]);
                if i == *sel {
                    l = l.style(t.selected());
                }
                lines.push(l);
            }
            lines.push(Line::default());
            let mut hints = Vec::new();
            for (k, l) in [
                ("p", "pick"),
                ("s", "squash"),
                ("f", "fixup"),
                ("e", "edit"),
                ("d", "drop"),
                ("J/K", "move"),
                ("⏎", "start"),
                ("esc", "cancel"),
            ] {
                hints.extend(key_hint(t, k, l));
            }
            lines.push(Line::from(hints));
            f.render_widget(Paragraph::new(lines).block(modal_block(t, " Interactive rebase ", false)), r);
        }
    }
}

fn draw_text(f: &mut Frame, area: Rect, input: &TextArea, t: &Theme, focused: bool, prompt: &str) {
    let text = input.lines().first().cloned().unwrap_or_default();
    let (_, col) = input.cursor();
    let avail = area.width.saturating_sub(prompt.len() as u16 + 1) as usize;
    let skip = col.saturating_sub(avail);
    let shown: String = text.chars().skip(skip).collect();
    f.render_widget(
        Paragraph::new(Line::from(vec![Span::styled(prompt.to_string(), t.accent()), Span::raw(shown)])),
        area,
    );
    if focused {
        f.set_cursor_position(Position::new(area.x + prompt.chars().count() as u16 + (col - skip) as u16, area.y));
    }
}

fn commit(f: &mut Frame, area: Rect, app: &App, subject: &TextArea, body: &TextArea, on_body: bool, amend: bool) {
    let t = &app.theme;
    let r = centered(area, 80, 20);
    f.render_widget(Clear, r);
    let staged = app.data.status.staged().count();
    let title = if amend { " Amend last commit ".to_string() } else { format!(" Commit · {staged} staged file(s) ") };
    let block = modal_block(t, title, false);
    let inner = block.inner(r);
    f.render_widget(block, r);
    let [note, s_label, s_line, _, b_label, b_area, hints] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Fill(1),
        Constraint::Length(1),
    ])
    .areas(inner);

    let note_line = if amend {
        Line::styled("Rewrites the last commit with this message and anything staged.", t.muted())
    } else if staged == 0 {
        Line::styled("Nothing staged: all changes will be staged and committed (git add --all).", t.fg(t.warn))
    } else {
        Line::styled("Commits what's staged. Unstaged changes stay in your working tree.", t.muted())
    };
    f.render_widget(Paragraph::new(note_line), note);

    let n = subject.text().chars().count();
    let (count_color, tip) = match n {
        0..=50 => (t.added, ""),
        51..=72 => (t.warn, " · keep it short"),
        _ => (t.error, " · too long for a summary"),
    };
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("Summary ", if on_body { t.muted() } else { t.accent() }),
            Span::styled(format!("{n}/50{tip}"), t.fg(count_color)),
        ])),
        s_label,
    );
    draw_text(f, s_line, subject, t, !on_body, "› ");

    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("Description ", if on_body { t.accent() } else { t.muted() }),
            Span::styled("optional · explain why, not what", t.muted()),
        ])),
        b_label,
    );
    let lines: Vec<Line> = body.lines().iter().map(|l| Line::raw(l.clone())).collect();
    let (row, col) = body.cursor();
    let scroll = (row as u16).saturating_sub(b_area.height.saturating_sub(1));
    f.render_widget(Paragraph::new(lines).scroll((scroll, 0)).style(Style::default().bg(t.selection_bg)), b_area);
    if on_body {
        f.set_cursor_position(Position::new(b_area.x + col as u16, b_area.y + row as u16 - scroll));
    }

    let mut h = Vec::new();
    for (k, l) in [("⏎", "commit"), ("tab", "switch field"), ("^s", "commit from anywhere"), ("esc", "cancel")] {
        h.extend(key_hint(t, k, l));
    }
    f.render_widget(Paragraph::new(Line::from(h)), hints);
}

fn blame(f: &mut Frame, area: Rect, t: &Theme, v: &crate::modal::BlameView) {
    let r = Rect {
        x: area.x + 1,
        y: area.y + 1,
        width: area.width.saturating_sub(2),
        height: area.height.saturating_sub(2),
    };
    f.render_widget(Clear, r);
    let at = match &v.rev {
        Some(rev) => format!(" @ {}", rev.get(..7).unwrap_or(rev)),
        None => " (working tree)".into(),
    };
    let block = modal_block(t, format!(" Blame · {}{at} ", v.path), false)
        .title_bottom(Line::styled(" j/k move · ⏎ go to commit · B blame before this change · esc close ", t.muted()));
    let inner = block.inner(r);
    f.render_widget(block, r);
    let [body, _, footer] =
        Layout::vertical([Constraint::Fill(1), Constraint::Length(1), Constraint::Length(1)]).areas(inner);

    let h = body.height as usize;
    let n = v.blame.lines.len();
    let start = v.cursor.saturating_sub(h / 2).min(n.saturating_sub(h));
    let palette = [t.hash, t.branch, t.accent_alt, t.remote, t.accent, t.conflict];
    // Colour each commit consistently by the order it first appears.
    let mut order: Vec<&str> = Vec::new();
    for l in &v.blame.lines {
        if !order.contains(&l.oid.as_str()) {
            order.push(&l.oid);
        }
    }
    let width = n.to_string().len();
    let lines: Vec<Line> = v
        .blame
        .lines
        .iter()
        .enumerate()
        .skip(start)
        .take(h)
        .map(|(i, l)| {
            let c = &v.blame.commits[&l.oid];
            let color = palette[order.iter().position(|o| *o == l.oid).unwrap_or(0) % palette.len()];
            // Only label the first line of each run of lines from the same commit.
            let first = i == 0 || v.blame.lines[i - 1].oid != l.oid;
            let who = if !first {
                format!("{:<8} {:<14} {:>4}", "│", "", "")
            } else if c.uncommitted {
                format!("{:<8} {:<14} {:>4}", "·······", "not committed", "")
            } else {
                format!("{:<8} {:<14} {:>4}", &c.oid[..7], trunc(&c.author, 14), ago(c.time))
            };
            let mut line = Line::from(vec![
                Span::styled(who, Style::default().fg(color)),
                Span::styled(format!(" {:>width$} ", l.line), t.muted()),
                Span::raw(l.content.replace('\t', "    ")),
            ]);
            if i == v.cursor {
                line = line.style(t.selected());
            }
            line
        })
        .collect();
    f.render_widget(Paragraph::new(lines), body);

    if let Some(l) = v.blame.lines.get(v.cursor) {
        let c = &v.blame.commits[&l.oid];
        let text = if c.uncommitted {
            Line::styled("Not committed yet", t.muted())
        } else {
            Line::from(vec![
                Span::styled(format!("{} ", &c.oid[..7]), t.fg(t.hash)),
                Span::styled(c.summary.clone(), Style::default().fg(t.fg).add_modifier(Modifier::BOLD)),
                Span::styled(format!("  {} · {}", c.author, ago(c.time)), t.muted()),
            ])
        };
        f.render_widget(Paragraph::new(text), footer);
    }
}

fn welcome(f: &mut Frame, area: Rect, t: &Theme) {
    let r = centered(area, 72, 22);
    f.render_widget(Clear, r);
    let path = crate::config::Config::path().map(|p| p.display().to_string()).unwrap_or_default();
    let k = |s: &str| Span::styled(format!(" {s} "), t.key());
    let lines = vec![
        Line::styled("Welcome to Canopy 🌳", t.accent()),
        Line::styled("A git dashboard for your terminal.", t.muted()),
        Line::default(),
        Line::from(vec![k("1-7"), Span::raw("  switch tabs: Home, Changes, History, Branches, …")]),
        Line::from(vec![k("space"), Span::raw("  stage or unstage the selected file")]),
        Line::from(vec![k("⏎"), Span::raw("  dive into a diff to stage single lines or hunks")]),
        Line::from(vec![
            k("c"),
            Span::raw("  commit     "),
            k("P"),
            Span::raw("  push     "),
            k("p"),
            Span::raw("  pull"),
        ]),
        Line::from(vec![k("z"), Span::raw("  undo the last commit/reset/checkout (via the reflog)")]),
        Line::from(vec![k(":"), Span::raw("  command palette: search every action by name")]),
        Line::from(vec![k("?"), Span::raw("  all keys for the current screen")]),
        Line::default(),
        Line::from(vec![
            Span::styled("Teach mode", t.fg(t.accent_alt).add_modifier(Modifier::BOLD)),
            Span::raw(" is on: the footer shows the git command behind"),
        ]),
        Line::raw("each action so you learn git as you go. Toggle it with T."),
        Line::default(),
        Line::styled(format!("A starter config will be written to {path}"), t.muted()),
        Line::default(),
        Line::styled("Press any key to start", t.accent()),
    ];
    f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }).block(modal_block(t, " canopy ", false)), r);
}

fn help(f: &mut Frame, area: Rect, app: &App, scroll: u16) {
    let t = &app.theme;
    let r = centered(area, 84, area.height.saturating_sub(4));
    f.render_widget(Clear, r);
    let mut lines = Vec::new();
    let section = |lines: &mut Vec<Line>, title: &str, ctx: Ctx| {
        let bs = app.keymap.bindings(ctx);
        if bs.is_empty() {
            return;
        }
        lines.push(Line::styled(title.to_string(), t.accent()));
        for b in bs {
            let keys = b.keys.iter().map(|k| pretty_key(k)).collect::<Vec<_>>().join(" / ");
            lines.push(Line::from(vec![
                Span::styled(format!("  {keys:<16}"), t.fg(t.accent_alt)),
                Span::raw(b.action.label()),
            ]));
        }
        lines.push(Line::default());
    };
    let screen_name = app.screen.title();
    let screen_name = match crate::input::screen_ctx(app) {
        Ctx::Tags => "Tags",
        Ctx::Remotes => "Remotes",
        _ => screen_name,
    };
    section(&mut lines, &format!("{screen_name} (this screen)"), crate::input::screen_ctx(app));
    if app.screen == Screen::Status {
        section(&mut lines, "Conflict panel (⏎ on a conflicted file)", Ctx::Conflict);
    }
    if matches!(app.screen, Screen::Status | Screen::Log | Screen::Branches | Screen::Stash | Screen::Reflog) {
        section(&mut lines, "Diff panel (after ⏎)", Ctx::Diff);
    }
    section(&mut lines, "Everywhere", Ctx::Global);
    if !app.config.custom_commands.is_empty() {
        lines.push(Line::styled("Custom commands", t.accent()));
        for c in &app.config.custom_commands {
            lines.push(Line::from(vec![
                Span::styled(format!("  {:<16}", c.key), t.fg(t.accent_alt)),
                Span::raw(if c.description.is_empty() { trunc(&c.cmd, 50) } else { c.description.clone() }),
            ]));
        }
    }
    let block =
        modal_block(t, " Keys ", false).title_bottom(Line::styled(" j/k scroll · any other key closes ", t.muted()));
    f.render_widget(Paragraph::new(lines).scroll((scroll, 0)).block(block), r);
}
