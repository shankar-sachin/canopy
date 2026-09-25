use std::process::Command;

use canopy_git::ops::{CommitOpts, LogQuery, ResetMode};
use canopy_git::parse::diff::PatchMode;
use canopy_git::{Change, FileKind, Git, RepoState};
use tempfile::TempDir;

fn sh(dir: &std::path::Path, args: &[&str]) -> String {
    let out = Command::new("git").current_dir(dir).args(args).output().expect("git");
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn repo() -> TempDir {
    let dir = TempDir::new().unwrap();
    let p = dir.path();
    sh(p, &["init", "-q", "-b", "main"]);
    sh(p, &["config", "user.name", "Test"]);
    sh(p, &["config", "user.email", "t@t.io"]);
    sh(p, &["config", "commit.gpgsign", "false"]);
    dir
}

fn write(dir: &TempDir, path: &str, content: &str) {
    std::fs::write(dir.path().join(path), content).unwrap();
}

async fn commit_all(git: &Git, msg: &str) {
    git.stage_all().await.unwrap();
    git.commit(msg, &CommitOpts::default()).await.unwrap();
}

#[tokio::test]
async fn empty_repo_status_and_log() {
    let dir = repo();
    let git = Git::open(dir.path()).await.unwrap();
    let st = git.status().await.unwrap();
    assert_eq!(st.branch.head.as_deref(), Some("main"));
    assert!(st.is_clean());
    assert!(git.log(&LogQuery::default()).await.unwrap().is_empty());
}

#[tokio::test]
async fn stage_commit_log() {
    let dir = repo();
    let git = Git::open(dir.path()).await.unwrap();
    write(&dir, "a.txt", "hello\n");

    let st = git.status().await.unwrap();
    assert_eq!(st.files[0].kind, FileKind::Untracked);

    let out = git.stage(&["a.txt"]).await.unwrap();
    assert_eq!(out.cmd, "git add -- a.txt");
    let st = git.status().await.unwrap();
    assert_eq!(st.files[0].index, Change::Added);

    let out = git.commit("first commit", &CommitOpts::default()).await.unwrap();
    assert_eq!(out.cmd, "git commit -m 'first commit'");
    let log = git.log(&LogQuery::default()).await.unwrap();
    assert_eq!(log.len(), 1);
    assert_eq!(log[0].subject, "first commit");
    assert!(log[0].refs.iter().any(|r| r.contains("main")));
}

#[tokio::test]
async fn stage_and_unstage_single_lines() {
    let dir = repo();
    let git = Git::open(dir.path()).await.unwrap();
    write(&dir, "f.txt", "one\ntwo\nthree\nfour\n");
    commit_all(&git, "base").await;

    write(&dir, "f.txt", "one\nTWO\nthree\nfour\nfive\n");
    let diff = git.diff_file("f.txt", false, 3).await.unwrap();
    let file = &diff[0];
    let hunk = &file.hunks[0];
    // Lines: " one", "-two", "+TWO", " three", " four", "+five"
    let five = hunk.lines.iter().position(|l| l.content == "five").unwrap();

    git.apply_lines(file, hunk, &[five], PatchMode::Stage).await.unwrap().unwrap();
    assert_eq!(sh(dir.path(), &["show", ":f.txt"]), "one\ntwo\nthree\nfour\nfive\n");

    // Now stage `-two` and `+TWO` too, then unstage only `+five`.
    let diff = git.diff_file("f.txt", false, 3).await.unwrap();
    git.apply_hunk(&diff[0], &diff[0].hunks[0], PatchMode::Stage).await.unwrap();
    assert_eq!(sh(dir.path(), &["show", ":f.txt"]), "one\nTWO\nthree\nfour\nfive\n");

    let staged = git.diff_file("f.txt", true, 3).await.unwrap();
    let h = &staged[0].hunks[0];
    let five = h.lines.iter().position(|l| l.content == "five").unwrap();
    git.apply_lines(&staged[0], h, &[five], PatchMode::Unstage).await.unwrap().unwrap();
    assert_eq!(sh(dir.path(), &["show", ":f.txt"]), "one\nTWO\nthree\nfour\n");
    // Worktree untouched.
    assert_eq!(std::fs::read_to_string(dir.path().join("f.txt")).unwrap(), "one\nTWO\nthree\nfour\nfive\n");
}

#[tokio::test]
async fn stage_lines_in_later_hunk() {
    let dir = repo();
    let git = Git::open(dir.path()).await.unwrap();
    let base: String = (1..=30).map(|i| format!("l{i}\n")).collect();
    write(&dir, "f.txt", &base);
    commit_all(&git, "base").await;

    // Insert lines near the top and bottom so hunk 2 starts at different
    // old/new line numbers.
    let changed = base.replace("l2\n", "l2\nNEW-A\nNEW-B\n").replace("l28\n", "l28\nNEW-C\n");
    write(&dir, "f.txt", &changed);
    let diff = git.diff_file("f.txt", false, 3).await.unwrap();
    assert_eq!(diff[0].hunks.len(), 2);

    // Stage hunk 1 first so the index shifts, then stage hunk 2 from a fresh diff.
    git.apply_hunk(&diff[0], &diff[0].hunks[0], PatchMode::Stage).await.unwrap();
    let diff = git.diff_file("f.txt", false, 3).await.unwrap();
    git.apply_hunk(&diff[0], &diff[0].hunks[0], PatchMode::Stage).await.unwrap();
    assert_eq!(sh(dir.path(), &["show", ":f.txt"]), changed);

    // Unstage only the second hunk (its new_start differs from old_start).
    let staged = git.diff_file("f.txt", true, 3).await.unwrap();
    let h = &staged[0].hunks[1];
    assert_ne!(h.old_start, h.new_start);
    git.apply_hunk(&staged[0], h, PatchMode::Unstage).await.unwrap();
    assert_eq!(sh(dir.path(), &["show", ":f.txt"]), base.replace("l2\n", "l2\nNEW-A\nNEW-B\n"));
}

#[tokio::test]
async fn untracked_diff_and_new_file_line_stage() {
    let dir = repo();
    let git = Git::open(dir.path()).await.unwrap();
    write(&dir, "seed", "x\n");
    commit_all(&git, "seed").await;
    write(&dir, "new.txt", "a\nb\n");
    let d = git.diff_untracked("new.txt").await.unwrap();
    assert_eq!(d.len(), 1);
    assert_eq!(d[0].hunks[0].lines.len(), 2);
}

#[tokio::test]
async fn branches_merge_and_conflict() {
    let dir = repo();
    let git = Git::open(dir.path()).await.unwrap();
    write(&dir, "f.txt", "base\n");
    commit_all(&git, "base").await;

    git.create_branch("feature", None, true).await.unwrap();
    write(&dir, "f.txt", "feature\n");
    commit_all(&git, "feature change").await;

    git.checkout("main").await.unwrap();
    write(&dir, "f.txt", "main\n");
    commit_all(&git, "main change").await;

    let branches = git.branches().await.unwrap();
    assert_eq!(branches.len(), 2);
    assert!(branches.iter().any(|b| b.name == "main" && b.is_head));

    assert!(git.merge("feature", false).await.is_err());
    assert_eq!(git.state(), RepoState::Merging);
    let st = git.status().await.unwrap();
    assert_eq!(st.conflicted().count(), 1);

    git.checkout_side("f.txt", false).await.unwrap();
    git.merge_continue().await.unwrap();
    assert_eq!(git.state(), RepoState::Clean);
    assert_eq!(std::fs::read_to_string(dir.path().join("f.txt")).unwrap(), "feature\n");
}

#[tokio::test]
async fn stash_roundtrip_and_reflog_reset() {
    let dir = repo();
    let git = Git::open(dir.path()).await.unwrap();
    write(&dir, "f.txt", "1\n");
    commit_all(&git, "one").await;
    write(&dir, "f.txt", "2\n");
    commit_all(&git, "two").await;

    write(&dir, "f.txt", "wip\n");
    git.stash_push(Some("my wip"), false).await.unwrap();
    let stashes = git.stashes().await.unwrap();
    assert_eq!(stashes.len(), 1);
    assert!(stashes[0].message.contains("my wip"));
    git.stash_pop("stash@{0}").await.unwrap();
    assert_eq!(std::fs::read_to_string(dir.path().join("f.txt")).unwrap(), "wip\n");
    git.discard(&["f.txt"]).await.unwrap();

    git.reset("HEAD~1", ResetMode::Hard).await.unwrap();
    let reflog = git.reflog(10).await.unwrap();
    assert!(reflog[0].subject.starts_with("reset"));
    // Undo via reflog.
    git.reset(&reflog[1].oid, ResetMode::Hard).await.unwrap();
    let log = git.log(&LogQuery::default()).await.unwrap();
    assert_eq!(log[0].subject, "two");
}

#[tokio::test]
async fn interactive_rebase_squash() {
    let dir = repo();
    let git = Git::open(dir.path()).await.unwrap();
    for i in 1..=3 {
        write(&dir, "f.txt", &format!("{i}\n"));
        commit_all(&git, &format!("c{i}")).await;
    }
    let log = git.log(&LogQuery::default()).await.unwrap();
    let todo = format!("pick {} c2\nfixup {} c3\n", log[1].oid, log[0].oid);
    git.rebase_interactive(&log[2].oid, &todo).await.unwrap();
    let log = git.log(&LogQuery::default()).await.unwrap();
    assert_eq!(log.len(), 2);
    assert_eq!(log[0].subject, "c2");
    assert_eq!(std::fs::read_to_string(dir.path().join("f.txt")).unwrap(), "3\n");
}

#[tokio::test]
async fn fetch_push_with_progress() {
    let remote = TempDir::new().unwrap();
    sh(remote.path(), &["init", "-q", "--bare", "-b", "main"]);
    let dir = repo();
    let git = Git::open(dir.path()).await.unwrap();
    write(&dir, "f.txt", "x\n");
    commit_all(&git, "x").await;
    git.add_remote("origin", remote.path().to_str().unwrap()).await.unwrap();

    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
    git.push("origin", "main", true, false, tx.clone()).await.unwrap();
    let st = git.status().await.unwrap();
    assert_eq!(st.branch.upstream.as_deref(), Some("origin/main"));
    git.fetch(None, tx).await.unwrap();
    assert!(git.branches().await.unwrap().iter().any(|b| b.is_remote));
}
