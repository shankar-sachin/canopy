use canopy_git::parse::diff::PatchMode;
use canopy_git::{DiffLine, DiffLineKind};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};
use ratatui::Frame;

use crate::app::App;
use crate::keymap::Focus;
use crate::theme::Theme;
use crate::ui::util::{panel, trunc};
use crate::views::diff::{DiffView, Row};
use canopy_highlight::Token;

pub fn draw(f: &mut Frame, area: Rect, app: &mut App) {
    let focused = app.focus == Focus::Diff;
    let theme = app.theme.clone();
    let syntax = app.config.syntax;
    let Some(view) = app.diff.as_mut() else {
        let block = panel(&theme, " Diff ", false);
        let msg = Paragraph::new(Line::styled("Select an item to see its changes", theme.muted())).block(block);
        f.render_widget(msg, area);
        return;
    };
    // `syntax = false` in the config: plain diffs.
    if !syntax {
        view.hl.clear();
    }

    let (added, removed) = view.added_removed();
    let mut title =
        vec![Span::raw(" "), Span::raw(trunc(&view.title, area.width.saturating_sub(24) as usize)), Span::raw(" ")];
    // Info-only panels (PR details, remotes, worktrees) have no counts to show.
    if !view.files.is_empty() {
        title.push(Span::styled(format!("+{added}"), theme.fg(theme.added)));
        title.push(Span::raw(" "));
        title.push(Span::styled(format!("-{removed} "), theme.fg(theme.removed)));
    }
    if view.anchor.is_some() {
        title.push(Span::styled("[range] ", theme.fg(theme.accent_alt)));
    }
    let block = panel(&theme, Line::from(title), focused);
    let inner = block.inner(area);
    f.render_widget(block, area);
    app.last_diff_width = inner.width as usize;

    if view.rows.is_empty() {
        let text = if view.files.iter().any(|f| f.binary) { "Binary file" } else { "No changes" };
        f.render_widget(Paragraph::new(Line::styled(text, theme.muted())), inner);
        return;
    }

    // Info-only panels (PR, issue and run details, remotes, worktrees) wrap
    // to the panel at draw time, so text fits whatever the width; ↑/↓ scroll.
    if view.files.is_empty() {
        view.cursor = view.cursor.min(view.meta.len().saturating_sub(1));
        let lines: Vec<Line> = view.meta.iter().map(|m| meta_line(m, &theme)).collect();
        let para = Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .scroll((u16::try_from(view.cursor).unwrap_or(u16::MAX), 0));
        f.render_widget(para, inner);
        return;
    }

    let height = inner.height as usize;
    if view.cursor < view.scroll {
        view.scroll = view.cursor;
    } else if view.cursor >= view.scroll + height {
        view.scroll = view.cursor + 1 - height;
    }

    if view.side_by_side && inner.width >= 60 {
        draw_split(f, inner, view, &theme, focused);
        return;
    }

    let gutter = gutter_width(view);
    let (sel_a, sel_b) = view.selection();
    let lines: Vec<Line> = view
        .rows
        .iter()
        .enumerate()
        .skip(view.scroll)
        .take(height)
        .map(|(i, row)| {
            let selected = focused && (i == view.cursor || (view.anchor.is_some() && i >= sel_a && i <= sel_b));
            let line = render_row(view, *row, &theme, gutter, selected);
            if selected {
                line.patch_style(Style::default().add_modifier(Modifier::REVERSED))
            } else {
                line
            }
        })
        .collect();
    f.render_widget(Paragraph::new(lines), inner);

    // Hint about the action this diff supports.
    if focused {
        let k = |a| {
            let key = app.keymap.key_for(&[crate::keymap::Ctx::Diff], a).unwrap_or("?");
            crate::keymap::pretty_key(key)
        };
        use crate::keymap::Action;
        let hint = match view.mode {
            Some(mode) => {
                let verb = if mode == PatchMode::Stage { "stage" } else { "unstage" };
                Some(format!(
                    " {} {verb} line · {} {verb} hunk · {} range ",
                    k(Action::StageLine),
                    k(Action::StageHunk),
                    k(Action::RangeSelect)
                ))
            }
            None if view.key.starts_with("pr:") => Some(format!(" {} comment on line ", k(Action::LineComment))),
            None => None,
        };
        if let Some(hint) = hint {
            let w = hint.chars().count() as u16;
            if area.width > w + 4 {
                let r = Rect { x: area.x + area.width - w - 2, y: area.y + area.height - 1, width: w, height: 1 };
                f.render_widget(Paragraph::new(Span::styled(hint, theme.muted())), r);
            }
        }
    }
}

fn gutter_width(view: &DiffView) -> usize {
    let max = view
        .files
        .iter()
        .flat_map(|f| f.hunks.iter())
        .map(|h| (h.old_start + h.old_len).max(h.new_start + h.new_len))
        .max()
        .unwrap_or(0);
    max.to_string().len().max(2)
}

fn num(n: Option<u32>, w: usize) -> String {
    match n {
        Some(n) => format!("{n:>w$}"),
        None => " ".repeat(w),
    }
}

fn line_style(theme: &Theme, l: &DiffLine) -> (Style, &'static str) {
    match l.kind {
        DiffLineKind::Added => (Style::default().fg(theme.added).bg(theme.added_bg), "+"),
        DiffLineKind::Removed => (Style::default().fg(theme.removed).bg(theme.removed_bg), "-"),
        DiffLineKind::Context => (Style::default().fg(theme.fg), " "),
        DiffLineKind::NoNewline => (theme.muted(), ""),
    }
}

/// The color for a kind of token, from the theme's palette.
fn token_style(theme: &Theme, tok: Token) -> Style {
    let fg = |c| Style::default().fg(c);
    match tok {
        Token::Plain => Style::default(),
        Token::Comment => fg(theme.muted).add_modifier(Modifier::ITALIC),
        Token::Keyword | Token::Tag => fg(theme.hash),
        Token::String => fg(theme.branch),
        Token::Number | Token::Constant | Token::Attribute => fg(theme.modified),
        Token::Function => fg(theme.accent),
        Token::Type => fg(theme.tag),
    }
}

/// A diff line's code: colored by token when it's highlighted (on the
/// line's background, plain text in the normal color), else all in `style`.
/// `plain` leaves the tokens uncolored: selected lines are drawn reversed,
/// and reversed token colors would turn into colored blocks.
fn code_spans<'a>(
    view: &DiffView,
    (fi, hi, li): (usize, usize, usize),
    style: Style,
    theme: &Theme,
    plain: bool,
) -> Vec<Span<'a>> {
    let l = &view.files[fi].hunks[hi].lines[li];
    let spans = view.hl.get(fi).and_then(|f| f.get(hi)).and_then(|h| h.get(li)).filter(|_| !plain);
    let Some(spans) = spans.filter(|s| !s.is_empty()) else {
        return vec![Span::styled(l.content.replace('\t', "    "), style)];
    };
    let base = style.fg(theme.fg);
    canopy_highlight::segments(&l.content, spans)
        .into_iter()
        .map(|(tok, text)| Span::styled(text.replace('\t', "    "), base.patch(token_style(theme, tok))))
        .collect()
}

fn render_row<'a>(view: &DiffView, row: Row, theme: &Theme, gw: usize, plain: bool) -> Line<'a> {
    match row {
        Row::Meta(i) => meta_line(&view.meta[i], theme),
        Row::File(fi) => {
            let f = &view.files[fi];
            let name = if f.old_path != f.new_path && !f.old_path.is_empty() {
                format!("{} → {}", f.old_path, f.new_path)
            } else {
                f.new_path.clone()
            };
            let mut spans = vec![Span::styled(format!("▌{name}"), theme.accent())];
            if f.binary {
                spans.push(Span::styled("  (binary)", theme.muted()));
            }
            Line::from(spans)
        }
        Row::Hunk(fi, hi) => {
            let h = &view.files[fi].hunks[hi];
            Line::styled(h.header.clone(), Style::default().fg(theme.accent_alt).add_modifier(Modifier::DIM))
        }
        Row::Line(fi, hi, li) => {
            let l = &view.files[fi].hunks[hi].lines[li];
            let (style, sign) = line_style(theme, l);
            let content = l.content.replace('\t', "    ");
            if l.kind == DiffLineKind::NoNewline {
                return Line::styled(format!("{} {content}", " ".repeat(gw * 2 + 1)), theme.muted());
            }
            let mut spans = vec![
                Span::styled(num(l.old_no, gw), theme.muted()),
                Span::styled(" ", theme.muted()),
                Span::styled(num(l.new_no, gw), theme.muted()),
                Span::styled(format!(" {sign}"), style),
            ];
            spans.extend(code_spans(view, (fi, hi, li), style, theme, plain));
            Line::from(spans)
        }
        Row::Note(n, k) => {
            // A review comment, indented under its line: header, then text.
            let text = view.notes[n].text[k].clone();
            let bar = Span::styled(format!("{}│ ", " ".repeat(gw * 2 + 2)), theme.fg(theme.accent_alt));
            let body = if k == 0 {
                Span::styled(text, theme.fg(theme.accent_alt).add_modifier(Modifier::BOLD))
            } else {
                Span::styled(text, Style::default().fg(theme.fg))
            };
            Line::from(vec![bar, body])
        }
    }
}

fn meta_line<'a>(s: &str, theme: &Theme) -> Line<'a> {
    if let Some(rest) = s.strip_prefix("commit ") {
        Line::from(vec![Span::styled("commit ", theme.muted()), Span::styled(rest.to_string(), theme.fg(theme.hash))])
    } else if s.starts_with("Author") || s.starts_with("Commit") || s.starts_with("Merge:") {
        match s.split_once(':') {
            Some((k, v)) => Line::from(vec![Span::styled(format!("{k}:"), theme.muted()), Span::raw(v.to_string())]),
            None => Line::raw(s.to_string()),
        }
    } else if s.starts_with(' ') && s.contains('|') {
        // --stat line: colour the +/- bar.
        let (name, bar) = s.split_once('|').unwrap_or((s, ""));
        let mut spans = vec![Span::raw(name.to_string()), Span::styled("|", theme.muted())];
        for ch in bar.chars() {
            let st = match ch {
                '+' => theme.fg(theme.added),
                '-' => theme.fg(theme.removed),
                _ => Style::default(),
            };
            spans.push(Span::styled(ch.to_string(), st));
        }
        Line::from(spans)
    } else {
        Line::styled(s.to_string(), Style::default().fg(theme.fg).add_modifier(Modifier::BOLD))
    }
}

/// Side-by-side: removed lines on the left paired with added lines on the right.
fn draw_split(f: &mut Frame, area: Rect, view: &DiffView, theme: &Theme, focused: bool) {
    struct VRow<'a> {
        left: Line<'a>,
        right: Line<'a>,
        /// The same two sides without token colors, for when it's selected.
        plain: (Line<'a>, Line<'a>),
        rows: Vec<usize>,
    }
    let gw = gutter_width(view);
    let mut vrows: Vec<VRow> = Vec::new();
    let mut i = 0;
    let side = |fi: usize, hi: usize, li: usize, no: Option<u32>, plain: bool| -> Line {
        let (style, _) = line_style(theme, &view.files[fi].hunks[hi].lines[li]);
        let mut spans = vec![Span::styled(format!("{} ", num(no, gw)), theme.muted())];
        spans.extend(code_spans(view, (fi, hi, li), style, theme, plain));
        Line::from(spans)
    };
    while i < view.rows.len() {
        match view.rows[i] {
            Row::Line(fi, hi, _) => {
                // Gather a run of removed then added lines in the same hunk.
                let lines = &view.files[fi].hunks[hi].lines;
                let mut rem = Vec::new();
                let mut add = Vec::new();
                let mut j = i;
                while j < view.rows.len() {
                    let Row::Line(f2, h2, li) = view.rows[j] else { break };
                    if (f2, h2) != (fi, hi) {
                        break;
                    }
                    match lines[li].kind {
                        DiffLineKind::Removed if add.is_empty() => rem.push((j, li)),
                        DiffLineKind::Added => add.push((j, li)),
                        _ => break,
                    }
                    j += 1;
                }
                if rem.is_empty() && add.is_empty() {
                    let Row::Line(_, _, li) = view.rows[i] else { unreachable!() };
                    let l = &lines[li];
                    vrows.push(VRow {
                        left: side(fi, hi, li, l.old_no, false),
                        right: side(fi, hi, li, l.new_no, false),
                        plain: (side(fi, hi, li, l.old_no, true), side(fi, hi, li, l.new_no, true)),
                        rows: vec![i],
                    });
                    i += 1;
                    continue;
                }
                for k in 0..rem.len().max(add.len()) {
                    let mut rows = Vec::new();
                    let draw = |plain| {
                        let left = rem.get(k).map(|&(_, li)| side(fi, hi, li, lines[li].old_no, plain));
                        let right = add.get(k).map(|&(_, li)| side(fi, hi, li, lines[li].new_no, plain));
                        (left.unwrap_or_default(), right.unwrap_or_default())
                    };
                    rows.extend(rem.get(k).map(|&(r, _)| r));
                    rows.extend(add.get(k).map(|&(r, _)| r));
                    let (left, right) = draw(false);
                    vrows.push(VRow { left, right, plain: draw(true), rows });
                }
                i = j;
            }
            Row::Note(n, _) if view.notes[n].right => {
                // Comments on the new side sit under the right column.
                let l = render_row(view, view.rows[i], theme, 0, false);
                vrows.push(VRow {
                    left: Line::default(),
                    right: l.clone(),
                    plain: (Line::default(), l),
                    rows: vec![i],
                });
                i += 1;
            }
            row => {
                let gw = if matches!(row, Row::Note(..)) { 0 } else { gw };
                let l = render_row(view, row, theme, gw, false);
                vrows.push(VRow {
                    left: l.clone(),
                    right: Line::default(),
                    plain: (l, Line::default()),
                    rows: vec![i],
                });
                i += 1;
            }
        }
    }
    let cur = vrows.iter().position(|v| v.rows.contains(&view.cursor)).unwrap_or(0);
    let h = area.height as usize;
    let start = cur.saturating_sub(h / 2).min(vrows.len().saturating_sub(h));
    let [l, r] = Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)]).spacing(1).areas(area);
    let mut left = Vec::new();
    let mut right = Vec::new();
    for (k, v) in vrows.into_iter().enumerate().skip(start).take(h) {
        let st = Style::default().add_modifier(Modifier::REVERSED);
        if focused && k == cur {
            left.push(v.plain.0.patch_style(st));
            right.push(v.plain.1.patch_style(st));
        } else {
            left.push(v.left);
            right.push(v.right);
        }
    }
    f.render_widget(Paragraph::new(left), l);
    f.render_widget(Paragraph::new(right), r);
}
