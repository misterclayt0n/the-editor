use renderer::{terminal::TerminalInterface, Renderer, RendererError, TerminalCommand};

/// Represents a window in the terminal
pub struct Window<T: TerminalInterface> {
    renderer: Renderer<T>,
}

impl<T> Window<T>
where
    T: TerminalInterface,
{
    pub fn new(renderer: Renderer<T>) -> Self {
        Self { renderer }
    }

    /// Exhibits welcome screen, cleaning the window
    pub fn display(&mut self) {
        self.renderer.welcome_screen()
    }

    /// Renders all commands queued in the window
    pub fn render(&mut self) -> Result<(), RendererError> {
        self.renderer.render()
    }

    pub fn enqueue_command(&mut self, command: TerminalCommand) {
        self.renderer.enqueue_command(command);
    }
}
