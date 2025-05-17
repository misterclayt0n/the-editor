use terminal::TerminalInterface;
use text_engine::Rope;
pub mod terminal;

/// `Component` is like a React component, but for the-editor.
pub trait Component {
    /// Renders the `Component`, does not require a `Renderer`, it assumes the `Component` has it's own.
    fn render<T>(&mut self, renderer: &mut Renderer<T>)
    where
        T: TerminalInterface;
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
    ClearLine,
    ForceError,
}

/// Renderer is responsible for rendering the state of the editor in the terminal.
#[derive(Clone)]
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
    pub fn render(&mut self) {
        for command in &self.command_queue {
            self.terminal.queue(command.clone());
        }

        self.terminal.flush();
        self.command_queue.clear();
    }
}
