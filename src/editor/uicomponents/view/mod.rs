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
use movement::Movement;
use searchdirection::SearchDirection;
mod fileinfo;
use fileinfo::FileInfo;
mod searchinfo;
use searchinfo::SearchInfo;
mod movement;

#[derive(Default)]
pub struct View {
    buffer: Buffer,
    needs_redraw: bool,
    size: Size,
    pub movement: Movement,
    scroll_offset: Position,
    search_info: Option<SearchInfo>,
    selection_start: Option<Location>,
    selection_end: Option<Location>,
}

impl View {
    pub fn get_status(&self) -> DocumentStatus {
        DocumentStatus {
            total_lines: self.buffer.height(),
            current_line_index: self.movement.text_location.line_index,
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
            prev_location: self.movement.text_location,
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
            self.movement.text_location = search_info.prev_location;
            self.scroll_offset = search_info.prev_scroll_offset;
            self.scroll_text_location_into_view(); // Ensure the previous location is still visible even if the terminal has been resized during search.
        }

        self.exit_search();
    }

    pub fn search(&mut self, query: &str) {
        if let Some(search_info) = &mut self.search_info {
            search_info.query = Some(Line::from(query));
        }

        self.search_in_direction(self.movement.text_location, SearchDirection::default());
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
            self.movement.text_location = location;
            self.center_text_location();
            self.set_needs_redraw(true);
        }
    }

    pub fn search_next(&mut self) {
        let step_right = self
            .get_search_query()
            .map_or(1, |query| min(query.grapheme_count(), 1));

        let location = Location {
            line_index: self.movement.text_location.line_index,
            grapheme_index: self
                .movement
                .text_location
                .grapheme_index
                .saturating_add(step_right), // Start the new search after the current match
        };

        self.search_in_direction(location, SearchDirection::Forward);
    }

    pub fn search_prev(&mut self) {
        self.search_in_direction(self.movement.text_location, SearchDirection::Backward);
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
        match command {
            Move::Up => self.movement.move_up(&self.buffer, 1),
            Move::Down => self.movement.move_down(&self.buffer, 1),
            Move::Left => self.movement.move_left(&self.buffer),
            Move::Right => self.movement.move_right(&self.buffer),
            Move::PageUp => self
                .movement
                .move_up(&self.buffer, self.size.height.saturating_sub(1)),
            Move::PageDown => self
                .movement
                .move_down(&self.buffer, self.size.height.saturating_sub(1)),
            Move::StartOfLine => self.movement.move_to_start_of_line(),
            Move::EndOfLine => self.movement.move_to_end_of_line(&self.buffer),
        }

        self.scroll_text_location_into_view();
    }

    //
    // Text Editing
    //

    fn insert_newline(&mut self) {
        self.buffer.insert_newline(self.movement.text_location);
        // move cursor to the beginning of the next line
        self.movement.text_location.line_index += 1;
        self.movement.text_location.grapheme_index = 0;
        self.set_needs_redraw(true);
    }

    fn delete_backward(&mut self) {
        if self.movement.text_location.line_index != 0
            || self.movement.text_location.grapheme_index != 0
        {
            self.handle_move_command(Move::Left);
            self.delete();
        }
    }

    fn delete(&mut self) {
        self.buffer.delete(self.movement.text_location);
        self.set_needs_redraw(true);
    }

    fn insert_char(&mut self, character: char) {
        let line_index = self.movement.text_location.line_index;

        // Get the old length of the line
        let old_len = if line_index < self.buffer.rope.len_lines() {
            let line_slice = self.buffer.rope.line(line_index);
            let line_str = line_slice.to_string();
            let line = Line::from(&line_str);
            line.grapheme_count()
        } else {
            0
        };

        self.buffer
            .insert_char(character, self.movement.text_location);

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
        let row = self.movement.text_location.line_index;
        debug_assert!(row.saturating_sub(1) <= self.buffer.rope.len_lines());
        let col = if row < self.buffer.rope.len_lines() {
            let line_slice = self.buffer.rope.line(row);
            let line_str = line_slice.to_string();
            let line = Line::from(&line_str);
            line.width_until(self.movement.text_location.grapheme_index)
        } else {
            0
        };
        Position { col, row }
    }

    pub fn update_insertion_point_to_cursor_position(&mut self) {
        let cursor_position = self.cursor_position();
        let location = Location {
            line_index: cursor_position.row + self.scroll_offset.row,
            grapheme_index: cursor_position.col + self.scroll_offset.col,
        };

        while self.buffer.height() <= location.line_index {
            self.buffer.insert_newline(Location {
                line_index: self.buffer.height(),
                grapheme_index: 0,
            });
        }

        self.movement.text_location = location;
    }

    //
    // Selection
    //

    pub fn clear_selection(&mut self) {
        self.selection_start = None;
        self.selection_end = None;
        self.set_needs_redraw(true);
    }

    pub fn start_selection(&mut self) {
        self.selection_start = Some(self.movement.text_location);
        self.selection_end = Some(self.movement.text_location);
        self.set_needs_redraw(true);
    }

    pub fn update_selection(&mut self) {
        if self.selection_start.is_some() {
            self.selection_end = Some(self.movement.text_location);
            self.set_needs_redraw(true);
        }
    }

    pub fn get_selection_range(&self) -> Option<(Location, Location)> {
        match (self.selection_start, self.selection_end) {
            (Some(start), Some(end)) => Some(
                if start.line_index <= end.line_index && start.grapheme_index <= end.grapheme_index
                {
                    (start, end)
                } else {
                    (end, start)
                },
            ),
            _ => None,
        }
    }
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
                .saturating_sub(origin_row)
                .saturating_add(scroll_top);

            if line_idx < self.buffer.rope.len_lines() {
                let line_slice = self.buffer.rope.line(line_idx);

                let left = self.scroll_offset.col;
                let right = self.scroll_offset.col.saturating_add(width);

                // Ensure left and right bounds are within the line's grapheme count
                let left = min(left, line_slice.len_chars());
                let right = min(right, line_slice.len_chars());

                // Only executes the slicing if the indices are valid
                if left <= right {
                    let visible_line = line_slice.slice(left..right);

                    let selection_range = self.get_selection_range().and_then(|(start, end)| {
                        if start.line_index <= line_idx && end.line_index >= line_idx {
                            let selection_start = if start.line_index == line_idx {
                                start.grapheme_index.saturating_sub(left)
                            } else {
                                0
                            };
                            let selection_end = if end.line_index == line_idx {
                                end.grapheme_index.saturating_sub(left)
                            } else {
                                right.saturating_sub(left)
                            };
                            Some((selection_start, selection_end))
                        } else {
                            None
                        }
                    });

                    Terminal::print_selected_row(current_row, visible_line, selection_range)?;
                } else {
                    Self::render_line(current_row, "~")?;
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
