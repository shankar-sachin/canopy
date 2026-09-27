//! Canopy for the terminal: git and GitHub, everything from the keyboard.

mod app;
mod config;
mod fuzzy;
mod github;
mod input;
mod keymap;
mod modal;
mod setup;
mod terminal;
mod textarea;
mod theme;
mod ui;
mod views;
mod workspace;

// The UI tests script git and a fake `gh` with shell scripts: Unix only.
#[cfg(all(test, unix))]
mod ui_tests;

use std::path::PathBuf;

use clap::Parser;

use crate::app::{App, Level};
use crate::config::Config;

#[derive(Parser, Debug)]
#[command(name = "canopy", version, about = "Canopy for the terminal: git and GitHub, everything from the keyboard")]
struct Cli {
    /// Repository to open (defaults to the current directory).
    path: Option<PathBuf>,

    /// Start in the Workspace view, scanning this directory for repositories.
    #[arg(short, long, value_name = "DIR")]
    workspace: Option<PathBuf>,

    /// Colour theme: canopy, catppuccin, gruvbox, nord, light, or a custom
    /// theme in ~/.canopy/themes (see --list-themes).
    #[arg(short, long)]
    theme: Option<String>,

    /// List the built-in and custom themes, then exit.
    #[arg(long)]
    list_themes: bool,

    /// Print a theme as JSON (to save in ~/.canopy/themes and edit),
    /// then exit.
    #[arg(long, value_name = "THEME")]
    export_theme: Option<String>,

    /// Print the settings Canopy Desktop can import (theme, editor, AI
    /// assistant, teach mode) as JSON, then exit.
    #[arg(long)]
    export_settings: bool,

    /// Take theme, editor, AI assistant and teach mode from a settings file
    /// exported by Canopy Desktop (or --export-settings), then exit.
    #[arg(long, value_name = "FILE")]
    import_settings: Option<PathBuf>,

    /// Print the path of the config file and exit.
    #[arg(long)]
    config_path: bool,

    /// Print an example config file and exit.
    #[arg(long)]
    example_config: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    if cli.config_path {
        println!("{}", Config::path().map(|p| p.display().to_string()).unwrap_or_default());
        return Ok(());
    }
    if cli.list_themes || cli.export_theme.is_some() || cli.export_settings || cli.import_settings.is_some() {
        return themes_and_settings(&cli);
    }
    if cli.example_config {
        print!("{}", Config::EXAMPLE);
        return Ok(());
    }
    if std::process::Command::new("git").arg("--version").output().is_err() {
        anyhow::bail!("git was not found on your PATH. Install it first (e.g. `brew install git`).");
    }

    let first_run = Config::first_run();
    let (mut config, config_err) = Config::load();
    if let Some(t) = cli.theme {
        config.theme = t;
    }

    let cwd = std::env::current_dir()?;
    let start = cli.path.clone().unwrap_or_else(|| cwd.clone());
    let git = if cli.workspace.is_some() { None } else { canopy_git::Git::open(&start).await.ok() };
    // A folder that isn't a repository: offer to create one (setup.rs).
    let setup = if git.is_none() && cli.workspace.is_none() {
        if cli.path.is_some() && !start.is_dir() {
            anyhow::bail!("{} is not a folder", start.display());
        }
        Some(setup::Setup::detect(start.clone()).await)
    } else {
        None
    };
    let workspace_root = match (&cli.workspace, &git) {
        (Some(w), _) => w.clone(),
        (None, Some(g)) => g.repo.root.parent().map(PathBuf::from).unwrap_or(cwd),
        (None, None) => cwd,
    };

    let mut app = App::new(git, config, workspace_root);
    if let Some(e) = config_err {
        app.toast(Level::Error, format!("Config error, using defaults: {e}"));
    }
    if setup.is_some() {
        app.setup = setup;
        setup::ask(&mut app);
    } else if first_run {
        app.modal = modal::Modal::Welcome;
    } else if app.config.desktop_tip && app.toast.is_none() {
        // Now and then, mention the desktop app (off with desktop_tip = false).
        let stamp = config::Config::path().and_then(|p| p.parent().map(|d| d.join("desktop-tip")));
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
        if stamp.is_some_and(|s| input::desktop_tip_due(&s, now)) {
            let key = app.keymap.key_for(&[keymap::Ctx::Global], keymap::Action::GetDesktop).map(keymap::pretty_key);
            let how = key.map_or("search \"desktop\" in the palette".to_string(), |k| format!("press {k}"));
            app.toast_for(
                app::Level::Info,
                format!("Prefer a window? Canopy Desktop is the friendly app (preview): {how}"),
                12,
            );
        }
    }

    let mut terminal = terminal::init();
    let res = app.run(&mut terminal).await;
    terminal::restore();
    res
}

/// --list-themes, --export-theme, --export-settings, --import-settings.
fn themes_and_settings(cli: &Cli) -> anyhow::Result<()> {
    use canopy_config::settings::Shared;
    use canopy_config::theme;
    let dir = theme::themes_dir();
    if cli.list_themes {
        println!("Built in:");
        for n in theme::builtin_names() {
            println!("  {n}");
        }
        let custom = dir.as_deref().map(theme::list_custom).unwrap_or_default();
        match &dir {
            Some(d) if !custom.is_empty() => {
                println!("Custom ({}):", d.display());
                for (path, t) in custom {
                    println!("  {}  ({})", t.name, path.file_name().unwrap_or_default().to_string_lossy());
                }
            }
            Some(d) => {
                println!("Custom: none yet. Save one with: canopy --export-theme canopy > {}/mine.json", d.display())
            }
            None => {}
        }
    }
    if let Some(name) = &cli.export_theme {
        let Some(p) = theme::find(name, dir.as_deref()) else {
            anyhow::bail!("no theme called {name:?} (see canopy --list-themes)");
        };
        let base = if theme::builtin_names().contains(&p.name.as_str()) { p.name.clone() } else { "canopy".into() };
        print!("{}", p.to_file(&p.name, &base).to_json());
    }
    let config_path = Config::path();
    let config_text = config_path.as_ref().and_then(|p| std::fs::read_to_string(p).ok()).unwrap_or_default();
    if cli.export_settings {
        let mut s = Shared::from_toml(&config_text).map_err(anyhow::Error::msg)?;
        // Carry a custom theme along so it works on another computer too.
        if let Some(name) = &s.theme {
            if !theme::builtin_names().contains(&name.as_str()) {
                s.theme_file = dir.as_deref().and_then(|d| theme::custom_file(name, d));
            }
        }
        print!("{}", s.to_json());
    }
    if let Some(file) = &cli.import_settings {
        let s = canopy_config::settings::read(file).map_err(anyhow::Error::msg)?;
        if let (Some(t), Some(d)) = (&s.theme_file, &dir) {
            let path = theme::save(d, t)?;
            println!("canopy: saved the theme {:?} to {}", t.name, path.display());
        }
        let Some(path) = config_path else { anyhow::bail!("couldn't find the config folder") };
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, s.apply_to_toml(&config_text))?;
        let mut what = Vec::new();
        if let Some(t) = &s.theme {
            what.push(format!("theme = {t}"));
        }
        if let Some(e) = &s.editor {
            what.push(format!("editor = {e}"));
        }
        if let Some(a) = &s.ai {
            what.push(format!("ai = {a}"));
        }
        if let Some(b) = s.show_commands {
            what.push(format!("teach_mode = {b}"));
        }
        println!(
            "canopy: updated {} ({})",
            path.display(),
            if what.is_empty() { "nothing to change".into() } else { what.join(", ") }
        );
    }
    Ok(())
}
