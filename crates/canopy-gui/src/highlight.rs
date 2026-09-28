//! Diffs as the page shows them: each file with its lines split into
//! highlighted pieces (see canopy-highlight). The page paints each kind of
//! piece with a CSS class, so the colors follow the theme.

use canopy_git::{DiffLineKind, FileDiff};
use canopy_highlight::{segments, Side};
use serde::Serialize;

/// A file's diff plus, per hunk and line, its pieces as `[kind, text]`
/// ("" is plain text). Lines with no highlighting have no pieces. The page
/// sends the file back to stage lines; the extra field is ignored there.
#[derive(Debug, Clone, Serialize)]
pub struct Shown {
    #[serde(flatten)]
    pub file: FileDiff,
    pub hl: Vec<Vec<Vec<(&'static str, String)>>>,
}

pub fn show(files: Vec<FileDiff>) -> Vec<Shown> {
    files.into_iter().map(show_file).collect()
}

fn show_file(file: FileDiff) -> Shown {
    let path = if file.new_path.is_empty() { &file.old_path } else { &file.new_path };
    if file.binary || !canopy_highlight::supports(path) {
        return Shown { file, hl: Vec::new() };
    }
    let hl = file
        .hunks
        .iter()
        .map(|h| {
            let lines: Vec<(Side, &str)> = h
                .lines
                .iter()
                .map(|l| match l.kind {
                    DiffLineKind::Added => (Side::New, l.content.as_str()),
                    DiffLineKind::Removed => (Side::Old, l.content.as_str()),
                    DiffLineKind::Context => (Side::Both, l.content.as_str()),
                    DiffLineKind::NoNewline => (Side::Both, ""),
                })
                .collect();
            canopy_highlight::diff_lines(path, &lines)
                .iter()
                .zip(&h.lines)
                .map(|(spans, l)| {
                    if spans.is_empty() {
                        return Vec::new();
                    }
                    segments(&l.content, spans).into_iter().map(|(t, s)| (t.name(), s.to_string())).collect()
                })
                .collect()
        })
        .collect();
    Shown { file, hl }
}

#[cfg(test)]
mod tests {
    use super::*;
    use canopy_highlight::Token;

    #[test]
    fn pieces_rebuild_each_line_and_the_file_round_trips() {
        let patch = "diff --git a/m.rs b/m.rs\n--- a/m.rs\n+++ b/m.rs\n@@ -1,2 +1,2 @@\n fn a() {}\n-let x = 1;\n+let x = \"two\";\n";
        let shown = show(canopy_git::parse::diff::parse(patch));
        let s = &shown[0];
        assert_eq!(s.hl.len(), 1);
        for (pieces, line) in s.hl[0].iter().zip(&s.file.hunks[0].lines) {
            assert_eq!(pieces.iter().map(|(_, t)| t.as_str()).collect::<String>(), line.content);
        }
        assert!(s.hl[0][2].iter().any(|(k, t)| *k == Token::String.name() && t.contains("two")), "{:?}", s.hl[0][2]);
        // What the page sends back to stage lines is still a FileDiff.
        let json = serde_json::to_string(s).unwrap();
        let back: FileDiff = serde_json::from_str(&json).unwrap();
        assert_eq!(back, s.file);
    }

    #[test]
    fn unknown_languages_and_binaries_have_no_pieces() {
        let patch = "diff --git a/notes.zzz b/notes.zzz\n--- a/notes.zzz\n+++ b/notes.zzz\n@@ -1 +1 @@\n-a\n+b\n";
        assert!(show(canopy_git::parse::diff::parse(patch))[0].hl.is_empty());
    }
}
