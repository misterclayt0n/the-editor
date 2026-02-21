//! Editor command types used by dispatch and clients.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
  Up,
  Down,
  Left,
  Right,
  Forward,
  Backward,
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
  InsertNewline,
  DeleteChar,
  DeleteCharForward {
    count: usize,
  },
  DeleteWordBackward {
    count: usize,
  },
  DeleteWordForward {
    count: usize,
  },
  KillToLineStart,
  KillToLineEnd,
  InsertTab,
  GotoLineStart {
    extend: bool,
  },
  GotoFirstNonWhitespace {
    extend: bool,
  },
  GotoLineEnd {
    extend: bool,
  },
  PageUp {
    extend: bool,
  },
  PageDown {
    extend: bool,
  },
  PageCursorHalfUp,
  PageCursorHalfDown,
  FindChar {
    direction: Direction,
    inclusive: bool,
    extend:    bool,
  },
  ParentNodeEnd {
    extend: bool,
  },
  ParentNodeStart {
    extend: bool,
  },
  Move(Direction),
  AddCursor(Direction),
  Motion(Motion),
  GotoNextBuffer {
    count: usize,
  },
  GotoPreviousBuffer {
    count: usize,
  },
  GotoWindowTop {
    count: usize,
  },
  GotoWindowCenter,
  GotoWindowBottom {
    count: usize,
  },
  RotateView,
  HSplit,
  VSplit,
  TransposeView,
  WClose,
  WOnly,
  JumpViewLeft,
  JumpViewDown,
  JumpViewUp,
  JumpViewRight,
  SwapViewLeft,
  SwapViewDown,
  SwapViewUp,
  SwapViewRight,
  GotoFileHSplit,
  GotoFileVSplit,
  HSplitNew,
  VSplitNew,
  ToggleComments,
  JumpForward {
    count: usize,
  },
  JumpBackward {
    count: usize,
  },
  SaveSelection,
  GotoLastAccessedFile,
  GotoLastModifiedFile,
  GotoLastModification,
  GotoWord,
  ExtendToWord,
  SplitSelectionOnNewline,
  MergeSelections,
  MergeConsecutiveSelections,
  SplitSelection,
  JoinSelections,
  JoinSelectionsSpace,
  KeepSelections,
  RemoveSelections,
  AlignSelections,
  KeepActiveSelection,
  RemoveActiveSelection,
  TrimSelections,
  CollapseSelection,
  FlipSelections,
  ExpandSelection,
  ShrinkSelection,
  SelectAllChildren,
  SelectAllSiblings,
  SelectPrevSibling,
  SelectNextSibling,
  DeleteSelection {
    yank: bool,
  },
  ChangeSelection {
    yank: bool,
  },
  Replace,
  ReplaceWithYanked,
  Yank,
  Paste {
    after: bool,
  },
  RecordMacro,
  ReplayMacro,
  RepeatLastMotion,
  SwitchCase,
  SwitchToUppercase,
  SwitchToLowercase,
  InsertAtLineStart,
  InsertAtLineEnd,
  AppendMode,
  OpenBelow,
  OpenAbove,
  CommitUndoCheckpoint,
  CopySelectionOnNextLine,
  CopySelectionOnPrevLine,
  SelectAll,
  ExtendLineBelow {
    count: usize,
  },
  ExtendLineAbove {
    count: usize,
  },
  ExtendToLineBounds,
  ShrinkToLineBounds,
  Undo {
    count: usize,
  },
  Redo {
    count: usize,
  },
  Earlier {
    count: usize,
  },
  Later {
    count: usize,
  },
  Indent {
    count: usize,
  },
  Unindent {
    count: usize,
  },
  MatchBrackets,
  SurroundAdd,
  SurroundDelete {
    count: usize,
  },
  SurroundReplace {
    count: usize,
  },
  SelectTextobjectAround,
  SelectTextobjectInner,
  GotoPrevDiag,
  GotoFirstDiag,
  GotoNextDiag,
  GotoLastDiag,
  GotoPrevChange,
  GotoFirstChange,
  GotoNextChange,
  GotoLastChange,
  GotoPrevFunction,
  GotoNextFunction,
  GotoPrevClass,
  GotoNextClass,
  GotoPrevParameter,
  GotoNextParameter,
  GotoPrevComment,
  GotoNextComment,
  GotoPrevEntry,
  GotoNextEntry,
  GotoPrevTest,
  GotoNextTest,
  GotoPrevXmlElement,
  GotoNextXmlElement,
  GotoPrevParagraph,
  GotoNextParagraph,
  AddNewlineAbove,
  AddNewlineBelow,
  SearchSelectionDetectWordBoundaries,
  SearchSelection,
  SearchNextOrPrev {
    direction: Direction,
    extend:    bool,
    count:     usize,
  },
  Search,
  RSearch,
  SelectRegex,
  FilePicker,
  LspGotoDeclaration,
  LspGotoDefinition,
  LspGotoTypeDefinition,
  LspGotoImplementation,
  LspHover,
  LspReferences,
  LspDocumentSymbols,
  LspWorkspaceSymbols,
  LspCompletion,
  CompletionNext,
  CompletionPrev,
  CompletionAccept,
  CompletionCancel,
  CompletionDocsScrollUp,
  CompletionDocsScrollDown,
  LspSignatureHelp,
  LspCodeActions,
  LspFormat,
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
  pub const fn goto_file_start() -> Self {
    Self::Motion(Motion::FileStart { extend: false })
  }

  #[must_use]
  pub const fn goto_last_line() -> Self {
    Self::Motion(Motion::LastLine { extend: false })
  }

  #[must_use]
  pub const fn goto_next_buffer(count: usize) -> Self {
    Self::GotoNextBuffer { count }
  }

  #[must_use]
  pub const fn goto_previous_buffer(count: usize) -> Self {
    Self::GotoPreviousBuffer { count }
  }

  #[must_use]
  pub const fn goto_window_top(count: usize) -> Self {
    Self::GotoWindowTop { count }
  }

  #[must_use]
  pub const fn goto_window_center() -> Self {
    Self::GotoWindowCenter
  }

  #[must_use]
  pub const fn goto_window_bottom(count: usize) -> Self {
    Self::GotoWindowBottom { count }
  }

  #[must_use]
  pub const fn rotate_view() -> Self {
    Self::RotateView
  }

  #[must_use]
  pub const fn hsplit() -> Self {
    Self::HSplit
  }

  #[must_use]
  pub const fn vsplit() -> Self {
    Self::VSplit
  }

  #[must_use]
  pub const fn transpose_view() -> Self {
    Self::TransposeView
  }

  #[must_use]
  pub const fn wclose() -> Self {
    Self::WClose
  }

  #[must_use]
  pub const fn wonly() -> Self {
    Self::WOnly
  }

  #[must_use]
  pub const fn jump_view_left() -> Self {
    Self::JumpViewLeft
  }

  #[must_use]
  pub const fn jump_view_down() -> Self {
    Self::JumpViewDown
  }

  #[must_use]
  pub const fn jump_view_up() -> Self {
    Self::JumpViewUp
  }

  #[must_use]
  pub const fn jump_view_right() -> Self {
    Self::JumpViewRight
  }

  #[must_use]
  pub const fn swap_view_left() -> Self {
    Self::SwapViewLeft
  }

  #[must_use]
  pub const fn swap_view_down() -> Self {
    Self::SwapViewDown
  }

  #[must_use]
  pub const fn swap_view_up() -> Self {
    Self::SwapViewUp
  }

  #[must_use]
  pub const fn swap_view_right() -> Self {
    Self::SwapViewRight
  }

  #[must_use]
  pub const fn goto_file_hsplit() -> Self {
    Self::GotoFileHSplit
  }

  #[must_use]
  pub const fn goto_file_vsplit() -> Self {
    Self::GotoFileVSplit
  }

  #[must_use]
  pub const fn hsplit_new() -> Self {
    Self::HSplitNew
  }

  #[must_use]
  pub const fn vsplit_new() -> Self {
    Self::VSplitNew
  }

  #[must_use]
  pub const fn toggle_comments() -> Self {
    Self::ToggleComments
  }

  #[must_use]
  pub const fn jump_forward(count: usize) -> Self {
    Self::JumpForward { count }
  }

  #[must_use]
  pub const fn jump_backward(count: usize) -> Self {
    Self::JumpBackward { count }
  }

  #[must_use]
  pub const fn save_selection() -> Self {
    Self::SaveSelection
  }

  #[must_use]
  pub const fn goto_last_accessed_file() -> Self {
    Self::GotoLastAccessedFile
  }

  #[must_use]
  pub const fn goto_last_modified_file() -> Self {
    Self::GotoLastModifiedFile
  }

  #[must_use]
  pub const fn goto_last_modification() -> Self {
    Self::GotoLastModification
  }

  #[must_use]
  pub const fn goto_word() -> Self {
    Self::GotoWord
  }

  #[must_use]
  pub const fn extend_to_word() -> Self {
    Self::ExtendToWord
  }

  #[must_use]
  pub const fn split_selection_on_newline() -> Self {
    Self::SplitSelectionOnNewline
  }

  #[must_use]
  pub const fn merge_selections() -> Self {
    Self::MergeSelections
  }

  #[must_use]
  pub const fn merge_consecutive_selections() -> Self {
    Self::MergeConsecutiveSelections
  }

  #[must_use]
  pub const fn split_selection() -> Self {
    Self::SplitSelection
  }

  #[must_use]
  pub const fn join_selections() -> Self {
    Self::JoinSelections
  }

  #[must_use]
  pub const fn join_selections_space() -> Self {
    Self::JoinSelectionsSpace
  }

  #[must_use]
  pub const fn keep_selections() -> Self {
    Self::KeepSelections
  }

  #[must_use]
  pub const fn remove_selections() -> Self {
    Self::RemoveSelections
  }

  #[must_use]
  pub const fn align_selections() -> Self {
    Self::AlignSelections
  }

  #[must_use]
  pub const fn keep_active_selection() -> Self {
    Self::KeepActiveSelection
  }

  #[must_use]
  pub const fn remove_active_selection() -> Self {
    Self::RemoveActiveSelection
  }

  #[must_use]
  pub const fn trim_selections() -> Self {
    Self::TrimSelections
  }

  #[must_use]
  pub const fn collapse_selection() -> Self {
    Self::CollapseSelection
  }

  #[must_use]
  pub const fn flip_selections() -> Self {
    Self::FlipSelections
  }

  #[must_use]
  pub const fn expand_selection() -> Self {
    Self::ExpandSelection
  }

  #[must_use]
  pub const fn shrink_selection() -> Self {
    Self::ShrinkSelection
  }

  #[must_use]
  pub const fn select_all_children() -> Self {
    Self::SelectAllChildren
  }

  #[must_use]
  pub const fn select_all_siblings() -> Self {
    Self::SelectAllSiblings
  }

  #[must_use]
  pub const fn select_prev_sibling() -> Self {
    Self::SelectPrevSibling
  }

  #[must_use]
  pub const fn select_next_sibling() -> Self {
    Self::SelectNextSibling
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

  #[must_use]
  pub const fn goto_column(col: usize) -> Self {
    Self::Motion(Motion::Column { col, extend: false })
  }

  #[must_use]
  pub const fn delete_char_forward(count: usize) -> Self {
    Self::DeleteCharForward { count }
  }

  #[must_use]
  pub const fn delete_word_backward(count: usize) -> Self {
    Self::DeleteWordBackward { count }
  }

  #[must_use]
  pub const fn delete_word_forward(count: usize) -> Self {
    Self::DeleteWordForward { count }
  }

  #[must_use]
  pub const fn kill_to_line_start() -> Self {
    Self::KillToLineStart
  }

  #[must_use]
  pub const fn kill_to_line_end() -> Self {
    Self::KillToLineEnd
  }

  #[must_use]
  pub const fn insert_tab() -> Self {
    Self::InsertTab
  }

  #[must_use]
  pub const fn insert_newline() -> Self {
    Self::InsertNewline
  }

  #[must_use]
  pub const fn goto_line_start() -> Self {
    Self::GotoLineStart { extend: false }
  }

  #[must_use]
  pub const fn extend_to_line_start() -> Self {
    Self::GotoLineStart { extend: true }
  }

  #[must_use]
  pub const fn goto_first_nonwhitespace() -> Self {
    Self::GotoFirstNonWhitespace { extend: false }
  }

  #[must_use]
  pub const fn extend_to_first_nonwhitespace() -> Self {
    Self::GotoFirstNonWhitespace { extend: true }
  }

  #[must_use]
  pub const fn goto_line_end() -> Self {
    Self::GotoLineEnd { extend: false }
  }

  #[must_use]
  pub const fn extend_to_line_end() -> Self {
    Self::GotoLineEnd { extend: true }
  }

  #[must_use]
  pub const fn page_up() -> Self {
    Self::PageUp { extend: false }
  }

  #[must_use]
  pub const fn page_down() -> Self {
    Self::PageDown { extend: false }
  }

  #[must_use]
  pub const fn extend_page_up() -> Self {
    Self::PageUp { extend: true }
  }

  #[must_use]
  pub const fn extend_page_down() -> Self {
    Self::PageDown { extend: true }
  }

  #[must_use]
  pub const fn page_cursor_half_up() -> Self {
    Self::PageCursorHalfUp
  }

  #[must_use]
  pub const fn page_cursor_half_down() -> Self {
    Self::PageCursorHalfDown
  }

  #[must_use]
  pub const fn find_next_char() -> Self {
    Self::FindChar {
      direction: Direction::Forward,
      inclusive: true,
      extend:    false,
    }
  }

  #[must_use]
  pub const fn find_till_char() -> Self {
    Self::FindChar {
      direction: Direction::Forward,
      inclusive: false,
      extend:    false,
    }
  }

  #[must_use]
  pub const fn find_prev_char() -> Self {
    Self::FindChar {
      direction: Direction::Backward,
      inclusive: true,
      extend:    false,
    }
  }

  #[must_use]
  pub const fn till_prev_char() -> Self {
    Self::FindChar {
      direction: Direction::Backward,
      inclusive: false,
      extend:    false,
    }
  }

  #[must_use]
  pub const fn extend_next_char() -> Self {
    Self::FindChar {
      direction: Direction::Forward,
      inclusive: true,
      extend:    true,
    }
  }

  #[must_use]
  pub const fn extend_till_char() -> Self {
    Self::FindChar {
      direction: Direction::Forward,
      inclusive: false,
      extend:    true,
    }
  }

  #[must_use]
  pub const fn extend_prev_char() -> Self {
    Self::FindChar {
      direction: Direction::Backward,
      inclusive: true,
      extend:    true,
    }
  }

  #[must_use]
  pub const fn extend_till_prev_char() -> Self {
    Self::FindChar {
      direction: Direction::Backward,
      inclusive: false,
      extend:    true,
    }
  }

  #[must_use]
  pub const fn move_parent_node_end() -> Self {
    Self::ParentNodeEnd { extend: false }
  }

  #[must_use]
  pub const fn extend_parent_node_end() -> Self {
    Self::ParentNodeEnd { extend: true }
  }

  #[must_use]
  pub const fn move_parent_node_start() -> Self {
    Self::ParentNodeStart { extend: false }
  }

  #[must_use]
  pub const fn extend_parent_node_start() -> Self {
    Self::ParentNodeStart { extend: true }
  }

  #[must_use]
  pub const fn delete_selection() -> Self {
    Self::DeleteSelection { yank: true }
  }

  #[must_use]
  pub const fn delete_selection_noyank() -> Self {
    Self::DeleteSelection { yank: false }
  }

  #[must_use]
  pub const fn change_selection() -> Self {
    Self::ChangeSelection { yank: true }
  }

  #[must_use]
  pub const fn change_selection_noyank() -> Self {
    Self::ChangeSelection { yank: false }
  }

  #[must_use]
  pub const fn replace() -> Self {
    Self::Replace
  }

  #[must_use]
  pub const fn replace_with_yanked() -> Self {
    Self::ReplaceWithYanked
  }

  #[must_use]
  pub const fn yank() -> Self {
    Self::Yank
  }

  #[must_use]
  pub const fn paste_after() -> Self {
    Self::Paste { after: true }
  }

  #[must_use]
  pub const fn paste_before() -> Self {
    Self::Paste { after: false }
  }

  #[must_use]
  pub const fn record_macro() -> Self {
    Self::RecordMacro
  }

  #[must_use]
  pub const fn replay_macro() -> Self {
    Self::ReplayMacro
  }

  #[must_use]
  pub const fn repeat_last_motion() -> Self {
    Self::RepeatLastMotion
  }

  #[must_use]
  pub const fn switch_case() -> Self {
    Self::SwitchCase
  }

  #[must_use]
  pub const fn switch_to_uppercase() -> Self {
    Self::SwitchToUppercase
  }

  #[must_use]
  pub const fn switch_to_lowercase() -> Self {
    Self::SwitchToLowercase
  }

  #[must_use]
  pub const fn insert_at_line_start() -> Self {
    Self::InsertAtLineStart
  }

  #[must_use]
  pub const fn insert_at_line_end() -> Self {
    Self::InsertAtLineEnd
  }

  #[must_use]
  pub const fn append_mode() -> Self {
    Self::AppendMode
  }

  #[must_use]
  pub const fn open_below() -> Self {
    Self::OpenBelow
  }

  #[must_use]
  pub const fn open_above() -> Self {
    Self::OpenAbove
  }

  #[must_use]
  pub const fn commit_undo_checkpoint() -> Self {
    Self::CommitUndoCheckpoint
  }

  #[must_use]
  pub const fn copy_selection_on_next_line() -> Self {
    Self::CopySelectionOnNextLine
  }

  #[must_use]
  pub const fn copy_selection_on_prev_line() -> Self {
    Self::CopySelectionOnPrevLine
  }

  #[must_use]
  pub const fn select_all() -> Self {
    Self::SelectAll
  }

  #[must_use]
  pub const fn extend_line_below(count: usize) -> Self {
    Self::ExtendLineBelow { count }
  }

  #[must_use]
  pub const fn extend_line_above(count: usize) -> Self {
    Self::ExtendLineAbove { count }
  }

  #[must_use]
  pub const fn extend_to_line_bounds() -> Self {
    Self::ExtendToLineBounds
  }

  #[must_use]
  pub const fn shrink_to_line_bounds() -> Self {
    Self::ShrinkToLineBounds
  }

  #[must_use]
  pub const fn undo(count: usize) -> Self {
    Self::Undo { count }
  }

  #[must_use]
  pub const fn redo(count: usize) -> Self {
    Self::Redo { count }
  }

  #[must_use]
  pub const fn earlier(count: usize) -> Self {
    Self::Earlier { count }
  }

  #[must_use]
  pub const fn later(count: usize) -> Self {
    Self::Later { count }
  }

  #[must_use]
  pub const fn indent(count: usize) -> Self {
    Self::Indent { count }
  }

  #[must_use]
  pub const fn unindent(count: usize) -> Self {
    Self::Unindent { count }
  }

  #[must_use]
  pub const fn match_brackets() -> Self {
    Self::MatchBrackets
  }

  #[must_use]
  pub const fn surround_add() -> Self {
    Self::SurroundAdd
  }

  #[must_use]
  pub const fn surround_delete(count: usize) -> Self {
    Self::SurroundDelete { count }
  }

  #[must_use]
  pub const fn surround_replace(count: usize) -> Self {
    Self::SurroundReplace { count }
  }

  #[must_use]
  pub const fn select_textobject_around() -> Self {
    Self::SelectTextobjectAround
  }

  #[must_use]
  pub const fn select_textobject_inner() -> Self {
    Self::SelectTextobjectInner
  }

  #[must_use]
  pub const fn goto_prev_diag() -> Self {
    Self::GotoPrevDiag
  }

  #[must_use]
  pub const fn goto_first_diag() -> Self {
    Self::GotoFirstDiag
  }

  #[must_use]
  pub const fn goto_next_diag() -> Self {
    Self::GotoNextDiag
  }

  #[must_use]
  pub const fn goto_last_diag() -> Self {
    Self::GotoLastDiag
  }

  #[must_use]
  pub const fn goto_prev_change() -> Self {
    Self::GotoPrevChange
  }

  #[must_use]
  pub const fn goto_first_change() -> Self {
    Self::GotoFirstChange
  }

  #[must_use]
  pub const fn goto_next_change() -> Self {
    Self::GotoNextChange
  }

  #[must_use]
  pub const fn goto_last_change() -> Self {
    Self::GotoLastChange
  }

  #[must_use]
  pub const fn goto_prev_function() -> Self {
    Self::GotoPrevFunction
  }

  #[must_use]
  pub const fn goto_next_function() -> Self {
    Self::GotoNextFunction
  }

  #[must_use]
  pub const fn goto_prev_class() -> Self {
    Self::GotoPrevClass
  }

  #[must_use]
  pub const fn goto_next_class() -> Self {
    Self::GotoNextClass
  }

  #[must_use]
  pub const fn goto_prev_parameter() -> Self {
    Self::GotoPrevParameter
  }

  #[must_use]
  pub const fn goto_next_parameter() -> Self {
    Self::GotoNextParameter
  }

  #[must_use]
  pub const fn goto_prev_comment() -> Self {
    Self::GotoPrevComment
  }

  #[must_use]
  pub const fn goto_next_comment() -> Self {
    Self::GotoNextComment
  }

  #[must_use]
  pub const fn goto_prev_entry() -> Self {
    Self::GotoPrevEntry
  }

  #[must_use]
  pub const fn goto_next_entry() -> Self {
    Self::GotoNextEntry
  }

  #[must_use]
  pub const fn goto_prev_test() -> Self {
    Self::GotoPrevTest
  }

  #[must_use]
  pub const fn goto_next_test() -> Self {
    Self::GotoNextTest
  }

  #[must_use]
  pub const fn goto_prev_xml_element() -> Self {
    Self::GotoPrevXmlElement
  }

  #[must_use]
  pub const fn goto_next_xml_element() -> Self {
    Self::GotoNextXmlElement
  }

  #[must_use]
  pub const fn goto_prev_paragraph() -> Self {
    Self::GotoPrevParagraph
  }

  #[must_use]
  pub const fn goto_next_paragraph() -> Self {
    Self::GotoNextParagraph
  }

  #[must_use]
  pub const fn add_newline_above() -> Self {
    Self::AddNewlineAbove
  }

  #[must_use]
  pub const fn add_newline_below() -> Self {
    Self::AddNewlineBelow
  }

  #[must_use]
  pub const fn search_selection_detect_word_boundaries() -> Self {
    Self::SearchSelectionDetectWordBoundaries
  }

  #[must_use]
  pub const fn search_selection() -> Self {
    Self::SearchSelection
  }

  #[must_use]
  pub const fn search_next() -> Self {
    Self::SearchNextOrPrev {
      direction: Direction::Forward,
      extend:    false,
      count:     1,
    }
  }

  #[must_use]
  pub const fn search_prev() -> Self {
    Self::SearchNextOrPrev {
      direction: Direction::Backward,
      extend:    false,
      count:     1,
    }
  }

  #[must_use]
  pub const fn extend_search_next() -> Self {
    Self::SearchNextOrPrev {
      direction: Direction::Forward,
      extend:    true,
      count:     1,
    }
  }

  #[must_use]
  pub const fn extend_search_prev() -> Self {
    Self::SearchNextOrPrev {
      direction: Direction::Backward,
      extend:    true,
      count:     1,
    }
  }

  #[must_use]
  pub const fn search() -> Self {
    Self::Search
  }

  #[must_use]
  pub const fn rsearch() -> Self {
    Self::RSearch
  }

  #[must_use]
  pub const fn select_regex() -> Self {
    Self::SelectRegex
  }

  #[must_use]
  pub const fn file_picker() -> Self {
    Self::FilePicker
  }

  #[must_use]
  pub const fn lsp_goto_definition() -> Self {
    Self::LspGotoDefinition
  }

  #[must_use]
  pub const fn lsp_goto_declaration() -> Self {
    Self::LspGotoDeclaration
  }

  #[must_use]
  pub const fn lsp_goto_type_definition() -> Self {
    Self::LspGotoTypeDefinition
  }

  #[must_use]
  pub const fn lsp_goto_implementation() -> Self {
    Self::LspGotoImplementation
  }

  #[must_use]
  pub const fn lsp_hover() -> Self {
    Self::LspHover
  }

  #[must_use]
  pub const fn lsp_references() -> Self {
    Self::LspReferences
  }

  #[must_use]
  pub const fn lsp_document_symbols() -> Self {
    Self::LspDocumentSymbols
  }

  #[must_use]
  pub const fn lsp_workspace_symbols() -> Self {
    Self::LspWorkspaceSymbols
  }

  #[must_use]
  pub const fn lsp_completion() -> Self {
    Self::LspCompletion
  }

  #[must_use]
  pub const fn completion_next() -> Self {
    Self::CompletionNext
  }

  #[must_use]
  pub const fn completion_prev() -> Self {
    Self::CompletionPrev
  }

  #[must_use]
  pub const fn completion_accept() -> Self {
    Self::CompletionAccept
  }

  #[must_use]
  pub const fn completion_cancel() -> Self {
    Self::CompletionCancel
  }

  #[must_use]
  pub const fn completion_docs_scroll_up() -> Self {
    Self::CompletionDocsScrollUp
  }

  #[must_use]
  pub const fn completion_docs_scroll_down() -> Self {
    Self::CompletionDocsScrollDown
  }

  #[must_use]
  pub const fn lsp_signature_help() -> Self {
    Self::LspSignatureHelp
  }

  #[must_use]
  pub const fn lsp_code_actions() -> Self {
    Self::LspCodeActions
  }

  #[must_use]
  pub const fn lsp_format() -> Self {
    Self::LspFormat
  }
}
