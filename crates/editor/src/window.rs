use renderer::{
    terminal::{Terminal, TerminalInterface},
    Component, Renderer, TerminalCommand,
};
use text_engine::{Rope, RopeSlice};
use utils::{Cursor, Position, Size};

use crate::buffer::Buffer;

/// Represents a window in the terminal.
pub struct Window {
    pub buffer: Buffer,
    pub cursor: Cursor,
    scroll_offset: Position,
    pub viewport_size: Size,
}

impl Window {
    const SCROLL_MARGIN: usize = 2;

    /// Loads a `Window` from a `Buffer` (can be `None`).
    pub fn from_file(file_path: Option<String>) -> Self {
        let (width, height) = Terminal::size();

        let viewport_size = Size { width, height };

        let buffer = if let Some(path) = file_path {
            Buffer::open(path)
        } else {
            Buffer::new()
        };

        Self {
            buffer,
            cursor: Cursor::new(),
            scroll_offset: Position::new(),
            viewport_size,
        }
    }

    //
    // Rendering
    //

    /// Renders a single row in the `Window`.
    fn render_row<T: TerminalInterface>(
        &self,
        row: usize,
        slice: RopeSlice,
        renderer: &mut Renderer<T>,
    ) {
        renderer.enqueue_command(TerminalCommand::MoveCursor(0, row));

        // Since this runs in O(log N), it's better then to turn it
        // into a string or something.
        let rope = Rope::from(slice);
        renderer.enqueue_command(TerminalCommand::PrintRope(rope));
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
        if self.cursor.position.x < self.scroll_offset.x + Self::SCROLL_MARGIN {
            self.scroll_offset.x = self.cursor.position.x.saturating_sub(Self::SCROLL_MARGIN);
        } else if self.cursor.position.x >= self.scroll_offset.x + width - Self::SCROLL_MARGIN {
            self.scroll_offset.x = self.cursor.position.x.saturating_sub(width - 1 - Self::SCROLL_MARGIN);
        }

        // Vertical scrolling.
        if self.cursor.position.y < self.scroll_offset.y + Self::SCROLL_MARGIN {
            self.scroll_offset.y = self.cursor.position.y.saturating_sub(Self::SCROLL_MARGIN);
        } else if self.cursor.position.y >= self.scroll_offset.y + height - Self::SCROLL_MARGIN {
            self.scroll_offset.y = self
                .cursor
                .position
                .y
                .saturating_sub(height - 1 - Self::SCROLL_MARGIN);
        }
    }
}

impl Component for Window {
    fn render<T: TerminalInterface>(&mut self, renderer: &mut Renderer<T>) {
        let content_height = self.viewport_size.height.saturating_sub(1);
        for row in 0..content_height {
            renderer.enqueue_command(TerminalCommand::MoveCursor(0, row));
            renderer.enqueue_command(TerminalCommand::ClearLine);
        }

        // Helpers.
        let start_line = self.scroll_offset.y;
        let width = self.viewport_size.width;
        let total_lines = std::cmp::max(self.buffer.len_nonempty_lines(), 1);

        for current_row in 0..content_height {
            let line_idx = start_line + current_row;

            if line_idx < total_lines {
                let line = self.buffer.get_trimmed_line(line_idx);
                let visible_text = self.calculate_visible_text(line, self.scroll_offset.x, width);
                self.render_row(current_row, visible_text, renderer);
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
    }
}
