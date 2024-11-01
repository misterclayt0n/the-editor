
use terminal::TerminalInterface;
use thiserror::Error;
pub mod terminal;

/// Represents all commands that can be queued to be rendered
#[derive(Debug, Clone)]
pub enum TerminalCommand {
    ClearScreen,
    Print(String),
    MoveCursor(usize, usize),
    HideCursor,
    ShowCursor,
    // NOTE: Add variants as needed
}

/// Represents all possible errors that can occur in `renderer`.
#[derive(Error, Debug)]
pub enum RendererError {
    /// Error in manipulating the terminal
    #[error("Crossterm error: {0}")]
    TerminalError(String),

    #[error("Generic error: {0}")]
    GenericError(String)
}

/// Renderer is responsible for rendering the state of the editor in the terminal
pub struct Renderer<T: TerminalInterface> {
    terminal: T,
    command_queue: Vec<TerminalCommand>
}

impl<T: TerminalInterface> Renderer<T> {
    pub fn new(terminal: T) -> Self {
        Renderer {
            terminal,
            command_queue: Vec::new(),
        }
    }

    /// Add a `Command` to the command queue
    pub fn enqueue_command(&mut self, command: TerminalCommand) {
        self.command_queue.push(command)
    }

    pub fn welcome_screen(&mut self) {
        self.enqueue_command(TerminalCommand::ClearScreen);
        self.enqueue_command(TerminalCommand::MoveCursor(0, 0));
        self.enqueue_command(TerminalCommand::HideCursor);
        self.enqueue_command(TerminalCommand::Print("welcome to the-editor, press 'q' to quit".to_string()));
        self.enqueue_command(TerminalCommand::ShowCursor);
    }

    /// Renders all enqueued commands
    pub fn render(&mut self) -> Result<(), RendererError> {
        for command in &self.command_queue {
            self.terminal.queue(command.clone())?;
        }

        self.terminal.flush()?;
        self.command_queue.clear();

        Ok(())
    }
}
