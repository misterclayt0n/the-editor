use events::Command;
use thiserror::Error;

/// Represents all possible errors that can occur in `editor`.
#[derive(Error, Debug)]
pub enum EditorError {
    /// Error in manipulating text buffers
    #[error("Error in manipulating text buffer: {0}")]
    BufferError(String),

    #[error("Generic error: {0}")]
    GenericError(String),
}

/// Structure that maintains the global state of the editor
pub struct EditorState {
    pub should_quit: bool,
    // TODO: buffer, windows, mode
}

impl EditorState {
    pub fn new() -> Self {
        EditorState { should_quit: false }
    }

    /// MOCK
    pub fn perform_action(&mut self) -> Result<(), EditorError> {
        Err(EditorError::GenericError("Not yet implemented".to_string()))
    }

    pub fn apply_command(&mut self, command: Command) -> Result<(), EditorError> {
        match command {
            Command::Quit => self.should_quit = true,
            Command::Print(_) => {},
            Command::None => {}
        }

        Ok(())
    }
}
