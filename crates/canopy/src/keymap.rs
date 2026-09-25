//! Every user-facing action and its default key. Key dispatch, the hint bar,
//! the help overlay and the command palette are all generated from this table.

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
}

impl Screen {
    pub const ALL: [Screen; 7] =
        [Screen::Home, Screen::Status, Screen::Log, Screen::Branches, Screen::Stash, Screen::Workspace, Screen::Reflog];

    pub fn title(self) -> &'static str {
        match self {
            Screen::Home => "Home",
            Screen::Status => "Changes",
            Screen::Log => "History",
            Screen::Branches => "Branches",
            Screen::Stash => "Stash",
            Screen::Workspace => "Workspace",
            Screen::Reflog => "Reflog",
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
}

/// Which bindings apply.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ctx {
    Global,
    Screen(Screen),
    Diff,
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
            NextScreen => "next tab",
            PrevScreen => "previous tab",
            Refresh => "refresh",
            Fetch => "fetch",
            Pull => "pull",
            Push => "push",
            Commit => "commit",
            Undo => "undo last action",
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
    b(&["?"], Action::Help, true),
    b(&[":", "ctrl-p"], Action::Palette, true),
    b(&["q", "ctrl-c"], Action::Quit, false),
    b(&["1"], Action::Goto(Screen::Home), false),
    b(&["2"], Action::Goto(Screen::Status), false),
    b(&["3"], Action::Goto(Screen::Log), false),
    b(&["4"], Action::Goto(Screen::Branches), false),
    b(&["5"], Action::Goto(Screen::Stash), false),
    b(&["6"], Action::Goto(Screen::Workspace), false),
    b(&["7"], Action::Goto(Screen::Reflog), false),
    b(&["tab"], Action::NextScreen, false),
    b(&["backtab"], Action::PrevScreen, false),
    b(&["ctrl-r"], Action::Refresh, false),
    b(&["c"], Action::Commit, true),
    b(&["f"], Action::Fetch, false),
    b(&["p"], Action::Pull, true),
    b(&["P"], Action::Push, true),
    b(&["z"], Action::Undo, false),
    b(&["S"], Action::StashPush, false),
    b(&["!"], Action::RawGit, false),
    b(&["T"], Action::ToggleTeach, false),
    b(&["ctrl-t"], Action::CycleTheme, false),
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
];

pub static DIFF: &[Binding] = &[
    b(&["space"], Action::StageLine, true),
    b(&["enter"], Action::StageHunk, true),
    b(&["v"], Action::RangeSelect, true),
    b(&["n", "]"], Action::NextHunk, false),
    b(&["N", "["], Action::PrevHunk, false),
    b(&["s"], Action::ToggleSideBySide, false),
    b(&["h", "left"], Action::Back, true),
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

pub fn bindings(ctx: Ctx) -> &'static [Binding] {
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
    }
}

/// Canonical string for a key event, matching the names used in the tables.
pub fn key_name(ev: &KeyEvent) -> String {
    let ctrl = ev.modifiers.contains(KeyModifiers::CONTROL);
    let base = match ev.code {
        KeyCode::Char(' ') => "space".to_string(),
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
    if ctrl {
        format!("ctrl-{base}")
    } else {
        base
    }
}

/// Resolve a key: the most specific context wins.
pub fn lookup(contexts: &[Ctx], key: &str) -> Option<Action> {
    contexts.iter().flat_map(|c| bindings(*c).iter()).find(|b| b.keys.contains(&key)).map(|b| b.action)
}

/// Display form of a key for hints, e.g. `space` -> `␣`.
pub fn pretty_key(k: &str) -> String {
    match k {
        "space" => "space".into(),
        "enter" => "⏎".into(),
        "backtab" => "⇧tab".into(),
        "up" => "↑".into(),
        "down" => "↓".into(),
        "left" => "←".into(),
        "right" => "→".into(),
        _ => match k.strip_prefix("ctrl-") {
            Some(rest) => format!("^{rest}"),
            None => k.to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn screen_context_shadows_global() {
        let ctx = [Ctx::Screen(Screen::Stash), Ctx::Global];
        // `g` is "top" globally but "pop" on the stash screen.
        assert_eq!(lookup(&ctx, "g"), Some(Action::StashPop));
        assert_eq!(lookup(&[Ctx::Global], "g"), Some(Action::Top));
    }

    #[test]
    fn no_duplicate_keys_within_a_context() {
        for ctx in [
            Ctx::Global,
            Ctx::Diff,
            Ctx::Screen(Screen::Status),
            Ctx::Screen(Screen::Log),
            Ctx::Screen(Screen::Branches),
            Ctx::Screen(Screen::Stash),
            Ctx::Screen(Screen::Workspace),
            Ctx::Screen(Screen::Reflog),
        ] {
            let mut seen = std::collections::HashSet::new();
            for b in bindings(ctx) {
                for k in b.keys {
                    assert!(seen.insert(*k), "duplicate key {k} in {ctx:?}");
                }
            }
        }
    }

    #[test]
    fn key_names() {
        let ev = KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL);
        assert_eq!(key_name(&ev), "ctrl-p");
        let ev = KeyEvent::new(KeyCode::Char('P'), KeyModifiers::SHIFT);
        assert_eq!(key_name(&ev), "P");
    }
}
