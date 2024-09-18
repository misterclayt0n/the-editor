pub type Row = usize;
pub type Col = usize;

#[derive(Copy, Clone, Default)]
pub struct Position {
    pub col: Col,
    pub row: Row,
}

impl Position {
    pub const fn saturating_sub(self, other: Self) -> Self {
        Self {
            row: self.row.saturating_sub(other.row),
            col: self.col.saturating_sub(other.col),
        }
    }
}
