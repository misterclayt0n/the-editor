use ropey::Rope;
use unicode_width::UnicodeWidthChar;
use unicode_segmentation::UnicodeSegmentation;

use super::FileInfo;
use crate::prelude::*;
use std::cmp::min;
use std::fs::File;
use std::io::Error;
use std::io::Write;

pub struct Buffer {
    pub rope: Rope,
    pub file_info: FileInfo,
    pub dirty: bool,
    phantom_line: bool,
}

impl Default for Buffer {
    fn default() -> Self {
        let mut rope = Rope::new();
        rope.insert(rope.len_chars(), "\n");

        Self {
            rope,
            file_info: FileInfo::default(),
            dirty: false,
            phantom_line: true,
        }
    }
}

#[derive(PartialEq)]
enum CharClass {
    Whitespace,
    Word,
    Punctuation,
}

fn get_char_class(c: char, word_type: WordType) -> CharClass {
    match word_type {
        WordType::Word => {
            if c.is_whitespace() {
                CharClass::Whitespace
            } else if is_word_char(c) {
                CharClass::Word
            } else {
                CharClass::Punctuation
            }
        }
        WordType::BigWord => {
            if c.is_whitespace() {
                CharClass::Whitespace
            } else {
                CharClass::Word // eveyrthing that is not a space is considered part of the word
            }
        }
    }
}

impl Buffer {
    pub fn load(file_name: &str) -> Result<Self, Error> {
        let rope = Rope::from_reader(File::open(file_name)?)?;

        let phantom_line = if rope.len_chars() == 0 {
            false
        } else {
            // check if last line ends with a new line
            let last_char = rope.char(rope.len_chars() - 1);
            if last_char != '\n' {
                // if it doesn't, add phantom line
                let mut new_rope = rope.clone();
                new_rope.insert(new_rope.len_chars(), "\n");
                true
            } else {
                // already ends with '\n'
                false
            }
        };

        let mut rope_to_use = rope;
        if phantom_line {
            rope_to_use.insert(rope_to_use.len_chars(), "\n");
        }

        Ok(Self {
            rope: rope_to_use,
            file_info: FileInfo::from(file_name),
            dirty: false,
            phantom_line: true,
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
                min(from.grapheme_index, line_slice.len_chars())
            } else {
                0
            };

            let line_str = line_slice.to_string();

            let start_byte_index = line_str
                .grapheme_indices(true)
                .nth(from_grapheme_idx)
                .map(|(byte_idx, _)| byte_idx)
                .unwrap_or(line_str.len());

            if start_byte_index > line_str.len() {
                continue;
            }

            // search from byte index
            if let Some(match_byte_index_rel) = line_str[start_byte_index..].find(query) {
                let match_byte_index = start_byte_index + match_byte_index_rel;

                let mut grapheme_index = 0;
                let mut found_grapheme_index = None;
                for (byte_idx, _) in line_str.grapheme_indices(true) {
                    if byte_idx >= match_byte_index {
                        found_grapheme_index = Some(grapheme_index);
                        break;
                    }
                    grapheme_index += 1;
                }
                if found_grapheme_index.is_none() {
                    found_grapheme_index = Some(grapheme_index);
                }

                return Some(Location {
                    grapheme_index: found_grapheme_index.unwrap_or(0),
                    line_index: line_idx,
                });
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
                min(from.grapheme_index, line_slice.len_chars())
            } else {
                line_slice.len_chars()
            };

            let line_str = line_slice.to_string();

            if from_grapheme_idx > line_str.len() {
                continue;
            }

            if let Some(char_idx) = line_str[..from_grapheme_idx].rfind(query) {
                return Some(Location {
                    grapheme_index: char_idx,
                    line_index: line_idx,
                });
            }
        }

        None
    }

    fn save_to_file(&self, file_info: &FileInfo) -> Result<(), Error> {
        if let Some(file_path) = &file_info.get_path() {
            let mut file = File::create(file_path)?;
            let total_lines = self.rope.len_lines();

            // determine how many lines should be saved
            let lines_to_save = if self.phantom_line {
                if total_lines == 0 {
                    0
                } else {
                    total_lines - 1
                }
            } else {
                total_lines
            };

            for line_idx in 0..lines_to_save {
                let line = self.rope.line(line_idx);
                write!(file, "{}", line)?;
            }

            // check if the original file ended with a new line, 
            // and if necessary, add one for the phantom line
            if !self.phantom_line && self.rope.char(self.rope.len_chars() - 1) != '\n' {
                write!(file, "\n")?;
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
        self.rope.len_chars() == 1
    }

    pub const fn is_file_loaded(&self) -> bool {
        self.file_info.has_path()
    }

    pub fn height(&self) -> usize {
        self.rope.len_lines() - 1
    }

    pub fn insert_char(&mut self, character: char, at: Location) {
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

    pub fn delete_line(&mut self, line_index: usize) {
        if self.rope.len_lines() == 2 {
            // if it's the last line of the document, just clean the buffer so it's not empty
            let line_start = self.rope.line_to_char(line_index);
            let line_end = self.rope.line_to_char(line_index + 1);
            if line_end > line_start {
                self.rope.remove(line_start..line_end);
            }
        } else {
            // if there's more than one line, we can delete as usual
            let line_start = self.rope.line_to_char(line_index);
            let line_end = self.rope.line_to_char(line_index + 1);
            self.rope.remove(line_start..line_end);
        }
        self.dirty = true;

        if self.rope.len_chars() == 0 {
            self.rope.insert(0, "\n");
            self.dirty = true;
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
        let mut char_index = self.location_to_char_index(location);

        if char_index >= total_chars {
            return None;
        }

        let c = self.rope.char(char_index);

        // determine the character class
        let current_class = get_char_class(c, word_type);

        // skip over characters of the same class
        while char_index < total_chars {
            let c = self.rope.char(char_index);
            let class = get_char_class(c, word_type);

            if class == current_class {
                char_index += c.len_utf8();
            } else {
                break;
            }
        }

        // skip over any whitespace characters
        while char_index < total_chars {
            let c = self.rope.char(char_index);
            if get_char_class(c, word_type) == CharClass::Whitespace {
                char_index += c.len_utf8();
            } else {
                break;
            }
        }

        if char_index >= total_chars {
            return None;
        }

        Some(self.char_index_to_location(char_index))
    }

    pub fn find_previous_word_start(
        &self,
        location: Location,
        word_type: WordType,
    ) -> Option<Location> {
        let mut char_index = self.location_to_char_index(location);

        if char_index == 0 {
            return None;
        }

        // move the cursor one step back to start looking at the previous character
        char_index = char_index.saturating_sub(1);

        // skip any trailing whitespace
        while char_index > 0 {
            let c = self.rope.char(char_index);
            if get_char_class(c, word_type) == CharClass::Whitespace {
                char_index = char_index.saturating_sub(c.len_utf8());
            } else {
                break;
            }
        }

        if char_index == 0 {
            return Some(self.char_index_to_location(0));
        }

        // get the class of the character at the new position
        let current_class = get_char_class(self.rope.char(char_index), word_type);

        // skip all characters that are of the same class
        while char_index > 0 {
            let c = self.rope.char(char_index);
            if get_char_class(c, word_type) == current_class {
                char_index = char_index.saturating_sub(c.len_utf8());
            } else {
                // stop at the boundary between different character classes
                char_index += c.len_utf8();
                break;
            }
        }

        // skip any leading whitespace before the next word
        while char_index > 0 {
            let c = self.rope.char(char_index);
            if get_char_class(c, word_type) == CharClass::Whitespace {
                char_index = char_index.saturating_sub(c.len_utf8());
            } else {
                break;
            }
        }

        Some(self.char_index_to_location(char_index))
    }

    pub fn find_next_word_end(&self, location: Location, word_type: WordType) -> Option<Location> {
        let total_chars = self.rope.len_chars();
        let mut char_index = self.location_to_char_index(location);

        if char_index >= total_chars {
            return None;
        }

        // move forward one character if possible
        if char_index + 1 < total_chars {
            char_index += 1;
        } else {
            // We're at the end of the buffer
            return None;
        }

        // skip over whitespace
        while char_index < total_chars {
            let c = self.rope.char(char_index);
            if get_char_class(c, word_type) == CharClass::Whitespace {
                char_index += c.len_utf8();
            } else {
                break;
            }
        }

        if char_index >= total_chars {
            return None;
        }

        let current_class = get_char_class(self.rope.char(char_index), word_type);

        let mut last_char_index = char_index;

        // move to the end of the current class sequence
        while char_index < total_chars {
            let c = self.rope.char(char_index);
            if get_char_class(c, word_type) == current_class {
                last_char_index = char_index;
                char_index += c.len_utf8();
            } else {
                break;
            }
        }

        Some(self.char_index_to_location(last_char_index))
    }

    pub fn get_end_location(&self) -> Location {
        let last_line_index = self.height().saturating_sub(1);
        let grapheme_index = self.rope.line(last_line_index).len_chars();
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

    pub fn get_line_length(&self, line_index: usize) -> usize {
        if line_index < self.rope.len_lines() {
            let line_slice = self.rope.line(line_index);
            let len = line_slice.len_chars();

            if len > 0 && line_slice.char(len - 1) == '\n' {
                len - 1
            } else {
                len
            }
        } else {
            0
        }
    }

    //
    // Helper functions
    //

    pub fn text_location_to_col(&self, location: Location) -> usize {
        let line_slice = self.rope.line(location.line_index);
        let mut col = 0;

        for (i, c) in line_slice.chars().enumerate() {
            if i >= location.grapheme_index {
                break;
            }
            if c == '\t' {
                let spaces_to_next_tab = TAB_WIDTH - (col % TAB_WIDTH);
                col += spaces_to_next_tab;
            } else {
                col += c.width().unwrap_or(1);
            }
        }

        col
    }

    pub fn col_to_grapheme_index(&self, line_index: usize, col: usize) -> usize {
        if line_index >= self.rope.len_lines() {
            return 0;
        }
        let line_slice = self.rope.line(line_index);
        let mut current_col = 0;
        let mut last_index = 0;

        for (i, c) in line_slice.chars().enumerate() {
            let char_width = if c == '\t' {
                let spaces_to_next_tab = TAB_WIDTH - (current_col % TAB_WIDTH);
                spaces_to_next_tab
            } else {
                c.width().unwrap_or(1)
            };

            if current_col + char_width > col {
                // if we are no longer close to next char, return next index
                return if col - current_col < (current_col + char_width) - col {
                    i
                } else {
                    i + 1
                };
            }
            current_col += char_width;
            last_index = i;
        }

        // if we got here, just return the last valid index
        last_index + 1
    }
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}
