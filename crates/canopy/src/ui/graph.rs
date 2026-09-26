//! Commit graph lanes for the History view.
//!
//! One text row per commit (no connector rows), in `--topo-order`. Each row
//! is a list of cells; each cell has a glyph and a lane index for colouring.

use canopy_git::Commit;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cell {
    pub glyph: char,
    pub lane: usize,
}

pub fn build(commits: &[Commit]) -> Vec<Vec<Cell>> {
    // lanes[i] = oid that lane i is waiting for.
    let mut lanes: Vec<Option<String>> = Vec::new();
    let mut rows = Vec::with_capacity(commits.len());

    for c in commits {
        let col = match lanes.iter().position(|l| l.as_deref() == Some(&c.oid)) {
            Some(i) => i,
            None => match lanes.iter().position(Option::is_none) {
                Some(i) => i,
                None => {
                    lanes.push(None);
                    lanes.len() - 1
                }
            },
        };

        // Other lanes that were also waiting for this commit merge into it.
        let merging: Vec<usize> = lanes
            .iter()
            .enumerate()
            .filter(|(i, l)| *i != col && l.as_deref() == Some(&c.oid))
            .map(|(i, _)| i)
            .collect();

        // Place parents.
        let mut forks: Vec<usize> = Vec::new();
        lanes[col] = c.parents.first().cloned();
        for p in c.parents.iter().skip(1) {
            if lanes.iter().any(|l| l.as_deref() == Some(p)) {
                continue;
            }
            let slot = match lanes.iter().position(Option::is_none) {
                Some(i) if !merging.contains(&i) => i,
                _ => {
                    lanes.push(None);
                    lanes.len() - 1
                }
            };
            lanes[slot] = Some(p.clone());
            forks.push(slot);
        }
        for &m in &merging {
            lanes[m] = None;
        }

        let width = lanes.len().max(col + 1);
        let mut row = Vec::with_capacity(width * 2);
        for i in 0..width {
            let glyph = if i == col {
                if c.parents.len() > 1 {
                    '○'
                } else {
                    '●'
                }
            } else if merging.contains(&i) {
                '╯'
            } else if forks.contains(&i) {
                '╮'
            } else if lanes.get(i).is_some_and(Option::is_some) {
                '│'
            } else {
                ' '
            };
            row.push(Cell { glyph, lane: i });
            row.push(Cell { glyph: ' ', lane: i });
        }
        // Horizontal connectors between the commit and merge/fork lanes.
        for &target in merging.iter().chain(forks.iter()) {
            let (a, b) = (col.min(target), col.max(target));
            for (x, cell) in row.iter_mut().enumerate().take(b * 2).skip(a * 2 + 1) {
                if cell.glyph == ' ' {
                    cell.glyph = '─';
                    cell.lane = target;
                } else if cell.glyph == '│' && x % 2 == 0 {
                    cell.glyph = '┼';
                }
            }
        }
        while lanes.last().is_some_and(Option::is_none) {
            lanes.pop();
        }
        while row.len() > 2 && row[row.len() - 1].glyph == ' ' && row[row.len() - 2].glyph == ' ' {
            row.truncate(row.len() - 2);
        }
        rows.push(row);
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

    fn render(rows: &[Vec<Cell>]) -> Vec<String> {
        rows.iter().map(|r| r.iter().map(|c| c.glyph).collect::<String>().trim_end().to_string()).collect()
    }

    #[test]
    fn linear_history() {
        let rows = build(&[c("c", &["b"]), c("b", &["a"]), c("a", &[])]);
        assert_eq!(render(&rows), vec!["●", "●", "●"]);
    }

    #[test]
    fn merge_and_branch() {
        // m merges x (feature) into b (main).
        let rows = build(&[c("m", &["b", "x"]), c("x", &["a"]), c("b", &["a"]), c("a", &[])]);
        assert_eq!(render(&rows), vec!["○─╮", "│ ●", "● │", "●─╯"]);
    }
}
