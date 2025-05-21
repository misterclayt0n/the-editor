use anyhow::Context;
use std::io::{stdout, Write};

use crossterm::{
    cursor::{Hide, MoveTo, Show},
    execute, queue,
    style::{Attribute, Print, SetAttribute},
    terminal::{
        disable_raw_mode, enable_raw_mode, size, Clear, ClearType, EnterAlternateScreen,
        LeaveAlternateScreen,
    },
    Command as CECommand,
};
use utils::error;

use crate::RenderTUICommand;

/// Terminal is a simple handler for managing terminal commands.
#[derive(Clone)]
pub struct Terminal {}

impl Terminal {
    pub fn new() -> Self {
        Self {}
    }

    pub fn queue_command<T: CECommand>(command: T) {
        queue!(stdout(), command)
            .context("Failed to place command in queue")
            .map_err(Self::handle_terminal_error)
            .unwrap()
    }

    /// Handles all terminal errors. They're the lowest point of the application, and if they fail,
    /// there's not a way to recover, so we just log gracefully them before crashing.
    /// This function can likely be used in other places, for irrecoverable errors.
    pub fn handle_terminal_error<E: std::fmt::Display>(error: E) -> ! {
        error!("Fatal terminal error: {}", error);

        let error_msg = format!("Fatal error: {}", error);
        let _ = execute!(
            stdout(),
            Clear(ClearType::All),
            MoveTo(0, 0),
            Print(error_msg),
            Show,
        );
        let _ = stdout().flush();

        // Give user time to read the error.
        std::thread::sleep(std::time::Duration::from_secs(2));

        // Shutdown recovering the terminal
        let _ = execute!(stdout(), LeaveAlternateScreen);
        let _ = stdout().flush();

        disable_raw_mode().unwrap();

        std::process::exit(1);
    }

    /// Puts a `RenderTUICommand` into a queue.
    pub fn queue(&self, command: RenderTUICommand) {
        match command {
            RenderTUICommand::ClearScreen => Self::queue_command(Clear(ClearType::All)),
            RenderTUICommand::ClearLine => Self::queue_command(Clear(ClearType::CurrentLine)),
            RenderTUICommand::Print(string) => Self::queue_command(Print(string)),
            RenderTUICommand::PrintRope(rope) => {
                for chunk in rope.chunks() {
                    Self::queue_command(Print(chunk));
                }
            }
            RenderTUICommand::MoveCursor(x, y) => Self::queue_command(MoveTo(x as u16, y as u16)),
            RenderTUICommand::HideCursor => Self::queue_command(Hide),
            RenderTUICommand::ShowCursor => Self::queue_command(Show),
            RenderTUICommand::ForceError => {
                Self::handle_terminal_error("This is a forced error design for testing")
            }
            RenderTUICommand::SetInverseVideo(enable) => {
                if enable {
                    execute!(stdout(), SetAttribute(Attribute::Reverse)).unwrap();
                } else {
                    execute!(stdout(), SetAttribute(Attribute::Reset)).unwrap();
                }
            }
            RenderTUICommand::SetUnderline(enable) => {
                if enable {
                    execute!(stdout(), SetAttribute(Attribute::Underlined)).unwrap();
                } else {
                    execute!(stdout(), SetAttribute(Attribute::Reset)).unwrap();
                }
            }
        }
    }

    /// Executes all commands that are in the queue.
    pub fn flush(&self) {
        stdout()
            .flush()
            .context("Failed to flush commands")
            .map_err(Self::handle_terminal_error)
            .unwrap();
    }

    /// Kills the terminal.
    pub fn kill(&self) {
        let mut stdout = stdout();

        disable_raw_mode()
            .context("Failed to disable raw mode")
            .map_err(Self::handle_terminal_error)
            .unwrap();

        execute!(stdout, LeaveAlternateScreen)
            .context("Failed to leave alternate screen")
            .map_err(Self::handle_terminal_error)
            .unwrap();
    }

    /// Returns the size of the terminal.
    pub fn size(&self) -> (usize, usize) {
        let (width, height) = size()
            .context("Failed to get the size of the terminal")
            .map_err(Self::handle_terminal_error)
            .unwrap();

        (width as usize, height as usize)
    }

    /// Inits the terminal.
    pub fn init(&self) {
        let mut stdout = stdout();

        enable_raw_mode()
            .context("Failed to enter raw mode")
            .map_err(Self::handle_terminal_error)
            .unwrap();

        execute!(stdout, EnterAlternateScreen)
            .context("Failed to enter alternate screen")
            .map_err(Self::handle_terminal_error)
            .unwrap();
    }
}
