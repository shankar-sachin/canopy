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
    sh(
        dir.path(),
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
    dir
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
    assert!(s.contains("Merge branch 'feature/parser'"));
    assert!(s.contains("(⌂ v0.1.0)"));
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
    assert!(s.contains("◉"), "merge commit glyph:\n{s}");
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
    assert!(s.contains("Branches · Tags · Remotes") && s.contains("v0.1.0"), "{s}");
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
  "run view") case "$*" in *--log-failed*) printf 'check (ubuntu)\tRun cargo test\t2026-09-26T00:32:32.4188517Z thread main panicked at src/lib.rs:10\n' ;; *) cat "{f}/run_jobs.json" ;; esac ;;
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
    assert!(s.contains("Issues · o/r · open · 1") && s.contains("#7") && s.contains("Crash on empty repo"), "{s}");
    assert!(s.contains("bug") && s.contains("crashes when you press 3") && s.contains("ada commented"), "{s}");

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
    assert!(s.contains("Actions · main · 2 · live"), "{s}");
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
async fn tab_bar_fits_any_width() {
    let dir = demo_repo();
    let mut app = app_for(dir.path()).await;
    press(&mut app, KeyCode::Char('0')).await;
    let wide = render(&mut app, 160, 20);
    assert!(wide.contains("8 Pull requests") && wide.contains(" 0 Actions "), "{wide}");
    let mid = render(&mut app, 100, 20);
    assert!(mid.contains(" 8 PRs ") && mid.contains(" 0 CI "), "{mid}");
    let narrow = render(&mut app, 60, 20);
    let tabs = narrow.lines().nth(1).unwrap();
    assert!(tabs.contains(" 0 CI ") && tabs.contains(" 8 ") && !tabs.contains("PRs"), "{tabs}");
}
