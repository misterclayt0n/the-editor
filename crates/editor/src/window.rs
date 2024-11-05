use renderer::{
    terminal::{Terminal, TerminalInterface},
    Component, Renderer, RendererError, TerminalCommand,
};
use utils::{Position, Size};

use crate::{buffer::Buffer, EditorError};

/// Represents a window in the terminal.
pub struct Window<T: TerminalInterface> {
    renderer: Renderer<T>,
    buffer: Option<Buffer>,
    cursor: Position,
    scroll_offset: Position,
    viewport_size: Size,
    pub needs_redraw: bool,
}

impl<T> Window<T>
where
    T: TerminalInterface,
{
    /// Loads a `Window` from a `Buffer` (can be `None`).
    pub fn from_file(renderer: Renderer<T>, buffer: Option<Buffer>) -> Result<Self, EditorError> {
        let (width, height) = Terminal::size()
            .map_err(|e| EditorError::RenderError(format!("Could not initialize viewport: {e}")))?;

        let viewport_size = Size { width, height };

        if let Some(buffer) = buffer {
            Ok(Self {
                renderer,
                buffer: Some(buffer),
                cursor: Position::zero(),
                scroll_offset: Position::zero(),
                viewport_size,
                needs_redraw: true,
            })
        } else {
            Ok(Self {
                renderer,
                buffer: None,
                cursor: Position::zero(),
                scroll_offset: Position::zero(),
                viewport_size,
                needs_redraw: true,
            })
        }
    }

    //
    // Rendering
    //

    /// Exhibits welcome screen, cleaning the window.
    pub fn display_welcome(&mut self) {
        self.enqueue_command(TerminalCommand::ClearScreen);
        self.enqueue_command(TerminalCommand::MoveCursor(0, 0));
        self.enqueue_command(TerminalCommand::HideCursor);
        self.enqueue_command(TerminalCommand::Print(
            "welcome to the-editor, press 'q' to quit".to_string(),
        ));
        self.enqueue_command(TerminalCommand::ShowCursor);
    }

    /// Renders a single row in the `Window`.
    fn render_row(&mut self, row: usize, text: &str) {
        self.enqueue_command(TerminalCommand::MoveCursor(0, row));
        self.enqueue_command(TerminalCommand::Print(text.to_string()));
    }

    /// Renders a single line with a '~' character
    /// to represent empty lines beyond the buffer.
    fn render_empty_row(&mut self, row: usize) {
        self.enqueue_command(TerminalCommand::MoveCursor(0, row));
        self.enqueue_command(TerminalCommand::Print("~".to_string()));
    }

    //
    // Helpers
    //

    /// Enqueue a command to the `Renderer` of the `Window`.
    pub fn enqueue_command(&mut self, command: TerminalCommand) {
        self.renderer.enqueue_command(command);
    }

    /// Calculate the visible text for the given line, considering scroll offset and viewport width.
    fn calculate_visible_text<'a>(&self, line: &'a str, start_col: usize, width: usize) -> &'a str {
        let end_col = std::cmp::min(start_col + width, line.len());
        if start_col < line.len() {
            &line[start_col..end_col]
        } else {
            ""
        }
    }

    ///
    /// MOCK: Gotta put this into a separate module.
    ///

    pub fn move_cursor_left(&mut self) {
        if self.cursor.x > 0 {
            self.cursor.x -= 1;
            self.scroll_to_cursor();
            self.needs_redraw = true;
        }
    }

    pub fn move_cursor_right(&mut self) {
        self.cursor.x += 1;
        self.scroll_to_cursor();
        self.needs_redraw = true;
    }

    pub fn move_cursor_up(&mut self) {
        if self.cursor.y > 0 {
            self.cursor.y -= 1;
            self.scroll_to_cursor();
            self.needs_redraw = true;
        }
    }

    pub fn move_cursor_down(&mut self) {
        self.cursor.y += 1;
        self.scroll_to_cursor();
        self.needs_redraw = true;
    }

    fn scroll_to_cursor(&mut self) {
        let Size { width, height } = self.viewport_size;

        // Horizontal scrolling.
        if self.cursor.x < self.scroll_offset.x {
            self.scroll_offset.x = self.cursor.x;
        } else if self.cursor.x >= self.scroll_offset.x + width {
            self.scroll_offset.x = self.cursor.x.saturating_sub(width - 1);
        }

        // Vertical scrolling.
        if self.cursor.y < self.scroll_offset.y {
            self.scroll_offset.y = self.cursor.y;
        } else if self.cursor.y >= self.scroll_offset.y + height {
            self.scroll_offset.y = self.cursor.y.saturating_sub(height - 1);
        }
    }
}

impl<T> Component for Window<T>
where
    T: TerminalInterface,
{
    /// Renders a window.
    fn render(&mut self) -> Result<(), RendererError> {
        if !self.needs_redraw {
            return Ok(());
        }

        self.enqueue_command(TerminalCommand::ClearScreen);

        if let Some(buffer) = &self.buffer {
            let lines = buffer.get_lines();

            let start_line = self.scroll_offset.y;
            let height = self.viewport_size.height;
            let width = self.viewport_size.width;

            for screen_row in 0..height {
                let line_idx = start_line + screen_row;
                if line_idx < lines.len() {
                    let line = &lines[line_idx];

                    // NOTE: I have no idea why adding 1 to `scroll_offset.x` works, but otherwise, it
                    // skips the render of the first character. My suspission is that this has something to do with
                    // `crossterm`, and I'm too lazy to fix this, so let it be +1.
                    let visible_text = self.calculate_visible_text(line, self.scroll_offset.x + 1, width);

                    self.render_row(screen_row, visible_text);
                } else {
                    self.render_empty_row(screen_row);
                }
            }
        }

        // Adjust cursor position.
        let cursor_x = self.cursor.x.saturating_sub(self.scroll_offset.x);
        let cursor_y = self.cursor.y.saturating_sub(self.scroll_offset.y);
        self.enqueue_command(TerminalCommand::MoveCursor(cursor_x, cursor_y));

        self.renderer.render()?;
        self.needs_redraw = false;

        Ok(())
    }
}
