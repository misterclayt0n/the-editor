use ropey::Rope;

use super::FileInfo;
use super::Line;
use crate::prelude::*;
use std::fs::File;
use std::io::Error;
use std::io::Write;

#[derive(Default)]
pub struct Buffer {
    pub rope: Rope,
    pub file_info: FileInfo,
    pub dirty: bool,
}

impl Buffer {
    pub fn load(file_name: &str) -> Result<Self, Error> {
        let rope = Rope::from_reader(File::open(file_name)?)?;

        Ok(Self {
            rope,
            file_info: FileInfo::from(file_name),
            dirty: false,
        })
    }

    pub fn search_forward(&self, query: &str, from: Location) -> Option<Location> {
        if query.is_empty() {
            return None;
        }

        let mut is_first = true;
        let total_lines = self.rope.len_lines();

        let line_indices = (0..total_lines)
            .cycle()
            .skip(from.line_index)
            .take(total_lines + 1);

        for line_idx in line_indices {
            let line_slice = self.rope.line(line_idx);

            let from_grapheme_idx = if is_first {
                is_first = false;
                from.grapheme_index
            } else {
                0
            };

            let line_str = line_slice.to_string();
            let line = Line::from(&line_str);

            if from_grapheme_idx < line.grapheme_count() {
                if let Some(grapheme_idx) = line.search_forward(query, from_grapheme_idx) {
                    return Some(Location {
                        grapheme_index: grapheme_idx,
                        line_index: line_idx,
                    });
                }
            }
        }

        None
    }

    pub fn search_backward(&self, query: &str, from: Location) -> Option<Location> {
        if query.is_empty() {
            return None;
        }

        let mut is_first = true;
        let total_lines = self.rope.len_lines();

        let line_indices = (0..total_lines)
            .rev()
            .cycle()
            .skip(total_lines - from.line_index - 1)
            .take(total_lines + 1);

        for line_idx in line_indices {
            let line_slice = self.rope.line(line_idx);

            let from_grapheme_idx = if is_first {
                is_first = false;
                from.grapheme_index
            } else {
                let line_str = line_slice.to_string();
                let line = Line::from(&line_str);
                line.grapheme_count()
            };

            let line_str = line_slice.to_string();
            let line = Line::from(&line_str);

            if let Some(grapheme_idx) = line.search_backward(query, from_grapheme_idx) {
                return Some(Location {
                    grapheme_index: grapheme_idx,
                    line_index: line_idx,
                });
            }
        }
        None
    }

    fn save_to_file(&self, file_info: &FileInfo) -> Result<(), Error> {
        if let Some(file_path) = &file_info.get_path() {
            let mut file = File::create(file_path)?;

            for line_slice in self.rope.lines() {
                write!(file, "{}", line_slice)?;
            }
        } else {
            #[cfg(debug_assertions)]
            {
                panic!("Attempting to save with no file path present");
            }
        }

        Ok(())
    }

    pub fn save_as(&mut self, file_name: &str) -> Result<(), Error> {
        let file_info = FileInfo::from(file_name);
        self.save_to_file(&file_info)?;
        self.file_info = file_info;
        self.dirty = false;
        Ok(())
    }

    pub fn save(&mut self) -> Result<(), Error> {
        self.save_to_file(&self.file_info)?;
        self.dirty = false;
        Ok(())
    }

    pub fn is_empty(&self) -> bool {
        self.rope.len_chars() == 0
    }

    pub const fn is_file_loaded(&self) -> bool {
        self.file_info.has_path()
    }

    pub fn height(&self) -> usize {
        self.rope.len_lines()
    }

    pub fn insert_char(&mut self, character: char, at: Location) {
        debug_assert!(at.line_index <= self.height());

        let char_idx = self.rope.line_to_char(at.line_index) + at.grapheme_index;
        self.rope.insert(char_idx, &character.to_string());
        self.dirty = true;
    }

    pub fn delete(&mut self, at: Location) {
        let line_start = self.rope.line_to_char(at.line_index);
        let line_slice = self.rope.line(at.line_index);
        let line_len = line_slice.len_chars();
        let char_idx = line_start + at.grapheme_index;

        if char_idx < self.rope.len_chars() {
            if at.grapheme_index >= line_len && at.line_index + 1 < self.rope.len_lines() {
                self.rope.remove(char_idx..char_idx + 1);
                self.dirty = true;
            } else {
                self.rope.remove(char_idx..char_idx + 1);
                self.dirty = true;
            }
        }
    }

    pub fn insert_newline(&mut self, at: Location) {
        let char_idx = self.rope.line_to_char(at.line_index) + at.grapheme_index;
        self.rope.insert(char_idx, "\n");
        self.dirty = true;
    }

    pub fn find_next_word_start(
        &self,
        location: Location,
        word_type: WordType,
    ) -> Option<Location> {
        let total_chars = self.rope.len_chars();
        let start_char_index = self.location_to_char_index(location);

        if start_char_index >= total_chars {
            return None;
        }

        let mut char_index = start_char_index;
        let mut in_word = false;
        let mut first_char = true;

        let mut chars = self.rope.chars_at(char_index).peekable();

        while let Some(c) = chars.next() {
            let is_word_char = match word_type {
                WordType::Word => !c.is_whitespace() && !is_w_delimiter(c),
                WordType::BigWord => !c.is_whitespace(),
            };

            if is_w_delimiter(c) && !first_char && word_type == WordType::Word {
                return Some(self.char_index_to_location(char_index));
            }

            if is_word_char {
                if !in_word && !first_char {
                    return Some(self.char_index_to_location(char_index));
                }
                in_word = true;
            } else {
                in_word = false;
            }

            first_char = false;
            char_index += 1;

            if char_index >= total_chars {
                break;
            }
        }

        None
    }

    pub fn find_previous_word_start(
        &self,
        location: Location,
        word_type: WordType,
    ) -> Option<Location> {
        let start_char_index = self.location_to_char_index(location);

        if start_char_index == 0 {
            return None;
        }

        let mut char_index = start_char_index.saturating_sub(1);
        let mut in_word = false;
        let mut first_char = true;

        loop {
            let c = self.rope.char(char_index);
            let is_word_char = match word_type {
                WordType::Word => !c.is_whitespace() && !is_b_delimiter(c),
                WordType::BigWord => !c.is_whitespace(),
            };

            if is_b_delimiter(c) && !first_char {
                return Some(self.char_index_to_location(char_index));
            }

            if is_word_char {
                in_word = true;
            } else if in_word {
                return Some(self.char_index_to_location(char_index + 1));
            }

            first_char = false;

            if char_index == 0 {
                break;
            }

            char_index = char_index.saturating_sub(1);
        }

        if in_word {
            return Some(self.char_index_to_location(0));
        }

        None
    }

    pub fn get_end_location(&self) -> Location {
        let last_line_index = self.rope.len_lines().saturating_sub(1);
        let last_line = self.rope.line(last_line_index);
        let grapheme_index = last_line.len_chars();
        Location {
            line_index: last_line_index,
            grapheme_index,
        }
    }

    //
    // Conversion methods
    //

    pub fn location_to_char_index(&self, location: Location) -> usize {
        let line_start = self.rope.line_to_char(location.line_index);
        line_start + location.grapheme_index
    }

    pub fn char_index_to_location(&self, char_index: usize) -> Location {
        let line_index = self.rope.char_to_line(char_index);
        let line_start_idx = self.rope.line_to_char(line_index);
        let char_in_line = char_index - line_start_idx;

        Location {
            line_index,
            grapheme_index: char_in_line,
        }
    }
}

fn is_b_delimiter(c: char) -> bool {
    c == '{' || c == '}' || c == ';' || c == ',' || c == '(' || c == ')' || c == '\\'
}

fn is_w_delimiter(c: char) -> bool {
    c == '(' || c == ')' || c == '{' || c == '}' || c == '\\'
}
