//! Syntax highlighting for diffs, shared by both apps.
//!
//! Lines are sorted into a few kinds of token (keyword, string, comment...)
//! rather than given colors, so each app paints them with its own theme:
//! the terminal app with its palette, the desktop app with CSS variables.
//!
//! A diff shows two versions of a file at once, so each hunk is read twice:
//! the old side (context and removed lines) and the new side (context and
//! added lines), each in order. That keeps multi-line strings and comments
//! right on both sides.

use std::ops::Range;
use std::sync::OnceLock;

use syntect::parsing::{ParseState, Scope, ScopeStack, SyntaxReference, SyntaxSet};

/// What a piece of a line is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Token {
    Plain,
    Comment,
    Keyword,
    String,
    Number,
    /// true, false, null, and other built-in constants.
    Constant,
    Function,
    Type,
    /// HTML/XML tag names.
    Tag,
    /// HTML/XML attribute names.
    Attribute,
}

impl Token {
    /// A short name, for CSS classes (`tk-keyword`) and JSON.
    pub fn name(self) -> &'static str {
        match self {
            Token::Plain => "",
            Token::Comment => "comment",
            Token::Keyword => "keyword",
            Token::String => "string",
            Token::Number => "number",
            Token::Constant => "constant",
            Token::Function => "function",
            Token::Type => "type",
            Token::Tag => "tag",
            Token::Attribute => "attr",
        }
    }
}

/// Byte ranges of one line and what they are. Plain text is left out.
pub type Spans = Vec<(Token, Range<usize>)>;

/// Which version of the file a diff line belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    /// An unchanged (context) line: in both.
    Both,
    /// A removed line.
    Old,
    /// An added line.
    New,
}

// Lines longer than this (minified code, data) are left plain: they're slow
// to read and highlighting them doesn't help.
const MAX_LINE: usize = 2000;
// So is a hunk this big (a generated file, a lock file).
const MAX_LINES: usize = 5000;

fn syntaxes() -> &'static SyntaxSet {
    static SET: OnceLock<SyntaxSet> = OnceLock::new();
    SET.get_or_init(SyntaxSet::load_defaults_newlines)
}

/// The language for `path` (by extension, then by file name, like
/// `Makefile`), or None when there's none to use.
fn syntax_for(path: &str) -> Option<&'static SyntaxReference> {
    let set = syntaxes();
    let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
    let ext = name.rsplit_once('.').map(|(_, e)| e).unwrap_or("");
    let s = set.find_syntax_by_extension(ext).or_else(|| set.find_syntax_by_extension(name))?;
    (s.name != "Plain Text").then_some(s)
}

/// Whether `path`'s language is one Canopy can highlight.
pub fn supports(path: &str) -> bool {
    syntax_for(path).is_some()
}

/// The language's name for `path` (like "Rust"), if it has one.
pub fn language(path: &str) -> Option<&'static str> {
    syntax_for(path).map(|s| s.name.as_str())
}

/// Highlight a hunk's lines (as `(side, text)`, in diff order) of the file
/// at `path`. Returns one `Spans` per line; all empty when the language is
/// unknown or the hunk is too big.
pub fn diff_lines(path: &str, lines: &[(Side, &str)]) -> Vec<Spans> {
    let mut out = vec![Spans::new(); lines.len()];
    let Some(syntax) = syntax_for(path) else { return out };
    if lines.len() > MAX_LINES {
        return out;
    }
    let mut old = Reader::new(syntax);
    let mut new = Reader::new(syntax);
    for (i, (side, text)) in lines.iter().enumerate() {
        match side {
            Side::Old => out[i] = old.line(text),
            Side::New => out[i] = new.line(text),
            Side::Both => {
                let _ = old.line(text);
                out[i] = new.line(text);
            }
        }
    }
    out
}

/// Highlight the lines of a whole file, in order.
pub fn file_lines(path: &str, lines: &[&str]) -> Vec<Spans> {
    let both: Vec<(Side, &str)> = lines.iter().map(|l| (Side::New, *l)).collect();
    diff_lines(path, &both)
}

/// Split `line` into (token, text) pieces by `spans`, plain text included.
pub fn segments<'a>(line: &'a str, spans: &Spans) -> Vec<(Token, &'a str)> {
    let mut out = Vec::new();
    let mut at = 0;
    for (tok, r) in spans {
        // Ranges come from this line; skip any that don't fit (never panic).
        if r.start < at || r.end > line.len() || !line.is_char_boundary(r.start) || !line.is_char_boundary(r.end) {
            continue;
        }
        if r.start > at {
            out.push((Token::Plain, &line[at..r.start]));
        }
        if r.end > r.start {
            out.push((*tok, &line[r.clone()]));
        }
        at = r.end;
    }
    if at < line.len() {
        out.push((Token::Plain, &line[at..]));
    }
    out
}

/// One side of a hunk, read line by line.
struct Reader {
    state: ParseState,
    stack: ScopeStack,
    broken: bool,
}

impl Reader {
    fn new(syntax: &SyntaxReference) -> Self {
        Reader { state: ParseState::new(syntax), stack: ScopeStack::new(), broken: false }
    }

    fn line(&mut self, text: &str) -> Spans {
        if self.broken || text.len() > MAX_LINE {
            return Spans::new();
        }
        let with_nl = format!("{text}\n");
        let Ok(ops) = self.state.parse_line(&with_nl, syntaxes()) else {
            // A grammar that errors stays plain for the rest of the hunk.
            self.broken = true;
            return Spans::new();
        };
        let mut spans = Spans::new();
        let mut at = 0;
        let mut push = |tok: Token, r: Range<usize>| {
            if tok == Token::Plain || r.is_empty() {
                return;
            }
            // Merge with the previous piece when it's the same kind.
            match spans.last_mut() {
                Some((t, prev)) if *t == tok && prev.end == r.start => prev.end = r.end,
                _ => spans.push((tok, r)),
            }
        };
        for (pos, op) in ops {
            let pos = pos.min(text.len());
            if pos > at {
                push(classify(&self.stack), at..pos);
                at = pos;
            }
            if self.stack.apply(&op).is_err() {
                self.broken = true;
                return Spans::new();
            }
        }
        if at < text.len() {
            push(classify(&self.stack), at..text.len());
        }
        spans
    }
}

/// Scope prefixes, most specific first, and the token each one means.
fn rules() -> &'static [(Scope, Token)] {
    static RULES: OnceLock<Vec<(Scope, Token)>> = OnceLock::new();
    RULES.get_or_init(|| {
        [
            ("comment", Token::Comment),
            ("punctuation.definition.comment", Token::Comment),
            ("string", Token::String),
            ("punctuation.definition.string", Token::String),
            ("constant.numeric", Token::Number),
            ("constant.language", Token::Constant),
            ("constant.character", Token::Constant),
            ("keyword.operator", Token::Plain),
            ("keyword", Token::Keyword),
            // `let`, `fn`, `class`, `int`: declaration keywords in the grammars.
            ("storage", Token::Keyword),
            ("entity.name.function", Token::Function),
            ("support.function", Token::Function),
            ("variable.function", Token::Function),
            ("meta.function-call.identifier", Token::Function),
            ("entity.name.tag", Token::Tag),
            ("entity.other.attribute-name", Token::Attribute),
            ("entity.name.type", Token::Type),
            ("entity.name.class", Token::Type),
            ("entity.name.struct", Token::Type),
            ("entity.name.enum", Token::Type),
            ("entity.name.trait", Token::Type),
            ("support.type", Token::Type),
            ("support.class", Token::Type),
            ("entity.other.inherited-class", Token::Type),
        ]
        .into_iter()
        .filter_map(|(s, t)| Scope::new(s).ok().map(|s| (s, t)))
        .collect()
    })
}

/// The token for text under `stack`: the innermost scope that means one.
fn classify(stack: &ScopeStack) -> Token {
    for scope in stack.as_slice().iter().rev() {
        for (prefix, tok) in rules() {
            if prefix.is_prefix_of(*scope) {
                return *tok;
            }
        }
    }
    Token::Plain
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The kinds of token in `line`, as (token, text) with plain text left out.
    fn kinds<'a>(line: &'a str, spans: &Spans) -> Vec<(Token, &'a str)> {
        segments(line, spans).into_iter().filter(|(t, s)| *t != Token::Plain && !s.trim().is_empty()).collect()
    }

    #[test]
    fn rust_tokens() {
        let line = r#"pub fn main() { let n = 42; println!("hi"); } // done"#;
        let spans = &file_lines("src/main.rs", &[line])[0];
        let k = kinds(line, spans);
        assert!(k.contains(&(Token::Keyword, "pub")), "{k:?}");
        assert!(k.contains(&(Token::Keyword, "fn")), "{k:?}");
        assert!(k.contains(&(Token::Function, "main")), "{k:?}");
        assert!(k.contains(&(Token::Number, "42")), "{k:?}");
        assert!(k.iter().any(|(t, s)| *t == Token::String && s.contains("hi")), "{k:?}");
        assert!(k.iter().any(|(t, s)| *t == Token::Comment && s.contains("done")), "{k:?}");
        // The pieces put back together are the line.
        assert_eq!(segments(line, spans).iter().map(|(_, s)| *s).collect::<String>(), line);
    }

    #[test]
    fn languages_by_extension_and_name() {
        assert_eq!(language("a/b/app.py"), Some("Python"));
        assert_eq!(language("web/index.js"), Some("JavaScript"));
        assert_eq!(language("Makefile"), Some("Makefile"));
        assert_eq!(language("x.go"), Some("Go"));
        assert!(!supports("notes.unknownext"));
        assert!(!supports("README"));
        assert!(file_lines("notes.unknownext", &["let x = 1"])[0].is_empty());
    }

    #[test]
    fn each_side_of_a_diff_is_read_in_order() {
        // The old side opens a block comment the new side doesn't have: the
        // context line after it is a comment only on the old side, and it
        // shows the new side.
        let lines = [(Side::Old, "/* old start"), (Side::New, "let a = 1;"), (Side::Both, "let b = 2;")];
        let out = diff_lines("x.rs", &lines);
        assert!(out[0].iter().all(|(t, _)| *t == Token::Comment));
        let ctx = kinds(lines[2].1, &out[2]);
        assert!(ctx.contains(&(Token::Keyword, "let")), "{ctx:?}");
    }

    #[test]
    fn a_multi_line_string_stays_a_string() {
        let lines = [(Side::New, r#"s = """start"#), (Side::New, "inside"), (Side::New, r#"end""""#)];
        let out = diff_lines("x.py", &lines);
        assert!(out[1].iter().any(|(t, r)| *t == Token::String && &lines[1].1[r.clone()] == "inside"), "{:?}", out[1]);
    }

    #[test]
    fn segments_never_panic_on_bad_ranges() {
        let spans: Spans = vec![(Token::Keyword, 0..3), (Token::String, 2..50), (Token::Comment, 4..6)];
        let s = segments("let é", &spans);
        assert_eq!(s.iter().map(|(_, t)| *t).collect::<String>(), "let é");
        // A range inside the two-byte é is skipped, not sliced.
        let s = segments("é", &vec![(Token::Keyword, 0..1)]);
        assert_eq!(s, vec![(Token::Plain, "é")]);
    }

    #[test]
    fn huge_lines_stay_plain() {
        let long = "x".repeat(MAX_LINE + 1);
        assert!(file_lines("a.rs", &[&long])[0].is_empty());
    }
}
