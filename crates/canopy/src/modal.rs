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
}

#[derive(Debug, Clone)]
pub enum InputKind {
    NewBranch { start: Option<String> },
    RenameBranch(String),
    Tag(String),
    RawGit,
    Search(Screen),
    StashMessage,
    SetUpstream,
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
pub fn palette_actions(keymap: &crate::keymap::Keymap, screen: Screen) -> Vec<(Action, String)> {
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
    push(Ctx::Screen(screen));
    push(Ctx::Global);
    out
}
