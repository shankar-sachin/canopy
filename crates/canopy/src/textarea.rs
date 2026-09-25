//! Minimal text editor used for prompts and the commit message.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug, Clone, Default)]
pub struct TextArea {
    lines: Vec<String>,
    row: usize,
    /// Cursor position in chars (not bytes).
    col: usize,
    single_line: bool,
}

impl TextArea {
    pub fn single(initial: &str) -> Self {
        let mut t = TextArea { lines: vec![initial.to_string()], single_line: true, ..Default::default() };
        t.col = initial.chars().count();
        t
    }

    pub fn multi(initial: &str) -> Self {
        let mut lines: Vec<String> = initial.split('\n').map(String::from).collect();
        if lines.is_empty() {
            lines.push(String::new());
        }
        let row = lines.len() - 1;
        let col = lines[row].chars().count();
        TextArea { lines, row, col, single_line: false }
    }

    pub fn text(&self) -> String {
        self.lines.join("\n")
    }

    pub fn lines(&self) -> &[String] {
        &self.lines
    }

    pub fn cursor(&self) -> (usize, usize) {
        (self.row, self.col)
    }

    fn byte_idx(line: &str, col: usize) -> usize {
        line.char_indices().nth(col).map(|(i, _)| i).unwrap_or(line.len())
    }

    fn line_len(&self) -> usize {
        self.lines[self.row].chars().count()
    }

    #[cfg(test)]
    pub fn insert_str(&mut self, s: &str) {
        for c in s.chars() {
            if c == '\n' {
                self.newline();
            } else {
                self.insert(c);
            }
        }
    }

    fn insert(&mut self, c: char) {
        let line = &mut self.lines[self.row];
        let i = Self::byte_idx(line, self.col);
        line.insert(i, c);
        self.col += 1;
    }

    fn newline(&mut self) {
        if self.single_line {
            return;
        }
        let line = &mut self.lines[self.row];
        let i = Self::byte_idx(line, self.col);
        let rest = line.split_off(i);
        self.row += 1;
        self.lines.insert(self.row, rest);
        self.col = 0;
    }

    fn backspace(&mut self) {
        if self.col > 0 {
            let line = &mut self.lines[self.row];
            let i = Self::byte_idx(line, self.col - 1);
            line.remove(i);
            self.col -= 1;
        } else if self.row > 0 {
            let cur = self.lines.remove(self.row);
            self.row -= 1;
            self.col = self.line_len();
            self.lines[self.row].push_str(&cur);
        }
    }

    fn delete(&mut self) {
        if self.col < self.line_len() {
            let line = &mut self.lines[self.row];
            let i = Self::byte_idx(line, self.col);
            line.remove(i);
        } else if self.row + 1 < self.lines.len() {
            let next = self.lines.remove(self.row + 1);
            self.lines[self.row].push_str(&next);
        }
    }

    fn delete_word(&mut self) {
        let chars: Vec<char> = self.lines[self.row].chars().collect();
        let mut i = self.col;
        while i > 0 && chars[i - 1].is_whitespace() {
            i -= 1;
        }
        while i > 0 && !chars[i - 1].is_whitespace() {
            i -= 1;
        }
        let line: String = chars[..i].iter().chain(chars[self.col..].iter()).collect();
        self.lines[self.row] = line;
        self.col = i;
    }

    /// Returns true if the key was consumed.
    pub fn handle_key(&mut self, key: KeyEvent) -> bool {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            KeyCode::Char('u') if ctrl => {
                let i = Self::byte_idx(&self.lines[self.row], self.col);
                self.lines[self.row].drain(..i);
                self.col = 0;
            }
            KeyCode::Char('w') if ctrl => self.delete_word(),
            KeyCode::Char('a') if ctrl => self.col = 0,
            KeyCode::Char('e') if ctrl => self.col = self.line_len(),
            KeyCode::Char(c) if !ctrl => self.insert(c),
            KeyCode::Enter if !self.single_line => self.newline(),
            KeyCode::Backspace => self.backspace(),
            KeyCode::Delete => self.delete(),
            KeyCode::Left => {
                if self.col > 0 {
                    self.col -= 1;
                } else if self.row > 0 {
                    self.row -= 1;
                    self.col = self.line_len();
                }
            }
            KeyCode::Right => {
                if self.col < self.line_len() {
                    self.col += 1;
                } else if self.row + 1 < self.lines.len() {
                    self.row += 1;
                    self.col = 0;
                }
            }
            KeyCode::Up if self.row > 0 => {
                self.row -= 1;
                self.col = self.col.min(self.line_len());
            }
            KeyCode::Down if self.row + 1 < self.lines.len() => {
                self.row += 1;
                self.col = self.col.min(self.line_len());
            }
            KeyCode::Home => self.col = 0,
            KeyCode::End => self.col = self.line_len(),
            _ => return false,
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(c: KeyCode) -> KeyEvent {
        KeyEvent::new(c, KeyModifiers::NONE)
    }

    #[test]
    fn editing() {
        let mut t = TextArea::multi("");
        t.insert_str("héllo\nworld");
        assert_eq!(t.text(), "héllo\nworld");
        t.handle_key(key(KeyCode::Up));
        t.handle_key(key(KeyCode::End));
        t.handle_key(key(KeyCode::Backspace));
        assert_eq!(t.text(), "héll\nworld");
        t.handle_key(key(KeyCode::Delete));
        assert_eq!(t.text(), "héllworld");
        t.handle_key(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::CONTROL));
        assert_eq!(t.text(), "world");
    }

    #[test]
    fn single_line_ignores_enter() {
        let mut t = TextArea::single("ab");
        t.handle_key(key(KeyCode::Enter));
        t.insert_str("c\nd");
        assert_eq!(t.text(), "abcd");
    }
}
