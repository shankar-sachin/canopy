//! Starting a repository in a folder that doesn't have one: `git init` with
//! starter files, then (optionally) a GitHub remote, over HTTPS or SSH, or a
//! brand-new GitHub repository made with gh.

use std::path::{Path, PathBuf};

use canopy_gh::Gh;
use canopy_git::init::{self, InitOptions, Protocol};
use canopy_git::Git;
use serde::Serialize;
use tauri::State;

use crate::actions::{done, err, Done};
use crate::{current, AppState, Res};

/// What the setup screen needs to know about a folder.
#[derive(Debug, Clone, Serialize)]
pub struct FolderInfo {
    pub path: String,
    pub name: String,
    pub exists: bool,
    pub is_repo: bool,
    /// Files already in the folder (they become part of the first commit).
    pub files: usize,
    pub default_branch: String,
    /// Your GitHub login, when gh is signed in.
    pub gh_login: Option<String>,
    pub suggested: Protocol,
    pub has_ssh_key: bool,
    pub https_help: &'static str,
    pub ssh_help: &'static str,
}

#[tauri::command]
pub async fn inspect_folder(path: String) -> FolderInfo {
    let p = PathBuf::from(&path);
    let (branch, suggested, login, repo) = tokio::join!(
        init::default_branch(),
        init::suggested_protocol(),
        async { Gh::new(&p, None).viewer().await.ok() },
        Git::open(&p),
    );
    FolderInfo {
        name: p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| path.clone()),
        exists: p.is_dir(),
        is_repo: repo.is_ok(),
        files: std::fs::read_dir(&p).map(|d| d.flatten().filter(|e| e.file_name() != ".DS_Store").count()).unwrap_or(0),
        default_branch: branch,
        gh_login: login,
        suggested,
        has_ssh_key: init::has_ssh_key(),
        https_help: Protocol::Https.explain(),
        ssh_help: Protocol::Ssh.explain(),
        path,
    }
}

/// Create the repository, and make it the open one.
#[tauri::command]
pub async fn init_repo(path: String, options: InitOptions, state: State<'_, AppState>) -> Res<Done> {
    let (git, cmds) = init::init(Path::new(&path), &options).await.map_err(|e| match e {
        canopy_git::GitError::Failed { stderr, .. } if stderr.contains("Please tell me who you are") => {
            "git doesn't know your name and email yet, so it can't commit. Set them in a terminal:\n\
             git config --global user.name \"Your Name\"\n\
             git config --global user.email \"you@example.com\""
                .to_string()
        }
        e => err(e),
    })?;
    if let Some(f) = crate::recent::store_path() {
        let _ = crate::recent::add(&f, &git.repo.root.display().to_string());
    }
    *state.git.lock().await = Some(git);
    Ok(Done { cmd: cmds.join("\n"), output: String::new() })
}

/// The remote's address for what was typed, or None if it isn't one.
#[tauri::command]
pub async fn remote_preview(input: String, protocol: Protocol, owner: Option<String>) -> Option<String> {
    init::remote_url(&input, protocol, owner.as_deref())
}

/// Add `url` as origin, then push the current branch there (when there's a
/// commit to push).
#[tauri::command]
pub async fn connect_remote(url: String, state: State<'_, AppState>) -> Res<Done> {
    connect(&current(&state).await?, &url).await
}

/// Point origin at `url` and push. Trying again after a failed push (the
/// repository wasn't on GitHub yet) updates origin instead of failing with
/// "remote origin already exists".
pub async fn connect(git: &Git, url: &str) -> Res<Done> {
    let mut cmds = vec![git.add_or_update_remote("origin", url).await.map_err(err)?.cmd];
    let status = git.status().await.map_err(err)?;
    if let (Some(branch), true) = (status.branch.head, git.has_head().await) {
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        match git.push("origin", &branch, true, false, tx).await {
            Ok(o) => cmds.push(o.cmd),
            Err(e) => {
                return Err(format!(
                    "Added the remote, but the push didn't go through. If the repository doesn't exist on GitHub yet, create it first (or use \"Create it on GitHub\"), then push.\n\n{}",
                    err(e)
                ))
            }
        }
    }
    Ok(Done { cmd: cmds.join("\n"), output: String::new() })
}

/// Create the repository on GitHub with gh, add it as origin, and push.
#[tauri::command]
pub async fn github_create(name: String, private: bool, state: State<'_, AppState>) -> Res<Done> {
    let git = current(&state).await?;
    let has_commit = git.has_head().await;
    let gh = Gh::new(&git.repo.root, None);
    let o = gh.repo_create(name.trim(), private, &git.repo.root, has_commit).await.map_err(|e| match e {
        canopy_gh::GhError::Failed { cmd, stderr } => format!("{cmd}\n{stderr}"),
        e => e.to_string(),
    })?;
    Ok(done(o))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sh(dir: &Path, args: &[&str]) {
        let ok = std::process::Command::new("git").current_dir(dir).args(args).status().unwrap().success();
        assert!(ok, "git {args:?}");
    }

    #[tokio::test]
    async fn connect_pushes_and_can_be_retried() {
        let tmp = tempfile::TempDir::new().unwrap();
        let (repo, remote) = (tmp.path().join("repo"), tmp.path().join("remote.git"));
        std::fs::create_dir_all(&repo).unwrap();
        std::fs::create_dir_all(&remote).unwrap();
        sh(&remote, &["init", "-q", "--bare", "-b", "main"]);
        sh(&repo, &["init", "-q", "-b", "main"]);
        sh(&repo, &["config", "user.name", "T"]);
        sh(&repo, &["config", "user.email", "t@t.io"]);
        sh(&repo, &["config", "commit.gpgsign", "false"]);
        let git = Git::open(&repo).await.unwrap();

        // No commit yet: origin is added, nothing is pushed.
        let missing = tmp.path().join("not-there.git");
        let d = connect(&git, missing.to_str().unwrap()).await.unwrap();
        assert!(d.cmd.starts_with("git remote add origin"), "{}", d.cmd);
        assert!(!d.cmd.contains("push"));

        // A commit, and a remote that doesn't exist: the push fails...
        std::fs::write(repo.join("f.txt"), "x\n").unwrap();
        sh(&repo, &["add", "."]);
        sh(&repo, &["commit", "-qm", "first"]);
        let e = connect(&git, missing.to_str().unwrap()).await.unwrap_err();
        assert!(e.contains("push didn't go through"), "{e}");

        // ...and trying again with the right address works.
        let d = connect(&git, remote.to_str().unwrap()).await.unwrap();
        assert!(d.cmd.contains("git remote set-url origin"), "{}", d.cmd);
        assert!(d.cmd.contains("git push"), "{}", d.cmd);
        assert_eq!(git.status().await.unwrap().branch.upstream.as_deref(), Some("origin/main"));
        assert_eq!(git.remotes().await.unwrap().len(), 1);
    }
}
