use std::fs::File;

use log::LevelFilter;
use simplelog::{CombinedLogger, Config, WriteLogger};

/// Initializes the logging system.
pub fn init_logging() -> Result<(), Box<dyn std::error::Error>> {
    // Create or open the log file.
    let log_file = File::create("editor.log")?;

    // Configure the mf.
    CombinedLogger::init(
        vec![
            WriteLogger::new(LevelFilter::Info, Config::default(), log_file),
        ]
    )?;

    Ok(())
}

// Export the crates from logging because I only want to
// have to import `utils`, not `log` and `simplelog`.
pub use log::{info, debug, warn, error};

/// Just like vim.
#[derive(Clone, Copy)]
pub enum Mode {
    Normal,
    Insert,
}

/// NOTE: Maybe I'll split this into multiple different commands.
/// Command is any sort of high-level command from the-editor.
#[derive(Clone)]
pub enum Command {
    Quit,
    None,
    MoveCursorLeft,
    MoveCursorDown,
    MoveCursorUp,
    MoveCursorRight,
    SwitchMode(Mode),
    Resize(Size),
}

/// Position determines any (x, y) point in the plane.
#[derive(Clone, Copy, Debug)]
pub struct Position {
    pub x: usize,
    pub y: usize,
}

impl Position {
    /// Returns a new `Position` at the point (0, 0).
    pub fn zero() -> Self {
        Self { x: 0, y: 0 }
    }
}

/// Size determines the width and height of any given object.
#[derive(Clone, Copy)]
pub struct Size {
    pub width: usize,
    pub height: usize,
}
