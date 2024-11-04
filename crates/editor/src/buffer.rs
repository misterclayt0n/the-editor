use std::path::Path;

use renderer::{Component, TerminalCommand};
use text_engine::TextEngine;

use crate::EditorError;

pub struct Buffer {
    text_engine: TextEngine,
    file_path: Option<String>, // File associated with `Buffer`
}

impl Component for Buffer {
    /// Dictates how a `Buffer` should be rendered
    fn render(&self) -> Vec<TerminalCommand> {
        let mut commands = Vec::new();

        commands.push(TerminalCommand::MoveCursor(0, 0));

        for (line_num, line) in self.get_lines().iter().enumerate() {
            commands.push(TerminalCommand::MoveCursor(0, line_num));
            commands.push(TerminalCommand::Print(line.to_owned()))
        }

        return commands;
    }
}

impl Buffer {
    /// Returns a `Buffer` with a file loaded
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
