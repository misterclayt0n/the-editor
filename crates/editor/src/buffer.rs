use std::path::Path;

use text_engine::{RopeSlice, TextEngine};
use utils::{get_char_class, CharClass, Position};

use crate::EditorError;

pub struct Buffer {
    text_engine: TextEngine,
    pub file_path: Option<String>, // File associated with `Buffer`.
}

impl Buffer {
    pub fn new() -> Self {
        Self {
            text_engine: TextEngine::new(),
            file_path: None,
        }
    }

    /// Returns a `Buffer` with a file loaded.
    pub fn open<P>(path: P) -> Result<Self, EditorError>
    where
        P: AsRef<Path>,
    {
        let text_engine = TextEngine::from_file(&path)
            .map_err(|e| EditorError::BufferError(format!("Could not load text engine: {e}")))?;
        let file_path = path.as_ref().to_string_lossy().to_string();

        Ok(Buffer {
            text_engine,
            file_path: Some(file_path),
        })
    }

    /// Returns a line with removed '\n' and empty lines from the end.
    /// This avoids the issue of not rendering the first character.
    pub fn get_trimmed_line(&self, line_idx: usize) -> RopeSlice {
        self.text_engine.get_trimmed_line(line_idx)
    }

    /// Returns the length of non empty lines of the `TextEngine`.
    pub fn len_nonempty_lines(&self) -> usize {
        self.text_engine.len_nonempty_lines()
    }

    /// Returns only the visible portion of the line, by subtracting by 1.
    pub fn get_visible_line_length(&self, line_idx: usize) -> usize {
        // `saturating_sub` to avoid underflow.
        self.text_engine
            .get_trimmed_line(line_idx)
            .len_chars()
            .saturating_sub(1)
    }

    /// Returns the index of the start of the next word from a given position.
    pub fn find_next_word_start(&self, position: Position, big_word: bool) -> Option<Position> {
        let total_chars = self.text_engine.len_chars();
        let line_start = self.text_engine.line_to_char(position.y);
        let mut char_idx = line_start + position.x;

        if char_idx >= total_chars {
            return None;
        }

        let c = self.text_engine.char(char_idx);

        // Determine the character class.
        let current_class = get_char_class(c, big_word);

        // Skip over characters of the same class.
        while char_idx < total_chars {
            let c = self.text_engine.char(char_idx);
            let class = get_char_class(c, big_word);

            if class == current_class {
                char_idx += c.len_utf8();
            } else {
                break;
            }
        }

        // Skip over any whitespace characters.
        while char_idx < total_chars {
            let c = self.text_engine.char(char_idx);
            if get_char_class(c, big_word) == CharClass::Whitespace {
                char_idx += c.len_utf8();
            } else {
                break;
            }
        }

        if char_idx >= total_chars {
            return None;
        }

        Some(self.text_engine.char_idx_to_position(char_idx))
    }

    /// Returns the index of the previous word from a given position.
    pub fn find_prev_word_start(&self, position: Position, big_word: bool) -> Option<Position> {
        let line_start = self.text_engine.line_to_char(position.y);
        let mut char_idx = line_start + position.x;

        if char_idx == 0 {
            return None;
        }

        // Move the cursor one step back to start looking at the previous character.
        char_idx = char_idx.saturating_sub(1);

        // Skip any trailing whitespace.
        while char_idx > 0 {
            let c = self.text_engine.char(char_idx);
            if get_char_class(c, big_word) == CharClass::Whitespace {
                char_idx = char_idx.saturating_sub(c.len_utf8());
            } else {
                break;
            }
        }

        if char_idx == 0 {
            return Some(self.text_engine.char_idx_to_position(0));
        }

        // Get the class of the character at the new position.
        let current_class = get_char_class(self.text_engine.char(char_idx), big_word);

        // Skip all characters that are of the same class.
        while char_idx > 0 {
            let c = self.text_engine.char(char_idx);
            if get_char_class(c, big_word) == current_class {
                char_idx = char_idx.saturating_sub(c.len_utf8());
            } else {
                // stop at the boundary between different character classes
                char_idx += c.len_utf8();
                break;
            }
        }

        // Skip any leading whitespace before the next word.
        while char_idx > 0 {
            let c = self.text_engine.char(char_idx);
            if get_char_class(c, big_word) == CharClass::Whitespace {
                char_idx = char_idx.saturating_sub(c.len_utf8());
            } else {
                break;
            }
        }

        Some(self.text_engine.char_idx_to_position(char_idx))
    }

    /// Returns the index of the end of the next word from a given position.
    pub fn find_next_word_end(&self, position: Position, big_word: bool) -> Option<Position> {
        let total_chars = self.text_engine.len_chars();
        let line_start = self.text_engine.line_to_char(position.y);
        let mut char_idx = line_start + position.x;

        if char_idx >= total_chars {
            return None;
        }

        // Move forward one character if possible.
        if char_idx + 1 < total_chars {
            char_idx += 1;
        } else {
            // We're at the end of the buffer.
            return None;
        }

        // Skip over whitespace.
        while char_idx < total_chars {
            let c = self.text_engine.char(char_idx);
            if get_char_class(c, big_word) == CharClass::Whitespace {
                char_idx += c.len_utf8();
            } else {
                break;
            }
        }

        if char_idx >= total_chars {
            return None;
        }

        let current_class = get_char_class(self.text_engine.char(char_idx), big_word);

        let mut last_char_index = char_idx;

        // Move to the end of the current class sequence.
        while char_idx < total_chars {
            let c = self.text_engine.char(char_idx);
            if get_char_class(c, big_word) == current_class {
                last_char_index = char_idx;
                char_idx += c.len_utf8();
            } else {
                break;
            }
        }

        Some(self.text_engine.char_idx_to_position(last_char_index))
    }

    //
    // Editing
    //

    pub fn insert_char(&mut self, position: Position, c: char) {
        let char_idx = self.position_to_char_idx(position);
        self.text_engine.insert_char(char_idx, c);
    }

    pub fn delete_char_backward(&mut self, position: Position) {
        let char_idx = self.position_to_char_idx(position);
        if char_idx == 0 {
            // At the beginning of the buffer, nothing to delete.
            return;
        }

        self.text_engine.delete_char_backward(char_idx);
    }

    pub fn delete_char_forward(&mut self, position: Position) {
        let total_chars = self.text_engine.len_chars();
        let char_idx = self.position_to_char_idx(position);

        if char_idx >= total_chars {
            // At the end of the buffer, nothing to delete.
            return;
        }

        self.text_engine.delete_char_forward(char_idx);
    }

    //
    // Helpers
    //

    fn position_to_char_idx(&self, position: Position) -> usize {
        let line_start_idx = self.text_engine.line_to_char(position.y);
        let line_len = self.text_engine.line(position.y).len_chars();

        // Ensure cursor.x does not exceed line length
        let x = position.x.min(line_len);

        line_start_idx + x
    }
}
