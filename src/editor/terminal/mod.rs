use crossterm::cursor::{Hide, MoveTo, Show};

use crossterm::style::{Attribute, Color, SetForegroundColor};
use crossterm::style::{
    Attribute::{Reset, Reverse},
    Print, ResetColor, SetBackgroundColor,
};

use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, size, Clear, ClearType, DisableLineWrap, EnableLineWrap,
    EnterAlternateScreen, LeaveAlternateScreen, SetTitle,
};

use crossterm::{queue, Command};
use std::io::{stdout, Error, Write};
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

    pub fn move_cursor_to(position: Position) -> Result<(), Error> {
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

    pub fn print_row(row: usize, line_text: &str) -> Result<(), Error> {
        Self::move_cursor_to(Position { row, col: 0 })?;
        Self::clear_line()?;
        Self::print(line_text)?;

        Ok(())
    }

    pub fn print_inverted_row(row: usize, line_text: &str) -> Result<(), Error> {
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

    pub fn styled_text(
        text: &str,
        foreground: Option<Color>,
        background: Option<Color>,
        attributes: &[Attribute],
    ) -> String {
        let mut styled = String::new();
        if let Some(bg) = background {
            styled.push_str(&format!("{}", SetBackgroundColor(bg)));
        }
        if let Some(fg) = foreground {
            styled.push_str(&format!("{}", SetForegroundColor(fg)));
        }
        for attr in attributes {
            styled.push_str(&format!("{}", attr));
        }
        styled.push_str(text);
        styled.push_str(&format!("{}", ResetColor));
        styled
    }
}
