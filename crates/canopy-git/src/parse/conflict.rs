//! Conflict markers in a working-tree file, and resolving them one by one.
//!
//! Handles the default and `diff3`/`zdiff3` styles:
//!
//! ```text
//! <<<<<<< ours-label
//! ours
//! ||||||| base-label      (diff3 only)
//! base
//! =======
//! theirs
//! >>>>>>> theirs-label
//! ```
//!
//! Text outside conflicts is kept byte-for-byte, including line endings.

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Segment {
    Text(String),
    Conflict(Conflict),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conflict {
    pub ours_label: String,
    pub theirs_label: String,
    pub ours: String,
    pub base: Option<String>,
    pub theirs: String,
    /// The original text including markers, used when left unresolved.
    pub raw: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Choice {
    Ours,
    Theirs,
    /// Ours followed by theirs.
    Both,
}

fn marker<'a>(line: &'a str, m: &str) -> Option<&'a str> {
    let body = line.trim_end_matches(['\n', '\r']);
    let rest = body.strip_prefix(m)?;
    if rest.is_empty() {
        Some("")
    } else {
        rest.strip_prefix(' ')
    }
}

/// Split `text` into plain text and conflicts. Returns `None` if the markers
/// are malformed (e.g. a `<<<<<<<` without its `>>>>>>>`).
pub fn parse(text: &str) -> Option<Vec<Segment>> {
    enum St {
        Text,
        Ours,
        Base,
        Theirs,
    }
    let mut out = Vec::new();
    let mut plain = String::new();
    let mut cur: Option<Conflict> = None;
    let mut st = St::Text;

    for line in text.split_inclusive('\n') {
        match st {
            St::Text => {
                if let Some(label) = marker(line, "<<<<<<<") {
                    if !plain.is_empty() {
                        out.push(Segment::Text(std::mem::take(&mut plain)));
                    }
                    cur = Some(Conflict {
                        ours_label: label.to_string(),
                        theirs_label: String::new(),
                        ours: String::new(),
                        base: None,
                        theirs: String::new(),
                        raw: line.to_string(),
                    });
                    st = St::Ours;
                } else {
                    plain.push_str(line);
                }
            }
            St::Ours | St::Base | St::Theirs => {
                let c = cur.as_mut()?;
                c.raw.push_str(line);
                match st {
                    St::Ours | St::Base if marker(line, "=======") == Some("") => st = St::Theirs,
                    St::Ours if marker(line, "|||||||").is_some() => {
                        c.base = Some(String::new());
                        st = St::Base;
                    }
                    St::Ours => c.ours.push_str(line),
                    St::Base => c.base.get_or_insert_with(String::new).push_str(line),
                    St::Theirs => {
                        if let Some(label) = marker(line, ">>>>>>>") {
                            c.theirs_label = label.to_string();
                            out.push(Segment::Conflict(cur.take()?));
                            st = St::Text;
                        } else {
                            c.theirs.push_str(line);
                        }
                    }
                    St::Text => unreachable!(),
                }
            }
        }
    }
    if cur.is_some() {
        return None;
    }
    if !plain.is_empty() {
        out.push(Segment::Text(plain));
    }
    Some(out)
}

pub fn count(segments: &[Segment]) -> usize {
    segments.iter().filter(|s| matches!(s, Segment::Conflict(_))).count()
}

/// Rebuild the file, resolving conflict `index` (0-based among conflicts)
/// with `choice` and leaving the others as they are.
pub fn resolve_one(segments: &[Segment], index: usize, choice: Choice) -> String {
    let mut out = String::new();
    let mut n = 0;
    for s in segments {
        match s {
            Segment::Text(t) => out.push_str(t),
            Segment::Conflict(c) => {
                if n == index {
                    match choice {
                        Choice::Ours => out.push_str(&c.ours),
                        Choice::Theirs => out.push_str(&c.theirs),
                        Choice::Both => {
                            out.push_str(&c.ours);
                            out.push_str(&c.theirs);
                        }
                    }
                } else {
                    out.push_str(&c.raw);
                }
                n += 1;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const TWO: &str = "\
top
<<<<<<< HEAD
main line
=======
feature line
>>>>>>> feature
middle
<<<<<<< HEAD
a
||||||| base
orig
=======
b
>>>>>>> feature
end
";

    #[test]
    fn parses_both_styles() {
        let segs = parse(TWO).unwrap();
        assert_eq!(count(&segs), 2);
        let Segment::Conflict(c) = &segs[1] else { panic!() };
        assert_eq!((c.ours_label.as_str(), c.theirs_label.as_str()), ("HEAD", "feature"));
        assert_eq!((c.ours.as_str(), c.theirs.as_str(), c.base.as_deref()), ("main line\n", "feature line\n", None));
        let Segment::Conflict(c) = &segs[3] else { panic!() };
        assert_eq!(c.base.as_deref(), Some("orig\n"));
        assert_eq!(c.theirs, "b\n");
    }

    #[test]
    fn resolves_one_at_a_time() {
        let segs = parse(TWO).unwrap();
        let once = resolve_one(&segs, 1, Choice::Both);
        let segs2 = parse(&once).unwrap();
        assert_eq!(count(&segs2), 1);
        assert!(once.contains("middle\na\nb\nend\n"));
        let done = resolve_one(&segs2, 0, Choice::Theirs);
        assert_eq!(done, "top\nfeature line\nmiddle\na\nb\nend\n");
        assert_eq!(count(&parse(&done).unwrap()), 0);
    }

    #[test]
    fn untouched_conflicts_round_trip_exactly() {
        let crlf = TWO.replace('\n', "\r\n");
        for text in [TWO, crlf.as_str()] {
            let segs = parse(text).unwrap();
            let mut rebuilt = String::new();
            for s in &segs {
                match s {
                    Segment::Text(t) => rebuilt.push_str(t),
                    Segment::Conflict(c) => rebuilt.push_str(&c.raw),
                }
            }
            assert_eq!(rebuilt, text);
        }
    }

    #[test]
    fn malformed_and_lookalikes() {
        assert!(parse("<<<<<<< HEAD\nnever closed\n").is_none());
        // `========` (8) and markers not at line start are ordinary text.
        let segs = parse("a ======= b\n========\n").unwrap();
        assert_eq!(count(&segs), 0);
        // No trailing newline on the last line.
        let segs = parse("<<<<<<< A\nx\n=======\ny\n>>>>>>> B").unwrap();
        assert_eq!(resolve_one(&segs, 0, Choice::Ours), "x\n");
    }
}
