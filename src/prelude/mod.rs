use std::{cmp::Ordering, fs::OpenOptions, sync::Mutex};
use lazy_static::lazy_static;
use std::io::Write;

pub const NAME: &str = "the-editor";
pub const VERSION: &str = "0.0.1";
pub const TAB_WIDTH: usize = 4;

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
