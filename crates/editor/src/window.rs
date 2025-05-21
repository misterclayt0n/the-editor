use renderer::{Color, Component, RenderGUICommand, RenderTUICommand, Renderer};
use text_engine::{Rope, RopeSlice};
use utils::{Cursor, InterfaceType, Position, PositionF, Size};

use crate::buffer::Buffer;

/// Represents a window in the terminal.
pub struct Window {
    pub buffer: Buffer,
    pub cursor: Cursor,
    pub scroll_offset: Position, // Target scroll based in characters.
    pub viewport_size: Size,
    pub gui_target_scroll_offset_px: PositionF, // Target scroll for GUI (in pixels).
    pub gui_current_scroll_offset_px: PositionF, // Current scroll for GUI (in pixels).
    pub gui_cursor_pos_px: PositionF,           // Cached cursor position in world space (pixels).
}

impl Window {
    const SCROLL_MARGIN: usize = 2;

    // Constants for GUI rendering
    const GUI_FONT_SIZE: i32 = 32;
    const GUI_LINE_HEIGHT: i32 = 36;
    const GUI_LEFT_PADDING: i32 = 5;
    const GUI_TOP_PADDING: i32 = 5;
    const GUI_SCROLL_SMOOTHNESS: f32 = 0.5;

    /// Loads a `Window` from a `Buffer` (can be `None`).
    /// `width` and `height` are of the viewport.
    pub fn from_file(file_path: Option<String>, width: usize, height: usize) -> Self {
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
            gui_target_scroll_offset_px: PositionF::default(),
            gui_current_scroll_offset_px: PositionF::default(),
            gui_cursor_pos_px: PositionF::default(),
        }
    }

    //
    // Rendering
    //

    /// Renders a single row in the `Window`.
    fn render_row(&self, row: usize, slice: RopeSlice, renderer: &mut Renderer) {
        renderer.enqueue_tui_command(RenderTUICommand::MoveCursor(0, row));
        let rope = Rope::from(slice);
        renderer.enqueue_tui_command(RenderTUICommand::PrintRope(rope));
    }

    /// Renders the cursor based on the interface type.
    fn render_cursor(&mut self, renderer: &mut Renderer) {
        match renderer.interface {
            InterfaceType::TUI => self.render_cursor_tui(renderer),
            InterfaceType::GUI => self.render_cursor_gui(renderer),
        }
    }

    /// Renders the cursor for TUI.
    fn render_cursor_tui(&self, renderer: &mut Renderer) {
        renderer.enqueue_tui_command(RenderTUICommand::HideCursor);

        let cursor_x = self.cursor.position.x.saturating_sub(self.scroll_offset.x);
        let cursor_y = self.cursor.position.y.saturating_sub(self.scroll_offset.y);

        // Only render if cursor is within the viewport.
        let content_height = self.viewport_size.height.saturating_sub(1);
        if cursor_y >= content_height {
            return;
        }

        let char_under_cursor = if self.cursor.position.y < self.buffer.len_nonempty_lines() {
            let line = self.buffer.get_trimmed_line(self.cursor.position.y);
            if self.cursor.position.x < line.len_chars() {
                line.char(self.cursor.position.x)
            } else {
                ' ' // Space if beyond end of line.
            }
        } else {
            ' ' // Space if beyond end of line.
        };

        renderer.enqueue_tui_command(RenderTUICommand::MoveCursor(cursor_x, cursor_y));
        renderer.enqueue_tui_command(RenderTUICommand::SetInverseVideo(true));
        renderer.enqueue_tui_command(RenderTUICommand::Print(char_under_cursor.to_string()));
        renderer.enqueue_tui_command(RenderTUICommand::SetInverseVideo(false));
    }

    /// Renders the cursor for GUI.
    fn render_cursor_gui(&self, renderer: &mut Renderer) {
        let font_size = Self::GUI_FONT_SIZE as f32;
        let line_height = Self::GUI_LINE_HEIGHT as f32;
        let char_width = self.get_gui_char_width(renderer);

        // Text area boundaries.
        let (text_area_x, text_area_y, text_area_width, text_area_height) =
            self.get_gui_text_area_bounds();

        // Scroll offsets.
        let scroll_offset_x_pixels = self.gui_current_scroll_offset_px.x;
        let scroll_offset_y_pixels = self.gui_current_scroll_offset_px.y;

        // Calculate cursor position.
        let cursor_line = self.cursor.position.y;
        let cursor_x_offset = self.gui_cursor_pos_px.x - Self::GUI_LEFT_PADDING as f32;
        let (cursor_x, cursor_y) = self.get_cursor_position_pixels(
            cursor_line,
            cursor_x_offset,
            scroll_offset_x_pixels,
            scroll_offset_y_pixels,
            text_area_x,
            text_area_y,
            line_height,
        );

        // Render cursor if within viewport bounds.
        if cursor_x >= text_area_x
            && cursor_x <= text_area_x + text_area_width
            && cursor_y >= text_area_y
            && cursor_y < text_area_y + text_area_height
        {
            renderer.enqueue_gui_command(RenderGUICommand::DrawCursor(
                cursor_x,
                cursor_y,
                char_width as i32,
                font_size as i32,
                Color::BLUE,
                128,
            ));

            let char_under_cursor = self.get_char_under_cursor();
            renderer.enqueue_gui_command(RenderGUICommand::DrawText(
                char_under_cursor,
                cursor_x,
                cursor_y,
                font_size as i32,
                Color::WHITE,
            ));
        }
    }

    //
    // Helpers
    //

    /// Calculates all the visible lines given a start and width of the viewport.
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

    /// Returns the GUI text area boundaries: (x, y, width, height).
    fn get_gui_text_area_bounds(&self) -> (i32, i32, i32, i32) {
        let text_area_x = Self::GUI_LEFT_PADDING;
        let text_area_y = Self::GUI_TOP_PADDING;
        let text_area_width = self.viewport_size.width as i32 - Self::GUI_LEFT_PADDING * 2;
        let status_bar_height = Self::GUI_LINE_HEIGHT;
        let text_area_height =
            self.viewport_size.height as i32 - status_bar_height - Self::GUI_TOP_PADDING;
        (text_area_x, text_area_y, text_area_width, text_area_height)
    }

    /// Returns the GUI usable dimensions: (width, height) in pixels.
    fn get_gui_usable_dimensions(&self) -> (f32, f32) {
        let usable_w = self.viewport_size.width as f32 - Self::GUI_LEFT_PADDING as f32 * 2.0;
        let usable_h = self.viewport_size.height as f32
            - Self::GUI_TOP_PADDING as f32
            - Self::GUI_LINE_HEIGHT as f32;
        (usable_w, usable_h)
    }

    /// Returns the GUI character width in pixels.
    fn get_gui_char_width(&self, renderer: &Renderer) -> f32 {
        renderer
            .gui
            .as_ref()
            .map(|gui| gui.gui_measure_font_width("M", Self::GUI_FONT_SIZE as f32))
            .unwrap_or(10.0)
    }

    /// Adjust the cursor scrolling.
    pub fn scroll_to_cursor(&mut self, renderer: &Renderer) {
        match renderer.interface {
            InterfaceType::TUI => {
                let width = self.viewport_size.width;
                let height = self.viewport_size.height.saturating_sub(1);

                // Horizontal scrolling.
                if self.cursor.position.x < self.scroll_offset.x + Self::SCROLL_MARGIN {
                    self.scroll_offset.x =
                        self.cursor.position.x.saturating_sub(Self::SCROLL_MARGIN);
                } else if self.cursor.position.x
                    >= self.scroll_offset.x + width - Self::SCROLL_MARGIN
                {
                    self.scroll_offset.x = self
                        .cursor
                        .position
                        .x
                        .saturating_sub(width - 1 - Self::SCROLL_MARGIN);
                }

                // Vertical scrolling.
                if self.cursor.position.y < self.scroll_offset.y + Self::SCROLL_MARGIN {
                    self.scroll_offset.y =
                        self.cursor.position.y.saturating_sub(Self::SCROLL_MARGIN);
                } else if self.cursor.position.y
                    >= self.scroll_offset.y + height - Self::SCROLL_MARGIN
                {
                    self.scroll_offset.y = self
                        .cursor
                        .position
                        .y
                        .saturating_sub(height - 1 - Self::SCROLL_MARGIN);
                }
            }
            InterfaceType::GUI => {
                let (usable_w, usable_h) = self.get_gui_usable_dimensions();
                let line_height = Self::GUI_LINE_HEIGHT as f32;
                let char_width = self.get_gui_char_width(renderer);

                // Get cursor position in world space (pixels).
                let cursor_line = self.cursor.position.y;
                let cursor_col = self.cursor.position.x;
                let text_before_cursor = self
                    .buffer
                    .line(cursor_line)
                    .chars()
                    .take(cursor_col)
                    .collect::<String>();

                let cur_x_world = Self::GUI_LEFT_PADDING as f32
                    + renderer
                        .gui
                        .as_ref()
                        .map(|gui| {
                            gui.gui_measure_font_width(
                                &text_before_cursor,
                                Self::GUI_FONT_SIZE as f32,
                            )
                        })
                        .unwrap_or(0.0);
                let cur_y_world = Self::GUI_TOP_PADDING as f32 + cursor_line as f32 * line_height;

                // Cache cursor position.
                self.gui_cursor_pos_px.x = cur_x_world;
                self.gui_cursor_pos_px.y = cur_y_world;

                // Define margins in pixels.
                let margin_x = (Self::SCROLL_MARGIN as f32) * char_width;
                let margin_y = (Self::SCROLL_MARGIN as f32) * line_height;

                // Calculate current screen position.
                let cur_x_scr = cur_x_world - self.gui_current_scroll_offset_px.x;
                let cur_y_scr = cur_y_world - self.gui_current_scroll_offset_px.y;

                // Horizontal target.
                if cur_x_scr < margin_x {
                    self.gui_target_scroll_offset_px.x = (cur_x_world - margin_x).max(0.0);
                } else if cur_x_scr > usable_w - margin_x {
                    let line_text = self.buffer.line(cursor_line).to_string();
                    let doc_width = Self::GUI_LEFT_PADDING as f32
                        + renderer
                            .gui
                            .as_ref()
                            .map(|gui| {
                                gui.gui_measure_font_width(&line_text, Self::GUI_FONT_SIZE as f32)
                            })
                            .unwrap_or(0.0);
                    self.gui_target_scroll_offset_px.x = (cur_x_world - (usable_w - margin_x))
                        .max(0.0)
                        .min((doc_width - usable_w).max(0.0));
                }

                // Vertical target.
                if cur_y_scr < margin_y {
                    self.gui_target_scroll_offset_px.y = (cur_y_world - margin_y).max(0.0);
                } else if cur_y_scr > usable_h - margin_y {
                    let doc_height =
                        Self::GUI_TOP_PADDING as f32 + self.buffer.len_lines() as f32 * line_height;
                    self.gui_target_scroll_offset_px.y = (cur_y_world - (usable_h - margin_y))
                        .max(0.0)
                        .min((doc_height - usable_h).max(0.0));
                }
            }
        }
    }

    //
    // Helpers
    // 

    /// Calculates the index of the first visible line based on scroll offset.
    fn calculate_first_visible_line(&self, scroll_offset_y_pixels: f32, line_height: f32) -> usize {
        (scroll_offset_y_pixels / line_height).floor() as usize
    }

    /// Checks if a line is within the visible text area.
    fn is_line_visible(
        &self,
        text_y_pos: i32,
        text_area_y: i32,
        text_area_height: i32,
        line_height: i32,
    ) -> bool {
        text_y_pos >= text_area_y - line_height && text_y_pos <= text_area_y + text_area_height
    }

    /// Computes the cursor's pixel position in the GUI.
    fn get_cursor_position_pixels(
        &self,
        cursor_line: usize,
        cursor_x_offset: f32,
        scroll_offset_x_pixels: f32,
        scroll_offset_y_pixels: f32,
        text_area_x: i32,
        text_area_y: i32,
        line_height: f32,
    ) -> (i32, i32) {
        let cursor_pixel_x = text_area_x + (cursor_x_offset - scroll_offset_x_pixels) as i32;
        let cursor_pixel_y =
            text_area_y + (cursor_line as f32 * line_height - scroll_offset_y_pixels) as i32;
        (cursor_pixel_x, cursor_pixel_y)
    }

    /// Retrieves the character under the cursor, returning a space if none exists.
    fn get_char_under_cursor(&self) -> String {
        if self.cursor.position.y < self.buffer.len_lines() {
            let line = self.buffer.line(self.cursor.position.y);
            if self.cursor.position.x < line.len_chars() {
                line.char(self.cursor.position.x).to_string()
            } else {
                " ".to_string()
            }
        } else {
            " ".to_string()
        }
    }
}

impl Component for Window {
    fn render_tui(&mut self, renderer: &mut Renderer) {
        let content_height = self.viewport_size.height.saturating_sub(1);
        for row in 0..content_height {
            renderer.enqueue_tui_command(RenderTUICommand::MoveCursor(0, row));
            renderer.enqueue_tui_command(RenderTUICommand::ClearLine);
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

        self.render_cursor(renderer);
    }

    fn render_gui(&mut self, renderer: &mut Renderer) {
        let font_size = Self::GUI_FONT_SIZE as f32;
        let line_height = Self::GUI_LINE_HEIGHT as f32;

        // Lerp current scroll offset towards target.
        self.gui_current_scroll_offset_px.x += (self.gui_target_scroll_offset_px.x
            - self.gui_current_scroll_offset_px.x)
            * Self::GUI_SCROLL_SMOOTHNESS;
        self.gui_current_scroll_offset_px.y += (self.gui_target_scroll_offset_px.y
            - self.gui_current_scroll_offset_px.y)
            * Self::GUI_SCROLL_SMOOTHNESS;

        // Use lerped scroll offset.
        let scroll_offset_x_pixels = self.gui_current_scroll_offset_px.x;
        let scroll_offset_y_pixels = self.gui_current_scroll_offset_px.y;

        // Text area boundaries.
        let (text_area_x, text_area_y, _, text_area_height) = self.get_gui_text_area_bounds();

        // Calculate visible lines.
        let first_visible_line =
            self.calculate_first_visible_line(scroll_offset_y_pixels, line_height);
        let num_visible_lines = (text_area_height as f32 / line_height).ceil() as usize + 1;

        // Render text lines.
        for i in 0..num_visible_lines {
            let line_idx = first_visible_line + i;
            if line_idx >= self.buffer.len_lines() {
                break;
            }

            let line = self.buffer.line(line_idx).to_string();
            let text_y_pos =
                text_area_y + (line_idx as f32 * line_height - scroll_offset_y_pixels) as i32;

            if !self.is_line_visible(
                text_y_pos,
                text_area_y,
                text_area_height,
                line_height as i32,
            ) {
                continue;
            }

            let text_x_pos = text_area_x - scroll_offset_x_pixels as i32;
            renderer.enqueue_gui_command(RenderGUICommand::DrawText(
                line,
                text_x_pos,
                text_y_pos,
                font_size as i32,
                Color::BLACK,
            ));
        }

        self.render_cursor(renderer);
    }
}
