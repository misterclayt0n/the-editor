// TODO: Implement specific redrawing based on changes, not redrawing the entire buffer all the time.
use renderer::{
    terminal::{Terminal, TerminalInterface},
    Component, Renderer, RendererError, TerminalCommand,
};
use text_engine::{Rope, RopeSlice};
use utils::{build_welcome_message, Cursor, Position, Size};

use crate::{buffer::Buffer, EditorError};

/// Represents a window in the terminal.
pub struct Window {
    pub buffer: Buffer,
    pub cursor: Cursor,
    scroll_offset: Position,
    pub viewport_size: Size,
    pub needs_redraw: bool,
}

impl Window
{
    /// Loads a `Window` from a `Buffer` (can be `None`).
    pub fn from_file(
        file_path: Option<String>,
    ) -> Result<Self, EditorError> {
        let (width, height) = Terminal::size()
            .map_err(|e| EditorError::RenderError(format!("Could not initialize viewport: {e}")))?;

        let viewport_size = Size { width, height };

        let buffer = if let Some(path) = file_path {
            Buffer::open(path)?
        } else {
            Buffer::new()
        };

        Ok(Self {
            buffer,
            cursor: Cursor::new(),
            scroll_offset: Position::new(),
            viewport_size,
            needs_redraw: true, // Initial drawing
        })
    }

    //
    // Rendering
    //

    fn render_welcome_message<T: TerminalInterface>(&self, viewport: Size, current_row: usize, renderer: &mut Renderer<T>) {
        let Size { width, height } = viewport;
        let vertical_center = height / 3;

        if current_row == vertical_center {
            let welcome_string = build_welcome_message(width);
            renderer.enqueue_command(TerminalCommand::MoveCursor(0, current_row));
            renderer.enqueue_command(TerminalCommand::Print(welcome_string))
        } else {
            self.render_empty_row(current_row, renderer);
        }
    }

    /// Renders a single row in the `Window`.
    fn render_row<T: TerminalInterface> (&self, row: usize, slice: RopeSlice, renderer: &mut Renderer<T>) {
        renderer.enqueue_command(TerminalCommand::MoveCursor(0, row));

        // Since this runs in O(log N), it's better then to turn it
        // into a string or something.
        let rope = Rope::from(slice);
        renderer.enqueue_command(TerminalCommand::PrintRope(rope));
    }

    /// Renders a single line with a '~' character
    /// to represent empty lines beyond the buffer.
    fn render_empty_row<T: TerminalInterface>(&self, row: usize, renderer: &mut Renderer<T>) {
        renderer.enqueue_command(TerminalCommand::MoveCursor(0, row));
        renderer.enqueue_command(TerminalCommand::Print("~".to_string()));
    }

    //
    // Helpers
    //

    /// Calculates all the visible lines given a start and width of the
    /// viewport.
    fn calculate_visible_text<'a>(
        &self,
        line: RopeSlice<'a>,
        start_col: usize,
        width: usize,
    ) -> RopeSlice<'a> {
        let end_col = start_col + width;
        let line_len = line.len_chars();

        if start_col < line_len {
            let end_col = std::cmp::min(end_col, line_len);
            line.slice(start_col..end_col)
        } else {
            RopeSlice::from("")
        }
    }

    /// Adjust the cursor scrolling based on the `scroll_offset` and `viewport_size`.
    pub fn scroll_to_cursor(&mut self) {
        let width = self.viewport_size.width;
        let height = self.viewport_size.height.saturating_sub(1); // NOTE: Subtract 1 to account for the status bar.

        // Horizontal scrolling.
        if self.cursor.position.x < self.scroll_offset.x {
            self.scroll_offset.x = self.cursor.position.x;
        } else if self.cursor.position.x >= self.scroll_offset.x + width {
            self.scroll_offset.x = self.cursor.position.x.saturating_sub(width - 1);
        }

        // Vertical scrolling.
        if self.cursor.position.y < self.scroll_offset.y {
            self.scroll_offset.y = self.cursor.position.y;
        } else if self.cursor.position.y >= self.scroll_offset.y + height {
            self.scroll_offset.y = self.cursor.position.y.saturating_sub(height - 1);
        }
    }
}

impl Component for Window {
    fn render<T: TerminalInterface>(&mut self, renderer: &mut Renderer<T>) -> Result<(), RendererError> {
        if !self.needs_redraw {
            return Ok(());
        }

        let content_height = self.viewport_size.height.saturating_sub(1);
        for row in 0..content_height {
            renderer.enqueue_command(TerminalCommand::MoveCursor(0, row));
            renderer.enqueue_command(TerminalCommand::ClearLine);
        }

        // Helpers.
        let start_line = self.scroll_offset.y;
        let width = self.viewport_size.width;
        let nonempty_lines = self.buffer.len_nonempty_lines();

        for current_row in 0..content_height {
            let line_idx = start_line + current_row;

            if self.buffer.file_path.is_none() {
                self.render_welcome_message(self.viewport_size, current_row, renderer);
            } else {
                if line_idx < nonempty_lines {
                    let line = self.buffer.get_trimmed_line(line_idx);
                    let visible_text = self.calculate_visible_text(line, self.scroll_offset.x, width);

                    self.render_row(current_row, visible_text, renderer);
                } else {
                    self.render_empty_row(current_row, renderer);
                }
            }
        }

        let cursor_x = self.cursor.position.x.saturating_sub(self.scroll_offset.x);
        let cursor_y = self.cursor.position.y.saturating_sub(self.scroll_offset.y);

        // Check that cursor does not get over status bar.
        let cursor_y = if cursor_y >= content_height {
            content_height.saturating_sub(1)
        } else {
            cursor_y
        };
        renderer.enqueue_command(TerminalCommand::MoveCursor(cursor_x, cursor_y));

        self.needs_redraw = false;
        Ok(())
    }
}
