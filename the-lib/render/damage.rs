#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Damage {
  pub full:      bool,
  pub start_row: usize,
  pub end_row:   usize,
}

impl Damage {
  pub const fn empty() -> Self {
    Self {
      full: false,
      start_row: 0,
      end_row: 0,
    }
  }

  pub const fn full(row_count: usize) -> Self {
    Self {
      full: true,
      start_row: 0,
      end_row: row_count,
    }
  }

  pub fn from_rows(rows: impl IntoIterator<Item = usize>) -> Self {
    let mut rows = rows.into_iter();
    let Some(first) = rows.next() else {
      return Self::empty();
    };

    let mut min_row = first;
    let mut max_row = first;
    for row in rows {
      min_row = min_row.min(row);
      max_row = max_row.max(row);
    }

    Self {
      full: false,
      start_row: min_row,
      end_row: max_row + 1,
    }
  }

  pub const fn is_empty(self) -> bool {
    !self.full && self.start_row == self.end_row
  }
}
