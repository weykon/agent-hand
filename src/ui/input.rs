/// A simple text input component with cursor support
#[derive(Debug, Clone, Default)]
pub struct TextInput {
    /// The text content
    text: String,
    /// Cursor position (byte index)
    cursor: usize,
}

impl TextInput {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_text(text: impl Into<String>) -> Self {
        let text = text.into();
        let cursor = text.len();
        Self { text, cursor }
    }

    /// Get the text content
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Get cursor position (byte index)
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// Set text and move cursor to end
    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
        self.cursor = self.text.len();
    }

    /// Insert a character at cursor position
    pub fn insert(&mut self, ch: char) {
        self.text.insert(self.cursor, ch);
        self.cursor += ch.len_utf8();
    }

    /// Delete character before cursor (backspace)
    pub fn backspace(&mut self) {
        if self.cursor > 0 {
            // Find the previous char boundary
            let prev = self.text[..self.cursor]
                .char_indices()
                .last()
                .map(|(i, _)| i)
                .unwrap_or(0);
            self.text.remove(prev);
            self.cursor = prev;
        }
    }

    /// Delete character at cursor (delete key)
    pub fn delete(&mut self) {
        if self.cursor < self.text.len() {
            self.text.remove(self.cursor);
        }
    }

    /// Move cursor left
    pub fn move_left(&mut self) {
        if self.cursor > 0 {
            // Find previous char boundary
            self.cursor = self.text[..self.cursor]
                .char_indices()
                .last()
                .map(|(i, _)| i)
                .unwrap_or(0);
        }
    }

    /// Move cursor right
    pub fn move_right(&mut self) {
        if self.cursor < self.text.len() {
            // Find next char boundary
            self.cursor = self.text[self.cursor..]
                .char_indices()
                .nth(1)
                .map(|(i, _)| self.cursor + i)
                .unwrap_or(self.text.len());
        }
    }

    /// Move cursor to start
    pub fn move_home(&mut self) {
        self.cursor = 0;
    }

    /// Move cursor to end
    pub fn move_end(&mut self) {
        self.cursor = self.text.len();
    }

    /// Clear all text
    pub fn clear(&mut self) {
        self.text.clear();
        self.cursor = 0;
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    /// Get cursor position in characters (for display)
    pub fn cursor_char_pos(&self) -> usize {
        self.text[..self.cursor].chars().count()
    }
}

impl From<String> for TextInput {
    fn from(text: String) -> Self {
        Self::with_text(text)
    }
}

impl From<&str> for TextInput {
    fn from(text: &str) -> Self {
        Self::with_text(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_operations() {
        let mut input = TextInput::new();
        
        input.insert('a');
        input.insert('b');
        input.insert('c');
        assert_eq!(input.text(), "abc");
        assert_eq!(input.cursor(), 3);
        
        input.backspace();
        assert_eq!(input.text(), "ab");
        assert_eq!(input.cursor(), 2);
        
        input.move_left();
        assert_eq!(input.cursor(), 1);
        
        input.insert('x');
        assert_eq!(input.text(), "axb");
        assert_eq!(input.cursor(), 2);
    }

    #[test]
    fn test_unicode() {
        let mut input = TextInput::with_text("你好");
        assert_eq!(input.cursor(), 6); // 2 chars × 3 bytes
        
        input.move_left();
        assert_eq!(input.cursor(), 3);
        assert_eq!(input.cursor_char_pos(), 1);
        
        input.insert('世');
        assert_eq!(input.text(), "你世好");
    }
}
