use std::io::{stdout, Write};

use crossterm::{
    cursor::{Hide, MoveTo, Show},
    queue,
    style::Print,
    terminal::{Clear, ClearType},
    Command as CECommand,
};

use crate::{TerminalCommand, RendererError};

pub trait TerminalInterface {
    /// Puts a `Commands` into a queue
    fn queue(&self, command: TerminalCommand) -> Result<(), RendererError>;

    /// Executes all commands that are in the queue
    fn flush(&self) -> Result<(), RendererError>;
}

/// `Terminal` implements `TerminalInterface` using `crossterm`,
/// but could be used by anything else (although I don't think that's ever going to happen)
pub struct Terminal {}

impl Terminal {
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
            TerminalCommand::Print(string) => Self::queue_command(Print(string)),
            TerminalCommand::MoveCursor(x, y) => Self::queue_command(MoveTo(x as u16, y as u16)),
            TerminalCommand::HideCursor => Self::queue_command(Hide),
            TerminalCommand::ShowCursor => Self::queue_command(Show),
        }
    }

    fn flush(&self) -> Result<(), RendererError> {
        stdout().flush().map_err(|e| {
            RendererError::TerminalError(
                format!("Could not flush commands: {e}").to_string()
            )
        })
    }
}
