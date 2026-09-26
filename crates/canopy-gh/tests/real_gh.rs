//! Talks to real GitHub through the installed `gh`. Ignored by default (CI
//! has no login); run with `cargo test -p canopy-gh --test real_gh -- --ignored`.

use canopy_gh::{Gh, GhStatus, IssueFilter, PrFilter};

#[tokio::test]
#[ignore]
async fn against_this_repository() {
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");
    let gh = Gh::new(root, None);
    let GhStatus::Ready(info) = gh.detect().await else { panic!("gh not ready") };
    println!("repo: {}", info.name_with_owner);
    let prs = gh.pr_list(PrFilter::All, 5).await.unwrap();
    assert!(!prs.is_empty());
    let pr = gh.pr_view(prs[0].number).await.unwrap();
    println!("PR #{} {} checks={:?} updated={}", pr.number, pr.title, pr.checks(), pr.updated_at);
    assert!(pr.updated_at > 1_700_000_000);
    let runs = gh.run_list(None, 3).await.unwrap();
    println!("runs: {:?}", runs.iter().map(|r| (r.number, r.state())).collect::<Vec<_>>());
    if let Some(r) = runs.first() {
        let jobs = gh.run_jobs(r.database_id).await.unwrap();
        println!("jobs of run {}: {:?}", r.number, jobs.iter().map(|j| &j.name).collect::<Vec<_>>());
    }
    let issues = gh.issue_list(IssueFilter::All, 5).await.unwrap();
    println!("issues: {}", issues.len());
    println!("viewer: {}", gh.viewer().await.unwrap());
}
