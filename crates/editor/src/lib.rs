use events::{Command, Event, EventHandler};
use renderer::{
    terminal::{Terminal, TerminalInterface},
    Renderer,
};
use thiserror::Error;
use window::Window;
mod window;

/// Represents all possible errors that can occur in `editor`.
#[derive(Error, Debug)]
pub enum EditorError {
    /// Error in manipulating text buffers
    #[error("Error in manipulating text buffer: {0}")]
    BufferError(String),

    /// Error in capturing events
    #[error("Error in capturing events: {0}")]
    EventError(String),

    /// Error in rendering
    #[error("Error in rendering: {0}")]
    RenderError(String),

    /// Error in terminal
    #[error("Error in terminal: {0}")]
    TerminalError(String),

    #[error("Generic error: {0}")]
    GenericError(String),
}

// TODO: buffer, mode
/// Structure that maintains the global state of the editor
pub struct EditorState<T: TerminalInterface> {
    pub should_quit: bool,
    event_handler: EventHandler,
    window: Window<T>, // NOTE: I should probably implement some sort of window manager
}

impl<T> EditorState<T>
where
    T: TerminalInterface,
{
    pub fn new(event_handler: EventHandler, renderer: Renderer<T>) -> Result<Self, EditorError> {
        Terminal::init().map_err(|e| {
            EditorError::TerminalError(format!("Could not initialize terminal: {e}"))
        })?;

        Ok(EditorState {
            should_quit: false,
            event_handler,
            window: Window::new(renderer),
        })
    }

    /// MOCK
    pub fn perform_action(&mut self) -> Result<(), EditorError> {
        Err(EditorError::GenericError("Not yet implemented".to_string()))
    }

    /// Main entrypoint of the application
    pub fn run(&mut self) -> Result<(), EditorError> {
        self.window.display();

        loop {
            // Capture events
            let events = self
                .event_handler
                .poll_events()
                .map_err(|e| EditorError::EventError(format!("Failed to poll events: {e}")))?;

            for event in events {
                if let Event::KeyPress(key_event) = event {
                    match self.event_handler.handle_key_event(key_event) {
                        Ok(commands) => {
                            for command in commands {
                                if let Err(e) = self.apply_command(command) {
                                    self.window
                                        .enqueue_command(renderer::TerminalCommand::Print(
                                            format!("ERROR: {}", e),
                                        ));
                                }
                            }
                        }
                        Err(e) => {
                            self.window
                                .enqueue_command(renderer::TerminalCommand::Print(format!(
                                    "ERROR: {}",
                                    e
                                )));
                        }
                    }
                }
            }

            self.window
                .render()
                .map_err(|e| EditorError::RenderError(format!("Failed to render window: {e}")))?;

            if self.should_quit {
                break;
            };
        }

        Ok(())
    }

    /// Proccess a command and apply it to the editor state
    pub fn apply_command(&mut self, command: Command) -> Result<(), EditorError> {
        match command {
            Command::Quit => self.should_quit = true,
            Command::Print(_) => {}
            Command::None => {}
        }

        Ok(())
    }
}

impl<T: TerminalInterface> Drop for EditorState<T> {
    fn drop(&mut self) {
        if let Err(_) = Terminal::kill() {
            // Do nothing for now
        }
    }
}
