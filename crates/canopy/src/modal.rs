//! Overlay dialogs.

use canopy_git::ops::ResetMode;
use canopy_git::Commit;

use crate::keymap::{Action, Screen};
use crate::textarea::TextArea;

/// A deferred operation, captured with all the data it needs so it stays
/// correct even if the selection changes while a dialog is open.
#[derive(Debug, Clone)]
pub enum Pending {
    Discard(Vec<(String, bool)>),
    DeleteBranch {
        name: String,
        remote: bool,
        force: bool,
    },
    StashDrop(String),
    Reset {
        rev: String,
        mode: ResetMode,
    },
    UndoCheckout(String),
    /// Move HEAD back; `soft` keeps the undone commit's changes staged.
    UndoReset {
        oid: String,
        soft: bool,
    },
    AbortOp,
    Checkout(String),
    Push {
        remote: String,
        branch: String,
        set_upstream: bool,
        force: bool,
    },
    Pull {
        rebase: bool,
    },
    Custom(usize),
    DeleteTag {
        name: String,
        remote: Option<String>,
    },
    RemoveRemote(String),
    RemoveWorktree {
        path: String,
        force: bool,
    },
    BisectStart {
        good: String,
    },
    /// `good`, `bad`, or `skip`.
    BisectMark(&'static str),
    BisectReset,
    /// Stop bisecting, then show this commit in History.
    BisectFinish {
        oid: String,
    },
    PrMerge {
        number: u64,
        method: canopy_gh::MergeMethod,
    },
    Close(crate::github::Target),
    Reopen(crate::github::Target),
    /// Open the compose dialog for a review of this kind.
    PrReview(u64, canopy_gh::ReviewKind),
}

#[derive(Debug, Clone)]
pub enum InputKind {
    NewBranch {
        start: Option<String>,
    },
    RenameBranch(String),
    Tag(String),
    RawGit,
    Search(Screen),
    StashMessage,
    SetUpstream,
    NewTag,
    /// "name url"
    AddRemote,
    RenameRemote(String),
    EditRemoteUrl(String),
    /// Branch name; the folder is derived from it.
    NewWorktree,
    /// Tag for a new GitHub release.
    ReleaseTag,
}

#[derive(Debug, Clone)]
pub struct MenuItem {
    pub key: char,
    pub label: String,
    pub detail: String,
    pub pending: Pending,
    pub danger: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TodoAction {
    Pick,
    Edit,
    Squash,
    Fixup,
    Drop,
}

impl TodoAction {
    pub fn word(self) -> &'static str {
        match self {
            TodoAction::Pick => "pick",
            TodoAction::Edit => "edit",
            TodoAction::Squash => "squash",
            TodoAction::Fixup => "fixup",
            TodoAction::Drop => "drop",
        }
    }
}

#[derive(Debug, Clone)]
pub struct RebaseItem {
    pub action: TodoAction,
    pub commit: Commit,
}

pub enum Modal {
    None,
    Help {
        scroll: u16,
    },
    Confirm {
        title: String,
        lines: Vec<String>,
        pending: Pending,
        danger: bool,
    },
    Input {
        title: String,
        hint: String,
        input: TextArea,
        kind: InputKind,
    },
    Commit {
        subject: TextArea,
        body: TextArea,
        on_body: bool,
        amend: bool,
    },
    Menu {
        title: String,
        items: Vec<MenuItem>,
        sel: usize,
    },
    Palette {
        input: TextArea,
        sel: usize,
    },
    Output {
        title: String,
        lines: Vec<String>,
        scroll: u16,
    },
    /// Oldest commit first, matching git's todo list order.
    Rebase {
        base: String,
        items: Vec<RebaseItem>,
        sel: usize,
    },
    Welcome,
    Blame(BlameView),
    /// A CI run's whole log, with search.
    RunLog(crate::views::runlog::LogView),
    /// Write a title and/or body for something on GitHub.
    Compose(Compose),
}

#[derive(Debug, Clone)]
pub enum ComposeFor {
    NewPullRequest {
        base: String,
    },
    NewIssue,
    NewRelease {
        tag: String,
    },
    Comment(crate::github::Target),
    Review(u64, canopy_gh::ReviewKind),
    /// A review comment on one line of a pull request's diff.
    LineComment {
        number: u64,
        commit: String,
        path: String,
        line: u32,
        side: String,
    },
}

pub struct Compose {
    pub heading: String,
    /// `None` for body-only messages (comments, reviews).
    pub title: Option<TextArea>,
    pub body: TextArea,
    pub on_body: bool,
    pub purpose: ComposeFor,
}

impl Compose {
    pub fn new(heading: impl Into<String>, with_title: bool, purpose: ComposeFor) -> Self {
        Compose {
            heading: heading.into(),
            title: with_title.then(|| TextArea::single("")),
            body: TextArea::multi(""),
            on_body: !with_title,
            purpose,
        }
    }
}

pub struct BlameView {
    pub path: String,
    /// `None` = working tree.
    pub rev: Option<String>,
    pub blame: canopy_git::parse::blame::Blame,
    pub cursor: usize,
}

impl Modal {
    pub fn output(title: impl Into<String>, text: impl AsRef<str>) -> Modal {
        Modal::Output { title: title.into(), lines: text.as_ref().lines().map(String::from).collect(), scroll: 0 }
    }

    pub fn input(title: impl Into<String>, hint: impl Into<String>, initial: &str, kind: InputKind) -> Modal {
        Modal::Input { title: title.into(), hint: hint.into(), input: TextArea::single(initial), kind }
    }

    pub fn is_open(&self) -> bool {
        !matches!(self, Modal::None)
    }
}

/// Entries listed in the command palette.
pub fn palette_actions(keymap: &crate::keymap::Keymap, screen_ctx: crate::keymap::Ctx) -> Vec<(Action, String)> {
    use crate::keymap::Ctx;
    let mut out: Vec<(Action, String)> = Vec::new();
    let mut push = |ctx: Ctx| {
        for b in keymap.bindings(ctx) {
            if matches!(
                b.action,
                Action::Up
                    | Action::Down
                    | Action::PageUp
                    | Action::PageDown
                    | Action::Top
                    | Action::Bottom
                    | Action::Back
                    | Action::Palette
            ) {
                continue;
            }
            if !out.iter().any(|(a, _)| *a == b.action) {
                out.push((b.action, b.keys.first().map(|k| crate::keymap::pretty_key(k)).unwrap_or_default()));
            }
        }
    };
    push(screen_ctx);
    push(Ctx::Global);
    out
}
