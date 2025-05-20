use gui::Gui;
use terminal::Terminal;
use text_engine::Rope;
pub mod gui;
pub mod terminal;

/// `Component` is like a React component, but for the-editor.
pub trait Component {
    /// Renders the TUI version of the component.
    fn render_tui(&mut self, renderer: &mut Renderer);

    fn render_gui(&mut self, renderer: &mut Renderer);

    /// Renders the `Component` by matching which interface type we have.
    fn render(&mut self, renderer: &mut Renderer) {
        match renderer.interface {
            InterfaceType::TUI => self.render_tui(renderer),
            InterfaceType::GUI => self.render_gui(renderer),
        }
    }
}

/// Represents all commands that can be queued to be rendered.
#[derive(Clone, Debug)]
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

pub enum RenderGUICommand {
    ClearBackground(Color),
}

#[derive(Clone, Debug)]
pub enum Color {
    BLACK,
    WHITE,
}

/// Renderer is responsible for rendering the state of the editor in the terminal.
pub struct Renderer {
    pub interface: InterfaceType,
    pub tui_command_queue: Vec<RenderTUICommand>,
    pub gui_command_queue: Vec<RenderGUICommand>,
    pub terminal: Option<Terminal>,
    pub gui: Option<Gui>,
}

pub enum InterfaceType {
    TUI,
    GUI,
}

impl Renderer {
    pub fn new(interface: InterfaceType) -> Self {
        match interface {
            InterfaceType::TUI => Renderer {
                interface,
                tui_command_queue: Vec::new(),
                gui_command_queue: Vec::new(),
                terminal: Some(Terminal::new()),
                gui: None,
            },
            InterfaceType::GUI => Renderer {
                interface,
                tui_command_queue: Vec::new(),
                gui_command_queue: Vec::new(),
                terminal: None,
                gui: Some(Gui::new(800, 600)),
            },
        }
    }

    /// Add a `TUICommand` to the TUI command queue.
    pub fn enqueue_tui_command(&mut self, command: RenderTUICommand) {
        self.tui_command_queue.push(command)
    }

    /// Add a `GUICommand` to the GUI command queue.
    pub fn enqueue_gui_command(&mut self, command: RenderGUICommand) {
        self.gui_command_queue.push(command)
    }

    /// Renders all enqueued commands.
    pub fn render(&mut self) {
        match self.interface {
            InterfaceType::TUI => {
                // We can safely unwrap here, because we know there is a Terminal.
                let terminal = self.terminal.as_mut().unwrap();
                for command in &self.tui_command_queue {
                    terminal.queue(command.clone());
                }

                terminal.flush();
                self.tui_command_queue.clear();
            }
            InterfaceType::GUI => {
                // Same thing here.
                let gui = self.gui.as_mut().unwrap();

                gui.process_commands(&self.gui_command_queue);
                self.gui_command_queue.clear();
            }
        }
    }
}
