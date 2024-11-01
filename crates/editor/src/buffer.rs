use std::path::Path;

use text_engine::TextEngine;

use crate::EditorError;

pub struct Buffer {
    text_engine: TextEngine,
    file_path: Option<String>, // File associated with `Buffer`
}

impl Buffer {
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

    pub fn get_lines(&self) -> Vec<String> {
        self.text_engine.lines().map(|line| line.to_string()).collect()
    }
}
