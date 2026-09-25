//! Discover repositories under the workspace directories and summarise them.

use std::path::{Path, PathBuf};

use canopy_git::ops::LogQuery;
use canopy_git::Git;
use tokio::task::JoinSet;

use crate::config::{expand_tilde, Config};

#[derive(Debug, Clone)]
pub struct RepoSummary {
    pub path: PathBuf,
    pub name: String,
    pub branch: String,
    pub ahead: u32,
    pub behind: u32,
    pub has_upstream: bool,
    pub staged: usize,
    pub unstaged: usize,
    pub conflicts: usize,
    pub last_subject: String,
    pub last_time: i64,
    pub error: Option<String>,
}

impl RepoSummary {
    pub fn dirty(&self) -> usize {
        self.staged + self.unstaged + self.conflicts
    }
}

/// Directories to scan: from config, else the parent of the current repo, else cwd.
pub fn roots(config: &Config, fallback: &Path) -> Vec<PathBuf> {
    if config.workspace_dirs.is_empty() {
        vec![fallback.to_path_buf()]
    } else {
        config.workspace_dirs.iter().map(|d| expand_tilde(d)).collect()
    }
}

const SKIP: &[&str] = &["node_modules", "target", "vendor", "dist", "build", "Library"];

pub fn find_repos(root: &Path, depth: usize) -> Vec<PathBuf> {
    let mut found = Vec::new();
    walk(root, depth, &mut found);
    found.sort();
    found.dedup();
    found
}

fn walk(dir: &Path, depth: usize, found: &mut Vec<PathBuf>) {
    if dir.join(".git").exists() {
        found.push(dir.to_path_buf());
        return;
    }
    if depth == 0 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for e in entries.flatten() {
        let name = e.file_name();
        let name = name.to_string_lossy();
        if name.starts_with('.') || SKIP.contains(&name.as_ref()) {
            continue;
        }
        if e.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            walk(&e.path(), depth - 1, found);
        }
    }
}

pub async fn scan(roots: Vec<PathBuf>, depth: usize) -> Vec<RepoSummary> {
    let paths: Vec<PathBuf> =
        tokio::task::spawn_blocking(move || roots.iter().flat_map(|r| find_repos(r, depth)).collect())
            .await
            .unwrap_or_default();

    let mut set = JoinSet::new();
    for p in paths {
        set.spawn(summarise(p));
    }
    let mut out = Vec::new();
    while let Some(r) = set.join_next().await {
        if let Ok(s) = r {
            out.push(s);
        }
    }
    // Most recently active first.
    out.sort_by(|a, b| b.last_time.cmp(&a.last_time).then(a.name.cmp(&b.name)));
    out
}

pub async fn summarise(path: PathBuf) -> RepoSummary {
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let mut s = RepoSummary {
        path: path.clone(),
        name,
        branch: String::new(),
        ahead: 0,
        behind: 0,
        has_upstream: false,
        staged: 0,
        unstaged: 0,
        conflicts: 0,
        last_subject: String::new(),
        last_time: 0,
        error: None,
    };
    let git = match Git::open(&path).await {
        Ok(g) => g,
        Err(e) => {
            s.error = Some(e.to_string());
            return s;
        }
    };
    let q = LogQuery { limit: 1, ..Default::default() };
    let (status, log) = tokio::join!(git.status(), git.log(&q));
    match status {
        Ok(st) => {
            s.branch = st.branch.head.clone().unwrap_or_else(|| "(detached)".into());
            s.ahead = st.branch.ahead;
            s.behind = st.branch.behind;
            s.has_upstream = st.branch.upstream.is_some();
            s.staged = st.staged().count();
            s.unstaged = st.unstaged().count() - st.conflicted().count();
            s.conflicts = st.conflicted().count();
        }
        Err(e) => s.error = Some(e.to_string()),
    }
    if let Some(c) = log.ok().and_then(|l| l.into_iter().next()) {
        s.last_subject = c.subject;
        s.last_time = c.time;
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_nested_repos_and_skips_noise() {
        let dir = tempfile::TempDir::new().unwrap();
        let root = dir.path();
        for p in ["a/.git", "b/c/.git", "node_modules/x/.git", ".hidden/y/.git", "a/inner/.git"] {
            std::fs::create_dir_all(root.join(p)).unwrap();
        }
        let found = find_repos(root, 3);
        let names: Vec<_> = found.iter().map(|p| p.strip_prefix(root).unwrap().to_path_buf()).collect();
        assert_eq!(names, vec![PathBuf::from("a"), PathBuf::from("b/c")]);
    }
}
