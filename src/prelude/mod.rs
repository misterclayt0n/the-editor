use core::fmt;
use lazy_static::lazy_static;
use std::io::Write;
use std::{cmp::Ordering, fs::OpenOptions, sync::Mutex};

pub const NAME: &str = "the-editor";
pub const VERSION: &str = "0.0.1";
pub const TAB_WIDTH: usize = 4;
pub const QUIT_TIMES: u8 = 3;

#[derive(Copy, Clone, Default, Debug, Eq, PartialEq)]
pub struct Location {
    pub grapheme_index: usize,
    pub line_index: usize,
}

impl Ord for Location {
    fn cmp(&self, other: &Self) -> Ordering {
        self.line_index
            .cmp(&other.line_index)
            .then(self.grapheme_index.cmp(&other.grapheme_index))
    }
}

impl PartialOrd for Location {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Copy, Clone, Default)]
pub struct Position {
    pub col: usize,
    pub row: usize,
}

#[derive(Default, Copy, Clone, Eq, PartialEq)]
pub struct Size {
    pub height: usize,
    pub width: usize,
}

#[derive(Clone, Copy, PartialEq)]
pub enum WordType {
    Word,
    BigWord,
}

#[derive(PartialEq, Copy, Clone)]
pub enum SelectionMode {
    Visual,
    VisualLine,
}

#[derive(Copy, Clone, Debug)]
pub enum GraphemeWidth {
    Half,
    Full,
}
impl From<GraphemeWidth> for usize {
    fn from(val: GraphemeWidth) -> Self {
        match val {
            GraphemeWidth::Half => 1,
            GraphemeWidth::Full => 2,
        }
    }
}

impl GraphemeWidth {
    pub fn as_usize(&self) -> usize {
        match self {
            GraphemeWidth::Half => 1,
            GraphemeWidth::Full => 2,
        }
    }
}

pub const MATCHING_DELIMITERS: [(char, char); 6] = [
    ('(', ')'),
    ('{', '}'),
    ('[', ']'),
    ('"', '"'),
    ('\'', '\''),
    ('<', '>'),
];

pub enum FocusDirection {
    Up,
    Down,
    Left,
    Right,
}

lazy_static! {
    static ref LOG_MUTEX: Mutex<()> = Mutex::new(());
}

pub fn log(message: &str) {
    let _lock = LOG_MUTEX.lock().unwrap();
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open("editor.log")
        .unwrap_or_else(|_| panic!("Could not open log file."));
    writeln!(file, "{}", message).unwrap_or_else(|_| panic!("Could not write to log file."));
}

#[derive(Clone, Copy, PartialEq)]
pub enum Operator {
    Delete,
    Change,
    Yank, // TODO
}

#[derive(Clone, Copy)]
pub enum TextObject {
    Inner(char), // represents 'i' followed by a delimiter, like '('
}

#[derive(Clone, Copy)]
pub enum Normal {
    PageUp,
    PageDown,
    StartOfLine,
    FirstCharLine,
    EndOfLine,
    Up,
    Left,
    LeftAfterDeletion,
    Right,
    Down,
    WordForward,
    WordBackward,
    BigWordForward,
    BigWordBackward,
    WordEndForward,
    BigWordEndForward,
    GoToTop,
    GoToBottom,
    AppendRight,
    InsertAtLineStart,
    InsertAtLineEnd,
}

#[derive(Clone, Copy)]
pub enum Edit {
    Insert(char),
    InsertNewline,
    Delete,
    DeleteBackward,
    SubstituteChar,
    ChangeLine,
    SubstitueSelection,
    InsertNewlineBelow,
    InsertNewlineAbove,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputType {
    Command,
    Search,
    Save,
    FindFile,
    Replace,
    ReplaceFor(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputModeType {
    Insert,
    Normal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModeType {
    Normal,
    Insert,
    Visual,
    VisualLine,
    Input(InputType, InputModeType),
}

impl fmt::Display for ModeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ModeType::Insert => write!(f, "INSERT"),
            ModeType::Normal => write!(f, "NORMAL"),
            ModeType::Visual => write!(f, "VISUAL"),
            ModeType::VisualLine => write!(f, "VISUAL LINE"),
            ModeType::Input(input_type, _) => {
                let input_str = match input_type {
                    InputType::Command => "COMMAND",
                    InputType::Search => "SEARCH",
                    InputType::Save => "SAVE",
                    InputType::FindFile => "FIND FILE",
                    InputType::Replace | InputType::ReplaceFor(_) => "REPLACE",
                };

                write!(f, "{}", input_str)
            }
        }
    }
}

impl Default for ModeType {
    fn default() -> Self {
        ModeType::Normal
    }
}

#[derive(Default, Eq, PartialEq, Clone, Copy)]
pub enum SearchDirection {
    #[default]
    Forward,
    Backward,
}

#[derive(Clone)]
pub struct SearchInfo {
    pub prev_location: Location,
    pub prev_scroll_offset: Position,
    pub query: Option<String>,
}
