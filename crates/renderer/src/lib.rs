use terminal::TerminalInterface;
use text_engine::Rope;
use thiserror::Error;
pub mod terminal;

/// `Component` is like a React component, but for the-editor.
pub trait Component {
    /// Renders the `Component`.
    fn render(&mut self) -> Result<(), RendererError>;
}

/// Represents all commands that can be queued to be rendered.
#[derive(Debug, Clone)]
pub enum TerminalCommand {
    ClearScreen,
    Print(String),
    PrintRope(Rope),
    MoveCursor(usize, usize),
    HideCursor,
    ShowCursor,
    ChangeCursorStyleBlock,
    ChangeCursorStyleBar,
}

/// Represents all possible errors that can occur in `renderer`.
#[derive(Error, Debug)]
pub enum RendererError {
    /// Error in manipulating the terminal
    #[error("Crossterm error: {0}")]
    TerminalError(String),

    #[error("Generic error: {0}")]
    GenericError(String),
}

/// Renderer is responsible for rendering the state of the editor in the terminal.
pub struct Renderer<T: TerminalInterface> {
    terminal: T,
    command_queue: Vec<TerminalCommand>,
}

impl<T: TerminalInterface> Renderer<T> {
    pub fn new(terminal: T) -> Self {
        Renderer {
            terminal,
            command_queue: Vec::new(),
        }
    }

    /// Add a `Command` to the command queue.
    pub fn enqueue_command(&mut self, command: TerminalCommand) {
        self.command_queue.push(command)
    }

    /// Renders all enqueued commands.
    pub fn render(&mut self) -> Result<(), RendererError> {
        for command in &self.command_queue {
            self.terminal.queue(command.clone())?;
        }

        self.terminal.flush()?;
        self.command_queue.clear();

        Ok(())
    }
}
