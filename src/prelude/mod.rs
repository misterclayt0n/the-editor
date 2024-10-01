use std::cmp::Ordering;

pub type GraphemeIndex = usize;
pub type LineIndex = usize;
pub type ByteIndex = usize;
pub type ColIndex = usize;
pub type RowIndex = usize;

pub const NAME: &str = "the-editor";
pub const VERSION: &str = "0.0.1";
pub const TAB_WIDTH: usize = 4;

#[derive(Copy, Clone, Default, Debug, Eq, PartialEq)]
pub struct Location {
    pub grapheme_index: GraphemeIndex,
    pub line_index: LineIndex,
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
    pub col: ColIndex,
    pub row: RowIndex,
}

impl Position {
    pub const fn saturating_sub(self, other: Self) -> Self {
        Self {
            row: self.row.saturating_sub(other.row),
            col: self.col.saturating_sub(other.col),
        }
    }
}

#[derive(Default, Copy, Clone, Eq, PartialEq)]
pub struct Size {
    pub height: usize,
    pub width: usize,
}

#[derive(Clone, Copy)]
pub enum WordType {
    Word,
    BigWord,
}
