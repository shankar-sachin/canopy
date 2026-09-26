//! GitHub backend for Canopy, built on the `gh` CLI.
//!
//! Like `canopy-git`, every call shells out to a real binary (`gh`) and
//! parses its machine-readable output (`--json`). Nothing is ever installed:
//! if `gh` is missing or logged out, [`Gh::detect`] says so and the UI shows
//! a setup card instead.

pub mod model;
mod time;

use std::path::{Path, PathBuf};
use std::process::Stdio;

use canopy_git::cli::display_program_cmd;
use canopy_git::Output;
use serde::de::DeserializeOwned;
use tokio::process::Command;

pub use model::*;
pub use time::parse_rfc3339;

#[derive(Debug, thiserror::Error)]
pub enum GhError {
    #[error("the GitHub CLI (gh) isn't installed")]
    NotInstalled,
    #[error("failed to run gh: {0}")]
    Spawn(std::io::Error),
    #[error("`{cmd}` failed: {stderr}")]
    Failed { cmd: String, stderr: String },
    #[error("couldn't read gh's output: {0}")]
    Parse(String),
}

pub type Result<T> = std::result::Result<T, GhError>;

/// Whether GitHub features can be used in this repository.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GhStatus {
    NotInstalled,
    NotLoggedIn,
    /// `gh` works, but this repo has no GitHub remote.
    NotGitHub,
    Ready(RepoInfo),
}

#[derive(Debug, Clone)]
pub struct Gh {
    program: PathBuf,
    root: PathBuf,
}

/// JSON fields requested for PR lists (kept in one place so the model and
/// the query can't drift apart).
const PR_FIELDS: &str = "number,title,author,headRefName,baseRefName,isDraft,state,url,updatedAt,\
reviewDecision,statusCheckRollup,labels,additions,deletions";
const PR_DETAIL_FIELDS: &str = "number,title,author,headRefName,baseRefName,isDraft,state,url,updatedAt,\
reviewDecision,statusCheckRollup,labels,additions,deletions,body,reviews,comments,mergeable";
const ISSUE_FIELDS: &str = "number,title,author,state,labels,url,updatedAt,comments";
const ISSUE_DETAIL_FIELDS: &str = "number,title,author,state,labels,url,updatedAt,comments,body";
const RUN_FIELDS: &str = "databaseId,number,displayTitle,workflowName,headBranch,status,conclusion,event,createdAt,url";

impl Gh {
    /// A handle that runs `gh` (or `program`, for tests) in `root`.
    pub fn new(root: impl AsRef<Path>, program: Option<PathBuf>) -> Self {
        let program =
            program.or_else(|| std::env::var_os("CANOPY_GH").map(PathBuf::from)).unwrap_or_else(|| PathBuf::from("gh"));
        Gh { program, root: root.as_ref().to_path_buf() }
    }

    fn command(&self, args: &[&str]) -> Command {
        let mut cmd = Command::new(&self.program);
        cmd.args(args)
            .current_dir(&self.root)
            .env("GH_PROMPT_DISABLED", "1")
            .env("GH_NO_UPDATE_NOTIFIER", "1")
            .env("GH_PAGER", "")
            .env("NO_COLOR", "1")
            .env("CLICOLOR", "0")
            .stdin(Stdio::null());
        cmd
    }

    /// Run `gh` and return its output; a non-zero exit is an error.
    pub async fn run(&self, args: &[&str]) -> Result<Output> {
        let out = match self.command(args).output().await {
            Ok(o) => o,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Err(GhError::NotInstalled),
            Err(e) => return Err(GhError::Spawn(e)),
        };
        let cmd = display_program_cmd("gh", args);
        let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
        if !out.status.success() {
            let stderr = if stderr.trim().is_empty() { stdout } else { stderr };
            return Err(GhError::Failed { cmd, stderr: stderr.trim().to_string() });
        }
        Ok(Output { cmd, stdout, stderr })
    }

    async fn json<T: DeserializeOwned>(&self, args: &[&str]) -> Result<T> {
        let out = self.run(args).await?;
        serde_json::from_str(&out.stdout).map_err(|e| GhError::Parse(format!("{}: {e}", out.cmd)))
    }

    /// Check that `gh` exists, is logged in, and this repo is on GitHub.
    pub async fn detect(&self) -> GhStatus {
        match self.run(&["auth", "status"]).await {
            Err(GhError::NotInstalled) => return GhStatus::NotInstalled,
            Err(_) => return GhStatus::NotLoggedIn,
            Ok(_) => {}
        }
        match self.json::<RepoInfo>(&["repo", "view", "--json", "nameWithOwner,url,defaultBranchRef"]).await {
            Ok(info) => GhStatus::Ready(info),
            Err(_) => GhStatus::NotGitHub,
        }
    }

    /// Login of the authenticated user.
    pub async fn viewer(&self) -> Result<String> {
        let out = self.run(&["api", "user", "--jq", ".login"]).await?;
        Ok(out.stdout.trim().to_string())
    }

    // --------------------------------------------------------- pull requests

    pub async fn pr_list(&self, filter: PrFilter, limit: usize) -> Result<Vec<PullRequest>> {
        let limit = limit.to_string();
        let mut args = vec!["pr", "list", "--json", PR_FIELDS, "--limit", &limit];
        match filter {
            PrFilter::Open => {}
            PrFilter::Mine => args.extend(["--author", "@me"]),
            PrFilter::ReviewRequested => args.extend(["--search", "review-requested:@me"]),
            PrFilter::All => args.extend(["--state", "all"]),
        }
        self.json(&args).await
    }

    pub async fn pr_view(&self, number: u64) -> Result<PullRequest> {
        let n = number.to_string();
        self.json(&["pr", "view", &n, "--json", PR_DETAIL_FIELDS]).await
    }

    /// The open PR whose head is `branch`, if any.
    pub async fn pr_for_branch(&self, branch: &str) -> Result<Option<PullRequest>> {
        let prs: Vec<PullRequest> = self
            .json(&["pr", "list", "--head", branch, "--state", "open", "--json", PR_FIELDS, "--limit", "1"])
            .await?;
        Ok(prs.into_iter().next())
    }

    pub async fn pr_diff(&self, number: u64) -> Result<String> {
        let n = number.to_string();
        Ok(self.run(&["pr", "diff", &n, "--color", "never"]).await?.stdout)
    }

    pub async fn pr_checkout(&self, number: u64) -> Result<Output> {
        let n = number.to_string();
        self.run(&["pr", "checkout", &n]).await
    }

    pub async fn pr_create(&self, title: &str, body: &str, base: Option<&str>, draft: bool) -> Result<Output> {
        let mut args = vec!["pr", "create", "--title", title, "--body", body];
        if let Some(b) = base {
            args.extend(["--base", b]);
        }
        if draft {
            args.push("--draft");
        }
        self.run(&args).await
    }

    pub async fn pr_review(&self, number: u64, kind: ReviewKind, body: &str) -> Result<Output> {
        let n = number.to_string();
        let flag = match kind {
            ReviewKind::Approve => "--approve",
            ReviewKind::Comment => "--comment",
            ReviewKind::RequestChanges => "--request-changes",
        };
        let mut args = vec!["pr", "review", &n, flag];
        if !body.is_empty() {
            args.extend(["--body", body]);
        }
        self.run(&args).await
    }

    pub async fn pr_comment(&self, number: u64, body: &str) -> Result<Output> {
        let n = number.to_string();
        self.run(&["pr", "comment", &n, "--body", body]).await
    }

    pub async fn pr_merge(&self, number: u64, method: MergeMethod, delete_branch: bool) -> Result<Output> {
        let n = number.to_string();
        let flag = match method {
            MergeMethod::Merge => "--merge",
            MergeMethod::Squash => "--squash",
            MergeMethod::Rebase => "--rebase",
        };
        let mut args = vec!["pr", "merge", &n, flag];
        if delete_branch {
            args.push("--delete-branch");
        }
        self.run(&args).await
    }

    pub async fn pr_close(&self, number: u64) -> Result<Output> {
        let n = number.to_string();
        self.run(&["pr", "close", &n]).await
    }

    // ---------------------------------------------------------------- issues

    pub async fn issue_list(&self, filter: IssueFilter, limit: usize) -> Result<Vec<Issue>> {
        let limit = limit.to_string();
        let mut args = vec!["issue", "list", "--json", ISSUE_FIELDS, "--limit", &limit];
        match filter {
            IssueFilter::Open => {}
            IssueFilter::Mine => args.extend(["--assignee", "@me"]),
            IssueFilter::All => args.extend(["--state", "all"]),
        }
        self.json(&args).await
    }

    pub async fn issue_view(&self, number: u64) -> Result<Issue> {
        let n = number.to_string();
        self.json(&["issue", "view", &n, "--json", ISSUE_DETAIL_FIELDS]).await
    }

    pub async fn issue_create(&self, title: &str, body: &str) -> Result<Output> {
        self.run(&["issue", "create", "--title", title, "--body", body]).await
    }

    pub async fn issue_comment(&self, number: u64, body: &str) -> Result<Output> {
        let n = number.to_string();
        self.run(&["issue", "comment", &n, "--body", body]).await
    }

    pub async fn issue_close(&self, number: u64) -> Result<Output> {
        let n = number.to_string();
        self.run(&["issue", "close", &n]).await
    }

    pub async fn issue_reopen(&self, number: u64) -> Result<Output> {
        let n = number.to_string();
        self.run(&["issue", "reopen", &n]).await
    }

    // --------------------------------------------------------------- actions

    pub async fn run_list(&self, branch: Option<&str>, limit: usize) -> Result<Vec<Run>> {
        let limit = limit.to_string();
        let mut args = vec!["run", "list", "--json", RUN_FIELDS, "--limit", &limit];
        if let Some(b) = branch {
            args.extend(["--branch", b]);
        }
        self.json(&args).await
    }

    pub async fn run_jobs(&self, id: u64) -> Result<Vec<Job>> {
        #[derive(serde::Deserialize)]
        struct Jobs {
            jobs: Vec<Job>,
        }
        let id = id.to_string();
        Ok(self.json::<Jobs>(&["run", "view", &id, "--json", "jobs"]).await?.jobs)
    }

    /// Log lines of the failed steps of a run.
    pub async fn run_failed_log(&self, id: u64) -> Result<String> {
        let id = id.to_string();
        Ok(self.run(&["run", "view", &id, "--log-failed"]).await?.stdout)
    }

    pub async fn run_rerun_failed(&self, id: u64) -> Result<Output> {
        let id = id.to_string();
        self.run(&["run", "rerun", &id, "--failed"]).await
    }
}
