use crossterm::event::KeyCode;

#[derive(Clone, Default)]
pub struct LineEdit {
    pub text: String,
    pub cursor: usize,
}

impl LineEdit {
    pub fn from(text: impl Into<String>) -> Self {
        let text = text.into();
        let cursor = text.len();
        Self { text, cursor }
    }

    pub fn clear(&mut self) {
        self.text.clear();
        self.cursor = 0;
    }

    pub fn insert(&mut self, c: char) {
        if c.is_control() {
            return;
        }
        self.text.insert(self.cursor, c);
        self.cursor += c.len_utf8();
    }

    pub fn paste(&mut self, text: &str) {
        for c in text.chars() {
            if c != '\n' {
                self.insert(c);
            }
        }
    }

    pub fn on_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Backspace => self.backspace(),
            KeyCode::Delete => self.delete(),
            KeyCode::Left => self.left(),
            KeyCode::Right => self.right(),
            KeyCode::Home => self.cursor = 0,
            KeyCode::End => self.cursor = self.text.len(),
            KeyCode::Char(c) => self.insert(c),
            _ => {}
        }
    }

    fn backspace(&mut self) {
        if self.cursor == 0 {
            return;
        }
        let prev = prev_boundary(&self.text, self.cursor);
        self.text.drain(prev..self.cursor);
        self.cursor = prev;
    }

    fn delete(&mut self) {
        if self.cursor >= self.text.len() {
            return;
        }
        let next = next_boundary(&self.text, self.cursor);
        self.text.drain(self.cursor..next);
    }

    fn left(&mut self) {
        self.cursor = prev_boundary(&self.text, self.cursor);
    }

    fn right(&mut self) {
        self.cursor = next_boundary(&self.text, self.cursor);
    }
}

fn prev_boundary(s: &str, cursor: usize) -> usize {
    s[..cursor]
        .chars()
        .next_back()
        .map(|c| cursor - c.len_utf8())
        .unwrap_or(0)
}

fn next_boundary(s: &str, cursor: usize) -> usize {
    s[cursor..]
        .chars()
        .next()
        .map(|c| cursor + c.len_utf8())
        .unwrap_or(cursor)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf8_backspace() {
        let mut line = LineEdit::default();
        line.insert('ñ');
        line.on_key(KeyCode::Backspace);
        assert!(line.text.is_empty());
        assert_eq!(line.cursor, 0);
    }
}
