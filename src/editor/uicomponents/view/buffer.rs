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

            if let Some(grapheme_idx) = line.search_forward(query, from_grapheme_idx) {
                return Some(Location {
                    grapheme_index: grapheme_idx,
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
                // No fim da linha, remover o caractere de nova linha para mesclar com a próxima linha
                self.rope.remove(char_idx..char_idx + 1);
                self.dirty = true;
            } else {
                // Remover o caractere no índice atual
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
}
