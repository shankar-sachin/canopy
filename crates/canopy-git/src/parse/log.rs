//! `git log` with our custom format (see [`FORMAT`]).

use super::{FS, RS};
use crate::cli::GitError;
use crate::model::{Commit, ReflogEntry, Stash};

/// Fields: oid, short, parents, author, email, author time, decorations, subject.
pub const FORMAT: &str = "--format=%H%x1f%h%x1f%P%x1f%an%x1f%ae%x1f%at%x1f%D%x1f%s%x1e";

pub fn parse_commits(out: &str) -> Result<Vec<Commit>, GitError> {
    records(out)
        .map(|rec| {
            let f: Vec<&str> = rec.split(FS).collect();
            if f.len() != 8 {
                return Err(GitError::Parse(format!("log record: {rec:?}")));
            }
            Ok(Commit {
                oid: f[0].to_string(),
                short: f[1].to_string(),
                parents: f[2].split_whitespace().map(String::from).collect(),
                author: f[3].to_string(),
                email: f[4].to_string(),
                time: f[5].parse().unwrap_or(0),
                refs: f[6].split(", ").filter(|r| !r.is_empty()).map(String::from).collect(),
                subject: f[7].to_string(),
            })
        })
        .collect()
}

/// Fields: selector, oid, time, subject. Used with `git stash list` and `git reflog`.
pub const SELECTOR_FORMAT: &str = "--format=%gd%x1f%H%x1f%ct%x1f%gs%x1e";

pub fn parse_stashes(out: &str) -> Result<Vec<Stash>, GitError> {
    records(out)
        .enumerate()
        .map(|(i, rec)| {
            let f: Vec<&str> = rec.split(FS).collect();
            if f.len() != 4 {
                return Err(GitError::Parse(format!("stash record: {rec:?}")));
            }
            Ok(Stash { index: i, name: f[0].to_string(), message: f[3].to_string(), time: f[2].parse().unwrap_or(0) })
        })
        .collect()
}

pub fn parse_reflog(out: &str) -> Result<Vec<ReflogEntry>, GitError> {
    records(out)
        .map(|rec| {
            let f: Vec<&str> = rec.split(FS).collect();
            if f.len() != 4 {
                return Err(GitError::Parse(format!("reflog record: {rec:?}")));
            }
            Ok(ReflogEntry {
                selector: f[0].to_string(),
                oid: f[1].to_string(),
                time: f[2].parse().unwrap_or(0),
                subject: f[3].to_string(),
            })
        })
        .collect()
}

fn records(out: &str) -> impl Iterator<Item = &str> {
    out.split(RS).map(|r| r.trim_start_matches('\n')).filter(|r| !r.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_commits() {
        let out = "aaa\x1fa\x1fbbb ccc\x1fAda\x1fada@x.io\x1f1700000000\x1fHEAD -> main, origin/main\x1fMerge it\x1e\n\
                   bbb\x1fb\x1f\x1fBo\x1fbo@x.io\x1f1690000000\x1f\x1fInitial commit\x1e\n";
        let c = parse_commits(out).unwrap();
        assert_eq!(c.len(), 2);
        assert_eq!(c[0].parents, vec!["bbb", "ccc"]);
        assert_eq!(c[0].refs, vec!["HEAD -> main", "origin/main"]);
        assert!(c[1].parents.is_empty());
        assert!(c[1].refs.is_empty());
        assert_eq!(c[1].subject, "Initial commit");
    }

    #[test]
    fn subject_may_contain_commas() {
        let out = "a\x1fa\x1f\x1fX\x1fx@x\x1f1\x1f\x1ffix a, b, and c\x1e";
        assert_eq!(parse_commits(out).unwrap()[0].subject, "fix a, b, and c");
    }

    #[test]
    fn parses_stashes() {
        let out = "stash@{0}\x1fabc\x1f1700000000\x1fWIP on main: 123 thing\x1e\n\
                   stash@{1}\x1fdef\x1f1600000000\x1fOn main: saved\x1e\n";
        let s = parse_stashes(out).unwrap();
        assert_eq!(s[1].index, 1);
        assert_eq!(s[1].name, "stash@{1}");
        assert_eq!(s[1].message, "On main: saved");
    }
}
