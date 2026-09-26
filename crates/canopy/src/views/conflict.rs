//! Resolving merge conflicts one at a time from the Changes tab.

use canopy_git::parse::conflict::{self, Choice, Segment};

use crate::app::{App, Level, Then};
use crate::keymap::Focus;
use crate::views::diff;

#[derive(Debug, Clone)]
pub struct ConflictView {
    pub path: String,
    pub segments: Vec<Segment>,
    /// Index among conflicts (not segments).
    pub current: usize,
    pub scroll: usize,
}

impl ConflictView {
    pub fn count(&self) -> usize {
        conflict::count(&self.segments)
    }

    pub fn step(&mut self, delta: isize) {
        let n = self.count();
        if n > 0 {
            self.current = (self.current as isize + delta).clamp(0, n as isize - 1) as usize;
        }
    }
}

pub fn choose(app: &mut App, choice: Choice) {
    let (Some(git), Some(view)) = (app.git.clone(), app.conflict.as_ref()) else { return };
    let path = view.path.clone();
    let (idx, total) = (view.current, view.count());
    if total == 0 {
        return;
    }
    let text = conflict::resolve_one(&view.segments, idx, choice);
    if let Err(e) = std::fs::write(git.repo.root.join(&path), &text) {
        app.toast(Level::Error, format!("Couldn't write {path}: {e}"));
        return;
    }
    let side = match choice {
        Choice::Ours => "ours",
        Choice::Theirs => "theirs",
        Choice::Both => "both",
    };
    app.history.push(format!("# edited {path}: kept {side} for conflict {} of {total}", idx + 1));

    let left = conflict::parse(&text).map(|s| conflict::count(&s)).unwrap_or(0);
    if left == 0 {
        // Nothing left: mark it resolved.
        app.focus = Focus::List;
        app.run_op(format!("Resolved {path}"), Then::Refresh, async move { git.stage(&[&path]).await });
    } else {
        app.toast(Level::Success, format!("Kept {side} · {left} conflict(s) left in {path}"));
        diff::load_for_selection(app);
    }
}

pub fn restore(app: &mut App) {
    let Some(git) = app.git.clone() else { return };
    let Some(row) = app.selected_status_row() else { return };
    let path = app.data.status.files[row.file].path.clone();
    app.run_op(format!("Restore conflict markers in {path}"), Then::Refresh, async move {
        git.restore_conflict(&path).await
    });
}
