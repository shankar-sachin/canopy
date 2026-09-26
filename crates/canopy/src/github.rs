//! GitHub tabs: state, loading, and turning PRs into the details panel.

use canopy_gh::{Gh, GhStatus, Issue, IssueFilter, PrFilter, PullRequest};

use crate::app::{App, Msg};
use crate::keymap::Screen;
use crate::ui::util::ago;
use crate::views::diff::DiffView;

#[derive(Default)]
pub struct GithubState {
    /// `None` until detection finishes.
    pub status: Option<GhStatus>,
    pub gh: Option<Gh>,
    pub viewer: Option<String>,
    pub prs: Vec<PullRequest>,
    pub pr_filter: PrFilterChoice,
    pub prs_loading: bool,
    pub prs_loaded: bool,
    /// Full details of the selected PR (body, reviews, comments).
    pub pr_detail: Option<PullRequest>,
    /// Show the PR's diff under its details.
    pub show_pr_diff: bool,
    pub pr_diff: Option<(u64, String)>,
    pub issues: Vec<Issue>,
    pub issue_filter: IssueFilterChoice,
    pub issues_loading: bool,
    pub issues_loaded: bool,
}

/// A GitHub item that can be commented on or closed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    Pr(u64),
    Issue(u64),
}

impl Target {
    pub fn label(self) -> String {
        match self {
            Target::Pr(n) | Target::Issue(n) => format!("#{n}"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IssueFilterChoice(pub IssueFilter);

impl Default for IssueFilterChoice {
    fn default() -> Self {
        IssueFilterChoice(IssueFilter::Open)
    }
}

impl IssueFilterChoice {
    pub fn label(self) -> &'static str {
        match self.0 {
            IssueFilter::Open => "open",
            IssueFilter::Mine => "assigned to me",
            IssueFilter::All => "all",
        }
    }

    pub fn next(self) -> Self {
        IssueFilterChoice(match self.0 {
            IssueFilter::Open => IssueFilter::Mine,
            IssueFilter::Mine => IssueFilter::All,
            IssueFilter::All => IssueFilter::Open,
        })
    }
}

/// Wrapper so the filter has a Default and a label.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PrFilterChoice(pub PrFilter);

impl Default for PrFilterChoice {
    fn default() -> Self {
        PrFilterChoice(PrFilter::Open)
    }
}

impl PrFilterChoice {
    pub fn label(self) -> &'static str {
        match self.0 {
            PrFilter::Open => "open",
            PrFilter::Mine => "mine",
            PrFilter::ReviewRequested => "review requested",
            PrFilter::All => "all",
        }
    }

    pub fn next(self) -> Self {
        PrFilterChoice(match self.0 {
            PrFilter::Open => PrFilter::Mine,
            PrFilter::Mine => PrFilter::ReviewRequested,
            PrFilter::ReviewRequested => PrFilter::All,
            PrFilter::All => PrFilter::Open,
        })
    }
}

impl GithubState {
    pub fn ready(&self) -> bool {
        matches!(self.status, Some(GhStatus::Ready(_)))
    }

    pub fn repo_name(&self) -> Option<&str> {
        match &self.status {
            Some(GhStatus::Ready(info)) => Some(&info.name_with_owner),
            _ => None,
        }
    }
}

/// Check whether gh works for the open repo (runs once per repo).
pub fn detect(app: &mut App) {
    let Some(git) = app.git.as_ref() else { return };
    let gh = Gh::new(&git.repo.root, app.config.gh_program.clone().map(Into::into));
    app.github = GithubState { gh: Some(gh.clone()), ..Default::default() };
    app.spawn(async move {
        let status = gh.detect().await;
        let viewer = if matches!(status, GhStatus::Ready(_)) { gh.viewer().await.ok() } else { None };
        Msg::GhDetected { status, viewer }
    });
}

pub fn load_prs(app: &mut App) {
    if !app.github.ready() {
        return;
    }
    let Some(gh) = app.github.gh.clone() else { return };
    let filter = app.github.pr_filter.0;
    app.github.prs_loading = true;
    app.spawn(async move { Msg::Prs(gh.pr_list(filter, 50).await.map_err(|e| e.to_string())) });
}

pub fn load_issues(app: &mut App) {
    if !app.github.ready() {
        return;
    }
    let Some(gh) = app.github.gh.clone() else { return };
    let filter = app.github.issue_filter.0;
    app.github.issues_loading = true;
    app.spawn(async move { Msg::Issues(gh.issue_list(filter, 50).await.map_err(|e| e.to_string())) });
}

/// Reload whichever GitHub lists have been opened (after an action).
pub fn reload_loaded(app: &mut App) {
    if app.github.prs_loaded {
        load_prs(app);
    }
    if app.github.issues_loaded {
        load_issues(app);
    }
}

/// Called when a GitHub tab becomes visible: load it the first time.
pub fn on_enter(app: &mut App, screen: Screen) {
    if !app.github.ready() {
        return;
    }
    let gh = &app.github;
    match screen {
        Screen::Pulls if !gh.prs_loaded && !gh.prs_loading => load_prs(app),
        Screen::Issues if !gh.issues_loaded && !gh.issues_loading => load_issues(app),
        _ => {}
    }
}

pub fn selected_issue(app: &App) -> Option<&Issue> {
    app.github.issues.get(app.selected(Screen::Issues))
}

pub fn load_issue_detail(app: &mut App, gen: u64) {
    let (Some(gh), Some(issue)) = (app.github.gh.clone(), selected_issue(app).cloned()) else {
        app.diff = None;
        return;
    };
    app.diff = Some(issue_view(&issue, app.last_diff_width));
    app.spawn(async move {
        let detail = gh.issue_view(issue.number).await.map(Box::new).map_err(|e| e.to_string());
        Msg::IssueDetail { gen, detail }
    });
}

pub fn issue_view(issue: &Issue, width: usize) -> DiffView {
    let mut meta = vec![
        format!("#{} {}", issue.number, issue.title),
        format!(
            "{} · opened by {} · updated {} · {} comment(s)",
            issue.state.to_lowercase(),
            issue.author.login,
            ago(issue.updated_at),
            issue.comments.len()
        ),
    ];
    if !issue.labels.is_empty() {
        meta.push(format!("labels: {}", issue.labels.iter().map(|l| l.name.as_str()).collect::<Vec<_>>().join(", ")));
    }
    meta.push(String::new());
    if issue.body.trim().is_empty() {
        meta.push("(no description)".into());
    } else {
        meta.extend(wrap(issue.body.trim(), width));
    }
    for c in &issue.comments {
        meta.push(String::new());
        meta.push(format!("── {} commented · {}", c.author.login, ago(c.created_at)));
        meta.extend(wrap(c.body.trim(), width));
    }
    DiffView::new(
        format!("issue:{}", issue.number),
        format!("#{} {}", issue.number, issue.title),
        meta,
        Vec::new(),
        None,
    )
}

pub fn selected_pr(app: &App) -> Option<&PullRequest> {
    app.github.prs.get(app.selected(Screen::Pulls))
}

/// Fetch details (and the diff if toggled) for the selected PR.
pub fn load_pr_detail(app: &mut App, gen: u64) {
    let (Some(gh), Some(pr)) = (app.github.gh.clone(), selected_pr(app).cloned()) else {
        app.diff = None;
        return;
    };
    // Show what we already know right away, then fill in details.
    let width = app.last_diff_width;
    app.diff = Some(pr_view(&pr, None, width));
    let want_diff = app.github.show_pr_diff;
    app.spawn(async move {
        let detail = gh.pr_view(pr.number).await.map(Box::new).map_err(|e| e.to_string());
        let diff = if want_diff { gh.pr_diff(pr.number).await.ok() } else { None };
        Msg::PrDetail { gen, detail, diff }
    });
}

/// Word-wrap `text` to `width` columns (paragraphs and indentation kept).
pub fn wrap(text: &str, width: usize) -> Vec<String> {
    let width = width.max(20);
    let mut out = Vec::new();
    for para in text.lines() {
        let indent: String = para.chars().take_while(|c| c.is_whitespace()).collect();
        let words: Vec<&str> = para.split_whitespace().collect();
        if words.is_empty() {
            out.push(String::new());
            continue;
        }
        let mut line = indent.clone();
        for w in words {
            let len = line.chars().count();
            if len > indent.len() && len + 1 + w.chars().count() > width {
                out.push(std::mem::replace(&mut line, indent.clone()));
            }
            if line.chars().count() > indent.len() {
                line.push(' ');
            }
            line.push_str(w);
        }
        out.push(line);
    }
    out
}

/// Build the details panel for a PR: header, checks, reviews, body,
/// comments, and optionally the diff.
pub fn pr_view(pr: &PullRequest, diff: Option<&str>, width: usize) -> DiffView {
    use canopy_gh::CheckState;
    let mut meta = vec![
        format!("#{} {}", pr.number, pr.title),
        format!("{} wants to merge {} into {}", pr.author.login, pr.head_ref_name, pr.base_ref_name),
        format!(
            "{}{} · +{} -{} · updated {}",
            pr.state.to_lowercase(),
            if pr.is_draft { " (draft)" } else { "" },
            pr.additions,
            pr.deletions,
            ago(pr.updated_at)
        ),
    ];
    if !pr.labels.is_empty() {
        meta.push(format!("labels: {}", pr.labels.iter().map(|l| l.name.as_str()).collect::<Vec<_>>().join(", ")));
    }
    let decision = match pr.review_decision.as_str() {
        "APPROVED" => "✓ approved",
        "CHANGES_REQUESTED" => "✗ changes requested",
        "REVIEW_REQUIRED" => "review required",
        _ => "",
    };
    if !decision.is_empty() {
        meta.push(format!("review: {decision}"));
    }
    match pr.mergeable.as_str() {
        "CONFLICTING" => meta.push("⚠ has merge conflicts with the base branch".into()),
        "MERGEABLE" => meta.push("✓ no conflicts with the base branch".into()),
        _ => {}
    }
    if !pr.status_check_rollup.is_empty() {
        let c = pr.checks();
        meta.push(String::new());
        meta.push(format!("Checks: {} passed, {} failed, {} pending", c.passed, c.failed, c.pending));
        for ch in &pr.status_check_rollup {
            let icon = match ch.state() {
                CheckState::Passed => "✓",
                CheckState::Failed => "✗",
                CheckState::Pending => "…",
                CheckState::Neutral => "-",
            };
            meta.push(format!("  {icon} {}", ch.name));
        }
    }
    meta.push(String::new());
    if pr.body.trim().is_empty() {
        meta.push("(no description)".into());
    } else {
        meta.extend(wrap(pr.body.trim(), width));
    }
    for r in pr.reviews.iter().filter(|r| r.state != "PENDING") {
        meta.push(String::new());
        let what = match r.state.as_str() {
            "APPROVED" => "approved",
            "CHANGES_REQUESTED" => "requested changes",
            "COMMENTED" => "reviewed",
            other => other,
        };
        meta.push(format!("── {} {what} · {}", r.author.login, ago(r.submitted_at)));
        if !r.body.trim().is_empty() {
            meta.extend(wrap(r.body.trim(), width));
        }
    }
    for c in &pr.comments {
        meta.push(String::new());
        meta.push(format!("── {} commented · {}", c.author.login, ago(c.created_at)));
        meta.extend(wrap(c.body.trim(), width));
    }
    let files = diff.map(canopy_git::parse::diff::parse).unwrap_or_default();
    let key = format!("pr:{}:{}", pr.number, diff.is_some());
    DiffView::new(key, format!("#{} {}", pr.number, pr.title), meta, files, None)
}

#[cfg(test)]
mod tests {
    use super::wrap;

    #[test]
    fn wraps_words_and_keeps_blank_lines() {
        let w = wrap("one two three four five six seven\n\n  indented words go here too", 20);
        assert_eq!(w, vec!["one two three four", "five six seven", "", "  indented words go", "  here too"]);
        // A single long word stays on its own line rather than looping.
        assert_eq!(wrap(&"x".repeat(50), 20), vec!["x".repeat(50)]);
    }
}
