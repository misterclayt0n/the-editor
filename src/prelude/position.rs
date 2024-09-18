use super::{ColIndex, RowIndex};

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
