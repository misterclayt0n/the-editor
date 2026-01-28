//! Editor command types used by dispatch and clients.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
  Up,
  Down,
  Left,
  Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WordMotion {
  NextWordStart,
  PrevWordStart,
  NextWordEnd,
  PrevWordEnd,
  NextLongWordStart,
  PrevLongWordStart,
  NextLongWordEnd,
  PrevLongWordEnd,
  NextSubWordStart,
  PrevSubWordStart,
  NextSubWordEnd,
  PrevSubWordEnd,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Motion {
  Char {
    dir:    Direction,
    extend: bool,
    count:  usize,
  },
  Line {
    dir:    Direction,
    extend: bool,
    count:  usize,
  },
  VisualLine {
    dir:    Direction,
    extend: bool,
    count:  usize,
  },
  Word {
    kind:   WordMotion,
    extend: bool,
    count:  usize,
  },
  FileStart {
    extend: bool,
  },
  FileEnd {
    extend: bool,
  },
  LastLine {
    extend: bool,
  },
  Column {
    col:    usize,
    extend: bool,
  },
}

impl Motion {
  #[must_use]
  pub const fn move_char_left(count: usize) -> Self {
    Self::Char {
      dir: Direction::Left,
      extend: false,
      count,
    }
  }

  #[must_use]
  pub const fn move_char_right(count: usize) -> Self {
    Self::Char {
      dir: Direction::Right,
      extend: false,
      count,
    }
  }

  #[must_use]
  pub const fn move_char_up(count: usize) -> Self {
    Self::Line {
      dir: Direction::Up,
      extend: false,
      count,
    }
  }

  #[must_use]
  pub const fn move_char_down(count: usize) -> Self {
    Self::Line {
      dir: Direction::Down,
      extend: false,
      count,
    }
  }

  #[must_use]
  pub const fn move_visual_line_up(count: usize) -> Self {
    Self::VisualLine {
      dir: Direction::Up,
      extend: false,
      count,
    }
  }

  #[must_use]
  pub const fn move_visual_line_down(count: usize) -> Self {
    Self::VisualLine {
      dir: Direction::Down,
      extend: false,
      count,
    }
  }

  #[must_use]
  pub const fn extend_char_left(count: usize) -> Self {
    Self::Char {
      dir: Direction::Left,
      extend: true,
      count,
    }
  }

  #[must_use]
  pub const fn extend_char_right(count: usize) -> Self {
    Self::Char {
      dir: Direction::Right,
      extend: true,
      count,
    }
  }

  #[must_use]
  pub const fn extend_char_up(count: usize) -> Self {
    Self::Line {
      dir: Direction::Up,
      extend: true,
      count,
    }
  }

  #[must_use]
  pub const fn extend_char_down(count: usize) -> Self {
    Self::Line {
      dir: Direction::Down,
      extend: true,
      count,
    }
  }

  #[must_use]
  pub const fn extend_visual_line_up(count: usize) -> Self {
    Self::VisualLine {
      dir: Direction::Up,
      extend: true,
      count,
    }
  }

  #[must_use]
  pub const fn extend_visual_line_down(count: usize) -> Self {
    Self::VisualLine {
      dir: Direction::Down,
      extend: true,
      count,
    }
  }

  #[must_use]
  pub const fn extend_line_up(count: usize) -> Self {
    Self::Line {
      dir: Direction::Up,
      extend: true,
      count,
    }
  }

  #[must_use]
  pub const fn extend_line_down(count: usize) -> Self {
    Self::Line {
      dir: Direction::Down,
      extend: true,
      count,
    }
  }

  #[must_use]
  pub const fn move_next_word_start(count: usize) -> Self {
    Self::Word {
      kind: WordMotion::NextWordStart,
      extend: false,
      count,
    }
  }

  #[must_use]
  pub const fn move_prev_word_start(count: usize) -> Self {
    Self::Word {
      kind: WordMotion::PrevWordStart,
      extend: false,
      count,
    }
  }

  #[must_use]
  pub const fn move_next_word_end(count: usize) -> Self {
    Self::Word {
      kind: WordMotion::NextWordEnd,
      extend: false,
      count,
    }
  }

  #[must_use]
  pub const fn move_prev_word_end(count: usize) -> Self {
    Self::Word {
      kind: WordMotion::PrevWordEnd,
      extend: false,
      count,
    }
  }

  #[must_use]
  pub const fn move_next_long_word_start(count: usize) -> Self {
    Self::Word {
      kind: WordMotion::NextLongWordStart,
      extend: false,
      count,
    }
  }

  #[must_use]
  pub const fn move_prev_long_word_start(count: usize) -> Self {
    Self::Word {
      kind: WordMotion::PrevLongWordStart,
      extend: false,
      count,
    }
  }

  #[must_use]
  pub const fn move_next_long_word_end(count: usize) -> Self {
    Self::Word {
      kind: WordMotion::NextLongWordEnd,
      extend: false,
      count,
    }
  }

  #[must_use]
  pub const fn move_prev_long_word_end(count: usize) -> Self {
    Self::Word {
      kind: WordMotion::PrevLongWordEnd,
      extend: false,
      count,
    }
  }

  #[must_use]
  pub const fn move_next_sub_word_start(count: usize) -> Self {
    Self::Word {
      kind: WordMotion::NextSubWordStart,
      extend: false,
      count,
    }
  }

  #[must_use]
  pub const fn move_prev_sub_word_start(count: usize) -> Self {
    Self::Word {
      kind: WordMotion::PrevSubWordStart,
      extend: false,
      count,
    }
  }

  #[must_use]
  pub const fn move_next_sub_word_end(count: usize) -> Self {
    Self::Word {
      kind: WordMotion::NextSubWordEnd,
      extend: false,
      count,
    }
  }

  #[must_use]
  pub const fn move_prev_sub_word_end(count: usize) -> Self {
    Self::Word {
      kind: WordMotion::PrevSubWordEnd,
      extend: false,
      count,
    }
  }

  #[must_use]
  pub const fn extend_next_word_start(count: usize) -> Self {
    Self::Word {
      kind: WordMotion::NextWordStart,
      extend: true,
      count,
    }
  }

  #[must_use]
  pub const fn extend_prev_word_start(count: usize) -> Self {
    Self::Word {
      kind: WordMotion::PrevWordStart,
      extend: true,
      count,
    }
  }

  #[must_use]
  pub const fn extend_next_word_end(count: usize) -> Self {
    Self::Word {
      kind: WordMotion::NextWordEnd,
      extend: true,
      count,
    }
  }

  #[must_use]
  pub const fn extend_prev_word_end(count: usize) -> Self {
    Self::Word {
      kind: WordMotion::PrevWordEnd,
      extend: true,
      count,
    }
  }

  #[must_use]
  pub const fn extend_next_long_word_start(count: usize) -> Self {
    Self::Word {
      kind: WordMotion::NextLongWordStart,
      extend: true,
      count,
    }
  }

  #[must_use]
  pub const fn extend_prev_long_word_start(count: usize) -> Self {
    Self::Word {
      kind: WordMotion::PrevLongWordStart,
      extend: true,
      count,
    }
  }

  #[must_use]
  pub const fn extend_next_long_word_end(count: usize) -> Self {
    Self::Word {
      kind: WordMotion::NextLongWordEnd,
      extend: true,
      count,
    }
  }

  #[must_use]
  pub const fn extend_prev_long_word_end(count: usize) -> Self {
    Self::Word {
      kind: WordMotion::PrevLongWordEnd,
      extend: true,
      count,
    }
  }

  #[must_use]
  pub const fn extend_next_sub_word_start(count: usize) -> Self {
    Self::Word {
      kind: WordMotion::NextSubWordStart,
      extend: true,
      count,
    }
  }

  #[must_use]
  pub const fn extend_prev_sub_word_start(count: usize) -> Self {
    Self::Word {
      kind: WordMotion::PrevSubWordStart,
      extend: true,
      count,
    }
  }

  #[must_use]
  pub const fn extend_next_sub_word_end(count: usize) -> Self {
    Self::Word {
      kind: WordMotion::NextSubWordEnd,
      extend: true,
      count,
    }
  }

  #[must_use]
  pub const fn extend_prev_sub_word_end(count: usize) -> Self {
    Self::Word {
      kind: WordMotion::PrevSubWordEnd,
      extend: true,
      count,
    }
  }

  #[must_use]
  pub const fn extend_to_file_start() -> Self {
    Self::FileStart { extend: true }
  }

  #[must_use]
  pub const fn extend_to_file_end() -> Self {
    Self::FileEnd { extend: true }
  }

  #[must_use]
  pub const fn extend_to_last_line() -> Self {
    Self::LastLine { extend: true }
  }

  #[must_use]
  pub const fn extend_to_column(col: usize) -> Self {
    Self::Column { col, extend: true }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
  InsertChar(char),
  DeleteChar,
  Move(Direction),
  AddCursor(Direction),
  Motion(Motion),
  Save,
  Quit,
}

impl Command {
  #[must_use]
  pub const fn move_char_left(count: usize) -> Self {
    Self::Motion(Motion::move_char_left(count))
  }

  #[must_use]
  pub const fn move_char_right(count: usize) -> Self {
    Self::Motion(Motion::move_char_right(count))
  }

  #[must_use]
  pub const fn move_char_up(count: usize) -> Self {
    Self::Motion(Motion::move_char_up(count))
  }

  #[must_use]
  pub const fn move_char_down(count: usize) -> Self {
    Self::Motion(Motion::move_char_down(count))
  }

  #[must_use]
  pub const fn move_visual_line_up(count: usize) -> Self {
    Self::Motion(Motion::move_visual_line_up(count))
  }

  #[must_use]
  pub const fn move_visual_line_down(count: usize) -> Self {
    Self::Motion(Motion::move_visual_line_down(count))
  }

  #[must_use]
  pub const fn extend_char_left(count: usize) -> Self {
    Self::Motion(Motion::extend_char_left(count))
  }

  #[must_use]
  pub const fn extend_char_right(count: usize) -> Self {
    Self::Motion(Motion::extend_char_right(count))
  }

  #[must_use]
  pub const fn extend_char_up(count: usize) -> Self {
    Self::Motion(Motion::extend_char_up(count))
  }

  #[must_use]
  pub const fn extend_char_down(count: usize) -> Self {
    Self::Motion(Motion::extend_char_down(count))
  }

  #[must_use]
  pub const fn extend_visual_line_up(count: usize) -> Self {
    Self::Motion(Motion::extend_visual_line_up(count))
  }

  #[must_use]
  pub const fn extend_visual_line_down(count: usize) -> Self {
    Self::Motion(Motion::extend_visual_line_down(count))
  }

  #[must_use]
  pub const fn extend_line_up(count: usize) -> Self {
    Self::Motion(Motion::extend_line_up(count))
  }

  #[must_use]
  pub const fn extend_line_down(count: usize) -> Self {
    Self::Motion(Motion::extend_line_down(count))
  }

  #[must_use]
  pub const fn move_next_word_start(count: usize) -> Self {
    Self::Motion(Motion::move_next_word_start(count))
  }

  #[must_use]
  pub const fn move_prev_word_start(count: usize) -> Self {
    Self::Motion(Motion::move_prev_word_start(count))
  }

  #[must_use]
  pub const fn move_next_word_end(count: usize) -> Self {
    Self::Motion(Motion::move_next_word_end(count))
  }

  #[must_use]
  pub const fn move_prev_word_end(count: usize) -> Self {
    Self::Motion(Motion::move_prev_word_end(count))
  }

  #[must_use]
  pub const fn move_next_long_word_start(count: usize) -> Self {
    Self::Motion(Motion::move_next_long_word_start(count))
  }

  #[must_use]
  pub const fn move_prev_long_word_start(count: usize) -> Self {
    Self::Motion(Motion::move_prev_long_word_start(count))
  }

  #[must_use]
  pub const fn move_next_long_word_end(count: usize) -> Self {
    Self::Motion(Motion::move_next_long_word_end(count))
  }

  #[must_use]
  pub const fn move_prev_long_word_end(count: usize) -> Self {
    Self::Motion(Motion::move_prev_long_word_end(count))
  }

  #[must_use]
  pub const fn move_next_sub_word_start(count: usize) -> Self {
    Self::Motion(Motion::move_next_sub_word_start(count))
  }

  #[must_use]
  pub const fn move_prev_sub_word_start(count: usize) -> Self {
    Self::Motion(Motion::move_prev_sub_word_start(count))
  }

  #[must_use]
  pub const fn move_next_sub_word_end(count: usize) -> Self {
    Self::Motion(Motion::move_next_sub_word_end(count))
  }

  #[must_use]
  pub const fn move_prev_sub_word_end(count: usize) -> Self {
    Self::Motion(Motion::move_prev_sub_word_end(count))
  }

  #[must_use]
  pub const fn extend_next_word_start(count: usize) -> Self {
    Self::Motion(Motion::extend_next_word_start(count))
  }

  #[must_use]
  pub const fn extend_prev_word_start(count: usize) -> Self {
    Self::Motion(Motion::extend_prev_word_start(count))
  }

  #[must_use]
  pub const fn extend_next_word_end(count: usize) -> Self {
    Self::Motion(Motion::extend_next_word_end(count))
  }

  #[must_use]
  pub const fn extend_prev_word_end(count: usize) -> Self {
    Self::Motion(Motion::extend_prev_word_end(count))
  }

  #[must_use]
  pub const fn extend_next_long_word_start(count: usize) -> Self {
    Self::Motion(Motion::extend_next_long_word_start(count))
  }

  #[must_use]
  pub const fn extend_prev_long_word_start(count: usize) -> Self {
    Self::Motion(Motion::extend_prev_long_word_start(count))
  }

  #[must_use]
  pub const fn extend_next_long_word_end(count: usize) -> Self {
    Self::Motion(Motion::extend_next_long_word_end(count))
  }

  #[must_use]
  pub const fn extend_prev_long_word_end(count: usize) -> Self {
    Self::Motion(Motion::extend_prev_long_word_end(count))
  }

  #[must_use]
  pub const fn extend_next_sub_word_start(count: usize) -> Self {
    Self::Motion(Motion::extend_next_sub_word_start(count))
  }

  #[must_use]
  pub const fn extend_prev_sub_word_start(count: usize) -> Self {
    Self::Motion(Motion::extend_prev_sub_word_start(count))
  }

  #[must_use]
  pub const fn extend_next_sub_word_end(count: usize) -> Self {
    Self::Motion(Motion::extend_next_sub_word_end(count))
  }

  #[must_use]
  pub const fn extend_prev_sub_word_end(count: usize) -> Self {
    Self::Motion(Motion::extend_prev_sub_word_end(count))
  }

  #[must_use]
  pub const fn extend_to_file_start() -> Self {
    Self::Motion(Motion::extend_to_file_start())
  }

  #[must_use]
  pub const fn extend_to_file_end() -> Self {
    Self::Motion(Motion::extend_to_file_end())
  }

  #[must_use]
  pub const fn extend_to_last_line() -> Self {
    Self::Motion(Motion::extend_to_last_line())
  }

  #[must_use]
  pub const fn extend_to_column(col: usize) -> Self {
    Self::Motion(Motion::extend_to_column(col))
  }
}
