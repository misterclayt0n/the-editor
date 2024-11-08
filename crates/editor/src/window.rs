use std::cell::RefCell;

// TODO: Implement specific redrawing based on changes, not redrawing the entire buffer all the time.
use renderer::{
    terminal::{Terminal, TerminalInterface},
    Component, Renderer, RendererError, TerminalCommand,
};
use text_engine::{Rope, RopeSlice};
use utils::{build_welcome_message, Cursor, Position, Size};

use crate::{buffer::Buffer, EditorError};

/// Represents a window in the terminal.
pub struct Window<T: TerminalInterface> {
    renderer: RefCell<Renderer<T>>, // Easiest way I've found to shared mutability.
    pub buffer: Buffer,
    pub cursor: Cursor,
    scroll_offset: Position,
    pub viewport_size: Size,
    pub needs_redraw: bool,
}

impl<T> Window<T>
where
    T: TerminalInterface,
{
    /// Loads a `Window` from a `Buffer` (can be `None`).
    pub fn from_file(
        renderer: Renderer<T>,
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
            renderer: RefCell::from(renderer),
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

    fn render_welcome_message(&self, viewport: Size, current_row: usize) {
        let Size { width, height } = viewport;
        let vertical_center = height / 3;

        if current_row == vertical_center {
            let welcome_string = build_welcome_message(width);
            self.enqueue_command(TerminalCommand::MoveCursor(0, current_row));
            self.enqueue_command(TerminalCommand::Print(welcome_string))
        } else {
            self.render_empty_row(current_row);
        }
    }

    /// Renders a single row in the `Window`.
    fn render_row(&self, row: usize, slice: RopeSlice) {
        self.enqueue_command(TerminalCommand::MoveCursor(0, row));

        // Since this runs in O(log N), it's better then to turn it
        // into a string or something.
        let rope = Rope::from(slice);
        self.enqueue_command(TerminalCommand::PrintRope(rope));
    }

    /// Renders a single line with a '~' character
    /// to represent empty lines beyond the buffer.
    fn render_empty_row(&self, row: usize) {
        self.enqueue_command(TerminalCommand::MoveCursor(0, row));
        self.enqueue_command(TerminalCommand::Print("~".to_string()));
    }

    //
    // Helpers
    //

    /// Enqueue a command to the `Renderer` of the `Window`.
    pub fn enqueue_command(&self, command: TerminalCommand) {
        self.renderer.borrow_mut().enqueue_command(command);
    }

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
        let Size { width, height } = self.viewport_size;

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

impl<T> Component for Window<T>
where
    T: TerminalInterface,
{
    fn render(&mut self) -> Result<(), RendererError> {
        if !self.needs_redraw {
            return Ok(());
        }

        self.enqueue_command(TerminalCommand::ClearScreen);

        // Helpers.
        let start_line = self.scroll_offset.y;
        let Size { width, height } = self.viewport_size;
        let nonempty_lines = self.buffer.len_nonempty_lines();

        for current_row in 0..height {
            let line_idx = start_line + current_row;

            if self.buffer.file_path.is_none() {
                self.render_welcome_message(self.viewport_size, current_row);
            } else {
                if line_idx < nonempty_lines {
                    let line = self.buffer.get_trimmed_line(line_idx);
                    let visible_text = self.calculate_visible_text(line, self.scroll_offset.x, width);

                    self.render_row(current_row, visible_text);
                } else {
                    self.render_empty_row(current_row);
                }
            }
        }

        let cursor_x = self.cursor.position.x.saturating_sub(self.scroll_offset.x);
        let cursor_y = self.cursor.position.y.saturating_sub(self.scroll_offset.y);
        self.enqueue_command(TerminalCommand::MoveCursor(cursor_x, cursor_y));

        self.renderer.borrow_mut().render()?;
        self.needs_redraw = false;

        Ok(())
    }
}
