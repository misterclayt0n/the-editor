use std::{cmp::min, io::Error};

use crate::prelude::*;

use super::super::{
    command::{Edit, Move},
    DocumentStatus, Line, Terminal,
};

use super::UIComponent;
mod buffer;
use buffer::Buffer;
mod searchdirection;
use searchdirection::SearchDirection;
mod fileinfo;
use fileinfo::FileInfo;
mod searchinfo;
use searchinfo::SearchInfo;

#[derive(Default)]
pub struct View {
    buffer: Buffer,
    needs_redraw: bool,
    size: Size,
    text_location: Location,
    scroll_offset: Position,
    search_info: Option<SearchInfo>,
}

impl View {
    pub fn get_status(&self) -> DocumentStatus {
        DocumentStatus {
            total_lines: self.buffer.height(),
            current_line_index: self.text_location.line_index,
            file_name: format!("{}", self.buffer.file_info),
            is_modified: self.buffer.dirty,
        }
    }

    pub const fn is_file_loaded(&self) -> bool {
        self.buffer.is_file_loaded()
    }

    //
    // Search
    //

    pub fn enter_search(&mut self) {
        self.search_info = Some(SearchInfo {
            prev_location: self.text_location,
            prev_scroll_offset: self.scroll_offset,
            query: None,
        });
    }

    pub fn exit_search(&mut self) {
        self.search_info = None;
        self.set_needs_redraw(true);
    }

    pub fn dismiss_search(&mut self) {
        if let Some(search_info) = &self.search_info {
            self.text_location = search_info.prev_location;
            self.scroll_offset = search_info.prev_scroll_offset;
            self.scroll_text_location_into_view(); // Ensure the previous location is still visible even if the terminal has been resized during search.
        }

        self.exit_search();
    }

    pub fn search(&mut self, query: &str) {
        if let Some(search_info) = &mut self.search_info {
            search_info.query = Some(Line::from(query));
        }

        self.search_in_direction(self.text_location, SearchDirection::default());
    }

    /// Attempts to get the current search query - for scenarios where the search query absolutely must be there.
    /// Panics if not present in debug, or if search info is not present in debug.
    /// Returns None on release.
    fn get_search_query(&self) -> Option<&Line> {
        let query = self
            .search_info
            .as_ref()
            .and_then(|search_info| search_info.query.as_ref());

        debug_assert!(
            query.is_some(),
            "Attempting to search with malformed search_info present"
        );
        query
    }

    fn search_in_direction(&mut self, from: Location, direction: SearchDirection) {
        if let Some(location) = self.get_search_query().and_then(|query| {
            if query.is_empty() {
                None
            } else if direction == SearchDirection::Forward {
                self.buffer.search_forward(query, from)
            } else {
                self.buffer.search_backward(query, from)
            }
        }) {
            self.text_location = location;
            self.center_text_location();
            self.set_needs_redraw(true);
        }
    }

    pub fn search_next(&mut self) {
        let step_right = self
            .get_search_query()
            .map_or(1, |query| min(query.grapheme_count(), 1));

        let location = Location {
            line_index: self.text_location.line_index,
            grapheme_index: self.text_location.grapheme_index.saturating_add(step_right), // Start the new search after the current match
        };

        self.search_in_direction(location, SearchDirection::Forward);
    }

    pub fn search_prev(&mut self) {
        self.search_in_direction(self.text_location, SearchDirection::Backward);
    }

    //
    // File I/O
    //

    pub fn load(&mut self, file_name: &str) -> Result<(), Error> {
        let buffer = Buffer::load(file_name)?;
        self.buffer = buffer;
        self.set_needs_redraw(true);
        Ok(())
    }

    pub fn save(&mut self) -> Result<(), Error> {
        self.buffer.save()
    }

    pub fn save_as(&mut self, file_name: &str) -> Result<(), Error> {
        self.buffer.save_as(file_name)
    }

    //
    // Command Handling
    //

    pub fn handle_edit_command(&mut self, command: Edit) {
        match command {
            Edit::Insert(character) => self.insert_char(character),
            Edit::Delete => self.delete(),
            Edit::DeleteBackward => self.delete_backward(),
            Edit::InsertNewline => self.insert_newline(),
        }
    }

    pub fn handle_move_command(&mut self, command: Move) {
        let Size { height, .. } = self.size;

        // This match moves the position but does not check for all boundaries.
        // The final boundary checking happens after the match statement.
        match command {
            Move::Up => self.move_up(1),
            Move::Down => self.move_down(1),
            Move::Left => self.move_left(),
            Move::Right => self.move_right(),
            Move::PageUp => self.move_up(height.saturating_sub(1)),
            Move::PageDown => self.move_down(height.saturating_sub(1)),
            Move::StartOfLine => self.move_to_start_of_line(),
            Move::EndOfLine => self.move_to_end_of_line(),
        }

        self.scroll_text_location_into_view();
    }

    //
    // Text Editing
    //

    fn insert_newline(&mut self) {
        self.buffer.insert_newline(self.text_location);
        // move cursor to the beginning of the next line
        self.text_location.line_index += 1;
        self.text_location.grapheme_index = 0;
        self.set_needs_redraw(true);
    }

    fn delete_backward(&mut self) {
        if self.text_location.line_index != 0 || self.text_location.grapheme_index != 0 {
            self.handle_move_command(Move::Left);
            self.delete();
        }
    }

    fn delete(&mut self) {
        self.buffer.delete(self.text_location);
        self.set_needs_redraw(true);
    }

    fn insert_char(&mut self, character: char) {
        let line_index = self.text_location.line_index;

        // Get the old length of the line
        let old_len = if line_index < self.buffer.rope.len_lines() {
            let line_slice = self.buffer.rope.line(line_index);
            let line_str = line_slice.to_string();
            let line = Line::from(&line_str);
            line.grapheme_count()
        } else {
            0
        };

        self.buffer.insert_char(character, self.text_location);

        // Get the new length of the line after insertion
        let new_len = if line_index < self.buffer.rope.len_lines() {
            let line_slice = self.buffer.rope.line(line_index);
            let line_str = line_slice.to_string();
            let line = Line::from(&line_str);
            line.grapheme_count()
        } else {
            0
        };

        let grapheme_delta = new_len.saturating_sub(old_len);

        if grapheme_delta > 0 {
            // Move right for an added grapheme (should be the regular case)
            self.handle_move_command(Move::Right);
        }

        self.set_needs_redraw(true);
    }

    //
    // Rendering
    //

    fn render_line(at: RowIndex, line_text: &str) -> Result<(), Error> {
        Terminal::print_row(at, line_text)
    }

    fn build_welcome_message(width: usize) -> String {
        if width == 0 {
            return String::new();
        }

        let welcome_message = format!("{NAME} -- version {VERSION}");
        let len = welcome_message.len();
        let remaining_width = width.saturating_sub(1);

        // Hide the welcome message if it doesn't fit entirely.
        if remaining_width < len {
            return "~".to_string();
        }

        format!("{:<1}{:^remaining_width$}", "~", welcome_message)
    }

    //
    // Scrolling
    //

    fn scroll_vertically(&mut self, to: RowIndex) {
        let Size { height, .. } = self.size;

        let offset_changed = if to < self.scroll_offset.row {
            self.scroll_offset.row = to;
            true
        } else if to >= self.scroll_offset.row.saturating_add(height) {
            self.scroll_offset.row = to.saturating_sub(height).saturating_add(1);
            true
        } else {
            false
        };

        if offset_changed {
            self.set_needs_redraw(true);
        }
    }

    fn scroll_horizontally(&mut self, to: ColIndex) {
        let Size { width, .. } = self.size;

        let offset_changed = if to < self.scroll_offset.col {
            self.scroll_offset.col = to;
            true
        } else if to >= self.scroll_offset.col.saturating_add(width) {
            self.scroll_offset.col = to.saturating_sub(width).saturating_add(1);
            true
        } else {
            false
        };

        if offset_changed {
            self.set_needs_redraw(true);
        }
    }

    fn scroll_text_location_into_view(&mut self) {
        let Position { row, col } = self.text_location_to_position();
        self.scroll_vertically(row);
        self.scroll_horizontally(col);
    }

    fn center_text_location(&mut self) {
        let Size { height, width } = self.size;
        let Position { row, col } = self.text_location_to_position();
        let vertical_mid = height.div_ceil(2);
        let horizontal_mid = width.div_ceil(2);
        self.scroll_offset.row = row.saturating_sub(vertical_mid);
        self.scroll_offset.col = col.saturating_sub(horizontal_mid);
        self.set_needs_redraw(true);
    }

    //
    // Location and Position Handling
    //

    pub fn cursor_position(&self) -> Position {
        self.text_location_to_position()
            .saturating_sub(self.scroll_offset)
    }

    fn text_location_to_position(&self) -> Position {
        let row = self.text_location.line_index;
        debug_assert!(row.saturating_sub(1) <= self.buffer.rope.len_lines());
        let col = if row < self.buffer.rope.len_lines() {
            let line_slice = self.buffer.rope.line(row);
            let line_str = line_slice.to_string();
            let line = Line::from(&line_str);
            line.width_until(self.text_location.grapheme_index)
        } else {
            0
        };
        Position { col, row }
    }

    //
    // Text Location Movement
    //

    fn move_up(&mut self, step: usize) {
        self.text_location.line_index = self.text_location.line_index.saturating_sub(step);
        self.snap_to_valid_grapheme();
    }

    fn move_down(&mut self, step: usize) {
        self.text_location.line_index = self.text_location.line_index.saturating_add(step);
        self.snap_to_valid_line();
        self.snap_to_valid_grapheme();
    }

    // clippy::arithmetic_side_effects: This function performs arithmetic calculations
    // after explicitly checking that the target value will be within bounds.
    #[allow(clippy::arithmetic_side_effects)]
    fn move_right(&mut self) {
        let line_width = if self.text_location.line_index < self.buffer.rope.len_lines() {
            let line_slice = self.buffer.rope.line(self.text_location.line_index);
            let line_str = line_slice.to_string();
            let line = Line::from(&line_str);
            line.grapheme_count()
        } else {
            0
        };
        if self.text_location.grapheme_index < line_width {
            self.text_location.grapheme_index += 1;
        } else {
            self.move_to_start_of_line();
            self.move_down(1);
        }
    }

    // clippy::arithmetic_side_effects: This function performs arithmetic calculations
    // after explicitly checking that the target value will be within bounds.
    #[allow(clippy::arithmetic_side_effects)]
    fn move_left(&mut self) {
        if self.text_location.grapheme_index > 0 {
            self.text_location.grapheme_index -= 1;
        } else if self.text_location.line_index > 0 {
            self.move_up(1);
            self.move_to_end_of_line();
        }
    }

    fn move_to_start_of_line(&mut self) {
        self.text_location.grapheme_index = 0;
    }

    fn move_to_end_of_line(&mut self) {
        self.text_location.grapheme_index =
            if self.text_location.line_index < self.buffer.rope.len_lines() {
                let line_slice = self.buffer.rope.line(self.text_location.line_index);
                let line_str = line_slice.to_string().trim_end_matches('\n').to_string();
                let line = Line::from(&line_str);
                line.grapheme_count()
            } else {
                0
            };
    }

    // Ensures self.text_location.grapheme_index points to a valid grapheme index by snapping it to the leftmost grapheme if appropriate.
    // Doesn't trigger scrolling.
    fn snap_to_valid_grapheme(&mut self) {
        self.text_location.grapheme_index =
            if self.text_location.line_index < self.buffer.rope.len_lines() {
                let line_slice = self.buffer.rope.line(self.text_location.line_index);
                let line_str = line_slice.to_string();
                let line = Line::from(&line_str);
                min(line.grapheme_count(), self.text_location.grapheme_index)
            } else {
                0
            };
    }

    // Ensures self.text_location.line_index points to a valid line index by snapping it to the bottommost line if appropriate.
    // Doesn't trigger scrolling.
    fn snap_to_valid_line(&mut self) {
        self.text_location.line_index = min(
            self.text_location.line_index,
            self.buffer.height().saturating_sub(1),
        );
    }

    // endregion
}

impl UIComponent for View {
    fn set_needs_redraw(&mut self, value: bool) {
        self.needs_redraw = value;
    }

    fn needs_redraw(&self) -> bool {
        self.needs_redraw
    }

    fn set_size(&mut self, size: Size) {
        self.size = size;
        self.scroll_text_location_into_view();
    }

    fn draw(&mut self, origin_row: RowIndex) -> Result<(), Error> {
        let Size { height, width } = self.size;
        let end_y = origin_row.saturating_add(height);
        let scroll_top = self.scroll_offset.row;

        let top_third = height.div_ceil(3);

        for current_row in origin_row..end_y {
            let line_idx = current_row
                .saturating_add(origin_row)
                .saturating_add(scroll_top);

            if line_idx < self.buffer.rope.len_lines() {
                let line_slice = self.buffer.rope.line(line_idx);

                let left = self.scroll_offset.col;
                let right = self.scroll_offset.col.saturating_add(width);
                let visible_line = line_slice.slice(left..min(right, line_slice.len_chars()));

                let query = self
                    .search_info
                    .as_ref()
                    .and_then(|search_info| search_info.query.as_ref());

                let selected_match = (self.text_location.line_index == line_idx && query.is_some())
                    .then_some(self.text_location.grapheme_index);

                if let Some(query) = query {
                    let line_str = visible_line.to_string();
                    let line = Line::from(&line_str);

                    Terminal::print_annotated_row(current_row, &line.get_annotated_visible_substr(0..line_str.len(), Some(query), selected_match))?;
                } else {
                    Terminal::print_rope_slice_row(current_row, visible_line)?;
                }
            } else if current_row == top_third && self.buffer.is_empty() {
                Self::render_line(current_row, &Self::build_welcome_message(width))?;
            } else {
                Self::render_line(current_row, "~")?;
            }
        }

        Ok(())
    }
}
