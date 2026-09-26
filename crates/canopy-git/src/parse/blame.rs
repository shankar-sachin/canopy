//! `git blame --porcelain`

use std::collections::HashMap;

use crate::cli::GitError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlameCommit {
    pub oid: String,
    pub author: String,
    pub time: i64,
    pub summary: String,
    /// Not yet committed (`0000…`).
    pub uncommitted: bool,
    /// The file's path in this commit (differs from today's after a rename).
    pub filename: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlameLine {
    pub oid: String,
    /// Line number in the blamed revision (1-based).
    pub line: u32,
    pub content: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Blame {
    pub lines: Vec<BlameLine>,
    pub commits: HashMap<String, BlameCommit>,
}

pub fn parse(out: &str) -> Result<Blame, GitError> {
    let mut blame = Blame::default();
    let mut cur: Option<(String, u32)> = None;
    for line in out.lines() {
        if let Some(content) = line.strip_prefix('\t') {
            let (oid, n) = cur.take().ok_or_else(|| GitError::Parse("blame: content without header".into()))?;
            blame.lines.push(BlameLine { oid, line: n, content: content.to_string() });
            continue;
        }
        match &cur {
            None => {
                // Header: <oid> <orig-line> <final-line> [<group-size>]
                let mut parts = line.split(' ');
                let oid = parts.next().unwrap_or_default();
                let final_line = parts.nth(1).and_then(|n| n.parse().ok());
                let (true, Some(n)) = (oid.len() >= 40 && oid.bytes().all(|b| b.is_ascii_hexdigit()), final_line)
                else {
                    return Err(GitError::Parse(format!("blame header: {line:?}")));
                };
                blame.commits.entry(oid.to_string()).or_insert_with(|| BlameCommit {
                    oid: oid.to_string(),
                    author: String::new(),
                    time: 0,
                    summary: String::new(),
                    uncommitted: oid.bytes().all(|b| b == b'0'),
                    filename: String::new(),
                });
                cur = Some((oid.to_string(), n));
            }
            Some((oid, _)) => {
                let (key, val) = line.split_once(' ').unwrap_or((line, ""));
                let c = blame.commits.get_mut(oid).expect("inserted with header");
                match key {
                    "author" => c.author = val.to_string(),
                    "author-time" => c.time = val.parse().unwrap_or(0),
                    "summary" => c.summary = val.to_string(),
                    "filename" => c.filename = val.to_string(),
                    _ => {}
                }
            }
        }
    }
    Ok(blame)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_porcelain() {
        let a = "a".repeat(40);
        let z = "0".repeat(40);
        let out = format!(
            "{a} 1 1 2\nauthor Ada\nauthor-mail <a@x>\nauthor-time 1700000000\nsummary First\nfilename f.rs\n\tfn main() {{\n\
             {a} 2 2\n\tbody\n\
             {z} 3 3 1\nauthor Not Committed Yet\nauthor-time 1800000000\nsummary Version of f.rs from f.rs\nfilename f.rs\n\t}}\n"
        );
        let b = parse(&out).unwrap();
        assert_eq!(b.lines.len(), 3);
        assert_eq!(b.lines[1].content, "body");
        assert_eq!(b.lines[1].line, 2);
        assert_eq!(b.commits[&a].author, "Ada");
        assert_eq!(b.commits[&a].summary, "First");
        assert_eq!(b.commits[&a].filename, "f.rs");
        assert!(b.commits[&z].uncommitted);
        assert!(!b.commits[&a].uncommitted);
    }

    #[test]
    fn rejects_garbage() {
        assert!(parse("not a header\n").is_err());
    }
}
