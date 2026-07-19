use unicode_width::UnicodeWidthChar;

use crate::tui::utils::wrap_hard;

pub struct InputBuffer {
    content: String,
    cursor: usize,
}

impl InputBuffer {
    pub fn new() -> Self {
        Self {
            content: String::new(),
            cursor: 0,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.content.trim().is_empty()
    }

    pub fn take(&mut self) -> String {
        let s = std::mem::take(&mut self.content);
        self.cursor = 0;
        s
    }

    pub fn insert(&mut self, c: char) {
        self.content.insert(self.cursor, c);
        self.cursor += c.len_utf8();
    }

    pub fn insert_newline(&mut self) {
        self.content.insert(self.cursor, '\n');
        self.cursor += 1;
    }

    pub fn backspace(&mut self) {
        if self.cursor > 0 {
            let prev = self.content[..self.cursor]
                .char_indices()
                .last()
                .map(|(i, _)| i)
                .unwrap_or(0);
            self.content.remove(prev);
            self.cursor = prev;
        }
    }

    pub fn delete_word_before_cursor(&mut self) {
        if self.cursor == 0 {
            return;
        }
        let text = &self.content[..self.cursor];
        let mut end = text.len();

        // Skip trailing whitespace
        while end > 0 {
            if let Some((i, c)) = text[..end].char_indices().last() {
                if c.is_whitespace() {
                    end = i;
                } else {
                    break;
                }
            } else {
                break;
            }
        }

        // Skip word characters
        while end > 0 {
            if let Some((i, c)) = text[..end].char_indices().last() {
                if !c.is_whitespace() {
                    end = i;
                } else {
                    break;
                }
            } else {
                break;
            }
        }

        self.content.replace_range(end..self.cursor, "");
        self.cursor = end;
    }

    pub fn delete_to_start_of_line(&mut self) {
        if self.cursor == 0 {
            self.backspace();
            return;
        }
        let start = self.content[..self.cursor]
            .rfind('\n')
            .map(|i| i + 1)
            .unwrap_or(0);
        self.content.replace_range(start..self.cursor, "");
        self.cursor = start;
    }

    pub fn move_left(&mut self) {
        if self.cursor > 0 {
            self.cursor = self.content[..self.cursor]
                .char_indices()
                .last()
                .map(|(i, _)| i)
                .unwrap_or(0);
        }
    }

    pub fn move_right(&mut self) {
        if self.cursor < self.content.len() {
            let mut chars = self.content[self.cursor..].char_indices();
            if let Some((_, c)) = chars.next() {
                self.cursor += c.len_utf8();
            }
        }
    }

    pub fn move_up(&mut self) -> bool {
        if let Some(pos) = self.content[..self.cursor].rfind('\n') {
            self.cursor = pos;
            true
        } else if self.cursor > 0 {
            self.cursor = 0;
            true
        } else {
            false
        }
    }

    pub fn move_down(&mut self) -> bool {
        if self.cursor < self.content.len() {
            if let Some(pos) = self.content[self.cursor + 1..].find('\n') {
                self.cursor = self.cursor + 1 + pos;
            } else {
                self.cursor = self.content.len();
            }
            true
        } else {
            false
        }
    }

    pub fn wrapped(&self, width: usize) -> Vec<String> {
        wrap_hard(&self.content, width)
    }

    pub fn cursor_xy(&self, width: usize) -> (u16, u16) {
        let mut y = 0u16;
        let mut x = 0u16;
        let mut chars_seen = 0usize;

        for line in self.content.lines() {
            let mut col_in_line = 0usize;
            for ch in line.chars() {
                if chars_seen == self.cursor {
                    x = col_in_line as u16;
                    return (x, y);
                }
                col_in_line += ch.width().unwrap_or(0);
                if col_in_line > width {
                    y += 1;
                    col_in_line = ch.width().unwrap_or(0);
                }
                chars_seen += ch.len_utf8();
            }

            if chars_seen == self.cursor {
                x = col_in_line as u16;
                return (x, y);
            }

            if chars_seen < self.cursor {
                chars_seen += 1; // '\n'
                y += 1;
            }
        }

        if chars_seen == self.cursor && self.content.ends_with('\n') {
            x = 0;
        }

        (x, y)
    }
}
