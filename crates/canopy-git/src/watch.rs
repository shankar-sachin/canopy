//! Notice changes made outside Canopy (an editor saving a file, git in
//! another terminal) the moment they happen, instead of on the next poll.
//!
//! Events are gathered for a moment (a save or a commit touches several
//! files), then filtered: files git ignores (build output, node_modules)
//! and git's own bookkeeping (objects, logs, lock files) don't count.

use std::collections::HashSet;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::{Duration, Instant};

use notify::{EventKind, RecursiveMode, Watcher};

use crate::model::Repo;

/// How long to wait for more events before reporting a change.
const SETTLE: Duration = Duration::from_millis(250);

/// What changed since the last report.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Change {
    /// A file in the working tree that git doesn't ignore.
    pub files: bool,
    /// git's state: HEAD, a branch or tag, the index, a merge or rebase.
    pub git: bool,
}

/// Keeps watching until dropped.
pub struct RepoWatcher {
    _watcher: notify::RecommendedWatcher,
}

impl std::fmt::Debug for RepoWatcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("RepoWatcher")
    }
}

/// Watch `repo`, calling `on_change` (on a background thread) after each
/// burst of changes that matter. Fails when the system can't watch the
/// folder (then keep polling instead).
pub fn watch(repo: &Repo, on_change: impl Fn(Change) + Send + 'static) -> notify::Result<RepoWatcher> {
    let (tx, rx) = mpsc::channel::<PathBuf>();
    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        let Ok(event) = res else { return };
        // Reads and opens change nothing.
        if matches!(event.kind, EventKind::Access(_)) {
            return;
        }
        for p in event.paths {
            let _ = tx.send(p);
        }
    })?;
    watcher.watch(&repo.root, RecursiveMode::Recursive)?;
    // A linked worktree keeps its git dir outside the working tree.
    if !repo.git_dir.starts_with(&repo.root) {
        watcher.watch(&repo.git_dir, RecursiveMode::Recursive)?;
    }

    let (root, git_dir) = (repo.root.clone(), repo.git_dir.clone());
    std::thread::spawn(move || {
        // Ends when the watcher (and with it `tx`) is dropped.
        while let Ok(first) = rx.recv() {
            let mut paths = HashSet::from([first]);
            let until = Instant::now() + SETTLE;
            loop {
                match rx.recv_timeout(until.saturating_duration_since(Instant::now())) {
                    Ok(p) => {
                        paths.insert(p);
                    }
                    Err(RecvTimeoutError::Timeout) => break,
                    Err(RecvTimeoutError::Disconnected) => return,
                }
            }
            let change = classify(&root, &git_dir, paths.into_iter().collect());
            if change.files || change.git {
                on_change(change);
            }
        }
    });
    Ok(RepoWatcher { _watcher: watcher })
}

/// Sort a burst of changed paths into working-tree files and git state.
fn classify(root: &Path, git_dir: &Path, paths: Vec<PathBuf>) -> Change {
    let mut change = Change::default();
    let mut files = Vec::new();
    for p in paths {
        if let Ok(rel) = p.strip_prefix(git_dir) {
            change.git |= git_state_file(rel);
        } else if let Ok(rel) = p.strip_prefix(root) {
            // `.git` itself (a folder, or a file in a linked worktree).
            if rel.as_os_str().is_empty() || rel.starts_with(".git") {
                continue;
            }
            files.push(rel.to_path_buf());
        }
    }
    if !files.is_empty() {
        change.files = files.len() > ignored(root, &files);
    }
    change
}

/// Whether a path inside the git dir is state Canopy shows (as opposed to
/// objects, logs, hooks and lock files).
pub fn git_state_file(rel: &Path) -> bool {
    let s = rel.to_string_lossy().replace('\\', "/");
    if s.ends_with(".lock") {
        return false;
    }
    const FILES: [&str; 7] =
        ["HEAD", "index", "packed-refs", "MERGE_HEAD", "CHERRY_PICK_HEAD", "REVERT_HEAD", "BISECT_LOG"];
    FILES.contains(&s.as_str())
        || s.starts_with("refs/")
        || s.starts_with("rebase-merge")
        || s.starts_with("rebase-apply")
        // A linked worktree's own HEAD and index live under worktrees/<name>/.
        || s.strip_prefix("worktrees/").and_then(|w| w.split_once('/')).is_some_and(|(_, f)| FILES.contains(&f))
}

/// How many of `files` (relative to `root`) git ignores. If git can't be
/// asked, none are.
fn ignored(root: &Path, files: &[PathBuf]) -> usize {
    let child = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["check-ignore", "--stdin", "-z"])
        .env("GIT_OPTIONAL_LOCKS", "0")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn();
    let Ok(mut child) = child else { return 0 };
    if let Some(mut stdin) = child.stdin.take() {
        for f in files {
            let _ = stdin.write_all(f.to_string_lossy().as_bytes());
            let _ = stdin.write_all(b"\0");
        }
    }
    // Exit 1 means "none ignored": the output is empty then.
    child.wait_with_output().map(|o| o.stdout.split(|b| *b == 0).filter(|s| !s.is_empty()).count()).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn git_state_files() {
        for s in ["HEAD", "index", "refs/heads/main", "refs/stash", "packed-refs", "MERGE_HEAD", "rebase-merge/done"] {
            assert!(git_state_file(Path::new(s)), "{s}");
        }
        for s in
            ["objects/ab/cdef", "logs/HEAD", "index.lock", "refs/heads/main.lock", "COMMIT_EDITMSG", "hooks/pre-commit"]
        {
            assert!(!git_state_file(Path::new(s)), "{s}");
        }
        assert!(git_state_file(Path::new("worktrees/wt/HEAD")));
        assert!(!git_state_file(Path::new("worktrees/wt/logs/HEAD")));
    }
}
