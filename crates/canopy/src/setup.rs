//! Starting a repository from the TUI: `canopy <folder>` on a folder that
//! isn't a repository asks whether to create one, sets it up (branch,
//! .gitignore, starter files, first commit), then optionally connects it to
//! GitHub: a new repository made with gh, or an existing one over HTTPS or
//! SSH (each explained).

use std::path::PathBuf;

use canopy_git::init::{self, Ignore, InitOptions, Protocol};

use crate::app::{App, Level, Msg, Then};
use crate::keymap::Screen;
use crate::modal::{InputKind, MenuItem, Modal, Pending};

/// What the setup has gathered so far.
#[derive(Debug, Clone)]
pub struct Setup {
    pub dir: PathBuf,
    pub branch: String,
    pub ignore: Ignore,
    /// Your GitHub login, when gh is signed in.
    pub gh_login: Option<String>,
    pub suggested: Protocol,
    /// Whether the first commit was made (there's something to push).
    pub committed: bool,
    /// What was typed for the remote.
    pub remote_input: String,
}

impl Setup {
    pub async fn detect(dir: PathBuf) -> Setup {
        let gh = canopy_gh::Gh::new(&dir, None);
        let (branch, suggested, login) = tokio::join!(init::default_branch(), init::suggested_protocol(), gh.viewer());
        Setup {
            dir,
            branch,
            ignore: Ignore::None,
            gh_login: login.ok().filter(|l| !l.is_empty()),
            suggested,
            committed: false,
            remote_input: String::new(),
        }
    }

    fn name(&self) -> String {
        self.dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "project".into())
    }
}

fn item(key: char, label: impl Into<String>, detail: impl Into<String>, pending: Pending) -> MenuItem {
    MenuItem { key, label: label.into(), detail: detail.into(), pending, danger: false }
}

/// Step 1: there's no repository here.
pub fn ask(app: &mut App) {
    let Some(s) = &app.setup else { return };
    app.modal = Modal::Menu {
        title: format!("No git repository in {}", s.dir.display()),
        items: vec![
            item(
                'c',
                "Create a repository here",
                "git init: git starts tracking this folder's history",
                Pending::SetupBranch,
            ),
            item('w', "Show repositories under this folder", "the Workspace view", Pending::SetupWorkspace),
            item('q', "Quit", "", Pending::SetupQuit),
        ],
        sel: 0,
    };
}

/// Run a setup step chosen from a menu.
pub fn step(app: &mut App, pending: Pending) {
    match pending {
        Pending::SetupBranch => {
            let branch = app.setup.as_ref().map(|s| s.branch.clone()).unwrap_or_else(|| "main".into());
            app.modal = Modal::input(
                "Branch name",
                "your main line of work; most projects call it main · enter to continue",
                &branch,
                InputKind::SetupBranch,
            );
        }
        Pending::SetupWorkspace => {
            app.setup = None;
            app.screen = Screen::Workspace;
            app.scan_workspace();
        }
        Pending::SetupQuit => app.should_quit = true,
        Pending::SetupIgnore(ignore) => {
            if let Some(s) = app.setup.as_mut() {
                s.ignore = ignore;
            }
            app.modal = Modal::Menu {
                title: "Start with which files?".into(),
                items: vec![
                    item(
                        'r',
                        "A README, and make the first commit",
                        "README.md says what the project is",
                        Pending::SetupFiles { license: false, commit: true },
                    ),
                    item(
                        'l',
                        "README + MIT license, and the first commit",
                        "the license lets others use your code",
                        Pending::SetupFiles { license: true, commit: true },
                    ),
                    item(
                        'e',
                        "Nothing yet: an empty repository",
                        "commit your own files later (c)",
                        Pending::SetupFiles { license: false, commit: false },
                    ),
                ],
                sel: 0,
            };
        }
        Pending::SetupFiles { license, commit } => create(app, license, commit),
        Pending::SetupGitHub => {
            let Some(s) = &app.setup else { return };
            let login = s.gh_login.clone().unwrap_or_default();
            app.modal = Modal::Menu {
                title: format!("Create github.com/{login}/{}", s.name()),
                items: vec![
                    item(
                        'p',
                        "Private",
                        "only you, and people you invite, can see it",
                        Pending::SetupGhCreate { private: true },
                    ),
                    item(
                        'u',
                        "Public",
                        "anyone can see it; only you can change it",
                        Pending::SetupGhCreate { private: false },
                    ),
                ],
                sel: 0,
            };
        }
        Pending::SetupGhCreate { private } => {
            let Some(s) = app.setup.take() else { return };
            let Some(git) = app.git.clone() else { return };
            let (name, push) = (s.name(), s.committed);
            app.run_op(format!("Create {name} on GitHub"), Then::GitHub, async move {
                canopy_gh::Gh::new(&git.repo.root, None).repo_create(&name, private, &git.repo.root, push).await
            });
        }
        Pending::SetupExisting => {
            let Some(s) = &app.setup else { return };
            let example = format!("{}/{}", s.gh_login.as_deref().unwrap_or("you"), s.name());
            app.modal = Modal::input(
                "Which repository?",
                format!("owner/name (like {example}) or paste its URL · make it on GitHub first, without a README"),
                "",
                InputKind::SetupRemote,
            );
        }
        Pending::SetupConnect(protocol) => connect(app, protocol),
        Pending::SetupSkip => {
            app.setup = None;
            app.toast(Level::Info, "You can add a remote any time: Branches → Remotes");
        }
        _ => {}
    }
}

/// Text typed into a setup input.
pub fn submit(app: &mut App, kind: InputKind, text: String) {
    match kind {
        InputKind::SetupBranch => {
            let branch = if text.is_empty() { "main".to_string() } else { text };
            if !init::valid_branch(&branch) {
                app.toast(Level::Warn, format!("{branch:?} can't be a branch name (no spaces, ~ ^ : ? * [ or ..)"));
                app.modal =
                    Modal::input("Branch name", "try again · enter to continue", &branch, InputKind::SetupBranch);
                return;
            }
            if let Some(s) = app.setup.as_mut() {
                s.branch = branch;
            }
            let items = Ignore::ALL
                .iter()
                .enumerate()
                .map(|(i, ig)| {
                    let detail = match ig {
                        Ignore::None => "you can add one later",
                        Ignore::Rust => "ignores /target",
                        Ignore::Node => "ignores node_modules/, dist/, .env",
                        Ignore::Python => "ignores __pycache__/, .venv/, .env",
                        Ignore::Macos => "ignores .DS_Store",
                    };
                    item(char::from(b'1' + i as u8), ig.label(), detail, Pending::SetupIgnore(*ig))
                })
                .collect();
            app.modal = Modal::Menu { title: ".gitignore: files git should leave alone".into(), items, sel: 0 };
        }
        InputKind::SetupRemote => {
            let Some(s) = app.setup.as_mut() else { return };
            s.remote_input = text;
            let (https, ssh) = (Protocol::Https, Protocol::Ssh);
            let tag = |p: Protocol| if s.suggested == p { " (suggested)" } else { "" };
            let ssh_note = if init::has_ssh_key() { "" } else { " No SSH key on this computer yet." };
            app.modal = Modal::Menu {
                title: "How should git connect to GitHub?".into(),
                items: vec![
                    item('h', format!("HTTPS{}", tag(https)), https.explain(), Pending::SetupConnect(https)),
                    item(
                        's',
                        format!("SSH{}", tag(ssh)),
                        format!("{}{ssh_note}", ssh.explain()),
                        Pending::SetupConnect(ssh),
                    ),
                ],
                sel: if s.suggested == ssh { 1 } else { 0 },
            };
        }
        _ => {}
    }
}

fn create(app: &mut App, license: bool, commit: bool) {
    let Some(s) = app.setup.as_mut() else { return };
    s.committed = commit;
    let opts =
        InitOptions { branch: s.branch.clone(), readme: commit, gitignore: s.ignore, mit_license: license, commit };
    let dir = s.dir.clone();
    app.busy = Some("Create repository".into());
    app.spawn(async move {
        let res = init::init(&dir, &opts).await.map_err(|e| match e {
            canopy_git::GitError::Failed { stderr, .. } if stderr.contains("Please tell me who you are") => {
                "git doesn't know your name and email yet. Run: git config --global user.name \"Your Name\" and git config --global user.email you@example.com".to_string()
            }
            e => e.to_string(),
        });
        Msg::SetupCreated(res)
    });
}

/// After `git init`: open the repository, then offer to connect it.
pub fn created(app: &mut App, res: Result<(canopy_git::Git, Vec<String>), String>) {
    app.busy = None;
    match res {
        Err(e) => {
            app.toast(Level::Error, format!("Couldn't create the repository: {e}"));
            ask(app);
        }
        Ok((git, cmds)) => {
            app.history.extend(cmds);
            app.set_repo(git);
            app.toast(Level::Success, "Created the repository");
            let Some(s) = &app.setup else { return };
            let mut items = Vec::new();
            if s.gh_login.is_some() {
                items.push(item('g', "Create it on GitHub", "with your gh login, then push", Pending::SetupGitHub));
            }
            items.push(item(
                'e',
                "Connect an existing GitHub repository",
                "by owner/name or URL, over HTTPS or SSH",
                Pending::SetupExisting,
            ));
            items.push(item('s', "Skip for now", "it stays on this computer", Pending::SetupSkip));
            app.modal =
                Modal::Menu { title: "Connect it to GitHub? (backs it up, lets you share it)".into(), items, sel: 0 };
        }
    }
}

fn connect(app: &mut App, protocol: Protocol) {
    let Some(s) = app.setup.take() else { return };
    let Some(git) = app.git.clone() else { return };
    let Some(url) = init::remote_url(&s.remote_input, protocol, s.gh_login.as_deref()) else {
        app.toast(
            Level::Warn,
            format!("{:?} doesn't look like a repository: type owner/name or paste its URL", s.remote_input),
        );
        app.setup = Some(s);
        step(app, Pending::SetupExisting);
        return;
    };
    let (branch, push) = (s.branch.clone(), s.committed);
    app.run_op(format!("Connect {url}"), Then::Refresh, async move {
        let added = git.add_remote("origin", &url).await?;
        if !push {
            return Ok(added);
        }
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        git.push("origin", &branch, true, false, tx).await
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    #[tokio::test]
    async fn walk_through_the_setup() {
        let tmp = tempfile::TempDir::new().unwrap();
        let dir = tmp.path().join("new-app");
        std::fs::create_dir_all(&dir).unwrap();
        let mut app = App::new(None, Config::default(), tmp.path().to_path_buf());
        app.setup = Some(Setup {
            dir: dir.clone(),
            branch: "main".into(),
            ignore: Ignore::None,
            gh_login: Some("ada".into()),
            suggested: Protocol::Https,
            committed: false,
            remote_input: String::new(),
        });
        ask(&mut app);
        assert!(matches!(&app.modal, Modal::Menu { title, .. } if title.starts_with("No git repository")));
        step(&mut app, Pending::SetupBranch);
        submit(&mut app, InputKind::SetupBranch, "bad name".into());
        assert!(matches!(app.modal, Modal::Input { kind: InputKind::SetupBranch, .. }));
        submit(&mut app, InputKind::SetupBranch, "trunk".into());
        assert!(matches!(&app.modal, Modal::Menu { title, .. } if title.starts_with(".gitignore")));
        step(&mut app, Pending::SetupIgnore(Ignore::Rust));
        step(&mut app, Pending::SetupFiles { license: true, commit: true });
        app.settle().await;
        let git = app.git.clone().expect("repository created");
        assert_eq!(git.status().await.unwrap().branch.head.as_deref(), Some("trunk"));
        assert!(dir.join(".gitignore").exists() && dir.join("LICENSE").exists() && dir.join("README.md").exists());
        // Offered: create on GitHub (gh is signed in), existing, skip.
        match &app.modal {
            Modal::Menu { items, .. } => assert_eq!(items.iter().map(|i| i.key).collect::<String>(), "ges"),
            _ => panic!("expected a menu"),
        }
        // An existing repository over SSH: bad input asks again.
        step(&mut app, Pending::SetupExisting);
        submit(&mut app, InputKind::SetupRemote, "not a repo name".into());
        step(&mut app, Pending::SetupConnect(Protocol::Ssh));
        assert!(matches!(app.modal, Modal::Input { kind: InputKind::SetupRemote, .. }));
        submit(&mut app, InputKind::SetupRemote, "ada/new-app".into());
        match &app.modal {
            Modal::Menu { items, .. } => assert!(items[0].label.contains("HTTPS (suggested)")),
            _ => panic!("expected a menu"),
        }
        step(&mut app, Pending::SetupConnect(Protocol::Ssh));
        app.settle().await;
        let remotes = git.remotes().await.unwrap();
        assert_eq!(remotes[0].fetch_url, "git@github.com:ada/new-app.git");
    }
}
