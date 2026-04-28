#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Damage {
  pub full:      bool,
  pub start_row: usize,
  pub end_row:   usize,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct DirtyFlag {
  pub dirty: bool,
}

impl DirtyFlag {
  pub fn mark(&mut self) {
    self.dirty = true;
  }

  pub fn clear(&mut self) {
    self.dirty = false;
  }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct DirtyRows {
  pub dirty: bool,
  pub full:  bool,
}

impl DirtyRows {
  pub fn mark(&mut self, full: bool) {
    self.dirty = true;
    self.full |= full;
  }

  pub fn clear(&mut self) {
    self.dirty = false;
    self.full = false;
  }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct DirtyState {
  pub rows:       DirtyRows,
  pub selections: DirtyFlag,
  pub cursors:    DirtyFlag,
  pub syntax:     DirtyFlag,
  pub widgets:    DirtyFlag,
}

impl DirtyState {
  pub fn clear(&mut self) {
    self.rows.clear();
    self.selections.clear();
    self.cursors.clear();
    self.syntax.clear();
    self.widgets.clear();
  }
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

  pub fn union(self, other: Self) -> Self {
    if self.full || other.full {
      return Self {
        full: true,
        start_row: self.start_row.min(other.start_row),
        end_row: self.end_row.max(other.end_row),
      };
    }

    if self.is_empty() {
      return other;
    }
    if other.is_empty() {
      return self;
    }

    Self {
      full: false,
      start_row: self.start_row.min(other.start_row),
      end_row: self.end_row.max(other.end_row),
    }
  }
}
