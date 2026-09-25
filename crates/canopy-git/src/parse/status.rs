//! `git status --porcelain=v2 --branch -z`

use crate::cli::GitError;
use crate::model::{BranchInfo, Change, FileKind, FileStatus, Status};

pub fn parse(out: &str) -> Result<Status, GitError> {
    let mut status = Status::default();
    let mut fields = out.split('\0').filter(|s| !s.is_empty());

    while let Some(entry) = fields.next() {
        let bad = || GitError::Parse(format!("status entry: {entry:?}"));
        if let Some(header) = entry.strip_prefix("# ") {
            parse_header(header, &mut status.branch);
            continue;
        }
        let (tag, rest) = entry.split_once(' ').ok_or_else(bad)?;
        match tag {
            "1" => {
                // XY sub mH mI mW hH hI path
                let parts: Vec<&str> = rest.splitn(8, ' ').collect();
                if parts.len() != 8 {
                    return Err(bad());
                }
                let (index, worktree) = xy(parts[0]);
                status.files.push(FileStatus {
                    path: parts[7].to_string(),
                    orig_path: None,
                    index,
                    worktree,
                    kind: FileKind::Tracked,
                });
            }
            "2" => {
                // XY sub mH mI mW hH hI Xscore path \0 origPath
                let parts: Vec<&str> = rest.splitn(9, ' ').collect();
                if parts.len() != 9 {
                    return Err(bad());
                }
                let orig = fields.next().ok_or_else(bad)?;
                let (index, worktree) = xy(parts[0]);
                status.files.push(FileStatus {
                    path: parts[8].to_string(),
                    orig_path: Some(orig.to_string()),
                    index,
                    worktree,
                    kind: FileKind::Tracked,
                });
            }
            "u" => {
                // XY sub m1 m2 m3 mW h1 h2 h3 path
                let parts: Vec<&str> = rest.splitn(10, ' ').collect();
                if parts.len() != 10 {
                    return Err(bad());
                }
                let (index, worktree) = xy(parts[0]);
                status.files.push(FileStatus {
                    path: parts[9].to_string(),
                    orig_path: None,
                    index,
                    worktree,
                    kind: FileKind::Conflicted,
                });
            }
            "?" | "!" => status.files.push(FileStatus {
                path: rest.to_string(),
                orig_path: None,
                index: Change::Unmodified,
                worktree: if tag == "?" { Change::Added } else { Change::Unmodified },
                kind: if tag == "?" { FileKind::Untracked } else { FileKind::Ignored },
            }),
            _ => return Err(bad()),
        }
    }
    Ok(status)
}

fn xy(s: &str) -> (Change, Change) {
    let mut c = s.chars();
    let x = c.next().unwrap_or('.');
    let y = c.next().unwrap_or('.');
    (Change::from_code(x), Change::from_code(y))
}

fn parse_header(h: &str, b: &mut BranchInfo) {
    let Some((key, val)) = h.split_once(' ') else { return };
    match key {
        "branch.oid" if val != "(initial)" => b.oid = Some(val.to_string()),
        "branch.head" if val != "(detached)" => b.head = Some(val.to_string()),
        "branch.upstream" => b.upstream = Some(val.to_string()),
        "branch.ab" => {
            for part in val.split(' ') {
                if let Some(n) = part.strip_prefix('+') {
                    b.ahead = n.parse().unwrap_or(0);
                } else if let Some(n) = part.strip_prefix('-') {
                    b.behind = n.parse().unwrap_or(0);
                }
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_all_entry_kinds() {
        let out = [
            "# branch.oid 1234567890abcdef1234567890abcdef12345678",
            "# branch.head main",
            "# branch.upstream origin/main",
            "# branch.ab +2 -1",
            "1 .M N... 100644 100644 100644 aaaa bbbb src/main.rs",
            "1 A. N... 000000 100644 100644 0000 cccc new file.txt",
            "2 R. N... 100644 100644 100644 dddd dddd R100 renamed.rs",
            "old.rs",
            "u UU N... 100644 100644 100644 100644 e f g conflict.rs",
            "? untracked.md",
            "! target/",
            "",
        ]
        .join("\0");
        let s = parse(&out).unwrap();
        assert_eq!(s.branch.head.as_deref(), Some("main"));
        assert_eq!(s.branch.upstream.as_deref(), Some("origin/main"));
        assert_eq!((s.branch.ahead, s.branch.behind), (2, 1));
        assert_eq!(s.files.len(), 6);

        assert_eq!(s.files[0].path, "src/main.rs");
        assert!(s.files[0].is_unstaged() && !s.files[0].is_staged());

        assert_eq!(s.files[1].path, "new file.txt");
        assert!(s.files[1].is_staged());

        assert_eq!(s.files[2].path, "renamed.rs");
        assert_eq!(s.files[2].orig_path.as_deref(), Some("old.rs"));
        assert_eq!(s.files[2].index, Change::Renamed);

        assert_eq!(s.files[3].kind, FileKind::Conflicted);
        assert_eq!(s.files[4].kind, FileKind::Untracked);
        assert_eq!(s.files[5].kind, FileKind::Ignored);

        assert_eq!(s.staged().count(), 2);
        assert_eq!(s.unstaged().count(), 3);
        assert_eq!(s.conflicted().count(), 1);
    }

    #[test]
    fn initial_and_detached() {
        let out = "# branch.oid (initial)\0# branch.head (detached)\0";
        let s = parse(out).unwrap();
        assert_eq!(s.branch.oid, None);
        assert_eq!(s.branch.head, None);
        assert!(s.is_clean());
    }
}
