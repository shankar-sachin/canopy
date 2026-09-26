//! Output of `git bisect start/good/bad/skip`.

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BisectStep {
    /// Still searching; git checked out the next commit to test.
    Testing { revisions_left: u32, steps_left: u32, oid: String, subject: String },
    /// Done: this commit introduced the problem.
    Found { oid: String, subject: String },
    /// Only skipped commits remain between good and bad.
    Inconclusive,
}

/// Parse a bisect command's output. Pass stdout and stderr together: which
/// stream git uses for these messages varies between versions.
pub fn parse(out: &str) -> Option<BisectStep> {
    let lines: Vec<&str> = out.lines().collect();
    for (i, line) in lines.iter().enumerate() {
        if let Some(oid) = line.strip_suffix(" is the first bad commit") {
            // `commit <oid>` header follows, then author/date, then the indented subject.
            let subject = lines[i + 1..]
                .iter()
                .skip_while(|l| !l.is_empty())
                .find(|l| l.starts_with("    "))
                .map(|l| l.trim().to_string())
                .unwrap_or_default();
            return Some(BisectStep::Found { oid: oid.trim().to_string(), subject });
        }
        if line.starts_with("There are only 'skip'ped commits left to test") {
            return Some(BisectStep::Inconclusive);
        }
        if let Some(rest) = line.strip_prefix("Bisecting: ") {
            // "3 revisions left to test after this (roughly 2 steps)"
            let nums: Vec<u32> = rest
                .split(|c: char| !c.is_ascii_digit())
                .filter(|s| !s.is_empty())
                .filter_map(|s| s.parse().ok())
                .collect();
            let (revisions_left, steps_left) = (*nums.first()?, nums.get(1).copied().unwrap_or(0));
            // Next line: "[<oid>] <subject>"
            let next = lines.get(i + 1)?;
            let (oid, subject) = next.strip_prefix('[')?.split_once("] ")?;
            return Some(BisectStep::Testing {
                revisions_left,
                steps_left,
                oid: oid.to_string(),
                subject: subject.to_string(),
            });
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn testing_step() {
        let out = "Bisecting: 3 revisions left to test after this (roughly 2 steps)\n[abc123] Add parser\n";
        assert_eq!(
            parse(out),
            Some(BisectStep::Testing {
                revisions_left: 3,
                steps_left: 2,
                oid: "abc123".into(),
                subject: "Add parser".into()
            })
        );
    }

    #[test]
    fn found() {
        let out = "abc123 is the first bad commit\ncommit abc123\nAuthor: A <a@x>\nDate:   now\n\n    Break the build\n\n f.txt | 2 +-\n";
        assert_eq!(parse(out), Some(BisectStep::Found { oid: "abc123".into(), subject: "Break the build".into() }));
    }

    #[test]
    fn other() {
        assert_eq!(parse("There are only 'skip'ped commits left to test.\n"), Some(BisectStep::Inconclusive));
        assert_eq!(parse("status: waiting for good commit(s)\n"), None);
    }
}
