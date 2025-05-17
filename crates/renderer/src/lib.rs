use gui::Gui;
use terminal::Terminal;
use text_engine::Rope;
pub mod terminal;
pub mod gui;

/// `Component` is like a React component, but for the-editor.
pub trait Component {
    /// Renders the `Component`, does not require a `Renderer`, it assumes the `Component` has it's own.
    fn render(&mut self, renderer: &mut Renderer);
}

/// Represents all commands that can be queued to be rendered.
#[derive(Debug, Clone)]
pub enum RenderCommand {
    ClearScreen,
    Print(String),
    PrintRope(Rope),
    MoveCursor(usize, usize),
    HideCursor,
    ShowCursor,
    ClearLine,
    ForceError,
    SetInverseVideo(bool),
    SetUnderline(bool),
}

pub trait RenderInterface {
    /// Puts a `RenderCommand` into a queue.
    fn queue(&self, command: RenderCommand);

    /// Executes all commands that are in the queue.
    fn flush(&self);

    /// Kills the interface.
    fn kill(&self);

    /// Inits the interface.
    fn init(&self);

    /// Returns the size of the interface.
    fn size(&self) -> (usize, usize);
}

/// Renderer is responsible for rendering the state of the editor in the terminal.
pub struct Renderer {
    pub interface: Box<dyn RenderInterface>,
    pub command_queue: Vec<RenderCommand>,
}

impl Renderer {
    pub fn new(terminal: bool) -> Self {
        let interface: Box<dyn RenderInterface> = match terminal {
            true => Box::new(Terminal::new()),
            false => Box::new(Gui::new()),
        };
        
        Renderer {
            interface,
            command_queue: Vec::new(),
        }
    }

    /// Add a `Command` to the command queue.
    pub fn enqueue_command(&mut self, command: RenderCommand) {
        self.command_queue.push(command)
    }

    /// Renders all enqueued commands.
    pub fn render(&mut self) {
        for command in &self.command_queue {
            self.interface.queue(command.clone());
        }

        self.interface.flush();
        self.command_queue.clear();
    }
}
