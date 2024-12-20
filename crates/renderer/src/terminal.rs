use std::io::{stdout, Write};

use crossterm::{
    cursor::{Hide, MoveTo, SetCursorStyle, Show},
    execute, queue,
    style::Print,
    terminal::{
        disable_raw_mode, enable_raw_mode, size, Clear, ClearType, EnterAlternateScreen,
        LeaveAlternateScreen,
    },
    Command as CECommand,
};

use crate::{RendererError, TerminalCommand};

pub trait TerminalInterface {
    /// Inits the terminal.
    fn init() -> Result<(), RendererError>;

    /// Puts a `Commands` into a queue.
    fn queue(&self, command: TerminalCommand) -> Result<(), RendererError>;

    /// Executes all commands that are in the queue.
    fn flush(&self) -> Result<(), RendererError>;

    /// Kills the terminal.
    fn kill() -> Result<(), RendererError>;

    /// Returns the size of the terminal.
    fn size() -> Result<(usize, usize), RendererError>;
}

/// `Terminal` implements `TerminalInterface` using `crossterm`,
/// but could be used by anything else (although I don't think that's ever going to happen).
pub struct Terminal {}

impl Terminal {
    pub fn new() -> Self {
        Self {}
    }

    fn queue_command<T: CECommand>(command: T) -> Result<(), RendererError> {
        queue!(stdout(), command).map_err(|e| {
            RendererError::TerminalError(
                format!("Could not put the command in queue: {e}").to_string(),
            )
        })
    }
}

impl TerminalInterface for Terminal {
    fn queue(&self, command: TerminalCommand) -> Result<(), RendererError> {
        match command {
            TerminalCommand::ClearScreen => Self::queue_command(Clear(ClearType::All)),
            TerminalCommand::ClearLine => Self::queue_command(Clear(ClearType::CurrentLine)),
            TerminalCommand::Print(string) => Self::queue_command(Print(string)),
            TerminalCommand::PrintRope(rope) => {
                for chunk in rope.chunks() {
                    Self::queue_command(Print(chunk))?;
                }
                Ok(())
            }
            TerminalCommand::MoveCursor(x, y) => Self::queue_command(MoveTo(x as u16, y as u16)),
            TerminalCommand::HideCursor => Self::queue_command(Hide),
            TerminalCommand::ShowCursor => Self::queue_command(Show),
            TerminalCommand::ChangeCursorStyleBar => {
                Self::queue_command(SetCursorStyle::BlinkingBar)
            }
            TerminalCommand::ChangeCursorStyleBlock => {
                Self::queue_command(SetCursorStyle::BlinkingBlock)
            }
        }
    }

    fn flush(&self) -> Result<(), RendererError> {
        stdout().flush().map_err(|e| {
            RendererError::TerminalError(format!("Could not flush commands: {e}").to_string())
        })
    }

    fn init() -> Result<(), RendererError> {
        let mut stdout = stdout();

        enable_raw_mode()
            .map_err(|e| RendererError::TerminalError(format!("Could not enter raw mode: {e}")))?;
        execute!(stdout, EnterAlternateScreen).map_err(|e| {
            RendererError::TerminalError(format!("Could not enter alternate screen: {e}"))
        })?;

        Ok(())
    }

    fn kill() -> Result<(), RendererError> {
        let mut stdout = stdout();

        disable_raw_mode().map_err(|e| {
            RendererError::TerminalError(format!("Could not disable raw mode: {e}"))
        })?;
        execute!(stdout, LeaveAlternateScreen).map_err(|e| {
            RendererError::TerminalError(format!("Could not leave alternate screen: {e}"))
        })?;

        Ok(())
    }

    fn size() -> Result<(usize, usize), RendererError> {
        let (width, height) = size().map_err(|e| {
            RendererError::TerminalError(format!("Could not get the size of the termminal: {e}"))
        })?;

        Ok((width as usize, height as usize))
    }
}
