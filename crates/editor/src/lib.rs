use events::{Event, EventHandler};
use movement::{move_cursor_down, move_cursor_end_of_line, move_cursor_first_char_of_line, move_cursor_left, move_cursor_right, move_cursor_start_of_line, move_cursor_up, move_cursor_word_forward};
use renderer::{
    terminal::{Terminal, TerminalInterface},
    Component, Renderer,
};
use thiserror::Error;
use utils::{Command, Mode, Size};
use window::Window;
mod buffer;
mod window;
mod movement;

/// Represents all possible errors that can occur in `editor`.
#[derive(Error, Debug)]
pub enum EditorError {
    /// Error in manipulating text buffers.
    #[error("Error in manipulating text buffer: {0}")]
    BufferError(String),

    /// Error in capturing events.
    #[error("Error in capturing events: {0}")]
    EventError(String),

    /// Error in rendering.
    #[error("Error in rendering: {0}")]
    RenderError(String),

    /// Error in terminal.
    #[error("Error in terminal: {0}")]
    TerminalError(String),

    #[error("Generic error: {0}")]
    GenericError(String),
}

/// Structure that maintains the global state of the editor.
pub struct EditorState<T: TerminalInterface> {
    should_quit: bool,
    event_handler: EventHandler,
    window: Window<T>, // NOTE: I should probably implement some sort of window manager.
    mode: Mode,
}

impl<T> EditorState<T>
where
    T: TerminalInterface,
{
    pub fn new(
        event_handler: EventHandler,
        renderer: Renderer<T>,
        file_path: Option<String>,
    ) -> Result<Self, EditorError> {
        Terminal::init().map_err(|e| {
            EditorError::TerminalError(format!("Could not initialize terminal: {e}"))
        })?;

        let window = Window::from_file(renderer, file_path)?;

        Ok(EditorState {
            should_quit: false,
            event_handler,
            window,
            mode: Mode::Normal, // Start with Normal mode.
        })
    }

    /// MOCK
    pub fn perform_action(&mut self) -> Result<(), EditorError> {
        Err(EditorError::GenericError("Not yet implemented".to_string()))
    }

    /// Main entrypoint of the application.
    pub fn run(&mut self) -> Result<(), EditorError> {
        loop {
            // Capture events.
            let events = self
                .event_handler
                .poll_events()
                .map_err(|e| EditorError::EventError(format!("Failed to poll events: {e}")))?;

            for event in events {
                match event {
                    Event::KeyPress(key_event) => {
                        match self.event_handler.handle_key_event(key_event, self.mode) {
                            Ok(commands) => {
                                for command in commands {
                                    if let Err(e) = self.apply_command(command) {
                                        self.window.enqueue_command(
                                            renderer::TerminalCommand::Print(format!(
                                                "ERROR: {}",
                                                e
                                            )),
                                        );
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
                    Event::Resize(width, height) => {
                        // Handle resize
                        let new_size = Size { width, height };
                        self.apply_command(Command::Resize(new_size))?;
                    },
                    _ => {}
                }
            }

            if self.window.needs_redraw {
                self.window.render().map_err(|e| {
                    EditorError::RenderError(format!("Failed to render window: {e}"))
                })?;
            }

            if self.should_quit {
                break;
            };
        }

        Ok(())
    }

    /// Proccess a command and apply it to the editor state.
    pub fn apply_command(&mut self, command: Command) -> Result<(), EditorError> {
        match command {
            Command::Quit => self.should_quit = true,
            Command::MoveCursorLeft => move_cursor_left(&mut self.window.cursor),
            Command::MoveCursorRight => move_cursor_right(&mut self.window.cursor, &self.window.buffer),
            Command::MoveCursorUp => move_cursor_up(&mut self.window.cursor, &self.window.buffer),
            Command::MoveCursorDown => move_cursor_down(&mut self.window.cursor, &self.window.buffer),
            Command::MoveCursorEndOfLine => move_cursor_end_of_line(&mut self.window.cursor, &self.window.buffer),
            Command::MoveCursorStartOfLine => move_cursor_start_of_line(&mut self.window.cursor),
            Command::MoveCursorFirstCharOfLine => move_cursor_first_char_of_line(&mut self.window.cursor, &self.window.buffer),
            Command::MoveCursorWordForward(big_word) => move_cursor_word_forward(&mut self.window.cursor, &self.window.buffer, big_word),
            Command::None => {}
            Command::SwitchMode(mode) => self.mode = mode,
            Command::Resize(new_size) => self.handle_resize(new_size)?,
        }

        self.window.scroll_to_cursor();
        self.window.needs_redraw = true;
        Ok(())
    }

    /// Updates the viewport size, scroll if necessary and mark the window for a
    /// redraw.
    fn handle_resize(&mut self, new_size: Size) -> Result<(), EditorError> {
        self.window.viewport_size = new_size;
        self.window.scroll_to_cursor();
        self.window.needs_redraw = true;

        Ok(())
    }
}

impl<T: TerminalInterface> Drop for EditorState<T> {
    fn drop(&mut self) {
        if let Err(_) = Terminal::kill() {
            // Do nothing for now.
        }
    }
}
