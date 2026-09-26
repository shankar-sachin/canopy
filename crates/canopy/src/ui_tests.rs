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

async fn chars(app: &mut App, s: &str) {
    for c in s.chars() {
        press(app, KeyCode::Char(c)).await;
    }
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
