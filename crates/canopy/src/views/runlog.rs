//! The CI log viewer: a run's whole log, failed jobs first, with search.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Job {
        failed: bool,
    },
    Step,
    Text,
    /// `##[error]` lines.
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogLine {
    pub kind: Kind,
    pub text: String,
}

#[derive(Debug, Clone)]
pub struct LogView {
    pub title: String,
    pub lines: Vec<LogLine>,
    pub cursor: usize,
    /// The search being typed, while `/` is open.
    pub typing: Option<String>,
    pub query: String,
    /// Lines that contain `query`.
    pub matches: Vec<usize>,
}

impl LogView {
    pub fn new(title: String, raw: &str, failed_jobs: &[String]) -> Self {
        let lines = parse(raw, failed_jobs);
        // Start at the first error, or the top of the first failed job.
        let cursor = lines
            .iter()
            .position(|l| l.kind == Kind::Error)
            .or_else(|| lines.iter().position(|l| l.kind == Kind::Job { failed: true }))
            .unwrap_or(0);
        LogView { title, lines, cursor, typing: None, query: String::new(), matches: Vec::new() }
    }

    pub fn move_cursor(&mut self, delta: isize) {
        let max = self.lines.len().saturating_sub(1) as isize;
        self.cursor = (self.cursor as isize + delta).clamp(0, max) as usize;
    }

    /// Search for `query` (ignoring case) and jump to the first match at or
    /// after the cursor.
    pub fn set_query(&mut self, query: &str) {
        self.query = query.to_string();
        let q = query.to_lowercase();
        self.matches = if q.is_empty() {
            Vec::new()
        } else {
            self.lines.iter().enumerate().filter(|(_, l)| l.text.to_lowercase().contains(&q)).map(|(i, _)| i).collect()
        };
        if let Some(&m) = self.matches.iter().find(|&&m| m >= self.cursor).or(self.matches.first()) {
            self.cursor = m;
        }
    }

    /// Go to the next (or previous) match, wrapping around.
    pub fn next_match(&mut self, forward: bool) -> bool {
        let found = if forward {
            self.matches.iter().find(|&&m| m > self.cursor).or(self.matches.first())
        } else {
            self.matches.iter().rev().find(|&&m| m < self.cursor).or(self.matches.last())
        };
        match found {
            Some(&m) => {
                self.cursor = m;
                true
            }
            None => false,
        }
    }

    /// "3/12" for the match under the cursor.
    pub fn match_position(&self) -> Option<(usize, usize)> {
        let i = self.matches.iter().position(|&m| m == self.cursor)?;
        Some((i + 1, self.matches.len()))
    }
}

/// `gh run view --log` prints `job<TAB>step<TAB>timestamp text`. Group the
/// lines by job (failed jobs first) and step, and tidy each line.
pub fn parse(raw: &str, failed_jobs: &[String]) -> Vec<LogLine> {
    // (job, [(step, lines)]) in the order they first appear.
    type Steps = Vec<(String, Vec<LogLine>)>;
    let mut jobs: Vec<(String, Steps)> = Vec::new();
    for line in raw.lines() {
        let mut parts = line.splitn(3, '\t');
        let (job, step, rest) = match (parts.next(), parts.next(), parts.next()) {
            (Some(j), Some(s), Some(r)) => (j, s, r),
            _ => ("", "", line),
        };
        let Some(text) = tidy(rest) else { continue };
        if jobs.last().is_none_or(|(j, _)| j != job) {
            match jobs.iter().position(|(j, _)| j == job) {
                // Keep a job's lines together even if they come back later.
                Some(i) => {
                    let g = jobs.remove(i);
                    jobs.push(g);
                }
                None => jobs.push((job.to_string(), Vec::new())),
            }
        }
        let steps = &mut jobs.last_mut().unwrap().1;
        if steps.last().is_none_or(|(s, _)| s != step) {
            steps.push((step.to_string(), Vec::new()));
        }
        steps.last_mut().unwrap().1.push(text);
    }
    // Failed jobs first; otherwise keep gh's order.
    jobs.sort_by_key(|(j, _)| !failed_jobs.contains(j));

    let mut out = Vec::new();
    for (job, steps) in jobs {
        if !out.is_empty() {
            out.push(LogLine { kind: Kind::Text, text: String::new() });
        }
        if !job.is_empty() {
            out.push(LogLine { kind: Kind::Job { failed: failed_jobs.contains(&job) }, text: job });
        }
        for (step, lines) in steps {
            // Newer logs don't name their steps; don't print a useless header.
            if !step.is_empty() && step != "UNKNOWN STEP" {
                out.push(LogLine { kind: Kind::Step, text: step });
            }
            out.extend(lines);
        }
    }
    out
}

/// Drop the timestamp, colour codes and `##[…]` markers from a log line.
/// `None` for lines that are only markers.
fn tidy(text: &str) -> Option<LogLine> {
    let text = text.trim_start_matches('\u{feff}');
    let text = match text.split_once(' ') {
        Some((ts, rest)) if ts.len() >= 20 && ts.as_bytes().get(10) == Some(&b'T') => rest,
        _ if text.len() >= 20 && text.as_bytes().get(10) == Some(&b'T') && !text.contains(' ') => "",
        _ => text,
    };
    let text = strip_escapes(text);
    // Runner bookkeeping, not output.
    if ["##[endgroup]", "##[start-action", "##[end-action", "##[debug]"].iter().any(|m| text.starts_with(m)) {
        return None;
    }
    let (kind, text) = if let Some(rest) = text.strip_prefix("##[error]") {
        (Kind::Error, rest.to_string())
    } else if let Some(rest) = text.strip_prefix("##[warning]") {
        (Kind::Text, format!("warning: {rest}"))
    } else if let Some(rest) = text.strip_prefix("##[group]") {
        (Kind::Step, rest.to_string())
    } else {
        (Kind::Text, text)
    };
    Some(LogLine { kind, text })
}

/// Remove ANSI escape sequences and other control characters; tabs become spaces.
fn strip_escapes(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\x1b' => {
                // CSI: ESC [ params final-byte.
                if chars.peek() == Some(&'[') {
                    chars.next();
                    for c in chars.by_ref() {
                        if ('@'..='~').contains(&c) {
                            break;
                        }
                    }
                }
            }
            '\t' => out.push_str("    "),
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const LOG: &str = "lint\tRun clippy\t2026-09-26T00:30:00.1Z ok\n\
        test\tUNKNOWN STEP\t\u{feff}2026-09-26T00:31:00.1Z ##[group]Run cargo test\n\
        test\tUNKNOWN STEP\t2026-09-26T00:31:00.2Z ##[end-action id=x;outcome=success]\n\
        test\tUNKNOWN STEP\t2026-09-26T00:31:01.1Z \x1b[1;31merror\x1b[0m: thread panicked\tat x\n\
        test\tUNKNOWN STEP\t2026-09-26T00:31:02.1Z ##[endgroup]\n\
        test\tUNKNOWN STEP\t2026-09-26T00:31:03.1Z ##[error]Process completed with exit code 101.\n\
        lint\tRun clippy\t2026-09-26T00:30:01.1Z done\n";

    fn texts(v: &[LogLine]) -> Vec<&str> {
        v.iter().map(|l| l.text.as_str()).collect()
    }

    #[test]
    fn failed_jobs_first_and_tidied() {
        let lines = parse(LOG, &["test".to_string()]);
        assert_eq!(
            texts(&lines),
            vec![
                "test",
                "Run cargo test",
                "error: thread panicked    at x",
                "Process completed with exit code 101.",
                "",
                "lint",
                "Run clippy",
                "ok",
                "done",
            ]
        );
        assert_eq!(lines[0].kind, Kind::Job { failed: true });
        assert_eq!(lines[3].kind, Kind::Error);
        assert_eq!(lines[5].kind, Kind::Job { failed: false });
    }

    #[test]
    fn search_and_jump() {
        let mut v = LogView::new("t".into(), LOG, &["test".to_string()]);
        assert_eq!(v.cursor, 3, "starts on the error");
        v.cursor = 0;
        v.set_query("RUN");
        assert_eq!(v.matches, vec![1, 6]);
        assert_eq!(v.cursor, 1);
        assert_eq!(v.match_position(), Some((1, 2)));
        assert!(v.next_match(true));
        assert_eq!(v.cursor, 6);
        assert!(v.next_match(true));
        assert_eq!(v.cursor, 1, "wraps around");
        assert!(v.next_match(false));
        assert_eq!(v.cursor, 6);
        v.set_query("nope");
        assert!(!v.next_match(true));
    }
}
