//! History, branches, tags, stashes and undo.

use canopy_git::ops::{LogQuery, ResetMode};
use canopy_git::{Commit, ReflogEntry, Stash, Tag};
use serde::Serialize;
use tauri::State;

use crate::actions::{done, err, Done};
use crate::graph::{self, Row};
use crate::{current, AppState, Res};

#[derive(Debug, Clone, Serialize)]
pub struct History {
    pub commits: Vec<Commit>,
    /// One per commit; empty when searching (a filtered graph means nothing).
    pub graph: Vec<Row>,
    /// True when there may be more commits after these.
    pub more: bool,
}

/// `limit` commits of the current branch (or every branch with `all`),
/// optionally filtered by message text or author.
#[tauri::command]
pub async fn history(limit: usize, all: bool, search: Option<String>, state: State<'_, AppState>) -> Res<History> {
    let git = current(&state).await?;
    let search = search.map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
    let mut q = LogQuery { limit: limit + 1, all, ..Default::default() };
    // "@name" searches authors; anything else searches commit messages.
    match search.as_deref() {
        Some(s) if s.starts_with('@') => q.author = Some(s[1..].to_string()),
        Some(s) => q.grep = Some(s.to_string()),
        None => {}
    }
    // A fresh repository has no commits; git log fails there.
    let mut commits = git.log(&q).await.unwrap_or_default();
    let more = commits.len() > limit;
    commits.truncate(limit);
    let graph = if search.is_some() { Vec::new() } else { graph::build(&commits) };
    Ok(History { commits, graph, more })
}

#[derive(Debug, Clone, Serialize)]
pub struct CommitDetails {
    /// `git show --format=fuller` header: author, dates, full message.
    pub header: String,
    pub files: Vec<crate::highlight::Shown>,
}

#[tauri::command]
pub async fn commit_details(rev: String, state: State<'_, AppState>) -> Res<CommitDetails> {
    let git = current(&state).await?;
    let (header, files) = git.show(&rev).await.map_err(err)?;
    Ok(CommitDetails { header, files: crate::highlight::show(files) })
}

#[derive(Debug, Clone, Serialize)]
pub struct Refs {
    pub tags: Vec<Tag>,
    pub stashes: Vec<Stash>,
}

#[tauri::command]
pub async fn refs(state: State<'_, AppState>) -> Res<Refs> {
    let git = current(&state).await?;
    let (tags, stashes) = tokio::join!(git.tags(), git.stashes());
    Ok(Refs { tags: tags.unwrap_or_default(), stashes: stashes.unwrap_or_default() })
}

#[tauri::command]
pub async fn stash_diff(name: String, state: State<'_, AppState>) -> Res<Vec<crate::highlight::Shown>> {
    let git = current(&state).await?;
    let o = git.run(&["stash", "show", "-p", "--include-untracked", "--no-ext-diff", &name]).await;
    // Older gits don't know --include-untracked for `stash show`.
    let o = match o {
        Ok(o) => o,
        Err(_) => git.run(&["stash", "show", "-p", "--no-ext-diff", &name]).await.map_err(err)?,
    };
    Ok(crate::highlight::show(canopy_git::parse::diff::parse(&o.stdout)))
}

/// One command for the many small git actions the History, Branches and
/// Stash pages offer. `args` depend on `op`; see the match below.
#[tauri::command]
pub async fn git_op(op: String, args: Vec<String>, state: State<'_, AppState>) -> Res<Done> {
    let git = current(&state).await?;
    let a = |i: usize| args.get(i).map(String::as_str).unwrap_or("");
    let res = match op.as_str() {
        "checkout" => git.checkout(a(0)).await,
        "checkout-remote" => git.checkout_remote(a(0)).await,
        // args: name, start point ("" = HEAD), "switch" to check it out
        "create-branch" => git.create_branch(a(0), Some(a(1)).filter(|s| !s.is_empty()), a(2) == "switch").await,
        "rename-branch" => git.rename_branch(a(0), a(1)).await,
        "delete-branch" => git.delete_branch(a(0), a(1) == "force").await,
        "delete-remote-branch" => git.delete_remote_branch(a(0), a(1)).await,
        "merge" => git.merge(a(0), false).await,
        "rebase" => git.rebase(a(0)).await,
        "cherry-pick" => git.cherry_pick(&[a(0)]).await,
        "revert" => git.revert(a(0)).await,
        "reset-soft" => git.reset(a(0), ResetMode::Soft).await,
        "reset-mixed" => git.reset(a(0), ResetMode::Mixed).await,
        "reset-hard" => git.reset(a(0), ResetMode::Hard).await,
        // args: name, rev, message ("" = lightweight)
        "create-tag" => git.create_tag(a(0), a(1), Some(a(2)).filter(|s| !s.is_empty())).await,
        "delete-tag" => git.delete_tag(a(0)).await,
        "push-tag" => git.push_tag(a(0), a(1)).await,
        // args: message, "untracked" to include new files
        "stash" => git.stash_push(Some(a(0)).filter(|s| !s.is_empty()), a(1) == "untracked").await,
        "stash-apply" => git.stash_apply(a(0)).await,
        "stash-pop" => git.stash_pop(a(0)).await,
        "stash-drop" => git.stash_drop(a(0)).await,
        other => return Err(format!("unknown operation {other}")),
    };
    res.map(done).map_err(err)
}

/// What "Undo" would do right now, from the reflog.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum UndoPlan {
    /// Nothing recorded to undo.
    None,
    /// Switch back to the branch we came from.
    Checkout { last: String, to: String },
    /// Move the branch back one step. `soft` keeps an undone commit's
    /// changes staged.
    Reset { last: String, oid: String, subject: String, soft: bool },
}

/// Enough reflog to see back past a long rebase.
const UNDO_REFLOG: usize = 200;

pub fn undo_plan(reflog: &[ReflogEntry]) -> UndoPlan {
    let Some(last) = reflog.first() else { return UndoPlan::None };
    if let Some((from, _)) = last.subject.strip_prefix("checkout: moving from ").and_then(|r| r.split_once(" to ")) {
        return UndoPlan::Checkout { last: last.subject.clone(), to: from.to_string() };
    }
    // A whole rebase counts as one action.
    let Some(prev) = canopy_git::parse::log::before_last_action(reflog) else { return UndoPlan::None };
    UndoPlan::Reset {
        last: last.subject.clone(),
        oid: prev.oid.clone(),
        subject: prev.subject.clone(),
        soft: last.subject.starts_with("commit"),
    }
}

#[tauri::command]
pub async fn undo_info(state: State<'_, AppState>) -> Res<UndoPlan> {
    let git = current(&state).await?;
    Ok(undo_plan(&git.reflog(UNDO_REFLOG).await.unwrap_or_default()))
}

#[tauri::command]
pub async fn undo(state: State<'_, AppState>) -> Res<Done> {
    let git = current(&state).await?;
    let res = match undo_plan(&git.reflog(UNDO_REFLOG).await.map_err(err)?) {
        UndoPlan::None => return Err("Nothing to undo.".into()),
        UndoPlan::Checkout { to, .. } => git.checkout(&to).await,
        UndoPlan::Reset { oid, soft: true, .. } => git.reset(&oid, ResetMode::Soft).await,
        // --keep refuses when your edits touch files that differ between the
        // two commits; --mixed never touches the working tree.
        UndoPlan::Reset { oid, .. } => match git.run(&["reset", "--keep", &oid]).await {
            Ok(o) => Ok(o),
            Err(_) => git.reset(&oid, ResetMode::Mixed).await,
        },
    };
    res.map(done).map_err(err)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(oid: &str, subject: &str) -> ReflogEntry {
        ReflogEntry { oid: oid.into(), selector: String::new(), subject: subject.into(), time: 0 }
    }

    #[test]
    fn undo_plans() {
        assert_eq!(undo_plan(&[]), UndoPlan::None);
        assert_eq!(
            undo_plan(&[r("b", "checkout: moving from main to feature"), r("a", "commit: x")]),
            UndoPlan::Checkout { last: "checkout: moving from main to feature".into(), to: "main".into() }
        );
        match undo_plan(&[r("b", "commit: add parser"), r("a", "commit (initial): start")]) {
            UndoPlan::Reset { oid, soft, .. } => assert_eq!((oid.as_str(), soft), ("a", true)),
            other => panic!("{other:?}"),
        }
        match undo_plan(&[r("b", "reset: moving to HEAD~2"), r("a", "commit: x")]) {
            UndoPlan::Reset { soft, .. } => assert!(!soft),
            other => panic!("{other:?}"),
        }
        // Undoing a rebase goes back to before it started, not one step.
        let rebase = [
            r("c2", "rebase (finish): returning to refs/heads/feature"),
            r("c2", "rebase (pick): x"),
            r("m", "rebase (start): checkout main"),
            r("f", "commit: x"),
        ];
        match undo_plan(&rebase) {
            UndoPlan::Reset { oid, soft, .. } => assert_eq!((oid.as_str(), soft), ("f", false)),
            other => panic!("{other:?}"),
        }
    }
}
