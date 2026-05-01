use std::num::NonZeroU64;

use ropey::RopeSlice;

use crate::{
  grapheme::{next_grapheme_boundary, prev_grapheme_boundary},
  selection::{CursorId, CursorRange, Range, Selection},
  transaction::{Replacement, ReplacementOverlapPolicy, Span, Transaction},
  Tendril,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edit {
  pub transaction: Transaction,
  pub selection: Selection,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum TextCommand {
  MoveCharLeft { count: usize },
  MoveCharRight { count: usize },
  MoveVisualLineUp { count: usize, preferred_column: Option<usize> },
  MoveVisualLineDown { count: usize, preferred_column: Option<usize> },
  GotoLineStart,
  GotoLineEnd,
  GotoDocumentStart,
  GotoDocumentEnd,
  InsertText { text: String },
  InsertNewline { indent: String, indent_unit: String },
  DeleteBackward,
  DeleteForward,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextCommandEffect {
  Selection(Selection),
  Edit(Edit),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextCommandState {
  pub preferred_column: Option<usize>,
}

pub struct TextCommandOutput {
  pub effect: TextCommandEffect,
  pub state:  TextCommandState,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SelectionSpec {
  pub ranges: Vec<CursorRangeSpec>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CursorRangeSpec {
  pub id: Option<u64>,
  pub anchor: usize,
  pub head: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TransactionSpec {
  pub replacements: Vec<ReplacementSpec>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ReplacementSpec {
  pub start: usize,
  pub end: usize,
  pub text: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SurfaceSnapshot {
  pub text: String,
  pub selection: SelectionSpec,
  pub active_cursor_id: Option<u64>,
}

pub fn execute_text_command(
  text: RopeSlice,
  selection: &Selection,
  command: TextCommand,
) -> crate::transaction::Result<TextCommandOutput> {
  let range = selection.ranges().first().copied().unwrap_or_else(|| Range::point(0));
  let len = text.len_chars();
  let output = match command {
    TextCommand::MoveCharLeft { count } => {
      let mut pos = range.cursor(text);
      for _ in 0..count {
        pos = prev_grapheme_boundary(text, pos);
      }
      selection_output(selection, range, pos, None)
    },
    TextCommand::MoveCharRight { count } => {
      let mut pos = range.cursor(text);
      for _ in 0..count {
        pos = next_grapheme_boundary(text, pos);
      }
      selection_output(selection, range, pos, None)
    },
    TextCommand::MoveVisualLineUp { count, preferred_column } => {
      vertical_output(text, selection, range, -(count as isize), preferred_column)
    },
    TextCommand::MoveVisualLineDown { count, preferred_column } => {
      vertical_output(text, selection, range, count as isize, preferred_column)
    },
    TextCommand::GotoLineStart => {
      let line = text.char_to_line(range.cursor(text));
      selection_output(selection, range, text.line_to_char(line), None)
    },
    TextCommand::GotoLineEnd => {
      let line = text.char_to_line(range.cursor(text));
      selection_output(selection, range, line_content_end(text, line), None)
    },
    TextCommand::GotoDocumentStart => selection_output(selection, range, 0, None),
    TextCommand::GotoDocumentEnd => selection_output(selection, range, len, None),
    TextCommand::InsertText { text: inserted } => edit_output(text, selection, range.from(), range.to(), inserted)?,
    TextCommand::InsertNewline { indent, indent_unit } => newline_output(text, selection, range, indent, indent_unit)?,
    TextCommand::DeleteBackward => {
      let (start, end) = if range.is_empty() {
        (prev_grapheme_boundary(text, range.head), range.head)
      } else {
        (range.from(), range.to())
      };
      edit_output(text, selection, start, end, String::new())?
    },
    TextCommand::DeleteForward => {
      let (start, end) = if range.is_empty() {
        (range.head, next_grapheme_boundary(text, range.head))
      } else {
        (range.from(), range.to())
      };
      edit_output(text, selection, start, end, String::new())?
    },
  };
  Ok(output)
}

fn selection_output(
  selection: &Selection,
  old_primary: Range,
  pos: usize,
  preferred_column: Option<usize>,
) -> TextCommandOutput {
  TextCommandOutput {
    effect: TextCommandEffect::Selection(selection_with_primary(selection, old_primary, Range::point(pos))),
    state:  TextCommandState { preferred_column },
  }
}

fn edit_output(
  text: RopeSlice,
  _selection: &Selection,
  start: usize,
  end: usize,
  inserted: String,
) -> crate::transaction::Result<TextCommandOutput> {
  let inserted_len = inserted.chars().count();
  let transaction = Transaction::from_replacements(
    text.len_chars(),
    [Replacement {
      span: Span { start, end },
      text: (!inserted.is_empty()).then(|| Tendril::from(inserted)),
    }],
    ReplacementOverlapPolicy::Reject,
  )?;
  let next = start + inserted_len;
  Ok(TextCommandOutput {
    effect: TextCommandEffect::Edit(Edit {
      transaction,
      selection: Selection::point(next),
    }),
    state: TextCommandState { preferred_column: None },
  })
}

fn newline_output(
  text: RopeSlice,
  _selection: &Selection,
  range: Range,
  indent: String,
  indent_unit: String,
) -> crate::transaction::Result<TextCommandOutput> {
  let pos = range.cursor(text);
  let line = text.char_to_line(pos);
  let line_start = text.line_to_char(line);
  let trailing_start = text
    .slice(line_start..pos)
    .chars()
    .enumerate()
    .filter_map(|(idx, ch)| (!ch.is_whitespace()).then_some(line_start + idx + 1))
    .last()
    .unwrap_or(line_start);
  let prev = if pos == 0 { ' ' } else { text.char(pos - 1) };
  let curr = if pos < text.len_chars() { text.char(pos) } else { ' ' };
  let base = format!("\n{indent}");
  let inserted = if is_auto_pair(prev, curr) {
    format!("{base}{indent_unit}\n{indent}")
  } else {
    base.clone()
  };
  let cursor = trailing_start + base.chars().count();
  let transaction = Transaction::from_replacements(
    text.len_chars(),
    [Replacement {
      span: Span { start: trailing_start, end: pos },
      text: Some(Tendril::from(inserted)),
    }],
    ReplacementOverlapPolicy::Reject,
  )?;
  Ok(TextCommandOutput {
    effect: TextCommandEffect::Edit(Edit { transaction, selection: Selection::point(cursor) }),
    state: TextCommandState { preferred_column: None },
  })
}

fn vertical_output(
  text: RopeSlice,
  selection: &Selection,
  range: Range,
  delta: isize,
  preferred_column: Option<usize>,
) -> TextCommandOutput {
  let pos = range.cursor(text);
  let line = text.char_to_line(pos);
  let col = pos - text.line_to_char(line);
  let preferred_column = preferred_column.unwrap_or(col);
  let target_line = (line as isize + delta).clamp(0, text.len_lines().saturating_sub(1) as isize) as usize;
  let line_start = text.line_to_char(target_line);
  let line_end = line_content_end(text, target_line);
  selection_output(selection, range, line_start + preferred_column.min(line_end - line_start), Some(preferred_column))
}

fn line_content_end(text: RopeSlice, line: usize) -> usize {
  let line_start = text.line_to_char(line);
  let mut end = line_start + text.line(line).len_chars();
  while end > line_start {
    let ch = text.char(end - 1);
    if ch != '\n' && ch != '\r' {
      break;
    }
    end -= 1;
  }
  end
}

fn selection_with_primary(selection: &Selection, old_primary: Range, new_primary: Range) -> Selection {
  let ranges = selection
    .ranges()
    .iter()
    .map(|range| if *range == old_primary { new_primary } else { *range })
    .collect::<Vec<_>>();
  Selection::from_cursors(
    ranges
      .into_iter()
      .zip(selection.cursor_ids().iter().copied())
      .map(|(range, id)| CursorRange { id, range })
      .collect(),
  )
  .unwrap_or_else(|_| Selection::point(new_primary.head))
}

fn is_auto_pair(prev: char, curr: char) -> bool {
  matches!((prev, curr), ('(', ')') | ('[', ']') | ('{', '}'))
}

pub fn selection_to_spec(selection: &Selection) -> SelectionSpec {
  SelectionSpec {
    ranges: selection
      .iter()
      .map(|cursor| CursorRangeSpec {
        id: Some(cursor.id.get()),
        anchor: cursor.range.anchor,
        head: cursor.range.head,
      })
      .collect(),
  }
}

pub fn selection_from_spec(spec: SelectionSpec) -> crate::selection::Result<Selection> {
  let cursors = spec
    .ranges
    .into_iter()
    .map(|range| CursorRange {
      id: range
        .id
        .and_then(NonZeroU64::new)
        .map(CursorId::new)
        .unwrap_or_else(CursorId::fresh),
      range: Range::new(range.anchor, range.head),
    })
    .collect();
  Selection::from_cursors(cursors)
}

pub fn transaction_from_spec(len: usize, spec: TransactionSpec) -> crate::transaction::Result<Transaction> {
  Transaction::from_replacements(
    len,
    spec.replacements.into_iter().map(|replacement| Replacement {
      span: Span {
        start: replacement.start,
        end: replacement.end,
      },
      text: replacement.text.map(Tendril::from),
    }),
    ReplacementOverlapPolicy::Reject,
  )
}
