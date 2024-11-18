use std::fs::File;

use log::LevelFilter;
use simplelog::{CombinedLogger, Config, WriteLogger};

const VERSION: &str = "0.0.1";
const WELCOME_MESSAGE: &str = "the-editor";

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

/// NOTE: Maybe I'll split this into multiple different commands.
/// Command is any sort of high-level command from the-editor.
#[derive(Clone)]
pub enum Command {
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
#[derive(Clone, Copy, Debug)]
pub struct Position {
    pub x: usize,
    pub y: usize,
}

impl Position {
    /// Returns a new `Position` at the point (0, 0).
    pub fn new() -> Self {
        Self { x: 0, y: 0 }
    }
}

/// Size determines the width and height of any given object.
#[derive(Clone, Copy)]
pub struct Size {
    pub width: usize,
    pub height: usize,
}

pub struct Cursor {
    pub position: Position,
    pub desired_x: usize, // This keeps the desired column when the position.x gets adjusted.
}

impl Cursor {
    /// Returns a new `Cursor` with positions (0, 0) and desired_col as 0.
    pub fn new() -> Self {
        Self {
            position: Position::new(),
            desired_x: 0,
        }
    }
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
    } else if big_word {
        CharClass::Word // Here, all that is not space is considered bart of the word.
    } else if c.is_alphanumeric() || c == '_' {
        CharClass::Word
    } else {
        CharClass::Punctuation
    }
}

/// Builds the welcome message.
pub fn build_welcome_message(width: usize) -> String {
    if width == 0 {
        return "~".to_string();
    }

    let message = format!("{} -- {}", WELCOME_MESSAGE, VERSION);
    let message_len = message.len();

    if width <= message_len {
        return message[..width].to_string();
    }

    let padding = (width - message_len) / 2;
    let mut full_message = format!("~{}{}", " ".repeat(padding), message);
    full_message.truncate(width);
    return full_message;
}
