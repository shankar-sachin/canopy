use canopy_git::{Change, FileKind, RepoState};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Cell, List, ListItem, ListState, Paragraph, Row, Sparkline, Table, TableState, Wrap};
use ratatui::Frame;

use crate::app::{App, Section};
use crate::input::state_word;
use crate::keymap::{Focus, RefsView, Screen};
use crate::theme::Theme;
use crate::ui::util::{ago, now, panel, trunc};
use crate::ui::{diff, graph};

/// Split into list + diff panes, stacking vertically on narrow terminals.
fn split(area: Rect, list_pct: u16) -> (Rect, Rect) {
    if area.width >= 100 {
        let [a, b] = Layout::horizontal([Constraint::Percentage(list_pct), Constraint::Fill(1)]).areas(area);
        (a, b)
    } else {
        let [a, b] = Layout::vertical([Constraint::Percentage(40), Constraint::Fill(1)]).areas(area);
        (a, b)
    }
}

fn empty(f: &mut Frame, area: Rect, theme: &Theme, title: &str, lines: &[&str]) {
    let block = panel(theme, format!(" {title} "), true);
    let inner = block.inner(area);
    f.render_widget(block, area);
    let mut text = vec![Line::default()];
    for (i, l) in lines.iter().enumerate() {
        text.push(Line::styled(l.to_string(), if i == 0 { theme.accent() } else { theme.muted() }).centered());
    }
    let h = text.len() as u16;
    let r = Rect { y: inner.y + inner.height.saturating_sub(h) / 3, height: h.min(inner.height), ..inner };
    f.render_widget(Paragraph::new(text).wrap(Wrap { trim: true }), r);
}

fn no_repo(f: &mut Frame, area: Rect, theme: &Theme) {
    empty(
        f,
        area,
        theme,
        "No repository",
        &[
            "No repository open",
            "Go to Workspace (6) and press enter on a repo,",
            "or run `canopy` inside a git repository.",
        ],
    );
}

pub fn draw(f: &mut Frame, area: Rect, app: &mut App) {
    if app.git.is_none() && app.screen != Screen::Workspace {
        no_repo(f, area, &app.theme.clone());
        return;
    }
    match app.screen {
        Screen::Home => home(f, area, app),
        Screen::Status => status(f, area, app),
        Screen::Log => log(f, area, app),
        Screen::Branches => branches(f, area, app),
        Screen::Stash => stash(f, area, app),
        Screen::Workspace => workspace(f, area, app),
        Screen::Reflog => reflog(f, area, app),
        Screen::Pulls => pulls(f, area, app),
        Screen::Issues => issues(f, area, app),
    }
}

// ------------------------------------------------------------------ home

struct Suggestion {
    key: String,
    text: String,
    level: u8, // 0 ok, 1 info, 2 warn
}

fn suggestions(app: &App) -> Vec<Suggestion> {
    use crate::keymap::{Action, Ctx};
    // Look keys up in the keymap so remapped keys show correctly.
    let key = |a: Action, screen: Screen| -> String {
        let k = app.keymap.key_for(&[Ctx::Screen(screen), Ctx::Global], a).unwrap_or("?");
        crate::keymap::pretty_key(k)
    };
    let (commit, push, pull, raw) = (
        key(Action::Commit, Screen::Home),
        key(Action::Push, Screen::Home),
        key(Action::Pull, Screen::Home),
        key(Action::RawGit, Screen::Home),
    );
    let (changes, branches) =
        (key(Action::Goto(Screen::Status), Screen::Home), key(Action::Goto(Screen::Branches), Screen::Home));
    let (cont, abort) = (key(Action::ContinueOp, Screen::Status), key(Action::AbortOp, Screen::Status));
    let stage = key(Action::ToggleStage, Screen::Status);
    let new_branch = key(Action::BranchFromCommit, Screen::Log);
    let bisect = key(Action::Bisect, Screen::Home);

    let s = &app.data.status;
    let b = &s.branch;
    let mut out = Vec::new();
    let mut add = |key: &str, text: String, level| out.push(Suggestion { key: key.to_string(), text, level });
    let staged = s.staged().count();
    let unstaged = s.unstaged().count() - s.conflicted().count();
    let conflicts = s.conflicted().count();

    if let Some(state) = app.data.state.filter(|s| *s != RepoState::Clean && *s != RepoState::Bisecting) {
        let w = state_word(state);
        if conflicts > 0 {
            add(&changes, format!("A {w} is in progress with {conflicts} conflict(s). Resolve them in Changes, then press {cont} to continue (or {abort} to abort)."), 2);
        } else {
            add(
                &cont,
                format!("A {w} is in progress and has no conflicts left. Press {cont} (in Changes) to continue."),
                2,
            );
        }
    } else if conflicts > 0 {
        add(&changes, format!("{conflicts} file(s) have conflicts. Open Changes to resolve them."), 2);
    }
    if app.data.state == Some(RepoState::Bisecting) {
        use canopy_git::parse::bisect::BisectStep;
        let text = match &app.bisect {
            Some(BisectStep::Testing { steps_left, oid, subject, .. }) => format!(
                "Bisecting: test {} {subject} (build it, run it), then press {bisect} to mark it good or bad. About {steps_left} step(s) left.",
                oid.get(..7).unwrap_or(oid)
            ),
            Some(BisectStep::Found { oid, subject }) => format!(
                "Bisect found it: {} {subject} introduced the problem. Press {bisect} to finish.",
                oid.get(..7).unwrap_or(oid)
            ),
            Some(BisectStep::Inconclusive) => {
                format!("Bisect can't decide: only skipped commits are left. Press {bisect} to stop.")
            }
            None => format!("A bisect is in progress. Test the checked-out commit, then press {bisect}."),
        };
        add(&bisect, text, 2);
    }
    if b.head.is_none() && b.oid.is_some() {
        add(&branches, format!("You're on a detached HEAD (not on a branch). Check out a branch in Branches, or create one with {new_branch} in History."), 2);
    }
    if staged > 0 {
        add(&commit, format!("{staged} file(s) staged and ready. Press {commit} to commit."), 1);
    } else if unstaged > 0 {
        add(
            &changes,
            format!("{unstaged} changed file(s). Open Changes, stage what you want with {stage}, then commit with {commit}."),
            1,
        );
    }
    if b.upstream.is_some() {
        if b.ahead > 0 && b.behind > 0 {
            add(
                &push,
                format!("Your branch has diverged ({} ahead, {} behind). {push} shows options.", b.ahead, b.behind),
                2,
            );
        } else if b.ahead > 0 {
            add(&push, format!("{} commit(s) not pushed yet. Press {push} to push.", b.ahead), 1);
        } else if b.behind > 0 {
            add(&pull, format!("{} new commit(s) on the remote. Press {pull} to pull.", b.behind), 1);
        }
    } else if b.head.is_some() && !app.data.log.is_empty() {
        if app.data.remotes.is_empty() {
            add(&raw, format!("No remote configured. Add one with {raw} then: remote add origin <url>"), 1);
        } else {
            add(&push, format!("This branch isn't on the remote yet. Press {push} to publish it."), 1);
        }
    }
    if app.data.log.is_empty() && app.loaded {
        add(&commit, format!("Fresh repository. Create some files, then press {commit} to make your first commit."), 1);
    }
    if out.is_empty() {
        out.push(Suggestion {
            key: "✓".into(),
            text: "All clear: everything is committed and in sync.".into(),
            level: 0,
        });
    }
    out
}

fn home(f: &mut Frame, area: Rect, app: &mut App) {
    let theme = app.theme.clone();
    // Narrow terminals get the essentials only: overview, next steps, commits.
    let wide = area.width >= 110;
    let (left, right) = if wide {
        let [l, r] = Layout::horizontal([Constraint::Percentage(58), Constraint::Fill(1)]).areas(area);
        (l, Some(r))
    } else {
        (area, None)
    };

    // --- Overview + suggestions
    let sugg = suggestions(app);
    let [overview, next, recent] =
        Layout::vertical([Constraint::Length(7), Constraint::Length(sugg.len() as u16 * 2 + 2), Constraint::Fill(1)])
            .areas(left);

    let s = &app.data.status;
    let b = &s.branch;
    let branch = b
        .head
        .clone()
        .unwrap_or_else(|| format!("detached @ {}", b.oid.as_deref().map(|o| &o[..7.min(o.len())]).unwrap_or("?")));
    let sync = match &b.upstream {
        Some(u) => {
            let mut v = vec![Span::styled(u.clone(), theme.fg(theme.remote))];
            if b.ahead == 0 && b.behind == 0 {
                v.push(Span::styled("  in sync", theme.fg(theme.added)));
            } else {
                v.push(Span::styled(format!("  ↑{} ↓{}", b.ahead, b.behind), theme.fg(theme.warn)));
            }
            v
        }
        None => vec![Span::styled("no upstream", theme.muted())],
    };
    let stat = |n: usize, label: &str, color| {
        vec![
            Span::styled(
                format!("{n}"),
                if n > 0 { Style::default().fg(color).add_modifier(Modifier::BOLD) } else { theme.muted() },
            ),
            Span::styled(format!(" {label}   "), theme.muted()),
        ]
    };
    let mut counts = Vec::new();
    counts.extend(stat(s.staged().count(), "staged", theme.added));
    counts.extend(stat(s.unstaged().count() - s.conflicted().count(), "changed", theme.modified));
    counts.extend(stat(s.conflicted().count(), "conflicts", theme.conflict));
    counts.extend(stat(app.data.stashes.len(), "stashed", theme.accent_alt));
    let lines = vec![
        Line::from(vec![
            Span::styled("branch   ", theme.muted()),
            Span::styled(branch, theme.fg(theme.branch).add_modifier(Modifier::BOLD)),
        ]),
        Line::from([vec![Span::styled("tracking ", theme.muted())], sync].concat()),
        Line::from(counts),
        Line::from(vec![
            Span::styled("repo     ", theme.muted()),
            Span::raw(app.git.as_ref().map(|g| g.repo.root.display().to_string()).unwrap_or_default()),
        ]),
    ];
    let title = format!(" {} ", app.repo_name());
    f.render_widget(Paragraph::new(lines).block(panel(&theme, title, false)), overview);

    let mut lines = Vec::new();
    for sg in &sugg {
        let color = match sg.level {
            0 => theme.added,
            1 => theme.accent,
            _ => theme.warn,
        };
        lines.push(Line::from(vec![
            Span::styled(format!(" {} ", sg.key), Style::default().fg(theme.bg).bg(color).add_modifier(Modifier::BOLD)),
            Span::raw("  "),
            Span::raw(sg.text.clone()),
        ]));
        lines.push(Line::default());
    }
    f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }).block(panel(&theme, " Next steps ", true)), next);

    // --- Recent commits with graph
    let n = recent.height.saturating_sub(2) as usize;
    let commits: Vec<_> = app.data.log.iter().take(n.max(1)).cloned().collect();
    let g = graph::build(&commits);
    let rows: Vec<Line> = commits
        .iter()
        .zip(g.iter())
        .map(|(c, cells)| {
            let mut spans = graph_spans(cells, &theme);
            spans.push(Span::raw(" "));
            spans.push(Span::styled(c.short.clone(), theme.fg(theme.hash)));
            spans.push(Span::raw(" "));
            spans.extend(ref_spans(&c.refs, &theme));
            spans.push(Span::raw(c.subject.clone()));
            spans.push(Span::styled(format!("  {} · {}", c.author, ago(c.time)), theme.muted()));
            Line::from(spans)
        })
        .collect();
    f.render_widget(Paragraph::new(rows).block(panel(&theme, " Recent commits ", false)), recent);

    // --- Right column: activity, branches, remotes
    let Some(right) = right else { return };
    let [act, br, misc] =
        Layout::vertical([Constraint::Length(7), Constraint::Fill(1), Constraint::Length(6)]).areas(right);
    let days = 30usize;
    let today = now() / 86400;
    let mut buckets = vec![0u64; days];
    for c in &app.data.log {
        let d = today - c.time / 86400;
        if (0..days as i64).contains(&d) {
            buckets[days - 1 - d as usize] += 1;
        }
    }
    let total: u64 = buckets.iter().sum();
    let block = panel(&theme, format!(" Activity · {total} commits in 30 days "), false);
    let inner = block.inner(act);
    f.render_widget(block, act);
    let spark = Sparkline::default().data(&buckets).style(theme.fg(theme.accent));
    f.render_widget(spark, inner);

    let local: Vec<_> =
        app.data.branches.iter().filter(|b| !b.is_remote).take(br.height.saturating_sub(2) as usize).collect();
    let lines: Vec<Line> = local
        .iter()
        .map(|b| {
            let (a, be) = b.track.as_deref().map(canopy_git::parse::refs::parse_track).unwrap_or((0, 0));
            let mut v = vec![
                Span::styled(if b.is_head { "● " } else { "  " }, theme.fg(theme.accent)),
                Span::styled(
                    trunc(&b.name, 28),
                    if b.is_head {
                        theme.fg(theme.branch).add_modifier(Modifier::BOLD)
                    } else {
                        theme.fg(theme.branch)
                    },
                ),
            ];
            if a + be > 0 {
                v.push(Span::styled(format!(" ↑{a}↓{be}"), theme.fg(theme.warn)));
            }
            if b.track.as_deref() == Some("gone") {
                v.push(Span::styled(" (upstream gone)", theme.fg(theme.error)));
            }
            v.push(Span::styled(format!("  {}", ago(b.time)), theme.muted()));
            Line::from(v)
        })
        .collect();
    let nb = app.data.branches.iter().filter(|b| !b.is_remote).count();
    f.render_widget(Paragraph::new(lines).block(panel(&theme, format!(" Branches ({nb}) "), false)), br);

    let mut lines: Vec<Line> = app
        .data
        .remotes
        .iter()
        .map(|r| {
            Line::from(vec![
                Span::styled(format!("{:<8}", r.name), theme.fg(theme.remote)),
                Span::styled(r.fetch_url.clone(), theme.muted()),
            ])
        })
        .collect();
    lines.push(Line::from(vec![
        Span::styled(format!("{} tags", app.data.tags.len()), theme.muted()),
        Span::styled(
            app.data.tags.first().map(|t| format!(" · latest {}", t.name)).unwrap_or_default(),
            theme.fg(theme.tag),
        ),
    ]));
    f.render_widget(Paragraph::new(lines).block(panel(&theme, " Remotes & tags ", false)), misc);
}

// --------------------------------------------------------------- changes

fn change_color(theme: &Theme, c: Change) -> ratatui::style::Color {
    match c {
        Change::Added => theme.added,
        Change::Deleted => theme.removed,
        Change::Renamed | Change::Copied => theme.accent_alt,
        Change::Unmerged => theme.conflict,
        _ => theme.modified,
    }
}

fn status(f: &mut Frame, area: Rect, app: &mut App) {
    let theme = app.theme.clone();
    let rows = app.status_rows();
    if rows.is_empty() {
        let msg = if app.loaded {
            vec![
                "✓ Working tree clean",
                "Edit some files and they'll show up here,",
                "ready to stage (space) and commit (c).",
            ]
        } else {
            vec!["Loading…"]
        };
        empty(f, area, &theme, "Changes", &msg);
        return;
    }
    let (la, da) = split(area, 38);
    let sel = app.selected(Screen::Status).min(rows.len() - 1);

    let mut items: Vec<ListItem> = Vec::new();
    let mut vis_sel = 0;
    let mut last: Option<Section> = None;
    let count = |s: Section| rows.iter().filter(|r| r.section == s).count();
    for (i, r) in rows.iter().enumerate() {
        if last != Some(r.section) {
            if last.is_some() {
                items.push(ListItem::new(Line::default()));
            }
            let (label, color) = match r.section {
                Section::Conflicts => ("Conflicts", theme.conflict),
                Section::Unstaged => ("Changes", theme.modified),
                Section::Staged => ("Staged", theme.added),
            };
            items.push(ListItem::new(Line::from(vec![
                Span::styled(label, Style::default().fg(color).add_modifier(Modifier::BOLD)),
                Span::styled(format!(" {}", count(r.section)), theme.muted()),
            ])));
            last = Some(r.section);
        }
        if i == sel {
            vis_sel = items.len();
        }
        let file = &app.data.status.files[r.file];
        let (code, color) = match (r.section, file.kind) {
            (Section::Conflicts, _) => ("!".to_string(), theme.conflict),
            (_, FileKind::Untracked) => ("?".to_string(), theme.added),
            (Section::Staged, _) => (file.index.code().to_string(), change_color(&theme, file.index)),
            _ => (file.worktree.code().to_string(), change_color(&theme, file.worktree)),
        };
        let (dir, name) = match file.path.rsplit_once('/') {
            Some((d, n)) => (format!("{d}/"), n.to_string()),
            None => (String::new(), file.path.clone()),
        };
        let mut spans = vec![
            Span::styled(format!(" {code} "), Style::default().fg(color).add_modifier(Modifier::BOLD)),
            Span::styled(dir, theme.muted()),
            Span::styled(name, theme.fg(theme.fg)),
        ];
        if let Some(orig) = &file.orig_path {
            spans.push(Span::styled(format!("  ← {orig}"), theme.muted()));
        }
        items.push(ListItem::new(Line::from(spans)));
    }

    let focused = app.focus == Focus::List;
    let staged = count(Section::Staged);
    let title = format!(
        " Changes · {} files · {staged} staged ",
        rows.iter().map(|r| r.file).collect::<std::collections::HashSet<_>>().len()
    );
    let list = List::new(items)
        .block(panel(&theme, title, focused))
        .highlight_style(if focused { theme.selected() } else { Style::default().bg(theme.selection_bg) })
        .highlight_symbol("▌");
    let mut st = ListState::default().with_offset(app.list(Screen::Status).offset()).with_selected(Some(vis_sel));
    f.render_stateful_widget(list, la, &mut st);
    *app.list(Screen::Status).offset_mut() = st.offset();

    let on_conflict = app.selected_status_row().is_some_and(|r| r.section == Section::Conflicts);
    if on_conflict && app.conflict.is_some() {
        crate::ui::conflict::draw(f, da, app);
    } else {
        diff::draw(f, da, app);
    }
}

// --------------------------------------------------------------- history

pub fn graph_spans<'a>(cells: &[graph::Cell], theme: &Theme) -> Vec<Span<'a>> {
    let palette = [theme.accent, theme.branch, theme.accent_alt, theme.hash, theme.remote, theme.conflict, theme.added];
    cells
        .iter()
        .map(|c| {
            let color = palette[c.lane % palette.len()];
            let st = if matches!(c.glyph, '●' | '◉') {
                Style::default().fg(color).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(color)
            };
            Span::styled(c.glyph.to_string(), st)
        })
        .collect()
}

pub fn ref_spans<'a>(refs: &[String], theme: &Theme) -> Vec<Span<'a>> {
    let mut out = Vec::new();
    for r in refs {
        let (text, color) = if let Some(b) = r.strip_prefix("HEAD -> ") {
            (format!("◆ {b}"), theme.accent)
        } else if let Some(t) = r.strip_prefix("tag: ") {
            (format!("⌂ {t}"), theme.tag)
        } else if r == "HEAD" {
            ("◆ HEAD".into(), theme.accent)
        } else if r.contains('/') {
            (r.clone(), theme.remote)
        } else {
            (r.clone(), theme.branch)
        };
        out.push(Span::styled(format!("({text})"), Style::default().fg(color).add_modifier(Modifier::BOLD)));
        out.push(Span::raw(" "));
    }
    out
}

fn log(f: &mut Frame, area: Rect, app: &mut App) {
    let theme = app.theme.clone();
    let vis = app.visible_log();
    if vis.is_empty() {
        let filtered = app.filters.contains_key(&Screen::Log);
        let file_msg;
        let msg: &[&str] = if filtered {
            &["No matching commits", "esc clears the filter"]
        } else if let Some(p) = &app.log_path {
            // log_limit is 0 until the first page for this path arrives.
            file_msg = if app.data.log_limit == 0 {
                format!("Loading history of {p}…")
            } else {
                format!("No commits touch {p} yet")
            };
            &[file_msg.as_str(), "esc shows all history"]
        } else {
            &["No commits yet", "Make your first commit with c"]
        };
        empty(f, area, &theme, "History", msg);
        return;
    }
    let (la, da) = split(area, 55);
    let filtered = app.filters.contains_key(&Screen::Log);
    // The graph only makes sense for the unfiltered, contiguous history.
    let graph = if filtered || app.log_path.is_some() { Vec::new() } else { graph::build(&app.data.log) };
    let gw = graph.iter().map(Vec::len).max().unwrap_or(0).min(24) as u16;
    let rows: Vec<Row> = vis
        .iter()
        .map(|&i| {
            let c = &app.data.log[i];
            let g = graph
                .get(i)
                .map(|cells| Line::from(graph_spans(&cells[..cells.len().min(24)], &theme)))
                .unwrap_or_default();
            let mut subj = ref_spans(&c.refs, &theme);
            subj.push(Span::raw(c.subject.clone()));
            Row::new(vec![
                Cell::from(g),
                Cell::from(Span::styled(c.short.clone(), theme.fg(theme.hash))),
                Cell::from(Line::from(subj)),
                Cell::from(Span::styled(trunc(&c.author, 14), theme.muted())),
                Cell::from(Span::styled(ago(c.time), theme.muted())),
            ])
        })
        .collect();
    let focused = app.focus == Focus::List;
    let more = if app.log_loading {
        " · loading more…"
    } else if app.log_has_more() {
        "+"
    } else {
        ""
    };
    let mut title = match &app.log_path {
        Some(p) => format!(" History of {p} · {}{more} commits · esc for all ", vis.len()),
        None => format!(" History · {}{more} commits ", vis.len()),
    };
    if let Some(flt) = app.filters.get(&Screen::Log) {
        // Filtering only covers what's loaded so far.
        let scope = if app.log_has_more() { format!(" of {} loaded", app.data.log.len()) } else { String::new() };
        title = format!(" History · filter: {flt} · {} matches{scope} ", vis.len());
    }
    let table = Table::new(
        rows,
        [
            Constraint::Length(gw.max(1)),
            Constraint::Length(8),
            Constraint::Fill(1),
            Constraint::Length(14),
            Constraint::Length(4),
        ],
    )
    .column_spacing(1)
    .block(panel(&theme, title, focused))
    .row_highlight_style(if focused { theme.selected() } else { Style::default().bg(theme.selection_bg) });
    let sel = app.selected(Screen::Log);
    let mut st = TableState::default().with_offset(app.list(Screen::Log).offset()).with_selected(Some(sel));
    f.render_stateful_widget(table, la, &mut st);
    *app.list(Screen::Log).offset_mut() = st.offset();
    diff::draw(f, da, app);
}

// -------------------------------------------------------------- branches

/// Panel title with a Branches · Tags · Remotes switcher and a summary.
/// Panel title with a view switcher and a summary. Shows every view name
/// when it fits in `width`, otherwise a compact `‹ Tags 2/5 ›`.
fn refs_title<'a>(app: &App, theme: &Theme, summary: String, width: u16) -> Line<'a> {
    let names: usize = RefsView::ALL.iter().map(|v| v.title().len() + 3).sum();
    let full = names + summary.chars().count() + 6 <= width as usize;
    let mut spans = vec![Span::raw(" ")];
    if full {
        for (i, v) in RefsView::ALL.iter().enumerate() {
            if i > 0 {
                spans.push(Span::styled(" · ", theme.muted()));
            }
            let style =
                if *v == app.refs_view { theme.accent().add_modifier(Modifier::UNDERLINED) } else { theme.muted() };
            spans.push(Span::styled(v.title(), style));
        }
    } else {
        let i = RefsView::ALL.iter().position(|v| *v == app.refs_view).unwrap_or(0);
        spans.push(Span::styled("‹ ", theme.muted()));
        spans.push(Span::styled(app.refs_view.title(), theme.accent()));
        spans.push(Span::styled(format!(" {}/{} ›", i + 1, RefsView::ALL.len()), theme.muted()));
    }
    spans.push(Span::styled(format!("  {summary} "), theme.muted()));
    Line::from(spans)
}

fn branches(f: &mut Frame, area: Rect, app: &mut App) {
    match app.refs_view {
        RefsView::Branches => branch_list(f, area, app),
        RefsView::Tags => tags(f, area, app),
        RefsView::Remotes => remotes(f, area, app),
        RefsView::Worktrees => worktrees(f, area, app),
        RefsView::Submodules => submodules(f, area, app),
    }
}

fn submodules(f: &mut Frame, area: Rect, app: &mut App) {
    use canopy_git::parse::submodule::SubmoduleState::*;
    let theme = app.theme.clone();
    let (la, da) = split(area, 50);
    let focused = app.focus == Focus::List;
    let title = refs_title(app, &theme, format!("{} submodules", app.data.submodules.len()), la.width);
    if app.data.submodules.is_empty() {
        let lines = vec![
            Line::styled("No submodules.", theme.muted()),
            Line::styled("A submodule is another git repository embedded at a", theme.muted()),
            Line::styled("pinned commit inside this one (added with", theme.muted()),
            Line::styled("`git submodule add <url> <path>`).", theme.muted()),
        ];
        f.render_widget(Paragraph::new(lines).block(panel(&theme, title, focused)), la);
        diff::draw(f, da, app);
        return;
    }
    let rows: Vec<Row> = app
        .data
        .submodules
        .iter()
        .map(|s| {
            let state = match s.state {
                InSync => Span::styled("✓ in sync", theme.fg(theme.added)),
                Uninitialized => Span::styled("not checked out", theme.muted()),
                Modified => Span::styled("● moved", theme.fg(theme.modified)),
                Conflict => Span::styled("! conflict", theme.fg(theme.conflict)),
            };
            Row::new(vec![
                Cell::from(Span::styled(s.path.clone(), theme.fg(theme.fg).add_modifier(Modifier::BOLD))),
                Cell::from(Span::styled(s.oid.get(..7).unwrap_or(&s.oid).to_string(), theme.fg(theme.hash))),
                Cell::from(Span::styled(s.describe.clone().unwrap_or_default(), theme.fg(theme.tag))),
                Cell::from(state),
            ])
        })
        .collect();
    let table =
        Table::new(rows, [Constraint::Fill(1), Constraint::Length(8), Constraint::Length(16), Constraint::Length(16)])
            .column_spacing(1)
            .block(panel(&theme, title, focused))
            .row_highlight_style(if focused { theme.selected() } else { Style::default().bg(theme.selection_bg) });
    let sel = app.selected(Screen::Branches);
    let mut st = TableState::default().with_offset(app.list(Screen::Branches).offset()).with_selected(Some(sel));
    f.render_stateful_widget(table, la, &mut st);
    *app.list(Screen::Branches).offset_mut() = st.offset();
    diff::draw(f, da, app);
}

fn worktrees(f: &mut Frame, area: Rect, app: &mut App) {
    let theme = app.theme.clone();
    let (la, da) = split(area, 50);
    let focused = app.focus == Focus::List;
    let title = refs_title(app, &theme, format!("{} worktrees", app.data.worktrees.len()), la.width);
    let current = app.git.as_ref().map(|g| g.repo.root.clone());
    let rows: Vec<Row> = app
        .data
        .worktrees
        .iter()
        .enumerate()
        .map(|(i, w)| {
            let is_cur = current.as_ref() == Some(&w.path);
            let branch = match (&w.branch, w.bare) {
                (Some(b), _) => Span::styled(b.clone(), theme.fg(theme.branch)),
                (None, true) => Span::styled("(bare)", theme.muted()),
                (None, false) => Span::styled("(detached)", theme.fg(theme.warn)),
            };
            let state = if w.prunable.is_some() {
                Span::styled("missing", theme.fg(theme.error))
            } else if w.locked.is_some() {
                Span::styled("locked", theme.fg(theme.warn))
            } else if i == 0 {
                Span::styled("main", theme.muted())
            } else {
                Span::raw("")
            };
            Row::new(vec![
                Cell::from(Span::styled(if is_cur { "●" } else { " " }, theme.fg(theme.accent))),
                Cell::from(branch),
                Cell::from(Span::styled(w.path.display().to_string(), theme.fg(theme.fg))),
                Cell::from(state),
            ])
        })
        .collect();
    let table = Table::new(
        rows,
        [Constraint::Length(1), Constraint::Percentage(30), Constraint::Fill(1), Constraint::Length(8)],
    )
    .column_spacing(1)
    .block(panel(&theme, title, focused))
    .row_highlight_style(if focused { theme.selected() } else { Style::default().bg(theme.selection_bg) });
    let sel = app.selected(Screen::Branches);
    let mut st = TableState::default().with_offset(app.list(Screen::Branches).offset()).with_selected(Some(sel));
    f.render_stateful_widget(table, la, &mut st);
    *app.list(Screen::Branches).offset_mut() = st.offset();
    diff::draw(f, da, app);
}

fn tags(f: &mut Frame, area: Rect, app: &mut App) {
    let theme = app.theme.clone();
    let vis = app.visible_tags();
    let (la, da) = split(area, 50);
    let focused = app.focus == Focus::List;
    let title = refs_title(app, &theme, format!("{} tags", app.data.tags.len()), la.width);
    if vis.is_empty() {
        let msg = if app.data.tags.is_empty() {
            "No tags yet. Tags mark releases: press n to tag HEAD."
        } else {
            "No matching tags (esc clears the filter)."
        };
        f.render_widget(Paragraph::new(Line::styled(msg, theme.muted())).block(panel(&theme, title, focused)), la);
        diff::draw(f, da, app);
        return;
    }
    let rows: Vec<Row> = vis
        .iter()
        .map(|&i| {
            let t = &app.data.tags[i];
            Row::new(vec![
                Cell::from(Span::styled(t.name.clone(), theme.fg(theme.tag).add_modifier(Modifier::BOLD))),
                Cell::from(Span::styled(t.oid[..7.min(t.oid.len())].to_string(), theme.fg(theme.hash))),
                Cell::from(Span::styled(t.subject.clone(), theme.muted())),
                Cell::from(Span::styled(ago(t.time), theme.muted())),
            ])
        })
        .collect();
    let table = Table::new(
        rows,
        [Constraint::Percentage(30), Constraint::Length(8), Constraint::Fill(1), Constraint::Length(4)],
    )
    .column_spacing(1)
    .block(panel(&theme, title, focused))
    .row_highlight_style(if focused { theme.selected() } else { Style::default().bg(theme.selection_bg) });
    let sel = app.selected(Screen::Branches);
    let mut st = TableState::default().with_offset(app.list(Screen::Branches).offset()).with_selected(Some(sel));
    f.render_stateful_widget(table, la, &mut st);
    *app.list(Screen::Branches).offset_mut() = st.offset();
    diff::draw(f, da, app);
}

fn remotes(f: &mut Frame, area: Rect, app: &mut App) {
    let theme = app.theme.clone();
    let (la, da) = split(area, 50);
    let focused = app.focus == Focus::List;
    let title = refs_title(app, &theme, format!("{} remotes", app.data.remotes.len()), la.width);
    if app.data.remotes.is_empty() {
        let lines = vec![
            Line::styled("No remotes yet.", theme.muted()),
            Line::styled("A remote is a copy of this repo somewhere else, like GitHub.", theme.muted()),
            Line::styled("Press n and enter e.g.  origin git@github.com:you/repo.git", theme.muted()),
        ];
        f.render_widget(Paragraph::new(lines).block(panel(&theme, title, focused)), la);
        diff::draw(f, da, app);
        return;
    }
    let rows: Vec<Row> = app
        .data
        .remotes
        .iter()
        .map(|r| {
            let n =
                app.data.branches.iter().filter(|b| b.is_remote && b.name.starts_with(&format!("{}/", r.name))).count();
            Row::new(vec![
                Cell::from(Span::styled(r.name.clone(), theme.fg(theme.remote).add_modifier(Modifier::BOLD))),
                Cell::from(Span::styled(r.fetch_url.clone(), theme.fg(theme.fg))),
                Cell::from(Span::styled(format!("{n} branches"), theme.muted())),
            ])
        })
        .collect();
    let table = Table::new(rows, [Constraint::Length(12), Constraint::Fill(1), Constraint::Length(12)])
        .column_spacing(1)
        .block(panel(&theme, title, focused))
        .row_highlight_style(if focused { theme.selected() } else { Style::default().bg(theme.selection_bg) });
    let sel = app.selected(Screen::Branches);
    let mut st = TableState::default().with_offset(app.list(Screen::Branches).offset()).with_selected(Some(sel));
    f.render_stateful_widget(table, la, &mut st);
    *app.list(Screen::Branches).offset_mut() = st.offset();
    diff::draw(f, da, app);
}

fn branch_list(f: &mut Frame, area: Rect, app: &mut App) {
    let theme = app.theme.clone();
    let vis = app.visible_branches();
    if vis.is_empty() {
        empty(f, area, &theme, "Branches", &["No branches yet", "Make a commit first, then create branches with n"]);
        return;
    }
    let (la, da) = split(area, 50);
    let mut rows = Vec::new();
    let mut last_remote = None;
    let sel = app.selected(Screen::Branches);
    let mut vis_sel = 0;
    for (k, &i) in vis.iter().enumerate() {
        let b = &app.data.branches[i];
        if last_remote != Some(b.is_remote) {
            if last_remote.is_some() {
                rows.push(Row::new(vec![Cell::from("")]));
            }
            let label = if b.is_remote { "Remote" } else { "Local" };
            rows.push(Row::new(vec![
                Cell::from(""),
                Cell::from(Span::styled(label, theme.muted().add_modifier(Modifier::BOLD))),
            ]));
            last_remote = Some(b.is_remote);
        }
        if k == sel {
            vis_sel = rows.len();
        }
        let (a, be) = b.track.as_deref().map(canopy_git::parse::refs::parse_track).unwrap_or((0, 0));
        let track = if b.track.as_deref() == Some("gone") {
            Span::styled("gone", theme.fg(theme.error))
        } else if a + be > 0 {
            Span::styled(format!("↑{a} ↓{be}"), theme.fg(theme.warn))
        } else if b.upstream.is_some() {
            Span::styled("✓", theme.fg(theme.added))
        } else {
            Span::raw("")
        };
        let name_style = if b.is_head {
            theme.fg(theme.accent).add_modifier(Modifier::BOLD)
        } else if b.is_remote {
            theme.fg(theme.remote)
        } else {
            theme.fg(theme.branch)
        };
        rows.push(Row::new(vec![
            Cell::from(Span::styled(if b.is_head { "●" } else { " " }, theme.fg(theme.accent))),
            Cell::from(Span::styled(b.name.clone(), name_style)),
            Cell::from(track),
            Cell::from(Span::styled(b.subject.clone(), theme.muted())),
            Cell::from(Span::styled(ago(b.time), theme.muted())),
        ]));
    }
    let focused = app.focus == Focus::List;
    let nl = app.data.branches.iter().filter(|b| !b.is_remote).count();
    let table = Table::new(
        rows,
        [
            Constraint::Length(1),
            Constraint::Percentage(35),
            Constraint::Length(8),
            Constraint::Fill(1),
            Constraint::Length(4),
        ],
    )
    .column_spacing(1)
    .block(panel(
        &theme,
        refs_title(app, &theme, format!("{nl} local · {} remote", app.data.branches.len() - nl), la.width),
        focused,
    ))
    .row_highlight_style(if focused { theme.selected() } else { Style::default().bg(theme.selection_bg) });
    let mut st = TableState::default().with_offset(app.list(Screen::Branches).offset()).with_selected(Some(vis_sel));
    f.render_stateful_widget(table, la, &mut st);
    *app.list(Screen::Branches).offset_mut() = st.offset();
    diff::draw(f, da, app);
}

// ----------------------------------------------------------------- stash

fn stash(f: &mut Frame, area: Rect, app: &mut App) {
    let theme = app.theme.clone();
    if app.data.stashes.is_empty() {
        empty(
            f,
            area,
            &theme,
            "Stash",
            &[
                "No stashes",
                "A stash shelves your uncommitted changes so you can",
                "switch tasks. Press S to stash, then pop it back here.",
            ],
        );
        return;
    }
    let (la, da) = split(area, 45);
    let rows: Vec<Row> = app
        .data
        .stashes
        .iter()
        .map(|s| {
            Row::new(vec![
                Cell::from(Span::styled(s.name.clone(), theme.fg(theme.accent_alt))),
                Cell::from(s.message.clone()),
                Cell::from(Span::styled(ago(s.time), theme.muted())),
            ])
        })
        .collect();
    let focused = app.focus == Focus::List;
    let table = Table::new(rows, [Constraint::Length(10), Constraint::Fill(1), Constraint::Length(4)])
        .block(panel(&theme, format!(" Stash · {} ", app.data.stashes.len()), focused))
        .row_highlight_style(if focused { theme.selected() } else { Style::default().bg(theme.selection_bg) });
    let sel = app.selected(Screen::Stash);
    let mut st = TableState::default().with_offset(app.list(Screen::Stash).offset()).with_selected(Some(sel));
    f.render_stateful_widget(table, la, &mut st);
    *app.list(Screen::Stash).offset_mut() = st.offset();
    diff::draw(f, da, app);
}

// ------------------------------------------------------------- workspace

fn workspace(f: &mut Frame, area: Rect, app: &mut App) {
    let theme = app.theme.clone();
    let vis = app.visible_workspace();
    if vis.is_empty() {
        let roots = crate::workspace::roots(&app.config, &app.workspace_root);
        let where_ = roots.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join(", ");
        let l2 = format!("Looked in: {where_}");
        let msg: Vec<&str> = if app.workspace_scanning {
            vec!["Scanning for repositories…"]
        } else {
            vec!["No repositories found", &l2, "Set workspace_dirs in ~/.config/canopy/config.toml"]
        };
        empty(f, area, &theme, "Workspace", &msg);
        return;
    }
    let current = app.git.as_ref().map(|g| g.repo.root.clone());
    let rows: Vec<Row> = vis
        .iter()
        .map(|&i| {
            let r = &app.workspace[i];
            let is_cur = current.as_ref() == Some(&r.path);
            let state = if let Some(e) = &r.error {
                Span::styled(trunc(e, 20), theme.fg(theme.error))
            } else if r.conflicts > 0 {
                Span::styled(format!("! {} conflicts", r.conflicts), theme.fg(theme.conflict))
            } else if r.dirty() > 0 {
                Span::styled(format!("● {} changed", r.dirty()), theme.fg(theme.modified))
            } else {
                Span::styled("✓ clean", theme.fg(theme.added))
            };
            let sync = if !r.has_upstream {
                Span::styled("local", theme.muted())
            } else if r.ahead + r.behind == 0 {
                Span::styled("in sync", theme.muted())
            } else {
                Span::styled(format!("↑{} ↓{}", r.ahead, r.behind), theme.fg(theme.warn))
            };
            Row::new(vec![
                Cell::from(Span::styled(if is_cur { "●" } else { " " }, theme.fg(theme.accent))),
                Cell::from(Span::styled(
                    r.name.clone(),
                    if is_cur { theme.accent() } else { theme.fg(theme.fg).add_modifier(Modifier::BOLD) },
                )),
                Cell::from(Span::styled(r.branch.clone(), theme.fg(theme.branch))),
                Cell::from(state),
                Cell::from(sync),
                Cell::from(Span::styled(r.last_subject.clone(), theme.muted())),
                Cell::from(Span::styled(ago(r.last_time), theme.muted())),
            ])
        })
        .collect();
    let header = Row::new(
        ["", "Repository", "Branch", "Status", "Remote", "Last commit", ""]
            .map(|h| Cell::from(Span::styled(h, theme.muted().add_modifier(Modifier::BOLD)))),
    )
    .bottom_margin(0);
    let dirty = app.workspace.iter().filter(|r| r.dirty() > 0).count();
    let unsynced = app.workspace.iter().filter(|r| r.ahead + r.behind > 0).count();
    let mut title =
        format!(" Workspace · {} repos · {dirty} with changes · {unsynced} out of sync ", app.workspace.len());
    if app.workspace_scanning {
        title.push_str("· scanning… ");
    }
    let table = Table::new(
        rows,
        [
            Constraint::Length(1),
            Constraint::Percentage(18),
            Constraint::Percentage(16),
            Constraint::Length(16),
            Constraint::Length(9),
            Constraint::Fill(1),
            Constraint::Length(4),
        ],
    )
    .header(header)
    .column_spacing(2)
    .block(panel(&theme, title, true))
    .row_highlight_style(theme.selected());
    let sel = app.selected(Screen::Workspace);
    let mut st = TableState::default().with_offset(app.list(Screen::Workspace).offset()).with_selected(Some(sel));
    f.render_stateful_widget(table, area, &mut st);
    *app.list(Screen::Workspace).offset_mut() = st.offset();
}

// ---------------------------------------------------------------- github

/// Shown instead of a GitHub tab when `gh` can't be used. Returns true if drawn.
fn github_setup(f: &mut Frame, area: Rect, app: &App, title: &str) -> bool {
    use canopy_gh::GhStatus;
    let theme = &app.theme;
    let lines: Vec<&str> = match &app.github.status {
        None => vec!["Checking GitHub…"],
        Some(GhStatus::Ready(_)) => return false,
        Some(GhStatus::NotInstalled) => vec![
            "GitHub features need the GitHub CLI (gh)",
            "Canopy never installs anything for you. To set it up, run in a terminal:",
            "brew install gh",
            "then: gh auth login",
            "and reopen Canopy.",
        ],
        Some(GhStatus::NotLoggedIn) => {
            vec!["You're not logged in to GitHub", "Run this in a terminal, then reopen Canopy:", "gh auth login"]
        }
        Some(GhStatus::NotGitHub) => vec![
            "This repository isn't on GitHub",
            "GitHub tabs work when a remote points at github.com.",
            "Add one in Branches → Remotes (4, then ]).",
        ],
    };
    empty(f, area, theme, title, &lines);
    true
}

fn check_span<'a>(pr: &canopy_gh::PullRequest, theme: &Theme) -> Span<'a> {
    use canopy_gh::CheckState;
    let c = pr.checks();
    match c.overall() {
        None => Span::raw(""),
        Some(CheckState::Passed) => Span::styled(format!("✓ {}/{}", c.passed, c.total), theme.fg(theme.added)),
        Some(CheckState::Failed) => Span::styled(format!("✗ {} failed", c.failed), theme.fg(theme.error)),
        Some(_) => Span::styled(format!("… {} running", c.pending), theme.fg(theme.warn)),
    }
}

fn pulls(f: &mut Frame, area: Rect, app: &mut App) {
    let theme = app.theme.clone();
    if github_setup(f, area, app, "Pull requests") {
        return;
    }
    let filter = app.github.pr_filter.label();
    if app.github.prs.is_empty() {
        let msg = if app.github.prs_loading || !app.github.prs_loaded {
            vec!["Loading pull requests…".to_string()]
        } else {
            vec![
                format!("No pull requests ({filter})"),
                "f changes the filter · n opens one for the current branch".to_string(),
            ]
        };
        let refs: Vec<&str> = msg.iter().map(String::as_str).collect();
        empty(f, area, &theme, "Pull requests", &refs);
        return;
    }
    let (la, da) = split(area, 50);
    let me = app.github.viewer.clone().unwrap_or_default();
    let rows: Vec<Row> = app
        .github
        .prs
        .iter()
        .map(|pr| {
            let review = match pr.review_decision.as_str() {
                "APPROVED" => Span::styled("approved", theme.fg(theme.added)),
                "CHANGES_REQUESTED" => Span::styled("changes", theme.fg(theme.error)),
                "REVIEW_REQUIRED" => Span::styled("review", theme.fg(theme.warn)),
                _ => Span::raw(""),
            };
            let state_color = match pr.state.as_str() {
                "MERGED" => theme.hash,
                "CLOSED" => theme.error,
                _ if pr.is_draft => theme.muted,
                _ => theme.added,
            };
            let author_style = if pr.author.login == me { theme.fg(theme.accent) } else { theme.muted() };
            let mut title = vec![Span::raw(pr.title.clone())];
            if pr.is_draft {
                title.push(Span::styled(" draft", theme.muted()));
            }
            Row::new(vec![
                Cell::from(Span::styled(
                    format!("#{}", pr.number),
                    Style::default().fg(state_color).add_modifier(Modifier::BOLD),
                )),
                Cell::from(Line::from(title)),
                Cell::from(Span::styled(trunc(&pr.author.login, 14), author_style)),
                Cell::from(check_span(pr, &theme)),
                Cell::from(review),
                Cell::from(Span::styled(ago(pr.updated_at), theme.muted())),
            ])
        })
        .collect();
    let focused = app.focus == Focus::List;
    let loading = if app.github.prs_loading { " · refreshing…" } else { "" };
    let repo = app.github.repo_name().unwrap_or_default();
    let title = format!(" Pull requests · {repo} · {filter} · {}{loading} ", app.github.prs.len());
    let table = Table::new(
        rows,
        [
            Constraint::Length(6),
            Constraint::Fill(1),
            Constraint::Length(14),
            Constraint::Length(11),
            Constraint::Length(8),
            Constraint::Length(4),
        ],
    )
    .column_spacing(1)
    .block(panel(&theme, title, focused))
    .row_highlight_style(if focused { theme.selected() } else { Style::default().bg(theme.selection_bg) });
    let sel = app.selected(Screen::Pulls);
    let mut st = TableState::default().with_offset(app.list(Screen::Pulls).offset()).with_selected(Some(sel));
    f.render_stateful_widget(table, la, &mut st);
    *app.list(Screen::Pulls).offset_mut() = st.offset();
    diff::draw(f, da, app);
}

fn issues(f: &mut Frame, area: Rect, app: &mut App) {
    let theme = app.theme.clone();
    if github_setup(f, area, app, "Issues") {
        return;
    }
    let filter = app.github.issue_filter.label();
    if app.github.issues.is_empty() {
        let msg = if app.github.issues_loading || !app.github.issues_loaded {
            vec!["Loading issues…".to_string()]
        } else {
            vec![format!("No issues ({filter})"), "f changes the filter · n opens a new issue".to_string()]
        };
        let refs: Vec<&str> = msg.iter().map(String::as_str).collect();
        empty(f, area, &theme, "Issues", &refs);
        return;
    }
    let (la, da) = split(area, 50);
    let rows: Vec<Row> = app
        .github
        .issues
        .iter()
        .map(|i| {
            let color = if i.state == "OPEN" { theme.added } else { theme.hash };
            let mut title = vec![Span::raw(i.title.clone())];
            for l in i.labels.iter().take(3) {
                title.push(Span::styled(format!(" {}", l.name), theme.fg(theme.tag)));
            }
            let comments = if i.comments.is_empty() { String::new() } else { format!("💬 {}", i.comments.len()) };
            Row::new(vec![
                Cell::from(Span::styled(
                    format!("#{}", i.number),
                    Style::default().fg(color).add_modifier(Modifier::BOLD),
                )),
                Cell::from(Line::from(title)),
                Cell::from(Span::styled(trunc(&i.author.login, 14), theme.muted())),
                Cell::from(Span::styled(comments, theme.muted())),
                Cell::from(Span::styled(ago(i.updated_at), theme.muted())),
            ])
        })
        .collect();
    let focused = app.focus == Focus::List;
    let loading = if app.github.issues_loading { " · refreshing…" } else { "" };
    let repo = app.github.repo_name().unwrap_or_default();
    let title = format!(" Issues · {repo} · {filter} · {}{loading} ", app.github.issues.len());
    let table = Table::new(
        rows,
        [
            Constraint::Length(6),
            Constraint::Fill(1),
            Constraint::Length(14),
            Constraint::Length(5),
            Constraint::Length(4),
        ],
    )
    .column_spacing(1)
    .block(panel(&theme, title, focused))
    .row_highlight_style(if focused { theme.selected() } else { Style::default().bg(theme.selection_bg) });
    let sel = app.selected(Screen::Issues);
    let mut st = TableState::default().with_offset(app.list(Screen::Issues).offset()).with_selected(Some(sel));
    f.render_stateful_widget(table, la, &mut st);
    *app.list(Screen::Issues).offset_mut() = st.offset();
    diff::draw(f, da, app);
}

// ---------------------------------------------------------------- reflog

fn reflog(f: &mut Frame, area: Rect, app: &mut App) {
    let theme = app.theme.clone();
    if app.data.reflog.is_empty() {
        empty(
            f,
            area,
            &theme,
            "Reflog",
            &["Reflog is empty", "Every commit, checkout, and reset you make shows up here"],
        );
        return;
    }
    let (la, da) = split(area, 55);
    let rows: Vec<Row> = app
        .data
        .reflog
        .iter()
        .map(|r| {
            let (kind, rest) = r.subject.split_once(": ").unwrap_or(("", &r.subject));
            Row::new(vec![
                Cell::from(Span::styled(r.oid[..7.min(r.oid.len())].to_string(), theme.fg(theme.hash))),
                Cell::from(Span::styled(kind.to_string(), theme.fg(theme.accent_alt))),
                Cell::from(rest.to_string()),
                Cell::from(Span::styled(ago(r.time), theme.muted())),
            ])
        })
        .collect();
    let focused = app.focus == Focus::List;
    let table =
        Table::new(rows, [Constraint::Length(8), Constraint::Length(16), Constraint::Fill(1), Constraint::Length(4)])
            .block(panel(&theme, " Reflog · where HEAD has been (your safety net) ", focused))
            .row_highlight_style(if focused { theme.selected() } else { Style::default().bg(theme.selection_bg) });
    let sel = app.selected(Screen::Reflog);
    let mut st = TableState::default().with_offset(app.list(Screen::Reflog).offset()).with_selected(Some(sel));
    f.render_stateful_widget(table, la, &mut st);
    *app.list(Screen::Reflog).offset_mut() = st.offset();
    diff::draw(f, da, app);
}
