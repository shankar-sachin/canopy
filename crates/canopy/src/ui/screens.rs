use canopy_git::{Change, FileKind, RepoState};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Cell, List, ListItem, ListState, Paragraph, Row, Sparkline, Table, TableState, Wrap};
use ratatui::Frame;

use crate::app::{App, Section};
use crate::input::state_word;
use crate::keymap::{Focus, Screen};
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
    }
}

// ------------------------------------------------------------------ home

struct Suggestion {
    key: &'static str,
    text: String,
    level: u8, // 0 ok, 1 info, 2 warn
}

fn suggestions(app: &App) -> Vec<Suggestion> {
    let s = &app.data.status;
    let b = &s.branch;
    let mut out = Vec::new();
    let mut add = |key, text: String, level| out.push(Suggestion { key, text, level });
    let staged = s.staged().count();
    let unstaged = s.unstaged().count() - s.conflicted().count();
    let conflicts = s.conflicted().count();

    if let Some(state) = app.data.state.filter(|s| *s != RepoState::Clean && *s != RepoState::Bisecting) {
        let w = state_word(state);
        if conflicts > 0 {
            add("2", format!("A {w} is in progress with {conflicts} conflict(s). Resolve them in Changes, then press C to continue (or X to abort)."), 2);
        } else {
            add("C", format!("A {w} is in progress and has no conflicts left. Press C (in Changes) to continue."), 2);
        }
    } else if conflicts > 0 {
        add("2", format!("{conflicts} file(s) have conflicts. Open Changes to resolve them."), 2);
    }
    if b.head.is_none() && b.oid.is_some() {
        add("4", "You're on a detached HEAD (not on a branch). Check out a branch in Branches, or create one with n in History.".into(), 2);
    }
    if staged > 0 {
        add("c", format!("{staged} file(s) staged and ready. Press c to commit."), 1);
    } else if unstaged > 0 {
        add(
            "2",
            format!("{unstaged} changed file(s). Open Changes, stage what you want with space, then commit with c."),
            1,
        );
    }
    if b.upstream.is_some() {
        if b.ahead > 0 && b.behind > 0 {
            add("P", format!("Your branch has diverged ({} ahead, {} behind). P shows options.", b.ahead, b.behind), 2);
        } else if b.ahead > 0 {
            add("P", format!("{} commit(s) not pushed yet. Press P to push.", b.ahead), 1);
        } else if b.behind > 0 {
            add("p", format!("{} new commit(s) on the remote. Press p to pull.", b.behind), 1);
        }
    } else if b.head.is_some() && !app.data.log.is_empty() {
        if app.data.remotes.is_empty() {
            add("!", "No remote configured. Add one with ! then: remote add origin <url>".into(), 1);
        } else {
            add("P", "This branch isn't on the remote yet. Press P to publish it.".into(), 1);
        }
    }
    if app.data.log.is_empty() && app.loaded {
        add("c", "Fresh repository. Create some files, then press c to make your first commit.".into(), 1);
    }
    if out.is_empty() {
        out.push(Suggestion { key: "✓", text: "All clear: everything is committed and in sync.".into(), level: 0 });
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

    diff::draw(f, da, app);
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
        let msg: &[&str] = if filtered {
            &["No matching commits", "esc clears the filter"]
        } else {
            &["No commits yet", "Make your first commit with c"]
        };
        empty(f, area, &theme, "History", msg);
        return;
    }
    let (la, da) = split(area, 55);
    let filtered = app.filters.contains_key(&Screen::Log);
    // The graph only makes sense for the unfiltered, contiguous history.
    let graph = if filtered { Vec::new() } else { graph::build(&app.data.log) };
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
    let mut title = format!(" History · {} commits ", vis.len());
    if let Some(flt) = app.filters.get(&Screen::Log) {
        title = format!(" History · filter: {flt} · {} matches ", vis.len());
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

fn branches(f: &mut Frame, area: Rect, app: &mut App) {
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
    .block(panel(&theme, format!(" Branches · {nl} local · {} remote ", app.data.branches.len() - nl), focused))
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
