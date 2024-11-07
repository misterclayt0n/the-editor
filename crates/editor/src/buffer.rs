use std::path::Path;

use text_engine::{RopeSlice, TextEngine};

use crate::EditorError;

pub struct Buffer {
    text_engine: TextEngine,
    file_path: Option<String>, // File associated with `Buffer`.
}

impl Buffer {
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

    /// Returns a line of `Text Engine` as a `RopeSlice`.
    pub fn get_line(&self, line_idx: usize) -> RopeSlice {
        self.text_engine.line(line_idx)
    }

    /// Returns the length of non empty lines of the `TextEngine`.
    pub fn len_nonempty_lines(&self) -> usize {
        self.text_engine.len_nonempty_lines()
    }
}
