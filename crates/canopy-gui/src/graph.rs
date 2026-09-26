//! Commit graph for the History page, as line segments the page draws with
//! SVG. Commits come in `--topo-order`; each gets one row. A row is split at
//! its middle (where the commit's dot sits): `up` segments run from lanes at
//! the row's top edge to lanes at the middle, `down` ones from the middle to
//! the bottom edge. A segment `(a, b)` goes from lane `a` to lane `b`.

use canopy_git::Commit;
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Row {
    /// The lane the commit's dot is in.
    pub col: usize,
    pub up: Vec<(usize, usize)>,
    pub down: Vec<(usize, usize)>,
    /// Lanes in use at this row, for the column's width.
    pub width: usize,
}

pub fn build(commits: &[Commit]) -> Vec<Row> {
    // lanes[i] = the commit lane i is heading down to.
    let mut lanes: Vec<Option<String>> = Vec::new();
    let mut rows = Vec::with_capacity(commits.len());
    for c in commits {
        let col = lanes
            .iter()
            .position(|l| l.as_deref() == Some(&c.oid))
            .or_else(|| lanes.iter().position(Option::is_none))
            .unwrap_or_else(|| {
                lanes.push(None);
                lanes.len() - 1
            });

        // Top half: lanes heading for this commit end at its dot; the rest
        // pass straight through.
        let mut up = Vec::new();
        for (i, l) in lanes.iter().enumerate() {
            match l {
                Some(oid) if *oid == c.oid => up.push((i, col)),
                Some(_) => up.push((i, i)),
                None => {}
            }
        }
        for l in lanes.iter_mut() {
            if l.as_deref() == Some(&c.oid) {
                *l = None;
            }
        }

        // Bottom half: the first parent continues in this lane; other parents
        // join a lane already heading there, or open a new one.
        let mut down = Vec::new();
        for (pi, p) in c.parents.iter().enumerate() {
            if let Some(j) = lanes.iter().position(|l| l.as_deref() == Some(p)) {
                down.push((col, j));
                continue;
            }
            let slot = if pi == 0 {
                col
            } else {
                lanes.iter().position(Option::is_none).unwrap_or_else(|| {
                    lanes.push(None);
                    lanes.len() - 1
                })
            };
            lanes[slot] = Some(p.clone());
            down.push((col, slot));
        }
        for (i, l) in lanes.iter().enumerate() {
            if l.is_some() && !down.iter().any(|&(_, b)| b == i) {
                down.push((i, i));
            }
        }
        while lanes.last().is_some_and(Option::is_none) {
            lanes.pop();
        }
        let width = up.iter().chain(&down).map(|&(a, b)| a.max(b) + 1).max().unwrap_or(0).max(col + 1);
        rows.push(Row { col, up, down, width });
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(oid: &str, parents: &[&str]) -> Commit {
        Commit {
            oid: oid.into(),
            short: oid.into(),
            parents: parents.iter().map(|p| p.to_string()).collect(),
            author: String::new(),
            email: String::new(),
            time: 0,
            refs: vec![],
            subject: String::new(),
        }
    }

    #[test]
    fn straight_line() {
        let rows = build(&[c("c", &["b"]), c("b", &["a"]), c("a", &[])]);
        assert_eq!(rows[0], Row { col: 0, up: vec![], down: vec![(0, 0)], width: 1 });
        assert_eq!(rows[1], Row { col: 0, up: vec![(0, 0)], down: vec![(0, 0)], width: 1 });
        assert_eq!(rows[2], Row { col: 0, up: vec![(0, 0)], down: vec![], width: 1 });
    }

    #[test]
    fn merge_and_fork() {
        // m merges f into b; f and b both come from a.
        //   m        col 0, forks lane 1 for f
        //   |\\
        //   | f      col 1
        //   b |      col 0
        //   |/
        //   a
        let rows = build(&[c("m", &["b", "f"]), c("f", &["a"]), c("b", &["a"]), c("a", &[])]);
        assert_eq!(rows[0].down, vec![(0, 0), (0, 1)]);
        assert_eq!(rows[1].col, 1);
        assert_eq!(rows[1].up, vec![(0, 0), (1, 1)]);
        // f's parent a goes down lane 1.
        assert_eq!(rows[1].down, vec![(1, 1), (0, 0)]);
        // b's parent a is already awaited in lane 1: b joins it.
        assert_eq!(rows[2].col, 0);
        assert_eq!(rows[2].down, vec![(0, 1)]);
        // a: the dot is in lane 1, which the branch ran down.
        assert_eq!(rows[3].col, 1);
        assert_eq!(rows[3].up, vec![(1, 1)]);
    }

    #[test]
    fn two_heads_share_a_parent() {
        // Two branch tips (as with --all) pointing at the same parent.
        let rows = build(&[c("x", &["p"]), c("y", &["p"]), c("p", &[])]);
        assert_eq!(rows[1].col, 1);
        assert_eq!(rows[1].down, vec![(1, 0)]);
        assert_eq!(rows[2].col, 0);
        assert_eq!(rows[2].up, vec![(0, 0)]);
    }
}
