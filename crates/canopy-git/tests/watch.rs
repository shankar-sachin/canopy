//! The repository watcher against a real folder and real file events.

use std::process::Command;
use std::sync::mpsc;
use std::time::Duration;

use canopy_git::watch::{watch, Change};
use canopy_git::Git;
use tempfile::TempDir;

fn sh(dir: &std::path::Path, args: &[&str]) {
    let ok = Command::new("git").current_dir(dir).args(args).status().expect("git").success();
    assert!(ok, "git {args:?}");
}

/// The next change, or None if nothing arrives in `secs`.
fn next(rx: &mpsc::Receiver<Change>, secs: u64) -> Option<Change> {
    rx.recv_timeout(Duration::from_secs(secs)).ok()
}

/// Let the events from setting things up arrive, and forget them.
fn drain(rx: &mpsc::Receiver<Change>) {
    while rx.recv_timeout(Duration::from_millis(1500)).is_ok() {}
}

#[tokio::test]
async fn edits_and_commits_are_noticed_ignored_files_are_not() {
    let dir = TempDir::new().unwrap();
    let p = dir.path();
    sh(p, &["init", "-q", "-b", "main"]);
    sh(p, &["config", "user.name", "T"]);
    sh(p, &["config", "user.email", "t@t.io"]);
    sh(p, &["config", "commit.gpgsign", "false"]);
    std::fs::write(p.join(".gitignore"), "/build\n").unwrap();
    std::fs::write(p.join("f.txt"), "1\n").unwrap();
    std::fs::create_dir(p.join("build")).unwrap();
    sh(p, &["add", "."]);
    sh(p, &["commit", "-qm", "one"]);

    let git = Git::open(p).await.unwrap();
    let (tx, rx) = mpsc::channel();
    let _w = watch(&git.repo, move |c| {
        let _ = tx.send(c);
    })
    .expect("watch the folder");
    drain(&rx);

    // Build output that git ignores: nothing to refresh.
    std::fs::write(p.join("build/out.o"), "x").unwrap();
    assert_eq!(next(&rx, 2), None, "an ignored file counted as a change");

    // An edit to a tracked file.
    std::fs::write(p.join("f.txt"), "2\n").unwrap();
    let c = next(&rx, 10).expect("the edit was noticed");
    assert!(c.files, "{c:?}");
    drain(&rx);

    // A commit from another terminal moves HEAD's branch.
    sh(p, &["commit", "-qam", "two"]);
    let mut seen = Change::default();
    while let Some(c) = next(&rx, 5) {
        seen.git |= c.git;
        if seen.git {
            break;
        }
    }
    assert!(seen.git, "the commit wasn't noticed");
}
