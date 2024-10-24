// TODO: abstract edit commands into a new file, same thing as movement.rs,
// but now edit.rs

use std::{cmp::min, io::Error};

use crate::editor::color_scheme::ColorScheme;
use crate::editor::{Edit, Normal, Operator, TextObject};
use crate::prelude::*;

use super::super::{DocumentStatus, Terminal};

use super::UIComponent;
mod buffer;
use buffer::Buffer;
use movement::Movement;
mod fileinfo;
use fileinfo::FileInfo;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};
mod movement;

#[derive(Default, Eq, PartialEq, Clone, Copy)]
pub enum SearchDirection {
    #[default]
    Forward,
    Backward,
}

#[derive(Clone)]
pub struct SearchInfo {
    pub prev_location: Location,
    pub prev_scroll_offset: Position,
    pub query: Option<String>,
}

#[derive(Default, Clone)]
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
    rendered_lines: Vec<String>,
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
            Normal::LeftAfterDeletion => self.movement.move_left_after_deletion(&self.buffer),
            Normal::Right => self.movement.move_right(&self.buffer),
            Normal::PageUp => {
                let half_screen = self.size.height / 2;
                self.movement.move_up(&self.buffer, half_screen);

                // new offset to center the cursor
                let desired_scroll = self
                    .movement
                    .text_location
                    .line_index
                    .saturating_sub(half_screen);

                // clamp offset to ensure it's within valid bounds
                self.scroll_offset.row = min(
                    desired_scroll,
                    self.buffer.height().saturating_sub(self.size.height),
                );

                self.scroll_offset.row = self.scroll_offset.row.max(0);
                self.set_needs_redraw(true);
                return;
            }
            Normal::PageDown => {
                let half_screen = self.size.height / 2;
                self.movement.move_down(&self.buffer, half_screen);

                let desired_scroll = self
                    .movement
                    .text_location
                    .line_index
                    .saturating_sub(half_screen);

                self.scroll_offset.row = min(
                    desired_scroll,
                    self.buffer.height().saturating_sub(self.size.height),
                );

                self.scroll_offset.row = self.scroll_offset.row.max(0);
                self.set_needs_redraw(true);
                return;
            }
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
            Normal::WordEndForward => self
                .movement
                .move_word_end_forward(&self.buffer, WordType::Word),
            Normal::BigWordEndForward => self
                .movement
                .move_word_end_forward(&self.buffer, WordType::BigWord),
            Normal::FirstCharLine => self.movement.move_to_first_non_whitespace(&self.buffer),
            Normal::GoToTop => self.movement.move_to_top(),
            Normal::GoToBottom => self.movement.move_to_bottom(&self.buffer),
            Normal::InsertAtLineStart => {
                self.movement.move_to_first_non_whitespace(&self.buffer);
                self.set_needs_redraw(true);
            }
            Normal::InsertAtLineEnd => {
                self.movement.move_to_end_of_line_beyond_end(&self.buffer);
                self.set_needs_redraw(true);
            }
            Normal::AppendRight => {
                self.movement.move_right_beyond_end(&self.buffer);
            }
        }

        self.scroll_text_location_into_view();
    }

    pub fn handle_operator_text_object(&mut self, operator: Operator, text_object: TextObject) {
        match operator {
            Operator::Delete => match text_object {
                TextObject::Inner(delimiter) => {
                    if let Some((start, end)) = self.find_text_object_range(delimiter, true) {
                        // remove all content between the delimiters
                        self.buffer.rope.remove(start..end);
                        self.buffer.dirty = true;

                        // update cursor location to the beginning of the removed interval
                        self.movement.text_location = self.buffer.char_index_to_location(start);
                        self.scroll_text_location_into_view();
                        self.set_needs_redraw(true);
                    }
                } // TODO: add more text objects
            },
            Operator::Change => match text_object {
                TextObject::Inner(delimiter) => {
                    if let Some((start, end)) = self.find_text_object_range(delimiter, true) {
                        // get initial and end locations
                        let start_location = self.buffer.char_index_to_location(start);
                        let end_location = self.buffer.char_index_to_location(end);

                        // TODO: I should probably abstract away most of the indentation code,
                        // I guess something like: adjust_indentation()
                        // check if delimiters are on the same line
                        if start_location.line_index == end_location.line_index {
                            // first cenario: same line
                            self.buffer.rope.remove(start..end);
                            self.buffer.dirty = true;
                            self.movement.text_location = self.buffer.char_index_to_location(start);
                            self.scroll_text_location_into_view();
                            self.set_needs_redraw(true);
                        } else {
                            // second cenario: delimiters in differnet lines
                            self.buffer.rope.remove(start..end);
                            self.buffer.dirty = true;

                            let current_indentation =
                                self.get_indentation_of_line(start_location.line_index);

                            let mut new_indentation = current_indentation.clone();
                            new_indentation.push('\t');

                            let indentation_string = format!("\n{}", new_indentation);
                            self.buffer.rope.insert(start, &indentation_string);
                            self.buffer.dirty = true;

                            let new_cursor_location = start + indentation_string.len();
                            self.movement.text_location =
                                self.buffer.char_index_to_location(new_cursor_location);
                            self.scroll_text_location_into_view();
                            self.set_needs_redraw(true);
                        }
                    }
                }
            },
            // TODO: add more operators
            _ => {}
        }
    }

    //
    // Text objects
    //

    fn find_text_object_range(&self, delimiter: char, inner: bool) -> Option<(usize, usize)> {
        // define closing and opening delimiters based on the provided delimiter
        let (open_delim, close_delim) = MATCHING_DELIMITERS
            .iter()
            .find(|&&(open, close)| open == delimiter || close == delimiter)
            .map_or(('\0', '\0'), |&(open, close)| (open, close));

        if open_delim == '\0' || close_delim == '\0' {
            return None;
        }

        let current_location = self.movement.text_location;
        let cursor_index = self.buffer.location_to_char_index(current_location);

        // first try: reverse search from the cursor position to find the opening delimiter
        if let Some(start) =
            self.find_matching_open_delim_backward(cursor_index, open_delim, close_delim)
        {
            // from the opening delimiter, find the corresponding closing delimiter
            if let Some(end) =
                self.find_matching_close_delim_forward(start, open_delim, close_delim)
            {
                let range_start = if inner { start + 1 } else { start };
                let range_end = if inner {
                    // search the last character (non whitespace) before the closing delimiter
                    let mut last_non_space = end - 1;
                    while last_non_space > range_start
                        && self.buffer.rope.char(last_non_space).is_whitespace()
                    {
                        last_non_space -= 1;
                    }
                    last_non_space + 1
                } else {
                    end + 1
                };
                return Some((range_start, range_end));
            }
        }

        // second try: direct search from the cursor position to find the next opening delimiter
        if let Some(start) =
            self.find_matching_open_delim_forward(cursor_index, open_delim, close_delim)
        {
            // from the opening delimiter, search the closing correspondence
            if let Some(end) =
                self.find_matching_close_delim_forward(start, open_delim, close_delim)
            {
                let range_start = if inner { start + 1 } else { start };
                let range_end = if inner { end } else { end + 1 };
                return Some((range_start, range_end));
            }
        }

        None
    }

    fn find_matching_open_delim_backward(
        &self,
        mut start_index: usize,
        open_delim: char,
        close_delim: char,
    ) -> Option<usize> {
        let mut depth = 0;

        while start_index > 0 {
            start_index -= 1;
            let c = self.buffer.rope.char(start_index);
            if c == close_delim {
                depth += 1;
            } else if c == open_delim {
                if depth == 0 {
                    return Some(start_index);
                } else {
                    depth -= 1;
                }
            }
        }

        None
    }

    fn find_matching_close_delim_forward(
        &self,
        start_index: usize,
        open_delim: char,
        close_delim: char,
    ) -> Option<usize> {
        let mut depth = 0;
        let total_chars = self.buffer.rope.len_chars();
        let mut index = start_index + 1;

        while index < total_chars {
            let c = self.buffer.rope.char(index);
            if c == open_delim {
                depth += 1;
            } else if c == close_delim {
                if depth == 0 {
                    return Some(index);
                } else {
                    depth -= 1;
                }
            }
            index += 1;
        }

        None
    }

    fn find_matching_open_delim_forward(
        &self,
        mut start_index: usize,
        open_delim: char,
        close_delim: char,
    ) -> Option<usize> {
        let total_chars = self.buffer.rope.len_chars();

        while start_index < total_chars {
            let c = self.buffer.rope.char(start_index);
            if c == open_delim {
                return Some(start_index);
            } else if c == close_delim {
                // if i find a closing delimiter before an opening one, ignore the motherfucker
                // (maybe) adjust logic as necessary
            }
            start_index += 1;
        }

        None
    }

    //
    // Text Editing
    //

    fn delete_backward(&mut self) {
        if self.movement.text_location.line_index != 0
            || self.movement.text_location.grapheme_index != 0
        {
            self.handle_normal_command(Normal::LeftAfterDeletion);
            self.delete();
        }
    }

    fn delete(&mut self) {
        self.buffer.delete(self.movement.text_location);
        self.set_needs_redraw(true);
    }

    pub fn delete_current_line(&mut self) {
        let line_index = self.movement.text_location.line_index;

        if line_index >= self.buffer.height() - 1 && self.buffer.height() > 1 {
            self.movement.move_up(&self.buffer, 1);
        }

        self.buffer.delete_line(line_index);

        self.set_needs_redraw(true);
    }

    pub fn delete_current_line_and_leave_empty(&mut self) {
        let line_index = self.movement.text_location.line_index;

        self.buffer.delete_line(line_index);

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

            // gotta make sure end_idx does not exceed the size of the rope
            end_idx = end_idx.min(self.buffer.rope.len_chars());

            if start_idx < end_idx {
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
                self.movement.text_location = self.buffer.char_index_to_location(
                    start_idx.min(self.buffer.rope.len_chars().saturating_sub(1)),
                );
                self.scroll_text_location_into_view();
                self.set_needs_redraw(true);
            }
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
                    grapheme_index: line_length,
                });
                self.buffer.rope.remove(start_idx..end_idx);
                self.buffer.dirty = true;
            }
        }

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
        let grapheme_index = self.movement.text_location.grapheme_index;

        if character == '"' || character == '\'' {
            let current_line = self.buffer.rope.line(line_index).to_string();
            let before_cursor = &current_line[..grapheme_index];

            let count_opening_delimiters = before_cursor.matches(character).count();

            // if there's an odd number of delimiters before the cursor, we insert a closing one
            // NOTE: I'm not really sure how people usually handle this sort of situation,
            // but I was to lazy to look up the source code of Zed or Neovim, and decided that
            // this was the easiest, most direct approach possible hahaha
            if count_opening_delimiters % 2 != 0 {
                self.buffer
                    .insert_char(character, self.movement.text_location);
                self.movement.text_location.grapheme_index += 1;
                self.set_needs_redraw(true);
                self.scroll_text_location_into_view();
                return;
            } else {
                // in this case, we handle both opening and closing
                self.buffer
                    .insert_char(character, self.movement.text_location);
                self.movement.text_location.grapheme_index += 1;
                self.buffer
                    .insert_char(character, self.movement.text_location);
                self.set_needs_redraw(true);
                self.scroll_text_location_into_view();
                return;
            }
        }

        if let Some(&(_, closing_delim)) = MATCHING_DELIMITERS
            .iter()
            .find(|&&(open, _)| open == character)
        {
            self.buffer
                .insert_char(character, self.movement.text_location);
            self.movement.text_location.grapheme_index += 1; // move right

            self.buffer
                .insert_char(closing_delim, self.movement.text_location); // position
            self.set_needs_redraw(true);
            self.scroll_text_location_into_view();
            return;
        }

        if let Some(&(_, closing_delim)) = MATCHING_DELIMITERS
            .iter()
            .find(|&&(_, close)| close == character)
        {
            // check if following character is the same closing delimiter
            let line_slice = self.buffer.rope.line(line_index).to_string();
            if grapheme_index < line_slice.len()
                && line_slice.chars().nth(grapheme_index) == Some(closing_delim)
            {
                self.movement.text_location.grapheme_index += 1;
                self.set_needs_redraw(true);
                self.scroll_text_location_into_view();
                return;
            }
        }

        // checking if the character is a closing delinmiter
        if let Some(&(_, _)) = MATCHING_DELIMITERS
            .iter()
            .find(|&&(_, close)| close == character)
        {
            let line_slice = self.buffer.rope.line(line_index);
            let line_str = line_slice.to_string();

            let prefix = if grapheme_index > 0 {
                &line_str[..grapheme_index]
            } else {
                ""
            };

            // Se a linha antes do delimitador estiver vazia (apenas espaços), diminui a indentação
            if prefix.trim().is_empty() {
                self.decrease_indentation(line_index);
            }
        }

        self.buffer
            .insert_char(character, self.movement.text_location);
        self.movement.text_location.grapheme_index += 1;
        self.set_needs_redraw(true);
        self.scroll_text_location_into_view()
    }

    fn insert_newline(&mut self) {
        if let Some(true) = self.is_cursor_between_matching_delimiters() {
            // insert new line after opening delimiter
            self.buffer.insert_newline(self.movement.text_location);
            self.movement.text_location.line_index += 1;
            self.movement.text_location.grapheme_index = 0;

            // insert augmented indentation in new line
            let mut indentation =
                self.get_indentation_of_line(self.movement.text_location.line_index - 1);
            indentation.push('\t'); // increase indentation (could be adjusted)

            for c in indentation.chars() {
                self.buffer.insert_char(c, self.movement.text_location);
                self.movement.text_location.grapheme_index += 1;
            }
            let cursor_grapheme_index_after_indent = self.movement.text_location.grapheme_index;

            // insert new line before the closing delimiter with reduced indentation
            self.buffer.insert_newline(self.movement.text_location);
            self.movement.text_location.line_index += 1;
            self.movement.text_location.grapheme_index = 0;

            // pop the mf
            indentation.pop();

            for c in indentation.chars() {
                self.buffer.insert_char(c, self.movement.text_location);
                self.movement.text_location.grapheme_index += 1;
            }

            // position the cursor in indented line
            self.movement.text_location = Location {
                line_index: self.movement.text_location.line_index - 1,
                grapheme_index: cursor_grapheme_index_after_indent,
            };
            self.movement.update_desired_col(&self.buffer);
        } else {
            // normal insertion behavior in new line insertion
            let line_index = self.movement.text_location.line_index;
            let grapheme_index = self.movement.text_location.grapheme_index;
            let current_line = self.buffer.rope.line(line_index).to_string();

            // get current line indentation
            let mut indentation = self.get_indentation_of_line(line_index);

            // text before and after the cursor
            let text_before_cursor = &current_line[..grapheme_index];
            let trimmed_text_before_cursor = text_before_cursor.trim_end();
            let text_after_cursor = &current_line[grapheme_index..];
            let trimmed_text_after_cursor = text_after_cursor.trim_start();

            // adjust indentation based on adjacent delimiters
            if trimmed_text_before_cursor.ends_with('{')
                || trimmed_text_before_cursor.ends_with('(')
                || trimmed_text_before_cursor.ends_with('[')
            {
                indentation.push('\t'); // make it biggger
            }
            if trimmed_text_after_cursor.starts_with('}')
                || trimmed_text_after_cursor.starts_with(')')
                || trimmed_text_after_cursor.starts_with(']')
            {
                if !indentation.is_empty() {
                    indentation.pop(); // make it smaller
                }
            }

            // insert new line
            self.buffer.insert_newline(self.movement.text_location);
            self.movement.text_location.line_index += 1;
            self.movement.text_location.grapheme_index = 0;

            // insert indentation in new line
            for c in indentation.chars() {
                self.buffer.insert_char(c, self.movement.text_location);
                self.movement.text_location.grapheme_index += 1;
            }
            self.movement.update_desired_col(&self.buffer);
        }

        // update cursor position to make sure it's visible
        self.scroll_text_location_into_view();
        self.set_needs_redraw(true);
    }

    pub fn insert_newline_below(&mut self) {
        let line_index = self.movement.text_location.line_index;

        let current_line = self.buffer.rope.line(line_index).to_string();

        let mut indentation = self.get_indentation_of_line(line_index);

        let trimmed_current_line = current_line.trim();

        // if the line ends with an opening delimiter, increase indentation
        if trimmed_current_line.ends_with('{')
            || trimmed_current_line.ends_with('(')
            || trimmed_current_line.ends_with('[')
        {
            indentation.push('\t');
        }

        // move cursor to the end of line before inserting a new line
        self.movement.text_location.grapheme_index = self.buffer.get_line_length(line_index);

        self.buffer.insert_newline(Location {
            line_index: line_index + 1,
            grapheme_index: 0,
        });

        // move cursor to the beginning of next line
        self.movement.text_location = Location {
            line_index: line_index + 1,
            grapheme_index: 0,
        };

        // insert indentation in new line
        for c in indentation.chars() {
            self.buffer.insert_char(c, self.movement.text_location);
            self.movement.text_location.grapheme_index += 1;
        }

        self.movement.update_desired_col(&self.buffer);
        self.scroll_text_location_into_view();
        self.set_needs_redraw(true);
    }

    pub fn insert_newline_above(&mut self) {
        let line_index = self.movement.text_location.line_index;

        let current_line = self.buffer.rope.line(line_index).to_string();

        let mut indentation = self.get_indentation_of_line(line_index);

        let trimmed_current_line = current_line.trim();

        // same thing as before, but with closing delimiters now
        if trimmed_current_line.starts_with('}')
            || trimmed_current_line.starts_with(')')
            || trimmed_current_line.starts_with(']')
        {
            indentation.push('\t');
        }

        self.buffer.insert_newline(Location {
            line_index,
            grapheme_index: 0,
        });

        self.movement.text_location = Location {
            line_index,
            grapheme_index: 0,
        };

        for c in indentation.chars() {
            self.buffer.insert_char(c, self.movement.text_location);
            self.movement.text_location.grapheme_index += 1;
        }

        self.movement.update_desired_col(&self.buffer);
        self.scroll_text_location_into_view();
        self.set_needs_redraw(true);
    }

    //
    // Rendering
    //

    fn get_rendered_line(&self, line_idx: usize, width: usize) -> Result<String, Error> {
        let line_slice = self.buffer.rope.line(line_idx);

        // just in case the line is empty
        if line_slice.len_chars() == 0 {
            return Ok(" ".to_string());
        }

        let (left, right) = self.calculate_visible_range(line_slice.len_chars(), width);
        let visible_line = line_slice.slice(left..right);

        // expand tabs and get char position
        let (expanded_line, char_positions) =
            self.expand_tabs_and_get_positions(&visible_line.to_string());

        // check if there is some selection in the current line
        let selection_ranges = self.calculate_selection_ranges(line_idx, left, &char_positions);

        // check if there is a corresponding search in the current line
        let search_ranges = self.calculate_search_ranges(&expanded_line);

        // build the rendered line with highlight
        let rendered_line = self.build_rendered_line_with_highlights(
            &expanded_line,
            &selection_ranges,
            &search_ranges,
        );

        Ok(rendered_line)
    }

    fn expand_tabs_and_get_positions(&self, input: &str) -> (String, Vec<usize>) {
        let mut output = String::new();
        let mut positions = Vec::new();
        let mut width = 0;

        // iterate through gragphemes
        // never, ever, characters
        for grapheme in input.graphemes(true) {
            if grapheme == "\t" {
                let spaces_to_next_tab = TAB_WIDTH - (width % TAB_WIDTH);
                output.push_str(&" ".repeat(spaces_to_next_tab));
                for _ in 0..spaces_to_next_tab {
                    positions.push(width);
                    width += 1;
                }
            } else {
                output.push_str(grapheme);
                positions.push(width);
                width += UnicodeWidthStr::width(grapheme);
            }
        }

        (output, positions)
    }

    fn calculate_selection_ranges(
        &self,
        line_idx: usize,
        left: usize,
        char_positions: &[usize],
    ) -> Vec<(usize, usize)> {
        let mut ranges = Vec::new();

        if let Some((start, end)) = self.get_selection_range() {
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

                        if start_idx < end_idx {
                            let selection_start = start_idx.saturating_sub(left);
                            let selection_end = end_idx.saturating_sub(left).saturating_add(1);
                            ranges.push((selection_start, selection_end));
                        }
                    }
                    Some(SelectionMode::VisualLine) => {
                        let line_length = self.get_expanded_line_length(line_idx);
                        ranges.push((0, line_length));
                    }
                    None => {}
                }
            }
        }

        ranges
    }

    fn calculate_search_ranges(&self, expanded_line: &str) -> Vec<(usize, usize)> {
        let mut ranges = Vec::new();

        if let Some(query) = self.get_search_query() {
            if !query.is_empty() {
                let mut search_start = 0;
                while let Some(start_idx) = expanded_line[search_start..].find(query) {
                    let start = search_start + start_idx;
                    let end = start + query.len();
                    ranges.push((start, end));
                    search_start = end;
                }
            }
        }

        ranges
    }

    fn build_rendered_line_with_highlights(
        &self,
        line: &str,
        selection_ranges: &[(usize, usize)],
        search_ranges: &[(usize, usize)],
    ) -> String {
        let mut rendered_line = String::new();

        // convert line into a grapheme vector
        let graphemes: Vec<&str> = line.graphemes(true).collect();
        let total_graphemes = graphemes.len();

        let mut idx = 0;

        let mut highlight_ranges = Vec::new();

        // combine ranges of selection and search
        for &(start, end) in selection_ranges {
            highlight_ranges.push((start, end, "selection"));
        }

        for &(start, end) in search_ranges {
            if !selection_ranges
                .iter()
                .any(|&(sel_start, sel_end)| start < sel_end && end > sel_start)
            {
                highlight_ranges.push((start, end, "search"));
            }
        }

        // order ranges by the initial position
        highlight_ranges.sort_by_key(|&(start, _, _)| start);

        // apply highlight using grapheme index
        for &(start, end, highlight_type) in &highlight_ranges {
            let start = start.min(total_graphemes);
            let end = end.min(total_graphemes);

            if idx < start {
                // normal text before highlight
                let normal_text = graphemes[idx..start].concat();
                rendered_line.push_str(&normal_text);
            }

            // apply highlight
            let mut highlighted_text = graphemes[start..end].concat();

            if highlighted_text == "\n" {
                highlighted_text = " ".to_string(); // render empty char if we find '\n'
            }

            let color_scheme = ColorScheme::default();
            let styled_text = match highlight_type {
                "selection" => Terminal::styled_text(
                    &highlighted_text,
                    Some(color_scheme.selection_foreground),
                    Some(color_scheme.selection_background),
                    &[],
                ),
                "search" => Terminal::styled_text(
                    &highlighted_text,
                    Some(color_scheme.search_match_foreground),
                    Some(color_scheme.search_match_background),
                    &[],
                ),
                _ => highlighted_text.to_string(),
            };
            rendered_line.push_str(&styled_text);

            idx = end;
        }

        // resulting text after highlight
        if idx < total_graphemes {
            let remaining_text = graphemes[idx..].concat();
            rendered_line.push_str(&remaining_text);
        }

        rendered_line
    }

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
        current_row: usize,
        origin_row: usize,
        scroll_top: usize,
    ) -> usize {
        current_row
            .saturating_sub(origin_row)
            .saturating_add(scroll_top)
    }

    //
    // Scrolling
    //

    fn scroll_vertically(&mut self, to: usize) {
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

    fn scroll_horizontally(&mut self, to: usize) {
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
        let max_row = self.buffer.height().saturating_sub(1);
        let clamped_row = row.min(max_row);

        self.scroll_vertically(clamped_row);
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

    pub fn select_text_object(&mut self, text_object: TextObject) {
        match text_object {
            TextObject::Inner(delimiter) => {
                if let Some((start_idx, end_idx)) = self.find_text_object_range(delimiter, true) {
                    let start_location = self.buffer.char_index_to_location(start_idx);
                    let end_location = self.buffer.char_index_to_location(end_idx - 1); // have to not account for the cursor itself

                    // configure beginning and end of the selection
                    self.selection_start = Some(start_location);
                    self.selection_end = Some(end_location);
                    self.selection_mode = Some(SelectionMode::Visual);

                    // move cursor to the end of the selection
                    self.movement.text_location = end_location;
                    self.scroll_text_location_into_view();
                    self.set_needs_redraw(true);
                }
            } // TODO: more cases
        }
    }

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

    //
    // Indentation
    //

    fn get_indentation_of_line(&self, line_index: usize) -> String {
        if line_index >= self.buffer.rope.len_lines() {
            return String::new();
        }

        let line_slice = self.buffer.rope.line(line_index);
        let line_str = line_slice.to_string();

        let mut indentation = String::new();

        for grapheme in line_str.graphemes(true) {
            if grapheme == " " || grapheme == "\t" {
                indentation.push_str(grapheme);
            } else {
                break;
            }
        }

        indentation
    }

    fn decrease_indentation(&mut self, line_index: usize) {
        if line_index >= self.buffer.rope.len_lines() {
            return;
        }

        let line_slice = self.buffer.rope.line(line_index);
        let line_str = line_slice.to_string();

        // find the size of current indentation
        let mut indentation_end = 0;
        for (i, grapheme) in line_str.graphemes(true).enumerate() {
            if grapheme == " " || grapheme == "\t" {
                indentation_end = i + 1;
            } else {
                break;
            }
        }

        // if there is, remove a level
        if indentation_end > 0 {
            // determine the size of a level of indentation
            let indent_level = if self.indentation_uses_tabs() { 1 } else { 4 }; // assuming 4 spaces if we're not using tabs

            let new_indentation_end = indentation_end.saturating_sub(indent_level);

            // remove diff of indentation
            let line_start = self.buffer.rope.line_to_char(line_index);
            let remove_start = line_start + new_indentation_end;
            let remove_end = line_start + indentation_end;

            if remove_end > remove_start {
                self.buffer.rope.remove(remove_start..remove_end);
                self.buffer.dirty = true;

                // adjust cursor position
                self.movement.text_location.grapheme_index = self
                    .movement
                    .text_location
                    .grapheme_index
                    .saturating_sub(remove_end - remove_start);
            }
        }
    }

    fn indentation_uses_tabs(&self) -> bool {
        // TODO: editor configuration for tabs or spaces
        true // assuming we're using tabs (like the lord wanted us to)
    }

    pub fn handle_visual_movement(&mut self, command: Normal) {
        self.handle_normal_command(command);
        self.update_selection();
    }

    pub fn handle_visual_line_movement(&mut self, command: Normal) {
        self.handle_normal_command(command);
        self.update_selection(); // update selection to include the new line
        self.scroll_text_location_into_view();
    }

    //
    // Helpers
    //

    fn is_cursor_between_matching_delimiters(&self) -> Option<bool> {
        let line_index = self.movement.text_location.line_index;
        let grapheme_index = self.movement.text_location.grapheme_index;

        if grapheme_index == 0 || line_index >= self.buffer.rope.len_lines() {
            return None;
        }

        let line_slice = self.buffer.rope.line(line_index);
        let line_len = line_slice.len_chars();

        if grapheme_index > line_len {
            return None;
        }

        let before_char = line_slice.char(grapheme_index - 1);
        let after_char = if grapheme_index < line_len {
            line_slice.char(grapheme_index)
        } else {
            '\0'
        };

        let matching = match before_char {
            '(' => ')',
            '{' => '}',
            '[' => ']',
            '<' => '>',
            '"' => '"',
            '\'' => '\'',
            _ => '\0',
        };

        if matching != '\0' && matching == after_char {
            Some(true)
        } else {
            None
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
        self.rendered_lines.clear();
    }

    fn draw(&mut self, origin_row: usize) -> Result<(), Error> {
        let Size { height, width } = self.size;
        let end_y = origin_row.saturating_add(height);
        let scroll_top = self.scroll_offset.row;
        let top_third = height.div_ceil(3);

        // prepare a new buffer with rendered lines
        let mut new_rendered_lines = Vec::with_capacity(height);

        for current_row in origin_row..end_y {
            let line_idx = self.calculate_line_index(current_row, origin_row, scroll_top);

            let rendered_line = if line_idx < self.buffer.height() {
                self.get_rendered_line(line_idx, width)?
            } else if current_row == top_third && self.buffer.is_empty() {
                Self::build_welcome_message(width)
            } else {
                "~".to_string()
            };

            new_rendered_lines.push(rendered_line);
        }

        // compare old buffer with the new one and apply diffing
        for (i, line) in new_rendered_lines.iter().enumerate() {
            if self.rendered_lines.get(i) != Some(line) {
                // move cursor to the correct line
                Terminal::move_cursor_to(Position {
                    row: origin_row + i,
                    col: 0,
                })?;
                // clean current line
                Terminal::clear_line()?;
                // print new line
                Terminal::print(line)?;
            }
        }

        // update the buffer with the rendered lines
        self.rendered_lines = new_rendered_lines;

        Ok(())
    }
}
