use std::path::Path;

use text_engine::{RopeSlice, TextEngine};

use crate::EditorError;

pub struct Buffer {
    text_engine: TextEngine,
    _file_path: Option<String>, // File associated with `Buffer`.
}

impl Buffer {
    pub fn new() -> Self {
        Self {
            text_engine: TextEngine::new(),
            _file_path: None
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
            _file_path: Some(file_path),
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
        self.text_engine.get_trimmed_line(line_idx).len_chars().saturating_sub(1)
    }
}
