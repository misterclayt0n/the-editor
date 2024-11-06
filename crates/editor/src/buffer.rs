use std::path::Path;

use text_engine::TextEngine;

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

    /// NOTE: I'm not quite sure if returning a `Vec<String> is a good or bad idea, but
    /// it seems in fact quite easier then returning `RopeSlice` when it comes to rendering.
    ///
    /// Returns all lines of the `Buffer` in a `Vec<String>` format.
    ///
    /// Removes '\n' and empty lines from the end.
    pub fn get_lines(&self) -> Vec<String> {
        let mut lines = self.text_engine
            .lines()
            .map(|line| {
                let mut s = line.to_string();
                if s.ends_with('\n') {
                    s.pop(); // Remove '\n' from the end.
                }
                s
            })
            .collect::<Vec<String>>();

        // Remove all empty lines from the end.
        while let Some(last) = lines.last() {
            if last.trim().is_empty() {
                lines.pop();
            } else {
                break;
            }
        }

        lines
    }
}
