use crate::prelude::*;
use crossterm::cursor::{Hide, MoveTo, Show};

use crossterm::style::SetForegroundColor;
use crossterm::style::{
    Attribute::{Reset, Reverse},
    Print, ResetColor, SetBackgroundColor,
};

use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, size, Clear, ClearType, DisableLineWrap, EnableLineWrap,
    EnterAlternateScreen, LeaveAlternateScreen, SetTitle,
};

use crossterm::{queue, Command};
use ropey::RopeSlice;
use std::io::{stdout, Error, Write};

use super::color_scheme::ColorScheme;
use super::{Position, Size};

pub struct Terminal;

impl Terminal {
    pub fn kill() -> Result<(), Error> {
        Self::leave_alternate_screen()?;
        Self::enable_line_wrap()?;
        Self::show_cursor()?;
        Self::execute()?;
        disable_raw_mode()?;

        Ok(())
    }

    pub fn init() -> Result<(), Error> {
        enable_raw_mode()?;
        Self::enter_alternate_screen()?;
        Self::disable_line_wrap()?;
        Self::clear_screen()?;
        Self::execute()?;

        Ok(())
    }

    pub fn clear_screen() -> Result<(), Error> {
        Self::queue_command(Clear(ClearType::All))?;

        Ok(())
    }
    pub fn clear_line() -> Result<(), Error> {
        Self::queue_command(Clear(ClearType::CurrentLine))?;

        Ok(())
    }

    /// Moves the cursor to the given Position.
    /// # Arguments
    /// * `Position` - the  `Position`to move the cursor to. Will be truncated to `u16::MAX` if bigger.
    pub fn move_cursor_to(position: Position) -> Result<(), Error> {
        // clippy::as_conversions: See doc above
        #[allow(clippy::as_conversions, clippy::cast_possible_truncation)]
        Self::queue_command(MoveTo(position.col as u16, position.row as u16))?;

        Ok(())
    }

    pub fn enter_alternate_screen() -> Result<(), Error> {
        Self::queue_command(EnterAlternateScreen)?;

        Ok(())
    }

    pub fn leave_alternate_screen() -> Result<(), Error> {
        Self::queue_command(LeaveAlternateScreen)?;

        Ok(())
    }

    pub fn hide_cursor() -> Result<(), Error> {
        Self::queue_command(Hide)?;

        Ok(())
    }

    pub fn show_cursor() -> Result<(), Error> {
        Self::queue_command(Show)?;

        Ok(())
    }

    pub fn disable_line_wrap() -> Result<(), Error> {
        Self::queue_command(DisableLineWrap)?;

        Ok(())
    }

    pub fn enable_line_wrap() -> Result<(), Error> {
        Self::queue_command(EnableLineWrap)?;

        Ok(())
    }

    pub fn set_title(title: &str) -> Result<(), Error> {
        Self::queue_command(SetTitle(title))?;

        Ok(())
    }

    pub fn size() -> Result<Size, Error> {
        let (width_u16, height_u16) = size()?;

        let height = height_u16 as usize;

        let width = width_u16 as usize;

        Ok(Size { height, width })
    }

    pub fn execute() -> Result<(), Error> {
        stdout().flush()?;

        Ok(())
    }

    fn queue_command<T: Command>(command: T) -> Result<(), Error> {
        queue!(stdout(), command)?;

        Ok(())
    }

    //
    // Printing
    //

    pub fn print(string: &str) -> Result<(), Error> {
        Self::queue_command(Print(string))?;
        Ok(())
    }

    pub fn print_row(row: RowIndex, line_text: &str) -> Result<(), Error> {
        Self::move_cursor_to(Position { row, col: 0 })?;
        Self::clear_line()?;
        Self::print(line_text)?;
        Ok(())
    }

    pub fn print_inverted_row(row: RowIndex, line_text: &str) -> Result<(), Error> {
        let width = Self::size()?.width;
        Self::print_row(row, &format!("{Reverse}{line_text:width$.width$}{Reset}"))
    }

    // pub fn print_rope_slice_row(row: RowIndex, rope_slice: RopeSlice) -> Result<(), Error> {
    //     Self::move_cursor_to(Position { row, col: 0 })?;
    //     Self::clear_line()?;
    //
    //     for chunk in rope_slice.chunks() {
    //         Self::print(chunk)?;
    //     }
    //     Ok(())
    // }

    pub fn print_selected_row(
        row: RowIndex,
        rope_slice: RopeSlice,
        selection_range: Option<(usize, usize)>,
    ) -> Result<(), Error> {
        Self::move_cursor_to(Position { row, col: 0 })?;
        Self::clear_line()?;

        let mut current_index = 0;

        let color_scheme = ColorScheme::default();

        for chunk in rope_slice.chunks() {
            let chunk_len = chunk.len();

            if let Some((start, end)) = selection_range {
                if current_index + chunk_len >= start && current_index <= end {
                    let relative_start = if start > current_index {
                        start - current_index
                    } else {
                        0
                    };
                    let relative_end = if end < current_index + chunk_len {
                        end - current_index
                    } else {
                        chunk_len
                    };

                    if relative_start > 0 {
                        Self::print(&chunk[0..relative_start])?;
                    }

                    if relative_end > relative_start {
                        Self::queue_command(SetBackgroundColor(color_scheme.selection_background))?;
                        Self::queue_command(SetForegroundColor(color_scheme.selection_foreground))?;
                        Self::print(&chunk[relative_start..relative_end])?;
                        Self::queue_command(ResetColor)?;
                    }

                    if relative_end < chunk_len {
                        Self::print(&chunk[relative_end..])?;
                    }
                } else {
                    Self::print(chunk)?;
                }
            } else {
                Self::print(chunk)?;
            }

            current_index += chunk_len;
        }

        Ok(())
    }

    pub fn print_searched_row(
        row: RowIndex,
        rope_slice: RopeSlice,
        selection_range: Option<(usize, usize)>,
    ) -> Result<(), Error> {
        Self::move_cursor_to(Position { row, col: 0 })?;
        Self::clear_line()?;

        let mut current_index = 0;

        let color_scheme = ColorScheme::default();

        for chunk in rope_slice.chunks() {
            let chunk_len = chunk.len();

            if let Some((start, end)) = selection_range {
                if current_index + chunk_len >= start && current_index <= end {
                    let relative_start = if start > current_index {
                        start - current_index
                    } else {
                        0
                    };
                    let relative_end = if end < current_index + chunk_len {
                        end - current_index
                    } else {
                        chunk_len
                    };

                    if relative_start > 0 {
                        Self::print(&chunk[0..relative_start])?;
                    }

                    if relative_end > relative_start {
                        Self::queue_command(SetBackgroundColor(
                            color_scheme.search_match_background,
                        ))?;
                        Self::queue_command(SetForegroundColor(
                            color_scheme.search_match_foreground,
                        ))?;
                        Self::print(&chunk[relative_start..relative_end])?;
                        Self::queue_command(ResetColor)?;
                    }

                    if relative_end < chunk_len {
                        Self::print(&chunk[relative_end..])?;
                    }
                } else {
                    Self::print(chunk)?;
                }
            } else {
                Self::print(chunk)?;
            }

            current_index += chunk_len;
        }

        Ok(())
    }
}
