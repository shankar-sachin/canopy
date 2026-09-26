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
    let scale = scale.max(1);
    // Scale the pixel grid, then fold pairs of pixel rows into one text row.
    let rows: Vec<Vec<char>> = PIXELS
        .iter()
        .flat_map(|r| {
            let row: Vec<char> = r.chars().flat_map(|c| std::iter::repeat_n(c, scale)).collect();
            std::iter::repeat_n(row, scale)
        })
        .collect();
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
