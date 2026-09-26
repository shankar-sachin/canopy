//! Application state and the event loop.

use std::collections::HashMap;
use std::future::Future;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use canopy_git::ops::LogQuery;
use canopy_git::parse::bisect::BisectStep;
use canopy_git::{Branch, Commit, FileKind, Git, GitError, Output, ReflogEntry, Remote, RepoState, Stash, Status, Tag};
use ratatui::crossterm::event::{self, Event, KeyEvent, MouseEventKind};
use ratatui::widgets::ListState;
use ratatui::DefaultTerminal;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

use crate::config::Config;
use crate::keymap::{Focus, Keymap, RefsView, Screen};
use crate::modal::Modal;
use crate::theme::Theme;
use crate::views::conflict::ConflictView;
use crate::views::diff::DiffView;
use crate::workspace::RepoSummary;

/// Everything loaded from the repository in one refresh.
#[derive(Debug, Clone, Default)]
pub struct Snapshot {
    pub status: Status,
    pub log: Vec<Commit>,
    pub branches: Vec<Branch>,
    pub stashes: Vec<Stash>,
    pub remotes: Vec<Remote>,
    pub tags: Vec<Tag>,
    pub reflog: Vec<ReflogEntry>,
    pub state: Option<RepoState>,
    pub worktrees: Vec<canopy_git::parse::worktree::Worktree>,
    pub submodules: Vec<canopy_git::parse::submodule::Submodule>,
    /// How many commits were requested; fewer means we reached the root.
    pub log_limit: usize,
}

pub enum Msg {
    /// Next page of History, fetched starting at `skip`.
    MoreLog {
        skip: usize,
        result: Result<Vec<Commit>, String>,
    },
    Key(KeyEvent),
    Mouse(MouseEventKind),
    Resize,
    Loaded(Result<Box<Snapshot>, String>),
    StatusOnly(Status, RepoState),
    Diff {
        gen: u64,
        view: Result<DiffView, String>,
    },
    Conflict {
        gen: u64,
        view: ConflictView,
    },
    OpDone {
        label: String,
        result: Result<Output, String>,
        then: Then,
    },
    Progress(String),
    Workspace(Vec<RepoSummary>),
    RepoOpened(Result<Git, String>),
    /// Full message of HEAD, fetched to prefill the amend dialog.
    OpenAmend(String),
    ShowOutput(String, String),
    GhDetected {
        status: canopy_gh::GhStatus,
        viewer: Option<String>,
    },
    Prs(Result<Vec<canopy_gh::PullRequest>, String>),
    PrDetail {
        gen: u64,
        detail: Result<Box<canopy_gh::PullRequest>, String>,
        diff: Option<String>,
    },
    Blame(Result<crate::modal::BlameView, String>),
}

/// What to do after an operation finishes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Then {
    Refresh,
    RefreshWorkspace,
    /// A bisect step: record git's answer, then refresh.
    Bisect,
    /// Something changed on GitHub: reload the repo and the PR list.
    GitHub,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Info,
    Success,
    Warn,
    Error,
}

pub struct Toast {
    pub text: String,
    pub level: Level,
    pub at: Instant,
}

/// Decrements the in-flight task count when dropped.
pub struct InFlight(Arc<AtomicUsize>);

impl Drop for InFlight {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

/// One row in the Changes list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Section {
    Conflicts,
    Unstaged,
    Staged,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StatusRow {
    pub file: usize,
    pub section: Section,
}

pub struct App {
    pub git: Option<Git>,
    pub config: Config,
    pub theme: Theme,
    pub keymap: Keymap,
    pub data: Snapshot,
    pub loaded: bool,
    pub screen: Screen,
    pub focus: Focus,
    pub lists: HashMap<Screen, ListState>,
    pub filters: HashMap<Screen, String>,
    pub diff: Option<DiffView>,
    /// Conflict panel for the selected conflicted file (replaces the diff).
    pub conflict: Option<ConflictView>,
    pub diff_gen: u64,
    pub modal: Modal,
    pub toast: Option<Toast>,
    /// Commands run this session (teach mode shows the latest).
    pub history: Vec<String>,
    pub busy: Option<String>,
    pub progress: Option<String>,
    pub workspace: Vec<RepoSummary>,
    pub workspace_scanning: bool,
    /// True while a History page is being fetched.
    pub log_loading: bool,
    /// When set, History shows only commits touching this path (following renames).
    pub log_path: Option<String>,
    pub github: crate::github::GithubState,
    /// Inner width of the diff panel at the last draw (for wrapping text).
    pub last_diff_width: usize,
    /// Git's answer to the last bisect step, while bisecting.
    pub bisect: Option<BisectStep>,
    /// Select this commit in History once the next refresh lands.
    pub jump_after_load: Option<String>,
    /// Branches, Tags, or Remotes in the Branches tab.
    pub refs_view: RefsView,
    pub workspace_root: PathBuf,
    pub should_quit: bool,
    pub tick: u64,
    pub tx: UnboundedSender<Msg>,
    rx: Option<UnboundedReceiver<Msg>>,
    pub paused: Arc<AtomicBool>,
    /// Background tasks that haven't finished sending their results.
    inflight: Arc<AtomicUsize>,
    pub needs_redraw_full: bool,
    last_status_poll: Instant,
}

impl App {
    pub fn new(git: Option<Git>, config: Config, workspace_root: PathBuf) -> Self {
        let (tx, rx) = unbounded_channel();
        let theme = Theme::by_name(&config.theme);
        let (keymap, key_warnings) = Keymap::with_overrides(&config.key_overrides());
        let screen = if git.is_some() { Screen::Home } else { Screen::Workspace };
        let mut app = App {
            git,
            config,
            theme,
            keymap,
            data: Snapshot::default(),
            loaded: false,
            screen,
            focus: Focus::List,
            lists: HashMap::new(),
            filters: HashMap::new(),
            diff: None,
            conflict: None,
            diff_gen: 0,
            modal: Modal::None,
            toast: None,
            history: Vec::new(),
            busy: None,
            progress: None,
            workspace: Vec::new(),
            workspace_scanning: false,
            workspace_root,
            should_quit: false,
            tick: 0,
            tx,
            rx: Some(rx),
            paused: Arc::new(AtomicBool::new(false)),
            inflight: Arc::new(AtomicUsize::new(0)),
            needs_redraw_full: false,
            last_status_poll: Instant::now(),
            log_loading: false,
            log_path: None,
            github: Default::default(),
            last_diff_width: 72,
            bisect: None,
            jump_after_load: None,
            refs_view: RefsView::Branches,
        };
        if !key_warnings.is_empty() {
            app.toast(Level::Error, format!("Config [keys]: {}", key_warnings.join("; ")));
        }
        app
    }

    // ------------------------------------------------------------ helpers

    pub fn list(&mut self, s: Screen) -> &mut ListState {
        self.lists.entry(s).or_default()
    }

    pub fn selected(&self, s: Screen) -> usize {
        self.lists.get(&s).and_then(|l| l.selected()).unwrap_or(0)
    }

    pub fn set_selected(&mut self, s: Screen, i: usize) {
        self.list(s).select(Some(i));
    }

    pub fn toast(&mut self, level: Level, text: impl Into<String>) {
        self.toast = Some(Toast { text: text.into(), level, at: Instant::now() });
    }

    pub fn repo_name(&self) -> String {
        self.git
            .as_ref()
            .and_then(|g| g.repo.root.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "no repository".into())
    }

    /// Rows of the Changes screen, in display order.
    pub fn status_rows(&self) -> Vec<StatusRow> {
        let files = &self.data.status.files;
        let mut rows = Vec::new();
        for (i, f) in files.iter().enumerate() {
            if f.kind == FileKind::Conflicted {
                rows.push(StatusRow { file: i, section: Section::Conflicts });
            }
        }
        for (i, f) in files.iter().enumerate() {
            if f.kind != FileKind::Conflicted && f.is_unstaged() {
                rows.push(StatusRow { file: i, section: Section::Unstaged });
            }
        }
        for (i, f) in files.iter().enumerate() {
            if f.is_staged() {
                rows.push(StatusRow { file: i, section: Section::Staged });
            }
        }
        rows
    }

    pub fn selected_status_row(&self) -> Option<StatusRow> {
        self.status_rows().get(self.selected(Screen::Status)).copied()
    }

    /// Every whitespace-separated term must appear (case-insensitive).
    fn matches(filter: &str, hay: &[&str]) -> bool {
        let hay = hay.join(" ").to_lowercase();
        filter.to_lowercase().split_whitespace().all(|t| hay.contains(t))
    }

    /// Indexes into `data.log` after filtering.
    pub fn visible_log(&self) -> Vec<usize> {
        let f = self.filters.get(&Screen::Log).map(String::as_str).unwrap_or("");
        (0..self.data.log.len())
            .filter(|&i| {
                let c = &self.data.log[i];
                f.is_empty() || Self::matches(f, &[&c.subject, &c.author, &c.short, &c.refs.join(" ")])
            })
            .collect()
    }

    pub fn visible_branches(&self) -> Vec<usize> {
        let f = self.filters.get(&Screen::Branches).map(String::as_str).unwrap_or("");
        let mut idx: Vec<usize> = (0..self.data.branches.len())
            .filter(|&i| f.is_empty() || Self::matches(f, &[&self.data.branches[i].name]))
            .collect();
        // Local first (current branch on top), then remotes.
        idx.sort_by_key(|&i| {
            let b = &self.data.branches[i];
            (b.is_remote, !b.is_head)
        });
        idx
    }

    pub fn visible_workspace(&self) -> Vec<usize> {
        let f = self.filters.get(&Screen::Workspace).map(String::as_str).unwrap_or("");
        (0..self.workspace.len())
            .filter(|&i| f.is_empty() || Self::matches(f, &[&self.workspace[i].name, &self.workspace[i].branch]))
            .collect()
    }

    pub fn list_len(&self, s: Screen) -> usize {
        match s {
            Screen::Home => 0,
            Screen::Status => self.status_rows().len(),
            Screen::Log => self.visible_log().len(),
            Screen::Branches => match self.refs_view {
                RefsView::Branches => self.visible_branches().len(),
                RefsView::Tags => self.visible_tags().len(),
                RefsView::Remotes => self.data.remotes.len(),
                RefsView::Worktrees => self.data.worktrees.len(),
                RefsView::Submodules => self.data.submodules.len(),
            },
            Screen::Stash => self.data.stashes.len(),
            Screen::Workspace => self.visible_workspace().len(),
            Screen::Reflog => self.data.reflog.len(),
            Screen::Pulls => self.github.prs.len(),
        }
    }

    pub fn selected_commit(&self) -> Option<&Commit> {
        let vis = self.visible_log();
        vis.get(self.selected(Screen::Log)).map(|&i| &self.data.log[i])
    }

    /// Indexes into `data.tags` after filtering (shares the Branches filter).
    pub fn visible_tags(&self) -> Vec<usize> {
        let f = self.filters.get(&Screen::Branches).map(String::as_str).unwrap_or("");
        (0..self.data.tags.len())
            .filter(|&i| f.is_empty() || Self::matches(f, &[&self.data.tags[i].name, &self.data.tags[i].subject]))
            .collect()
    }

    pub fn selected_tag(&self) -> Option<&Tag> {
        if self.refs_view != RefsView::Tags {
            return None;
        }
        self.visible_tags().get(self.selected(Screen::Branches)).map(|&i| &self.data.tags[i])
    }

    pub fn selected_remote(&self) -> Option<&Remote> {
        if self.refs_view != RefsView::Remotes {
            return None;
        }
        self.data.remotes.get(self.selected(Screen::Branches))
    }

    pub fn selected_worktree(&self) -> Option<&canopy_git::parse::worktree::Worktree> {
        if self.refs_view != RefsView::Worktrees {
            return None;
        }
        self.data.worktrees.get(self.selected(Screen::Branches))
    }

    pub fn selected_submodule(&self) -> Option<&canopy_git::parse::submodule::Submodule> {
        if self.refs_view != RefsView::Submodules {
            return None;
        }
        self.data.submodules.get(self.selected(Screen::Branches))
    }

    pub fn selected_branch(&self) -> Option<&Branch> {
        if self.refs_view != RefsView::Branches {
            return None;
        }
        let vis = self.visible_branches();
        vis.get(self.selected(Screen::Branches)).map(|&i| &self.data.branches[i])
    }

    pub fn current_branch(&self) -> Option<&str> {
        self.data.status.branch.head.as_deref()
    }

    // ------------------------------------------------------------- async

    pub fn spawn<F>(&self, fut: F)
    where
        F: Future<Output = Msg> + Send + 'static,
    {
        let tx = self.tx.clone();
        let guard = self.in_flight();
        tokio::spawn(async move {
            let _ = tx.send(fut.await);
            drop(guard);
        });
    }

    /// Count a background task as running until the guard is dropped.
    pub fn in_flight(&self) -> InFlight {
        self.inflight.fetch_add(1, Ordering::SeqCst);
        InFlight(self.inflight.clone())
    }

    /// Run a git operation in the background and report the result.
    pub fn run_op<F, E>(&mut self, label: impl Into<String>, then: Then, fut: F)
    where
        F: Future<Output = Result<Output, E>> + Send + 'static,
        E: std::fmt::Display,
    {
        let label = label.into();
        self.busy = Some(label.clone());
        self.spawn(async move {
            let result = fut.await.map_err(|e| e.to_string());
            Msg::OpDone { label, result, then }
        });
    }

    pub fn refresh(&mut self) {
        let Some(git) = self.git.clone() else { return };
        // Reload as many commits as are already loaded so History keeps its place.
        let limit = self.config.log_page_size.max(self.data.log.len());
        let path = self.log_path.clone();
        self.spawn(async move {
            let q = LogQuery { limit, follow: path.is_some(), path, ..Default::default() };
            let (status, log, branches, stashes, remotes, tags, reflog, worktrees, submodules) = tokio::join!(
                git.status(),
                git.log(&q),
                git.branches(),
                git.stashes(),
                git.remotes(),
                git.tags(),
                git.reflog(200),
                git.worktrees(),
                git.submodules(),
            );
            let snap = (|| -> Result<Snapshot, GitError> {
                Ok(Snapshot {
                    status: status?,
                    log: log?,
                    branches: branches?,
                    stashes: stashes.unwrap_or_default(),
                    remotes: remotes.unwrap_or_default(),
                    tags: tags.unwrap_or_default(),
                    reflog: reflog.unwrap_or_default(),
                    worktrees: worktrees.unwrap_or_default(),
                    submodules: submodules.unwrap_or_default(),
                    state: Some(git.state()),
                    log_limit: limit,
                })
            })();
            Msg::Loaded(snap.map(Box::new).map_err(|e| e.to_string()))
        });
    }

    /// True when more history exists beyond what's loaded.
    pub fn log_has_more(&self) -> bool {
        self.data.log.len() >= self.data.log_limit && self.data.log_limit > 0
    }

    /// Fetch the next page of History if the selection is near the end.
    pub fn maybe_load_more_log(&mut self) {
        let near_end = self.selected(Screen::Log) + 50 >= self.data.log.len();
        if self.log_loading || !near_end || !self.log_has_more() || self.filters.contains_key(&Screen::Log) {
            return;
        }
        let Some(git) = self.git.clone() else { return };
        self.log_loading = true;
        let skip = self.data.log.len();
        let limit = self.config.log_page_size;
        let path = self.log_path.clone();
        self.spawn(async move {
            let q = LogQuery { limit, skip, follow: path.is_some(), path, ..Default::default() };
            Msg::MoreLog { skip, result: git.log(&q).await.map_err(|e| e.to_string()) }
        });
    }

    /// Show History for one file (or all history with `None`), reloading it.
    pub fn set_log_path(&mut self, path: Option<String>) {
        self.log_path = path;
        self.data.log.clear();
        self.data.log_limit = 0;
        self.filters.remove(&Screen::Log);
        self.set_selected(Screen::Log, 0);
        *self.list(Screen::Log).offset_mut() = 0;
        self.refresh();
    }

    fn poll_status(&mut self) {
        let Some(git) = self.git.clone() else { return };
        self.spawn(async move {
            match git.status().await {
                Ok(s) => Msg::StatusOnly(s, git.state()),
                Err(_) => Msg::Resize,
            }
        });
    }

    pub fn scan_workspace(&mut self) {
        self.workspace_scanning = true;
        let dirs = crate::workspace::roots(&self.config, &self.workspace_root);
        let depth = self.config.workspace_depth;
        self.spawn(async move { Msg::Workspace(crate::workspace::scan(dirs, depth).await) });
    }

    pub fn open_repo(&mut self, path: PathBuf) {
        self.spawn(async move { Msg::RepoOpened(Git::open(&path).await.map_err(|e| e.to_string())) });
    }

    // ------------------------------------------------------------ update

    pub fn update(&mut self, msg: Msg) {
        match msg {
            Msg::Key(k) => crate::input::handle_key(self, k),
            Msg::Mouse(kind) => crate::input::handle_mouse(self, kind),
            Msg::Resize => {}
            Msg::Loaded(Ok(snap)) => {
                let status_changed = snap.status != self.data.status;
                self.data = *snap;
                self.loaded = true;
                self.clamp_selections();
                if let Some(oid) = self.jump_after_load.take() {
                    crate::input::jump_to_commit(self, &oid);
                } else if status_changed || self.diff.is_none() || self.screen != Screen::Status {
                    crate::views::diff::load_for_selection(self);
                }
                if self.data.state != Some(RepoState::Bisecting) {
                    self.bisect = None;
                }
            }
            Msg::Loaded(Err(e)) => self.toast(Level::Error, e),
            Msg::MoreLog { skip, result } => {
                self.log_loading = false;
                match result {
                    // Ignore a page that no longer lines up (a refresh replaced the log).
                    Ok(commits) if skip == self.data.log.len() => {
                        self.data.log_limit = skip + self.config.log_page_size;
                        self.data.log.extend(commits);
                    }
                    Ok(_) => {}
                    Err(e) => self.toast(Level::Error, e),
                }
            }
            Msg::StatusOnly(status, state) => {
                let changed = status != self.data.status || Some(state) != self.data.state;
                if changed {
                    // Something changed outside Canopy: do a full reload.
                    self.refresh();
                }
            }
            Msg::Conflict { gen, mut view } => {
                if gen != self.diff_gen {
                    return;
                }
                if let Some(old) = self.conflict.as_ref().filter(|c| c.path == view.path) {
                    view.current = old.current.min(view.count().saturating_sub(1));
                }
                self.conflict = Some(view);
                self.diff = None;
            }
            Msg::Diff { gen, view } => {
                if gen != self.diff_gen {
                    return;
                }
                self.conflict = None;
                if self.focus == Focus::Conflict {
                    self.focus = Focus::List;
                }
                match view {
                    Ok(mut v) => {
                        if let Some(old) = &self.diff {
                            if old.key == v.key {
                                v.cursor = old.cursor.min(v.rows.len().saturating_sub(1));
                                v.scroll = old.scroll;
                                v.side_by_side = old.side_by_side;
                                v.snap_cursor();
                            }
                        }
                        self.diff = Some(v);
                    }
                    Err(e) => {
                        self.diff = None;
                        self.toast(Level::Error, e);
                    }
                }
                if self.diff.as_ref().is_none_or(|d| d.rows.is_empty()) && self.focus == Focus::Diff {
                    self.focus = Focus::List;
                }
            }
            Msg::OpDone { label, result, then } => {
                self.busy = None;
                self.progress = None;
                if then == Then::Bisect {
                    self.bisect = match &result {
                        Ok(out) if self.git.as_ref().is_some_and(|g| g.state() == RepoState::Bisecting) => {
                            canopy_git::parse::bisect::parse(&format!("{}{}", out.stdout, out.stderr))
                        }
                        _ => None,
                    };
                }
                match result {
                    Ok(out) => {
                        self.history.push(out.cmd.clone());
                        let detail = out
                            .stdout
                            .lines()
                            .chain(out.stderr.lines())
                            .map(str::trim)
                            .rfind(|l| !l.is_empty())
                            .unwrap_or("")
                            .to_string();
                        let text = if detail.is_empty() || detail.len() > 90 {
                            format!("✓ {label}")
                        } else {
                            format!("✓ {label} — {detail}")
                        };
                        self.toast(Level::Success, text);
                    }
                    Err(e) => {
                        let msg = e.lines().filter(|l| !l.trim().is_empty()).collect::<Vec<_>>();
                        let short = msg.iter().rev().find(|l| !l.starts_with("hint:")).unwrap_or(&"failed");
                        self.toast(Level::Error, format!("✗ {label}: {}", short.trim()));
                        if msg.len() > 2 {
                            self.modal = Modal::output(format!("{label} failed"), e);
                        }
                    }
                }
                match then {
                    Then::Refresh => self.refresh(),
                    Then::RefreshWorkspace => self.scan_workspace(),
                    Then::GitHub => {
                        self.refresh();
                        crate::github::load_prs(self);
                    }
                    Then::Bisect => {
                        self.refresh();
                        if let Some(BisectStep::Found { oid, subject }) = self.bisect.clone() {
                            self.modal = crate::input::bisect_found_menu(&oid, &subject);
                        }
                    }
                }
            }
            Msg::Progress(line) => self.progress = Some(line),
            Msg::Workspace(repos) => {
                self.workspace = repos;
                self.workspace_scanning = false;
                self.clamp_selections();
            }
            Msg::RepoOpened(Ok(git)) => {
                self.github = Default::default();
                self.toast(Level::Info, format!("Opened {}", git.repo.root.display()));
                self.git = Some(git);
                self.data = Snapshot::default();
                self.loaded = false;
                self.diff = None;
                self.lists.clear();
                self.filters.remove(&Screen::Log);
                self.filters.remove(&Screen::Branches);
                self.screen = Screen::Home;
                self.focus = Focus::List;
                self.refresh();
                crate::github::detect(self);
            }
            Msg::RepoOpened(Err(e)) => self.toast(Level::Error, e),
            Msg::Blame(Ok(view)) => {
                self.busy = None;
                if view.blame.lines.is_empty() {
                    self.toast(Level::Info, format!("{} is empty", view.path));
                } else if !self.modal.is_open() || matches!(self.modal, Modal::Blame(_)) {
                    self.modal = Modal::Blame(view);
                }
            }
            Msg::Blame(Err(e)) => {
                self.busy = None;
                self.toast(Level::Error, e);
            }
            Msg::GhDetected { status, viewer } => {
                self.github.status = Some(status);
                self.github.viewer = viewer;
                crate::github::on_enter(self, self.screen);
            }
            Msg::Prs(result) => {
                self.github.prs_loading = false;
                match result {
                    Ok(prs) => {
                        self.github.prs = prs;
                        self.github.prs_loaded = true;
                        self.clamp_selections();
                        if self.screen == Screen::Pulls {
                            crate::views::diff::load_for_selection(self);
                        }
                    }
                    Err(e) => self.toast(Level::Error, e),
                }
            }
            Msg::PrDetail { gen, detail, diff } => {
                if gen != self.diff_gen || self.screen != Screen::Pulls {
                    return;
                }
                match detail {
                    Ok(pr) => {
                        let mut v = crate::github::pr_view(&pr, diff.as_deref(), self.last_diff_width);
                        if let Some(old) = self.diff.as_ref().filter(|o| o.key == v.key) {
                            v.cursor = old.cursor.min(v.rows.len().saturating_sub(1));
                        }
                        self.github.pr_diff = diff.map(|d| (pr.number, d));
                        self.github.pr_detail = Some(*pr);
                        self.diff = Some(v);
                    }
                    Err(e) => self.toast(Level::Error, e),
                }
            }
            Msg::ShowOutput(title, text) => {
                self.progress = None;
                if !self.modal.is_open() {
                    self.modal = Modal::output(title, text);
                }
            }
            Msg::OpenAmend(message) => {
                let (subject, body) = match message.split_once('\n') {
                    Some((s, b)) => (s.to_string(), b.trim_start_matches('\n').to_string()),
                    None => (message.clone(), String::new()),
                };
                self.modal = Modal::Commit {
                    subject: crate::textarea::TextArea::single(&subject),
                    body: crate::textarea::TextArea::multi(body.trim_end()),
                    on_body: false,
                    amend: true,
                };
            }
        }
    }

    pub fn clamp_selections(&mut self) {
        for s in Screen::ALL {
            let len = self.list_len(s);
            let sel = self.selected(s);
            let st = self.list(s);
            if len == 0 {
                st.select(None);
            } else {
                st.select(Some(sel.min(len - 1)));
            }
        }
    }

    // --------------------------------------------------------- main loop

    pub async fn run(mut self, terminal: &mut DefaultTerminal) -> anyhow::Result<()> {
        let mut rx = self.rx.take().expect("run called once");
        spawn_input_thread(self.tx.clone(), self.paused.clone());
        if self.git.is_some() {
            self.refresh();
            crate::github::detect(&mut self);
        }
        self.scan_workspace();

        let mut ticker = tokio::time::interval(Duration::from_millis(100));
        loop {
            if self.needs_redraw_full {
                terminal.clear()?;
                self.needs_redraw_full = false;
            }
            terminal.draw(|f| crate::ui::draw(f, &mut self))?;
            tokio::select! {
                Some(msg) = rx.recv() => {
                    self.update(msg);
                    // Drain anything else queued before redrawing.
                    while let Ok(m) = rx.try_recv() {
                        self.update(m);
                    }
                }
                _ = ticker.tick() => {
                    self.tick = self.tick.wrapping_add(1);
                    if let Some(t) = &self.toast {
                        let ttl = if t.level == Level::Error { 8 } else { 4 };
                        if t.at.elapsed() > Duration::from_secs(ttl) {
                            self.toast = None;
                        }
                    }
                    if self.busy.is_none()
                        && matches!(self.modal, Modal::None)
                        && self.last_status_poll.elapsed() > Duration::from_secs(3)
                    {
                        self.last_status_poll = Instant::now();
                        self.poll_status();
                    }
                }
            }
            if self.should_quit {
                break;
            }
        }
        Ok(())
    }

    /// Process queued messages until nothing arrives for a short while.
    #[cfg(test)]
    pub async fn settle(&mut self) {
        let mut rx = self.rx.take().expect("receiver");
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            match tokio::time::timeout(Duration::from_millis(20), rx.recv()).await {
                Ok(Some(msg)) => self.update(msg),
                // Idle: done only once no background task is still running.
                _ if self.inflight.load(Ordering::SeqCst) == 0 => break,
                _ => assert!(Instant::now() < deadline, "background work never finished"),
            }
        }
        self.rx = Some(rx);
    }

    /// Suspend the TUI, run an interactive program, then restore.
    pub fn suspend<R>(&mut self, f: impl FnOnce() -> R) -> R {
        self.paused.store(true, Ordering::SeqCst);
        // Give the input thread a moment to stop polling.
        std::thread::sleep(Duration::from_millis(60));
        crate::terminal::restore();
        let r = f();
        crate::terminal::enter();
        self.paused.store(false, Ordering::SeqCst);
        self.needs_redraw_full = true;
        r
    }
}

fn spawn_input_thread(tx: UnboundedSender<Msg>, paused: Arc<AtomicBool>) {
    std::thread::spawn(move || loop {
        if paused.load(Ordering::SeqCst) {
            std::thread::sleep(Duration::from_millis(30));
            continue;
        }
        if !event::poll(Duration::from_millis(50)).unwrap_or(false) {
            continue;
        }
        let msg = match event::read() {
            Ok(Event::Key(k)) if k.kind == event::KeyEventKind::Press => Msg::Key(k),
            Ok(Event::Mouse(m)) => Msg::Mouse(m.kind),
            Ok(Event::Resize(..)) => Msg::Resize,
            _ => continue,
        };
        if tx.send(msg).is_err() {
            break;
        }
    });
}
