//! Everything the Home screen shows, loaded in one go, plus the "next steps"
//! suggestions (the desktop take on the TUI's Home card).

use canopy_git::ops::LogQuery;
use canopy_git::{Branch, Commit, FileKind, Git, Remote, RepoState, Status};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct Overview {
    pub name: String,
    pub root: String,
    pub state: RepoState,
    pub status: Status,
    pub counts: Counts,
    pub log: Vec<Commit>,
    pub branches: Vec<Branch>,
    pub remotes: Vec<Remote>,
    pub stashes: usize,
    pub next_steps: Vec<Step>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Counts {
    pub staged: usize,
    /// Changed tracked files plus untracked ones, not counting conflicts.
    pub unstaged: usize,
    pub untracked: usize,
    pub conflicts: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    Ok,
    Info,
    Warn,
}

/// What the suggestion's button does in the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum StepAction {
    Commit,
    Push,
    Pull,
    Changes,
    Branches,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Step {
    pub level: Level,
    pub text: String,
    pub action: Option<StepAction>,
}

pub async fn load(git: &Git) -> Result<Overview, canopy_git::GitError> {
    let q = LogQuery { limit: 30, ..Default::default() };
    let (status, log, branches, remotes, stashes) =
        tokio::join!(git.status(), git.log(&q), git.branches(), git.remotes(), git.stashes());
    let status = status?;
    // A fresh repo has no commits, and `git log` fails there.
    let log = log.unwrap_or_default();
    let state = git.state();
    let counts = counts(&status);
    let remotes = remotes.unwrap_or_default();
    let next_steps = next_steps(&status, &counts, state, !log.is_empty(), !remotes.is_empty());
    let root = git.repo.root.clone();
    Ok(Overview {
        name: root.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| root.display().to_string()),
        root: root.display().to_string(),
        state,
        status,
        counts,
        log,
        branches: branches.unwrap_or_default(),
        remotes,
        stashes: stashes.map(|s| s.len()).unwrap_or(0),
        next_steps,
    })
}

pub fn counts(s: &Status) -> Counts {
    let conflicts = s.conflicted().count();
    Counts {
        staged: s.staged().count(),
        unstaged: s.unstaged().count() - conflicts,
        untracked: s.files.iter().filter(|f| f.kind == FileKind::Untracked).count(),
        conflicts,
    }
}

fn state_word(state: RepoState) -> &'static str {
    match state {
        RepoState::Merging => "merge",
        RepoState::Rebasing => "rebase",
        RepoState::CherryPicking => "cherry-pick",
        RepoState::Reverting => "revert",
        RepoState::Bisecting => "bisect",
        RepoState::Clean => "",
    }
}

pub fn next_steps(s: &Status, c: &Counts, state: RepoState, has_commits: bool, has_remotes: bool) -> Vec<Step> {
    use StepAction::*;
    let mut out = Vec::new();
    let mut add = |level, text: String, action| out.push(Step { level, text, action });
    let b = &s.branch;

    if state != RepoState::Clean && state != RepoState::Bisecting {
        let w = state_word(state);
        if c.conflicts > 0 {
            add(
                Level::Warn,
                format!(
                    "A {w} is in progress with {} conflict(s). Resolve them in Changes, then continue the {w}.",
                    c.conflicts
                ),
                Some(Changes),
            );
        } else {
            add(
                Level::Warn,
                format!("A {w} is in progress and has no conflicts left. Continue it from Changes."),
                Some(Changes),
            );
        }
    } else if c.conflicts > 0 {
        add(
            Level::Warn,
            format!("{} file(s) have conflicts. Open Changes to resolve them.", c.conflicts),
            Some(Changes),
        );
    }
    if state == RepoState::Bisecting {
        add(
            Level::Warn,
            "A bisect is in progress. Test the checked-out commit, then mark it good or bad.".into(),
            None,
        );
    }
    if b.head.is_none() && b.oid.is_some() {
        add(
            Level::Warn,
            "You're on a detached HEAD (not on a branch). Check out a branch to keep working.".into(),
            Some(Branches),
        );
    }
    if c.staged > 0 {
        add(Level::Info, format!("{} file(s) staged and ready to commit.", c.staged), Some(Commit));
    } else if c.unstaged > 0 {
        add(Level::Info, format!("{} changed file(s). Stage what you want, then commit.", c.unstaged), Some(Changes));
    }
    if b.upstream.is_some() {
        if b.ahead > 0 && b.behind > 0 {
            add(
                Level::Warn,
                format!(
                    "Your branch has diverged ({} ahead, {} behind). Pull to combine them, then push.",
                    b.ahead, b.behind
                ),
                Some(Pull),
            );
        } else if b.ahead > 0 {
            add(Level::Info, format!("{} commit(s) not pushed yet.", b.ahead), Some(Push));
        } else if b.behind > 0 {
            add(Level::Info, format!("{} new commit(s) on the remote.", b.behind), Some(Pull));
        }
    } else if b.head.is_some() && has_commits {
        if has_remotes {
            add(Level::Info, "This branch isn't on the remote yet. Push to publish it.".into(), Some(Push));
        } else {
            add(
                Level::Info,
                "No remote configured yet. Add one (for example on GitHub) to back up and share your work.".into(),
                None,
            );
        }
    }
    if !has_commits {
        add(Level::Info, "Fresh repository. Create some files, then make your first commit.".into(), Some(Changes));
    }
    if out.is_empty() {
        out.push(Step {
            level: Level::Ok,
            text: "All clear: everything is committed and in sync.".into(),
            action: None,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use canopy_git::{BranchInfo, Change, FileStatus};

    fn file(path: &str, kind: FileKind, index: Change, worktree: Change) -> FileStatus {
        FileStatus { path: path.into(), orig_path: None, index, worktree, kind }
    }

    fn status(files: Vec<FileStatus>, ahead: u32, behind: u32, upstream: bool) -> Status {
        Status {
            branch: BranchInfo {
                head: Some("main".into()),
                oid: Some("abc".into()),
                upstream: upstream.then(|| "origin/main".into()),
                ahead,
                behind,
            },
            files,
        }
    }

    fn steps(s: &Status, state: RepoState) -> Vec<(Level, Option<StepAction>)> {
        next_steps(s, &counts(s), state, true, true).into_iter().map(|s| (s.level, s.action)).collect()
    }

    #[test]
    fn all_clear() {
        let s = status(vec![], 0, 0, true);
        assert_eq!(steps(&s, RepoState::Clean), vec![(Level::Ok, None)]);
    }

    #[test]
    fn counts_and_suggestions() {
        let s = status(
            vec![
                file("a", FileKind::Tracked, Change::Modified, Change::Unmodified),
                file("b", FileKind::Tracked, Change::Unmodified, Change::Modified),
                file("c", FileKind::Untracked, Change::Unmodified, Change::Unmodified),
            ],
            2,
            0,
            true,
        );
        assert_eq!(counts(&s), Counts { staged: 1, unstaged: 2, untracked: 1, conflicts: 0 });
        assert_eq!(
            steps(&s, RepoState::Clean),
            vec![(Level::Info, Some(StepAction::Commit)), (Level::Info, Some(StepAction::Push))]
        );
    }

    #[test]
    fn conflicts_and_divergence_warn() {
        let s = status(vec![file("x", FileKind::Conflicted, Change::Unmerged, Change::Unmerged)], 1, 3, true);
        let got = steps(&s, RepoState::Merging);
        assert_eq!(got[0], (Level::Warn, Some(StepAction::Changes)));
        assert_eq!(got.last(), Some(&(Level::Warn, Some(StepAction::Pull))));
    }

    #[test]
    fn unpublished_branch_and_fresh_repo() {
        let s = status(vec![], 0, 0, false);
        let got = next_steps(&s, &counts(&s), RepoState::Clean, true, true);
        assert_eq!(got[0].action, Some(StepAction::Push));
        let got = next_steps(&s, &counts(&s), RepoState::Clean, false, false);
        assert!(got[0].text.starts_with("Fresh repository"));
    }

    #[tokio::test]
    async fn loads_a_real_repo() {
        let dir = tempfile::TempDir::new().unwrap();
        let run = |args: &[&str]| {
            let ok = std::process::Command::new("git").current_dir(dir.path()).args(args).status().unwrap().success();
            assert!(ok, "git {args:?}");
        };
        run(&["init", "-q", "-b", "main"]);
        run(&["config", "user.name", "T"]);
        run(&["config", "user.email", "t@t.io"]);
        let git = Git::open(dir.path()).await.unwrap();
        // Empty repo: no commits yet, but it still loads.
        let o = load(&git).await.unwrap();
        assert!(o.log.is_empty() && o.next_steps[0].text.starts_with("Fresh"));

        std::fs::write(dir.path().join("a.txt"), "hi\n").unwrap();
        run(&["add", "."]);
        run(&["commit", "-qm", "first"]);
        std::fs::write(dir.path().join("b.txt"), "new\n").unwrap();
        let o = load(&git).await.unwrap();
        assert_eq!(o.log[0].subject, "first");
        assert_eq!(o.counts.untracked, 1);
        assert_eq!(o.branches.len(), 1);
        let json = serde_json::to_value(&o).unwrap();
        assert_eq!(json["next_steps"][0]["action"], "changes");
        assert_eq!(json["state"], "Clean");
    }

    /// Writes this repo's overview as JSON for the UI preview harness:
    /// `CANOPY_OVERVIEW_OUT=/tmp/o.json cargo test -p canopy-desktop dump_overview -- --ignored`
    #[tokio::test]
    #[ignore]
    async fn dump_overview() {
        let out = std::env::var("CANOPY_OVERVIEW_OUT").expect("set CANOPY_OVERVIEW_OUT");
        let git = Git::open(env!("CARGO_MANIFEST_DIR")).await.unwrap();
        let o = load(&git).await.unwrap();
        std::fs::write(out, serde_json::to_string_pretty(&o).unwrap()).unwrap();
    }
}
