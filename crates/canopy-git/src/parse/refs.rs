//! Branches (`for-each-ref`), tags, and remotes.

use super::FS;
use crate::cli::GitError;
use crate::model::{Branch, Remote, Tag};

/// Fields: HEAD marker, full refname, oid, upstream, track, time, subject.
pub const BRANCH_FORMAT: &str = "--format=%(HEAD)%1f%(refname)%1f%(objectname)%1f%(upstream:short)%1f%(upstream:track,nobracket)%1f%(committerdate:unix)%1f%(contents:subject)";

pub fn parse_branches(out: &str) -> Result<Vec<Branch>, GitError> {
    let mut branches = Vec::new();
    for line in out.lines().filter(|l| !l.is_empty()) {
        let f: Vec<&str> = line.splitn(7, FS).collect();
        if f.len() != 7 {
            return Err(GitError::Parse(format!("branch line: {line:?}")));
        }
        let (name, is_remote) = if let Some(n) = f[1].strip_prefix("refs/heads/") {
            (n, false)
        } else if let Some(n) = f[1].strip_prefix("refs/remotes/") {
            // Skip symbolic `origin/HEAD`.
            if n.ends_with("/HEAD") {
                continue;
            }
            (n, true)
        } else {
            continue;
        };
        branches.push(Branch {
            name: name.to_string(),
            is_remote,
            is_head: f[0] == "*",
            oid: f[2].to_string(),
            upstream: non_empty(f[3]),
            track: non_empty(f[4]),
            time: f[5].parse().unwrap_or(0),
            subject: f[6].to_string(),
        });
    }
    Ok(branches)
}

/// Parse `ahead N, behind M` track info into counts.
pub fn parse_track(track: &str) -> (u32, u32) {
    let (mut ahead, mut behind) = (0, 0);
    for part in track.split(", ") {
        if let Some(n) = part.strip_prefix("ahead ") {
            ahead = n.parse().unwrap_or(0);
        } else if let Some(n) = part.strip_prefix("behind ") {
            behind = n.parse().unwrap_or(0);
        }
    }
    (ahead, behind)
}

/// Fields: tag name, oid of the tagged object, time, subject.
pub const TAG_FORMAT: &str = "--format=%(refname:short)%1f%(objectname)%1f%(creatordate:unix)%1f%(contents:subject)";

pub fn parse_tags(out: &str) -> Result<Vec<Tag>, GitError> {
    out.lines()
        .filter(|l| !l.is_empty())
        .map(|line| {
            let f: Vec<&str> = line.splitn(4, FS).collect();
            if f.len() != 4 {
                return Err(GitError::Parse(format!("tag line: {line:?}")));
            }
            Ok(Tag {
                name: f[0].to_string(),
                oid: f[1].to_string(),
                time: f[2].parse().unwrap_or(0),
                subject: f[3].to_string(),
            })
        })
        .collect()
}

/// `git remote -v`
pub fn parse_remotes(out: &str) -> Vec<Remote> {
    let mut remotes: Vec<Remote> = Vec::new();
    for line in out.lines() {
        let mut parts = line.split_whitespace();
        let (Some(name), Some(url), Some(kind)) = (parts.next(), parts.next(), parts.next()) else {
            continue;
        };
        let idx = match remotes.iter().position(|r| r.name == name) {
            Some(i) => i,
            None => {
                remotes.push(Remote { name: name.to_string(), fetch_url: String::new(), push_url: String::new() });
                remotes.len() - 1
            }
        };
        match kind {
            "(fetch)" => remotes[idx].fetch_url = url.to_string(),
            "(push)" => remotes[idx].push_url = url.to_string(),
            _ => {}
        }
    }
    remotes
}

fn non_empty(s: &str) -> Option<String> {
    (!s.is_empty()).then(|| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_branches() {
        let out = "*\x1frefs/heads/main\x1faaa\x1forigin/main\x1fahead 1, behind 2\x1f1700000000\x1fTip\n \
                   \x1frefs/heads/feature\x1fbbb\x1f\x1f\x1f1600000000\x1fWork\n \
                   \x1frefs/remotes/origin/HEAD\x1faaa\x1f\x1f\x1f1\x1fx\n \
                   \x1frefs/remotes/origin/main\x1faaa\x1f\x1f\x1f1700000000\x1fTip\n";
        let b = parse_branches(out).unwrap();
        assert_eq!(b.len(), 3);
        assert!(b[0].is_head);
        assert_eq!(b[0].upstream.as_deref(), Some("origin/main"));
        assert_eq!(parse_track(b[0].track.as_deref().unwrap()), (1, 2));
        assert_eq!(b[1].upstream, None);
        assert!(b[2].is_remote);
        assert_eq!(b[2].name, "origin/main");
    }

    #[test]
    fn parses_remotes() {
        let out = "origin\tgit@github.com:a/b.git (fetch)\norigin\tgit@github.com:a/b.git (push)\nup\thttps://x/y (fetch)\nup\thttps://x/y (push)\n";
        let r = parse_remotes(out);
        assert_eq!(r.len(), 2);
        assert_eq!(r[1].name, "up");
        assert_eq!(r[1].push_url, "https://x/y");
    }
}
