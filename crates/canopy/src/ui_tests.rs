//! Render real frames into a TestBackend against a scripted demo repository.
//! Set CANOPY_PRINT=1 to dump every frame to stdout.

use std::path::Path;
use std::process::Command;

use ratatui::backend::TestBackend;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Terminal;
use tempfile::TempDir;

use crate::app::App;
use crate::config::Config;
use crate::keymap::Screen;
use crate::modal::Modal;

fn sh(dir: &Path, script: &str) {
    let out = Command::new("sh").arg("-c").arg(script).current_dir(dir).output().unwrap();
    assert!(out.status.success(), "{script}: {}", String::from_utf8_lossy(&out.stderr));
}

fn demo_repo() -> TempDir {
    let dir = TempDir::new().unwrap();
    demo_repo_at(dir.path());
    dir
}

/// Build the demo repository in `path` (which must exist).
fn demo_repo_at(path: &Path) {
    sh(
        path,
        r#"
set -e
git init -q -b main
git config user.name "Ada Lovelace"; git config user.email ada@example.com; git config commit.gpgsign false
printf 'fn main() {\n    println!("hello");\n}\n' > main.rs
printf '# Demo\n' > README.md
git add -A; git commit -qm "Initial commit"
printf 'pub fn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n' > lib.rs
git add -A; git commit -qm "Add math library"
git switch -qc feature/parser
printf 'pub fn parse() {}\n' > parser.rs; git add -A; git commit -qm "Add CSV parser"
git switch -q main
printf '\npub fn sub() {}\n' >> lib.rs; git commit -qam "Add subtraction"
git merge -q --no-edit feature/parser
git tag v0.1.0
printf 'fn main() {\n    println!("hello, world");\n    let x = 42;\n}\n' > main.rs
printf 'pub fn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n\npub fn mul() {}\n' > lib.rs
git add lib.rs
echo "TODO" > TODO.md
"#,
    );
}

async fn app_for(dir: &Path) -> App {
    let git = canopy_git::Git::open(dir).await.unwrap();
    let config = Config { workspace_dirs: vec![dir.display().to_string()], workspace_depth: 1, ..Config::default() };
    let mut app = App::new(Some(git), config, dir.parent().unwrap().to_path_buf());
    app.refresh();
    app.settle().await;
    app
}

fn render(app: &mut App, w: u16, h: u16) -> String {
    let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
    term.draw(|f| crate::ui::draw(f, app)).unwrap();
    let buf = term.backend().buffer();
    let mut out = String::new();
    for y in 0..h {
        for x in 0..w {
            out.push_str(buf[(x, y)].symbol());
        }
        out.push('\n');
    }
    if std::env::var("CANOPY_PRINT").is_ok() {
        println!("{out}");
    }
    out
}

async fn press(app: &mut App, code: KeyCode) {
    crate::input::handle_key(app, KeyEvent::new(code, KeyModifiers::NONE));
    app.settle().await;
}

/// Type text, then settle once (typing itself starts no background work).
async fn chars(app: &mut App, s: &str) {
    for c in s.chars() {
        crate::input::handle_key(app, KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
    }
    app.settle().await;
}

fn git_out(dir: &Path, args: &[&str]) -> String {
    let o = Command::new("git").args(args).current_dir(dir).output().unwrap();
    String::from_utf8_lossy(&o.stdout).into_owned()
}

#[tokio::test]
async fn home_dashboard() {
    let dir = demo_repo();
    let mut app = app_for(dir.path()).await;
    let s = render(&mut app, 130, 36);
    assert!(s.contains("canopy"));
    assert!(s.contains("Next steps"));
    assert!(s.contains("1 file(s) staged and ready. Press c to commit."), "{s}");
    assert!(s.contains("Merge branch 'feature/pars"), "{s}");
    assert!(s.contains("(tag v0.1.0)") && s.contains("(HEAD → main)"));
    assert!(s.contains("Activity"));
    // Narrow terminal still renders.
    render(&mut app, 70, 24);
}

#[tokio::test]
async fn stage_line_then_commit_flow() {
    let dir = demo_repo();
    let mut app = app_for(dir.path()).await;
    press(&mut app, KeyCode::Char('2')).await;
    assert_eq!(app.screen, Screen::Status);
    let s = render(&mut app, 130, 36);
    assert!(s.contains("Changes") && s.contains("Staged") && s.contains("TODO.md"), "{s}");

    // Select main.rs (unstaged, modified) and dive into its diff.
    let rows = app.status_rows();
    let idx = rows.iter().position(|r| app.data.status.files[r.file].path == "main.rs").unwrap();
    app.set_selected(Screen::Status, idx);
    crate::views::diff::load_for_selection(&mut app);
    app.settle().await;
    press(&mut app, KeyCode::Enter).await;
    let s = render(&mut app, 130, 36);
    assert!(s.contains("main.rs · unstaged"), "{s}");

    // Cursor starts on the first change (`-    println!("hello");`); move to
    // the `+    let x = 42;` line and stage only that.
    let view = app.diff.as_ref().unwrap();
    let target = view
        .rows
        .iter()
        .position(|r| matches!(r, crate::views::diff::Row::Line(f, h, l) if view.files[*f].hunks[*h].lines[*l].content.contains("let x")))
        .unwrap();
    app.diff.as_mut().unwrap().cursor = target;
    press(&mut app, KeyCode::Char(' ')).await;
    let staged = git_out(dir.path(), &["show", ":main.rs"]);
    assert!(staged.contains("let x = 42") && staged.contains("println!(\"hello\")"), "{staged}");
    assert!(app.history.last().unwrap().starts_with("git apply --cached"));

    // Commit via the dialog.
    press(&mut app, KeyCode::Esc).await;
    press(&mut app, KeyCode::Char('c')).await;
    assert!(matches!(app.modal, Modal::Commit { .. }));
    chars(&mut app, "Add mul and x").await;
    let s = render(&mut app, 130, 36);
    assert!(s.contains("Summary") && s.contains("13/50"), "{s}");
    press(&mut app, KeyCode::Enter).await;
    assert_eq!(git_out(dir.path(), &["log", "-1", "--format=%s"]).trim(), "Add mul and x");
    // Remaining unstaged change is still there.
    assert!(git_out(dir.path(), &["diff"]).contains("hello, world"));
    let s = render(&mut app, 130, 36);
    assert!(s.contains("git commit -m 'Add mul and x'"), "teach line:\n{s}");
}

#[tokio::test]
async fn history_graph_and_undo() {
    let dir = demo_repo();
    let mut app = app_for(dir.path()).await;
    press(&mut app, KeyCode::Char('3')).await;
    let s = render(&mut app, 130, 30);
    assert!(s.contains("○─╮"), "merge commit glyph:\n{s}");
    assert!(s.contains("Add CSV parser"));

    // Filter
    press(&mut app, KeyCode::Char('/')).await;
    chars(&mut app, "csv").await;
    press(&mut app, KeyCode::Enter).await;
    assert_eq!(app.visible_log().len(), 1);
    render(&mut app, 130, 30);
    press(&mut app, KeyCode::Esc).await;
    assert_eq!(app.visible_log().len(), 5);

    // Reset (mixed) to the previous commit, then undo it with z.
    let before = git_out(dir.path(), &["rev-parse", "HEAD"]);
    press(&mut app, KeyCode::Char('j')).await;
    press(&mut app, KeyCode::Char('r')).await;
    render(&mut app, 130, 30);
    press(&mut app, KeyCode::Char('m')).await;
    assert_ne!(git_out(dir.path(), &["rev-parse", "HEAD"]), before);
    press(&mut app, KeyCode::Char('z')).await;
    let s = render(&mut app, 130, 30);
    assert!(s.contains("Undo last action?"), "{s}");
    press(&mut app, KeyCode::Char('y')).await;
    assert_eq!(git_out(dir.path(), &["rev-parse", "HEAD"]), before);
}

#[tokio::test]
async fn undo_commit_keeps_changes_staged() {
    let dir = demo_repo();
    let mut app = app_for(dir.path()).await;
    let before = git_out(dir.path(), &["rev-parse", "HEAD"]);
    press(&mut app, KeyCode::Char('c')).await;
    chars(&mut app, "oops").await;
    press(&mut app, KeyCode::Enter).await;
    assert_ne!(git_out(dir.path(), &["rev-parse", "HEAD"]), before);
    press(&mut app, KeyCode::Char('z')).await;
    let s = render(&mut app, 130, 30);
    assert!(s.contains("come back as staged"), "{s}");
    press(&mut app, KeyCode::Char('y')).await;
    assert_eq!(git_out(dir.path(), &["rev-parse", "HEAD"]), before);
    assert!(git_out(dir.path(), &["diff", "--cached", "--name-only"]).contains("lib.rs"));
}

#[tokio::test]
async fn branches_checkout_and_new() {
    let dir = demo_repo();
    let mut app = app_for(dir.path()).await;
    // Commit staged work so switching is clean.
    sh(dir.path(), "git stash -q -u");
    app.refresh();
    app.settle().await;
    press(&mut app, KeyCode::Char('4')).await;
    let s = render(&mut app, 130, 30);
    assert!(s.contains("feature/parser") && s.contains("Local"), "{s}");

    press(&mut app, KeyCode::Char('n')).await;
    chars(&mut app, "my feature").await;
    press(&mut app, KeyCode::Enter).await;
    assert_eq!(git_out(dir.path(), &["branch", "--show-current"]).trim(), "my-feature");

    // Undo the checkout goes back to main.
    press(&mut app, KeyCode::Char('z')).await;
    press(&mut app, KeyCode::Char('y')).await;
    assert_eq!(git_out(dir.path(), &["branch", "--show-current"]).trim(), "main");
}

#[tokio::test]
async fn palette_help_workspace_stash() {
    let dir = demo_repo();
    let mut app = app_for(dir.path()).await;
    press(&mut app, KeyCode::Char(':')).await;
    chars(&mut app, "stash").await;
    let s = render(&mut app, 120, 34);
    assert!(s.contains("Command palette") && s.contains("stash changes"), "{s}");
    press(&mut app, KeyCode::Esc).await;

    press(&mut app, KeyCode::Char('?')).await;
    let s = render(&mut app, 120, 40);
    assert!(s.contains("Everywhere") && s.contains("undo last action"), "{s}");
    press(&mut app, KeyCode::Esc).await;

    // Stash with a message, then see it on the Stash tab.
    press(&mut app, KeyCode::Char('S')).await;
    chars(&mut app, "wip").await;
    press(&mut app, KeyCode::Enter).await;
    press(&mut app, KeyCode::Char('5')).await;
    let s = render(&mut app, 120, 30);
    assert!(s.contains("stash@{0}") && s.contains("wip"), "{s}");
    assert!(app.data.status.is_clean());

    press(&mut app, KeyCode::Char('6')).await;
    app.settle().await;
    let s = render(&mut app, 120, 30);
    assert!(s.contains("Workspace") && s.contains("Repository"), "{s}");
}

#[tokio::test]
async fn merge_conflict_resolution() {
    let dir = demo_repo();
    sh(
        dir.path(),
        r#"
set -e
git stash -q -u
git switch -qc other; printf 'OTHER\n' > README.md; git commit -qam other
git switch -q main; printf 'MAIN\n' > README.md; git commit -qam main
git merge other >/dev/null 2>&1 || true
"#,
    );
    let mut app = app_for(dir.path()).await;
    let s = render(&mut app, 130, 30);
    assert!(s.contains("MERGE IN PROGRESS"), "{s}");
    assert!(s.contains("conflict(s)"), "{s}");
    press(&mut app, KeyCode::Char('2')).await;
    let s = render(&mut app, 130, 30);
    assert!(s.contains("Conflicts"), "{s}");
    // Take theirs, then continue.
    press(&mut app, KeyCode::Char('t')).await;
    press(&mut app, KeyCode::Char('C')).await;
    assert_eq!(std::fs::read_to_string(dir.path().join("README.md")).unwrap(), "OTHER\n");
    assert!(!dir.path().join(".git/MERGE_HEAD").exists());
    render(&mut app, 130, 30);
}

#[tokio::test]
async fn history_loads_more_pages() {
    let dir = TempDir::new().unwrap();
    sh(
        dir.path(),
        "git init -q -b main && git config user.name T && git config user.email t@t.io && \
         for i in $(seq 1 130); do git commit -q --allow-empty -m \"c$i\"; done",
    );
    let git = canopy_git::Git::open(dir.path()).await.unwrap();
    let config = Config { log_page_size: 50, ..Config::default() };
    let mut app = App::new(Some(git), config, dir.path().to_path_buf());
    app.refresh();
    app.settle().await;
    assert_eq!(app.data.log.len(), 50);
    assert!(app.log_has_more());
    press(&mut app, KeyCode::Char('3')).await;
    let s = render(&mut app, 120, 20);
    assert!(s.contains("History · 50+ commits"), "{s}");

    // Moving down near the end fetches the next pages.
    press(&mut app, KeyCode::Char('j')).await;
    assert_eq!(app.data.log.len(), 100);
    press(&mut app, KeyCode::Char('G')).await;
    press(&mut app, KeyCode::Char('G')).await;
    assert_eq!(app.data.log.len(), 130);
    assert!(!app.log_has_more());
    assert_eq!(app.data.log.last().unwrap().subject, "c1");

    // A refresh keeps everything loaded.
    app.refresh();
    app.settle().await;
    assert_eq!(app.data.log.len(), 130);
}

#[tokio::test]
async fn remapped_keys_work_and_show_in_hints() {
    let dir = demo_repo();
    let git = canopy_git::Git::open(dir.path()).await.unwrap();
    let config: Config = toml::from_str("[keys]\ncommit = \"C\"\ntoggle_stage = \"s\"\n").unwrap();
    let mut app = App::new(Some(git), config, dir.path().to_path_buf());
    app.refresh();
    app.settle().await;
    // `C` was "continue" on Changes; the user is told it lost its key.
    assert!(app.toast.as_ref().unwrap().text.contains("`continue_op` has no key left"));
    app.toast = None;
    let s = render(&mut app, 130, 30);
    assert!(s.contains("Press C to commit"), "{s}");
    assert!(s.contains(" C  commit"), "hint bar:\n{s}");

    press(&mut app, KeyCode::Char('2')).await;
    let s = render(&mut app, 130, 30);
    assert!(s.contains(" s  stage"), "{s}");
    // Old key does nothing; new key stages.
    press(&mut app, KeyCode::Char(' ')).await;
    let unstaged_before = app.data.status.unstaged().count();
    press(&mut app, KeyCode::Char('s')).await;
    assert_eq!(app.data.status.unstaged().count(), unstaged_before - 1);
    press(&mut app, KeyCode::Char('c')).await;
    assert!(!app.modal.is_open());
    press(&mut app, KeyCode::Char('C')).await;
    assert!(matches!(app.modal, Modal::Commit { .. }));
}

#[tokio::test]
async fn tags_and_remotes_views() {
    let dir = demo_repo();
    let bare = TempDir::new().unwrap();
    sh(bare.path(), "git init -q --bare -b main");
    let mut app = app_for(dir.path()).await;
    press(&mut app, KeyCode::Char('4')).await;
    press(&mut app, KeyCode::Char(']')).await;
    let s = render(&mut app, 130, 30);
    // The view switcher is full or compact depending on width; either names Tags.
    assert!((s.contains("Branches · Tags · Remotes") || s.contains("‹ Tags 2/5 ›")) && s.contains("v0.1.0"), "{s}");
    assert!(s.contains(" P  push"), "tags hint bar:\n{s}");

    // Annotated tag via `name: message`.
    press(&mut app, KeyCode::Char('n')).await;
    chars(&mut app, "v0.2.0: second release").await;
    press(&mut app, KeyCode::Enter).await;
    assert_eq!(git_out(dir.path(), &["tag", "-l", "v0.2.0", "--format=%(contents:subject)"]).trim(), "second release");

    // Remotes: add, rename, edit URL.
    press(&mut app, KeyCode::Char(']')).await;
    let s = render(&mut app, 130, 30);
    assert!(s.contains("No remotes yet"), "{s}");
    press(&mut app, KeyCode::Char('n')).await;
    chars(&mut app, &format!("up {}", bare.path().display())).await;
    press(&mut app, KeyCode::Enter).await;
    press(&mut app, KeyCode::Char('R')).await;
    // Clear the prefilled name, then type the new one.
    crate::input::handle_key(&mut app, KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL));
    chars(&mut app, "origin").await;
    press(&mut app, KeyCode::Enter).await;
    assert_eq!(git_out(dir.path(), &["remote"]).trim(), "origin");
    let s = render(&mut app, 130, 30);
    assert!(s.contains("Fetch URL:"), "remote details panel:\n{s}");

    // Push a tag, then delete it here and on the remote.
    press(&mut app, KeyCode::Char('[')).await;
    let i = app.visible_tags().iter().position(|&i| app.data.tags[i].name == "v0.2.0").unwrap();
    app.set_selected(Screen::Branches, i);
    press(&mut app, KeyCode::Char('P')).await;
    assert!(git_out(bare.path(), &["tag"]).contains("v0.2.0"));
    press(&mut app, KeyCode::Char('d')).await;
    press(&mut app, KeyCode::Char('D')).await;
    assert!(!git_out(bare.path(), &["tag"]).contains("v0.2.0"));
    assert!(!git_out(dir.path(), &["tag"]).contains("v0.2.0"));

    // Remove the remote (confirmed).
    press(&mut app, KeyCode::Char(']')).await;
    press(&mut app, KeyCode::Char('d')).await;
    press(&mut app, KeyCode::Char('y')).await;
    assert_eq!(git_out(dir.path(), &["remote"]).trim(), "");
}

#[tokio::test]
async fn resolve_conflicts_one_by_one() {
    let dir = demo_repo();
    sh(
        dir.path(),
        r#"
set -e
git stash -q -u
printf '1\nshared\n2\n3\n4\n5\n6\nshared\n7\n' > c.txt; git add c.txt; git commit -qm base
git switch -qc other; printf '1\nTHEIRS-A\n2\n3\n4\n5\n6\nTHEIRS-B\n7\n' > c.txt; git commit -qam other
git switch -q main; printf '1\nOURS-A\n2\n3\n4\n5\n6\nOURS-B\n7\n' > c.txt; git commit -qam main
git merge other >/dev/null 2>&1 || true
"#,
    );
    let mut app = app_for(dir.path()).await;
    press(&mut app, KeyCode::Char('2')).await;
    let s = render(&mut app, 130, 34);
    assert!(s.contains("Conflict 1 of 2") && s.contains("ours · HEAD") && s.contains("theirs · other"), "{s}");
    assert!(s.contains("to resolve conflicts one by one"), "{s}");

    press(&mut app, KeyCode::Enter).await;
    let s = render(&mut app, 130, 34);
    assert!(s.contains(" o  ours") && s.contains(" t  theirs"), "hint bar:\n{s}");

    // Resolve the first, change our mind, restore, then do it properly.
    press(&mut app, KeyCode::Char('o')).await;
    let text = std::fs::read_to_string(dir.path().join("c.txt")).unwrap();
    assert!(text.starts_with("1\nOURS-A\n2\n") && text.contains("<<<<<<<"), "{text}");
    press(&mut app, KeyCode::Char('u')).await;
    let text = std::fs::read_to_string(dir.path().join("c.txt")).unwrap();
    assert_eq!(text.matches("<<<<<<<").count(), 2);

    press(&mut app, KeyCode::Enter).await;
    press(&mut app, KeyCode::Char('t')).await; // conflict 1 -> theirs
    let s = render(&mut app, 130, 34);
    assert!(s.contains("Conflict 1 of 1"), "{s}");
    press(&mut app, KeyCode::Char('b')).await; // conflict 2 -> both
    assert_eq!(
        std::fs::read_to_string(dir.path().join("c.txt")).unwrap(),
        "1\nTHEIRS-A\n2\n3\n4\n5\n6\nOURS-B\nTHEIRS-B\n7\n"
    );
    // Fully resolved: staged automatically, merge can continue.
    assert_eq!(app.data.status.conflicted().count(), 0);
    press(&mut app, KeyCode::Char('C')).await;
    assert!(!dir.path().join(".git/MERGE_HEAD").exists());
}

#[tokio::test]
async fn file_history_and_blame() {
    let dir = demo_repo();
    let mut app = app_for(dir.path()).await;
    // lib.rs is staged; `L` from Changes shows only its commits.
    press(&mut app, KeyCode::Char('2')).await;
    let i = app.status_rows().iter().position(|r| app.data.status.files[r.file].path == "lib.rs").unwrap();
    app.set_selected(Screen::Status, i);
    press(&mut app, KeyCode::Char('L')).await;
    assert_eq!(app.screen, Screen::Log);
    let subjects: Vec<_> = app.data.log.iter().map(|c| c.subject.as_str()).collect();
    assert_eq!(subjects, vec!["Add subtraction", "Add math library"], "{subjects:?}");
    let s = render(&mut app, 130, 30);
    assert!(s.contains("History of lib.rs · 2 commits"), "{s}");
    // esc returns to all history.
    press(&mut app, KeyCode::Esc).await;
    assert!(app.log_path.is_none());
    assert_eq!(app.data.log.len(), 5);

    // Blame main.rs from the Changes tab (working tree, with uncommitted lines).
    press(&mut app, KeyCode::Char('2')).await;
    let i = app.status_rows().iter().position(|r| app.data.status.files[r.file].path == "main.rs").unwrap();
    app.set_selected(Screen::Status, i);
    press(&mut app, KeyCode::Char('B')).await;
    let s = render(&mut app, 130, 30);
    assert!(s.contains("Blame · main.rs (working tree)") && s.contains("not committed"), "{s}");
    // First line comes from "Initial commit"; enter jumps there in History.
    let Modal::Blame(v) = &app.modal else { panic!("blame not open") };
    let first = v.blame.lines[0].oid.clone();
    assert_eq!(v.blame.commits[&first].summary, "Initial commit");
    press(&mut app, KeyCode::Enter).await;
    assert!(!app.modal.is_open());
    assert_eq!(app.screen, Screen::Log);
    assert_eq!(app.selected_commit().unwrap().oid, first);

    // From a commit's diff: blame lib.rs as of "Add math library".
    let i = app.data.log.iter().position(|c| c.subject == "Add math library").unwrap();
    app.set_selected(Screen::Log, i);
    crate::views::diff::load_for_selection(&mut app);
    app.settle().await;
    press(&mut app, KeyCode::Enter).await;
    press(&mut app, KeyCode::Char('G')).await; // bottom of the diff = inside lib.rs
    press(&mut app, KeyCode::Char('B')).await;
    let Modal::Blame(v) = &app.modal else { panic!("blame not open") };
    assert_eq!(v.path, "lib.rs");
    assert_eq!(v.blame.lines.len(), 3);
    render(&mut app, 130, 30);
}

#[tokio::test]
async fn worktrees_view() {
    // Put the repo in a subfolder so sibling worktrees land inside our temp dir.
    let outer = TempDir::new().unwrap();
    let repo = outer.path().join("app");
    std::fs::create_dir(&repo).unwrap();
    sh(&repo, "git init -q -b main && git config user.name T && git config user.email t@t.io && git commit -q --allow-empty -m init");
    let mut app = app_for(&repo).await;
    press(&mut app, KeyCode::Char('4')).await;
    for _ in 0..3 {
        press(&mut app, KeyCode::Char(']')).await;
    }
    let s = render(&mut app, 130, 24);
    assert!(s.contains("Worktrees") && s.contains("1 worktrees"), "{s}");

    press(&mut app, KeyCode::Char('n')).await;
    chars(&mut app, "fix/login").await;
    press(&mut app, KeyCode::Enter).await;
    let wt = outer.path().join("app-fix-login");
    assert!(wt.join(".git").exists(), "worktree folder created");
    assert_eq!(app.data.worktrees.len(), 2);
    let s = render(&mut app, 130, 24);
    assert!(s.contains("fix/login"), "{s}");

    // Open it in Canopy, then back to the main one.
    app.set_selected(Screen::Branches, 1);
    press(&mut app, KeyCode::Enter).await;
    assert_eq!(app.git.as_ref().unwrap().repo.root.canonicalize().unwrap(), wt.canonicalize().unwrap());
    assert_eq!(app.current_branch(), Some("fix/login"));
    app.open_repo(repo.clone());
    app.settle().await;

    // Remove it (safe remove).
    press(&mut app, KeyCode::Char('4')).await;
    app.refs_view = crate::keymap::RefsView::Worktrees;
    app.set_selected(Screen::Branches, 1);
    press(&mut app, KeyCode::Char('d')).await;
    press(&mut app, KeyCode::Char('d')).await;
    assert!(!wt.exists());
    assert_eq!(app.data.worktrees.len(), 1);
}

#[test]
fn worktree_folder_names() {
    use std::path::Path;
    assert_eq!(crate::input::worktree_path(Path::new("/code/app"), "fix/login"), Path::new("/code/app-fix-login"));
}

#[tokio::test]
async fn bisect_flow_finds_culprit() {
    let dir = TempDir::new().unwrap();
    sh(
        dir.path(),
        r#"git init -q -b main && git config user.name T && git config user.email t@t.io &&
           for i in 1 2 3 4 5 6 7 8; do
             if [ $i -ge 5 ]; then echo "$i BROKEN" > f.txt; else echo $i > f.txt; fi
             git add f.txt && git commit -qm "c$i"
           done"#,
    );
    let mut app = app_for(dir.path()).await;
    press(&mut app, KeyCode::Char('3')).await;
    press(&mut app, KeyCode::Char('G')).await; // oldest commit: c1 (known good)
    press(&mut app, KeyCode::Char('b')).await;
    let s = render(&mut app, 120, 30);
    assert!(s.contains("Start bisect?") && s.contains("c1"), "{s}");
    press(&mut app, KeyCode::Char('y')).await;
    let s = render(&mut app, 120, 30);
    assert!(s.contains("BISECT IN PROGRESS") && s.contains("steps left"), "{s}");

    for _ in 0..6 {
        if matches!(app.bisect, Some(canopy_git::parse::bisect::BisectStep::Found { .. })) {
            break;
        }
        let broken = std::fs::read_to_string(dir.path().join("f.txt")).unwrap().contains("BROKEN");
        press(&mut app, KeyCode::Char('b')).await; // open the menu
        press(&mut app, KeyCode::Char(if broken { 'b' } else { 'g' })).await;
    }
    let s = render(&mut app, 120, 30);
    assert!(s.contains("Found it:") && s.contains("c5"), "{s}");
    press(&mut app, KeyCode::Char('f')).await;
    assert!(!dir.path().join(".git/BISECT_LOG").exists());
    assert_eq!(app.screen, Screen::Log);
    assert_eq!(app.selected_commit().unwrap().subject, "c5");
    assert_eq!(app.current_branch(), Some("main"));
}

#[tokio::test]
async fn submodules_view() {
    let lib = TempDir::new().unwrap();
    sh(lib.path(), "git init -q -b main && git config user.name T && git config user.email t@t.io && echo v1 > lib.txt && git add -A && git commit -qm v1");
    let dir = TempDir::new().unwrap();
    sh(
        dir.path(),
        &format!(
            "git init -q -b main && git config user.name T && git config user.email t@t.io && \
             git -c protocol.file.allow=always submodule add -q {} vendor/lib && git commit -qm 'add lib'",
            lib.path().display()
        ),
    );
    let mut app = app_for(dir.path()).await;
    press(&mut app, KeyCode::Char('4')).await;
    for _ in 0..4 {
        press(&mut app, KeyCode::Char(']')).await;
    }
    let s = render(&mut app, 130, 24);
    assert!(s.contains("vendor/lib") && s.contains("✓ in sync"), "{s}");

    // Move the submodule; it shows as moved, then u restores it.
    sh(&dir.path().join("vendor/lib"), "echo v2 > lib.txt && git -c user.name=T -c user.email=t@t.io commit -qam v2");
    app.refresh();
    app.settle().await;
    let s = render(&mut app, 130, 24);
    assert!(s.contains("● moved") && s.contains("Press u"), "{s}");
    press(&mut app, KeyCode::Char('u')).await;
    let s = render(&mut app, 130, 24);
    assert!(s.contains("✓ in sync"), "{s}");

    // Open it in Canopy.
    press(&mut app, KeyCode::Enter).await;
    assert!(app.git.as_ref().unwrap().repo.root.ends_with("vendor/lib"));
}

// ------------------------------------------------------------------ GitHub

/// A fake `gh` that serves the canopy-gh fixtures and logs its arguments.
fn fake_gh(dir: &Path, logged_in: bool) -> std::path::PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let fixtures = concat!(env!("CARGO_MANIFEST_DIR"), "/../canopy-gh/tests/fixtures");
    let script = format!(
        r#"#!/bin/sh
echo "$@" >> "{log}"
case "$1 $2" in
  "auth status") {auth} ;;
  "repo view") echo '{{"nameWithOwner":"o/r","url":"https://github.com/o/r","defaultBranchRef":{{"name":"main"}}}}' ;;
  "api user") echo ada ;;
  "pr list") cat "{f}/pr_list.json" ;;
  "pr view") cat "{f}/pr_view.json" ;;
  "issue list") case "$*" in *"--state all"*) sed 's/"OPEN"/"CLOSED"/' "{f}/issue_list.json" ;; *) cat "{f}/issue_list.json" ;; esac ;;
  "issue view") cat "{f}/issue_view.json" ;;
  "run list") cat "{f}/run_list.json" ;;
  "release list") cat "{f}/release_list.json" ;;
  "release view") cat "{f}/release_view.json" ;;
  "api --method") case "$*" in *GET*notifications*) cat "{f}/notifications.json" ;; *GET*pulls*comments*) cat "{f}/review_comments.json" ;; *) : ;; esac ;;
  "run view") case "$*" in *--log-failed*) printf 'check (ubuntu)\tRun cargo test\t2026-09-26T00:32:32.4188517Z thread main panicked at src/lib.rs:10\n' ;; *--log*) printf 'lint\tRun clippy\t2026-09-26T00:30:00.1Z clippy is happy\ncheck (ubuntu-latest)\tUNKNOWN STEP\t2026-09-26T00:31:00.1Z ##[group]Run cargo test\ncheck (ubuntu-latest)\tUNKNOWN STEP\t2026-09-26T00:31:01.1Z thread main panicked at src/lib.rs:10\ncheck (ubuntu-latest)\tUNKNOWN STEP\t2026-09-26T00:31:02.1Z ##[error]Process completed with exit code 101.\n' ;; *) cat "{f}/run_jobs.json" ;; esac ;;
  "pr diff") printf 'diff --git a/csv.rs b/csv.rs\n--- a/csv.rs\n+++ b/csv.rs\n@@ -0,0 +1 @@\n+fn parse() {{}}\n' ;;
  *) : ;;
esac
"#,
        log = dir.join("gh-calls.log").display(),
        f = fixtures,
        auth = if logged_in { "exit 0" } else { "exit 1" },
    );
    let path = dir.join("fake-gh");
    std::fs::write(&path, script).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    path
}

async fn github_app(dir: &Path, gh: &Path) -> App {
    let git = canopy_git::Git::open(dir).await.unwrap();
    let config = Config { gh_program: Some(gh.display().to_string()), ..Config::default() };
    let mut app = App::new(Some(git), config, dir.to_path_buf());
    app.refresh();
    crate::github::detect(&mut app);
    app.settle().await;
    app
}

fn gh_calls(dir: &Path) -> Vec<String> {
    std::fs::read_to_string(dir.join("gh-calls.log")).unwrap_or_default().lines().map(String::from).collect()
}

#[tokio::test]
async fn pull_requests_tab() {
    let dir = demo_repo();
    let bin = TempDir::new().unwrap();
    let gh = fake_gh(bin.path(), true);
    let mut app = github_app(dir.path(), &gh).await;
    press(&mut app, KeyCode::Char('8')).await;
    let s = render(&mut app, 140, 34);
    assert!(s.contains("Pull requests · o/r · open · 2"), "{s}");
    assert!(
        s.contains("#12") && s.contains("Add CSV parser") && s.contains("✗ 1 failed") && s.contains("approved"),
        "{s}"
    );
    // Details panel: description, checks, review, comment.
    assert!(s.contains("ada wants to merge feature/parser into main"), "{s}");
    assert!(s.contains("Parses CSV") && s.contains("bob approved") && s.contains("Nice!"), "{s}");

    // D shows the diff under the details.
    press(&mut app, KeyCode::Char('D')).await;
    let s = render(&mut app, 140, 34);
    assert!(s.contains("csv.rs"), "{s}");

    // Filter cycles to "mine" and asks gh for --author @me.
    press(&mut app, KeyCode::Char('f')).await;
    assert!(gh_calls(bin.path()).iter().any(|l| l.starts_with("pr list") && l.contains("--author @me")));

    // Review → approve (message optional) → sends `gh pr review 12 --approve`.
    press(&mut app, KeyCode::Char('r')).await;
    press(&mut app, KeyCode::Char('a')).await;
    assert!(matches!(app.modal, Modal::Compose(_)));
    crate::input::handle_key(&mut app, KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL));
    app.settle().await;
    assert!(gh_calls(bin.path()).contains(&"pr review 12 --approve".to_string()), "{:?}", gh_calls(bin.path()));

    // Request changes needs a message.
    press(&mut app, KeyCode::Char('r')).await;
    press(&mut app, KeyCode::Char('x')).await;
    crate::input::handle_key(&mut app, KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL));
    assert!(matches!(app.modal, Modal::Compose(_)), "empty request-changes must not send");
    chars(&mut app, "Please add tests").await;
    crate::input::handle_key(&mut app, KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL));
    app.settle().await;
    assert!(gh_calls(bin.path()).contains(&"pr review 12 --request-changes --body Please add tests".to_string()));

    // Merge → squash.
    press(&mut app, KeyCode::Char('M')).await;
    let s = render(&mut app, 140, 34);
    assert!(s.contains("checks are failing"), "{s}");
    press(&mut app, KeyCode::Char('s')).await;
    assert!(gh_calls(bin.path()).contains(&"pr merge 12 --squash --delete-branch".to_string()));
    assert!(app.history.iter().any(|c| c == "gh pr merge 12 --squash --delete-branch"), "teach mode");

    // Checkout.
    press(&mut app, KeyCode::Char(' ')).await;
    assert!(gh_calls(bin.path()).contains(&"pr checkout 12".to_string()));
}

#[tokio::test]
async fn pr_line_comments() {
    use crate::views::diff::Row;
    let dir = demo_repo();
    let bin = TempDir::new().unwrap();
    let gh = fake_gh(bin.path(), true);
    let mut app = github_app(dir.path(), &gh).await;
    press(&mut app, KeyCode::Char('8')).await;

    press(&mut app, KeyCode::Char('D')).await;
    let s = render(&mut app, 140, 40);
    // The review comment sits under its line; the outdated one is left out.
    assert!(s.contains("bob · ") && s.contains("Should this handle quoted fields?"), "{s}");
    assert!(!s.contains("(outdated)"), "{s}");
    let v = app.diff.as_ref().unwrap();
    let line = v.rows.iter().position(|r| matches!(r, Row::Line(..))).unwrap();
    assert!(matches!(v.rows[line + 1], Row::Note(0, 0)), "note right under its line: {:?}", v.rows);

    // Enter focuses the diff; C on the added line comments on csv.rs:1.
    press(&mut app, KeyCode::Enter).await;
    app.diff.as_mut().unwrap().cursor = line;
    let s = render(&mut app, 140, 40);
    assert!(s.contains("comment on line"), "hint: {s}");
    press(&mut app, KeyCode::Char('C')).await;
    let s = render(&mut app, 140, 40);
    assert!(s.contains("Comment on csv.rs:1"), "{s}");
    crate::input::handle_key(&mut app, KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL));
    assert!(matches!(app.modal, Modal::Compose(_)), "empty comment must not send");
    chars(&mut app, "Quoted fields too").await;
    crate::input::handle_key(&mut app, KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL));
    app.settle().await;
    let want = "api --method POST repos/{owner}/{repo}/pulls/12/comments -f commit_id=4f2a9c1d0e7b -f path=csv.rs \
                -F line=1 -f side=RIGHT -f body=Quoted fields too";
    assert!(gh_calls(bin.path()).iter().any(|c| c == want), "{:?}", gh_calls(bin.path()));

    // On a note row (not a diff line) C asks for a line instead.
    app.focus = crate::keymap::Focus::Diff;
    app.diff.as_mut().unwrap().cursor = line + 1;
    press(&mut app, KeyCode::Char('C')).await;
    assert!(app.toast.as_ref().unwrap().text.contains("Move to a line"), "{:?}", app.toast.as_ref().map(|t| &t.text));
}

#[tokio::test]
async fn line_comment_needs_a_pr_diff() {
    let dir = demo_repo();
    let mut app = app_for(dir.path()).await;
    press(&mut app, KeyCode::Char('2')).await;
    press(&mut app, KeyCode::Enter).await;
    assert_eq!(app.focus, crate::keymap::Focus::Diff);
    press(&mut app, KeyCode::Char('C')).await;
    assert!(
        app.toast.as_ref().unwrap().text.contains("pull request's diff"),
        "{:?}",
        app.toast.as_ref().map(|t| &t.text)
    );
}

#[tokio::test]
async fn create_pr_needs_a_pushed_branch() {
    let dir = demo_repo();
    let bare = TempDir::new().unwrap();
    sh(bare.path(), "git init -q --bare -b main");
    let bin = TempDir::new().unwrap();
    let gh = fake_gh(bin.path(), true);
    sh(
        dir.path(),
        &format!(
            "git stash -q -u && git remote add origin {} && git push -q -u origin main && git switch -qc feat",
            bare.path().display()
        ),
    );
    let mut app = github_app(dir.path(), &gh).await;
    press(&mut app, KeyCode::Char('8')).await;
    press(&mut app, KeyCode::Char('n')).await;
    assert!(app.toast.as_ref().unwrap().text.contains("Push this branch first"));
    sh(dir.path(), "git push -q -u origin feat");
    app.refresh();
    app.settle().await;
    press(&mut app, KeyCode::Char('n')).await;
    let s = render(&mut app, 140, 34);
    assert!(s.contains("New pull request: feat → main"), "{s}");
    chars(&mut app, "Add feature").await;
    press(&mut app, KeyCode::Tab).await;
    chars(&mut app, "Why it matters").await;
    crate::input::handle_key(&mut app, KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL));
    app.settle().await;
    assert!(
        gh_calls(bin.path()).contains(&"pr create --title Add feature --body Why it matters --base main".to_string())
    );
}

#[tokio::test]
async fn github_setup_card_when_logged_out() {
    let dir = demo_repo();
    let bin = TempDir::new().unwrap();
    let gh = fake_gh(bin.path(), false);
    let mut app = github_app(dir.path(), &gh).await;
    press(&mut app, KeyCode::Char('8')).await;
    let s = render(&mut app, 120, 24);
    assert!(s.contains("not logged in to GitHub") && s.contains("gh auth login"), "{s}");
    // Actions explain instead of failing.
    press(&mut app, KeyCode::Char('n')).await;
    assert!(app.toast.as_ref().unwrap().text.contains("isn't set up"));
    // Nothing but detection was run.
    assert_eq!(gh_calls(bin.path()), vec!["auth status"]);
}

#[tokio::test]
async fn issues_tab() {
    let dir = demo_repo();
    let bin = TempDir::new().unwrap();
    let gh = fake_gh(bin.path(), true);
    let mut app = github_app(dir.path(), &gh).await;
    press(&mut app, KeyCode::Char('9')).await;
    let s = render(&mut app, 140, 30);
    assert!(
        s.contains("Issues · Notifications  o/r · open · 1") && s.contains("#7") && s.contains("Crash on empty repo"),
        "{s}"
    );
    assert!(s.contains("bug") && s.contains("crashes when you") && s.contains("ada commented"), "{s}");

    // New issue: title required.
    press(&mut app, KeyCode::Char('n')).await;
    crate::input::handle_key(&mut app, KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL));
    assert!(matches!(app.modal, Modal::Compose(_)), "no title -> stays open");
    chars(&mut app, "Dark mode").await;
    press(&mut app, KeyCode::Enter).await;
    assert!(
        gh_calls(bin.path()).contains(&"issue create --title Dark mode --body ".to_string()),
        "{:?}",
        gh_calls(bin.path())
    );

    // Comment, then close (confirmed).
    press(&mut app, KeyCode::Char('C')).await;
    chars(&mut app, "Same here").await;
    crate::input::handle_key(&mut app, KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL));
    app.settle().await;
    press(&mut app, KeyCode::Char('X')).await;
    press(&mut app, KeyCode::Char('y')).await;
    let calls = gh_calls(bin.path());
    assert!(calls.contains(&"issue comment 7 --body Same here".to_string()), "{calls:?}");
    assert!(calls.contains(&"issue close 7".to_string()), "{calls:?}");

    // "all" shows it closed; X then offers to reopen.
    press(&mut app, KeyCode::Char('f')).await; // assigned to me
    press(&mut app, KeyCode::Char('f')).await; // all
    assert_eq!(app.github.issues[0].state, "CLOSED");
    press(&mut app, KeyCode::Char('X')).await;
    let s = render(&mut app, 140, 30);
    assert!(s.contains("Reopen #7?"), "{s}");
    press(&mut app, KeyCode::Char('y')).await;
    assert!(gh_calls(bin.path()).contains(&"issue reopen 7".to_string()));
}

#[tokio::test]
async fn actions_tab() {
    let dir = demo_repo();
    let bin = TempDir::new().unwrap();
    let gh = fake_gh(bin.path(), true);
    let mut app = github_app(dir.path(), &gh).await;
    press(&mut app, KeyCode::Char('0')).await;
    let s = render(&mut app, 140, 30);
    assert!(s.contains("Actions · Releases  main · 2 · live"), "{s}");
    assert!(s.contains("✗") && s.contains("Add CSV parser") && s.contains("pull_request"), "{s}");
    // Failed run: jobs, failing step, and the cleaned log tail.
    assert!(s.contains("✗ check (ubuntu-latest)") && s.contains("✗ Run cargo test"), "{s}");
    assert!(s.contains("thread main panicked at src/lib.rs:10") && !s.contains("2026-09-26T00:32:32"), "{s}");
    assert!(gh_calls(bin.path()).iter().any(|l| l.starts_with("run list") && l.contains("--branch main")));

    press(&mut app, KeyCode::Char('R')).await;
    assert!(gh_calls(bin.path()).contains(&"run rerun 1001 --failed".to_string()), "{:?}", gh_calls(bin.path()));

    // The second run is still going: R explains instead of calling gh.
    press(&mut app, KeyCode::Char('j')).await;
    press(&mut app, KeyCode::Char('R')).await;
    assert!(app.toast.as_ref().unwrap().text.contains("Only failed runs"));

    press(&mut app, KeyCode::Char('f')).await;
    let last = gh_calls(bin.path()).into_iter().rfind(|l| l.starts_with("run list")).unwrap();
    assert!(!last.contains("--branch"), "{last}");
}

#[tokio::test]
async fn run_log_viewer() {
    let dir = demo_repo();
    let bin = TempDir::new().unwrap();
    let gh = fake_gh(bin.path(), true);
    let mut app = github_app(dir.path(), &gh).await;
    press(&mut app, KeyCode::Char('0')).await;
    press(&mut app, KeyCode::Char('L')).await;
    assert!(gh_calls(bin.path()).contains(&"run view 1001 --log".to_string()), "{:?}", gh_calls(bin.path()));
    let s = render(&mut app, 120, 30);
    assert!(s.contains("Log · CI #"), "{s}");
    // The failed job comes first, without timestamps or ##[…] markers.
    let failed = s.find("✗ check (ubuntu-latest)").expect(&s);
    let passed = s.find("✓ lint").expect(&s);
    assert!(failed < passed, "{s}");
    assert!(s.contains("── Run cargo test") && s.contains("Process completed with exit code 101."), "{s}");
    assert!(!s.contains("##[") && !s.contains("2026-09-26T00:31"), "{s}");
    let Modal::RunLog(v) = &app.modal else { panic!("log viewer open") };
    assert_eq!(v.lines[v.cursor].text, "Process completed with exit code 101.", "starts on the error");

    // / search, n/N between matches.
    press(&mut app, KeyCode::Char('/')).await;
    chars(&mut app, "clippy").await;
    let s = render(&mut app, 120, 30);
    assert!(s.contains("/clippy"), "{s}");
    press(&mut app, KeyCode::Enter).await;
    let s = render(&mut app, 120, 30);
    assert!(s.contains("1/2 matches"), "{s}");
    press(&mut app, KeyCode::Char('n')).await;
    let s = render(&mut app, 120, 30);
    assert!(s.contains("2/2 matches"), "{s}");
    press(&mut app, KeyCode::Esc).await;
    assert!(!app.modal.is_open());

    // A run that's still going has no full log yet.
    press(&mut app, KeyCode::Char('j')).await;
    press(&mut app, KeyCode::Char('L')).await;
    assert!(app.toast.as_ref().unwrap().text.contains("once the run finishes"));
}

#[tokio::test]
async fn tab_bar_fits_any_width() {
    let dir = demo_repo();
    let mut app = app_for(dir.path()).await;
    press(&mut app, KeyCode::Char('0')).await;
    let wide = render(&mut app, 160, 20);
    assert!(wide.contains("8 Pull requests") && wide.contains(" 0 Actions "), "{wide}");
    let mid = render(&mut app, 100, 20);
    assert!(mid.contains(" 8 PRs ") && mid.contains(" 0 CI "), "{mid}");
    let narrow = render(&mut app, 60, 20);
    let tabs = narrow.lines().find(|l| l.contains(" 0 CI ")).unwrap_or_default();
    assert!(tabs.contains(" 0 CI ") && tabs.contains(" 8 ") && !tabs.contains("PRs"), "{tabs}");
}

#[tokio::test]
async fn home_github_card() {
    let dir = demo_repo();
    let bin = TempDir::new().unwrap();
    let gh = fake_gh(bin.path(), true);
    let mut app = github_app(dir.path(), &gh).await;
    let s = render(&mut app, 150, 36);
    // The fake returns PR #12 (1 failing check, approved) for this branch,
    // and 2 PRs for "review requested".
    assert!(s.contains("GitHub · o/r") && s.contains("#12 Add CSV parser"), "{s}");
    assert!(s.contains("✗ 1 check(s) failing") && s.contains("✓ approved") && s.contains("2 review request(s)"), "{s}");
    assert!(s.contains("Checks are failing on #12") && s.contains("waiting for your review"), "{s}");
    let calls = gh_calls(bin.path());
    assert!(calls.iter().any(|l| l.contains("--head main")), "{calls:?}");

    // Logged out: a one-line hint, and no PR calls.
    let bin2 = TempDir::new().unwrap();
    let gh2 = fake_gh(bin2.path(), false);
    let mut app = github_app(dir.path(), &gh2).await;
    let s = render(&mut app, 150, 36);
    assert!(s.contains("Run gh auth login to connect GitHub"), "{s}");
    assert_eq!(gh_calls(bin2.path()), vec!["auth status"]);
}

// ------------------------------------------------------------ site export

/// Render the current frame as HTML: one `<span>` per run of same-styled cells.
fn frame_html(app: &mut App, w: u16, h: u16) -> String {
    use ratatui::style::{Color, Modifier};
    let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
    term.draw(|f| crate::ui::draw(f, app)).unwrap();
    let buf = term.backend().buffer().clone();
    // Panel borders use a subtle color that reads fine in a terminal but
    // fades in a scaled-down screenshot; lift it a little for the web.
    let border = app.theme.border;
    let hex = |c: Color| match c {
        c if c == border => Some("#43604a".to_string()),
        Color::Rgb(r, g, b) => Some(format!("#{r:02x}{g:02x}{b:02x}")),
        _ => None,
    };
    // Pin every non-ASCII glyph to one column so browser font fallback
    // can't push the box-drawing borders out of line.
    let esc = |s: &str| {
        let mut out = String::with_capacity(s.len());
        for c in s.chars() {
            match c {
                '&' => out.push_str("&amp;"),
                '<' => out.push_str("&lt;"),
                '>' => out.push_str("&gt;"),
                // These come from JetBrains Mono (the site loads them with a
                // `text=` font request), so they're already exactly one column.
                c if c.is_ascii() || ('\u{2500}'..='\u{259F}').contains(&c) || "·…✓✗●○•↑↓←→↵‹›—⌃⌥⇧".contains(c) => {
                    out.push(c)
                }
                c => out.push_str(&format!("<i class=\"g\">{c}</i>")),
            }
        }
        out
    };
    let mut out = String::new();
    for y in 0..h {
        let mut x = 0;
        let mut run = String::new();
        let mut run_style: Option<String> = None;
        let flush = |out: &mut String, run: &mut String, style: &Option<String>| {
            if run.is_empty() {
                return;
            }
            match style {
                Some(st) if !st.is_empty() => out.push_str(&format!("<span style=\"{st}\">{}</span>", esc(run))),
                _ => out.push_str(&esc(run)),
            }
            run.clear();
        };
        while x < w {
            let cell = &buf[(x, y)];
            let mut css = String::new();
            let (mut fg, mut bg) = (cell.fg, cell.bg);
            if cell.modifier.contains(Modifier::REVERSED) {
                std::mem::swap(&mut fg, &mut bg);
            }
            if let Some(c) = hex(fg) {
                css.push_str(&format!("color:{c};"));
            }
            if let Some(c) = hex(bg) {
                css.push_str(&format!("background:{c};"));
            }
            if cell.modifier.contains(Modifier::BOLD) {
                css.push_str("font-weight:700;");
            }
            if cell.modifier.contains(Modifier::DIM) {
                css.push_str("opacity:.6;");
            }
            if cell.modifier.contains(Modifier::UNDERLINED) {
                css.push_str("text-decoration:underline;");
            }
            if run_style.as_ref() != Some(&css) {
                flush(&mut out, &mut run, &run_style);
                run_style = Some(css);
            }
            let sym = cell.symbol();
            run.push_str(sym);
            // A wide glyph (emoji) covers the next cell too: skip its filler.
            x += if ratatui::text::Span::raw(sym).width() > 1 { 2 } else { 1 };
        }
        flush(&mut out, &mut run, &run_style);
        out.push('\n');
    }
    out
}

/// Regenerates the website's screenshots and key reference from the real app.
/// Fills `<!-- SCREEN:name -->…<!-- /SCREEN:name -->` and
/// `<!-- KEYS:START -->…<!-- KEYS:END -->` in every `docs/*.html`.
/// Run with: cargo test -p canopy-git-tui export_site_screens -- --ignored
#[tokio::test]
#[ignore]
async fn export_site_screens() {
    use std::collections::BTreeMap;
    let (w, h) = (118, 32);
    // Fixed, friendly paths so the screenshots read well and are stable.
    let base = Path::new("/tmp/canopy-demo");
    let _ = std::fs::remove_dir_all(base);
    let dir = base.join("acme-app");
    let bare = base.join("origin.git");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::create_dir_all(&bare).unwrap();
    demo_repo_at(&dir);
    sh(&bare, "git init -q --bare -b main");
    sh(&dir, &format!("git remote add origin {} && git push -q -u origin main 2>/dev/null; git -c user.name='Grace Hopper' -c user.email=g@h.io commit -q --allow-empty -m 'Document the parser API'", bare.display()));
    let bin = TempDir::new().unwrap();
    let gh = fake_gh(bin.path(), true);
    let mut app = github_app(&dir, &gh).await;
    let mut shots: BTreeMap<&str, (&str, String)> = BTreeMap::new();

    shots.insert("home", ("Home", frame_html(&mut app, w, h)));

    press(&mut app, KeyCode::Char('2')).await;
    let i = app.status_rows().iter().position(|r| app.data.status.files[r.file].path == "main.rs").unwrap();
    app.set_selected(Screen::Status, i);
    crate::views::diff::load_for_selection(&mut app);
    app.settle().await;
    press(&mut app, KeyCode::Enter).await;
    press(&mut app, KeyCode::Char('j')).await;
    press(&mut app, KeyCode::Char('j')).await;
    shots.insert("changes", ("Stage single lines", frame_html(&mut app, w, h)));
    press(&mut app, KeyCode::Esc).await;

    press(&mut app, KeyCode::Char('3')).await;
    shots.insert("history", ("History graph", frame_html(&mut app, w, h)));

    press(&mut app, KeyCode::Char('2')).await;
    let i = app.status_rows().iter().position(|r| app.data.status.files[r.file].path == "main.rs").unwrap();
    app.set_selected(Screen::Status, i);
    press(&mut app, KeyCode::Char('B')).await;
    shots.insert("blame", ("Blame", frame_html(&mut app, w, h)));
    press(&mut app, KeyCode::Esc).await;

    press(&mut app, KeyCode::Char('4')).await;
    shots.insert("refs", ("Branches", frame_html(&mut app, w, h)));

    press(&mut app, KeyCode::Char('6')).await;
    app.settle().await;
    shots.insert("workspace", ("Workspace", frame_html(&mut app, w, h)));

    press(&mut app, KeyCode::Char('8')).await;
    shots.insert("prs", ("Pull requests", frame_html(&mut app, w, h)));
    press(&mut app, KeyCode::Char('9')).await;
    shots.insert("issues", ("Issues", frame_html(&mut app, w, h)));
    press(&mut app, KeyCode::Char('0')).await;
    shots.insert("actions", ("CI runs", frame_html(&mut app, w, h)));

    press(&mut app, KeyCode::Char('1')).await;
    press(&mut app, KeyCode::Char(':')).await;
    chars(&mut app, "undo").await;
    shots.insert("palette", ("Command palette", frame_html(&mut app, w, h)));
    press(&mut app, KeyCode::Esc).await;

    // Conflicts need their own repository mid-merge.
    let cdir = base.join("merge").join("acme-app");
    std::fs::create_dir_all(&cdir).unwrap();
    demo_repo_at(&cdir);
    sh(
        &cdir,
        r#"
set -e
git stash -q -u
printf 'fn greet() {\n    println!("hi");\n}\n\nfn main() {\n    greet();\n}\n' > app.rs; git add app.rs; git commit -qm base
git switch -qc polite; printf 'fn greet() {\n    println!("Hello, friend!");\n}\n\nfn main() {\n    greet();\n}\n' > app.rs; git commit -qam polite
git switch -q main; printf 'fn greet() {\n    println!("Hey there");\n}\n\nfn main() {\n    greet();\n}\n' > app.rs; git commit -qam casual
git merge polite >/dev/null 2>&1 || true
"#,
    );
    let mut capp = app_for(&cdir).await;
    press(&mut capp, KeyCode::Char('2')).await;
    press(&mut capp, KeyCode::Enter).await;
    shots.insert("conflicts", ("Conflicts", frame_html(&mut capp, w, h)));

    // Fill every page.
    let docs = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs");
    let keys = keys_html();
    let mut filled = 0;
    let themes = themes_html();
    let wiki = format!("{docs}/wiki");
    let pages = std::fs::read_dir(docs).unwrap().chain(std::fs::read_dir(&wiki).unwrap());
    for entry in pages {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|e| e != "html") {
            continue;
        }
        let mut page = std::fs::read_to_string(&path).unwrap();
        for (name, (label, frame)) in &shots {
            let (open, close) = (format!("<!-- SCREEN:{name} -->"), format!("<!-- /SCREEN:{name} -->"));
            while let (Some(a), Some(b)) = (page.find(&open), page.find(&close)) {
                let pre = format!(
                    "<pre class=\"frame\" id=\"shot-{name}\" data-label=\"{label}\" role=\"img\" aria-label=\"Canopy: {label}\">{frame}</pre>"
                );
                // Temporarily mark as filled so the loop moves past it.
                page = format!(
                    "{}<!-- SCREEN-DONE:{name} -->{pre}<!-- /SCREEN-DONE:{name} -->{}",
                    &page[..a],
                    &page[b + close.len()..]
                );
                filled += 1;
            }
            page = page
                .replace(&format!("<!-- SCREEN-DONE:{name} -->"), &open)
                .replace(&format!("<!-- /SCREEN-DONE:{name} -->"), &close);
        }
        let (ts, te) = ("<!-- THEMES:START -->", "<!-- THEMES:END -->");
        if let (Some(a), Some(b)) = (page.find(ts), page.find(te)) {
            page = format!("{}{ts}{themes}{}", &page[..a], &page[b..]);
        }
        let (ks, ke) = ("<!-- KEYS:START -->", "<!-- KEYS:END -->");
        if let (Some(a), Some(b)) = (page.find(ks), page.find(ke)) {
            page = format!("{}{ks}{keys}{}", &page[..a], &page[b..]);
        }
        let missing: Vec<_> = page
            .match_indices("<!-- SCREEN:")
            .map(|(i, _)| &page[i..i + 30])
            .filter(|m| {
                let name = m.trim_start_matches("<!-- SCREEN:").split(' ').next().unwrap_or("");
                !shots.contains_key(name)
            })
            .collect();
        assert!(missing.is_empty(), "{}: unknown screens {missing:?}", path.display());
        std::fs::write(&path, page).unwrap();
    }
    let _ = std::fs::remove_dir_all(base);
    println!("filled {filled} screenshots and the key reference");
}

/// Theme swatches for the wiki, from the real theme definitions.
fn themes_html() -> String {
    use crate::theme::Theme;
    use ratatui::style::Color;
    let hex = |c: Color| match c {
        Color::Rgb(r, g, b) => format!("#{r:02x}{g:02x}{b:02x}"),
        _ => "transparent".into(),
    };
    let mut out = String::from("\n");
    for name in Theme::NAMES {
        let t = Theme::by_name(name);
        let chips = [
            ("background", t.bg),
            ("text", t.fg),
            ("accent", t.accent),
            ("highlight", t.accent_alt),
            ("added", t.added),
            ("removed", t.removed),
            ("modified", t.modified),
            ("commit", t.hash),
            ("branch", t.branch),
            ("remote", t.remote),
        ];
        out.push_str(&format!(
            "<h2 id=\"theme-{name}\">{name}</h2>\n<div class=\"theme-sample\" style=\"background:{bg};color:{fg};border-color:{border}\">\
             <span style=\"color:{accent};font-weight:700\">● main</span> <span style=\"color:{hash}\">f5be172</span> \
             <span style=\"color:{added}\">+ added line</span> <span style=\"color:{removed}\">- removed line</span> \
             <span style=\"color:{muted}\">3h ago</span></div>\n<div class=\"swatches\">",
            bg = hex(t.bg),
            fg = hex(t.fg),
            border = hex(t.border),
            accent = hex(t.accent),
            hash = hex(t.hash),
            added = hex(t.added),
            removed = hex(t.removed),
            muted = hex(t.muted),
        ));
        for (label, c) in chips {
            out.push_str(&format!(
                "<span class=\"swatch\"><i style=\"background:{}\"></i>{label}<code>{}</code></span>",
                hex(c),
                hex(c)
            ));
        }
        out.push_str("</div>\n");
    }
    out
}

/// Every config option must be documented on the wiki's Configuration page.
#[test]
fn every_config_option_is_documented() {
    let src = include_str!("config.rs");
    let start = src.find("pub struct Config {").unwrap();
    let body = &src[start..start + src[start..].find("\n}").unwrap()];
    let fields: Vec<&str> = body
        .lines()
        .filter_map(|l| l.trim().strip_prefix("pub "))
        .filter_map(|l| l.split(':').next())
        .filter(|f| !f.contains(' '))
        .collect();
    assert!(fields.len() >= 10, "{fields:?}");
    let doc = include_str!("../../../docs/wiki/config.html");
    for f in fields {
        let shown = if f == "keys" {
            "[keys]".to_string()
        } else if f == "custom_commands" {
            "[[custom_commands]]".to_string()
        } else {
            f.to_string()
        };
        assert!(doc.contains(&format!("<code>{shown}</code>")), "config option `{f}` isn't on docs/wiki/config.html");
    }
}

/// The key reference, one table per context, from the real key tables.
fn keys_html() -> String {
    use crate::keymap::{defaults, pretty_key, Ctx, Screen as S};
    let sections: [(&str, &str, Ctx); 16] = [
        ("k-global", "Everywhere", Ctx::Global),
        ("k-status", "Changes", Ctx::Screen(S::Status)),
        ("k-diff", "Diff panel (after ↵)", Ctx::Diff),
        ("k-conflict", "Conflict panel", Ctx::Conflict),
        ("k-log", "History", Ctx::Screen(S::Log)),
        ("k-branches", "Branches", Ctx::Screen(S::Branches)),
        ("k-tags", "Tags", Ctx::Tags),
        ("k-remotes", "Remotes", Ctx::Remotes),
        ("k-worktrees", "Worktrees", Ctx::Worktrees),
        ("k-submodules", "Submodules", Ctx::Submodules),
        ("k-stash", "Stash", Ctx::Screen(S::Stash)),
        ("k-workspace", "Workspace", Ctx::Screen(S::Workspace)),
        ("k-reflog", "Reflog", Ctx::Screen(S::Reflog)),
        ("k-pulls", "Pull requests", Ctx::Screen(S::Pulls)),
        ("k-issues", "Issues", Ctx::Screen(S::Issues)),
        ("k-runs", "Actions", Ctx::Screen(S::Runs)),
    ];
    let esc = |s: &str| s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
    let mut out = String::from("\n");
    for (id, title, ctx) in sections {
        out.push_str(&format!("<h2 id=\"{id}\">{title}</h2>\n<table class=\"ref\">\n"));
        for b in defaults(ctx) {
            let keys: Vec<String> = b.keys.iter().map(|k| format!("<kbd>{}</kbd>", esc(&pretty_key(k)))).collect();
            out.push_str(&format!(
                "<tr><td>{}</td><td>{}</td><td><code>{}</code></td></tr>\n",
                keys.join(" "),
                esc(b.action.label()),
                b.action.name()
            ));
        }
        out.push_str("</table>\n");
    }
    out
}

/// Every glyph Canopy draws must be one column wide in every monospace font,
/// or borders drift out of line. Allowed: ASCII, box drawing, block
/// elements (sparklines) and a short list of widely supported symbols.
#[tokio::test]
async fn only_single_width_glyphs() {
    // ⌃⌥⇧ only appear with Mac key symbols, and every macOS font has them.
    const SAFE: &str = "·…✓✗●○•↑↓←→↵‹›—▌⌃⌥⇧";
    let ok = |c: char| c.is_ascii() || ('\u{2500}'..='\u{259F}').contains(&c) || SAFE.contains(c);
    let dir = demo_repo();
    let bin = TempDir::new().unwrap();
    let gh = fake_gh(bin.path(), true);
    let mut app = github_app(dir.path(), &gh).await;
    let mut bad = std::collections::BTreeSet::new();
    let check = |s: String, bad: &mut std::collections::BTreeSet<char>| {
        bad.extend(s.chars().filter(|c| !ok(*c) && *c != '\n'));
    };
    for key in ['1', '2', '3', '4', '5', '6', '7', '8', '9', '0'] {
        press(&mut app, KeyCode::Char(key)).await;
        check(render(&mut app, 140, 34), &mut bad);
        press(&mut app, KeyCode::Enter).await;
        check(render(&mut app, 140, 34), &mut bad);
        press(&mut app, KeyCode::Esc).await;
    }
    for key in ['?', ':', 'c'] {
        press(&mut app, KeyCode::Char('2')).await;
        press(&mut app, KeyCode::Char(key)).await;
        check(render(&mut app, 140, 34), &mut bad);
        press(&mut app, KeyCode::Esc).await;
    }
    for _ in 0..4 {
        press(&mut app, KeyCode::Char('4')).await;
        press(&mut app, KeyCode::Char(']')).await;
        check(render(&mut app, 140, 34), &mut bad);
    }
    assert!(bad.is_empty(), "risky glyphs on screen: {bad:?}");
}

#[tokio::test]
async fn modifier_shortcuts() {
    let dir = demo_repo();
    let key = |c, m| KeyEvent::new(c, m);

    // alt-<n> jumps tabs; alt-left/right cycle.
    let mut app = app_for(dir.path()).await;
    crate::input::handle_key(&mut app, key(KeyCode::Char('3'), KeyModifiers::ALT));
    assert_eq!(app.screen, Screen::Log);
    crate::input::handle_key(&mut app, key(KeyCode::Right, KeyModifiers::ALT));
    assert_eq!(app.screen, Screen::Branches);
    // Option+2 in macOS Terminal types ™: still goes to tab 2.
    crate::input::handle_key(&mut app, key(KeyCode::Char('™'), KeyModifiers::NONE));
    assert_eq!(app.screen, Screen::Status);

    // ctrl-q quits even from inside the commit dialog…
    press(&mut app, KeyCode::Char('c')).await;
    assert!(matches!(app.modal, Modal::Commit { .. }));
    crate::input::handle_key(&mut app, key(KeyCode::Char('q'), KeyModifiers::CONTROL));
    assert!(app.should_quit);

    // …but typing œ in a message is just text.
    let mut app = app_for(dir.path()).await;
    press(&mut app, KeyCode::Char('c')).await;
    crate::input::handle_key(&mut app, key(KeyCode::Char('œ'), KeyModifiers::NONE));
    assert!(!app.should_quit);
    let Modal::Commit { subject, .. } = &app.modal else { panic!() };
    assert_eq!(subject.text(), "œ");

    // Outside dialogs, Option+Q (œ) quits.
    press(&mut app, KeyCode::Esc).await;
    crate::input::handle_key(&mut app, key(KeyCode::Char('œ'), KeyModifiers::NONE));
    assert!(app.should_quit);

    // The hint bar shows how to quit.
    let mut app = app_for(dir.path()).await;
    let s = render(&mut app, 160, 24);
    assert!(s.contains(" q  quit"), "{s}");
}

#[tokio::test]
async fn logo_on_welcome_and_no_repo() {
    let dir = demo_repo();
    let mut app = app_for(dir.path()).await;
    app.modal = Modal::Welcome;
    let s = render(&mut app, 100, 30);
    assert!(s.contains("▄") && s.contains("Welcome to Canopy"), "{s}");
    let line = s.lines().find(|l| l.contains("Welcome to Canopy")).unwrap();
    assert!(line.contains('█') || line.contains('▀'), "tree beside the title: {line}");

    let mut app = App::new(None, Config::default(), dir.path().to_path_buf());
    app.screen = Screen::Home;
    let s = render(&mut app, 100, 30);
    assert!(s.contains("██") && s.contains("No repository open"), "{s}");
}

#[tokio::test]
async fn spacious_layout_fits_80x24() {
    let dir = demo_repo();
    let mut app = app_for(dir.path()).await;
    for key in ['1', '2', '3', '4'] {
        press(&mut app, KeyCode::Char(key)).await;
        let s = render(&mut app, 80, 24);
        // The footer keys and the tab bar are always visible.
        assert!(s.lines().take(4).any(|l| l.contains("1 Home") || l.contains(" 1 ")), "tabs:\n{s}");
        assert!(s.lines().last().unwrap().contains("help"), "footer:\n{s}");
    }
    press(&mut app, KeyCode::Char('2')).await;
    let s = render(&mut app, 80, 24);
    assert!(s.contains("main.rs") && s.contains("TODO.md"), "{s}");
    press(&mut app, KeyCode::Char('3')).await;
    let s = render(&mut app, 80, 24);
    assert!(s.contains("Add CSV parser"), "{s}");
}

#[tokio::test]
async fn compact_restores_dense_layout() {
    let dir = demo_repo();
    let git = canopy_git::Git::open(dir.path()).await.unwrap();
    let config = Config { compact: true, ..Config::default() };
    let mut app = App::new(Some(git), config, dir.path().to_path_buf());
    app.refresh();
    app.settle().await;
    let s = render(&mut app, 120, 30);
    // No blank row between the tabs and the first panel.
    assert!(s.lines().nth(2).unwrap().starts_with('╭'), "{s}");
}

#[tokio::test]
async fn startup_splash() {
    use std::time::{Duration, Instant};
    let dir = demo_repo();
    let mut app = app_for(dir.path()).await;
    let at = |ms: u64| Instant::now().checked_sub(Duration::from_millis(ms)).unwrap();

    // Just started: nothing grown yet, no wordmark.
    app.splash_start = Some(at(0));
    let s = render(&mut app, 100, 30);
    assert!(!s.contains("canopy") && !s.contains('█'), "{s}");
    assert!(s.contains("any key to skip"), "{s}");

    // Part-way: the trunk is up, the canopy isn't, still no wordmark.
    app.splash_start = Some(at(200));
    let s = render(&mut app, 100, 30);
    let tree_rows: Vec<&str> = s.lines().filter(|l| l.contains('█') || l.contains('▀') || l.contains('▄')).collect();
    assert!(!tree_rows.is_empty() && tree_rows.len() < 8, "{s}");

    // Finished: whole tree, "canopy", tagline and version.
    app.splash_start = Some(at(1000));
    let s = render(&mut app, 100, 30);
    assert!(s.contains("canopy") && s.contains("a git dashboard for your terminal"), "{s}");
    assert!(s.contains(&format!("v{}", env!("CARGO_PKG_VERSION"))), "{s}");

    // Any key skips it and does nothing else.
    press(&mut app, KeyCode::Char('3')).await;
    assert!(app.splash_start.is_none());
    assert_eq!(app.screen, Screen::Home);

    // Too small a terminal: straight to the app.
    app.splash_start = Some(at(0));
    let s = render(&mut app, 40, 12);
    assert!(!s.contains("any key to skip"), "{s}");
}

/// Writes one real frame as a standalone HTML page, for screenshots.
/// CANOPY_SHOT=path/to/out.html cargo test -p canopy-git-tui screenshot_frame -- --ignored
#[tokio::test]
#[ignore]
async fn screenshot_frame() {
    let Ok(out) = std::env::var("CANOPY_SHOT") else { return };
    let base = Path::new("/tmp/canopy-shot");
    let _ = std::fs::remove_dir_all(base);
    let dir = base.join("acme-app");
    std::fs::create_dir_all(&dir).unwrap();
    demo_repo_at(&dir);
    let bin = TempDir::new().unwrap();
    let gh = fake_gh(bin.path(), true);
    let mut app = github_app(&dir, &gh).await;
    app.history.push("git add -- lib.rs".into());
    let (w, h) = (120, 36);
    let frame = frame_html(&mut app, w, h);
    let page = format!(
        "<!doctype html><meta charset=utf-8><style>html,body{{margin:0;background:#101612}}\
         pre{{margin:0;padding:18px 22px;font:14px/1.2 Menlo,monospace;color:#d6e2d6;background:#101612;white-space:pre}}\
         i.g{{font-style:normal;display:inline-block;width:1ch;text-align:center}}</style><pre>{frame}</pre>"
    );
    std::fs::write(&out, page).unwrap();
    let _ = std::fs::remove_dir_all(base);
}

#[tokio::test]
async fn notifications_view() {
    let dir = demo_repo();
    let bin = TempDir::new().unwrap();
    let gh = fake_gh(bin.path(), true);
    let mut app = github_app(dir.path(), &gh).await;
    press(&mut app, KeyCode::Char('9')).await;
    press(&mut app, KeyCode::Char(']')).await;
    let s = render(&mut app, 150, 30);
    assert!(s.contains("Notifications") && s.contains("2 unread"), "{s}");
    assert!(s.contains("Add CSV parser") && s.contains("review requested"), "{s}");
    assert!(s.contains("Why you got this: your review was requested."), "{s}");
    // The Issues tab shows the unread count.
    assert!(s.contains("9 Issues 2"), "{s}");

    press(&mut app, KeyCode::Char('m')).await;
    press(&mut app, KeyCode::Char('M')).await;
    press(&mut app, KeyCode::Char('f')).await; // all repos
    press(&mut app, KeyCode::Char('u')).await; // include read
    let calls = gh_calls(bin.path());
    assert!(calls.contains(&"api --method PATCH notifications/threads/101".to_string()), "{calls:?}");
    assert!(
        calls.contains(&"api --method PUT repos/{owner}/{repo}/notifications -F read=true".to_string()),
        "{calls:?}"
    );
    assert!(calls.iter().any(|c| c.starts_with("api --method GET notifications -f all=true")), "{calls:?}");

    // [ goes back to issues.
    press(&mut app, KeyCode::Char('[')).await;
    let s = render(&mut app, 150, 30);
    assert!(s.contains("Crash on empty repo") && s.contains("Issues · Notifications"), "{s}");
}

#[tokio::test]
async fn releases_view() {
    let dir = demo_repo();
    let bin = TempDir::new().unwrap();
    let gh = fake_gh(bin.path(), true);
    let mut app = github_app(dir.path(), &gh).await;
    press(&mut app, KeyCode::Char('0')).await;
    press(&mut app, KeyCode::Char(']')).await;
    let s = render(&mut app, 150, 30);
    assert!(s.contains("Actions · Releases  3 releases"), "{s}");
    assert!(s.contains("v1.2.0") && s.contains("latest") && s.contains("pre-release") && s.contains("draft"), "{s}");
    // Details: notes and downloads.
    assert!(s.contains("CSV parser by @ada") && s.contains("1.4 MB · 42"), "{s}");

    // n suggests the next tag after the latest published one.
    press(&mut app, KeyCode::Char('n')).await;
    let Modal::Input { input, .. } = &app.modal else { panic!("tag prompt") };
    assert_eq!(input.text(), "v1.2.1");
    press(&mut app, KeyCode::Enter).await;
    assert!(matches!(app.modal, Modal::Compose(_)), "compose after the tag");
    // Title is prefilled with the tag; leave notes empty to generate them.
    crate::input::handle_key(&mut app, KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL));
    app.settle().await;
    let calls = gh_calls(bin.path());
    assert!(calls.contains(&"release create v1.2.1 --title v1.2.1 --generate-notes".to_string()), "{calls:?}");
}
