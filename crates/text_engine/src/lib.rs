use std::{
    fs::File,
    io::{self, BufReader},
    path::Path,
};

/// Represents all possible errors that can occur in `text_engine`.
#[derive(Error, Debug)]
pub enum TextEngineError {
    /// Error in IO operations
    #[error("Crossterm error: {0}")]
    IOError(#[from] io::Error),

    #[error("Generic error: {0}")]
    GenericError(String),
}

pub use ropey::{Rope, RopeSlice};
use thiserror::Error;
use utils::Position;

/// This encapsulates `Rope` as the main data structure of the-editor, with some
/// given modifications.
pub struct TextEngine {
    rope: Rope,
}

impl TextEngine {
    /// Creates a new empty `TextEngine`.
    pub fn new() -> Self {
        TextEngine { rope: Rope::new() }
    }

    /// Loads a `TextEngine` from a file.
    pub fn from_file<P>(path: P) -> Result<Self, TextEngineError>
    where
        P: AsRef<Path>,
    {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let rope = Rope::from_reader(reader)?;
        Ok(TextEngine { rope })
    }

    /// Returns the length of the lines.
    pub fn len_lines(&self) -> usize {
        self.rope.len_lines()
    }

    /// Returns the length of the characters, works similarly as
    /// `len()` of a String.
    pub fn len_chars(&self) -> usize {
        self.rope.len_chars()
    }

    /// Returns an iterator over the rope's lines.
    pub fn lines(&self) -> ropey::iter::Lines {
        self.rope.lines()
    }

    /// Get a `RopeSlice` at a given line index.
    pub fn line(&self, line_idx: usize) -> RopeSlice {
        self.rope.line(line_idx)
    }

    pub fn char(&self, char_idx: usize) -> char {
        self.rope.char(char_idx)
    }

    /// Returns a line with removed '\n' and empty lines from the end.
    /// This mostly exists for rendering. Buffer operations should probably not be done
    /// using this method.
    pub fn get_trimmed_line(&self, line_idx: usize) -> RopeSlice {
        let line = self.rope.line(line_idx);
        let len = line.len_chars();

        if len == 0 {
            // Empty line, just return the mf.
            return line;
        }

        let last_char = line.char(len - 1);

        if last_char == '\n' || last_char == '\r' {
            return line.slice(..len - 1);
        }

        return line;
    }

    pub fn len_nonempty_lines(&self) -> usize {
        let num_lines = self.len_lines();

        for idx in (0..num_lines).rev() {
            let line = self.get_trimmed_line(idx);
            if !line.chars().all(|c| c.is_whitespace()) {
                return idx + 1;
            }
        }
        0
    }

    pub fn line_to_char(&self, line_idx: usize) -> usize {
        self.rope.line_to_char(line_idx)
    }

    /// Transforms a character index into a `Position`
    pub fn char_idx_to_position(&self, char_idx: usize) -> Position {
        let line_idx = self.rope.char_to_line(char_idx);
        let line_start_idx = self.rope.line_to_char(line_idx);
        let char_in_line = char_idx - line_start_idx;

        Position {
            x: char_in_line,
            y: line_idx
        }
    }
}
