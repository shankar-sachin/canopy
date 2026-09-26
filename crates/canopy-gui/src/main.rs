//! Canopy Desktop: the Canopy dashboard in a window.
//!
//! The UI is plain HTML/CSS/JS in `ui/`. It calls the commands below with
//! `window.__TAURI__.core.invoke`; they wrap `canopy-git` exactly like the
//! TUI does, so both apps run the same git commands.

// No console window behind the app on Windows release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod actions;
mod github;
mod graph;
mod history;
mod overview;
mod recent;
mod settings;

use canopy_git::Git;
use tauri::State;
use tokio::sync::Mutex;

use overview::Overview;
use recent::RecentRepo;

#[derive(Default)]
pub(crate) struct AppState {
    git: Mutex<Option<Git>>,
    /// A repo path given on the command line, opened at startup.
    initial: Option<String>,
    /// `--smoke`: render Home, report what the page shows, and quit.
    smoke: bool,
}

pub(crate) type Res<T> = Result<T, String>;

pub(crate) async fn current(state: &AppState) -> Res<Git> {
    state.git.lock().await.clone().ok_or_else(|| "No repository is open".to_string())
}

#[tauri::command]
fn app_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[tauri::command]
fn initial_path(state: State<'_, AppState>) -> Option<String> {
    state.initial.clone()
}

#[tauri::command]
fn smoke_mode(state: State<'_, AppState>) -> bool {
    state.smoke
}

/// Called by the page in `--smoke` mode once it has rendered.
#[tauri::command]
fn smoke_report(app: tauri::AppHandle, ok: bool, text: String) {
    println!("smoke: {}\n{text}", if ok { "ok" } else { "FAILED" });
    app.exit(if ok { 0 } else { 1 });
}

#[tauri::command]
fn recent_repos() -> Vec<RecentRepo> {
    recent::store_path().map(|f| recent::list(&f)).unwrap_or_default()
}

#[tauri::command]
fn forget_repo(path: String) -> Res<()> {
    let Some(f) = recent::store_path() else { return Ok(()) };
    recent::remove(&f, &path).map_err(|e| e.to_string())
}

/// Show the system folder picker; `None` when cancelled.
#[tauri::command]
async fn pick_folder(app: tauri::AppHandle) -> Option<String> {
    use tauri_plugin_dialog::DialogExt;
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog().file().set_title("Open a repository").pick_folder(move |p| {
        let _ = tx.send(p);
    });
    let path = rx.await.ok().flatten()?.into_path().ok()?;
    Some(path.display().to_string())
}

#[tauri::command]
async fn open_repo(path: String, state: State<'_, AppState>) -> Res<Overview> {
    let git = Git::open(&path).await.map_err(|_| format!("{path} isn't inside a git repository"))?;
    let o = overview::load(&git).await.map_err(|e| e.to_string())?;
    if let Some(f) = recent::store_path() {
        let _ = recent::add(&f, &o.root);
    }
    *state.git.lock().await = Some(git);
    Ok(o)
}

#[tauri::command]
async fn close_repo(state: State<'_, AppState>) -> Res<()> {
    *state.git.lock().await = None;
    Ok(())
}

#[tauri::command]
async fn overview(state: State<'_, AppState>) -> Res<Overview> {
    let git = current(&state).await?;
    overview::load(&git).await.map_err(|e| e.to_string())
}

/// Apps started from Finder or a desktop launcher get a minimal PATH
/// (`/usr/bin:/bin:...`), so Homebrew's git and gh would be invisible. Take
/// the PATH your login shell sets up, and fall back to the usual folders.
#[cfg(unix)]
fn fix_path() {
    use std::time::{Duration, Instant};
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into());
    let from_shell = std::process::Command::new(&shell)
        .args(["-lc", "printf '%s' \"$PATH\""])
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .ok()
        .and_then(|mut child| {
            // Don't let a slow shell profile hold up the window.
            let start = Instant::now();
            while start.elapsed() < Duration::from_secs(3) {
                if let Ok(Some(_)) = child.try_wait() {
                    let mut out = String::new();
                    std::io::Read::read_to_string(child.stdout.as_mut()?, &mut out).ok()?;
                    return Some(out);
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            let _ = child.kill();
            None
        })
        .filter(|p| !p.trim().is_empty());
    let current = std::env::var("PATH").unwrap_or_default();
    let mut dirs: Vec<String> = from_shell.as_deref().unwrap_or("").split(':').map(String::from).collect();
    dirs.extend(current.split(':').map(String::from));
    dirs.extend(["/opt/homebrew/bin", "/usr/local/bin", "/home/linuxbrew/.linuxbrew/bin"].map(String::from));
    let mut seen = std::collections::HashSet::new();
    dirs.retain(|d| !d.is_empty() && seen.insert(d.clone()));
    std::env::set_var("PATH", dirs.join(":"));
}

fn main() {
    #[cfg(unix)]
    fix_path();
    let mut smoke = false;
    let mut path = None;
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--version" | "-V" => {
                println!("canopy-desktop {}", env!("CARGO_PKG_VERSION"));
                return;
            }
            "--smoke" => smoke = true,
            _ => path = Some(arg),
        }
    }
    let initial = path.map(|a| std::fs::canonicalize(&a).map(|p| p.display().to_string()).unwrap_or(a));

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState { initial, smoke, ..Default::default() })
        .invoke_handler(tauri::generate_handler![
            app_version,
            initial_path,
            smoke_mode,
            smoke_report,
            recent_repos,
            forget_repo,
            pick_folder,
            open_repo,
            close_repo,
            overview,
            actions::file_diff,
            actions::stage,
            actions::unstage,
            actions::stage_all,
            actions::unstage_all,
            actions::discard,
            actions::apply_lines,
            actions::commit,
            actions::last_message,
            actions::take_side,
            actions::op_continue,
            actions::op_abort,
            actions::sync,
            history::history,
            history::commit_details,
            history::refs,
            history::stash_diff,
            history::git_op,
            history::undo_info,
            history::undo,
            github::gh_status,
            github::gh_prs,
            github::gh_pr,
            github::gh_home,
            github::gh_issues,
            github::gh_issue,
            github::gh_runs,
            github::gh_run_jobs,
            github::gh_failed_log,
            github::gh_op,
            github::open_url,
            settings::get_settings,
            settings::set_settings,
            settings::clear_recent,
            settings::check_update,
            settings::environment,
            settings::profile,
            settings::account,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Canopy");
}
