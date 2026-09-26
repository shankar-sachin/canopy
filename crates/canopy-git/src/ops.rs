//! Typed git operations. Reads return parsed models; writes return the
//! [`Output`] (which carries the command string for teach mode).

use tokio::sync::mpsc;

use crate::cli::{Git, Output, Result};
use crate::model::*;
use crate::parse::{self, diff::PatchMode};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResetMode {
    Soft,
    Mixed,
    Hard,
}

impl ResetMode {
    fn flag(self) -> &'static str {
        match self {
            ResetMode::Soft => "--soft",
            ResetMode::Mixed => "--mixed",
            ResetMode::Hard => "--hard",
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct LogQuery {
    pub rev: Option<String>,
    pub all: bool,
    pub limit: usize,
    pub skip: usize,
    pub author: Option<String>,
    pub grep: Option<String>,
    pub path: Option<String>,
    /// Follow renames (only meaningful with `path`).
    pub follow: bool,
}

#[derive(Debug, Clone, Default)]
pub struct CommitOpts {
    pub amend: bool,
    pub signoff: bool,
    pub no_verify: bool,
    pub allow_empty: bool,
}

impl Git {
    // ---------------------------------------------------------------- reads

    pub async fn status(&self) -> Result<Status> {
        let out = self.run(&["status", "--porcelain=v2", "--branch", "-z", "--untracked-files=all"]).await?;
        parse::status::parse(&out.stdout)
    }

    pub async fn log(&self, q: &LogQuery) -> Result<Vec<Commit>> {
        let limit = format!("-n{}", if q.limit == 0 { 200 } else { q.limit });
        let skip = format!("--skip={}", q.skip);
        let mut args: Vec<String> = vec!["log".into(), parse::log::FORMAT.into(), "--topo-order".into(), limit, skip];
        if let Some(a) = &q.author {
            args.push(format!("--author={a}"));
        }
        if let Some(g) = &q.grep {
            args.push(format!("--grep={g}"));
            args.push("-i".into());
        }
        if q.all {
            args.push("--all".into());
        } else if let Some(r) = &q.rev {
            args.push(r.clone());
        }
        if let Some(p) = &q.path {
            if q.follow {
                args.push("--follow".into());
            }
            args.push("--".into());
            args.push(p.clone());
        }
        let argv: Vec<&str> = args.iter().map(String::as_str).collect();
        match self.run(&argv).await {
            Ok(out) => parse::log::parse_commits(&out.stdout),
            // Fresh repo with no commits yet.
            Err(crate::GitError::Failed { stderr, .. }) if stderr.contains("does not have any commits") => {
                Ok(Vec::new())
            }
            Err(e) => Err(e),
        }
    }

    pub async fn branches(&self) -> Result<Vec<Branch>> {
        let out = self
            .run(&["for-each-ref", parse::refs::BRANCH_FORMAT, "--sort=-committerdate", "refs/heads", "refs/remotes"])
            .await?;
        parse::refs::parse_branches(&out.stdout)
    }

    pub async fn tags(&self) -> Result<Vec<Tag>> {
        let out = self.run(&["for-each-ref", parse::refs::TAG_FORMAT, "--sort=-creatordate", "refs/tags"]).await?;
        parse::refs::parse_tags(&out.stdout)
    }

    pub async fn remotes(&self) -> Result<Vec<Remote>> {
        let out = self.run(&["remote", "-v"]).await?;
        Ok(parse::refs::parse_remotes(&out.stdout))
    }

    pub async fn stashes(&self) -> Result<Vec<Stash>> {
        let out = self.run(&["stash", "list", parse::log::SELECTOR_FORMAT]).await?;
        parse::log::parse_stashes(&out.stdout)
    }

    pub async fn reflog(&self, limit: usize) -> Result<Vec<ReflogEntry>> {
        let n = format!("-n{limit}");
        match self.run(&["reflog", parse::log::SELECTOR_FORMAT, &n]).await {
            Ok(out) => parse::log::parse_reflog(&out.stdout),
            Err(_) => Ok(Vec::new()),
        }
    }

    /// Diff of the working tree (`staged = false`) or index (`staged = true`).
    pub async fn diff_file(&self, path: &str, staged: bool, context: u32) -> Result<Vec<FileDiff>> {
        let u = format!("-U{context}");
        let mut args = vec!["diff", "--no-ext-diff", u.as_str()];
        if staged {
            args.push("--cached");
        }
        args.extend(["--", path]);
        let out = self.run(&args).await?;
        Ok(parse::diff::parse(&out.stdout))
    }

    /// Diff for an untracked file, shown as a new file.
    pub async fn diff_untracked(&self, path: &str) -> Result<Vec<FileDiff>> {
        // `--no-index` exits 1 when there are differences; treat as success.
        let args = ["diff", "--no-ext-diff", "--no-index", "--", "/dev/null", path];
        let out = match self.run(&args).await {
            Ok(o) => o.stdout,
            Err(crate::GitError::Failed { code: 1, stderr, .. }) => stderr,
            Err(e) => return Err(e),
        };
        Ok(parse::diff::parse(&out))
    }

    pub async fn show(&self, rev: &str) -> Result<(String, Vec<FileDiff>)> {
        let out = self.run(&["show", "--no-ext-diff", "--stat", "--patch", "--format=fuller", rev]).await?;
        let body_start = out.stdout.find("\ndiff --git ").map(|i| i + 1);
        let (header, diff) = match body_start {
            Some(i) => out.stdout.split_at(i),
            None => (out.stdout.as_str(), ""),
        };
        Ok((header.to_string(), parse::diff::parse(diff)))
    }

    pub async fn diff_refs(&self, from: &str, to: &str) -> Result<Vec<FileDiff>> {
        let range = format!("{from}...{to}");
        let out = self.run(&["diff", "--no-ext-diff", &range]).await?;
        Ok(parse::diff::parse(&out.stdout))
    }

    pub async fn config_get(&self, key: &str) -> Option<String> {
        self.run(&["config", "--get", key]).await.ok().map(|o| o.stdout.trim().to_string())
    }

    // --------------------------------------------------------------- writes

    pub async fn stage(&self, paths: &[&str]) -> Result<Output> {
        let mut args = vec!["add", "--"];
        args.extend_from_slice(paths);
        self.run(&args).await
    }

    pub async fn stage_all(&self) -> Result<Output> {
        self.run(&["add", "--all"]).await
    }

    pub async fn unstage(&self, paths: &[&str]) -> Result<Output> {
        let mut args = vec!["restore", "--staged", "--"];
        args.extend_from_slice(paths);
        match self.run(&args).await {
            Ok(o) => Ok(o),
            // No HEAD yet: fall back to removing from the index.
            Err(_) => {
                let mut args = vec!["rm", "--cached", "-r", "-q", "--"];
                args.extend_from_slice(paths);
                self.run(&args).await
            }
        }
    }

    pub async fn unstage_all(&self) -> Result<Output> {
        self.run(&["reset", "-q"]).await
    }

    /// Discard worktree changes to tracked files.
    pub async fn discard(&self, paths: &[&str]) -> Result<Output> {
        let mut args = vec!["restore", "--worktree", "--"];
        args.extend_from_slice(paths);
        self.run(&args).await
    }

    /// Delete untracked files.
    pub async fn clean(&self, paths: &[&str]) -> Result<Output> {
        let mut args = vec!["clean", "-f", "-d", "--"];
        args.extend_from_slice(paths);
        self.run(&args).await
    }

    /// Stage or unstage selected lines of a hunk.
    pub async fn apply_lines(
        &self,
        file: &FileDiff,
        hunk: &Hunk,
        lines: &[usize],
        mode: PatchMode,
    ) -> Result<Option<Output>> {
        let Some(patch) = parse::diff::build_patch(file, hunk, lines, mode) else {
            return Ok(None);
        };
        let mut args = vec!["apply", "--cached", "--unidiff-zero", "--whitespace=nowarn"];
        if mode == PatchMode::Unstage {
            args.push("--reverse");
        }
        args.push("-");
        let mut out = self.run_with_stdin(&args, &patch).await?;
        out.cmd.push_str("  # stdin: a patch with just the selected lines");
        Ok(Some(out))
    }

    /// Stage or unstage a whole hunk.
    pub async fn apply_hunk(&self, file: &FileDiff, hunk: &Hunk, mode: PatchMode) -> Result<Option<Output>> {
        let all: Vec<usize> = (0..hunk.lines.len()).collect();
        self.apply_lines(file, hunk, &all, mode).await
    }

    pub async fn commit(&self, message: &str, opts: &CommitOpts) -> Result<Output> {
        let mut args = vec!["commit", "-m", message];
        if opts.amend {
            args.push("--amend");
        }
        if opts.signoff {
            args.push("--signoff");
        }
        if opts.no_verify {
            args.push("--no-verify");
        }
        if opts.allow_empty {
            args.push("--allow-empty");
        }
        self.run(&args).await
    }

    pub async fn commit_fixup(&self, oid: &str) -> Result<Output> {
        let f = format!("--fixup={oid}");
        self.run(&["commit", &f]).await
    }

    pub async fn checkout(&self, rev: &str) -> Result<Output> {
        // `switch` refuses bare commits without --detach; fall back to checkout.
        match self.run(&["switch", rev]).await {
            Ok(o) => Ok(o),
            Err(_) => self.run(&["checkout", rev]).await,
        }
    }

    /// Check out a remote branch as a new local tracking branch.
    pub async fn checkout_remote(&self, remote_branch: &str) -> Result<Output> {
        self.run(&["switch", "--track", remote_branch]).await
    }

    pub async fn create_branch(&self, name: &str, start: Option<&str>, switch: bool) -> Result<Output> {
        let mut args = if switch { vec!["switch", "-c", name] } else { vec!["branch", name] };
        if let Some(s) = start {
            args.push(s);
        }
        self.run(&args).await
    }

    pub async fn rename_branch(&self, old: &str, new: &str) -> Result<Output> {
        self.run(&["branch", "-m", old, new]).await
    }

    pub async fn delete_branch(&self, name: &str, force: bool) -> Result<Output> {
        self.run(&["branch", if force { "-D" } else { "-d" }, name]).await
    }

    pub async fn delete_remote_branch(&self, remote: &str, branch: &str) -> Result<Output> {
        self.run(&["push", remote, "--delete", branch]).await
    }

    pub async fn set_upstream(&self, upstream: &str) -> Result<Output> {
        self.run(&["branch", "--set-upstream-to", upstream]).await
    }

    pub async fn merge(&self, rev: &str, no_ff: bool) -> Result<Output> {
        let mut args = vec!["merge", "--no-edit"];
        if no_ff {
            args.push("--no-ff");
        }
        args.push(rev);
        self.run(&args).await
    }

    pub async fn rebase(&self, onto: &str) -> Result<Output> {
        self.run(&["rebase", onto]).await
    }

    /// Run an interactive rebase with a pre-built todo list, no editor needed.
    pub async fn rebase_interactive(&self, base: &str, todo: &str) -> Result<Output> {
        let dir = std::env::temp_dir().join(format!("canopy-todo-{}", std::process::id()));
        std::fs::write(&dir, todo)?;
        let editor = format!("cp {}", dir.display());
        let seq = format!("sequence.editor={editor}");
        let res = self
            // core.editor=true keeps squash/fixup messages without prompting.
            .run(&["-c", &seq, "-c", "core.editor=true", "rebase", "-i", "--autostash", base])
            .await;
        let _ = std::fs::remove_file(&dir);
        res
    }

    pub async fn rebase_continue(&self) -> Result<Output> {
        // Keep the existing message rather than opening an editor.
        self.run(&["-c", "core.editor=true", "rebase", "--continue"]).await
    }

    pub async fn rebase_abort(&self) -> Result<Output> {
        self.run(&["rebase", "--abort"]).await
    }

    pub async fn rebase_skip(&self) -> Result<Output> {
        self.run(&["rebase", "--skip"]).await
    }

    pub async fn merge_abort(&self) -> Result<Output> {
        self.run(&["merge", "--abort"]).await
    }

    pub async fn merge_continue(&self) -> Result<Output> {
        self.run(&["-c", "core.editor=true", "merge", "--continue"]).await
    }

    pub async fn cherry_pick(&self, oids: &[&str]) -> Result<Output> {
        let mut args = vec!["cherry-pick"];
        args.extend_from_slice(oids);
        self.run(&args).await
    }

    pub async fn cherry_pick_abort(&self) -> Result<Output> {
        self.run(&["cherry-pick", "--abort"]).await
    }

    pub async fn revert(&self, oid: &str) -> Result<Output> {
        self.run(&["revert", "--no-edit", oid]).await
    }

    pub async fn reset(&self, rev: &str, mode: ResetMode) -> Result<Output> {
        self.run(&["reset", mode.flag(), rev]).await
    }

    pub async fn create_tag(&self, name: &str, rev: &str, message: Option<&str>) -> Result<Output> {
        match message {
            Some(m) => self.run(&["tag", "-a", name, "-m", m, rev]).await,
            None => self.run(&["tag", name, rev]).await,
        }
    }

    pub async fn delete_tag(&self, name: &str) -> Result<Output> {
        self.run(&["tag", "-d", name]).await
    }

    pub async fn stash_push(&self, message: Option<&str>, include_untracked: bool) -> Result<Output> {
        let mut args = vec!["stash", "push"];
        if include_untracked {
            args.push("--include-untracked");
        }
        if let Some(m) = message {
            args.extend(["-m", m]);
        }
        self.run(&args).await
    }

    pub async fn stash_apply(&self, name: &str) -> Result<Output> {
        self.run(&["stash", "apply", name]).await
    }

    pub async fn stash_pop(&self, name: &str) -> Result<Output> {
        self.run(&["stash", "pop", name]).await
    }

    pub async fn stash_drop(&self, name: &str) -> Result<Output> {
        self.run(&["stash", "drop", name]).await
    }

    pub async fn add_remote(&self, name: &str, url: &str) -> Result<Output> {
        self.run(&["remote", "add", name, url]).await
    }

    pub async fn remove_remote(&self, name: &str) -> Result<Output> {
        self.run(&["remote", "remove", name]).await
    }

    pub async fn rename_remote(&self, old: &str, new: &str) -> Result<Output> {
        self.run(&["remote", "rename", old, new]).await
    }

    pub async fn set_remote_url(&self, name: &str, url: &str) -> Result<Output> {
        self.run(&["remote", "set-url", name, url]).await
    }

    pub async fn delete_remote_tag(&self, remote: &str, tag: &str) -> Result<Output> {
        let refspec = format!("refs/tags/{tag}");
        self.run(&["push", remote, "--delete", &refspec]).await
    }

    pub async fn fetch(&self, remote: Option<&str>, progress: mpsc::UnboundedSender<String>) -> Result<Output> {
        let mut args = vec!["fetch", "--progress", "--prune"];
        match remote {
            Some(r) => args.push(r),
            None => args.push("--all"),
        }
        self.run_streaming(&args, progress).await
    }

    pub async fn pull(&self, rebase: bool, progress: mpsc::UnboundedSender<String>) -> Result<Output> {
        let args =
            if rebase { vec!["pull", "--progress", "--rebase"] } else { vec!["pull", "--progress", "--no-rebase"] };
        self.run_streaming(&args, progress).await
    }

    pub async fn push(
        &self,
        remote: &str,
        branch: &str,
        set_upstream: bool,
        force_with_lease: bool,
        progress: mpsc::UnboundedSender<String>,
    ) -> Result<Output> {
        let mut args = vec!["push", "--progress"];
        if set_upstream {
            args.push("--set-upstream");
        }
        if force_with_lease {
            args.push("--force-with-lease");
        }
        args.extend([remote, branch]);
        self.run_streaming(&args, progress).await
    }

    pub async fn push_tag(&self, remote: &str, tag: &str) -> Result<Output> {
        self.run(&["push", remote, tag]).await
    }

    /// Mark a conflicted file resolved by taking one side.
    pub async fn checkout_side(&self, path: &str, ours: bool) -> Result<Output> {
        let side = if ours { "--ours" } else { "--theirs" };
        self.run(&["checkout", side, "--", path]).await?;
        self.stage(&[path]).await
    }

    /// Who last changed each line of `path` at `rev` (working tree if `None`).
    pub async fn blame(&self, path: &str, rev: Option<&str>) -> Result<crate::parse::blame::Blame> {
        let mut args = vec!["blame", "--porcelain"];
        if let Some(r) = rev {
            args.push(r);
        }
        args.extend(["--", path]);
        let out = self.run(&args).await?;
        crate::parse::blame::parse(&out.stdout)
    }

    /// Put the conflict markers back into a file (undoes a manual resolution).
    pub async fn restore_conflict(&self, path: &str) -> Result<Output> {
        self.run(&["checkout", "-m", "--", path]).await
    }

    /// Run an arbitrary git command typed by the user.
    pub async fn raw(&self, args: &[&str]) -> Result<Output> {
        self.run(args).await
    }
}
