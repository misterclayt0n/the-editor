use std::cmp::min;

use crate::{editor::line::Line, prelude::*};

use super::buffer::Buffer;

#[derive(Default)]
pub struct Movement {
    pub text_location: Location,
}

impl Movement {
    pub fn move_up(&mut self, buffer: &Buffer, step: usize) {
        self.text_location.line_index = self.text_location.line_index.saturating_sub(step);
        self.snap_to_valid_grapheme(buffer);
    }

    pub fn move_down(&mut self, buffer: &Buffer, step: usize) {
        self.text_location.line_index = self.text_location.line_index.saturating_add(step);
        self.snap_to_valid_line(buffer);
        self.snap_to_valid_grapheme(buffer);
    }

    pub fn move_left(&mut self, buffer: &Buffer) {
        if self.text_location.grapheme_index > 0 {
            self.text_location.grapheme_index -= 1;
        } else if self.text_location.line_index > 0 {
            self.move_up(buffer, 1);
            self.move_to_end_of_line(buffer);
        }
    }

    pub fn move_right(&mut self, buffer: &Buffer) {
        let line_width = if self.text_location.line_index < buffer.rope.len_lines() {
            let line_slice = buffer.rope.line(self.text_location.line_index);
            let line_str = line_slice.to_string();
            let line = Line::from(&line_str);
            line.grapheme_count()
        } else {
            0
        };
        if self.text_location.grapheme_index < line_width {
            self.text_location.grapheme_index += 1;
        }
    }

    pub fn snap_to_valid_grapheme(&mut self, buffer: &Buffer) {
        self.text_location.line_index = min(
            self.text_location.line_index,
            buffer.height().saturating_sub(1),
        )
    }

    fn snap_to_valid_line(&mut self, buffer: &Buffer) {
        self.text_location.line_index = min(
            self.text_location.line_index,
            buffer.height().saturating_sub(1),
        );
    }

    pub fn move_to_start_of_line(&mut self) {
        self.text_location.grapheme_index = 0;
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
    }

    pub fn move_word_forward(&mut self, buffer: &Buffer, word_type: WordType) {
        if let Some(new_location) = buffer.find_next_word_start(self.text_location, word_type) {
            self.text_location = new_location;
        } else {
            // move to the end of the buffer
            self.text_location = buffer.get_end_location();
        }
    }

    pub fn move_word_backward(&mut self, buffer: &Buffer, word_type: WordType) {
        if let Some(new_location) = buffer.find_previous_word_start(self.text_location, word_type) {
            self.text_location = new_location;
        } else {
            // move to start of buffer
            self.text_location = Location {
                line_index: 0,
                grapheme_index: 0,
            };
        }
    }
}
