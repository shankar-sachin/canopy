//! Types deserialized from `gh ... --json`.

use serde::{Deserialize, Deserializer};

use crate::time::parse_rfc3339;

fn ts<'de, D: Deserializer<'de>>(d: D) -> Result<i64, D::Error> {
    let s = Option::<String>::deserialize(d)?;
    Ok(s.as_deref().and_then(parse_rfc3339).unwrap_or(0))
}

fn null_default<'de, D: Deserializer<'de>, T: Default + Deserialize<'de>>(d: D) -> Result<T, D::Error> {
    Ok(Option::<T>::deserialize(d)?.unwrap_or_default())
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoInfo {
    pub name_with_owner: String,
    pub url: String,
    #[serde(default)]
    pub default_branch_ref: Option<BranchRef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct BranchRef {
    pub name: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct Author {
    #[serde(default)]
    pub login: String,
    #[serde(default, deserialize_with = "null_default")]
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Label {
    pub name: String,
    #[serde(default)]
    pub color: String,
}

/// One entry of `statusCheckRollup`: either a check run (Actions etc.) or a
/// legacy commit status. Both shapes are folded into this struct.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Check {
    /// Check runs have `name`; statuses have `context`.
    #[serde(default, alias = "context")]
    pub name: String,
    #[serde(default, deserialize_with = "null_default")]
    pub workflow_name: String,
    /// QUEUED / IN_PROGRESS / COMPLETED (check runs).
    #[serde(default, deserialize_with = "null_default")]
    pub status: String,
    /// SUCCESS / FAILURE / NEUTRAL / CANCELLED / SKIPPED / TIMED_OUT / ... (check runs).
    #[serde(default, deserialize_with = "null_default")]
    pub conclusion: String,
    /// SUCCESS / FAILURE / ERROR / PENDING / EXPECTED (statuses).
    #[serde(default, deserialize_with = "null_default")]
    pub state: String,
    #[serde(default, alias = "targetUrl", deserialize_with = "null_default")]
    pub details_url: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckState {
    Passed,
    Failed,
    Pending,
    /// Neutral or skipped: doesn't count either way.
    Neutral,
}

impl Check {
    pub fn state(&self) -> CheckState {
        let c = if self.conclusion.is_empty() { &self.state } else { &self.conclusion };
        match c.as_str() {
            "SUCCESS" => CheckState::Passed,
            "FAILURE" | "ERROR" | "TIMED_OUT" | "CANCELLED" | "ACTION_REQUIRED" | "STARTUP_FAILURE" => {
                CheckState::Failed
            }
            "NEUTRAL" | "SKIPPED" | "STALE" => CheckState::Neutral,
            _ => CheckState::Pending,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ChecksSummary {
    pub passed: usize,
    pub failed: usize,
    pub pending: usize,
    pub total: usize,
}

impl ChecksSummary {
    pub fn of(checks: &[Check]) -> Self {
        let mut s = ChecksSummary { total: checks.len(), ..Default::default() };
        for c in checks {
            match c.state() {
                CheckState::Passed => s.passed += 1,
                CheckState::Failed => s.failed += 1,
                CheckState::Pending => s.pending += 1,
                CheckState::Neutral => {}
            }
        }
        s
    }

    /// Overall: failed wins, then pending, then passed.
    pub fn overall(&self) -> Option<CheckState> {
        if self.total == 0 {
            None
        } else if self.failed > 0 {
            Some(CheckState::Failed)
        } else if self.pending > 0 {
            Some(CheckState::Pending)
        } else {
            Some(CheckState::Passed)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Review {
    #[serde(default)]
    pub author: Author,
    /// APPROVED / CHANGES_REQUESTED / COMMENTED / DISMISSED / PENDING
    pub state: String,
    #[serde(default, deserialize_with = "null_default")]
    pub body: String,
    #[serde(default, deserialize_with = "ts")]
    pub submitted_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Comment {
    #[serde(default)]
    pub author: Author,
    #[serde(default, deserialize_with = "null_default")]
    pub body: String,
    #[serde(default, deserialize_with = "ts")]
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PullRequest {
    pub number: u64,
    pub title: String,
    #[serde(default)]
    pub author: Author,
    #[serde(default)]
    pub head_ref_name: String,
    #[serde(default)]
    pub base_ref_name: String,
    #[serde(default)]
    pub is_draft: bool,
    /// OPEN / CLOSED / MERGED
    pub state: String,
    pub url: String,
    #[serde(default, deserialize_with = "ts")]
    pub updated_at: i64,
    /// APPROVED / CHANGES_REQUESTED / REVIEW_REQUIRED / "" (none)
    #[serde(default, deserialize_with = "null_default")]
    pub review_decision: String,
    #[serde(default, deserialize_with = "null_default")]
    pub status_check_rollup: Vec<Check>,
    #[serde(default, deserialize_with = "null_default")]
    pub labels: Vec<Label>,
    #[serde(default)]
    pub additions: u64,
    #[serde(default)]
    pub deletions: u64,
    // Only present from `pr view`:
    #[serde(default, deserialize_with = "null_default")]
    pub body: String,
    #[serde(default, deserialize_with = "null_default")]
    pub reviews: Vec<Review>,
    #[serde(default, deserialize_with = "null_default")]
    pub comments: Vec<Comment>,
    /// MERGEABLE / CONFLICTING / UNKNOWN
    #[serde(default, deserialize_with = "null_default")]
    pub mergeable: String,
}

impl PullRequest {
    pub fn checks(&self) -> ChecksSummary {
        ChecksSummary::of(&self.status_check_rollup)
    }
}

/// `comments` is a list in `issue view` but only needed as a count in lists;
/// gh returns the full list either way.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Issue {
    pub number: u64,
    pub title: String,
    #[serde(default)]
    pub author: Author,
    /// OPEN / CLOSED
    pub state: String,
    #[serde(default, deserialize_with = "null_default")]
    pub labels: Vec<Label>,
    pub url: String,
    #[serde(default, deserialize_with = "ts")]
    pub updated_at: i64,
    #[serde(default, deserialize_with = "null_default")]
    pub comments: Vec<Comment>,
    #[serde(default, deserialize_with = "null_default")]
    pub body: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Run {
    pub database_id: u64,
    #[serde(default)]
    pub number: u64,
    #[serde(default)]
    pub display_title: String,
    #[serde(default)]
    pub workflow_name: String,
    #[serde(default)]
    pub head_branch: String,
    /// queued / in_progress / completed / ...
    #[serde(default)]
    pub status: String,
    /// success / failure / cancelled / skipped / "" while running
    #[serde(default, deserialize_with = "null_default")]
    pub conclusion: String,
    #[serde(default)]
    pub event: String,
    #[serde(default, deserialize_with = "ts")]
    pub created_at: i64,
    pub url: String,
}

impl Run {
    pub fn state(&self) -> CheckState {
        match (self.status.as_str(), self.conclusion.as_str()) {
            ("completed", "success") => CheckState::Passed,
            ("completed", "skipped" | "neutral") => CheckState::Neutral,
            ("completed", _) => CheckState::Failed,
            _ => CheckState::Pending,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Job {
    #[serde(default)]
    pub database_id: u64,
    pub name: String,
    #[serde(default)]
    pub status: String,
    #[serde(default, deserialize_with = "null_default")]
    pub conclusion: String,
    #[serde(default, deserialize_with = "null_default")]
    pub steps: Vec<Step>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Step {
    pub name: String,
    #[serde(default)]
    pub status: String,
    #[serde(default, deserialize_with = "null_default")]
    pub conclusion: String,
    #[serde(default)]
    pub number: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Release {
    pub tag_name: String,
    #[serde(default, deserialize_with = "null_default")]
    pub name: String,
    #[serde(default, deserialize_with = "ts")]
    pub published_at: i64,
    #[serde(default, deserialize_with = "ts")]
    pub created_at: i64,
    #[serde(default)]
    pub is_latest: bool,
    #[serde(default)]
    pub is_draft: bool,
    #[serde(default)]
    pub is_prerelease: bool,
    // Only from `release view`:
    #[serde(default, deserialize_with = "null_default")]
    pub body: String,
    #[serde(default, deserialize_with = "null_default")]
    pub url: String,
    #[serde(default)]
    pub author: Author,
    #[serde(default, deserialize_with = "null_default")]
    pub assets: Vec<ReleaseAsset>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseAsset {
    pub name: String,
    #[serde(default)]
    pub size: u64,
    #[serde(default)]
    pub download_count: u64,
}

/// A GitHub notification thread (REST API shape, snake_case).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Notification {
    pub id: String,
    #[serde(default)]
    pub unread: bool,
    /// review_requested, mention, assign, author, comment, ...
    #[serde(default)]
    pub reason: String,
    #[serde(default, deserialize_with = "ts")]
    pub updated_at: i64,
    pub subject: NotificationSubject,
    pub repository: NotificationRepo,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct NotificationSubject {
    pub title: String,
    /// API URL of the thing (PR, issue, release...), may be null.
    #[serde(default, deserialize_with = "null_default")]
    pub url: String,
    /// PullRequest, Issue, Release, Commit, Discussion, CheckSuite, ...
    #[serde(rename = "type", default)]
    pub kind: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct NotificationRepo {
    pub full_name: String,
    pub html_url: String,
}

impl Notification {
    /// The page to open in a browser: the PR or issue itself when possible.
    pub fn web_url(&self) -> String {
        let api = &self.subject.url;
        if let Some(rest) = api.strip_prefix("https://api.github.com/repos/") {
            let web = format!("https://github.com/{rest}");
            if self.subject.kind == "PullRequest" {
                return web.replacen("/pulls/", "/pull/", 1);
            }
            if self.subject.kind == "Issue" {
                return web;
            }
        }
        self.repository.html_url.clone()
    }

    /// Why you got it, in plain words.
    pub fn reason_text(&self) -> &'static str {
        match self.reason.as_str() {
            "review_requested" => "your review was requested",
            "mention" => "you were mentioned",
            "team_mention" => "your team was mentioned",
            "assign" => "you were assigned",
            "author" => "you opened it",
            "comment" => "you commented",
            "state_change" => "you changed its state",
            "subscribed" => "you watch this repository",
            "manual" => "you subscribed",
            "ci_activity" => "a workflow run finished",
            "security_alert" => "a security alert",
            "invitation" => "you were invited",
            _ => "activity",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrFilter {
    Open,
    Mine,
    ReviewRequested,
    All,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IssueFilter {
    Open,
    Mine,
    All,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewKind {
    Approve,
    Comment,
    RequestChanges,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergeMethod {
    Merge,
    Squash,
    Rebase,
}
