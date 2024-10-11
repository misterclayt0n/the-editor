use std::{cmp::min, io::Error};

use crate::editor::color_scheme::ColorScheme;
use crate::editor::{Edit, Normal};
use crate::prelude::*;

use super::super::{DocumentStatus, Terminal};

use super::UIComponent;
mod buffer;
use buffer::Buffer;
use movement::Movement;
use ropey::RopeSlice;
mod fileinfo;
use fileinfo::FileInfo;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthChar;
mod movement;

#[derive(Default, Eq, PartialEq, Clone, Copy)]
pub enum SearchDirection {
    #[default]
    Forward,
    Backward,
}

pub struct SearchInfo {
    pub prev_location: Location,
    pub prev_scroll_offset: Position,
    pub query: Option<String>,
}

#[derive(Default)]
pub struct View {
    pub buffer: Buffer,
    needs_redraw: bool,
    size: Size,
    pub movement: Movement,
    scroll_offset: Position,
    search_info: Option<SearchInfo>,
    last_search_query: Option<String>,
    selection_start: Option<Location>,
    selection_end: Option<Location>,
    selection_mode: Option<SelectionMode>,
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
            self.scroll_text_location_into_view(); // ensure the previous location is still visible even if the terminal has been resized during search.
        }

        self.exit_search();
    }

    pub fn search(&mut self, query: &str) {
        if let Some(search_info) = &mut self.search_info {
            search_info.query = Some(query.to_string())
        }

        self.last_search_query = Some(query.to_string());
        self.search_in_direction(self.movement.text_location, SearchDirection::default());
    }

    fn get_search_query(&self) -> Option<&str> {
        if let Some(search_info) = &self.search_info {
            if let Some(query) = search_info.query.as_deref() {
                return Some(query);
            }
        }
        self.last_search_query.as_deref()
    }

    fn search_in_direction(&mut self, from: Location, direction: SearchDirection) {
        if let Some(query) = self.get_search_query() {
            if query.is_empty() {
                return;
            }

            let location = match direction {
                SearchDirection::Forward => self.buffer.search_forward(query, from),
                SearchDirection::Backward => self.buffer.search_backward(query, from),
            };

            if let Some(location) = location {
                self.movement.text_location = location;
                self.center_text_location();
                self.set_needs_redraw(true);
            }
        }
    }

    pub fn search_next(&mut self) {
        let step_right = self
            .get_search_query()
            .map_or(1, |query| query.graphemes(true).count().max(1));

        let location = Location {
            line_index: self.movement.text_location.line_index,
            grapheme_index: self
                .movement
                .text_location
                .grapheme_index
                .saturating_add(step_right),
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
            Edit::SubstituteChar => self.delete_char_at_cursor(),
            Edit::ChangeLine => {
                if let Some((start, end)) = self.get_selection_range() {
                    let start_line = start.line_index;
                    let end_line = end.line_index;

                    if start_line == end_line {
                        // only one line selected
                        self.replace_line_with_empty(start_line);
                    } else {
                        // multiple lines selected
                        // determine the index of initial and final char to line interval
                        let start_idx = self.buffer.rope.line_to_char(start_line);
                        let end_idx = self.buffer.rope.line_to_char(end_line + 1);

                        // remove all lines all at once
                        self.buffer.rope.remove(start_idx..end_idx);

                        // insert only one empty line in place of the first selected line
                        self.buffer.rope.insert(start_idx, "\n");

                        // update cursor postiion
                        self.movement.text_location = Location {
                            line_index: start_line,
                            grapheme_index: 0,
                        };

                        self.buffer.dirty = true;
                    }

                    // clear selection
                    self.clear_selection();
                    self.set_needs_redraw(true);
                }
            }
            Edit::SubstitueSelection => {
                self.delete_selection();
                self.clear_selection();
                self.update_insertion_point_to_cursor_position();
                self.set_needs_redraw(true);
            }
            Edit::InsertNewlineAbove => self.insert_newline_above(),
            Edit::InsertNewlineBelow => self.insert_newline_below(),
        }
    }

    pub fn handle_normal_command(&mut self, command: Normal) {
        match command {
            Normal::Up => self.movement.move_up(&self.buffer, 1),
            Normal::Down => self.movement.move_down(&self.buffer, 1),
            Normal::Left => self.movement.move_left(&self.buffer),
            Normal::Right => self.movement.move_right(&self.buffer),
            Normal::PageUp => self
                .movement
                .move_up(&self.buffer, self.size.height.saturating_div(2)),
            Normal::PageDown => self
                .movement
                .move_down(&self.buffer, self.size.height.saturating_div(2)),
            Normal::StartOfLine => self.movement.move_to_start_of_line(),
            Normal::EndOfLine => self.movement.move_to_end_of_line(&self.buffer),
            Normal::WordForward => self
                .movement
                .move_word_forward(&self.buffer, WordType::Word),
            Normal::WordBackward => self
                .movement
                .move_word_backward(&self.buffer, WordType::Word),
            Normal::BigWordForward => self
                .movement
                .move_word_forward(&self.buffer, WordType::BigWord),
            Normal::BigWordBackward => self
                .movement
                .move_word_backward(&self.buffer, WordType::BigWord),
            Normal::FirstCharLine => self.movement.move_to_first_non_whitespace(&self.buffer),
            Normal::GoToTop => self.movement.move_to_top(),
            Normal::GoToBottom => self.movement.move_to_bottom(&self.buffer),
            Normal::InsertAtLineStart => {
                self.movement.move_to_first_non_whitespace(&self.buffer);
                self.set_needs_redraw(true);
            }
            Normal::InsertAtLineEnd => {
                self.movement.move_to_end_of_line(&self.buffer);
                self.set_needs_redraw(true);
            }
            Normal::AppendRight => {
                if self.at_end_of_line() {
                    self.movement.move_to_end_of_line(&self.buffer);
                } else {
                    self.movement.move_right(&self.buffer);
                }
            }
            _ => {}
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

        // make sure the cursor is visible after inserting a newline
        self.scroll_text_location_into_view();
        self.set_needs_redraw(true);
    }

    fn delete_backward(&mut self) {
        if self.movement.text_location.line_index != 0
            || self.movement.text_location.grapheme_index != 0
        {
            self.handle_normal_command(Normal::Left);
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

    pub fn delete_current_line_and_leave_empty(&mut self) {
        let line_index = self.movement.text_location.line_index;

        self.buffer.delete_line(line_index);

        self.buffer.insert_newline(Location {
            line_index,
            grapheme_index: 0,
        });

        self.movement.text_location = Location {
            line_index,
            grapheme_index: 0,
        };

        self.set_needs_redraw(true);
    }

    pub fn delete_selection(&mut self) {
        if let Some((start, end)) = self.get_selection_range() {
            let start_idx = self.buffer.location_to_char_index(start);
            let mut end_idx = self.buffer.location_to_char_index(end);

            // include chars contained by the cursor and make sure they do not
            // exceed the length of the rope. we also only want this behavior for visual mode, not
            // visual line
            if end_idx < self.buffer.rope.len_chars()
                && self.selection_mode == Some(SelectionMode::Visual)
            {
                end_idx += 1;
            }

            self.buffer.rope.remove(start_idx..end_idx);
            self.buffer.dirty = true;

            // if the buffer gets empty after exclusion, make sure
            // at least one empty line is inserted
            if self.buffer.rope.len_chars() == 0 {
                self.buffer.insert_newline(Location {
                    line_index: 0,
                    grapheme_index: 0,
                });
            }

            // update cursor position
            self.movement.text_location = start;
            self.scroll_text_location_into_view();
            self.set_needs_redraw(true);
        }
    }

    pub fn delete_until_end_of_line(&mut self) {
        let line_index = self.movement.text_location.line_index;
        let grapheme_index = self.movement.text_location.grapheme_index;

        // check if cursor is within a valid line
        if line_index < self.buffer.height() {
            let line_length = self.buffer.get_line_length(line_index);
            if grapheme_index < line_length {
                // remove content from the cursor position till the end of the line
                let start_idx = self
                    .buffer
                    .location_to_char_index(self.movement.text_location);
                let end_idx = self.buffer.location_to_char_index(Location {
                    line_index,
                    grapheme_index: line_length.saturating_sub(1),
                });
                self.buffer.rope.remove(start_idx..end_idx);
                self.buffer.dirty = true;
            }
        }

        self.set_needs_redraw(true);
    }

    pub fn replace_line_with_empty(&mut self, line_index: usize) {
        if line_index < self.buffer.height() {
            self.buffer.delete_line(line_index); // delete existing line
            self.buffer.insert_newline(Location {
                line_index,
                grapheme_index: 0,
            }); // insert a new empty line in the same position
            self.movement.text_location = Location {
                line_index,
                grapheme_index: 0,
            }; // move the cursor to the beginning of the substituted line
        }
        self.set_needs_redraw(true);
    }

    fn insert_char(&mut self, character: char) {
        let line_index = self.movement.text_location.line_index;

        // if the buffer is empty, insert a new line before adding more characters
        if self.buffer.height() == 0 {
            self.buffer.insert_newline(Location {
                line_index: 0,
                grapheme_index: 0,
            });
        }

        // get the length of the line to assure we are inserting in the right point
        let line_width = self.buffer.get_line_length(line_index);

        // if the cursor is at the end of line, insert new char at the end
        if self.movement.text_location.grapheme_index >= line_width {
            self.buffer
                .insert_char(character, self.movement.text_location);
        } else {
            // else, just insert in the current cursor position
            self.buffer
                .insert_char(character, self.movement.text_location);
        }

        // move right after insertion
        self.movement.text_location.grapheme_index += 1;

        self.set_needs_redraw(true);
    }

    pub fn insert_newline_below(&mut self) {
        let line_index = self.movement.text_location.line_index;

        // move the cursor to the end of the line before inserting a new line
        self.movement.text_location.grapheme_index = self.buffer.get_line_length(line_index);

        self.buffer.insert_newline(Location {
            line_index: line_index + 1,
            grapheme_index: 0,
        });

        // move cursor to the beginning of the new line
        self.movement.text_location = Location {
            line_index: line_index + 1,
            grapheme_index: 0,
        };

        self.scroll_text_location_into_view();

        self.set_needs_redraw(true);
    }

    pub fn insert_newline_above(&mut self) {
        let line_index = self.movement.text_location.line_index;

        // move cursor to the beginning of the new line
        self.buffer.insert_newline(Location {
            line_index,
            grapheme_index: 0,
        });

        // move cursor to the beginning of the new line
        self.movement.text_location = Location {
            line_index,
            grapheme_index: 0,
        };

        self.set_needs_redraw(true);
    }

    pub fn delete_char_at_cursor(&mut self) {
        let line_index = self.movement.text_location.line_index;
        let grapheme_index = self.movement.text_location.grapheme_index;

        // check if the cursor is within a valid line and if there's a char to be deleted
        if line_index < self.buffer.height()
            && grapheme_index < self.buffer.get_line_length(line_index)
        {
            self.buffer.delete(self.movement.text_location);

            // no need to change the cursor position
            self.set_needs_redraw(true);
        }
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

        // hide the welcome message if it doesn't fit entirely.
        if remaining_width < len {
            return "~".to_string();
        }

        format!("{:<1}{:^remaining_width$}", "~", welcome_message)
    }

    /// calculates the line index in the buffer corresponding to the current row in the screen
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

    /// renders the current row based on the line index and other parameters
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

    /// renders a line that exists in the buffer
    fn render_existing_line(
        &self,
        current_row: RowIndex,
        line_idx: usize,
        width: usize,
    ) -> Result<(), Error> {
        let line_slice = self.buffer.rope.line(line_idx);

        if line_slice.len_chars() == 0 {
            self.render_empty_line(current_row, None)?;
        } else {
            let (left, right) = self.calculate_visible_range(line_slice.len_chars(), width);
            let visible_line = line_slice.slice(left..right);
            self.render_visible_line(current_row, line_idx, visible_line, left)?;
        }
        Ok(())
    }

    fn render_empty_line(
        &self,
        current_row: RowIndex,
        selection_range: Option<(usize, usize)>,
    ) -> Result<(), Error> {
        let expanded_line = if selection_range.is_some() {
            "~".to_string()
        } else {
            " ".to_string()
        };

        Terminal::print_selected_row(
            current_row,
            RopeSlice::from(expanded_line.as_str()),
            selection_range,
        )?;
        Ok(())
    }

    /// renders the visible portion of the line, handling search highlighting and selection.
    fn render_visible_line(
        &self,
        current_row: RowIndex,
        line_idx: usize,
        visible_line: RopeSlice,
        left: usize,
    ) -> Result<(), Error> {
        let query = self
            .search_info
            .as_ref()
            .and_then(|search_info| search_info.query.as_ref());

        let selected_match = (self.movement.text_location.line_index == line_idx
            && query.is_some())
        .then_some(self.movement.text_location.grapheme_index);

        // Store positions of each character after tab expansion
        let mut expanded_line = String::new();
        let mut char_positions = Vec::new(); // positions of each character after expansion
        let mut width = 0;

        for c in visible_line.chars() {
            if c == '\t' {
                let spaces_to_next_tab = TAB_WIDTH - (width % TAB_WIDTH);
                expanded_line.push_str(&" ".repeat(spaces_to_next_tab));
                for _ in 0..spaces_to_next_tab {
                    char_positions.push(width);
                    width += 1;
                }
            } else {
                expanded_line.push(c);
                char_positions.push(width);
                width += UnicodeWidthChar::width(c).unwrap_or(0);
            }
        }

        let selection_range = self.calculate_selection_range(line_idx, left, &char_positions);

        if let Some(_query) = query {
            self.render_line_with_search(
                current_row,
                RopeSlice::from(expanded_line.as_str()),
                selected_match,
            )?;
        } else if let Some((start, end)) = selection_range {
            self.render_line_with_selection(
                current_row,
                RopeSlice::from(expanded_line.as_str()),
                start,
                end,
            )?;
        } else {
            self.render_line_normal(current_row, RopeSlice::from(expanded_line.as_str()))?;
        }

        Ok(())
    }

    /// renders a line with search highlights.
    fn render_line_with_search(
        &self,
        current_row: RowIndex,
        visible_line: RopeSlice,
        _selected_match: Option<usize>,
    ) -> Result<(), Error> {
        let query = self.get_search_query();
        let line_str = visible_line.to_string();

        if let Some(query) = query {
            if let Some(start_idx) = line_str.find(query) {
                let end_idx = start_idx + query.len();
                Terminal::print_searched_row(
                    current_row,
                    RopeSlice::from(line_str.as_str()),
                    Some((start_idx, end_idx)),
                )?;
            } else {
                Terminal::print_row(current_row, &line_str)?;
            }
        } else {
            Terminal::print_row(current_row, &line_str)?;
        }
        Ok(())
    }

    /// renders a line with selection highlights.
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

    /// renders a line without any highlights.
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

    /// renders a line with a given string.
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
        let position = self.text_location_to_position();
        Position {
            row: position.row.saturating_sub(self.scroll_offset.row),
            col: position.col.saturating_sub(self.scroll_offset.col),
        }
    }

    fn text_location_to_position(&self) -> Position {
        let row = self.movement.text_location.line_index;
        debug_assert!(row.saturating_add(1) <= self.buffer.rope.len_lines());

        let col = self.expand_tabs_before_index(self.movement.text_location.grapheme_index, row);

        Position { col, row }
    }

    pub fn update_insertion_point_to_cursor_position(&mut self) {
        let cursor_position = self.cursor_position();
        let line_index = cursor_position.row + self.scroll_offset.row;

        if self.buffer.height() == 0 {
            self.movement.text_location = Location {
                line_index: 0,
                grapheme_index: 0,
            };
        }

        // make sure buffer has enough lines
        while self.buffer.height() <= line_index {
            self.buffer.insert_newline(Location {
                line_index: self.buffer.height(),
                grapheme_index: 0,
            });
        }

        // calculate the grapheme index based on the width of the cursor
        let grapheme_index = if line_index < self.buffer.rope.len_lines() {
            let line_slice = self.buffer.rope.line(line_index);
            let mut current_width = 0;
            let mut grapheme_index = 0;

            // iterate over graphemes of the line to calculate the position
            for (i, grapheme) in line_slice.to_string().graphemes(true).enumerate() {
                let char_width = if grapheme == "\t" {
                    // calculate how many spaces the tabs represent
                    TAB_WIDTH - (current_width % TAB_WIDTH)
                } else {
                    // get grapheme width
                    UnicodeWidthChar::width(grapheme.chars().next().unwrap()).unwrap_or(0)
                };

                if current_width >= cursor_position.col + self.scroll_offset.col {
                    grapheme_index = i;
                    break;
                }

                current_width += char_width;
                grapheme_index = i + 1;
            }

            grapheme_index
        } else {
            0
        };

        // update the location of insertion point
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
        self.selection_mode = None;
        self.set_needs_redraw(true);
    }

    pub fn start_selection(&mut self, mode: SelectionMode) {
        self.selection_mode = Some(mode);

        match mode {
            SelectionMode::Visual => {
                self.selection_start = Some(self.movement.text_location);
                self.selection_end = Some(self.movement.text_location);
            }
            SelectionMode::VisualLine => {
                let current_line = self.movement.text_location.line_index;
                self.selection_start = Some(Location {
                    line_index: current_line,
                    grapheme_index: 0,
                });
                self.selection_end = Some(Location {
                    line_index: current_line,
                    grapheme_index: self.buffer.get_line_length(current_line),
                });
            }
        }

        self.set_needs_redraw(true);
    }

    pub fn update_selection(&mut self) {
        if let Some(mode) = self.selection_mode {
            match mode {
                SelectionMode::Visual => {
                    self.selection_end = Some(self.movement.text_location);
                }
                SelectionMode::VisualLine => {
                    // only update selection_end, keep selection_start fixed
                    self.selection_end = Some(Location {
                        line_index: self.movement.text_location.line_index,
                        grapheme_index: self
                            .buffer
                            .get_line_length(self.movement.text_location.line_index),
                    });
                }
            }
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

    fn get_expanded_line_length(&self, line_idx: usize) -> usize {
        let line_slice = self.buffer.rope.line(line_idx);
        let mut expanded_width = 0;

        for c in line_slice.chars() {
            if c == '\t' {
                let spaces_to_next_tab = TAB_WIDTH - (expanded_width % TAB_WIDTH);
                expanded_width += spaces_to_next_tab;
            } else {
                expanded_width += UnicodeWidthChar::width(c).unwrap_or(0);
            }
        }

        if expanded_width == 0 {
            1
        } else {
            expanded_width
        }
    }

    pub fn handle_visual_movement(&mut self, command: Normal) {
        self.handle_normal_command(command);
        self.update_selection();
    }

    pub fn handle_visual_line_movement(&mut self, command: Normal) {
        match command {
            Normal::Up => self.movement.move_up(&self.buffer, 1),
            Normal::Down => self.movement.move_down(&self.buffer, 1),
            Normal::Left => self.movement.move_left(&self.buffer),
            Normal::Right => self.movement.move_right(&self.buffer),
            Normal::GoToTop => self.movement.move_to_top(),
            Normal::GoToBottom => self.movement.move_to_bottom(&self.buffer),
            Normal::PageUp => self
                .movement
                .move_up(&self.buffer, self.size.height.saturating_div(2)),
            Normal::PageDown => self
                .movement
                .move_down(&self.buffer, self.size.height.saturating_div(2)),
            _ => {}
        }

        self.update_selection(); // update selection to include the new line
        self.scroll_text_location_into_view();
    }

    pub fn at_end_of_line(&self) -> bool {
        let line_index = self.movement.text_location.line_index;
        let grapheme_index = self.movement.text_location.grapheme_index;

        // check if the cursor is at the end of the line
        if line_index < self.buffer.height() {
            let line_length = self.buffer.get_line_length(line_index);
            grapheme_index >= line_length
        } else {
            false
        }
    }

    /// calculates the selection range for the current line if any selection exists.
    fn calculate_selection_range(
        &self,
        line_idx: usize,
        left: usize,
        char_positions: &[usize],
    ) -> Option<(usize, usize)> {
        self.get_selection_range().and_then(|(start, end)| {
            if start.line_index <= line_idx && end.line_index >= line_idx {
                match self.selection_mode {
                    Some(SelectionMode::Visual) => {
                        let start_idx = if start.line_index == line_idx {
                            self.expand_tabs_before_index(start.grapheme_index, line_idx)
                        } else {
                            0
                        };

                        let end_idx = if end.line_index == line_idx {
                            self.expand_tabs_before_index(end.grapheme_index, line_idx)
                        } else {
                            char_positions.len()
                        };

                        if start_idx >= end_idx {
                            return None;
                        }

                        let selection_start = start_idx.saturating_sub(left);
                        let selection_end = end_idx.saturating_sub(left).saturating_add(1);
                        Some((selection_start, selection_end))
                    }
                    Some(SelectionMode::VisualLine) => {
                        let line_length = self.get_expanded_line_length(line_idx);
                        Some((0, line_length))
                    }
                    None => None,
                }
            } else {
                None
            }
        })
    }

    /// calculates the visible range (left and right indices) of the line based on scrolling.
    fn calculate_visible_range(&self, line_length: usize, width: usize) -> (usize, usize) {
        let left = min(self.scroll_offset.col, line_length);
        let right = min(self.scroll_offset.col.saturating_add(width), line_length);
        (left, right)
    }

    fn expand_tabs_before_index(&self, index: usize, line_idx: usize) -> usize {
        let line_slice = self.buffer.rope.line(line_idx);
        let mut expanded_width = 0;
        let mut grapheme_count = 0;

        for c in line_slice.chars() {
            if grapheme_count >= index {
                break;
            }
            if c == '\t' {
                let spaces_to_next_tab = TAB_WIDTH - (expanded_width % TAB_WIDTH);
                expanded_width += spaces_to_next_tab;
            } else {
                expanded_width += UnicodeWidthChar::width(c).unwrap_or(0);
            }
            grapheme_count += 1;
        }
        expanded_width
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
