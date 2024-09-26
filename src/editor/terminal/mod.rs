mod attribute;
use crate::prelude::*;
use attribute::Attribute;
use crossterm::cursor::{Hide, MoveTo, Show};

use crossterm::style::{
    Attribute::{Reset, Reverse},
    Print, ResetColor, SetBackgroundColor, SetForegroundColor,
};

use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, size, Clear, ClearType, DisableLineWrap, EnableLineWrap,
    EnterAlternateScreen, LeaveAlternateScreen, SetTitle,
};

use crossterm::{queue, Command};
use ropey::RopeSlice;
use std::io::{stdout, Error, Write};

use super::AnnotatedString;
use super::{Position, Size};

/// Represents the Terminal.
/// Edge Case for platforms where `usize` < `u16`:
/// Regardless of the actual size of the Terminal, this representation
/// only spans over at most `usize::MAX` or `u16::size` rows/columns, whichever is smaller.
/// Each size returned truncates to min(`usize::MAX`, `u16::MAX`)
/// And should you attempt to set the cursor out of these bounds, it will also be truncated.
pub struct Terminal;

impl Terminal {
    pub fn terminate() -> Result<(), Error> {
        Self::leave_alternate_screen()?;
        Self::enable_line_wrap()?;
        Self::show_cursor()?;
        Self::execute()?;
        disable_raw_mode()?;

        Ok(())
    }

    pub fn initialize() -> Result<(), Error> {
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

    pub fn print_annotated_row(
        row: RowIndex,
        annotated_string: &AnnotatedString,
    ) -> Result<(), Error> {
        Self::move_cursor_to(Position { row, col: 0 })?;
        Self::clear_line()?;

        annotated_string
            .into_iter()
            .try_for_each(|part| -> Result<(), Error> {
                if let Some(annotation_type) = part.annotation_type {
                    let attribute: Attribute = annotation_type.into();
                    Self::set_attribute(&attribute)?;
                }

                Self::print(part.string)?;
                Self::reset_color()?;
                Ok(())
            })?;

        Ok(())
    }

    fn set_attribute(attribute: &Attribute) -> Result<(), Error> {
        if let Some(foreground_color) = attribute.foreground {
            Self::queue_command(SetForegroundColor(foreground_color))?;
        }

        if let Some(background_color) = attribute.background {
            Self::queue_command(SetBackgroundColor(background_color))?;
        }

        Ok(())
    }

    fn reset_color() -> Result<(), Error> {
        Self::queue_command(ResetColor)?;

        Ok(())
    }

    pub fn print_inverted_row(row: RowIndex, line_text: &str) -> Result<(), Error> {
        let width = Self::size()?.width;
        Self::print_row(row, &format!("{Reverse}{line_text:width$.width$}{Reset}"))
    }

    /// Returns the current size of this Terminal.
    /// Edge Case for systems with `usize` < `u16`:
    /// * A `Size` representing the terminal size. Any coordinate `z` truncated to `usize` if `usize` < `z` < `u16`
    pub fn size() -> Result<Size, Error> {
        let (width_u16, height_u16) = size()?;

        // clippy::as_conversions: See doc above
        #[allow(clippy::as_conversions)]
        let height = height_u16 as usize;

        // clippy::as_conversions: See doc above
        #[allow(clippy::as_conversions)]
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

    pub fn print_rope_slice_row(row: RowIndex, rope_slice: RopeSlice) -> Result<(), Error> {
        Self::move_cursor_to(Position { row, col: 0 })?;
        Self::clear_line()?;

        for chunk in rope_slice.chunks() {
            Self::print(chunk)?;
        }
        Ok(())
    }
}
