use std::cmp::min;

use crate::{editor::line::Line, prelude::*};

use super::buffer::Buffer;

pub struct Movement {
    pub text_location: Location,
    desired_col: usize,
}

impl Movement {
    pub fn move_up(&mut self, buffer: &Buffer, n: usize) {
        self.text_location.line_index = self.text_location.line_index.saturating_sub(n);
        self.adjust_grapheme_index(buffer);
    }

    pub fn move_down(&mut self, buffer: &Buffer, n: usize) {
        self.text_location.line_index = min(
            self.text_location.line_index + n,
            buffer.height().saturating_sub(1),
        );
        self.adjust_grapheme_index(buffer);
    }

    pub fn move_left(&mut self, buffer: &Buffer) {
        if self.text_location.grapheme_index > 0 {
            self.text_location.grapheme_index -= 1;
        } else if self.text_location.line_index > 0 {
            self.move_up(buffer, 1);
            self.move_to_end_of_line(buffer);
        }
        self.update_desired_col(buffer);
    }

    pub fn move_right(&mut self, buffer: &Buffer) {
        let line_width = buffer.get_line_length(self.text_location.line_index);
        if self.text_location.grapheme_index < line_width.saturating_sub(1) {
            self.text_location.grapheme_index += 1;
        }
        self.update_desired_col(buffer);
    }

    pub fn move_to_start_of_line(&mut self) {
        self.text_location.grapheme_index = 0;
        self.desired_col = 0;
    }

    pub fn move_to_first_non_whitespace(&mut self, buffer: &Buffer) {
        if self.text_location.line_index < buffer.rope.len_lines() {
            let line_slice = buffer.rope.line(self.text_location.line_index);
            let line_str = line_slice.to_string();

            // find first non whitespace char
            for (i, c) in line_str.chars().enumerate() {
                if !c.is_whitespace() {
                    self.text_location.grapheme_index = i;
                    break;
                }
            }
        } else {
            self.text_location.grapheme_index = 0;
        }
        self.update_desired_col(buffer);
    }

    pub fn move_to_end_of_line(&mut self, buffer: &Buffer) {
        self.text_location.grapheme_index =
            if self.text_location.line_index < buffer.rope.len_lines() {
                let line_slice = buffer.rope.line(self.text_location.line_index);
                let line_str = line_slice.to_string().trim_end_matches('\n').to_string();
                let line = Line::from(&line_str);
                line.grapheme_count()
            } else {
                0
            };
        self.update_desired_col(buffer);
    }

    pub fn move_word_forward(&mut self, buffer: &Buffer, word_type: WordType) {
        if let Some(new_location) = buffer.find_next_word_start(self.text_location, word_type) {
            self.text_location = new_location;
        } else {
            // move to the end of the buffer
            self.text_location = buffer.get_end_location();
        }
        self.update_desired_col(buffer);
    }

    pub fn move_word_backward(&mut self, buffer: &Buffer, word_type: WordType) {
        if let Some(new_location) = buffer.find_previous_word_start(self.text_location, word_type) {
            self.text_location = new_location;
        } else {
            self.text_location = Location {
                line_index: 0,
                grapheme_index: 0,
            };
        }
        self.update_desired_col(buffer);
    }

    pub fn move_to_top(&mut self) {
        self.text_location.line_index = 0;
        self.text_location.grapheme_index = 0;
    }

    pub fn move_to_bottom(&mut self, buffer: &Buffer) {
        self.text_location = buffer.get_end_location();
    }

    //
    // Helper functions
    //

    pub fn update_desired_col(&mut self, buffer: &Buffer) {
        self.desired_col = buffer.text_location_to_col(self.text_location);
    }

    fn adjust_grapheme_index(&mut self, buffer: &Buffer) {
        let new_line_index = self.text_location.line_index;
        self.text_location.grapheme_index =
            buffer.col_to_grapheme_index(new_line_index, self.desired_col);
    }
}

impl Default for Movement {
    fn default() -> Self {
        Self {
            text_location: Location {
                line_index: 0,
                grapheme_index: 0,
            },
            desired_col: 0,
        }
    }
}
