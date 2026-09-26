//! Canopy: a git dashboard for the terminal.

mod app;
mod config;
mod fuzzy;
mod github;
mod input;
mod keymap;
mod modal;
mod terminal;
mod textarea;
mod theme;
mod ui;
mod views;
mod workspace;

#[cfg(test)]
mod ui_tests;

use std::path::PathBuf;

use clap::Parser;

use crate::app::{App, Level};
use crate::config::Config;

#[derive(Parser, Debug)]
#[command(name = "canopy", version, about = "A beautiful, powerful git dashboard for your terminal")]
struct Cli {
    /// Repository to open (defaults to the current directory).
    path: Option<PathBuf>,

    /// Start in the Workspace view, scanning this directory for repositories.
    #[arg(short, long, value_name = "DIR")]
    workspace: Option<PathBuf>,

    /// Colour theme: canopy, catppuccin, gruvbox, nord, light.
    #[arg(short, long)]
    theme: Option<String>,

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
    if cli.path.is_some() && git.is_none() && cli.workspace.is_none() {
        anyhow::bail!("{} is not inside a git repository", start.display());
    }
    let workspace_root = match (&cli.workspace, &git) {
        (Some(w), _) => w.clone(),
        (None, Some(g)) => g.repo.root.parent().map(PathBuf::from).unwrap_or(cwd),
        (None, None) => cwd,
    };

    let mut app = App::new(git, config, workspace_root);
    if let Some(e) = config_err {
        app.toast(Level::Error, format!("Config error, using defaults: {e}"));
    }
    if first_run {
        app.modal = modal::Modal::Welcome;
    }

    let mut terminal = terminal::init();
    let res = app.run(&mut terminal).await;
    terminal::restore();
    res
}
