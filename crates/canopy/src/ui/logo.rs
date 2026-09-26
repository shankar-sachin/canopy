//! The Canopy tree, drawn with block characters.
//!
//! Each character cell holds two pixels stacked vertically (`▀`/`▄` with
//! foreground and background colors), so pixels come out roughly square.

use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};

/// The tree, one character per pixel: `h` highlight, `g` leaf, `m` shade,
/// `t` trunk, `.` empty. Same art as the website's "Block Canopy" mark.
const PIXELS: [&str; 8] =
    ["..hhhhh..", ".hhggggh.", "ggggggggg", "ggggmgggg", ".mmmmmmm.", "...ttt...", "....t....", "..ttttt.."];

/// A 2-row mini tree for the top-right corner of the header.
const MINI: [&str; 4] = [".hhhh.", "hggggg", ".mmmm.", "..tt.."];

fn color(c: char) -> Option<Color> {
    match c {
        'h' => Some(Color::Rgb(142, 224, 160)),
        'g' => Some(Color::Rgb(86, 195, 123)),
        'm' => Some(Color::Rgb(47, 143, 85)),
        't' => Some(Color::Rgb(201, 178, 138)),
        _ => None,
    }
}

/// The tree as lines of text. `scale` 1 is 9x4 cells; 2 is 18x8.
pub fn lines(scale: usize) -> Vec<Line<'static>> {
    grown(scale, 1.0)
}

/// The mini tree: 6x2 cells.
pub fn mini() -> Vec<Line<'static>> {
    let rows: Vec<Vec<char>> = MINI.iter().map(|r| r.chars().collect()).collect();
    fold(&rows)
}

/// The tree part-way through growing: `progress` 0.0 shows nothing, 1.0 the
/// whole tree. Pixel rows appear from the ground up (trunk, then canopy).
pub fn grown(scale: usize, progress: f32) -> Vec<Line<'static>> {
    let scale = scale.max(1);
    let total = PIXELS.len() * scale;
    let visible = (progress.clamp(0.0, 1.0) * total as f32).ceil() as usize;
    // Scale the pixel grid, then fold pairs of pixel rows into one text row.
    let rows: Vec<Vec<char>> = PIXELS
        .iter()
        .flat_map(|r| {
            let row: Vec<char> = r.chars().flat_map(|c| std::iter::repeat_n(c, scale)).collect();
            std::iter::repeat_n(row, scale)
        })
        .enumerate()
        // Rows above the growth line are still empty.
        .map(|(y, row)| if y + visible >= total { row } else { vec!['.'; row.len()] })
        .collect();
    fold(&rows)
}

/// Fold pairs of pixel rows into text rows of half blocks.
fn fold(rows: &[Vec<char>]) -> Vec<Line<'static>> {
    rows.chunks(2)
        .map(|pair| {
            let top = &pair[0];
            let bottom = pair.get(1);
            let spans: Vec<Span> = (0..top.len())
                .map(|x| {
                    let (t, b) = (color(top[x]), bottom.and_then(|r| color(r[x])));
                    match (t, b) {
                        (None, None) => Span::raw(" "),
                        (Some(t), None) => Span::styled("▀", Style::default().fg(t)),
                        (None, Some(b)) => Span::styled("▄", Style::default().fg(b)),
                        (Some(t), Some(b)) if t == b => Span::styled("█", Style::default().fg(t)),
                        (Some(t), Some(b)) => Span::styled("▀", Style::default().fg(t).bg(b)),
                    }
                })
                .collect();
            Line::from(spans)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grows_from_the_ground_up() {
        let text = |ls: Vec<Line>| ls.iter().map(|l| l.to_string()).collect::<Vec<_>>();
        let none = text(grown(1, 0.0));
        assert!(none.iter().all(|l| l.trim().is_empty()));
        // A quarter grown: only the bottom row (trunk) has anything.
        let quarter = text(grown(1, 0.25));
        assert!(quarter[..3].iter().all(|l| l.trim().is_empty()) && !quarter[3].trim().is_empty());
        assert_eq!(text(grown(1, 1.0)), text(lines(1)));
    }

    #[test]
    fn mini_tree() {
        let m = mini();
        assert_eq!(m.len(), 2);
        assert!(m.iter().all(|l| l.width() == 6));
    }

    #[test]
    fn sizes_and_glyphs() {
        let small = lines(1);
        assert_eq!(small.len(), 4);
        assert!(small.iter().all(|l| l.width() == 9));
        let big = lines(2);
        assert_eq!(big.len(), 8);
        assert!(big.iter().all(|l| l.width() == 18));
        // Only block elements and spaces: one column wide in every font.
        for l in &big {
            for s in &l.spans {
                assert!(s.content.chars().all(|c| c == ' ' || ('\u{2580}'..='\u{259F}').contains(&c)));
            }
        }
    }
}
