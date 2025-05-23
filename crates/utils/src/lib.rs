use std::fs::File;

use log::LevelFilter;
use simplelog::{CombinedLogger, Config, WriteLogger};

/// Initializes the logging system.
pub fn init_logging() -> Result<(), Box<dyn std::error::Error>> {
    // Create or open the log file.
    let log_file = File::create("editor.log")?;

    // Configure the mf.
    CombinedLogger::init(vec![WriteLogger::new(
        LevelFilter::Info,
        Config::default(),
        log_file,
    )])?;

    Ok(())
}

// Export the crates from logging because I only want to
// have to import `utils`, not `log` and `simplelog`.
pub use log::{debug, error, info, warn};

/// Just like vim.
#[derive(Clone, Copy)]
pub enum Mode {
    Normal,
    Insert,
}

/// Which version of the renderer to use.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum InterfaceType {
    TUI,
    GUI,
}

/// NOTE: Maybe I'll split this into multiple different commands.
/// Command is any sort of high-level command from the-editor.
#[derive(Clone)]
pub enum Command {
    ForceError,
    Quit,
    None,
    InsertChar(char),
    DeleteCharBackward,
    DeleteCharForward,
    MoveCursorLeft,
    MoveCursorDown,
    MoveCursorUp,
    MoveCursorRight(bool),
    SwitchMode(Mode),
    Resize(Size),
    MoveCursorEndOfLine,
    MoveCursorStartOfLine,
    MoveCursorFirstCharOfLine,
    MoveCursorWordForward(bool), // bool indicates if the word is big or not.
    MoveCursorWordBackward(bool),
    MoveCursorWordForwardEnd(bool),
}

/// Position determines any (x, y) point in the plane.
#[derive(Clone, Copy, Debug, Default)]
pub struct Position {
    pub x: i32,
    pub y: i32,
}

/// PositionF is just like Position, but using float.
#[derive(Clone, Copy, Debug, Default)]
pub struct PositionF {
    pub x: f32,
    pub y: f32,
}

/// Size determines the width and height of any given object.
#[derive(Clone, Copy)]
pub struct Size {
    pub width: i32,
    pub height: i32,
}

#[derive(Default, Clone, Copy)]
pub struct Cursor {
    pub position: Position,
    pub desired_x: i32, // This keeps the desired column when the position.x gets adjusted.
}

#[derive(PartialEq)]
pub enum CharClass {
    Whitespace,
    Word,
    Punctuation,
}

pub fn get_char_class(c: char, big_word: bool) -> CharClass {
    if c.is_whitespace() {
        CharClass::Whitespace
    } else if big_word || c.is_alphanumeric() || c == '_' {
        CharClass::Word // Here, all that is not space is considered bart of the word.
    } else {
        CharClass::Punctuation
    }
}
