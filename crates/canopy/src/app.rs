//! Application state and the event loop.

use std::collections::HashMap;
use std::future::Future;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use canopy_git::ops::LogQuery;
use canopy_git::{Branch, Commit, FileKind, Git, GitError, Output, ReflogEntry, Remote, RepoState, Stash, Status, Tag};
use ratatui::crossterm::event::{self, Event, KeyEvent, MouseEventKind};
use ratatui::widgets::ListState;
use ratatui::DefaultTerminal;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

use crate::config::Config;
use crate::keymap::{Focus, Screen};
use crate::modal::Modal;
use crate::theme::Theme;
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
}

pub enum Msg {
    Key(KeyEvent),
    Mouse(MouseEventKind),
    Resize,
    Loaded(Result<Box<Snapshot>, String>),
    StatusOnly(Status, RepoState),
    Diff {
        gen: u64,
        view: Result<DiffView, String>,
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
}

/// What to do after an operation finishes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Then {
    Refresh,
    RefreshWorkspace,
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
    pub data: Snapshot,
    pub loaded: bool,
    pub screen: Screen,
    pub focus: Focus,
    pub lists: HashMap<Screen, ListState>,
    pub filters: HashMap<Screen, String>,
    pub diff: Option<DiffView>,
    pub diff_gen: u64,
    pub modal: Modal,
    pub toast: Option<Toast>,
    /// Commands run this session (teach mode shows the latest).
    pub history: Vec<String>,
    pub busy: Option<String>,
    pub progress: Option<String>,
    pub workspace: Vec<RepoSummary>,
    pub workspace_scanning: bool,
    pub workspace_root: PathBuf,
    pub should_quit: bool,
    pub tick: u64,
    pub tx: UnboundedSender<Msg>,
    rx: Option<UnboundedReceiver<Msg>>,
    pub paused: Arc<AtomicBool>,
    pub needs_redraw_full: bool,
    last_status_poll: Instant,
}

impl App {
    pub fn new(git: Option<Git>, config: Config, workspace_root: PathBuf) -> Self {
        let (tx, rx) = unbounded_channel();
        let theme = Theme::by_name(&config.theme);
        let screen = if git.is_some() { Screen::Home } else { Screen::Workspace };
        App {
            git,
            config,
            theme,
            data: Snapshot::default(),
            loaded: false,
            screen,
            focus: Focus::List,
            lists: HashMap::new(),
            filters: HashMap::new(),
            diff: None,
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
            needs_redraw_full: false,
            last_status_poll: Instant::now(),
        }
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
            Screen::Branches => self.visible_branches().len(),
            Screen::Stash => self.data.stashes.len(),
            Screen::Workspace => self.visible_workspace().len(),
            Screen::Reflog => self.data.reflog.len(),
        }
    }

    pub fn selected_commit(&self) -> Option<&Commit> {
        let vis = self.visible_log();
        vis.get(self.selected(Screen::Log)).map(|&i| &self.data.log[i])
    }

    pub fn selected_branch(&self) -> Option<&Branch> {
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
        tokio::spawn(async move {
            let _ = tx.send(fut.await);
        });
    }

    /// Run a git operation in the background and report the result.
    pub fn run_op<F>(&mut self, label: impl Into<String>, then: Then, fut: F)
    where
        F: Future<Output = Result<Output, GitError>> + Send + 'static,
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
        let page = self.config.log_page_size;
        self.spawn(async move {
            let q = LogQuery { limit: page, ..Default::default() };
            let (status, log, branches, stashes, remotes, tags, reflog) = tokio::join!(
                git.status(),
                git.log(&q),
                git.branches(),
                git.stashes(),
                git.remotes(),
                git.tags(),
                git.reflog(200),
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
                    state: Some(git.state()),
                })
            })();
            Msg::Loaded(snap.map(Box::new).map_err(|e| e.to_string()))
        });
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
                if status_changed || self.diff.is_none() || self.screen != Screen::Status {
                    crate::views::diff::load_for_selection(self);
                }
            }
            Msg::Loaded(Err(e)) => self.toast(Level::Error, e),
            Msg::StatusOnly(status, state) => {
                let changed = status != self.data.status || Some(state) != self.data.state;
                if changed {
                    // Something changed outside Canopy: do a full reload.
                    self.refresh();
                }
            }
            Msg::Diff { gen, view } => {
                if gen != self.diff_gen {
                    return;
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
                }
            }
            Msg::Progress(line) => self.progress = Some(line),
            Msg::Workspace(repos) => {
                self.workspace = repos;
                self.workspace_scanning = false;
                self.clamp_selections();
            }
            Msg::RepoOpened(Ok(git)) => {
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
            }
            Msg::RepoOpened(Err(e)) => self.toast(Level::Error, e),
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
        while let Ok(Some(msg)) = tokio::time::timeout(Duration::from_millis(400), rx.recv()).await {
            self.update(msg);
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
