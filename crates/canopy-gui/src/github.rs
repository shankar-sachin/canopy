//! GitHub pages (pull requests, issues, Actions) through the `gh` CLI via
//! canopy-gh, exactly like the TUI. Nothing is installed: when gh is missing
//! or logged out the page shows how to set it up.

use canopy_gh::{
    CheckState, ChecksSummary, Gh, GhError, GhStatus, Issue, IssueFilter, Job, MergeMethod, PrFilter, PullRequest,
    RepoInfo, ReviewKind, Run,
};
use serde::Serialize;
use tauri::State;

use crate::actions::Done;
use crate::{current, AppState, Res};

async fn gh(state: &AppState) -> Res<Gh> {
    let git = current(state).await?;
    Ok(Gh::new(&git.repo.root, None))
}

fn err(e: GhError) -> String {
    match e {
        GhError::Failed { cmd, stderr } => format!("{cmd}\n{stderr}"),
        e => e.to_string(),
    }
}

fn done(o: canopy_git::Output) -> Done {
    let output = if o.stdout.trim().is_empty() { o.stderr } else { o.stdout };
    Done { cmd: o.cmd, output: output.trim().to_string() }
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Status {
    NotInstalled,
    NotLoggedIn,
    NotGithub,
    Ready { repo: RepoInfo },
}

#[tauri::command]
pub async fn gh_status(state: State<'_, AppState>) -> Res<Status> {
    Ok(match gh(&state).await?.detect().await {
        GhStatus::NotInstalled => Status::NotInstalled,
        GhStatus::NotLoggedIn => Status::NotLoggedIn,
        GhStatus::NotGitHub => Status::NotGithub,
        GhStatus::Ready(repo) => Status::Ready { repo },
    })
}

/// A pull request plus its checks, summed up.
#[derive(Debug, Clone, Serialize)]
pub struct Pr {
    #[serde(flatten)]
    pub pr: PullRequest,
    pub checks: ChecksSummary,
    pub check_state: Option<CheckState>,
}

impl From<PullRequest> for Pr {
    fn from(pr: PullRequest) -> Self {
        let checks = pr.checks();
        Pr { check_state: checks.overall(), checks, pr }
    }
}

#[tauri::command]
pub async fn gh_prs(filter: String, state: State<'_, AppState>) -> Res<Vec<Pr>> {
    let filter = match filter.as_str() {
        "mine" => PrFilter::Mine,
        "review" => PrFilter::ReviewRequested,
        "all" => PrFilter::All,
        _ => PrFilter::Open,
    };
    let prs = gh(&state).await?.pr_list(filter, 50).await.map_err(err)?;
    Ok(prs.into_iter().map(Pr::from).collect())
}

#[tauri::command]
pub async fn gh_pr(number: u64, state: State<'_, AppState>) -> Res<Pr> {
    gh(&state).await?.pr_view(number).await.map(Pr::from).map_err(err)
}

/// For Home: this branch's PR (if any) and how many PRs wait for your review.
#[derive(Debug, Clone, Serialize)]
pub struct HomeCard {
    pub branch_pr: Option<Pr>,
    pub review_requests: usize,
}

#[tauri::command]
pub async fn gh_home(state: State<'_, AppState>) -> Res<HomeCard> {
    let git = current(&state).await?;
    let gh = Gh::new(&git.repo.root, None);
    let branch = git.status().await.ok().and_then(|s| s.branch.head);
    let (pr, reviews) = tokio::join!(
        async {
            match &branch {
                Some(b) => gh.pr_for_branch(b).await.ok().flatten(),
                None => None,
            }
        },
        gh.pr_list(PrFilter::ReviewRequested, 50)
    );
    Ok(HomeCard { branch_pr: pr.map(Pr::from), review_requests: reviews.map(|r| r.len()).unwrap_or(0) })
}

#[tauri::command]
pub async fn gh_issues(filter: String, state: State<'_, AppState>) -> Res<Vec<Issue>> {
    let filter = match filter.as_str() {
        "mine" => IssueFilter::Mine,
        "all" => IssueFilter::All,
        _ => IssueFilter::Open,
    };
    gh(&state).await?.issue_list(filter, 50).await.map_err(err)
}

#[tauri::command]
pub async fn gh_issue(number: u64, state: State<'_, AppState>) -> Res<Issue> {
    gh(&state).await?.issue_view(number).await.map_err(err)
}

#[derive(Debug, Clone, Serialize)]
pub struct RunRow {
    #[serde(flatten)]
    pub run: Run,
    pub state: CheckState,
}

/// Workflow runs of the current branch, or of every branch with `all`.
#[tauri::command]
pub async fn gh_runs(all: bool, state: State<'_, AppState>) -> Res<Vec<RunRow>> {
    let git = current(&state).await?;
    let branch = if all { None } else { git.status().await.ok().and_then(|s| s.branch.head) };
    let gh = Gh::new(&git.repo.root, None);
    let runs = gh.run_list(branch.as_deref(), 30).await.map_err(err)?;
    Ok(runs.into_iter().map(|run| RunRow { state: run.state(), run }).collect())
}

#[tauri::command]
pub async fn gh_run_jobs(id: u64, state: State<'_, AppState>) -> Res<Vec<Job>> {
    gh(&state).await?.run_jobs(id).await.map_err(err)
}

/// The end of the failed steps' log: `step<TAB>text` lines, newest last.
#[tauri::command]
pub async fn gh_failed_log(id: u64, state: State<'_, AppState>) -> Res<String> {
    let log = gh(&state).await?.run_failed_log(id).await.map_err(err)?;
    Ok(tail_log(&log, 150))
}

/// Keep the last `n` lines, drop each line's job name and timestamp.
pub fn tail_log(log: &str, n: usize) -> String {
    let lines: Vec<&str> = log.lines().collect();
    lines[lines.len().saturating_sub(n)..]
        .iter()
        .map(|l| {
            // job<TAB>step<TAB>2026-09-26T00:00:00.0000000Z text
            let mut parts = l.splitn(3, '\t');
            let (_job, step, rest) = (parts.next(), parts.next(), parts.next());
            match (step, rest) {
                (Some(step), Some(rest)) => {
                    let text = rest.split_once(' ').filter(|(t, _)| t.ends_with('Z')).map_or(rest, |(_, t)| t);
                    format!("{step}\t{text}")
                }
                _ => l.to_string(),
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// GitHub actions that change something. `args` depend on `op`.
#[tauri::command]
pub async fn gh_op(op: String, args: Vec<String>, state: State<'_, AppState>) -> Res<Done> {
    let gh = gh(&state).await?;
    let a = |i: usize| args.get(i).map(String::as_str).unwrap_or("");
    let n = || a(0).parse::<u64>().map_err(|_| format!("not a number: {}", a(0)));
    let res = match op.as_str() {
        "pr-checkout" => gh.pr_checkout(n()?).await,
        // args: title, body, base ("" = default branch), "draft"
        "pr-create" => gh.pr_create(a(0), a(1), Some(a(2)).filter(|s| !s.is_empty()), a(3) == "draft").await,
        // args: number, approve | comment | request-changes, body
        "pr-review" => {
            let kind = match a(1) {
                "approve" => ReviewKind::Approve,
                "request-changes" => ReviewKind::RequestChanges,
                _ => ReviewKind::Comment,
            };
            gh.pr_review(n()?, kind, a(2)).await
        }
        "pr-comment" => gh.pr_comment(n()?, a(1)).await,
        // args: number, merge | squash | rebase, "delete" to delete the branch
        "pr-merge" => {
            let method = match a(1) {
                "squash" => MergeMethod::Squash,
                "rebase" => MergeMethod::Rebase,
                _ => MergeMethod::Merge,
            };
            gh.pr_merge(n()?, method, a(2) == "delete").await
        }
        "pr-close" => gh.pr_close(n()?).await,
        "issue-create" => gh.issue_create(a(0), a(1)).await,
        "issue-comment" => gh.issue_comment(n()?, a(1)).await,
        "issue-close" => gh.issue_close(n()?).await,
        "issue-reopen" => gh.issue_reopen(n()?).await,
        "run-rerun" => gh.run_rerun_failed(n()?).await,
        other => return Err(format!("unknown GitHub operation {other}")),
    };
    res.map(done).map_err(err)
}

/// Open a web page in the default browser (GitHub links only).
#[tauri::command]
pub fn open_url(url: String) -> Res<()> {
    if !url.starts_with("https://") {
        return Err(format!("not a web link: {url}"));
    }
    let mut cmd = if cfg!(target_os = "macos") {
        std::process::Command::new("open")
    } else if cfg!(windows) {
        // `start` is a cmd built-in; the empty string is the window title.
        let mut c = std::process::Command::new("cmd");
        c.args(["/C", "start", ""]);
        c
    } else {
        std::process::Command::new("xdg-open")
    };
    cmd.arg(&url)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("couldn't open a browser: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_log_tail() {
        let log = "build\tRun tests\t2026-09-26T10:00:00.1234567Z running 3 tests\n\
                   build\tRun tests\t2026-09-26T10:00:01.0000000Z test x ... FAILED\n\
                   plain line";
        assert_eq!(tail_log(log, 2), "Run tests\ttest x ... FAILED\nplain line");
        assert_eq!(tail_log(log, 10).lines().next(), Some("Run tests\trunning 3 tests"));
    }

    #[test]
    fn prs_serialize_with_checks() {
        let pr: PullRequest = serde_json::from_str(
            r#"{"number":7,"title":"t","state":"OPEN","url":"https://x","statusCheckRollup":[{"name":"ci","status":"COMPLETED","conclusion":"FAILURE"}]}"#,
        )
        .unwrap();
        let v = serde_json::to_value(Pr::from(pr)).unwrap();
        assert_eq!(v["number"], 7);
        assert_eq!(v["check_state"], "failed");
        assert_eq!(v["checks"]["failed"], 1);
        assert_eq!(v["statusCheckRollup"][0]["name"], "ci");
    }

    /// Turns canopy-gh's fixtures into what the page receives, for the UI
    /// preview harness: `CANOPY_GH_OUT=/tmp/gh.json cargo test -p canopy-desktop dump_gh -- --ignored`
    #[test]
    #[ignore]
    fn dump_gh() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../canopy-gh/tests/fixtures/");
        let read = |f: &str| std::fs::read_to_string(format!("{dir}{f}")).unwrap();
        let prs: Vec<PullRequest> = serde_json::from_str(&read("pr_list.json")).unwrap();
        let pr: PullRequest = serde_json::from_str(&read("pr_view.json")).unwrap();
        let issues: Vec<Issue> = serde_json::from_str(&read("issue_list.json")).unwrap();
        let issue: Issue = serde_json::from_str(&read("issue_view.json")).unwrap();
        let runs: Vec<Run> = serde_json::from_str(&read("run_list.json")).unwrap();
        #[derive(serde::Deserialize)]
        struct Jobs {
            jobs: Vec<Job>,
        }
        let jobs: Jobs = serde_json::from_str(&read("run_jobs.json")).unwrap();
        let out = serde_json::json!({
            "prs": prs.into_iter().map(Pr::from).collect::<Vec<_>>(),
            "pr": Pr::from(pr),
            "issues": issues,
            "issue": issue,
            "runs": runs.into_iter().map(|run| RunRow { state: run.state(), run }).collect::<Vec<_>>(),
            "jobs": jobs.jobs,
        });
        std::fs::write(std::env::var("CANOPY_GH_OUT").unwrap(), serde_json::to_string_pretty(&out).unwrap()).unwrap();
    }
}
