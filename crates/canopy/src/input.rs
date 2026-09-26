//! Key handling: dispatch to actions, modal dialogs, and pending operations.

use canopy_git::ops::{CommitOpts, ResetMode};
use canopy_git::parse::conflict::Choice;
use canopy_git::{FileKind, Output, RepoState};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEventKind};
use tokio::sync::mpsc;

use crate::app::{App, Level, Msg, Section, Then};
use crate::keymap::{self, Action, Ctx, Focus, RefsView, Screen};
use crate::modal::{palette_actions, InputKind, MenuItem, Modal, Pending, RebaseItem, TodoAction};
use crate::textarea::TextArea;
use crate::theme::Theme;
use crate::views::diff;

/// The key context for the current screen (and Branches sub-view).
pub fn screen_ctx(app: &App) -> Ctx {
    match (app.screen, app.refs_view) {
        (Screen::Branches, RefsView::Tags) => Ctx::Tags,
        (Screen::Branches, RefsView::Remotes) => Ctx::Remotes,
        (s, _) => Ctx::Screen(s),
    }
}

pub fn contexts(app: &App) -> Vec<Ctx> {
    if app.focus == Focus::Diff && app.diff.is_some() {
        vec![Ctx::Diff, Ctx::Global]
    } else if app.focus == Focus::Conflict && app.conflict.is_some() {
        vec![Ctx::Conflict, Ctx::Global]
    } else {
        vec![screen_ctx(app), Ctx::Global]
    }
}

pub fn handle_key(app: &mut App, key: KeyEvent) {
    if app.modal.is_open() {
        modal_key(app, key);
        return;
    }
    let name = keymap::key_name(&key);
    if let Some(i) = app.config.custom_commands.iter().position(|c| c.key == name) {
        do_action(app, Action::Custom(i));
        return;
    }
    if let Some(action) = app.keymap.lookup(&contexts(app), &name) {
        do_action(app, action);
    }
}

pub fn handle_mouse(app: &mut App, kind: MouseEventKind) {
    if app.modal.is_open() {
        return;
    }
    match kind {
        MouseEventKind::ScrollUp => do_action(app, Action::Up),
        MouseEventKind::ScrollDown => do_action(app, Action::Down),
        _ => {}
    }
}

fn progress_sender(app: &App) -> mpsc::UnboundedSender<String> {
    let (ptx, mut prx) = mpsc::unbounded_channel::<String>();
    let tx = app.tx.clone();
    tokio::spawn(async move {
        while let Some(line) = prx.recv().await {
            let _ = tx.send(Msg::Progress(line));
        }
    });
    ptx
}

fn move_list(app: &mut App, delta: isize) {
    let s = app.screen;
    let len = app.list_len(s);
    if len == 0 {
        return;
    }
    let cur = app.selected(s) as isize;
    let next = (cur + delta).clamp(0, len as isize - 1) as usize;
    if next != cur as usize || app.lists.get(&s).and_then(|l| l.selected()).is_none() {
        app.set_selected(s, next);
        diff::load_for_selection(app);
    }
    if s == Screen::Log {
        app.maybe_load_more_log();
    }
}

fn navigate(app: &mut App, delta: isize) {
    if app.focus == Focus::Diff {
        if let Some(d) = app.diff.as_mut() {
            d.move_cursor(delta);
        }
    } else if app.focus == Focus::Conflict {
        if let Some(c) = app.conflict.as_mut() {
            c.step(delta.signum());
        }
    } else {
        move_list(app, delta);
    }
}

fn goto(app: &mut App, s: Screen) {
    app.screen = s;
    app.focus = Focus::List;
    if s == Screen::Workspace && app.workspace.is_empty() && !app.workspace_scanning {
        app.scan_workspace();
    }
    app.clamp_selections();
    if app.lists.get(&s).and_then(|l| l.selected()).is_none() && app.list_len(s) > 0 {
        app.set_selected(s, 0);
    }
    diff::load_for_selection(app);
}

fn needs_repo(app: &mut App) -> bool {
    if app.git.is_none() {
        app.toast(Level::Warn, "Open a repository first (Workspace, tab 6)");
        return true;
    }
    false
}

fn confirm(app: &mut App, title: &str, lines: Vec<String>, pending: Pending, danger: bool) {
    if !app.config.confirm_destructive && danger {
        execute(app, pending);
        return;
    }
    app.modal = Modal::Confirm { title: title.into(), lines, pending, danger };
}

pub fn do_action(app: &mut App, action: Action) {
    use Action::*;
    match action {
        Quit => app.should_quit = true,
        Help => app.modal = Modal::Help { scroll: 0 },
        Palette => app.modal = Modal::Palette { input: TextArea::single(""), sel: 0 },
        Goto(s) => goto(app, s),
        NextScreen | PrevScreen => {
            let n = Screen::ALL.len();
            let i = app.screen.index();
            let next = if action == NextScreen { (i + 1) % n } else { (i + n - 1) % n };
            goto(app, Screen::ALL[next]);
        }
        Refresh => {
            app.refresh();
            if app.screen == Screen::Workspace {
                app.scan_workspace();
            }
            app.toast(Level::Info, "Refreshed");
        }
        Up => navigate(app, -1),
        Down => navigate(app, 1),
        PageUp => navigate(app, -15),
        PageDown => navigate(app, 15),
        Top => navigate(app, -1_000_000),
        Bottom => navigate(app, 1_000_000),
        Back => {
            if let Some(d) = app.diff.as_mut().filter(|d| d.anchor.is_some()) {
                d.anchor = None;
            } else if app.focus != Focus::List {
                app.focus = Focus::List;
            } else if app.filters.remove(&app.screen).is_some() {
                app.clamp_selections();
                diff::load_for_selection(app);
            }
        }
        Enter => match app.screen {
            Screen::Workspace => do_action(app, OpenRepo),
            Screen::Home => {}
            _ => {
                let on_conflict = app.selected_status_row().is_some_and(|r| r.section == Section::Conflicts);
                if app.screen == Screen::Status && on_conflict && app.conflict.is_some() {
                    app.focus = Focus::Conflict;
                } else if app.diff.as_ref().is_some_and(|d| !d.rows.is_empty()) {
                    app.focus = Focus::Diff;
                }
            }
        },
        Search => {
            let cur = app.filters.get(&app.screen).cloned().unwrap_or_default();
            app.modal =
                Modal::input("Filter", "type to filter · enter keep · esc clear", &cur, InputKind::Search(app.screen));
        }
        ToggleTeach => {
            app.config.teach_mode = !app.config.teach_mode;
            let s = if app.config.teach_mode { "on — the git command behind each action is shown" } else { "off" };
            app.toast(Level::Info, format!("Teach mode {s}"));
        }
        CycleTheme => {
            let i = Theme::NAMES.iter().position(|n| *n == app.theme.name).unwrap_or(0);
            app.theme = Theme::by_name(Theme::NAMES[(i + 1) % Theme::NAMES.len()]);
            app.toast(Level::Info, format!("Theme: {}", app.theme.name));
        }
        RawGit => {
            if needs_repo(app) {
                return;
            }
            app.modal =
                Modal::input("Run git command", "e.g. log --oneline -5 · runs in the repo root", "", InputKind::RawGit);
        }
        OpenRepo => {
            let vis = app.visible_workspace();
            if let Some(&i) = vis.get(app.selected(Screen::Workspace)) {
                let path = app.workspace[i].path.clone();
                app.open_repo(path);
            }
        }
        FetchAll => {
            let paths: Vec<_> = app.workspace.iter().map(|r| r.path.clone()).collect();
            if paths.is_empty() {
                return;
            }
            let n = paths.len();
            app.run_op(format!("Fetch {n} repos"), Then::RefreshWorkspace, async move {
                let mut set = tokio::task::JoinSet::new();
                for p in paths {
                    set.spawn(async move {
                        let g = canopy_git::Git::open(&p).await?;
                        g.run(&["fetch", "--all", "--prune", "--quiet"]).await
                    });
                }
                let mut failed = 0;
                while let Some(r) = set.join_next().await {
                    if !matches!(r, Ok(Ok(_))) {
                        failed += 1;
                    }
                }
                Ok(Output {
                    cmd: "git fetch --all --prune  # in every repo".into(),
                    stdout: if failed > 0 { format!("{failed} failed") } else { String::new() },
                    stderr: String::new(),
                })
            });
        }
        _ => {
            if needs_repo(app) {
                return;
            }
            repo_action(app, action);
        }
    }
}

fn repo_action(app: &mut App, action: Action) {
    use Action::*;
    let git = app.git.clone().expect("checked by caller");
    match action {
        Fetch => {
            let p = progress_sender(app);
            app.run_op("Fetch", Then::Refresh, async move { git.fetch(None, p).await });
        }
        Pull => {
            if app.data.status.branch.upstream.is_none() {
                app.toast(Level::Warn, "This branch has no upstream to pull from — push it first (P)");
                return;
            }
            let p = progress_sender(app);
            app.run_op("Pull", Then::Refresh, async move { git.run_streaming(&["pull", "--progress"], p).await });
        }
        Push => push(app),
        Commit => {
            let staged = app.data.status.staged().count();
            let unstaged = app.data.status.unstaged().count();
            if staged == 0 && unstaged == 0 {
                app.toast(Level::Info, "Nothing to commit — working tree clean");
                return;
            }
            app.modal = Modal::Commit {
                subject: TextArea::single(""),
                body: TextArea::multi(""),
                on_body: false,
                amend: false,
            };
        }
        Amend => {
            if app.data.log.is_empty() {
                app.toast(Level::Warn, "No commits to amend yet");
                return;
            }
            app.spawn(async move {
                match git.run(&["log", "-1", "--format=%B"]).await {
                    Ok(o) => Msg::OpenAmend(o.stdout.trim_end().to_string()),
                    Err(_) => Msg::OpenAmend(String::new()),
                }
            });
        }
        StashPush => {
            if app.data.status.is_clean() {
                app.toast(Level::Info, "Nothing to stash");
                return;
            }
            app.modal = Modal::input(
                "Stash changes",
                "optional message · includes untracked files",
                "",
                InputKind::StashMessage,
            );
        }
        Undo => undo(app),
        ToggleStage => toggle_stage(app),
        StageAll => {
            let has_unstaged = app.data.status.unstaged().count() > 0;
            if has_unstaged {
                app.run_op("Stage all", Then::Refresh, async move { git.stage_all().await });
            } else {
                app.run_op("Unstage all", Then::Refresh, async move { git.unstage_all().await });
            }
        }
        Discard => {
            let Some(row) = app.selected_status_row() else { return };
            let f = app.data.status.files[row.file].clone();
            if row.section != Section::Unstaged {
                app.toast(Level::Info, "Unstage the file first (space), then discard");
                return;
            }
            let untracked = f.kind == FileKind::Untracked;
            let what = if untracked { "delete this untracked file" } else { "throw away your unstaged edits to" };
            confirm(
                app,
                "Discard changes?",
                vec![
                    format!("This will {what}:"),
                    format!("  {}", f.path),
                    String::new(),
                    "This cannot be undone.".into(),
                ],
                Pending::Discard(vec![(f.path, untracked)]),
                true,
            );
        }
        OpenEditor => {
            let path = match app.screen {
                Screen::Status => app.selected_status_row().map(|r| app.data.status.files[r.file].path.clone()),
                _ => None,
            };
            let Some(path) = path else { return };
            let full = git.repo.root.join(path);
            let editor = std::env::var("VISUAL").or_else(|_| std::env::var("EDITOR")).unwrap_or_else(|_| "vi".into());
            let status = app.suspend(|| {
                std::process::Command::new("sh").arg("-c").arg(format!("{editor} \"$1\"")).arg("sh").arg(&full).status()
            });
            if let Err(e) = status {
                app.toast(Level::Error, format!("Could not run {editor}: {e}"));
            }
            app.refresh();
        }
        Ignore => {
            let Some(row) = app.selected_status_row() else { return };
            let path = app.data.status.files[row.file].path.clone();
            let gi = git.repo.root.join(".gitignore");
            let mut text = std::fs::read_to_string(&gi).unwrap_or_default();
            if !text.is_empty() && !text.ends_with('\n') {
                text.push('\n');
            }
            text.push_str(&format!("/{path}\n"));
            match std::fs::write(&gi, text) {
                Ok(_) => {
                    app.history.push(format!("echo '/{path}' >> .gitignore"));
                    app.toast(Level::Success, format!("Ignored {path}"));
                    app.refresh();
                }
                Err(e) => app.toast(Level::Error, e.to_string()),
            }
        }
        StageLine => diff::apply_selection(app, false),
        StageHunk => diff::apply_selection(app, true),
        RangeSelect => {
            if let Some(d) = app.diff.as_mut() {
                d.anchor = if d.anchor.is_some() { None } else { Some(d.cursor) };
            }
        }
        NextHunk | PrevHunk => {
            if let Some(d) = app.diff.as_mut() {
                d.jump_hunk(action == NextHunk);
            }
        }
        ToggleSideBySide => {
            if let Some(d) = app.diff.as_mut() {
                d.side_by_side = !d.side_by_side;
            }
        }
        CheckoutCommit => {
            let Some(c) = app.selected_commit().cloned() else { return };
            // Prefer checking out a local branch that points here.
            if let Some(b) = app.data.branches.iter().find(|b| !b.is_remote && b.oid == c.oid && !b.is_head) {
                let name = b.name.clone();
                app.run_op(format!("Checkout {name}"), Then::Refresh, async move { git.checkout(&name).await });
                return;
            }
            confirm(
                app,
                "Check out this commit?",
                vec![
                    format!("{} {}", c.short, c.subject),
                    String::new(),
                    "You'll be in 'detached HEAD' state: you can look around and".into(),
                    "experiment, but new commits won't belong to any branch unless".into(),
                    "you create one (n in History). Check out a branch to return.".into(),
                ],
                Pending::Checkout(c.oid),
                false,
            );
        }
        CherryPick => {
            let Some(c) = app.selected_commit().cloned() else { return };
            app.run_op(
                format!("Cherry-pick {}", c.short),
                Then::Refresh,
                async move { git.cherry_pick(&[&c.oid]).await },
            );
        }
        Revert => {
            let Some(c) = app.selected_commit().cloned() else { return };
            app.run_op(format!("Revert {}", c.short), Then::Refresh, async move { git.revert(&c.oid).await });
        }
        ResetMenu => {
            let target = match app.screen {
                Screen::Reflog => {
                    app.data.reflog.get(app.selected(Screen::Reflog)).map(|r| (r.oid.clone(), r.selector.clone()))
                }
                _ => app.selected_commit().map(|c| (c.oid.clone(), c.short.clone())),
            };
            let Some((oid, name)) = target else { return };
            reset_menu(app, oid, name);
        }
        ResetToEntry => do_action_reset_reflog(app),
        TagCommit => {
            let Some(c) = app.selected_commit() else { return };
            let oid = c.oid.clone();
            app.modal = Modal::input(
                format!("Tag {}", &oid[..7.min(oid.len())]),
                "tag name, e.g. v1.0.0",
                "",
                InputKind::Tag(oid),
            );
        }
        BranchFromCommit => {
            let Some(c) = app.selected_commit() else { return };
            let oid = c.oid.clone();
            app.modal = Modal::input(
                format!("New branch at {}", &oid[..7.min(oid.len())]),
                "branch name · switches to it",
                "",
                InputKind::NewBranch { start: Some(oid) },
            );
        }
        FixupCommit => {
            let Some(c) = app.selected_commit().cloned() else { return };
            if app.data.status.staged().count() == 0 {
                app.toast(Level::Warn, "Stage the changes you want to fold into this commit first");
                return;
            }
            app.run_op(format!("Fixup into {}", c.short), Then::Refresh, async move { git.commit_fixup(&c.oid).await });
        }
        CopyHash => {
            let Some(c) = app.selected_commit() else { return };
            let oid = c.oid.clone();
            if crate::terminal::copy_to_clipboard(&oid) {
                app.toast(Level::Success, format!("Copied {oid}"));
            }
        }
        RebaseInteractive => open_rebase(app),
        Checkout => {
            let Some(b) = app.selected_branch().cloned() else { return };
            if b.is_head {
                app.toast(Level::Info, format!("Already on {}", b.name));
                return;
            }
            if b.is_remote {
                let short = b.name.split_once('/').map(|x| x.1).unwrap_or(&b.name).to_string();
                if app.data.branches.iter().any(|x| !x.is_remote && x.name == short) {
                    app.run_op(format!("Checkout {short}"), Then::Refresh, async move { git.checkout(&short).await });
                } else {
                    let name = b.name.clone();
                    app.run_op(format!("Checkout {name} as {short}"), Then::Refresh, async move {
                        git.checkout_remote(&name).await
                    });
                }
            } else {
                app.run_op(format!("Checkout {}", b.name), Then::Refresh, async move { git.checkout(&b.name).await });
            }
        }
        NewBranch => {
            app.modal = Modal::input(
                "New branch",
                "name · created from HEAD and checked out",
                "",
                InputKind::NewBranch { start: None },
            );
        }
        RenameBranch => {
            let Some(b) = app.selected_branch().filter(|b| !b.is_remote) else { return };
            let name = b.name.clone();
            app.modal =
                Modal::input(format!("Rename {name}"), "new name", &name, InputKind::RenameBranch(name.clone()));
        }
        DeleteBranch => {
            let Some(b) = app.selected_branch().cloned() else { return };
            if b.is_head {
                app.toast(Level::Warn, "Can't delete the branch you're on — check out another first");
                return;
            }
            if b.is_remote {
                confirm(
                    app,
                    "Delete remote branch?",
                    vec![format!("This deletes {} on the server for everyone.", b.name)],
                    Pending::DeleteBranch { name: b.name, remote: true, force: false },
                    true,
                );
            } else {
                let items = vec![
                    MenuItem {
                        key: 'd',
                        label: "Delete".into(),
                        detail: "only if fully merged (safe)".into(),
                        pending: Pending::DeleteBranch { name: b.name.clone(), remote: false, force: false },
                        danger: false,
                    },
                    MenuItem {
                        key: 'D',
                        label: "Force delete".into(),
                        detail: "even with unmerged commits".into(),
                        pending: Pending::DeleteBranch { name: b.name.clone(), remote: false, force: true },
                        danger: true,
                    },
                ];
                app.modal = Modal::Menu { title: format!("Delete {}", b.name), items, sel: 0 };
            }
        }
        Merge => {
            let Some(b) = app.selected_branch().cloned() else { return };
            if b.is_head {
                return;
            }
            app.run_op(format!("Merge {}", b.name), Then::Refresh, async move { git.merge(&b.name, false).await });
        }
        Rebase => {
            let Some(b) = app.selected_branch().cloned() else { return };
            if b.is_head {
                return;
            }
            app.run_op(format!("Rebase onto {}", b.name), Then::Refresh, async move { git.rebase(&b.name).await });
        }
        SetUpstream => {
            let cur = app.current_branch().unwrap_or_default().to_string();
            app.modal = Modal::input(
                format!("Upstream for {cur}"),
                "e.g. origin/main",
                &format!("origin/{cur}"),
                InputKind::SetUpstream,
            );
        }
        NextRefsView | PrevRefsView => {
            let all = RefsView::ALL;
            let i = all.iter().position(|v| *v == app.refs_view).unwrap_or(0);
            let n = all.len();
            app.refs_view = all[if action == NextRefsView { (i + 1) % n } else { (i + n - 1) % n }];
            app.filters.remove(&Screen::Branches);
            app.focus = Focus::List;
            let st = app.list(Screen::Branches);
            *st.offset_mut() = 0;
            st.select(Some(0));
            app.clamp_selections();
            diff::load_for_selection(app);
        }
        CheckoutTag => {
            let Some(t) = app.selected_tag().cloned() else { return };
            confirm(
                app,
                &format!("Check out tag {}?", t.name),
                vec![
                    "You'll be in 'detached HEAD' state at this tag: good for".into(),
                    "building or testing a release. Create a branch (n in History)".into(),
                    "if you want to commit from here.".into(),
                ],
                Pending::Checkout(t.name),
                false,
            );
        }
        NewTag => {
            app.modal =
                Modal::input("New tag at HEAD", "name, or `name: message` for an annotated tag", "", InputKind::NewTag);
        }
        DeleteTag => {
            let Some(t) = app.selected_tag().cloned() else { return };
            let mut items = vec![MenuItem {
                key: 'd',
                label: "Delete locally".into(),
                detail: "remote copies are untouched".into(),
                pending: Pending::DeleteTag { name: t.name.clone(), remote: None },
                danger: false,
            }];
            if let Some(r) = default_remote(app) {
                items.push(MenuItem {
                    key: 'D',
                    label: format!("Delete here and on {r}"),
                    detail: "removes it for everyone".into(),
                    pending: Pending::DeleteTag { name: t.name.clone(), remote: Some(r) },
                    danger: true,
                });
            }
            app.modal = Modal::Menu { title: format!("Delete tag {}", t.name), items, sel: 0 };
        }
        PushTag => {
            let Some(t) = app.selected_tag().cloned() else { return };
            let Some(remote) = default_remote(app) else {
                app.toast(Level::Warn, "No remote to push to");
                return;
            };
            app.run_op(format!("Push tag {} → {remote}", t.name), Then::Refresh, async move {
                git.push_tag(&remote, &t.name).await
            });
        }
        AddRemote => {
            app.modal = Modal::input(
                "Add remote",
                "name url, e.g. origin git@github.com:you/repo.git",
                "",
                InputKind::AddRemote,
            );
        }
        RemoveRemote => {
            let Some(r) = app.selected_remote().cloned() else { return };
            confirm(
                app,
                &format!("Remove remote {}?", r.name),
                vec![
                    format!("Forgets {} and its remote-tracking branches locally.", r.fetch_url),
                    "Nothing on the server is deleted.".into(),
                ],
                Pending::RemoveRemote(r.name),
                true,
            );
        }
        RenameRemote => {
            let Some(r) = app.selected_remote().cloned() else { return };
            app.modal = Modal::input(
                format!("Rename remote {}", r.name),
                "new name",
                &r.name,
                InputKind::RenameRemote(r.name.clone()),
            );
        }
        EditRemoteUrl => {
            let Some(r) = app.selected_remote().cloned() else { return };
            app.modal = Modal::input(
                format!("URL for {}", r.name),
                "fetch and push URL",
                &r.fetch_url,
                InputKind::EditRemoteUrl(r.name.clone()),
            );
        }
        FetchRemote => {
            let Some(r) = app.selected_remote().cloned() else { return };
            let p = progress_sender(app);
            app.run_op(format!("Fetch {}", r.name), Then::Refresh, async move { git.fetch(Some(&r.name), p).await });
        }
        NextConflict | PrevConflict => {
            if let Some(c) = app.conflict.as_mut() {
                c.step(if action == NextConflict { 1 } else { -1 });
            }
        }
        KeepOurs => crate::views::conflict::choose(app, Choice::Ours),
        KeepTheirs => crate::views::conflict::choose(app, Choice::Theirs),
        KeepBoth => crate::views::conflict::choose(app, Choice::Both),
        RestoreConflict => crate::views::conflict::restore(app),
        StashApply | StashPop => {
            let Some(s) = app.data.stashes.get(app.selected(Screen::Stash)).cloned() else { return };
            if action == StashApply {
                app.run_op(format!("Apply {}", s.name), Then::Refresh, async move { git.stash_apply(&s.name).await });
            } else {
                app.run_op(format!("Pop {}", s.name), Then::Refresh, async move { git.stash_pop(&s.name).await });
            }
        }
        StashDrop => {
            let Some(s) = app.data.stashes.get(app.selected(Screen::Stash)).cloned() else { return };
            confirm(app, "Drop stash?", vec![format!("{}: {}", s.name, s.message)], Pending::StashDrop(s.name), true);
        }
        TakeOurs | TakeTheirs => {
            let Some(row) = app.selected_status_row().filter(|r| r.section == Section::Conflicts) else {
                app.toast(Level::Info, "Select a conflicted file first");
                return;
            };
            let path = app.data.status.files[row.file].path.clone();
            let ours = action == TakeOurs;
            let side = if ours { "ours" } else { "theirs" };
            app.run_op(format!("Resolve {path} with {side}"), Then::Refresh, async move {
                git.checkout_side(&path, ours).await
            });
        }
        ContinueOp => {
            let state = git.state();
            let label = format!("Continue {}", state_word(state));
            match state {
                RepoState::Merging => app.run_op(label, Then::Refresh, async move { git.merge_continue().await }),
                RepoState::Rebasing => app.run_op(label, Then::Refresh, async move { git.rebase_continue().await }),
                RepoState::CherryPicking => app.run_op(label, Then::Refresh, async move {
                    git.run(&["-c", "core.editor=true", "cherry-pick", "--continue"]).await
                }),
                RepoState::Reverting => app.run_op(label, Then::Refresh, async move {
                    git.run(&["-c", "core.editor=true", "revert", "--continue"]).await
                }),
                _ => app.toast(Level::Info, "No merge, rebase, or cherry-pick in progress"),
            }
        }
        AbortOp => {
            let state = git.state();
            if matches!(state, RepoState::Clean | RepoState::Bisecting) {
                app.toast(Level::Info, "No merge, rebase, or cherry-pick in progress");
                return;
            }
            confirm(
                app,
                &format!("Abort {}?", state_word(state)),
                vec!["Everything goes back to how it was before it started.".into()],
                Pending::AbortOp,
                true,
            );
        }
        Custom(i) => {
            let Some(c) = app.config.custom_commands.get(i).cloned() else { return };
            if c.confirm {
                confirm(app, "Run custom command?", vec![format!("$ {}", c.cmd)], Pending::Custom(i), true);
            } else {
                execute(app, Pending::Custom(i));
            }
        }
        _ => {}
    }
}

fn do_action_reset_reflog(app: &mut App) {
    let Some(r) = app.data.reflog.get(app.selected(Screen::Reflog)).cloned() else { return };
    reset_menu(app, r.oid, r.selector);
}

pub fn state_word(s: RepoState) -> &'static str {
    match s {
        RepoState::Merging => "merge",
        RepoState::Rebasing => "rebase",
        RepoState::CherryPicking => "cherry-pick",
        RepoState::Reverting => "revert",
        RepoState::Bisecting => "bisect",
        RepoState::Clean => "operation",
    }
}

fn reset_menu(app: &mut App, oid: String, name: String) {
    let item = |key, label: &str, detail: &str, mode, danger| MenuItem {
        key,
        label: label.into(),
        detail: detail.into(),
        pending: Pending::Reset { rev: oid.clone(), mode },
        danger,
    };
    app.modal = Modal::Menu {
        title: format!("Reset current branch to {name}"),
        items: vec![
            item('s', "Soft", "keep changes staged", ResetMode::Soft, false),
            item('m', "Mixed", "keep changes, unstaged", ResetMode::Mixed, false),
            item('h', "Hard", "discard all changes (undo with z)", ResetMode::Hard, true),
        ],
        sel: 1,
    };
}

fn toggle_stage(app: &mut App) {
    let git = app.git.clone().expect("repo");
    let Some(row) = app.selected_status_row() else { return };
    let f = app.data.status.files[row.file].clone();
    let path = f.path.clone();
    match row.section {
        Section::Unstaged => {
            app.run_op(format!("Stage {path}"), Then::Refresh, async move { git.stage(&[&path]).await })
        }
        Section::Staged => {
            app.run_op(format!("Unstage {path}"), Then::Refresh, async move { git.unstage(&[&path]).await })
        }
        Section::Conflicts => {
            let text = std::fs::read_to_string(git.repo.root.join(&path)).unwrap_or_default();
            if text.lines().any(|l| l.starts_with("<<<<<<<") || l.starts_with(">>>>>>>")) {
                app.toast(Level::Warn, "Still has conflict markers — edit it (e), or keep ours (o) / theirs (t)");
                return;
            }
            app.run_op(format!("Mark {path} resolved"), Then::Refresh, async move { git.stage(&[&path]).await });
        }
    }
}

fn push(app: &mut App) {
    let b = &app.data.status.branch;
    let Some(branch) = b.head.clone() else {
        app.toast(Level::Warn, "Detached HEAD: create a branch first (n in History)");
        return;
    };
    let remote =
        app.data.remotes.iter().find(|r| r.name == "origin").or(app.data.remotes.first()).map(|r| r.name.clone());
    let Some(remote) = remote else {
        app.toast(Level::Warn, "No remote configured — try: ! remote add origin <url>");
        return;
    };
    if b.upstream.is_none() {
        execute(app, Pending::Push { remote, branch, set_upstream: true, force: false });
        return;
    }
    let up_remote = b.upstream.as_deref().and_then(|u| u.split_once('/')).map(|x| x.0.to_string()).unwrap_or(remote);
    match (b.ahead, b.behind) {
        (0, 0) => app.toast(Level::Info, "Up to date — nothing to push"),
        (_, 0) => execute(app, Pending::Push { remote: up_remote, branch, set_upstream: false, force: false }),
        (0, n) => app.toast(Level::Info, format!("Behind by {n} — pull first (p)")),
        (a, n) => {
            app.modal = Modal::Menu {
                title: format!("Branch has diverged: {a} ahead, {n} behind"),
                items: vec![
                    MenuItem {
                        key: 'r',
                        label: "Pull with rebase".into(),
                        detail: "replay your commits on top, then push again".into(),
                        pending: Pending::Pull { rebase: true },
                        danger: false,
                    },
                    MenuItem {
                        key: 'm',
                        label: "Pull with merge".into(),
                        detail: "create a merge commit".into(),
                        pending: Pending::Pull { rebase: false },
                        danger: false,
                    },
                    MenuItem {
                        key: 'f',
                        label: "Force push (with lease)".into(),
                        detail: "overwrite the remote; refuses if it moved unexpectedly".into(),
                        pending: Pending::Push { remote: up_remote, branch, set_upstream: false, force: true },
                        danger: true,
                    },
                ],
                sel: 0,
            };
        }
    }
}

/// `origin` if it exists, else the first remote.
fn default_remote(app: &App) -> Option<String> {
    app.data.remotes.iter().find(|r| r.name == "origin").or(app.data.remotes.first()).map(|r| r.name.clone())
}

/// Undo the last HEAD movement using the reflog.
fn undo(app: &mut App) {
    let Some(last) = app.data.reflog.first().cloned() else {
        app.toast(Level::Info, "Nothing to undo");
        return;
    };
    let Some(prev) = app.data.reflog.get(1).cloned() else {
        app.toast(Level::Info, "Nothing to undo");
        return;
    };
    if let Some(rest) = last.subject.strip_prefix("checkout: moving from ") {
        if let Some((from, _to)) = rest.split_once(" to ") {
            confirm(
                app,
                "Undo checkout?",
                vec![format!("Last action: {}", last.subject), format!("Switch back to {from}.")],
                Pending::UndoCheckout(from.to_string()),
                false,
            );
            return;
        }
    }
    let soft = last.subject.starts_with("commit");
    let how = if soft {
        "The commit's changes come back as staged changes (git reset --soft)."
    } else {
        "Your uncommitted changes are kept (git reset --keep, or --mixed if needed)."
    };
    confirm(
        app,
        "Undo last action?",
        vec![
            format!("Last action: {}", last.subject),
            format!("Move the branch back to {} ({})", &prev.oid[..7], prev.subject),
            String::new(),
            how.into(),
        ],
        Pending::UndoReset { oid: prev.oid, soft },
        false,
    );
}

fn open_rebase(app: &mut App) {
    if app.filters.contains_key(&Screen::Log) {
        app.toast(Level::Warn, "Clear the filter (esc) before rebasing");
        return;
    }
    let sel = app.selected(Screen::Log);
    let commits = &app.data.log;
    if sel >= commits.len() {
        return;
    }
    let range = &commits[..=sel];
    if range.iter().any(|c| c.parents.len() > 1) {
        app.toast(Level::Warn, "That range contains merge commits — interactive rebase would flatten them");
        return;
    }
    let Some(base) = commits[sel].parents.first().cloned() else {
        app.toast(Level::Warn, "Can't rebase the root commit from here");
        return;
    };
    let items =
        range.iter().rev().map(|c| RebaseItem { action: TodoAction::Pick, commit: c.clone() }).collect::<Vec<_>>();
    let sel = items.len() - 1;
    app.modal = Modal::Rebase { base, items, sel };
}

// ---------------------------------------------------------------- modals

fn modal_key(app: &mut App, key: KeyEvent) {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    let modal = std::mem::replace(&mut app.modal, Modal::None);
    app.modal = match modal {
        Modal::None => Modal::None,
        Modal::Welcome => {
            if let Some(path) = crate::config::Config::path() {
                if !path.exists() {
                    let _ = std::fs::create_dir_all(path.parent().unwrap_or(&path));
                    let _ = std::fs::write(&path, crate::config::Config::EXAMPLE);
                }
            }
            Modal::None
        }
        Modal::Help { scroll } => match key.code {
            KeyCode::Char('j') | KeyCode::Down => Modal::Help { scroll: scroll.saturating_add(1) },
            KeyCode::Char('k') | KeyCode::Up => Modal::Help { scroll: scroll.saturating_sub(1) },
            KeyCode::PageDown => Modal::Help { scroll: scroll.saturating_add(10) },
            KeyCode::PageUp => Modal::Help { scroll: scroll.saturating_sub(10) },
            _ => Modal::None,
        },
        Modal::Output { title, lines, scroll } => match key.code {
            KeyCode::Char('j') | KeyCode::Down => Modal::Output { title, lines, scroll: scroll.saturating_add(1) },
            KeyCode::Char('k') | KeyCode::Up => Modal::Output { title, lines, scroll: scroll.saturating_sub(1) },
            KeyCode::PageDown | KeyCode::Char(' ') => Modal::Output { title, lines, scroll: scroll.saturating_add(10) },
            KeyCode::PageUp => Modal::Output { title, lines, scroll: scroll.saturating_sub(10) },
            _ => Modal::None,
        },
        Modal::Confirm { title, lines, pending, danger } => match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                execute(app, pending);
                // `execute` may have opened another modal.
                std::mem::replace(&mut app.modal, Modal::None)
            }
            KeyCode::Char('n') | KeyCode::Esc | KeyCode::Char('q') => Modal::None,
            _ => Modal::Confirm { title, lines, pending, danger },
        },
        Modal::Menu { title, items, sel } => match key.code {
            KeyCode::Esc | KeyCode::Char('q') => Modal::None,
            KeyCode::Char('j') | KeyCode::Down => {
                let sel = (sel + 1).min(items.len() - 1);
                Modal::Menu { title, items, sel }
            }
            KeyCode::Char('k') | KeyCode::Up => Modal::Menu { title, items, sel: sel.saturating_sub(1) },
            KeyCode::Enter => {
                execute(app, items[sel].pending.clone());
                std::mem::replace(&mut app.modal, Modal::None)
            }
            KeyCode::Char(c) => match items.iter().find(|i| i.key == c) {
                Some(i) => {
                    execute(app, i.pending.clone());
                    std::mem::replace(&mut app.modal, Modal::None)
                }
                None => Modal::Menu { title, items, sel },
            },
            _ => Modal::Menu { title, items, sel },
        },
        Modal::Input { title, hint, mut input, kind } => match key.code {
            KeyCode::Esc => {
                if let InputKind::Search(s) = kind {
                    app.filters.remove(&s);
                    app.clamp_selections();
                    diff::load_for_selection(app);
                }
                Modal::None
            }
            KeyCode::Enter => {
                submit_input(app, input.text(), kind);
                std::mem::replace(&mut app.modal, Modal::None)
            }
            _ => {
                input.handle_key(key);
                if let InputKind::Search(s) = &kind {
                    let text = input.text();
                    if text.is_empty() {
                        app.filters.remove(s);
                    } else {
                        app.filters.insert(*s, text);
                    }
                    app.set_selected(*s, 0);
                    app.clamp_selections();
                    diff::load_for_selection(app);
                }
                Modal::Input { title, hint, input, kind }
            }
        },
        Modal::Commit { mut subject, mut body, on_body, amend } => {
            let submit = (key.code == KeyCode::Char('s') && ctrl) || (key.code == KeyCode::Enter && !on_body);
            match key.code {
                KeyCode::Esc => Modal::None,
                _ if submit => {
                    if subject.text().trim().is_empty() {
                        app.toast(Level::Warn, "Write a short summary first");
                        Modal::Commit { subject, body, on_body, amend }
                    } else {
                        submit_commit(app, subject.text(), body.text(), amend);
                        Modal::None
                    }
                }
                KeyCode::Tab | KeyCode::BackTab => Modal::Commit { subject, body, on_body: !on_body, amend },
                KeyCode::Down if !on_body => Modal::Commit { subject, body, on_body: true, amend },
                KeyCode::Up if on_body && body.cursor().0 == 0 => {
                    Modal::Commit { subject, body, on_body: false, amend }
                }
                _ => {
                    if on_body {
                        body.handle_key(key);
                    } else {
                        subject.handle_key(key);
                    }
                    Modal::Commit { subject, body, on_body, amend }
                }
            }
        }
        Modal::Palette { mut input, sel } => {
            let entries = palette_matches(app, &input.text());
            match key.code {
                KeyCode::Esc => Modal::None,
                KeyCode::Enter => {
                    if let Some((a, _)) = entries.get(sel) {
                        let a = *a;
                        do_action(app, a);
                        std::mem::replace(&mut app.modal, Modal::None)
                    } else {
                        Modal::None
                    }
                }
                KeyCode::Down => Modal::Palette { input, sel: (sel + 1).min(entries.len().saturating_sub(1)) },
                KeyCode::Char('n') if ctrl => {
                    Modal::Palette { input, sel: (sel + 1).min(entries.len().saturating_sub(1)) }
                }
                KeyCode::Up => Modal::Palette { input, sel: sel.saturating_sub(1) },
                KeyCode::Char('p') if ctrl => Modal::Palette { input, sel: sel.saturating_sub(1) },
                _ => {
                    input.handle_key(key);
                    Modal::Palette { input, sel: 0 }
                }
            }
        }
        Modal::Rebase { base, mut items, mut sel } => {
            let set = |items: &mut Vec<RebaseItem>, sel: usize, a| items[sel].action = a;
            match key.code {
                KeyCode::Esc | KeyCode::Char('q') => return,
                KeyCode::Char('j') | KeyCode::Down if !ctrl => sel = (sel + 1).min(items.len() - 1),
                KeyCode::Char('k') | KeyCode::Up if !ctrl => sel = sel.saturating_sub(1),
                KeyCode::Char('K') | KeyCode::Char('k') if sel > 0 => {
                    items.swap(sel, sel - 1);
                    sel -= 1;
                }
                KeyCode::Char('J') | KeyCode::Char('j') if sel + 1 < items.len() => {
                    items.swap(sel, sel + 1);
                    sel += 1;
                }
                KeyCode::Char('p') => set(&mut items, sel, TodoAction::Pick),
                KeyCode::Char('e') => set(&mut items, sel, TodoAction::Edit),
                KeyCode::Char('s') if sel > 0 => set(&mut items, sel, TodoAction::Squash),
                KeyCode::Char('f') if sel > 0 => set(&mut items, sel, TodoAction::Fixup),
                KeyCode::Char('d') => set(&mut items, sel, TodoAction::Drop),
                KeyCode::Enter => {
                    let todo: String = items
                        .iter()
                        .map(|i| format!("{} {} {}\n", i.action.word(), i.commit.oid, i.commit.subject))
                        .collect();
                    let git = app.git.clone().expect("repo");
                    app.run_op("Interactive rebase", Then::Refresh, async move {
                        git.rebase_interactive(&base, &todo).await
                    });
                    return;
                }
                _ => {}
            }
            Modal::Rebase { base, items, sel }
        }
    };
}

pub fn palette_matches(app: &App, query: &str) -> Vec<(Action, String)> {
    let mut entries: Vec<(i64, Action, String)> = palette_actions(&app.keymap, screen_ctx(app))
        .into_iter()
        .chain(app.config.custom_commands.iter().enumerate().map(|(i, c)| (Action::Custom(i), c.key.clone())))
        .filter_map(|(a, k)| {
            let label = match a {
                Action::Custom(i) => {
                    let c = &app.config.custom_commands[i];
                    if c.description.is_empty() {
                        c.cmd.clone()
                    } else {
                        c.description.clone()
                    }
                }
                _ => a.label().to_string(),
            };
            crate::fuzzy::score(query, &label).map(|s| (s, a, k))
        })
        .collect();
    if !query.is_empty() {
        entries.sort_by_key(|e| std::cmp::Reverse(e.0));
    }
    entries.into_iter().map(|(_, a, k)| (a, k)).collect()
}

fn submit_commit(app: &mut App, subject: String, body: String, amend: bool) {
    let git = app.git.clone().expect("repo");
    let mut message = subject.trim().to_string();
    if !body.trim().is_empty() {
        message.push_str("\n\n");
        message.push_str(body.trim_end());
    }
    let stage_all = !amend && app.data.status.staged().count() == 0;
    let opts = CommitOpts { amend, ..Default::default() };
    let label = if amend { "Amend commit" } else { "Commit" };
    app.run_op(label, Then::Refresh, async move {
        if stage_all {
            git.stage_all().await?;
        }
        git.commit(&message, &opts).await
    });
    if stage_all {
        app.history.push("git add --all".into());
    }
}

fn submit_input(app: &mut App, text: String, kind: InputKind) {
    let git = app.git.clone();
    let text = text.trim().to_string();
    match kind {
        InputKind::Search(_) => {}
        _ if text.is_empty() && !matches!(kind, InputKind::StashMessage) => {}
        InputKind::NewBranch { start } => {
            let git = git.expect("repo");
            let name = text.replace(' ', "-");
            app.run_op(format!("Create branch {name}"), Then::Refresh, async move {
                git.create_branch(&name, start.as_deref(), true).await
            });
        }
        InputKind::RenameBranch(old) => {
            let git = git.expect("repo");
            app.run_op(format!("Rename {old} → {text}"), Then::Refresh, async move {
                git.rename_branch(&old, &text).await
            });
        }
        InputKind::Tag(oid) => {
            let git = git.expect("repo");
            app.run_op(format!("Tag {text}"), Then::Refresh, async move { git.create_tag(&text, &oid, None).await });
        }
        InputKind::StashMessage => {
            let git = git.expect("repo");
            app.run_op("Stash", Then::Refresh, async move {
                git.stash_push((!text.is_empty()).then_some(text.as_str()), true).await
            });
        }
        InputKind::NewTag => {
            let git = git.expect("repo");
            let (name, message) = match text.split_once(':') {
                Some((n, m)) if !m.trim().is_empty() => (n.trim().replace(' ', "-"), Some(m.trim().to_string())),
                _ => (text.trim_end_matches(':').replace(' ', "-"), None),
            };
            app.run_op(format!("Tag {name}"), Then::Refresh, async move {
                git.create_tag(&name, "HEAD", message.as_deref()).await
            });
        }
        InputKind::AddRemote => {
            let git = git.expect("repo");
            let Some((name, url)) = text.split_once(char::is_whitespace) else {
                app.toast(Level::Warn, "Enter a name and a URL, separated by a space");
                return;
            };
            let (name, url) = (name.to_string(), url.trim().to_string());
            app.run_op(format!("Add remote {name}"), Then::Refresh, async move { git.add_remote(&name, &url).await });
        }
        InputKind::RenameRemote(old) => {
            let git = git.expect("repo");
            app.run_op(format!("Rename remote {old} → {text}"), Then::Refresh, async move {
                git.rename_remote(&old, &text).await
            });
        }
        InputKind::EditRemoteUrl(name) => {
            let git = git.expect("repo");
            app.run_op(
                format!("Set URL of {name}"),
                Then::Refresh,
                async move { git.set_remote_url(&name, &text).await },
            );
        }
        InputKind::SetUpstream => {
            let git = git.expect("repo");
            app.run_op(format!("Set upstream {text}"), Then::Refresh, async move { git.set_upstream(&text).await });
        }
        InputKind::RawGit => {
            let git = git.expect("repo");
            let args = split_args(text.strip_prefix("git ").unwrap_or(&text));
            let label = format!("git {text}");
            let tx = app.tx.clone();
            app.busy = Some(label.clone());
            tokio::spawn(async move {
                let argv: Vec<&str> = args.iter().map(String::as_str).collect();
                let result = git.raw(&argv).await.map_err(|e| e.to_string());
                // Show output in a scrollable panel.
                let _ = tx.send(Msg::OpDone { label: label.clone(), result: result.clone(), then: Then::Refresh });
                if let Ok(o) = result {
                    let text = format!("{}{}", o.stdout, o.stderr);
                    if !text.trim().is_empty() {
                        let _ = tx.send(Msg::Progress(String::new()));
                        let _ = tx.send(Msg::ShowOutput(label, text));
                    }
                }
            });
        }
    }
}

/// Split a command line on whitespace, honouring single/double quotes.
pub fn split_args(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quote: Option<char> = None;
    let mut has = false;
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (None, '\'' | '"') => {
                quote = Some(c);
                has = true;
            }
            (None, '\\') | (Some('"'), '\\') => {
                if let Some(n) = chars.next() {
                    cur.push(n);
                    has = true;
                }
            }
            (None, c) if c.is_whitespace() => {
                if has {
                    out.push(std::mem::take(&mut cur));
                    has = false;
                }
            }
            (_, c) => {
                cur.push(c);
                has = true;
            }
        }
    }
    if has {
        out.push(cur);
    }
    out
}

/// Run a confirmed/pending operation.
pub fn execute(app: &mut App, pending: Pending) {
    let Some(git) = app.git.clone() else { return };
    match pending {
        Pending::Discard(files) => {
            app.run_op("Discard", Then::Refresh, async move {
                let tracked: Vec<&str> = files.iter().filter(|f| !f.1).map(|f| f.0.as_str()).collect();
                let untracked: Vec<&str> = files.iter().filter(|f| f.1).map(|f| f.0.as_str()).collect();
                let mut last = None;
                if !tracked.is_empty() {
                    last = Some(git.discard(&tracked).await?);
                }
                if !untracked.is_empty() {
                    last = Some(git.clean(&untracked).await?);
                }
                Ok(last.expect("at least one file"))
            });
        }
        Pending::DeleteBranch { name, remote, force } => {
            if remote {
                let (r, b) = name.split_once('/').map(|(a, b)| (a.to_string(), b.to_string())).unwrap_or_default();
                app.run_op(
                    format!("Delete {name}"),
                    Then::Refresh,
                    async move { git.delete_remote_branch(&r, &b).await },
                );
            } else {
                app.run_op(
                    format!("Delete {name}"),
                    Then::Refresh,
                    async move { git.delete_branch(&name, force).await },
                );
            }
        }
        Pending::DeleteTag { name, remote } => {
            let label = match &remote {
                Some(r) => format!("Delete tag {name} (local + {r})"),
                None => format!("Delete tag {name}"),
            };
            app.run_op(label, Then::Refresh, async move {
                if let Some(r) = remote {
                    // Remote first: if that fails, the local tag is still there to retry.
                    git.delete_remote_tag(&r, &name).await?;
                }
                git.delete_tag(&name).await
            });
        }
        Pending::RemoveRemote(name) => {
            app.run_op(format!("Remove remote {name}"), Then::Refresh, async move { git.remove_remote(&name).await });
        }
        Pending::StashDrop(name) => {
            app.run_op(format!("Drop {name}"), Then::Refresh, async move { git.stash_drop(&name).await });
        }
        Pending::Reset { rev, mode } => {
            let m = match mode {
                ResetMode::Soft => "soft",
                ResetMode::Mixed => "mixed",
                ResetMode::Hard => "hard",
            };
            app.run_op(format!("Reset ({m}) to {}", &rev[..7.min(rev.len())]), Then::Refresh, async move {
                git.reset(&rev, mode).await
            });
        }
        Pending::UndoCheckout(branch) => {
            app.run_op(format!("Undo: back to {branch}"), Then::Refresh, async move { git.checkout(&branch).await });
        }
        Pending::UndoReset { oid, soft } => {
            app.run_op("Undo", Then::Refresh, async move {
                if soft {
                    return git.reset(&oid, ResetMode::Soft).await;
                }
                // --keep refuses when local edits touch files that differ
                // between the commits; --mixed never touches the worktree.
                match git.run(&["reset", "--keep", &oid]).await {
                    Ok(o) => Ok(o),
                    Err(_) => git.reset(&oid, ResetMode::Mixed).await,
                }
            });
        }
        Pending::Checkout(rev) => {
            app.run_op(format!("Checkout {}", &rev[..7.min(rev.len())]), Then::Refresh, async move {
                git.checkout(&rev).await
            });
        }
        Pending::AbortOp => {
            let label = format!("Abort {}", state_word(git.state()));
            app.run_op(label, Then::Refresh, async move {
                match git.state() {
                    RepoState::Merging => git.merge_abort().await,
                    RepoState::Rebasing => git.rebase_abort().await,
                    RepoState::CherryPicking => git.cherry_pick_abort().await,
                    RepoState::Reverting => git.run(&["revert", "--abort"]).await,
                    _ => git.run(&["status", "--short"]).await,
                }
            });
        }
        Pending::Push { remote, branch, set_upstream, force } => {
            let p = progress_sender(app);
            let label = if force { format!("Force push {branch}") } else { format!("Push {branch} → {remote}") };
            app.run_op(label, Then::Refresh, async move { git.push(&remote, &branch, set_upstream, force, p).await });
        }
        Pending::Pull { rebase } => {
            let p = progress_sender(app);
            app.run_op(if rebase { "Pull (rebase)" } else { "Pull (merge)" }, Then::Refresh, async move {
                git.pull(rebase, p).await
            });
        }
        Pending::Custom(i) => {
            let Some(c) = app.config.custom_commands.get(i).cloned() else { return };
            let root = git.repo.root.clone();
            let label = if c.description.is_empty() { c.cmd.clone() } else { c.description.clone() };
            let tx = app.tx.clone();
            app.busy = Some(label.clone());
            tokio::spawn(async move {
                let res = tokio::process::Command::new("sh")
                    .arg("-c")
                    .arg(&c.cmd)
                    .current_dir(root)
                    .stdin(std::process::Stdio::null())
                    .output()
                    .await;
                let (result, text) = match res {
                    Ok(o) => {
                        let text =
                            format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr));
                        if o.status.success() {
                            (Ok(Output { cmd: c.cmd.clone(), stdout: String::new(), stderr: String::new() }), text)
                        } else {
                            (Err(text.clone()), String::new())
                        }
                    }
                    Err(e) => (Err(e.to_string()), String::new()),
                };
                let _ = tx.send(Msg::OpDone { label: label.clone(), result, then: Then::Refresh });
                if !text.trim().is_empty() {
                    let _ = tx.send(Msg::ShowOutput(label, text));
                }
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::split_args;

    #[test]
    fn splits_quoted_args() {
        assert_eq!(split_args("log --oneline -5"), vec!["log", "--oneline", "-5"]);
        assert_eq!(split_args(r#"commit -m "hello world""#), vec!["commit", "-m", "hello world"]);
        assert_eq!(split_args("commit -m 'it''s'"), vec!["commit", "-m", "its"]);
        assert_eq!(split_args(r"a\ b c"), vec!["a b", "c"]);
        assert_eq!(split_args("x ''"), vec!["x", ""]);
    }
}
