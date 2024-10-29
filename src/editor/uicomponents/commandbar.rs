use std::io::Error;
use unicode_segmentation::UnicodeSegmentation;

use super::super::Terminal;
use super::UIComponent;
use crate::editor::Edit;
use crate::prelude::*;

#[derive(Default, Clone)]
pub struct CommandBar {
    prompt: String,
    value: String,
    needs_redraw: bool,
    size: Size,
    pub cursor_position: usize,
}

#[derive(PartialEq)]
enum CharClass {
    Whitespace,
    Word,
    Punctuation,
}

impl CommandBar {
    pub fn handle_edit_command(&mut self, command: Edit) {
        match command {
            Edit::Insert(character) => {
                // collect graphemes into a vector of owned Strings
                let mut graphemes: Vec<String> =
                    self.value.graphemes(true).map(|g| g.to_string()).collect();
                // insert the new character as an owned String
                graphemes.insert(self.cursor_position, character.to_string());
                // reconstruct the value from the graphemes
                self.value = graphemes.concat();
                self.cursor_position += 1;
            }
            Edit::Delete => {
                let mut graphemes: Vec<String> =
                    self.value.graphemes(true).map(|g| g.to_string()).collect();
                if self.cursor_position < graphemes.len() {
                    graphemes.remove(self.cursor_position);
                    self.value = graphemes.concat();
                }
            }
            Edit::DeleteBackward => {
                let mut graphemes: Vec<String> =
                    self.value.graphemes(true).map(|g| g.to_string()).collect();
                if self.cursor_position > 0 {
                    self.cursor_position -= 1;
                    graphemes.remove(self.cursor_position);
                    self.value = graphemes.concat();
                }
            }
            _ => {}
        }

        self.set_needs_redraw(true);
    }

    pub fn cursor_position_col(&self) -> usize {
        let scroll = self.scroll_offset();
        self.prompt.graphemes(true).count() + self.cursor_position - scroll
    }

    pub fn value(&self) -> String {
        self.value.clone()
    }

    pub fn set_value(&mut self, value: String) {
        self.value = value;
        self.cursor_position = self.value.graphemes(true).count();
        self.set_needs_redraw(true);
    }

    pub fn set_cursor_position(&mut self, pos: usize) {
        self.cursor_position = pos.min(self.value.graphemes(true).count());
        self.set_needs_redraw(true);
    }

    pub fn get_cursor_position(&self) -> usize {
        self.cursor_position
    }

    pub fn set_prompt(&mut self, prompt: &str) {
        self.prompt = prompt.to_string();
        self.set_needs_redraw(true);
    }

    pub fn clear_value(&mut self) {
        self.value.clear();
        self.cursor_position = 0;
        self.set_needs_redraw(true);
    }

    pub fn move_cursor_left(&mut self) {
        if self.cursor_position > 0 {
            self.cursor_position -= 1;
            self.needs_redraw = true;
        }
    }

    pub fn move_cursor_right(&mut self) {
        let grapheme_count = self.value.graphemes(true).count();
        if self.cursor_position < grapheme_count {
            self.cursor_position += 1;
            self.needs_redraw = true;
        }
    }

    pub fn move_cursor_start(&mut self) {
        self.cursor_position = 0;
        self.needs_redraw = true;
    }

    pub fn move_cursor_end(&mut self) {
        self.cursor_position = self.value.graphemes(true).count();
        self.needs_redraw = true;
    }

    fn scroll_offset(&self) -> usize {
        let available_width = self
            .size
            .width
            .saturating_sub(self.prompt.graphemes(true).count());
        if self.cursor_position >= available_width {
            self.cursor_position - available_width + 1
        } else {
            0
        }
    }

    pub fn move_cursor_word_forward(&mut self, word_type: WordType) {
        let graphemes: Vec<&str> = self.value.graphemes(true).collect();
        let total_graphemes = graphemes.len();

        if self.cursor_position >= total_graphemes {
            return;
        }

        let mut index = self.cursor_position;

        fn is_word_char(c: char) -> bool {
            c.is_alphanumeric() || c == '_'
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
                        CharClass::Word
                    }
                }
            }
        }

        let current_class =
            get_char_class(graphemes[index].chars().next().unwrap_or(' '), word_type);

        while index < total_graphemes {
            let c = graphemes[index].chars().next().unwrap_or(' ');
            let class = get_char_class(c, word_type);
            if class == current_class {
                index += 1;
            } else {
                break;
            }
        }

        while index < total_graphemes {
            let c = graphemes[index].chars().next().unwrap_or(' ');
            if c.is_whitespace() {
                index += 1;
            } else {
                break;
            }
        }

        self.cursor_position = index;
        self.needs_redraw = true;
    }

    pub fn move_cursor_word_backward(&mut self, word_type: WordType) {
        let graphemes: Vec<&str> = self.value.graphemes(true).collect();

        if self.cursor_position == 0 {
            return;
        }

        let mut index = self.cursor_position - 1;

        fn is_word_char(c: char) -> bool {
            c.is_alphanumeric() || c == '_'
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
                        CharClass::Word
                    }
                }
            }
        }

        while index > 0 {
            let c = graphemes[index].chars().next().unwrap_or(' ');
            if c.is_whitespace() {
                index = index.saturating_sub(1);
            } else {
                break;
            }
        }

        let current_class =
            get_char_class(graphemes[index].chars().next().unwrap_or(' '), word_type);

        while index > 0 {
            let c = graphemes[index - 1].chars().next().unwrap_or(' ');
            let class = get_char_class(c, word_type);
            if class == current_class {
                index -= 1;
            } else {
                break;
            }
        }

        self.cursor_position = index;
        self.needs_redraw = true;
    }

    pub fn move_cursor_word_end_forward(&mut self, word_type: WordType) {
        let graphemes: Vec<&str> = self.value.graphemes(true).collect();
        let total_graphemes = graphemes.len();

        if self.cursor_position >= total_graphemes {
            return;
        }

        let mut index = self.cursor_position;

        fn is_word_char(c: char) -> bool {
            c.is_alphanumeric() || c == '_'
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
                        CharClass::Word
                    }
                }
            }
        }

        while index < total_graphemes {
            let c = graphemes[index].chars().next().unwrap_or(' ');
            if c.is_whitespace() {
                index += 1;
            } else {
                break;
            }
        }

        if index >= total_graphemes {
            return;
        }

        let current_class =
            get_char_class(graphemes[index].chars().next().unwrap_or(' '), word_type);

        while index + 1 < total_graphemes {
            let c = graphemes[index + 1].chars().next().unwrap_or(' ');
            let class = get_char_class(c, word_type);
            if class == current_class {
                index += 1;
            } else {
                break;
            }
        }

        self.cursor_position = index + 1;
        self.needs_redraw = true;
    }
}

impl UIComponent for CommandBar {
    fn set_needs_redraw(&mut self, value: bool) {
        self.needs_redraw = value;
    }

    fn needs_redraw(&self) -> bool {
        self.needs_redraw
    }

    fn set_size(&mut self, size: Size) {
        self.size = size;
    }

    fn draw(&mut self, origin: Position) -> Result<(), Error> {
        let available_width = self
            .size
            .width
            .saturating_sub(self.prompt.graphemes(true).count());
        let scroll = self.scroll_offset();

        let visible_value: String = self
            .value
            .graphemes(true)
            .skip(scroll)
            .take(available_width)
            .collect();

        let message = format!("{}{}", self.prompt, visible_value);

        Terminal::print_row(origin.row, &message)
    }
}
