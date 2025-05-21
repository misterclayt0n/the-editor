use std::{fmt::Display, fs::File, io::BufReader, path::Path};

use anyhow::{Context, Result};
pub use ropey::{Rope, RopeSlice};
use utils::Position;

/// This encapsulates `Rope` as the main data structure of the-editor, with some
/// given modifications.
#[derive(Default)]
pub struct TextEngine {
    rope: Rope,
}

impl TextEngine {
    /// Loads a `TextEngine` from a file.
    pub fn from_file<P>(path: P) -> Result<Self>
    where
        P: AsRef<Path> + Display,
    {
        let file = File::open(&path).context(format!("Failed to read {}", &path))?;
        let reader = BufReader::new(file);
        let rope = Rope::from_reader(reader).context("Failed to turn reader into rope")?;
        Ok(TextEngine { rope })
    }

    /// Returns the length of the lines.
    pub fn len_lines(&self) -> i32 {
        self.rope.len_lines() as i32
    }

    /// Returns the length of the characters, works similarly as
    /// `len()` of a String.
    pub fn len_chars(&self) -> i32 {
        self.rope.len_chars() as i32
    }

    /// Returns an iterator over the rope's lines.
    pub fn lines(&self) -> ropey::iter::Lines {
        self.rope.lines()
    }

    /// Get a `RopeSlice` at a given line index.
    pub fn line(&self, line_idx: i32) -> RopeSlice {
        self.rope.line(line_idx as usize)
    }

    pub fn char(&self, char_idx: i32) -> char {
        self.rope.char(char_idx as usize)
    }

    /// Returns a line with removed '\n' and empty lines from the end.
    pub fn get_trimmed_line(&self, line_idx: i32) -> RopeSlice {
        let line = self.rope.line(line_idx as usize);
        let len = line.len_chars();

        if len == 0 {
            // Empty line, just return the mf.
            return line;
        }

        let last_char = line.char(len - 1);

        if last_char == '\n' || last_char == '\r' {
            return line.slice(..len - 1);
        }

        line
    }

    pub fn len_nonempty_lines(&self) -> i32 {
        let num_lines = self.len_lines();

        for idx in (0..num_lines).rev() {
            let line = self.get_trimmed_line(idx);
            if !line.chars().all(|c| c.is_whitespace()) {
                return idx + 1;
            }
        }
        0
    }

    pub fn line_to_char(&self, line_idx: i32) -> i32 {
        self.rope.line_to_char(line_idx as usize) as i32
    }

    /// Transforms a character index into a `Position`.
    pub fn char_idx_to_position(&self, char_idx: i32) -> Position {
        let line_idx = self.rope.char_to_line(char_idx as usize);
        let line_start_idx = self.rope.line_to_char(line_idx);
        let char_in_line = char_idx - line_start_idx as i32;

        Position {
            x: char_in_line,
            y: line_idx as i32,
        }
    }

    //
    // Editing
    //

    /// Inserts a character at a given index.
    pub fn insert_char(&mut self, idx: i32, c: char) {
        self.rope.insert_char(idx as usize, c)
    }

    /// Deletes a character before the given index (backspace).
    pub fn delete_char_backward(&mut self, idx: i32) {
        if idx == 0 {
            return;
        }

        let idx = idx as usize;
        self.rope.remove(idx - 1..idx);
    }

    pub fn delete_char_forward(&mut self, idx: i32) {
        if idx >= self.rope.len_chars() as i32 {
            return;
        }
        
        let idx = idx as usize;
        self.rope.remove(idx..idx + 1);
    }
}
