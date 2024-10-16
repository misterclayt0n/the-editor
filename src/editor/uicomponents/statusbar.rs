use std::io::Error;

use crate::editor::ModeType;

use super::super::{DocumentStatus, Size, Terminal};
use super::UIComponent;

#[derive(Default)]
pub struct StatusBar {
    current_status: DocumentStatus,
    needs_redraw: bool,
    size: Size,
    mode: ModeType,
}

impl StatusBar {
    pub fn update_status(&mut self, new_status: DocumentStatus, mode: ModeType) {
        if new_status != self.current_status || self.mode != mode {
            self.current_status = new_status;
            self.mode = mode;
            self.set_needs_redraw(true);
        }
    }

    pub fn set_mode(&mut self, mode: ModeType) {
        if self.mode != mode {
            self.mode = mode;
            self.set_needs_redraw(true);
        }
    }

    pub fn clear_mode(&mut self) {
        self.set_mode(ModeType::Normal);
    }
}

impl UIComponent for StatusBar {
    fn set_needs_redraw(&mut self, value: bool) {
        self.needs_redraw = value;
    }

    fn needs_redraw(&self) -> bool {
        self.needs_redraw
    }

    fn set_size(&mut self, size: Size) {
        self.size = size;
    }

    fn draw(&mut self, origin_row: usize) -> Result<(), Error> {
        // assemble the first part of the status bar
        let line_count = self.current_status.line_count_to_string();
        let modified_indicator = self.current_status.modified_indicator_to_string();
        let vim_mode_display = format!("{}", self.mode); // show the mode

        let beginning = format!(
            "{} - {line_count} -- {vim_mode_display} -- {modified_indicator}",
            self.current_status.file_name
        );

        // assemble the whole status bar, with the position indicator at the back
        let position_indicator = self.current_status.position_indicator_to_string();
        let remainder_len = self.size.width.saturating_sub(beginning.len());
        let status = format!("{beginning}{position_indicator:>remainder_len$}");

        // only print out the status if it fits. Otherwise write out an empty string to ensure the row is cleared.
        let to_print = if status.len() <= self.size.width {
            status
        } else {
            String::new()
        };
        Terminal::print_inverted_row(origin_row, &to_print)?;

        Ok(())
    }
}
