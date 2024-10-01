pub type GraphemeIndex = usize;
pub type LineIndex = usize;
pub type ByteIndex = usize;
pub type ColIndex = usize;
pub type RowIndex = usize;

pub const NAME: &str = "the-editor";
pub const VERSION: &str = "0.0.1";

#[derive(Copy, Clone, Default, Debug)]
pub struct Location {
    pub grapheme_index: GraphemeIndex,
    pub line_index: LineIndex,
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
