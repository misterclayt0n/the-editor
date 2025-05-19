use terminal::Terminal;
use text_engine::Rope;
pub mod terminal;
pub mod gui;

/// `Component` is like a React component, but for the-editor.
pub trait Component {
    fn render_tui(&mut self, renderer: &mut Renderer);
    
    /// Renders the `Component`, does not require a `Renderer`, it assumes the `Component` has it's own.
    fn render(&mut self, renderer: &mut Renderer) {
        match renderer.interface {
            InterfaceType::TUI => self.render_tui(renderer),
            InterfaceType::GUI => todo!(),
        }
    }
}

/// Represents all commands that can be queued to be rendered.
#[derive(Debug, Clone)]
pub enum RenderTUICommand {
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

/// Renderer is responsible for rendering the state of the editor in the terminal.
pub struct Renderer {
    pub interface: InterfaceType,
    pub tui_command_queue: Vec<RenderTUICommand>,
    pub terminal: Terminal,
}

pub enum InterfaceType {
    TUI,
    GUI,
}

impl Renderer {
    pub fn new(interface: InterfaceType) -> Self {
        Renderer {
            interface,
            tui_command_queue: Vec::new(),
            terminal: Terminal::new(),
        }
    }

    /// Add a `Command` to the command queue.
    pub fn enqueue_command(&mut self, command: RenderTUICommand) {
        self.tui_command_queue.push(command)
    }

    /// Renders all enqueued commands.
    pub fn render(&mut self) {
        match self.interface {
            InterfaceType::TUI => {
                for command in &self.tui_command_queue {
                    self.terminal.queue(command.clone());
                }

                self.terminal.flush();
                self.tui_command_queue.clear();
            }
            InterfaceType::GUI => {
                todo!()
            }
        }
        
    }
}
