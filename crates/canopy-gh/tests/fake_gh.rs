//! Runs `canopy-gh` against a fake `gh` script that returns fixture JSON and
//! records its arguments, so these tests need no network or GitHub login.

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use canopy_gh::{CheckState, Gh, GhStatus, IssueFilter, MergeMethod, PrFilter, ReviewKind};
use tempfile::TempDir;

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures");

/// A fake gh: logs "$@" to calls.log, then prints a fixture chosen by args.
fn fake_gh(dir: &Path, logged_in: bool) -> PathBuf {
    let script = format!(
        r#"#!/bin/sh
echo "$@" >> "{log}"
case "$1 $2" in
  "auth status") {auth} ;;
  "repo view") echo '{{"nameWithOwner":"o/r","url":"https://github.com/o/r","defaultBranchRef":{{"name":"main"}}}}' ;;
  "api user") echo ada ;;
  "pr list") cat "{f}/pr_list.json" ;;
  "pr view") cat "{f}/pr_view.json" ;;
  "pr diff") printf 'diff --git a/x b/x\n' ;;
  "issue list") cat "{f}/issue_list.json" ;;
  "run list") cat "{f}/run_list.json" ;;
  "run view") case "$*" in *--log-failed*) printf 'test\tRun cargo test\tpanicked at x\n' ;; *) cat "{f}/run_jobs.json" ;; esac ;;
  "pr merge") echo "merged" ;;
  *) : ;;
esac
"#,
        log = dir.join("calls.log").display(),
        f = FIXTURES,
        auth = if logged_in { "exit 0" } else { "echo 'You are not logged into any GitHub hosts' >&2; exit 1" },
    );
    let path = dir.join("gh");
    std::fs::write(&path, script).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    path
}

fn calls(dir: &Path) -> Vec<String> {
    std::fs::read_to_string(dir.join("calls.log")).unwrap_or_default().lines().map(String::from).collect()
}

#[tokio::test]
async fn detect_states() {
    let dir = TempDir::new().unwrap();
    let gh = Gh::new(dir.path(), Some(fake_gh(dir.path(), true)));
    match gh.detect().await {
        GhStatus::Ready(info) => assert_eq!(info.name_with_owner, "o/r"),
        other => panic!("{other:?}"),
    }
    let gh = Gh::new(dir.path(), Some(fake_gh(dir.path(), false)));
    assert_eq!(gh.detect().await, GhStatus::NotLoggedIn);
    let gh = Gh::new(dir.path(), Some(dir.path().join("no-such-gh")));
    assert_eq!(gh.detect().await, GhStatus::NotInstalled);
}

#[tokio::test]
async fn pull_requests() {
    let dir = TempDir::new().unwrap();
    let gh = Gh::new(dir.path(), Some(fake_gh(dir.path(), true)));
    let prs = gh.pr_list(PrFilter::ReviewRequested, 30).await.unwrap();
    assert_eq!(prs.len(), 2);
    let pr = &prs[0];
    assert_eq!((pr.number, pr.author.login.as_str(), pr.head_ref_name.as_str()), (12, "ada", "feature/parser"));
    assert_eq!(pr.labels[0].name, "enhancement");
    assert_eq!(pr.updated_at, canopy_gh::parse_rfc3339("2026-09-26T00:41:06Z").unwrap());
    // One passed, one running, one legacy status failed.
    let c = pr.checks();
    assert_eq!((c.passed, c.pending, c.failed, c.total), (1, 1, 1, 3));
    assert_eq!(c.overall(), Some(CheckState::Failed));
    assert_eq!(pr.status_check_rollup[2].name, "ci/legacy");
    // Nulls become defaults.
    assert!(prs[1].is_draft && prs[1].labels.is_empty() && prs[1].review_decision.is_empty());
    assert_eq!(prs[1].checks().overall(), None);

    let pr = gh.pr_view(12).await.unwrap();
    assert!(pr.body.contains("Parses CSV"));
    assert_eq!(pr.reviews[0].state, "APPROVED");
    assert_eq!(pr.comments[0].body, "Nice!");
    assert_eq!(pr.mergeable, "MERGEABLE");

    gh.pr_review(12, ReviewKind::Approve, "").await.unwrap();
    let out = gh.pr_merge(12, MergeMethod::Squash, true).await.unwrap();
    assert_eq!(out.cmd, "gh pr merge 12 --squash --delete-branch");
    gh.pr_create("Add it", "Body text", Some("main"), true).await.unwrap();

    let log = calls(dir.path());
    assert!(log.iter().any(|l| l.starts_with("pr list") && l.contains("--search review-requested:@me")), "{log:?}");
    assert!(log.contains(&"pr review 12 --approve".to_string()), "{log:?}");
    assert!(log.contains(&"pr create --title Add it --body Body text --base main --draft".to_string()), "{log:?}");
}

#[tokio::test]
async fn issues_and_runs() {
    let dir = TempDir::new().unwrap();
    let gh = Gh::new(dir.path(), Some(fake_gh(dir.path(), true)));
    let issues = gh.issue_list(IssueFilter::Mine, 30).await.unwrap();
    assert_eq!(issues[0].number, 7);
    assert_eq!(issues[0].comments.len(), 1);
    assert_eq!(issues[0].labels[0].name, "bug");

    let runs = gh.run_list(Some("feature/parser"), 20).await.unwrap();
    assert_eq!(runs[0].state(), CheckState::Failed);
    assert_eq!(runs[1].state(), CheckState::Pending);
    let jobs = gh.run_jobs(1001).await.unwrap();
    assert_eq!(jobs[0].steps[1].conclusion, "failure");
    assert!(gh.run_failed_log(1001).await.unwrap().contains("panicked"));
    assert_eq!(gh.viewer().await.unwrap(), "ada");

    let log = calls(dir.path());
    assert!(log.iter().any(|l| l.starts_with("issue list") && l.contains("--assignee @me")), "{log:?}");
    assert!(log.iter().any(|l| l.starts_with("run list") && l.contains("--branch feature/parser")), "{log:?}");
}

#[tokio::test]
async fn failures_are_reported() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("gh");
    std::fs::write(&path, "#!/bin/sh\necho 'GraphQL: Could not resolve to a Repository' >&2\nexit 1\n").unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    let gh = Gh::new(dir.path(), Some(path));
    let err = gh.pr_list(PrFilter::Open, 10).await.unwrap_err().to_string();
    assert!(err.contains("Could not resolve"), "{err}");
}
