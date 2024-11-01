use renderer::{terminal::TerminalInterface, Renderer, RendererError, TerminalCommand};

use crate::buffer::Buffer;

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

    /// Renders all commands queued in the window loaded from a buffer
    pub fn display_buffer(&mut self, buffer: &Buffer) -> Result<(), RendererError> {
        self.renderer.enqueue_command(TerminalCommand::ClearScreen);
        self.renderer.enqueue_command(TerminalCommand::MoveCursor(0, 0));

        for (line_num, line) in buffer.get_lines().iter().enumerate() {
            self.renderer.enqueue_command(TerminalCommand::MoveCursor(0, line_num));
            self.renderer.enqueue_command(TerminalCommand::Print(line.clone()));
        }

        Ok(())
    }

    pub fn render(&mut self) -> Result<(), RendererError> {
        self.renderer.render()
    }

    pub fn enqueue_command(&mut self, command: TerminalCommand) {
        self.renderer.enqueue_command(command);
    }
}
