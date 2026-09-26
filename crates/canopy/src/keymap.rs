//! Every user-facing action and its default key. Key dispatch, the hint bar,
//! the help overlay and the command palette are all generated from this table.

use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::{AtomicBool, Ordering};

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Screen {
    Home,
    Status,
    Log,
    Branches,
    Stash,
    Workspace,
    Reflog,
    Pulls,
    Issues,
    Runs,
}

impl Screen {
    pub const ALL: [Screen; 10] = [
        Screen::Home,
        Screen::Status,
        Screen::Log,
        Screen::Branches,
        Screen::Stash,
        Screen::Workspace,
        Screen::Reflog,
        Screen::Pulls,
        Screen::Issues,
        Screen::Runs,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Screen::Home => "Home",
            Screen::Status => "Changes",
            Screen::Log => "History",
            Screen::Branches => "Branches",
            Screen::Stash => "Stash",
            Screen::Workspace => "Workspace",
            Screen::Reflog => "Reflog",
            Screen::Pulls => "Pull requests",
            Screen::Issues => "Issues",
            Screen::Runs => "Actions",
        }
    }

    /// Compact tab label for narrow terminals.
    pub fn short_title(self) -> &'static str {
        match self {
            Screen::Home => "Home",
            Screen::Status => "Changes",
            Screen::Log => "Log",
            Screen::Branches => "Refs",
            Screen::Stash => "Stash",
            Screen::Workspace => "Repos",
            Screen::Reflog => "Reflog",
            Screen::Pulls => "PRs",
            Screen::Issues => "Issues",
            Screen::Runs => "CI",
        }
    }

    pub fn index(self) -> usize {
        Screen::ALL.iter().position(|s| *s == self).unwrap_or(0)
    }
}

/// Where keyboard focus is within a screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    List,
    Diff,
    Conflict,
}

/// Which bindings apply.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Ctx {
    Global,
    Screen(Screen),
    Diff,
    /// Sub-views of the Branches tab.
    Tags,
    Remotes,
    Worktrees,
    Submodules,
    Notifications,
    Releases,
    /// The conflict panel on the Changes tab.
    Conflict,
}

/// Which list the Branches tab is showing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RefsView {
    #[default]
    Branches,
    Tags,
    Remotes,
    Worktrees,
    Submodules,
}

impl RefsView {
    pub const ALL: [RefsView; 5] =
        [RefsView::Branches, RefsView::Tags, RefsView::Remotes, RefsView::Worktrees, RefsView::Submodules];

    pub fn title(self) -> &'static str {
        match self {
            RefsView::Branches => "Branches",
            RefsView::Tags => "Tags",
            RefsView::Remotes => "Remotes",
            RefsView::Worktrees => "Worktrees",
            RefsView::Submodules => "Submodules",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    // Global
    Quit,
    Help,
    Palette,
    Goto(Screen),
    NextScreen,
    PrevScreen,
    Refresh,
    Fetch,
    Pull,
    Push,
    Commit,
    Undo,
    Redraw,
    RawGit,
    ToggleTeach,
    CycleTheme,
    StashPush,
    // Navigation
    Up,
    Down,
    PageUp,
    PageDown,
    Top,
    Bottom,
    Enter,
    Back,
    Search,
    // Status
    ToggleStage,
    StageAll,
    Discard,
    Amend,
    OpenEditor,
    Ignore,
    // Diff
    StageLine,
    StageHunk,
    RangeSelect,
    NextHunk,
    PrevHunk,
    ToggleSideBySide,
    // Log
    CheckoutCommit,
    CherryPick,
    Revert,
    ResetMenu,
    TagCommit,
    BranchFromCommit,
    FixupCommit,
    CopyHash,
    RebaseInteractive,
    // Branches
    Checkout,
    NewBranch,
    RenameBranch,
    DeleteBranch,
    Merge,
    Rebase,
    SetUpstream,
    // Stash
    StashApply,
    StashPop,
    StashDrop,
    // Workspace
    OpenRepo,
    FetchAll,
    // Reflog
    ResetToEntry,
    FileHistory,
    Blame,
    // GitHub
    PrCheckout,
    PrCreate,
    PrReview,
    Comment,
    LineComment,
    PrMerge,
    CloseItem,
    IssueCreate,
    RerunFailed,
    RunLog,
    ReleaseCreate,
    MarkRead,
    MarkAllRead,
    ToggleUnread,
    OpenInBrowser,
    CycleFilter,
    ToggleDiff,
    Bisect,
    // Conflict panel
    NextConflict,
    PrevConflict,
    KeepOurs,
    KeepTheirs,
    KeepBoth,
    RestoreConflict,
    // Branches tab sub-views
    NextRefsView,
    PrevRefsView,
    CheckoutTag,
    NewTag,
    DeleteTag,
    PushTag,
    AddRemote,
    RemoveRemote,
    RenameRemote,
    EditRemoteUrl,
    FetchRemote,
    OpenWorktree,
    NewWorktree,
    RemoveWorktree,
    PruneWorktrees,
    OpenSubmodule,
    UpdateSubmodule,
    UpdateAllSubmodules,
    // Conflicts / in-progress operations
    TakeOurs,
    TakeTheirs,
    ContinueOp,
    AbortOp,
    Custom(usize),
}

impl Action {
    pub fn label(self) -> &'static str {
        use Action::*;
        match self {
            Quit => "quit",
            Help => "help",
            Palette => "command palette",
            Goto(Screen::Home) => "go to home",
            Goto(Screen::Status) => "go to changes",
            Goto(Screen::Log) => "go to history",
            Goto(Screen::Branches) => "go to branches",
            Goto(Screen::Stash) => "go to stash",
            Goto(Screen::Workspace) => "go to workspace",
            Goto(Screen::Reflog) => "go to reflog",
            Goto(Screen::Pulls) => "go to pull requests",
            Goto(Screen::Issues) => "go to issues",
            Goto(Screen::Runs) => "go to actions",
            NextScreen => "next tab",
            PrevScreen => "previous tab",
            Refresh => "refresh",
            Fetch => "fetch",
            Pull => "pull",
            Push => "push",
            Commit => "commit",
            Undo => "undo last action",
            Redraw => "redraw the screen",
            RawGit => "run git command",
            ToggleTeach => "toggle teach mode",
            CycleTheme => "cycle theme",
            StashPush => "stash changes",
            Up => "up",
            Down => "down",
            PageUp => "page up",
            PageDown => "page down",
            Top => "top",
            Bottom => "bottom",
            Enter => "open",
            Back => "back",
            Search => "search",
            ToggleStage => "stage/unstage",
            StageAll => "stage/unstage all",
            Discard => "discard changes",
            Amend => "amend last commit",
            OpenEditor => "open in editor",
            Ignore => "add to .gitignore",
            StageLine => "stage/unstage line(s)",
            StageHunk => "stage/unstage hunk",
            RangeSelect => "select range",
            NextHunk => "next hunk",
            PrevHunk => "previous hunk",
            ToggleSideBySide => "toggle split view",
            CheckoutCommit => "checkout commit",
            CherryPick => "cherry-pick",
            Revert => "revert commit",
            ResetMenu => "reset to commit",
            TagCommit => "tag commit",
            BranchFromCommit => "new branch here",
            FixupCommit => "fixup staged into commit",
            CopyHash => "copy hash",
            RebaseInteractive => "interactive rebase from here",
            Checkout => "checkout",
            NewBranch => "new branch",
            RenameBranch => "rename branch",
            DeleteBranch => "delete branch",
            Merge => "merge into current",
            Rebase => "rebase current onto",
            SetUpstream => "set upstream",
            StashApply => "apply stash",
            StashPop => "pop stash",
            StashDrop => "drop stash",
            OpenRepo => "open repository",
            FetchAll => "fetch all repos",
            ResetToEntry => "reset to this entry",
            TakeOurs => "resolve: keep ours",
            TakeTheirs => "resolve: take theirs",
            ContinueOp => "continue merge/rebase",
            AbortOp => "abort merge/rebase",
            Bisect => "bisect: find the commit that broke something",
            FileHistory => "history of this file",
            PrCheckout => "check out this pull request",
            PrCreate => "create a pull request for this branch",
            PrReview => "review: approve / comment / request changes",
            Comment => "comment",
            LineComment => "comment on this line of a pull request",
            IssueCreate => "new issue",
            RerunFailed => "re-run failed jobs",
            RunLog => "read the whole log (failed jobs first)",
            ReleaseCreate => "create a release",
            MarkRead => "mark notification read",
            MarkAllRead => "mark all notifications read",
            ToggleUnread => "show read notifications too",
            PrMerge => "merge pull request",
            CloseItem => "close / reopen",
            OpenInBrowser => "open in browser",
            CycleFilter => "cycle filter",
            ToggleDiff => "show diff / details",
            Blame => "blame: who changed each line",
            NextConflict => "next conflict",
            PrevConflict => "previous conflict",
            KeepOurs => "keep ours for this conflict",
            KeepTheirs => "take theirs for this conflict",
            KeepBoth => "keep both (ours, then theirs)",
            RestoreConflict => "restore conflict markers",
            NextRefsView => "next view (branches/tags/remotes)",
            PrevRefsView => "previous view (branches/tags/remotes)",
            CheckoutTag => "checkout tag",
            NewTag => "new tag at HEAD",
            DeleteTag => "delete tag",
            PushTag => "push tag",
            AddRemote => "add remote",
            RemoveRemote => "remove remote",
            RenameRemote => "rename remote",
            EditRemoteUrl => "change remote URL",
            FetchRemote => "fetch this remote",
            OpenWorktree => "open worktree in Canopy",
            NewWorktree => "new worktree for a branch",
            RemoveWorktree => "remove worktree",
            PruneWorktrees => "prune missing worktrees",
            OpenSubmodule => "open submodule in Canopy",
            UpdateSubmodule => "update submodule to recorded commit",
            UpdateAllSubmodules => "update all submodules",
            Custom(_) => "custom command",
        }
    }

    /// Short label for the hint bar.
    pub fn short(self) -> &'static str {
        use Action::*;
        match self {
            ToggleStage => "stage",
            StageAll => "all",
            Discard => "discard",
            Commit => "commit",
            StageLine => "line",
            StageHunk => "hunk",
            RangeSelect => "range",
            Enter => "open",
            Back => "back",
            Push => "push",
            Pull => "pull",
            Fetch => "fetch",
            Checkout | CheckoutCommit => "checkout",
            NewBranch => "new",
            DeleteBranch => "delete",
            Merge => "merge",
            Rebase => "rebase",
            CherryPick => "pick",
            Revert => "revert",
            ResetMenu => "reset",
            StashApply => "apply",
            StashPop => "pop",
            StashDrop => "drop",
            Help => "help",
            Palette => "palette",
            Undo => "undo",
            OpenRepo => "open",
            FetchAll => "fetch all",
            ResetToEntry => "reset here",
            TakeOurs => "ours",
            TakeTheirs => "theirs",
            ContinueOp => "continue",
            AbortOp => "abort",
            Quit => "quit",
            RebaseInteractive => "rebase -i",
            NextRefsView => "switch view",
            PrCheckout => "checkout",
            PrCreate => "new",
            PrReview => "review",
            PrMerge => "merge",
            IssueCreate => "new",
            RerunFailed => "re-run",
            RunLog => "log",
            ReleaseCreate => "new release",
            MarkRead => "read",
            MarkAllRead => "all read",
            ToggleUnread => "unread/all",
            Comment => "comment",
            LineComment => "comment on line",
            CloseItem => "close",
            OpenInBrowser => "browser",
            CycleFilter => "filter",
            ToggleDiff => "diff",
            NextConflict => "next",
            KeepOurs => "ours",
            KeepTheirs => "theirs",
            KeepBoth => "both",
            RestoreConflict => "restore",
            CheckoutTag => "checkout",
            NewTag => "new",
            DeleteTag => "delete",
            PushTag => "push",
            AddRemote => "add",
            RemoveRemote => "remove",
            EditRemoteUrl => "edit url",
            FetchRemote => "fetch",
            OpenWorktree => "open",
            NewWorktree => "new",
            RemoveWorktree => "remove",
            PruneWorktrees => "prune",
            OpenSubmodule => "open",
            UpdateSubmodule => "update",
            UpdateAllSubmodules => "update all",
            ToggleSideBySide => "split",
            _ => self.label(),
        }
    }
}

pub struct Binding {
    pub keys: &'static [&'static str],
    pub action: Action,
    /// Shown in the context hint bar.
    pub hint: bool,
}

const fn b(keys: &'static [&'static str], action: Action, hint: bool) -> Binding {
    Binding { keys, action, hint }
}

pub static GLOBAL: &[Binding] = &[
    b(&["?", "alt-h", "f1"], Action::Help, true),
    b(&[":", "ctrl-p", "ctrl-k"], Action::Palette, true),
    b(&["q", "ctrl-q", "alt-q", "ctrl-c"], Action::Quit, true),
    // Number keys jump to tabs; alt-<number> does too, even on screens
    // that use digits for something else.
    b(&["1", "alt-1"], Action::Goto(Screen::Home), false),
    b(&["2", "alt-2"], Action::Goto(Screen::Status), false),
    b(&["3", "alt-3"], Action::Goto(Screen::Log), false),
    b(&["4", "alt-4"], Action::Goto(Screen::Branches), false),
    b(&["5", "alt-5"], Action::Goto(Screen::Stash), false),
    b(&["6", "alt-6"], Action::Goto(Screen::Workspace), false),
    b(&["7", "alt-7"], Action::Goto(Screen::Reflog), false),
    b(&["8", "alt-8"], Action::Goto(Screen::Pulls), false),
    b(&["9", "alt-9"], Action::Goto(Screen::Issues), false),
    b(&["0", "alt-0"], Action::Goto(Screen::Runs), false),
    b(&["tab", "alt-right", "ctrl-right"], Action::NextScreen, false),
    b(&["backtab", "alt-left", "ctrl-left"], Action::PrevScreen, false),
    b(&["ctrl-r", "alt-r"], Action::Refresh, false),
    b(&["ctrl-l"], Action::Redraw, false),
    b(&["c", "alt-c", "ctrl-s"], Action::Commit, true),
    b(&["f", "alt-f"], Action::Fetch, false),
    b(&["p", "alt-p"], Action::Pull, true),
    b(&["P", "alt-P"], Action::Push, true),
    b(&["z", "alt-z"], Action::Undo, false),
    b(&["S", "alt-s"], Action::StashPush, false),
    b(&["b"], Action::Bisect, false),
    b(&["!", "alt-g"], Action::RawGit, false),
    b(&["T"], Action::ToggleTeach, false),
    b(&["ctrl-t", "alt-t"], Action::CycleTheme, false),
    b(&["k", "up"], Action::Up, false),
    b(&["j", "down"], Action::Down, false),
    b(&["ctrl-u", "pageup"], Action::PageUp, false),
    b(&["ctrl-d", "pagedown"], Action::PageDown, false),
    b(&["g", "home"], Action::Top, false),
    b(&["G", "end"], Action::Bottom, false),
    b(&["esc"], Action::Back, false),
];

pub static STATUS: &[Binding] = &[
    b(&["space"], Action::ToggleStage, true),
    b(&["a"], Action::StageAll, true),
    b(&["d"], Action::Discard, true),
    b(&["enter", "l"], Action::Enter, true),
    b(&["A"], Action::Amend, false),
    b(&["e"], Action::OpenEditor, false),
    b(&["i"], Action::Ignore, false),
    b(&["o"], Action::TakeOurs, false),
    b(&["t"], Action::TakeTheirs, false),
    b(&["C"], Action::ContinueOp, false),
    b(&["X"], Action::AbortOp, false),
    b(&["L"], Action::FileHistory, false),
    b(&["B"], Action::Blame, false),
];

pub static DIFF: &[Binding] = &[
    b(&["space"], Action::StageLine, true),
    b(&["enter"], Action::StageHunk, true),
    b(&["v"], Action::RangeSelect, true),
    b(&["n", "]"], Action::NextHunk, false),
    b(&["N", "["], Action::PrevHunk, false),
    b(&["s"], Action::ToggleSideBySide, false),
    b(&["h", "left"], Action::Back, true),
    b(&["L"], Action::FileHistory, false),
    b(&["B"], Action::Blame, false),
    b(&["C"], Action::LineComment, false),
];

pub static LOG: &[Binding] = &[
    b(&["enter", "l"], Action::Enter, true),
    b(&["space"], Action::CheckoutCommit, true),
    b(&["C"], Action::CherryPick, true),
    b(&["t"], Action::Revert, false),
    b(&["r"], Action::ResetMenu, true),
    b(&["i"], Action::RebaseInteractive, true),
    b(&["n"], Action::BranchFromCommit, false),
    b(&["T"], Action::TagCommit, false),
    b(&["F"], Action::FixupCommit, false),
    b(&["y"], Action::CopyHash, false),
    b(&["/"], Action::Search, false),
];

pub static BRANCHES: &[Binding] = &[
    b(&["space", "enter"], Action::Checkout, true),
    b(&["n"], Action::NewBranch, true),
    b(&["d"], Action::DeleteBranch, true),
    b(&["M"], Action::Merge, true),
    b(&["r"], Action::Rebase, true),
    b(&["R"], Action::RenameBranch, false),
    b(&["u"], Action::SetUpstream, false),
    b(&["/"], Action::Search, false),
    b(&["]"], Action::NextRefsView, true),
    b(&["["], Action::PrevRefsView, false),
];

pub static STASH: &[Binding] = &[
    b(&["space"], Action::StashApply, true),
    b(&["g"], Action::StashPop, true),
    b(&["d"], Action::StashDrop, true),
    b(&["enter", "l"], Action::Enter, true),
];

pub static WORKSPACE: &[Binding] = &[
    b(&["enter", "space"], Action::OpenRepo, true),
    b(&["F"], Action::FetchAll, true),
    b(&["/"], Action::Search, false),
];

pub static REFLOG: &[Binding] = &[b(&["enter", "l"], Action::Enter, true), b(&["r"], Action::ResetToEntry, true)];

pub static HOME: &[Binding] = &[];

pub static RUNS: &[Binding] = &[
    b(&["]"], Action::NextRefsView, true),
    b(&["["], Action::PrevRefsView, false),
    b(&["enter", "l"], Action::Enter, false),
    b(&["R"], Action::RerunFailed, true),
    b(&["L"], Action::RunLog, true),
    b(&["o"], Action::OpenInBrowser, true),
    b(&["f"], Action::CycleFilter, true),
];

pub static RELEASES: &[Binding] = &[
    b(&["enter", "l"], Action::Enter, false),
    b(&["n"], Action::ReleaseCreate, true),
    b(&["o"], Action::OpenInBrowser, true),
    b(&["]"], Action::NextRefsView, true),
    b(&["["], Action::PrevRefsView, false),
];

pub static NOTIFICATIONS: &[Binding] = &[
    b(&["enter", "o"], Action::OpenInBrowser, true),
    b(&["m"], Action::MarkRead, true),
    b(&["M"], Action::MarkAllRead, true),
    b(&["f"], Action::CycleFilter, true),
    b(&["u"], Action::ToggleUnread, false),
    b(&["]"], Action::NextRefsView, true),
    b(&["["], Action::PrevRefsView, false),
];

pub static ISSUES: &[Binding] = &[
    b(&["]"], Action::NextRefsView, true),
    b(&["["], Action::PrevRefsView, false),
    b(&["enter", "l"], Action::Enter, false),
    b(&["n"], Action::IssueCreate, true),
    b(&["C"], Action::Comment, true),
    b(&["X"], Action::CloseItem, true),
    b(&["o"], Action::OpenInBrowser, true),
    b(&["f"], Action::CycleFilter, true),
];

pub static PULLS: &[Binding] = &[
    b(&["enter", "l"], Action::Enter, false),
    b(&["space"], Action::PrCheckout, true),
    b(&["n"], Action::PrCreate, true),
    b(&["r"], Action::PrReview, true),
    b(&["M"], Action::PrMerge, true),
    b(&["C"], Action::Comment, false),
    b(&["X"], Action::CloseItem, false),
    b(&["D"], Action::ToggleDiff, true),
    b(&["o"], Action::OpenInBrowser, true),
    b(&["f"], Action::CycleFilter, true),
];

pub static CONFLICT: &[Binding] = &[
    b(&["o"], Action::KeepOurs, true),
    b(&["t"], Action::KeepTheirs, true),
    b(&["b"], Action::KeepBoth, true),
    b(&["j", "down", "n"], Action::NextConflict, true),
    b(&["k", "up", "N"], Action::PrevConflict, false),
    b(&["u"], Action::RestoreConflict, false),
    b(&["e"], Action::OpenEditor, false),
    b(&["h", "left"], Action::Back, true),
];

pub static WORKTREES: &[Binding] = &[
    b(&["enter", "space"], Action::OpenWorktree, true),
    b(&["n"], Action::NewWorktree, true),
    b(&["d"], Action::RemoveWorktree, true),
    b(&["x"], Action::PruneWorktrees, false),
    b(&["]"], Action::NextRefsView, true),
    b(&["["], Action::PrevRefsView, false),
];

pub static SUBMODULES: &[Binding] = &[
    b(&["enter", "space"], Action::OpenSubmodule, true),
    b(&["u"], Action::UpdateSubmodule, true),
    b(&["U"], Action::UpdateAllSubmodules, true),
    b(&["]"], Action::NextRefsView, true),
    b(&["["], Action::PrevRefsView, false),
];

pub static TAGS: &[Binding] = &[
    b(&["space", "enter"], Action::CheckoutTag, true),
    b(&["n"], Action::NewTag, true),
    b(&["d"], Action::DeleteTag, true),
    b(&["P"], Action::PushTag, true),
    b(&["]"], Action::NextRefsView, true),
    b(&["["], Action::PrevRefsView, false),
    b(&["/"], Action::Search, false),
];

pub static REMOTES: &[Binding] = &[
    b(&["f"], Action::FetchRemote, true),
    b(&["n"], Action::AddRemote, true),
    b(&["d"], Action::RemoveRemote, true),
    b(&["e"], Action::EditRemoteUrl, true),
    b(&["R"], Action::RenameRemote, false),
    b(&["]"], Action::NextRefsView, true),
    b(&["["], Action::PrevRefsView, false),
];

pub fn defaults(ctx: Ctx) -> &'static [Binding] {
    match ctx {
        Ctx::Global => GLOBAL,
        Ctx::Diff => DIFF,
        Ctx::Screen(Screen::Home) => HOME,
        Ctx::Screen(Screen::Status) => STATUS,
        Ctx::Screen(Screen::Log) => LOG,
        Ctx::Screen(Screen::Branches) => BRANCHES,
        Ctx::Screen(Screen::Stash) => STASH,
        Ctx::Screen(Screen::Workspace) => WORKSPACE,
        Ctx::Screen(Screen::Reflog) => REFLOG,
        Ctx::Screen(Screen::Pulls) => PULLS,
        Ctx::Screen(Screen::Issues) => ISSUES,
        Ctx::Screen(Screen::Runs) => RUNS,
        Ctx::Tags => TAGS,
        Ctx::Conflict => CONFLICT,
        Ctx::Remotes => REMOTES,
        Ctx::Worktrees => WORKTREES,
        Ctx::Submodules => SUBMODULES,
        Ctx::Notifications => NOTIFICATIONS,
        Ctx::Releases => RELEASES,
    }
}

/// On a US keyboard, macOS's Option key types these characters unless the
/// terminal is set to send Option as Meta/Alt. Treat them as `alt-<key>` so
/// Option shortcuts work out of the box (dead keys like ⌥E are skipped).
pub fn mac_option_key(c: char) -> Option<char> {
    Some(match c {
        'å' => 'a',
        '∫' => 'b',
        'ç' => 'c',
        '∂' => 'd',
        'ƒ' => 'f',
        '©' => 'g',
        '˙' => 'h',
        '∆' => 'j',
        '˚' => 'k',
        '¬' => 'l',
        'µ' => 'm',
        'ø' => 'o',
        'π' => 'p',
        'œ' => 'q',
        '®' => 'r',
        'ß' => 's',
        '†' => 't',
        '√' => 'v',
        '∑' => 'w',
        '≈' => 'x',
        '¥' => 'y',
        'Ω' => 'z',
        '∏' => 'P',
        '÷' => '/',
        '¡' => '1',
        '™' => '2',
        '£' => '3',
        '¢' => '4',
        '∞' => '5',
        '§' => '6',
        '¶' => '7',
        '•' => '8',
        'ª' => '9',
        'º' => '0',
        _ => return None,
    })
}

/// Canonical string for a key event, matching the names used in the tables:
/// `x`, `X`, `ctrl-x`, `alt-x`, `ctrl-alt-x`, `enter`, `alt-left`, ...
pub fn key_name(ev: &KeyEvent) -> String {
    let mut ctrl = ev.modifiers.contains(KeyModifiers::CONTROL);
    let mut alt = ev.modifiers.contains(KeyModifiers::ALT);
    let base = match ev.code {
        KeyCode::Char(' ') => "space".to_string(),
        KeyCode::Char(c) if !ctrl && !alt && mac_option_key(c).is_some() => {
            alt = true;
            mac_option_key(c).unwrap_or(c).to_string()
        }
        // Some terminals report ctrl-letter as the control character itself.
        KeyCode::Char(c) if ('\u{1}'..='\u{1a}').contains(&c) && !matches!(c, '\t' | '\r' | '\n') => {
            ctrl = true;
            ((c as u8 - 1 + b'a') as char).to_string()
        }
        KeyCode::Char(c) => c.to_string(),
        KeyCode::Enter => "enter".into(),
        KeyCode::Esc => "esc".into(),
        KeyCode::Tab => "tab".into(),
        KeyCode::BackTab => "backtab".into(),
        KeyCode::Backspace => "backspace".into(),
        KeyCode::Up => "up".into(),
        KeyCode::Down => "down".into(),
        KeyCode::Left => "left".into(),
        KeyCode::Right => "right".into(),
        KeyCode::PageUp => "pageup".into(),
        KeyCode::PageDown => "pagedown".into(),
        KeyCode::Home => "home".into(),
        KeyCode::End => "end".into(),
        KeyCode::Delete => "delete".into(),
        KeyCode::F(n) => format!("f{n}"),
        _ => String::new(),
    };
    match (ctrl, alt) {
        (true, true) => format!("ctrl-alt-{base}"),
        (true, false) => format!("ctrl-{base}"),
        (false, true) => format!("alt-{base}"),
        (false, false) => base,
    }
}

pub const ALL_CTX: [Ctx; 19] = [
    Ctx::Global,
    Ctx::Diff,
    Ctx::Screen(Screen::Home),
    Ctx::Screen(Screen::Status),
    Ctx::Screen(Screen::Log),
    Ctx::Screen(Screen::Branches),
    Ctx::Screen(Screen::Stash),
    Ctx::Screen(Screen::Workspace),
    Ctx::Screen(Screen::Reflog),
    Ctx::Screen(Screen::Pulls),
    Ctx::Screen(Screen::Issues),
    Ctx::Screen(Screen::Runs),
    Ctx::Tags,
    Ctx::Remotes,
    Ctx::Worktrees,
    Ctx::Submodules,
    Ctx::Notifications,
    Ctx::Releases,
    Ctx::Conflict,
];

impl Action {
    /// Stable snake_case name used in the `[keys]` config table,
    /// e.g. `ToggleStage` -> `toggle_stage`, `Goto(Log)` -> `goto_history`.
    pub fn name(self) -> String {
        if let Action::Goto(s) = self {
            return format!("goto_{}", s.title().to_lowercase());
        }
        let debug = format!("{self:?}");
        let mut out = String::new();
        for (i, c) in debug.chars().enumerate() {
            if c.is_uppercase() {
                if i > 0 {
                    out.push('_');
                }
                out.extend(c.to_lowercase());
            } else {
                out.push(c);
            }
        }
        out
    }
}

/// A binding at runtime (defaults with user overrides applied).
#[derive(Debug, Clone)]
pub struct Bind {
    pub keys: Vec<String>,
    pub action: Action,
    pub hint: bool,
}

#[derive(Debug, Clone)]
pub struct Keymap {
    map: HashMap<Ctx, Vec<Bind>>,
}

impl Default for Keymap {
    fn default() -> Self {
        let map = ALL_CTX
            .iter()
            .map(|&ctx| {
                let binds = defaults(ctx)
                    .iter()
                    .map(|b| Bind {
                        keys: b.keys.iter().map(|k| k.to_string()).collect(),
                        action: b.action,
                        hint: b.hint,
                    })
                    .collect();
                (ctx, binds)
            })
            .collect();
        Keymap { map }
    }
}

impl Keymap {
    /// Apply `[keys]` overrides (action name -> keys). Returns warnings for
    /// unknown actions or keys. An override replaces the action's keys in every
    /// context it appears in, and takes those keys away from other actions there.
    pub fn with_overrides(overrides: &BTreeMap<String, Vec<String>>) -> (Keymap, Vec<String>) {
        let mut km = Keymap::default();
        let mut warnings = Vec::new();
        for (name, keys) in overrides {
            let found =
                ALL_CTX.iter().flat_map(|c| km.map[c].iter()).find(|b| b.action.name() == *name).map(|b| b.action);
            let Some(action) = found else {
                warnings.push(format!("unknown action `{name}` in [keys]"));
                continue;
            };
            let keys: Vec<String> = keys
                .iter()
                .filter(|k| {
                    let ok = is_valid_key(k);
                    if !ok {
                        warnings.push(format!("unknown key `{k}` for `{name}`"));
                    }
                    ok
                })
                .cloned()
                .collect();
            if keys.is_empty() {
                continue;
            }
            // A global action is active on every screen, so its keys must be
            // freed everywhere; otherwise only where the action lives.
            let global = km.map[&Ctx::Global].iter().any(|b| b.action == action);
            for binds in km.map.values_mut() {
                if !global && !binds.iter().any(|b| b.action == action) {
                    continue;
                }
                for b in binds.iter_mut() {
                    if b.action == action {
                        b.keys = keys.clone();
                    } else if !b.keys.is_empty() {
                        b.keys.retain(|k| !keys.contains(k));
                        if b.keys.is_empty() && !overrides.contains_key(&b.action.name()) {
                            warnings.push(format!("`{}` has no key left (given to `{name}`)", b.action.name()));
                        }
                    }
                }
            }
        }
        (km, warnings)
    }

    pub fn bindings(&self, ctx: Ctx) -> &[Bind] {
        self.map.get(&ctx).map(Vec::as_slice).unwrap_or(&[])
    }

    /// Resolve a key: the most specific context wins.
    pub fn lookup(&self, contexts: &[Ctx], key: &str) -> Option<Action> {
        contexts
            .iter()
            .flat_map(|c| self.bindings(*c).iter())
            .find(|b| b.keys.iter().any(|k| k == key))
            .map(|b| b.action)
    }

    /// First key bound to `action` in any of `contexts`, for hints.
    pub fn key_for(&self, contexts: &[Ctx], action: Action) -> Option<&str> {
        contexts
            .iter()
            .flat_map(|c| self.bindings(*c).iter())
            .find(|b| b.action == action)
            .and_then(|b| b.keys.first())
            .map(String::as_str)
    }
}

const NAMED_KEYS: &[&str] = &[
    "space",
    "enter",
    "esc",
    "tab",
    "backtab",
    "backspace",
    "up",
    "down",
    "left",
    "right",
    "pageup",
    "pagedown",
    "home",
    "end",
    "delete",
];

fn is_valid_key(k: &str) -> bool {
    let k = k.strip_prefix("ctrl-").unwrap_or(k);
    let base = k.strip_prefix("alt-").unwrap_or(k);
    base.chars().count() == 1
        || NAMED_KEYS.contains(&base)
        || base.strip_prefix('f').is_some_and(|n| n.parse::<u8>().is_ok_and(|n| (1..=12).contains(&n)))
}

/// Display form of a key for hints, e.g. `space` -> `␣`.
/// Show modifier keys as Mac symbols (⌃ ⌥ ⇧)? On by default on macOS, whose
/// terminal fonts all include them; `mac_key_symbols` in the config overrides.
static MAC_KEY_SYMBOLS: AtomicBool = AtomicBool::new(cfg!(target_os = "macos"));

pub fn set_mac_key_symbols(on: bool) {
    MAC_KEY_SYMBOLS.store(on, Ordering::Relaxed);
}

/// Display form of a key for hints and help.
pub fn pretty_key(k: &str) -> String {
    pretty_key_with(k, MAC_KEY_SYMBOLS.load(Ordering::Relaxed))
}

/// `mac`: `ctrl-q` → `⌃Q`, `alt-P` → `⌥⇧P`, `alt-left` → `⌥←`.
/// Otherwise: `ctrl-q` → `^q`, `alt-left` → `alt-←`.
pub fn pretty_key_with(k: &str, mac: bool) -> String {
    let named = |k: &str| -> Option<&'static str> {
        Some(match k {
            "space" => "space",
            "enter" => "↵",
            "up" => "↑",
            "down" => "↓",
            "left" => "←",
            "right" => "→",
            _ => return None,
        })
    };
    if k == "backtab" {
        return if mac { "⇧tab".into() } else { "shift-tab".into() };
    }
    let (ctrl, rest) = match k.strip_prefix("ctrl-") {
        Some(r) => (true, r),
        None => (false, k),
    };
    let (alt, base) = match rest.strip_prefix("alt-") {
        Some(r) => (true, r),
        None => (false, rest),
    };
    let base_shown = named(base).map(String::from).unwrap_or_else(|| base.to_string());
    if mac && (ctrl || alt) {
        let mut out = String::new();
        if ctrl {
            out.push('⌃');
        }
        if alt {
            out.push('⌥');
        }
        let mut chars = base.chars();
        match (chars.next(), chars.next()) {
            // Single letters read like macOS menus: ⌥⇧P, ⌃Q.
            (Some(c), None) if c.is_ascii_uppercase() => {
                out.push('⇧');
                out.push(c);
            }
            (Some(c), None) if c.is_ascii_lowercase() => out.push(c.to_ascii_uppercase()),
            _ => out.push_str(&base_shown),
        }
        return out;
    }
    match (ctrl, alt) {
        (true, true) => format!("^alt-{base_shown}"),
        (true, false) => format!("^{base_shown}"),
        (false, true) => format!("alt-{base_shown}"),
        (false, false) => base_shown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn screen_context_shadows_global() {
        let km = Keymap::default();
        let ctx = [Ctx::Screen(Screen::Stash), Ctx::Global];
        // `g` is "top" globally but "pop" on the stash screen.
        assert_eq!(km.lookup(&ctx, "g"), Some(Action::StashPop));
        assert_eq!(km.lookup(&[Ctx::Global], "g"), Some(Action::Top));
    }

    #[test]
    fn no_duplicate_keys_within_a_context() {
        for ctx in ALL_CTX {
            let mut seen = std::collections::HashSet::new();
            for b in defaults(ctx) {
                for k in b.keys {
                    assert!(seen.insert(*k), "duplicate key {k} in {ctx:?}");
                }
            }
        }
    }

    #[test]
    fn action_names() {
        assert_eq!(Action::ToggleStage.name(), "toggle_stage");
        assert_eq!(Action::Goto(Screen::Log).name(), "goto_history");
        assert_eq!(Action::Quit.name(), "quit");
        // Names must be unique so config entries are unambiguous.
        let mut by_name: HashMap<String, Action> = HashMap::new();
        for ctx in ALL_CTX {
            for b in defaults(ctx) {
                let prev = by_name.insert(b.action.name(), b.action);
                assert!(prev.is_none_or(|p| p == b.action), "name clash: {}", b.action.name());
            }
        }
    }

    #[test]
    fn overrides_replace_and_steal_keys() {
        let mut o = BTreeMap::new();
        o.insert("toggle_stage".to_string(), vec!["s".to_string()]);
        // `a` is stage-all by default; give it to commit instead.
        o.insert("commit".to_string(), vec!["a".to_string(), "ctrl-enter".to_string()]);
        o.insert("bogus".to_string(), vec!["x".to_string()]);
        o.insert("quit".to_string(), vec!["nope-key".to_string()]);
        let (km, warnings) = Keymap::with_overrides(&o);
        let status = [Ctx::Screen(Screen::Status), Ctx::Global];
        assert_eq!(km.lookup(&status, "s"), Some(Action::ToggleStage));
        assert_eq!(km.lookup(&status, "space"), None);
        // Global remaps win on every screen, even over a screen's own key.
        assert_eq!(km.lookup(&status, "a"), Some(Action::Commit));
        assert_eq!(km.lookup(&[Ctx::Global], "c"), None);
        // Quit keeps its defaults because the only key given was invalid.
        assert_eq!(km.lookup(&[Ctx::Global], "q"), Some(Action::Quit));
        // bogus action, bad key, and stage_all losing `a`.
        assert_eq!(warnings.len(), 3, "{warnings:?}");
    }

    #[test]
    fn modifier_key_names() {
        let k = |c, m| key_name(&KeyEvent::new(c, m));
        assert_eq!(k(KeyCode::Char('q'), KeyModifiers::ALT), "alt-q");
        assert_eq!(k(KeyCode::Char('q'), KeyModifiers::CONTROL | KeyModifiers::ALT), "ctrl-alt-q");
        assert_eq!(k(KeyCode::Left, KeyModifiers::ALT), "alt-left");
        // macOS Terminal without "Option as Meta": Option+Q types œ, Option+1 types ¡.
        assert_eq!(k(KeyCode::Char('œ'), KeyModifiers::NONE), "alt-q");
        assert_eq!(k(KeyCode::Char('¡'), KeyModifiers::NONE), "alt-1");
        assert_eq!(k(KeyCode::Char('∏'), KeyModifiers::SHIFT), "alt-P");
        // Raw control characters.
        assert_eq!(k(KeyCode::Char('\u{11}'), KeyModifiers::NONE), "ctrl-q");
        assert_eq!(pretty_key_with("ctrl-q", false), "^q");
        assert_eq!(pretty_key_with("alt-left", false), "alt-←");
        assert_eq!(pretty_key_with("ctrl-alt-x", false), "^alt-x");
        assert_eq!(pretty_key_with("ctrl-q", true), "⌃Q");
        assert_eq!(pretty_key_with("alt-P", true), "⌥⇧P");
        assert_eq!(pretty_key_with("alt-left", true), "⌥←");
        assert_eq!(pretty_key_with("alt-1", true), "⌥1");
        assert_eq!(pretty_key_with("backtab", true), "⇧tab");
        // Plain keys look the same either way.
        assert_eq!(pretty_key_with("q", true), "q");
        assert_eq!(pretty_key_with("enter", true), "↵");
        let km = Keymap::default();
        assert_eq!(km.lookup(&[Ctx::Screen(Screen::Pulls), Ctx::Global], "alt-q"), Some(Action::Quit));
        assert_eq!(km.lookup(&[Ctx::Global], "alt-3"), Some(Action::Goto(Screen::Log)));
    }

    #[test]
    fn key_names() {
        let ev = KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL);
        assert_eq!(key_name(&ev), "ctrl-p");
        let ev = KeyEvent::new(KeyCode::Char('P'), KeyModifiers::SHIFT);
        assert_eq!(key_name(&ev), "P");
    }
}
