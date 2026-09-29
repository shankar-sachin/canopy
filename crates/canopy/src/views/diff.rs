//! Diff panel state: which diff is shown, the cursor, and line staging.

use canopy_git::parse::diff::PatchMode;
use canopy_git::{DiffLineKind, FileDiff, FileKind};

use crate::app::{App, Level, Msg, Section, Then};
use crate::keymap::{RefsView, Screen};
use crate::views::conflict::ConflictView;
use canopy_git::parse::conflict;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Row {
    Meta(usize),
    File(usize),
    Hunk(usize, usize),
    Line(usize, usize, usize),
    /// A review comment under a line: (note, line of the note's text).
    Note(usize, usize),
}

/// A PR review comment attached to a diff line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Note {
    pub file: usize,
    /// Line number on the side the comment is on.
    pub line: u32,
    /// true = new side (added/unchanged lines), false = removed lines.
    pub right: bool,
    /// First line is "author · age", then the comment text.
    pub text: Vec<String>,
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
    /// The commit this diff shows, when it's a commit (for blame).
    pub rev: Option<String>,
    /// Review comments shown under their lines (PR diffs).
    pub notes: Vec<Note>,
    /// Syntax highlighting per file, hunk and line (empty: none).
    pub hl: Vec<Vec<Vec<canopy_highlight::Spans>>>,
}

impl DiffView {
    pub fn new(key: String, title: String, meta: Vec<String>, files: Vec<FileDiff>, mode: Option<PatchMode>) -> Self {
        let rows = build_rows(meta.len(), &files, &[]);
        let hl = highlight(&files);
        let mut v = DiffView {
            key,
            title,
            meta,
            files,
            mode,
            rows,
            cursor: 0,
            anchor: None,
            scroll: 0,
            side_by_side: false,
            rev: None,
            notes: Vec::new(),
            hl,
        };
        if v.mode.is_some() {
            // Start on the first change so `space` does something useful.
            v.cursor = v.rows.iter().position(|r| v.is_change(*r)).unwrap_or(0);
        }
        v
    }

    /// Show review comments under the lines they're about.
    pub fn attach_notes(&mut self, notes: Vec<Note>) {
        self.rows = build_rows(self.meta.len(), &self.files, &notes);
        self.notes = notes;
        self.snap_cursor();
    }

    /// The diff line under the cursor as (file index, line number, new side?),
    /// for commenting on it. Removed lines are on the old side.
    pub fn line_target(&self) -> Option<(usize, u32, bool)> {
        let Row::Line(fi, hi, li) = *self.rows.get(self.cursor)? else { return None };
        let l = &self.files[fi].hunks[hi].lines[li];
        match l.kind {
            DiffLineKind::Removed => l.old_no.map(|n| (fi, n, false)),
            DiffLineKind::NoNewline => None,
            _ => l.new_no.map(|n| (fi, n, true)),
        }
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
        // Land on the first changed line of the hunk.
        let land = |i: usize| (i..self.rows.len()).find(|&j| self.is_change(self.rows[j])).unwrap_or(i);
        let found = if forward {
            self.rows.iter().enumerate().skip(self.cursor + 1).find(|(_, r)| is_hunk(r)).map(|(i, _)| land(i))
        } else {
            // Going back from a hunk's first change (where a jump lands) goes
            // to the hunk before it, not to the same spot.
            (0..self.cursor).rev().filter(|&i| is_hunk(&self.rows[i])).map(land).find(|&to| to < self.cursor)
        };
        if let Some(to) = found {
            self.cursor = to;
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
        // An empty diff, or a cursor left past the end after the diff got
        // shorter, selects nothing (instead of an out-of-range slice).
        let Some(last) = self.rows.len().checked_sub(1) else { return groups };
        if a > last {
            return groups;
        }
        for r in &self.rows[a..=b.min(last)] {
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

    /// Path of the file under the cursor (the new path, or old if deleted).
    pub fn current_file(&self) -> Option<&str> {
        let fi = match self.rows.get(self.cursor)? {
            Row::File(fi) | Row::Hunk(fi, _) | Row::Line(fi, _, _) => *fi,
            Row::Note(n, _) => self.notes[*n].file,
            Row::Meta(_) => return self.files.first().map(|f| f.new_path.as_str()),
        };
        let f = &self.files[fi];
        Some(if f.new_path.is_empty() { &f.old_path } else { &f.new_path })
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

/// Rows for meta lines, then each file, hunk and line, with any review
/// comments inserted right after the line they belong to.
/// Syntax highlighting for every line, reading each hunk's old and new
/// sides in order. Files in an unknown language get none.
fn highlight(files: &[FileDiff]) -> Vec<Vec<Vec<canopy_highlight::Spans>>> {
    use canopy_highlight::Side;
    files
        .iter()
        .map(|f| {
            let path = if f.new_path.is_empty() { &f.old_path } else { &f.new_path };
            if f.binary || !canopy_highlight::supports(path) {
                return Vec::new();
            }
            f.hunks
                .iter()
                .map(|h| {
                    let lines: Vec<(Side, &str)> = h
                        .lines
                        .iter()
                        .map(|l| {
                            let side = match l.kind {
                                DiffLineKind::Added => Side::New,
                                DiffLineKind::Removed => Side::Old,
                                _ => Side::Both,
                            };
                            // The "\ No newline" marker isn't code.
                            (side, if l.kind == DiffLineKind::NoNewline { "" } else { l.content.as_str() })
                        })
                        .collect();
                    canopy_highlight::diff_lines(path, &lines)
                })
                .collect()
        })
        .collect()
}

fn build_rows(meta: usize, files: &[FileDiff], notes: &[Note]) -> Vec<Row> {
    let mut rows: Vec<Row> = (0..meta).map(Row::Meta).collect();
    for (fi, f) in files.iter().enumerate() {
        rows.push(Row::File(fi));
        for (hi, h) in f.hunks.iter().enumerate() {
            rows.push(Row::Hunk(fi, hi));
            for (li, l) in h.lines.iter().enumerate() {
                rows.push(Row::Line(fi, hi, li));
                let (new, old) = match l.kind {
                    DiffLineKind::Added | DiffLineKind::Context => (l.new_no, None),
                    DiffLineKind::Removed => (None, l.old_no),
                    DiffLineKind::NoNewline => (None, None),
                };
                for (ni, n) in notes.iter().enumerate() {
                    let at = if n.right { new } else { old };
                    if n.file == fi && at == Some(n.line) {
                        rows.extend((0..n.text.len()).map(|k| Row::Note(ni, k)));
                    }
                }
            }
        }
    }
    rows
}

fn remote_view(app: &App, r: &canopy_git::Remote) -> DiffView {
    let prefix = format!("{}/", r.name);
    let mut meta = vec![
        format!("Remote {}", r.name),
        String::new(),
        format!("Fetch URL: {}", r.fetch_url),
        format!("Push URL:  {}", r.push_url),
        String::new(),
    ];
    let branches: Vec<_> = app.data.branches.iter().filter(|b| b.is_remote && b.name.starts_with(&prefix)).collect();
    if branches.is_empty() {
        meta.push("No branches fetched from this remote yet (f to fetch).".into());
    } else {
        meta.push(format!("{} branch(es):", branches.len()));
        meta.extend(branches.iter().map(|b| format!("  {}  {}", b.name, b.subject)));
    }
    DiffView::new(format!("remote:{}", r.name), format!("remote {}", r.name), meta, Vec::new(), None)
}

fn submodule_view(s: &canopy_git::parse::submodule::Submodule) -> DiffView {
    use canopy_git::parse::submodule::SubmoduleState::*;
    // Short lines: this panel doesn't wrap.
    let state: &[&str] = match s.state {
        InSync => &["Checked out at the commit this repo records."],
        Uninitialized => &["Not checked out yet.", "Press u to clone and check it out."],
        Modified => &[
            "Checked out at a different commit than this repo records.",
            "Press u to go back to the recorded commit, or commit",
            "the new pointer from Changes to keep it.",
        ],
        Conflict => &["Has a merge conflict over which commit to use."],
    };
    let mut meta = vec![
        format!("Submodule {}", s.path),
        String::new(),
        format!(
            "Commit:  {}{}",
            s.oid.get(..10).unwrap_or(&s.oid),
            s.describe.as_ref().map(|d| format!(" ({d})")).unwrap_or_default()
        ),
        String::new(),
    ];
    meta.extend(state.iter().map(|l| l.to_string()));
    DiffView::new(format!("submodule:{}", s.path), format!("submodule {}", s.path), meta, Vec::new(), None)
}

fn worktree_view(app: &App, w: &canopy_git::parse::worktree::Worktree) -> DiffView {
    let current = app.git.as_ref().is_some_and(|g| g.repo.root == w.path);
    let mut meta = vec![
        format!("Worktree {}", w.path.display()),
        String::new(),
        format!("Branch: {}", w.branch.as_deref().unwrap_or(if w.bare { "(bare)" } else { "(detached HEAD)" })),
    ];
    if let Some(h) = &w.head {
        let subject = app.data.log.iter().find(|c| &c.oid == h).map(|c| c.subject.as_str()).unwrap_or("");
        meta.push(format!("HEAD:   {} {subject}", h.get(..7).unwrap_or(h)));
    }
    meta.push(String::new());
    if current {
        meta.push("This is the worktree canopy has open.".into());
    }
    if let Some(r) = &w.locked {
        meta.push(format!("Locked{}", if r.is_empty() { String::new() } else { format!(": {r}") }));
    }
    if let Some(r) = &w.prunable {
        meta.push(format!("Missing on disk ({r}). Press x to prune it."));
    }
    let key = format!("worktree:{}", w.path.display());
    DiffView::new(key, format!("worktree {}", w.path.display()), meta, Vec::new(), None)
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
        Screen::Branches => match app.refs_view {
            RefsView::Branches => {
                app.selected_branch().map(|b| Req::Show { rev: b.oid.clone(), title: format!("{} (tip)", b.name) })
            }
            RefsView::Tags => app
                .selected_tag()
                .map(|t| Req::Show { rev: format!("refs/tags/{}", t.name), title: format!("tag {}", t.name) }),
            RefsView::Remotes => {
                // Remote details come from data we already have; no git call.
                app.diff = app.selected_remote().map(|r| remote_view(app, r));
                return;
            }
            RefsView::Worktrees => {
                app.diff = app.selected_worktree().map(|w| worktree_view(app, w));
                return;
            }
            RefsView::Submodules => {
                app.diff = app.selected_submodule().map(submodule_view);
                return;
            }
        },
        Screen::Stash => app.data.stashes.get(app.selected(Screen::Stash)).map(|s| Req::Stash { name: s.name.clone() }),
        Screen::Reflog => app
            .data
            .reflog
            .get(app.selected(Screen::Reflog))
            .map(|r| Req::Show { rev: r.oid.clone(), title: format!("{} {}", r.selector, r.subject) }),
        Screen::Pulls => {
            crate::github::load_pr_detail(app, gen);
            return;
        }
        Screen::Issues => {
            if app.github.issues_view == crate::github::IssuesView::Notifications {
                app.diff = crate::github::selected_notification(app).map(crate::github::notification_view);
            } else {
                crate::github::load_issue_detail(app, gen);
            }
            return;
        }
        Screen::Runs => {
            if app.github.runs_view == crate::github::RunsView::Releases {
                crate::github::load_release_detail(app, gen);
            } else {
                crate::github::load_run_detail(app, gen);
            }
            return;
        }
        Screen::Home | Screen::Workspace => None,
    };

    let Some(req) = req else {
        app.diff = None;
        return;
    };

    app.spawn(async move {
        let view = match req {
            Req::Status { path, section, untracked } => {
                // Conflicted text files get the conflict panel instead of a diff.
                if section == Section::Conflicts {
                    let file = git.repo.root.join(&path);
                    let text = tokio::task::spawn_blocking(move || std::fs::read_to_string(file).unwrap_or_default())
                        .await
                        .unwrap_or_default();
                    if let Some(segments) = conflict::parse(&text).filter(|s| conflict::count(s) > 0) {
                        let view = ConflictView { path, segments, current: 0, scroll: 0 };
                        return Msg::Conflict { gen, view };
                    }
                }
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
                let mut v = DiffView::new(format!("show:{rev}"), title, meta, files, None);
                v.rev = Some(rev);
                v
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
        Ok::<_, canopy_git::GitError>(last.unwrap_or_else(|| canopy_git::Output {
            cmd: "git apply --cached".into(),
            stdout: String::new(),
            stderr: String::new(),
        }))
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    const PATCH: &str = "diff --git a/f.txt b/f.txt
--- a/f.txt
+++ b/f.txt
@@ -1,3 +1,3 @@
 one
-two
+TWO
 three
@@ -10,2 +10,3 @@
 ten
+new
 eleven
diff --git a/g.txt b/g.txt
deleted file mode 100644
--- a/g.txt
+++ /dev/null
@@ -1 +0,0 @@
-gone
";

    fn view(mode: Option<PatchMode>) -> DiffView {
        let files = canopy_git::parse::diff::parse(PATCH);
        DiffView::new("k".into(), "t".into(), vec!["meta".into()], files, mode)
    }

    #[test]
    fn rows_and_counts() {
        let v = view(None);
        // meta, then per file: file, hunk, lines.
        assert_eq!(v.rows[0], Row::Meta(0));
        assert_eq!(v.rows[1], Row::File(0));
        assert_eq!(v.rows[2], Row::Hunk(0, 0));
        assert_eq!(v.rows.len(), 1 + (1 + 1 + 4 + 1 + 3) + (1 + 1 + 1));
        assert_eq!(v.added_removed(), (2, 2));
        // Without staging the cursor starts at the top.
        assert_eq!(v.cursor, 0);
        assert_eq!(v.current_file(), Some("f.txt"));
    }

    #[test]
    fn staging_starts_on_the_first_change_and_hunks_jump_to_changes() {
        let mut v = view(Some(PatchMode::Stage));
        assert_eq!(v.rows[v.cursor], Row::Line(0, 0, 1));
        v.jump_hunk(true);
        assert_eq!(v.rows[v.cursor], Row::Line(0, 1, 1));
        v.jump_hunk(true);
        assert_eq!(v.rows[v.cursor], Row::Line(1, 0, 0));
        assert_eq!(v.current_file(), Some("g.txt"));
        // No hunk after the last one: stay put.
        v.jump_hunk(true);
        assert_eq!(v.rows[v.cursor], Row::Line(1, 0, 0));
        v.jump_hunk(false);
        assert_eq!(v.current_hunk(), Some((0, 1)));
    }

    #[test]
    fn cursor_moves_stay_in_range() {
        let mut v = view(None);
        v.move_cursor(-5);
        assert_eq!(v.cursor, 0);
        v.move_cursor(1000);
        assert_eq!(v.cursor, v.rows.len() - 1);
        v.cursor = 999;
        v.snap_cursor();
        assert_eq!(v.cursor, v.rows.len() - 1);
    }

    #[test]
    fn selected_lines_group_by_hunk_bottom_first() {
        let mut v = view(Some(PatchMode::Stage));
        // From the first change down to the added line in the second hunk.
        v.anchor = Some(v.cursor);
        v.cursor = v.rows.iter().position(|r| *r == Row::Line(0, 1, 1)).unwrap();
        // Context lines in the range are left out.
        assert_eq!(v.selected_lines(), vec![(0, 1, vec![1]), (0, 0, vec![1, 2])]);
        // Just the cursor, on a context line: nothing to stage.
        v.anchor = None;
        v.cursor = v.rows.iter().position(|r| *r == Row::Line(0, 0, 0)).unwrap();
        assert!(v.selected_lines().is_empty());
    }

    #[test]
    fn selected_lines_never_slice_out_of_range() {
        let mut empty = DiffView::new("k".into(), "t".into(), vec![], vec![], Some(PatchMode::Stage));
        assert!(empty.selected_lines().is_empty());
        empty.anchor = Some(3);
        assert!(empty.selected_lines().is_empty());
        // The diff got shorter under a cursor and anchor near the old end.
        let mut v = view(Some(PatchMode::Stage));
        v.cursor = 500;
        v.anchor = Some(400);
        assert!(v.selected_lines().is_empty());
        v.anchor = Some(0);
        assert_eq!(v.selected_lines().len(), 3);
    }

    #[test]
    fn line_targets_and_review_notes() {
        let mut v = view(Some(PatchMode::Stage));
        // The removed "two" is line 2 on the old side.
        assert_eq!(v.line_target(), Some((0, 2, false)));
        v.cursor = v.rows.iter().position(|r| *r == Row::Line(0, 0, 2)).unwrap();
        assert_eq!(v.line_target(), Some((0, 2, true)));
        v.cursor = 0;
        assert_eq!(v.line_target(), None);

        let before = v.rows.len();
        v.attach_notes(vec![Note { file: 0, line: 11, right: true, text: vec!["ada · 1h".into(), "why?".into()] }]);
        assert_eq!(v.rows.len(), before + 2);
        let at = v.rows.iter().position(|r| *r == Row::Note(0, 0)).unwrap();
        assert_eq!(v.rows[at - 1], Row::Line(0, 1, 1));
        v.cursor = at;
        assert_eq!(v.current_file(), Some("f.txt"));
    }
}
