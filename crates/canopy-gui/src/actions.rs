//! Commands that change the repository: staging, commits, conflicts, and
//! fetch / pull / push. Each returns the git command it ran so the page can
//! show it (the desktop take on the TUI's teach mode).

use canopy_git::ops::CommitOpts;
use canopy_git::parse::diff::PatchMode;
use canopy_git::{FileDiff, Git, GitError, Hunk, Output, RepoState};
use serde::Serialize;
use tauri::{Emitter, State};

use crate::{current, AppState, Res};

#[derive(Debug, Clone, Serialize)]
pub struct Done {
    /// The git command(s) that ran, as a user would type them.
    pub cmd: String,
    /// What git printed, for the few commands where it's worth showing.
    pub output: String,
}

fn done(o: Output) -> Done {
    let output = if o.stdout.trim().is_empty() { o.stderr } else { o.stdout };
    Done { cmd: o.cmd, output: output.trim().to_string() }
}

fn err(e: GitError) -> String {
    match e {
        GitError::Failed { cmd, stderr, .. } => format!("{cmd}\n{}", stderr.trim()),
        e => e.to_string(),
    }
}

fn refs(v: &[String]) -> Vec<&str> {
    v.iter().map(String::as_str).collect()
}

#[tauri::command]
pub async fn file_diff(path: String, staged: bool, untracked: bool, state: State<'_, AppState>) -> Res<Vec<FileDiff>> {
    let git = current(&state).await?;
    let res = if untracked { git.diff_untracked(&path).await } else { git.diff_file(&path, staged, 3).await };
    res.map_err(err)
}

#[tauri::command]
pub async fn stage(paths: Vec<String>, state: State<'_, AppState>) -> Res<Done> {
    let git = current(&state).await?;
    git.stage(&refs(&paths)).await.map(done).map_err(err)
}

#[tauri::command]
pub async fn unstage(paths: Vec<String>, state: State<'_, AppState>) -> Res<Done> {
    let git = current(&state).await?;
    git.unstage(&refs(&paths)).await.map(done).map_err(err)
}

#[tauri::command]
pub async fn stage_all(state: State<'_, AppState>) -> Res<Done> {
    current(&state).await?.stage_all().await.map(done).map_err(err)
}

#[tauri::command]
pub async fn unstage_all(state: State<'_, AppState>) -> Res<Done> {
    current(&state).await?.unstage_all().await.map(done).map_err(err)
}

/// Throw away unstaged edits to `tracked` files and delete `untracked` ones.
#[tauri::command]
pub async fn discard(tracked: Vec<String>, untracked: Vec<String>, state: State<'_, AppState>) -> Res<Done> {
    let git = current(&state).await?;
    let mut cmds = Vec::new();
    if !tracked.is_empty() {
        cmds.push(git.discard(&refs(&tracked)).await.map_err(err)?.cmd);
    }
    if !untracked.is_empty() {
        cmds.push(git.clean(&refs(&untracked)).await.map_err(err)?.cmd);
    }
    Ok(Done { cmd: cmds.join("\n"), output: String::new() })
}

/// Stage (or unstage) some lines of one hunk. `lines` index into `hunk.lines`.
#[tauri::command]
pub async fn apply_lines(
    file: FileDiff,
    hunk: Hunk,
    lines: Vec<usize>,
    unstage: bool,
    state: State<'_, AppState>,
) -> Res<Done> {
    let git = current(&state).await?;
    let mode = if unstage { PatchMode::Unstage } else { PatchMode::Stage };
    match git.apply_lines(&file, &hunk, &lines, mode).await.map_err(err)? {
        Some(o) => Ok(done(o)),
        None => Err("Those lines have no changes to stage.".into()),
    }
}

#[tauri::command]
pub async fn commit(message: String, amend: bool, state: State<'_, AppState>) -> Res<Done> {
    let git = current(&state).await?;
    if message.trim().is_empty() {
        return Err("Write a commit message first.".into());
    }
    let opts = CommitOpts { amend, ..Default::default() };
    git.commit(message.trim_end(), &opts).await.map(done).map_err(err)
}

/// The last commit's message, to start an amend from.
#[tauri::command]
pub async fn last_message(state: State<'_, AppState>) -> Res<String> {
    let git = current(&state).await?;
    let o = git.run(&["log", "-1", "--format=%B"]).await.map_err(err)?;
    Ok(o.stdout.trim_end().to_string())
}

/// Resolve a conflicted file by taking one side entirely.
#[tauri::command]
pub async fn take_side(path: String, ours: bool, state: State<'_, AppState>) -> Res<Done> {
    let git = current(&state).await?;
    git.checkout_side(&path, ours).await.map(done).map_err(err)
}

async fn continue_or_abort(git: &Git, abort: bool) -> Result<Output, String> {
    let verb = if abort { "--abort" } else { "--continue" };
    let res = match git.state() {
        RepoState::Merging if abort => git.merge_abort().await,
        RepoState::Merging => git.merge_continue().await,
        RepoState::Rebasing if abort => git.rebase_abort().await,
        RepoState::Rebasing => git.rebase_continue().await,
        RepoState::CherryPicking => git.run(&["-c", "core.editor=true", "cherry-pick", verb]).await,
        RepoState::Reverting => git.run(&["-c", "core.editor=true", "revert", verb]).await,
        RepoState::Bisecting | RepoState::Clean => return Err("Nothing to continue or abort.".into()),
    };
    res.map_err(err)
}

/// Continue the merge / rebase / cherry-pick / revert in progress.
#[tauri::command]
pub async fn op_continue(state: State<'_, AppState>) -> Res<Done> {
    let git = current(&state).await?;
    continue_or_abort(&git, false).await.map(done)
}

#[tauri::command]
pub async fn op_abort(state: State<'_, AppState>) -> Res<Done> {
    let git = current(&state).await?;
    continue_or_abort(&git, true).await.map(done)
}

/// Where a push goes: the upstream's remote, else `origin`, else the first
/// remote (then it also sets the upstream).
pub fn push_target(upstream: Option<&str>, remotes: &[String]) -> Option<(String, bool)> {
    if let Some((remote, _)) = upstream.and_then(|u| u.split_once('/')) {
        return Some((remote.to_string(), false));
    }
    let r = remotes.iter().find(|r| *r == "origin").or(remotes.first())?;
    Some((r.clone(), true))
}

/// `kind` is fetch, pull, pull-rebase, pull-merge, push or force-push.
/// Progress lines arrive as `progress` events while it runs.
#[tauri::command]
pub async fn sync(kind: String, app: tauri::AppHandle, state: State<'_, AppState>) -> Res<Done> {
    let git = current(&state).await?;
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    let forward = tokio::spawn(async move {
        while let Some(line) = rx.recv().await {
            let _ = app.emit("progress", line);
        }
    });
    let res = match kind.as_str() {
        "fetch" => git.fetch(None, tx).await,
        "pull" => git.run_streaming(&["pull", "--progress"], tx).await,
        "pull-rebase" => git.pull(true, tx).await,
        "pull-merge" => git.pull(false, tx).await,
        "push" | "force-push" => {
            let status = git.status().await.map_err(err)?;
            let branch =
                status.branch.head.ok_or("You're not on a branch (detached HEAD), so there's nothing to push.")?;
            let remotes: Vec<String> = git.remotes().await.map_err(err)?.into_iter().map(|r| r.name).collect();
            let (remote, set_upstream) = push_target(status.branch.upstream.as_deref(), &remotes)
                .ok_or("No remote is configured. Add one first (for example: git remote add origin <url>).")?;
            git.push(&remote, &branch, set_upstream, kind == "force-push", tx).await
        }
        other => return Err(format!("unknown sync kind {other}")),
    };
    let _ = forward.await;
    res.map(done).map_err(err)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_targets() {
        let r = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(push_target(Some("up/main"), &r(&["origin", "up"])), Some(("up".into(), false)));
        assert_eq!(push_target(None, &r(&["fork", "origin"])), Some(("origin".into(), true)));
        assert_eq!(push_target(None, &r(&["fork"])), Some(("fork".into(), true)));
        assert_eq!(push_target(None, &[]), None);
    }

    /// The page sends back the FileDiff/Hunk it was given as JSON; staging
    /// some of its lines must still work after that round trip.
    #[tokio::test]
    async fn stage_lines_after_json_round_trip() {
        let dir = tempfile::TempDir::new().unwrap();
        let run = |args: &[&str]| {
            let ok = std::process::Command::new("git").current_dir(dir.path()).args(args).status().unwrap().success();
            assert!(ok, "git {args:?}");
        };
        run(&["init", "-q", "-b", "main"]);
        run(&["config", "user.name", "T"]);
        run(&["config", "user.email", "t@t.io"]);
        run(&["config", "core.autocrlf", "false"]);
        std::fs::write(dir.path().join("f.txt"), "a\nb\nc\n").unwrap();
        run(&["add", "."]);
        run(&["commit", "-qm", "base"]);
        std::fs::write(dir.path().join("f.txt"), "a\nB1\nB2\nc\n").unwrap();

        let git = Git::open(dir.path()).await.unwrap();
        let diff = git.diff_file("f.txt", false, 3).await.unwrap();
        let json = serde_json::to_string(&diff[0]).unwrap();
        let file: FileDiff = serde_json::from_str(&json).unwrap();
        let hunk = file.hunks[0].clone();
        // Pick only the first added line ("B1").
        let li = hunk.lines.iter().position(|l| l.content == "B1").unwrap();
        git.apply_lines(&file, &hunk, &[li], PatchMode::Stage).await.unwrap().unwrap();
        let out = std::process::Command::new("git").current_dir(dir.path()).args(["show", ":f.txt"]).output().unwrap();
        assert_eq!(String::from_utf8_lossy(&out.stdout), "a\nb\nB1\nc\n");
    }
}
