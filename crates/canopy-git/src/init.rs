//! Starting a repository: `git init`, starter files, and building the
//! address of a remote from whatever the user pastes. Shared by the TUI and
//! the desktop app.

use std::path::Path;

use tokio::process::Command;

use crate::cli::{display_cmd, Git, GitError, Output, Result};

/// How git talks to the remote.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum Protocol {
    Https,
    Ssh,
}

impl Protocol {
    /// One-line explanation for a beginner choosing between the two.
    pub fn explain(self) -> &'static str {
        match self {
            Protocol::Https => {
                "Works everywhere and signs in through your browser or a token (gh auth login sets it up). Easiest to start with."
            }
            Protocol::Ssh => {
                "Uses an SSH key on this computer: no password prompts once the key is added to GitHub. Needs a key set up first."
            }
        }
    }
}

/// A `.gitignore` to start with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum Ignore {
    None,
    Rust,
    Node,
    Python,
    Macos,
}

impl Ignore {
    pub const ALL: [Ignore; 5] = [Ignore::None, Ignore::Rust, Ignore::Node, Ignore::Python, Ignore::Macos];

    pub fn label(self) -> &'static str {
        match self {
            Ignore::None => "No .gitignore",
            Ignore::Rust => "Rust",
            Ignore::Node => "Node / JavaScript",
            Ignore::Python => "Python",
            Ignore::Macos => "macOS only (.DS_Store)",
        }
    }

    pub fn contents(self) -> Option<&'static str> {
        const MAC: &str = "# macOS\n.DS_Store\n";
        match self {
            Ignore::None => None,
            Ignore::Rust => Some("# Rust build output\n/target\n\n# macOS\n.DS_Store\n"),
            Ignore::Node => Some("# Dependencies and builds\nnode_modules/\ndist/\n.env\nnpm-debug.log*\n\n# macOS\n.DS_Store\n"),
            Ignore::Python => {
                Some("# Python\n__pycache__/\n*.py[cod]\n.venv/\nvenv/\n.env\ndist/\nbuild/\n*.egg-info/\n\n# macOS\n.DS_Store\n")
            }
            Ignore::Macos => Some(MAC),
        }
    }
}

/// What to put in a new repository.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct InitOptions {
    pub branch: String,
    pub readme: bool,
    pub gitignore: Ignore,
    /// Add an MIT license with this year and name.
    pub mit_license: bool,
    /// Commit the starter files ("Initial commit").
    pub commit: bool,
}

impl Default for InitOptions {
    fn default() -> Self {
        InitOptions { branch: "main".into(), readme: true, gitignore: Ignore::None, mit_license: false, commit: true }
    }
}

/// The branch name git would use: `init.defaultBranch`, else main.
pub async fn default_branch() -> String {
    let out = Command::new("git").args(["config", "--global", "init.defaultBranch"]).output().await;
    out.ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|b| !b.is_empty())
        .unwrap_or_else(|| "main".into())
}

/// Whether `branch` is an acceptable branch name (the common rules of
/// `git check-ref-format --branch`).
pub fn valid_branch(branch: &str) -> bool {
    !branch.is_empty()
        && !branch.starts_with(['-', '/', '.'])
        && !branch.ends_with(['/', '.'])
        && !branch.ends_with(".lock")
        && !branch.contains("..")
        && !branch.contains("//")
        && !branch.contains("@{")
        && !branch.chars().any(|c| c.is_whitespace() || c.is_control() || "~^:?*[\\".contains(c))
}

fn readme(name: &str) -> String {
    format!("# {name}\n")
}

fn mit(year: i32, holder: &str) -> String {
    format!(
        "MIT License\n\nCopyright (c) {year} {holder}\n\n\
Permission is hereby granted, free of charge, to any person obtaining a copy\n\
of this software and associated documentation files (the \"Software\"), to deal\n\
in the Software without restriction, including without limitation the rights\n\
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell\n\
copies of the Software, and to permit persons to whom the Software is\n\
furnished to do so, subject to the following conditions:\n\n\
The above copyright notice and this permission notice shall be included in all\n\
copies or substantial portions of the Software.\n\n\
THE SOFTWARE IS PROVIDED \"AS IS\", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR\n\
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,\n\
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE\n\
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER\n\
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,\n\
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE\n\
SOFTWARE.\n"
    )
}

fn this_year() -> i32 {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    // Days since 1970 to a year, good enough for a copyright line.
    1970 + (secs as f64 / 31_556_952.0) as i32
}

/// `git init` in `dir` with the starter files, and optionally a first commit.
/// Existing files are never overwritten. Returns the repository and the
/// commands that ran.
pub async fn init(dir: &Path, opts: &InitOptions) -> Result<(Git, Vec<String>)> {
    if !valid_branch(&opts.branch) {
        return Err(GitError::Parse(format!("{:?} isn't a valid branch name", opts.branch)));
    }
    let mut cmds = Vec::new();
    std::fs::create_dir_all(dir)?;
    let args = ["init", "-b", opts.branch.as_str()];
    let out = Command::new("git").arg("-C").arg(dir).args(args).output().await?;
    if !out.status.success() {
        // Git before 2.28 has no -b: init, then point HEAD at the branch.
        let plain = Command::new("git").arg("-C").arg(dir).arg("init").output().await?;
        if !plain.status.success() {
            return Err(GitError::Failed {
                cmd: display_cmd(&["init"]),
                code: plain.status.code().unwrap_or(-1),
                stderr: String::from_utf8_lossy(&plain.stderr).into_owned(),
            });
        }
        let head = format!("refs/heads/{}", opts.branch);
        Command::new("git").arg("-C").arg(dir).args(["symbolic-ref", "HEAD", &head]).output().await?;
    }
    cmds.push(display_cmd(&args));
    let git = Git::open(dir).await?;

    let name = dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "project".into());
    let mut write = |file: &str, text: String| -> std::io::Result<()> {
        let path = dir.join(file);
        if !path.exists() {
            std::fs::write(path, text)?;
            cmds.push(format!("# created {file}"));
        }
        Ok(())
    };
    if opts.readme {
        write("README.md", readme(&name))?;
    }
    if let Some(text) = opts.gitignore.contents() {
        write(".gitignore", text.to_string())?;
    }
    if opts.mit_license {
        let holder = git.config_get("user.name").await.unwrap_or_else(|| "the authors".into());
        write("LICENSE", mit(this_year(), &holder))?;
    }
    if opts.commit {
        git.stage_all().await?;
        let has_files = !git.status().await?.files.is_empty();
        if has_files {
            let o: Output = git.commit("Initial commit", &Default::default()).await?;
            cmds.push(o.cmd);
        }
    }
    Ok((git, cmds))
}

/// The remote's address from what the user typed: `owner/name`, `name`
/// (with `default_owner`, like your GitHub login), or any GitHub URL, in the
/// chosen protocol. None when it can't be made sense of.
pub fn remote_url(input: &str, protocol: Protocol, default_owner: Option<&str>) -> Option<String> {
    let s = input.trim().trim_end_matches('/');
    let s = s.strip_suffix(".git").unwrap_or(s);
    // A URL for another host: keep it as it is.
    let path = if let Some(rest) = s.strip_prefix("git@github.com:") {
        rest.to_string()
    } else if let Some(rest) = s
        .strip_prefix("https://github.com/")
        .or_else(|| s.strip_prefix("http://github.com/"))
        .or_else(|| s.strip_prefix("github.com/"))
    {
        rest.to_string()
    } else if s.contains("://") || s.contains('@') {
        return Some(input.trim().to_string());
    } else if s.contains('/') {
        s.to_string()
    } else {
        format!("{}/{s}", default_owner?)
    };
    let mut parts = path.split('/');
    let (owner, repo) = (parts.next()?, parts.next()?);
    let ok = |p: &str| !p.is_empty() && p.chars().all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c));
    if parts.next().is_some() || !ok(owner) || !ok(repo) {
        return None;
    }
    Some(match protocol {
        Protocol::Https => format!("https://github.com/{owner}/{repo}.git"),
        Protocol::Ssh => format!("git@github.com:{owner}/{repo}.git"),
    })
}

/// Which protocol to suggest: gh's `git_protocol` setting, else SSH when
/// this computer has an SSH key, else HTTPS.
pub async fn suggested_protocol() -> Protocol {
    if let Ok(o) = Command::new("gh").args(["config", "get", "git_protocol"]).output().await {
        match String::from_utf8_lossy(&o.stdout).trim() {
            "ssh" => return Protocol::Ssh,
            "https" => return Protocol::Https,
            _ => {}
        }
    }
    if has_ssh_key() {
        Protocol::Ssh
    } else {
        Protocol::Https
    }
}

/// Whether ~/.ssh has a public key.
pub fn has_ssh_key() -> bool {
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"));
    let Some(dir) = home.map(|h| Path::new(&h).join(".ssh")) else { return false };
    std::fs::read_dir(dir)
        .map(|entries| entries.flatten().any(|e| e.file_name().to_string_lossy().ends_with(".pub")))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remote_urls() {
        use Protocol::*;
        let r = |i: &str, p| remote_url(i, p, Some("ada"));
        assert_eq!(r("ada/csvkit", Https).as_deref(), Some("https://github.com/ada/csvkit.git"));
        assert_eq!(r("ada/csvkit", Ssh).as_deref(), Some("git@github.com:ada/csvkit.git"));
        assert_eq!(r("csvkit", Ssh).as_deref(), Some("git@github.com:ada/csvkit.git"));
        assert_eq!(remote_url("csvkit", Ssh, None), None);
        // Full URLs are converted to the chosen protocol.
        assert_eq!(r("https://github.com/ada/csvkit.git", Ssh).as_deref(), Some("git@github.com:ada/csvkit.git"));
        assert_eq!(r("git@github.com:ada/csvkit.git", Https).as_deref(), Some("https://github.com/ada/csvkit.git"));
        assert_eq!(r("https://github.com/ada/csvkit/", Https).as_deref(), Some("https://github.com/ada/csvkit.git"));
        assert_eq!(r("github.com/ada/csvkit", Https).as_deref(), Some("https://github.com/ada/csvkit.git"));
        // Other hosts are kept as typed.
        assert_eq!(r("https://gitlab.com/ada/x.git", Ssh).as_deref(), Some("https://gitlab.com/ada/x.git"));
        // Nonsense.
        assert_eq!(r("ada/csv kit", Https), None);
        assert_eq!(r("a/b/c", Https), None);
        assert_eq!(r("", Https), None);
    }

    #[test]
    fn branch_names() {
        assert!(valid_branch("main") && valid_branch("feature/x") && valid_branch("v1.0"));
        for bad in ["", "-x", "a..b", "a b", "a~1", "x.lock", "x/", "@{u}", "a:b"] {
            assert!(!valid_branch(bad), "{bad}");
        }
    }

    #[tokio::test]
    async fn init_with_starter_files_and_commit() {
        let tmp = tempfile::TempDir::new().unwrap();
        let dir = tmp.path().join("new-project");
        let opts = InitOptions {
            branch: "trunk".into(),
            readme: true,
            gitignore: Ignore::Rust,
            mit_license: true,
            commit: true,
        };
        // (The commit uses git's global identity, which CI sets up.)
        let (git, cmds) = init(&dir, &opts).await.unwrap();
        assert!(cmds[0].starts_with("git init -b trunk"), "{cmds:?}");
        assert_eq!(std::fs::read_to_string(dir.join("README.md")).unwrap(), "# new-project\n");
        assert!(std::fs::read_to_string(dir.join(".gitignore")).unwrap().contains("/target"));
        assert!(std::fs::read_to_string(dir.join("LICENSE")).unwrap().starts_with("MIT License"));
        let st = git.status().await.unwrap();
        assert_eq!(st.branch.head.as_deref(), Some("trunk"));
        assert!(st.is_clean());
        let log = git.log(&Default::default()).await.unwrap();
        assert_eq!(log[0].subject, "Initial commit");

        // Existing files are kept; an empty repo without a commit works too.
        let other = tmp.path().join("existing");
        std::fs::create_dir_all(&other).unwrap();
        std::fs::write(other.join("README.md"), "mine\n").unwrap();
        let opts = InitOptions { commit: false, ..Default::default() };
        let (git, _) = init(&other, &opts).await.unwrap();
        assert_eq!(std::fs::read_to_string(other.join("README.md")).unwrap(), "mine\n");
        assert!(git.log(&Default::default()).await.unwrap().is_empty());
        assert!(init(&other, &InitOptions { branch: "bad name".into(), ..Default::default() }).await.is_err());
    }
}
