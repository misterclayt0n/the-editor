use events::{Event, EventHandler};
use movement::{
    move_cursor_after_insert, move_cursor_before_deleting_backward, move_cursor_down,
    move_cursor_end_of_line, move_cursor_first_char_of_line, move_cursor_left, move_cursor_right,
    move_cursor_start_of_line, move_cursor_up, move_cursor_word_backward, move_cursor_word_forward,
    move_cursor_word_forward_end,
};
use renderer::{
    terminal::{Terminal, TerminalInterface},
    Component, Renderer,
};
use status_bar::StatusBar;
use thiserror::Error;
use utils::{Command, Mode, Size};
use window::Window;
mod buffer;
mod movement;
mod status_bar;
mod window;

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
    window: Window, // NOTE: I should probably implement some sort of window manager.
    mode: Mode,
    status_bar: StatusBar,
    renderer: Renderer<T>,
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

        let window = Window::from_file(file_path)?;
        let (width, height) = Terminal::size()
            .map_err(|e| EditorError::RenderError(format!("Could not initialize viewport: {e}")))?;

        let viewport_size = Size { width, height };

        let status_bar = StatusBar::new(viewport_size);

        Ok(EditorState {
            should_quit: false,
            event_handler,
            window,
            mode: Mode::Normal, // Start with Normal mode.
            status_bar,
            renderer,
        })
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
                                        self.renderer.enqueue_command(
                                            renderer::TerminalCommand::Print(format!(
                                                "ERROR: {}",
                                                e
                                            )),
                                        );
                                    }
                                }
                            }
                            Err(e) => {
                                self.renderer
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
                    }
                    _ => {}
                }
            }

            if self.window.needs_redraw {
                self.render()?;
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
            Command::MoveCursorRight(exceed) => {
                move_cursor_right(&mut self.window.cursor, &self.window.buffer, exceed)
            }
            Command::MoveCursorUp => move_cursor_up(&mut self.window.cursor, &self.window.buffer),
            Command::MoveCursorDown => {
                move_cursor_down(&mut self.window.cursor, &self.window.buffer)
            }
            Command::MoveCursorEndOfLine => {
                move_cursor_end_of_line(&mut self.window.cursor, &self.window.buffer)
            }
            Command::MoveCursorStartOfLine => move_cursor_start_of_line(&mut self.window.cursor),
            Command::MoveCursorFirstCharOfLine => {
                move_cursor_first_char_of_line(&mut self.window.cursor, &self.window.buffer)
            }
            Command::MoveCursorWordForward(big_word) => {
                move_cursor_word_forward(&mut self.window.cursor, &self.window.buffer, big_word)
            }
            Command::MoveCursorWordBackward(big_word) => {
                move_cursor_word_backward(&mut self.window.cursor, &self.window.buffer, big_word)
            }
            Command::MoveCursorWordForwardEnd(big_word) => {
                move_cursor_word_forward_end(&mut self.window.cursor, &self.window.buffer, big_word)
            }
            Command::None => {}
            Command::SwitchMode(mode) => self.switch_mode(mode),
            Command::Resize(new_size) => self.handle_resize(new_size)?,
            Command::InsertChar(c) => {
                self.window
                    .buffer
                    .insert_char(self.window.cursor.position, c);
                move_cursor_after_insert(&mut self.window.cursor, c)
            }
            Command::DeleteCharBackward => {
                self.window
                    .buffer
                    .delete_char_backward(self.window.cursor.position);
                move_cursor_before_deleting_backward(&mut self.window.cursor, &self.window.buffer);
            }
            Command::DeleteCharForward => {
                self.window
                    .buffer
                    .delete_char_forward(self.window.cursor.position);
            }
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
        self.status_bar.size = new_size;

        Ok(())
    }

    fn switch_mode(&mut self, mode: Mode) {
        match mode {
            Mode::Insert => self
                .renderer
                .enqueue_command(renderer::TerminalCommand::ChangeCursorStyleBar),
            Mode::Normal => self
                .renderer
                .enqueue_command(renderer::TerminalCommand::ChangeCursorStyleBlock),
        }

        self.mode = mode;
    }

    fn render(&mut self) -> Result<(), EditorError> {
        let file_name = self.window.buffer.file_path.clone();
        let cursor_position = self.window.cursor.position.clone();

        self.status_bar
            .render(&mut self.renderer)
            .map_err(|e| EditorError::RenderError(format!("Could not render status bar: {e}")))?;

        self.window
            .render(&mut self.renderer)
            .map_err(|e| EditorError::RenderError(format!("Could not render window: {e}")))?;

        self.status_bar
            .update(self.mode, file_name, cursor_position);

        self.renderer
            .render()
            .map_err(|e| EditorError::RenderError(format!("Could not flush renderer: {e}")))?;

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
