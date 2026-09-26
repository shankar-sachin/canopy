use std::path::{Path, PathBuf};
use std::process::Stdio;

use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;

use crate::model::{Repo, RepoState};

#[derive(Debug, thiserror::Error)]
pub enum GitError {
    #[error("failed to run git: {0}")]
    Spawn(#[from] std::io::Error),
    #[error("`{cmd}` failed ({code}): {stderr}")]
    Failed { cmd: String, code: i32, stderr: String },
    #[error("not a git repository: {0}")]
    NotARepo(PathBuf),
    #[error("could not parse git output: {0}")]
    Parse(String),
}

pub type Result<T> = std::result::Result<T, GitError>;

#[derive(Debug, Clone)]
pub struct Output {
    /// The command as a user would type it, for teach mode.
    pub cmd: String,
    pub stdout: String,
    pub stderr: String,
}

/// Handle to a repository. Cheap to clone; all methods run `git -C <root>`.
#[derive(Debug, Clone)]
pub struct Git {
    pub repo: Repo,
}

/// Quote an argument for display only (never passed to a shell): POSIX sh
/// on macOS/Linux, PowerShell on Windows.
pub fn shell_quote(arg: &str) -> String {
    if cfg!(windows) {
        powershell_quote(arg)
    } else {
        posix_quote(arg)
    }
}

fn posix_quote(arg: &str) -> String {
    let safe = !arg.is_empty() && arg.chars().all(|c| c.is_ascii_alphanumeric() || "-_./=:@%+,^~".contains(c));
    if safe {
        arg.to_string()
    } else {
        format!("'{}'", arg.replace('\'', r"'\''"))
    }
}

/// PowerShell treats `,` `@` and more specially; inside '…' only `'` is,
/// and it doubles.
fn powershell_quote(arg: &str) -> String {
    let safe = !arg.is_empty() && arg.chars().all(|c| c.is_ascii_alphanumeric() || "-_./=:%+^~\\".contains(c));
    if safe {
        arg.to_string()
    } else {
        format!("'{}'", arg.replace('\'', "''"))
    }
}

pub fn display_cmd(args: &[&str]) -> String {
    display_program_cmd("git", args)
}

/// `program args…` as a user would type it, for teach mode.
pub fn display_program_cmd(program: &str, args: &[&str]) -> String {
    let mut s = String::from(program);
    for a in args {
        s.push(' ');
        s.push_str(&shell_quote(a));
    }
    s
}

impl Git {
    /// Discover the repository containing `path`.
    pub async fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let out = Command::new("git")
            .arg("-C")
            .arg(path)
            .args(["rev-parse", "--show-toplevel", "--absolute-git-dir"])
            .stdin(Stdio::null())
            .output()
            .await?;
        if !out.status.success() {
            return Err(GitError::NotARepo(path.to_path_buf()));
        }
        let text = String::from_utf8_lossy(&out.stdout);
        let mut lines = text.lines();
        let root = lines.next().ok_or_else(|| GitError::NotARepo(path.into()))?;
        let git_dir = lines.next().ok_or_else(|| GitError::NotARepo(path.into()))?;
        Ok(Git { repo: Repo { root: PathBuf::from(root), git_dir: PathBuf::from(git_dir) } })
    }

    pub(crate) fn command(&self, args: &[&str]) -> Command {
        let mut cmd = Command::new("git");
        cmd.arg("-C")
            .arg(&self.repo.root)
            // Stable, parseable output regardless of user config.
            .args(["-c", "color.ui=false", "-c", "core.quotepath=false"])
            .args(args)
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("GIT_OPTIONAL_LOCKS", "0")
            .env("LC_ALL", "C")
            .stdin(Stdio::null());
        cmd
    }

    /// Run git and return stdout; non-zero exit is an error.
    pub async fn run(&self, args: &[&str]) -> Result<Output> {
        let out = self.command(args).output().await?;
        let cmd = display_cmd(args);
        let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
        if !out.status.success() {
            return Err(GitError::Failed {
                cmd,
                code: out.status.code().unwrap_or(-1),
                stderr: if stderr.trim().is_empty() { stdout } else { stderr },
            });
        }
        Ok(Output { cmd, stdout, stderr })
    }

    /// Run git feeding `input` on stdin (used for `git apply --cached`).
    pub async fn run_with_stdin(&self, args: &[&str], input: &str) -> Result<Output> {
        use tokio::io::AsyncWriteExt;
        let mut child =
            self.command(args).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()?;
        let mut stdin = child.stdin.take().expect("piped stdin");
        let data = input.as_bytes().to_vec();
        let writer = tokio::spawn(async move {
            let _ = stdin.write_all(&data).await;
        });
        let out = child.wait_with_output().await?;
        let _ = writer.await;
        let cmd = display_cmd(args);
        let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
        if !out.status.success() {
            return Err(GitError::Failed { cmd, code: out.status.code().unwrap_or(-1), stderr });
        }
        Ok(Output { cmd, stdout, stderr })
    }

    /// Run a long operation (fetch/pull/push), streaming progress lines from
    /// stderr to `progress` as they arrive. Handles `\r`-separated updates.
    pub async fn run_streaming(&self, args: &[&str], progress: mpsc::UnboundedSender<String>) -> Result<Output> {
        let mut child = self.command(args).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()?;
        let stderr = child.stderr.take().expect("piped stderr");
        let stdout = child.stdout.take().expect("piped stdout");

        let err_task = tokio::spawn(async move {
            let mut reader = BufReader::new(stderr);
            let mut all = String::new();
            let mut buf = Vec::new();
            loop {
                buf.clear();
                // Split on either \r or \n to catch in-place progress updates.
                let mut byte = [0u8; 1];
                let mut got = false;
                loop {
                    match tokio::io::AsyncReadExt::read(&mut reader, &mut byte).await {
                        Ok(0) | Err(_) => break,
                        Ok(_) => {
                            got = true;
                            if byte[0] == b'\n' || byte[0] == b'\r' {
                                break;
                            }
                            buf.push(byte[0]);
                        }
                    }
                }
                if !got {
                    break;
                }
                let line = String::from_utf8_lossy(&buf).trim().to_string();
                if !line.is_empty() {
                    let _ = progress.send(line.clone());
                    all.push_str(&line);
                    all.push('\n');
                }
            }
            all
        });
        let out_task = tokio::spawn(async move {
            let mut reader = BufReader::new(stdout);
            let mut all = String::new();
            let mut line = String::new();
            while reader.read_line(&mut line).await.unwrap_or(0) > 0 {
                all.push_str(&line);
                line.clear();
            }
            all
        });

        let status = child.wait().await?;
        let stderr = err_task.await.unwrap_or_default();
        let stdout = out_task.await.unwrap_or_default();
        let cmd = display_cmd(args);
        if !status.success() {
            return Err(GitError::Failed { cmd, code: status.code().unwrap_or(-1), stderr });
        }
        Ok(Output { cmd, stdout, stderr })
    }

    /// Detect merge/rebase/etc. from state files in the git dir.
    pub fn state(&self) -> RepoState {
        let g = &self.repo.git_dir;
        if g.join("rebase-merge").exists() || g.join("rebase-apply").exists() {
            RepoState::Rebasing
        } else if g.join("MERGE_HEAD").exists() {
            RepoState::Merging
        } else if g.join("CHERRY_PICK_HEAD").exists() {
            RepoState::CherryPicking
        } else if g.join("REVERT_HEAD").exists() {
            RepoState::Reverting
        } else if g.join("BISECT_LOG").exists() {
            RepoState::Bisecting
        } else {
            RepoState::Clean
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quoting() {
        assert_eq!(posix_quote("main"), "main");
        assert_eq!(posix_quote("HEAD~1"), "HEAD~1");
        assert_eq!(posix_quote("fix bug"), "'fix bug'");
        assert_eq!(posix_quote("it's"), r"'it'\''s'");
        assert_eq!(powershell_quote("HEAD~1"), "HEAD~1");
        assert_eq!(powershell_quote(r"src\main.rs"), r"src\main.rs");
        assert_eq!(powershell_quote("a,b"), "'a,b'");
        assert_eq!(powershell_quote("@{u}"), "'@{u}'");
        assert_eq!(powershell_quote("it's"), "'it''s'");
        assert_eq!(display_cmd(&["commit", "-m", "hello world"]), "git commit -m 'hello world'");
    }
}
