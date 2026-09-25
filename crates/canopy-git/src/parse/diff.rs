//! Unified diff parsing plus patch generation for hunk/line staging.

use crate::model::{DiffLine, DiffLineKind, FileDiff, Hunk};

pub fn parse(out: &str) -> Vec<FileDiff> {
    let mut files: Vec<FileDiff> = Vec::new();
    let mut hunk: Option<Hunk> = None;
    let (mut old_no, mut new_no) = (0u32, 0u32);

    let flush = |files: &mut Vec<FileDiff>, hunk: &mut Option<Hunk>| {
        if let (Some(h), Some(f)) = (hunk.take(), files.last_mut()) {
            f.hunks.push(h);
        }
    };

    for line in out.lines() {
        if let Some(paths) = line.strip_prefix("diff --git ") {
            flush(&mut files, &mut hunk);
            let (a, b) = split_git_paths(paths);
            files.push(FileDiff { old_path: a, new_path: b, header: vec![line.to_string()], ..Default::default() });
            continue;
        }
        let Some(file) = files.last_mut() else { continue };

        if line.starts_with("@@") {
            flush(&mut files, &mut hunk);
            if let Some(h) = parse_hunk_header(line) {
                old_no = h.old_start;
                new_no = h.new_start;
                hunk = Some(h);
            }
            continue;
        }

        match hunk.as_mut() {
            None => {
                // Still in the file header.
                if let Some(p) = line.strip_prefix("--- ") {
                    if p != "/dev/null" {
                        file.old_path = strip_ab(p);
                    }
                } else if let Some(p) = line.strip_prefix("+++ ") {
                    if p != "/dev/null" {
                        file.new_path = strip_ab(p);
                    }
                } else if let Some(p) = line.strip_prefix("rename from ") {
                    file.old_path = p.to_string();
                } else if let Some(p) = line.strip_prefix("rename to ") {
                    file.new_path = p.to_string();
                } else if line.starts_with("Binary files") || line == "GIT binary patch" {
                    file.binary = true;
                }
                file.header.push(line.to_string());
            }
            Some(h) => {
                let (kind, content) = match line.chars().next() {
                    Some('+') => (DiffLineKind::Added, &line[1..]),
                    Some('-') => (DiffLineKind::Removed, &line[1..]),
                    Some('\\') => (DiffLineKind::NoNewline, line),
                    Some(' ') => (DiffLineKind::Context, &line[1..]),
                    None => (DiffLineKind::Context, ""),
                    _ => (DiffLineKind::Context, line),
                };
                let (o, n) = match kind {
                    DiffLineKind::Added => {
                        new_no += 1;
                        (None, Some(new_no - 1))
                    }
                    DiffLineKind::Removed => {
                        old_no += 1;
                        (Some(old_no - 1), None)
                    }
                    DiffLineKind::Context => {
                        old_no += 1;
                        new_no += 1;
                        (Some(old_no - 1), Some(new_no - 1))
                    }
                    DiffLineKind::NoNewline => (None, None),
                };
                h.lines.push(DiffLine { kind, content: content.to_string(), old_no: o, new_no: n });
            }
        }
    }
    flush(&mut files, &mut hunk);
    files
}

fn strip_ab(p: &str) -> String {
    p.strip_prefix("a/").or_else(|| p.strip_prefix("b/")).unwrap_or(p).to_string()
}

fn split_git_paths(s: &str) -> (String, String) {
    // `a/foo b/foo` — paths with spaces are ambiguous here, but `---`/`+++`
    // lines later override these values.
    match s.find(" b/") {
        Some(i) => (strip_ab(&s[..i]), strip_ab(&s[i + 1..])),
        None => (s.to_string(), s.to_string()),
    }
}

fn parse_range(s: &str) -> Option<(u32, u32)> {
    match s.split_once(',') {
        Some((a, b)) => Some((a.parse().ok()?, b.parse().ok()?)),
        None => Some((s.parse().ok()?, 1)),
    }
}

pub fn parse_hunk_header(line: &str) -> Option<Hunk> {
    // @@ -a,b +c,d @@ optional context
    let rest = line.strip_prefix("@@ ")?;
    let end = rest.find(" @@")?;
    let mut ranges = rest[..end].split(' ');
    let (old_start, old_len) = parse_range(ranges.next()?.strip_prefix('-')?)?;
    let (new_start, new_len) = parse_range(ranges.next()?.strip_prefix('+')?)?;
    Some(Hunk { header: line.to_string(), old_start, old_len, new_start, new_len, lines: Vec::new() })
}

/// Which way a patch will be applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatchMode {
    /// Stage selected lines from an unstaged diff (`git apply --cached`).
    Stage,
    /// Unstage selected lines from a staged diff (`git apply --cached --reverse`).
    Unstage,
}

/// Build a patch containing only the `selected` line indices of `hunk`.
///
/// Unselected changes are neutralised so the patch still applies:
/// - For [`PatchMode::Stage`], an unselected `+` is dropped and an unselected
///   `-` becomes context (the line stays in the index).
/// - For [`PatchMode::Unstage`] the patch is applied in reverse, so an
///   unselected `-` is dropped and an unselected `+` becomes context.
///
/// Returns `None` if the selection contains no changes.
pub fn build_patch(file: &FileDiff, hunk: &Hunk, selected: &[usize], mode: PatchMode) -> Option<String> {
    let mut body = Vec::new();
    let (mut old_len, mut new_len) = (0u32, 0u32);
    let mut any = false;
    let mut last_kept_dropped = false;

    for (i, l) in hunk.lines.iter().enumerate() {
        let sel = selected.contains(&i);
        let (keep_as, counts_old, counts_new) = match (&l.kind, sel, mode) {
            (DiffLineKind::Context, _, _) => (Some(' '), true, true),
            (DiffLineKind::Added, true, _) => (Some('+'), false, true),
            (DiffLineKind::Removed, true, _) => (Some('-'), true, false),
            (DiffLineKind::Added, false, PatchMode::Stage) => (None, false, false),
            (DiffLineKind::Removed, false, PatchMode::Stage) => (Some(' '), true, true),
            (DiffLineKind::Added, false, PatchMode::Unstage) => (Some(' '), true, true),
            (DiffLineKind::Removed, false, PatchMode::Unstage) => (None, false, false),
            (DiffLineKind::NoNewline, _, _) => {
                if !last_kept_dropped {
                    body.push(l.content.clone());
                }
                continue;
            }
        };
        match keep_as {
            Some(c) => {
                if c != ' ' {
                    any = true;
                }
                body.push(format!("{c}{}", l.content));
                last_kept_dropped = false;
            }
            None => last_kept_dropped = true,
        }
        old_len += counts_old as u32;
        new_len += counts_new as u32;
    }
    if !any {
        return None;
    }

    let mut patch = String::new();
    let old = if file.old_path.is_empty() { &file.new_path } else { &file.old_path };
    patch.push_str(&format!("diff --git a/{old} b/{}\n", file.new_path));
    for h in &file.header[1..] {
        if h.starts_with("---") || h.starts_with("+++") || h.starts_with("index ") {
            continue;
        }
        patch.push_str(h);
        patch.push('\n');
    }
    let is_new = file.header.iter().any(|h| h.starts_with("new file mode"));
    let is_deleted = file.header.iter().any(|h| h.starts_with("deleted file mode"));
    patch.push_str(&if is_new { "--- /dev/null\n".to_string() } else { format!("--- a/{old}\n") });
    patch.push_str(&if is_deleted { "+++ /dev/null\n".to_string() } else { format!("+++ b/{}\n", file.new_path) });
    // Anchor on the side the patch is applied against: the index for
    // staging (old side), the index-as-new-side when reverse-applying.
    let start = match mode {
        PatchMode::Stage => hunk.old_start,
        PatchMode::Unstage => hunk.new_start,
    };
    patch.push_str(&format!("@@ -{start},{old_len} +{start},{new_len} @@\n"));
    for l in body {
        patch.push_str(&l);
        patch.push('\n');
    }
    Some(patch)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIFF: &str = "\
diff --git a/src/lib.rs b/src/lib.rs
index 111..222 100644
--- a/src/lib.rs
+++ b/src/lib.rs
@@ -1,4 +1,5 @@ fn main
 one
-two
+TWO
+three
 four
@@ -10 +11 @@
-x
+y
\\ No newline at end of file
diff --git a/new.txt b/new.txt
new file mode 100644
index 000..333
--- /dev/null
+++ b/new.txt
@@ -0,0 +1 @@
+hello
";

    #[test]
    fn parses_files_and_hunks() {
        let files = parse(DIFF);
        assert_eq!(files.len(), 2);
        let f = &files[0];
        assert_eq!(f.new_path, "src/lib.rs");
        assert_eq!(f.hunks.len(), 2);
        let h = &f.hunks[0];
        assert_eq!((h.old_start, h.old_len, h.new_start, h.new_len), (1, 4, 1, 5));
        assert_eq!(h.lines.len(), 5);
        assert_eq!(h.lines[1].kind, DiffLineKind::Removed);
        assert_eq!(h.lines[1].old_no, Some(2));
        assert_eq!(h.lines[3].new_no, Some(3));
        assert_eq!(f.hunks[1].lines[2].kind, DiffLineKind::NoNewline);
        assert_eq!(files[1].new_path, "new.txt");
        assert_eq!(files[1].hunks[0].lines[0].content, "hello");
    }

    #[test]
    fn partial_stage_patch() {
        let files = parse(DIFF);
        let (f, h) = (&files[0], &files[0].hunks[0]);
        // Select only `+three` (index 3): `-two` stays as context, `+TWO` dropped.
        let p = build_patch(f, h, &[3], PatchMode::Stage).unwrap();
        assert!(p.contains("@@ -1,3 +1,4 @@\n one\n two\n+three\n four\n"), "{p}");
        // Nothing selected -> no patch.
        assert!(build_patch(f, h, &[0], PatchMode::Stage).is_none());
    }

    #[test]
    fn partial_unstage_patch() {
        let files = parse(DIFF);
        let (f, h) = (&files[0], &files[0].hunks[0]);
        // Unstage only `-two` (index 1): `+TWO`/`+three` become context.
        let p = build_patch(f, h, &[1], PatchMode::Unstage).unwrap();
        assert!(p.contains("@@ -1,5 +1,4 @@\n one\n-two\n TWO\n three\n four\n"), "{p}");
    }
}
