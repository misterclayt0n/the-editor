use std::{cmp::min, io::Error};

use crate::prelude::*;
use super::super::{command::Edit, Terminal};
use super::UIComponent;

#[derive(Default)]
pub struct CommandBar {
    prompt: String,
    value: String,
    needs_redraw: bool,
    size: Size,
}

impl CommandBar {
    pub fn handle_edit_command(&mut self, command: Edit) {
        match command {
            Edit::Insert(character) => self.value.push(character),
            Edit::Delete | Edit::InsertNewline => {}
            Edit::DeleteBackward => {
                self.value.pop();
            }
        }

        self.set_needs_redraw(true);
    }

    pub fn cursor_position_col(&self) -> ColIndex {
        let max_width = self
            .prompt
            .len()
            .saturating_add(self.value.len());
        min(max_width, self.size.width)
    }

    pub fn value(&self) -> String {
        self.value.to_string()
    }

    pub fn set_prompt(&mut self, prompt: &str) {
        self.prompt = prompt.to_string();
        self.set_needs_redraw(true);
    }

    pub fn clear_value(&mut self) {
        self.value.clear();
        self.set_needs_redraw(true);
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
    fn draw(&mut self, origin: RowIndex) -> Result<(), Error> {
        let area_for_value = self.size.width.saturating_sub(self.prompt.len()); // this is how much space there is between the right side of the prompt and the edge of the bar

        let value_end = self.value.len(); // we always want to show the left part of the value, therefore the end of the visible range we try to access will be equal to the full width
        let value_start = value_end.saturating_sub(area_for_value); // this should give us the start for the grapheme subrange we want to print out.

        let visible_line: String = self.value.chars().skip(value_start).take(area_for_value).collect();

        let message = format!("{}{}", self.prompt, visible_line);

        let to_print = if message.len() <= self.size.width {
            message
        } else {
            String::new()
        };

        Terminal::print_row(origin, &to_print)
    }
}
