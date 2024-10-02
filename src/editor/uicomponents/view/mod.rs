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
use ropey::RopeSlice;
use searchdirection::SearchDirection;
mod fileinfo;
use fileinfo::FileInfo;
mod searchinfo;
use searchinfo::SearchInfo;
use unicode_width::UnicodeWidthChar;
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
                .move_up(&self.buffer, self.size.height.saturating_div(2)),
            Move::PageDown => self
                .movement
                .move_down(&self.buffer, self.size.height.saturating_div(2)),
            Move::StartOfLine => self.movement.move_to_start_of_line(),
            Move::EndOfLine => self.movement.move_to_end_of_line(&self.buffer),
            Move::WordForward => self
                .movement
                .move_word_forward(&self.buffer, WordType::Word),
            Move::WordBackward => self
                .movement
                .move_word_backward(&self.buffer, WordType::Word),
            Move::BigWordForward => self
                .movement
                .move_word_forward(&self.buffer, WordType::BigWord),
            Move::BigWordBackward => self
                .movement
                .move_word_backward(&self.buffer, WordType::BigWord),
            Move::FirstCharLine => self.movement.move_to_first_non_whitespace(&self.buffer)
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

    pub fn delete_current_line(&mut self) {
        let line_index = self.movement.text_location.line_index;

        self.buffer.delete_line(line_index);

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

    /// Calculates the line index in the buffer corresponding to the current row in the screen
    fn calculate_line_index(
        &self,
        current_row: RowIndex,
        origin_row: RowIndex,
        scroll_top: usize,
    ) -> usize {
        current_row
            .saturating_sub(origin_row)
            .saturating_add(scroll_top)
    }

    /// Renders the current row based on the line index and other parameters
    fn render_current_row(
        &self,
        current_row: RowIndex,
        line_idx: usize,
        top_third: usize,
        width: usize,
    ) -> Result<(), Error> {
        if line_idx < self.buffer.rope.len_lines() {
            self.render_existing_line(current_row, line_idx, width)?;
        } else if current_row == top_third && self.buffer.is_empty() {
            Self::render_line(current_row, &Self::build_welcome_message(width))?;
        } else {
            Self::render_line(current_row, "~")?;
        }

        Ok(())
    }

    /// Renders a line that exists in the buffer
    fn render_existing_line(
        &self,
        current_row: RowIndex,
        line_idx: usize,
        width: usize,
    ) -> Result<(), Error> {
        let line_slice = self.buffer.rope.line(line_idx);
        let (left, right) = self.calculate_visible_range(line_slice.len_chars(), width);

        if left <= right {
            let visible_line = line_slice.slice(left..right);
            self.render_visible_line(current_row, line_idx, visible_line, left, right)?;
        } else {
            Self::render_line(current_row, "~")?;
        }

        Ok(())
    }

    /// Calculates the visible range (left and right indices) of the line based on scrolling.
    fn calculate_visible_range(&self, line_length: usize, width: usize) -> (usize, usize) {
        let left = min(self.scroll_offset.col, line_length);
        let right = min(self.scroll_offset.col.saturating_add(width), line_length);
        (left, right)
    }

    /// Renders the visible portion of the line, handling search highlighting and selection.
    fn render_visible_line(
        &self,
        current_row: RowIndex,
        line_idx: usize,
        visible_line: RopeSlice,
        left: usize,
        right: usize,
    ) -> Result<(), Error> {
        let query = self
            .search_info
            .as_ref()
            .and_then(|search_info| search_info.query.as_ref());

        let selected_match = (self.movement.text_location.line_index == line_idx
            && query.is_some())
        .then_some(self.movement.text_location.grapheme_index);

        let selection_range = self.calculate_selection_range(line_idx, left, right);

        let mut expanded_line = String::new();
        let mut width = 0;
        for c in visible_line.chars() {
            if c == '\t' {
                let spaces_to_next_tab = TAB_WIDTH - (width % TAB_WIDTH);
                expanded_line.push_str(&" ".repeat(spaces_to_next_tab));
                width += spaces_to_next_tab;
            } else {
                expanded_line.push(c);
                width += UnicodeWidthChar::width(c).unwrap_or(0);
            }
        }

        if let Some(query) = query {
            self.render_line_with_search(
                current_row,
                RopeSlice::from(expanded_line.as_str()), // Mudança aqui
                query,
                selected_match,
            )?;
        } else if let Some((start, end)) = selection_range {
            self.render_line_with_selection(
                current_row,
                RopeSlice::from(expanded_line.as_str()), // Mudança aqui
                start,
                end,
            )?;
        } else {
            self.render_line_normal(current_row, RopeSlice::from(expanded_line.as_str()))?;
            // Mudança aqui
        }

        Ok(())
    }

    /// Calculates the selection range for the current line if any selection exists.
    fn calculate_selection_range(
        &self,
        line_idx: usize,
        left: usize,
        right: usize,
    ) -> Option<(usize, usize)> {
        self.get_selection_range().and_then(|(start, end)| {
            if start.line_index <= line_idx && end.line_index >= line_idx {
                let selection_start = if start.line_index == line_idx {
                    self.expand_tabs_before_index(start.grapheme_index, line_idx) - left
                } else {
                    0
                };
                let selection_end = if end.line_index == line_idx {
                    self.expand_tabs_before_index(end.grapheme_index, line_idx) - left
                } else {
                    right.saturating_sub(left)
                };
                Some((selection_start, selection_end))
            } else {
                None
            }
        })
    }

    fn expand_tabs_before_index(&self, index: usize, line_idx: usize) -> usize {
        let line_slice = self.buffer.rope.line(line_idx);
        let mut expanded_width = 0;

        for c in line_slice.chars().take(index) {
            if c == '\t' {
                expanded_width += TAB_WIDTH - (expanded_width % TAB_WIDTH);
            } else {
                expanded_width += UnicodeWidthChar::width(c).unwrap_or(0);
            }
        }

        expanded_width
    }

    /// Renders a line with search highlights.
    fn render_line_with_search(
        &self,
        current_row: RowIndex,
        visible_line: RopeSlice,
        query: &Line,
        selected_match: Option<GraphemeIndex>,
    ) -> Result<(), Error> {
        let line_str = visible_line.to_string();
        let line = Line::from(&line_str);

        Terminal::print_annotated_row(
            current_row,
            &line.get_annotated_visible_substr(0..line_str.len(), Some(query), selected_match),
        )?;

        Ok(())
    }

    /// Renders a line with selection highlights.
    fn render_line_with_selection(
        &self,
        current_row: RowIndex,
        visible_line: RopeSlice,
        start: usize,
        end: usize,
    ) -> Result<(), Error> {
        Terminal::print_selected_row(current_row, visible_line, Some((start, end)))?;
        Ok(())
    }

    /// Renders a line without any highlights.
    fn render_line_normal(
        &self,
        current_row: RowIndex,
        visible_line: RopeSlice,
    ) -> Result<(), Error> {
        let line_str = visible_line.to_string();
        let expanded_line = self.expand_tabs(&line_str);
        Terminal::print_row(current_row, &expanded_line)?;
        Ok(())
    }

    fn expand_tabs(&self, input: &str) -> String {
        let mut output = String::new();
        let mut width = 0;

        for c in input.chars() {
            if c == '\t' {
                let spaces_to_next_tab = TAB_WIDTH - (width % TAB_WIDTH);
                for _ in 0..spaces_to_next_tab {
                    output.push(' ');
                }
                width += spaces_to_next_tab;
            } else {
                output.push(c);
                width += UnicodeWidthChar::width(c).unwrap_or(0);
            }
        }

        output
    }

    /// Renders a line with a given string.
    fn render_line(at: RowIndex, line_text: &str) -> Result<(), Error> {
        Terminal::print_row(at, line_text)
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
        debug_assert!(row.saturating_add(1) <= self.buffer.rope.len_lines());

        let mut col = 0;
        if row < self.buffer.rope.len_lines() {
            let line_slice = self.buffer.rope.line(row);
            let line_str = line_slice.to_string();

            for (_, c) in line_str
                .chars()
                .take(self.movement.text_location.grapheme_index)
                .enumerate()
            {
                if c == '\t' {
                    col += TAB_WIDTH - (col % TAB_WIDTH);
                } else {
                    col += UnicodeWidthChar::width(c).unwrap_or(0);
                }
            }
        }

        Position { col, row }
    }

    pub fn update_insertion_point_to_cursor_position(&mut self) {
        let cursor_position = self.cursor_position();
        let line_index = cursor_position.row + self.scroll_offset.row;

        while self.buffer.height() <= line_index {
            self.buffer.insert_newline(Location {
                line_index: self.buffer.height(),
                grapheme_index: 0,
            });
        }

        let grapheme_index = if line_index < self.buffer.rope.len_lines() {
            let line_slice = self.buffer.rope.line(line_index);
            let line_str = line_slice.to_string();
            let line = Line::from(&line_str);
            line.grapheme_index_at_width(cursor_position.col + self.scroll_offset.col)
        } else {
            0
        };

        self.movement.text_location = Location {
            line_index,
            grapheme_index,
        };
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
            (Some(start), Some(end)) => {
                if start <= end {
                    Some((start, end))
                } else {
                    Some((end, start))
                }
            }
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
            let line_idx = self.calculate_line_index(current_row, origin_row, scroll_top);
            self.render_current_row(current_row, line_idx, top_third, width)?;
        }

        Ok(())
    }
}
