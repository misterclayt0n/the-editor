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

use ropey::{Rope, RopeSlice};
use thiserror::Error;

pub struct TextEngine {
    rope: Rope,
}

impl TextEngine {
    /// Creates a new empty `TextEngine`
    pub fn new() -> Self {
        TextEngine { rope: Rope::new() }
    }

    pub fn from_file<P>(path: P) -> Result<Self, TextEngineError>
    where
        P: AsRef<Path>,
    {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let rope = Rope::from_reader(reader)?;
        Ok(TextEngine { rope })
    }

    pub fn len_lines(&self) -> usize {
        self.rope.len_lines()
    }

    pub fn len_chars(&self) -> usize {
        self.rope.len_chars()
    }

    pub fn lines(&self) -> ropey::iter::Lines {
        self.rope.lines()
    }

    pub fn line(&self, line_idx: usize) -> RopeSlice {
        self.rope.line(line_idx)
    }
}
