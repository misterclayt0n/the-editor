use renderer::{terminal::TerminalInterface, Component, Renderer, RendererError, TerminalCommand};

use crate::{buffer::Buffer, EditorError};

/// Represents a window in the terminal
pub struct Window<T: TerminalInterface> {
    renderer: Renderer<T>,
    buffer: Option<Buffer>
}

impl<T> Window<T>
where
    T: TerminalInterface,
{
    pub fn from_file(renderer: Renderer<T>, buffer: Option<Buffer>) -> Result<Self, EditorError> {
        if let Some(buffer) = buffer {
            Ok(Self {
                renderer,
                buffer: Some(buffer),
            })
        } else {
            Ok(Self {
                renderer,
                buffer: None
            })
        }
    }

    /// Exhibits welcome screen, cleaning the window
    pub fn display(&mut self) {
        self.renderer.welcome_screen()
    }

    /// Renders all commands returned by a buffer, this should be
    pub fn display_buffer(&mut self) -> Result<(), RendererError> {
        let commands = self.buffer.as_ref().unwrap().render();
        for command in commands {
            self.renderer.enqueue_command(command)
        }

        Ok(())
    }

    pub fn render(&mut self) -> Result<(), RendererError> {
        self.renderer.render()
    }

    pub fn enqueue_command(&mut self, command: TerminalCommand) {
        self.renderer.enqueue_command(command);
    }

    pub fn is_buffer_loaded(&self) -> bool {
        self.buffer.is_some()
    }
}
