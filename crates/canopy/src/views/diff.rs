//! Diff panel state: which diff is shown, the cursor, and line staging.

use canopy_git::parse::diff::PatchMode;
use canopy_git::{DiffLineKind, FileDiff, FileKind};

use crate::app::{App, Level, Msg, Section, Then};
use crate::keymap::Screen;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Row {
    Meta(usize),
    File(usize),
    Hunk(usize, usize),
    Line(usize, usize, usize),
}

#[derive(Debug, Clone)]
pub struct DiffView {
    /// Identity, so a reload of the same diff keeps the cursor.
    pub key: String,
    pub title: String,
    pub meta: Vec<String>,
    pub files: Vec<FileDiff>,
    /// `Some` when lines can be staged/unstaged from this diff.
    pub mode: Option<PatchMode>,
    pub rows: Vec<Row>,
    pub cursor: usize,
    pub anchor: Option<usize>,
    pub scroll: usize,
    pub side_by_side: bool,
}

impl DiffView {
    pub fn new(key: String, title: String, meta: Vec<String>, files: Vec<FileDiff>, mode: Option<PatchMode>) -> Self {
        let mut rows: Vec<Row> = (0..meta.len()).map(Row::Meta).collect();
        for (fi, f) in files.iter().enumerate() {
            rows.push(Row::File(fi));
            for (hi, h) in f.hunks.iter().enumerate() {
                rows.push(Row::Hunk(fi, hi));
                for li in 0..h.lines.len() {
                    rows.push(Row::Line(fi, hi, li));
                }
            }
        }
        let mut v =
            DiffView { key, title, meta, files, mode, rows, cursor: 0, anchor: None, scroll: 0, side_by_side: false };
        if v.mode.is_some() {
            // Start on the first change so `space` does something useful.
            v.cursor = v.rows.iter().position(|r| v.is_change(*r)).unwrap_or(0);
        }
        v
    }

    pub fn is_change(&self, r: Row) -> bool {
        match r {
            Row::Line(fi, hi, li) => {
                matches!(self.files[fi].hunks[hi].lines[li].kind, DiffLineKind::Added | DiffLineKind::Removed)
            }
            _ => false,
        }
    }

    pub fn snap_cursor(&mut self) {
        if self.rows.is_empty() {
            self.cursor = 0;
            return;
        }
        self.cursor = self.cursor.min(self.rows.len() - 1);
    }

    pub fn move_cursor(&mut self, delta: isize) {
        if self.rows.is_empty() {
            return;
        }
        let max = self.rows.len() as isize - 1;
        self.cursor = (self.cursor as isize + delta).clamp(0, max) as usize;
    }

    pub fn jump_hunk(&mut self, forward: bool) {
        let is_hunk = |r: &Row| matches!(r, Row::Hunk(..));
        let found = if forward {
            self.rows.iter().enumerate().skip(self.cursor + 1).find(|(_, r)| is_hunk(r))
        } else {
            self.rows.iter().enumerate().take(self.cursor).rev().find(|(_, r)| is_hunk(r))
        };
        if let Some((i, _)) = found {
            // Land on the first changed line of the hunk.
            self.cursor = (i..self.rows.len()).find(|&j| self.is_change(self.rows[j])).unwrap_or(i);
        }
    }

    pub fn selection(&self) -> (usize, usize) {
        match self.anchor {
            Some(a) => (a.min(self.cursor), a.max(self.cursor)),
            None => (self.cursor, self.cursor),
        }
    }

    /// Selected changed lines grouped by (file, hunk), bottom-most hunk first.
    pub fn selected_lines(&self) -> Vec<(usize, usize, Vec<usize>)> {
        let (a, b) = self.selection();
        let mut groups: Vec<(usize, usize, Vec<usize>)> = Vec::new();
        for r in &self.rows[a..=b.min(self.rows.len().saturating_sub(1))] {
            if let Row::Line(fi, hi, li) = *r {
                if !self.is_change(*r) {
                    continue;
                }
                match groups.last_mut() {
                    Some(g) if g.0 == fi && g.1 == hi => g.2.push(li),
                    _ => groups.push((fi, hi, vec![li])),
                }
            }
        }
        groups.reverse();
        groups
    }

    pub fn current_hunk(&self) -> Option<(usize, usize)> {
        match self.rows.get(self.cursor)? {
            Row::Hunk(fi, hi) | Row::Line(fi, hi, _) => Some((*fi, *hi)),
            _ => None,
        }
    }

    pub fn added_removed(&self) -> (usize, usize) {
        let mut a = 0;
        let mut r = 0;
        for f in &self.files {
            for h in &f.hunks {
                for l in &h.lines {
                    match l.kind {
                        DiffLineKind::Added => a += 1,
                        DiffLineKind::Removed => r += 1,
                        _ => {}
                    }
                }
            }
        }
        (a, r)
    }
}

/// Load the diff that matches the current screen's selection.
pub fn load_for_selection(app: &mut App) {
    let Some(git) = app.git.clone() else { return };
    app.diff_gen += 1;
    let gen = app.diff_gen;

    enum Req {
        Status { path: String, section: Section, untracked: bool },
        Show { rev: String, title: String },
        Stash { name: String },
    }

    let req = match app.screen {
        Screen::Status => app.selected_status_row().map(|row| {
            let f = &app.data.status.files[row.file];
            Req::Status { path: f.path.clone(), section: row.section, untracked: f.kind == FileKind::Untracked }
        }),
        Screen::Log => {
            app.selected_commit().map(|c| Req::Show { rev: c.oid.clone(), title: format!("{} {}", c.short, c.subject) })
        }
        Screen::Branches => {
            app.selected_branch().map(|b| Req::Show { rev: b.oid.clone(), title: format!("{} (tip)", b.name) })
        }
        Screen::Stash => app.data.stashes.get(app.selected(Screen::Stash)).map(|s| Req::Stash { name: s.name.clone() }),
        Screen::Reflog => app
            .data
            .reflog
            .get(app.selected(Screen::Reflog))
            .map(|r| Req::Show { rev: r.oid.clone(), title: format!("{} {}", r.selector, r.subject) }),
        Screen::Home | Screen::Workspace => None,
    };

    let Some(req) = req else {
        app.diff = None;
        return;
    };

    app.spawn(async move {
        let view = match req {
            Req::Status { path, section, untracked } => {
                let res = match (section, untracked) {
                    (Section::Unstaged, true) => git.diff_untracked(&path).await,
                    (Section::Staged, _) => git.diff_file(&path, true, 3).await,
                    _ => git.diff_file(&path, false, 3).await,
                };
                let mode = match section {
                    Section::Unstaged => Some(PatchMode::Stage),
                    Section::Staged => Some(PatchMode::Unstage),
                    Section::Conflicts => None,
                };
                let label = match section {
                    Section::Staged => "staged",
                    Section::Unstaged if untracked => "new file",
                    Section::Unstaged => "unstaged",
                    Section::Conflicts => "conflict",
                };
                res.map(|files| {
                    DiffView::new(
                        format!("status:{label}:{path}"),
                        format!("{path} · {label}"),
                        Vec::new(),
                        files,
                        mode,
                    )
                })
            }
            Req::Show { rev, title } => git.show(&rev).await.map(|(header, files)| {
                let meta = header.lines().map(String::from).collect();
                DiffView::new(format!("show:{rev}"), title, meta, files, None)
            }),
            Req::Stash { name } => git.run(&["stash", "show", "-p", "--no-ext-diff", &name]).await.map(|o| {
                let files = canopy_git::parse::diff::parse(&o.stdout);
                DiffView::new(format!("stash:{name}"), name.clone(), Vec::new(), files, None)
            }),
        };
        Msg::Diff { gen, view: view.map_err(|e| e.to_string()) }
    });
}

/// Stage/unstage the selected lines (or the line under the cursor).
pub fn apply_selection(app: &mut App, whole_hunk: bool) {
    let (Some(git), Some(view)) = (app.git.clone(), app.diff.as_mut()) else { return };
    let Some(mode) = view.mode else {
        app.toast(Level::Warn, "This diff is read-only (resolve conflicts from the Changes list)");
        return;
    };

    let mut jobs: Vec<(FileDiff, canopy_git::Hunk, Vec<usize>)> = Vec::new();
    if whole_hunk {
        if let Some((fi, hi)) = view.current_hunk() {
            let h = view.files[fi].hunks[hi].clone();
            let all = (0..h.lines.len()).collect();
            jobs.push((view.files[fi].clone(), h, all));
        }
    } else {
        for (fi, hi, lines) in view.selected_lines() {
            jobs.push((view.files[fi].clone(), view.files[fi].hunks[hi].clone(), lines));
        }
    }
    view.anchor = None;
    if jobs.is_empty() {
        app.toast(Level::Info, "Move to a + or - line to stage it");
        return;
    }
    let n: usize = jobs.iter().map(|j| j.2.len()).sum();
    let verb = if mode == PatchMode::Stage { "Stage" } else { "Unstage" };
    let label = if whole_hunk { format!("{verb} hunk") } else { format!("{verb} {n} line(s)") };
    app.run_op(label, Then::Refresh, async move {
        let mut last = None;
        // Bottom-most hunk first so earlier hunk line numbers stay valid.
        for (file, hunk, lines) in jobs {
            if let Some(out) = git.apply_lines(&file, &hunk, &lines, mode).await? {
                last = Some(out);
            }
        }
        Ok(last.unwrap_or_else(|| canopy_git::Output {
            cmd: "git apply --cached".into(),
            stdout: String::new(),
            stderr: String::new(),
        }))
    });
}
