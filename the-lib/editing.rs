use ropey::{
  Rope,
  RopeSlice,
};
use smallvec::SmallVec;
use the_stdx::rope::RopeSliceExt;
use thiserror::Error;

use crate::{
  Tendril,
  auto_pairs::{
    self,
    AutoPairs,
    AutoPairsError,
  },
  grapheme::{
    next_grapheme_boundary,
    nth_next_grapheme_boundary,
    nth_prev_grapheme_boundary,
    prev_grapheme_boundary,
  },
  indent::{
    IndentStyle,
    indent_for_newline,
  },
  line_ending::{
    get_line_ending_of_str,
    line_end_char_index,
  },
  movement,
  selection::{
    CursorId,
    CursorRange,
    Range,
    Selection,
    SelectionError,
  },
  surround,
  syntax::{
    IndentQuery,
    IndentationHeuristic,
    Syntax,
  },
  transaction::{
    Replacement,
    ReplacementOverlapPolicy,
    Span,
    Transaction,
    TransactionError,
  },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edit {
  pub transaction: Transaction,
  pub selection:   Selection,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum EditingError {
  #[error(transparent)]
  Transaction(#[from] TransactionError),

  #[error(transparent)]
  Selection(#[from] SelectionError),

  #[error(transparent)]
  AutoPairs(#[from] AutoPairsError),

  #[error(transparent)]
  Surround(#[from] surround::Error),
}

pub type Result<T> = std::result::Result<T, EditingError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenLine {
  Above,
  Below,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InsertNewlineOptions<'a> {
  pub line_ending:      &'a str,
  pub indent:           &'a str,
  pub indent_unit:      &'a str,
  pub expand_auto_pair: bool,
}

impl<'a> InsertNewlineOptions<'a> {
  pub fn new(line_ending: &'a str, indent: &'a str, indent_unit: &'a str) -> Self {
    Self {
      line_ending,
      indent,
      indent_unit,
      expand_auto_pair: true,
    }
  }
}

pub fn insert_text(doc: &Rope, selection: &Selection, text: impl Into<Tendril>) -> Result<Edit> {
  let text = text.into();
  let mut chars = text.chars();
  if let (Some(ch), None) = (chars.next(), chars.next()) {
    let pairs = AutoPairs::default();
    if let Some(edit) = auto_pairs::edit_for_typed_char(doc, selection, ch, &pairs)? {
      return Ok(edit);
    }
  }

  replace_selection_with(doc, selection, Some(text), InsertSelection::PointAfterText)
}

pub fn insert_tab(doc: &Rope, selection: &Selection, indent: &str) -> Result<Edit> {
  insert_text(doc, selection, indent)
}

pub fn insert_newline(
  doc: &Rope,
  selection: &Selection,
  options: InsertNewlineOptions<'_>,
) -> Result<Edit> {
  let contents = doc;
  let text = contents.slice(..);
  let line_ending = options.line_ending;
  let indent_unit = options.indent_unit;

  let mut ranges = SmallVec::with_capacity(selection.len());
  let mut global_offs: isize = 0;
  let mut new_text = String::new();
  let mut last_pos = 0;
  let pairs = AutoPairs::default();
  let mut replacements = Vec::with_capacity(selection.len());

  for cursor in selection.iter() {
    let range = cursor.range;
    let pos = range.head;
    let current_line = text.char_to_line(pos);
    let line_start = text.line_to_char(current_line);
    let mut chars_deleted = 0usize;

    let (from, to, local_offs) =
      if let Some(idx) = text.slice(line_start..pos).last_non_whitespace_char() {
        let first_trailing_whitespace_char = (line_start + idx + 1).clamp(last_pos, pos);
        last_pos = pos;
        chars_deleted = pos - first_trailing_whitespace_char;

        let line = text.line(current_line);
        let indent = match line.first_non_whitespace_char() {
          Some(pos) => line.slice(..pos).to_string(),
          None => String::new(),
        };

        let prev = if pos == 0 {
          ' '
        } else {
          contents.char(pos - 1)
        };
        let curr = contents.get_char(pos).unwrap_or(' ');

        let on_auto_pair = options.expand_auto_pair
          && pairs.pairs().iter().any(|pair| {
            pair.open_last_char() == Some(prev) && pair.close_first_char() == Some(curr)
          });

        if on_auto_pair {
          let inner_indent = indent.clone() + indent_unit;
          new_text.reserve_exact(line_ending.len() * 2 + indent.len() + inner_indent.len());
          new_text.push_str(line_ending);
          new_text.push_str(&inner_indent);
          let local_offs = new_text.chars().count();
          new_text.push_str(line_ending);
          new_text.push_str(&indent);
          (
            first_trailing_whitespace_char,
            pos,
            local_offs as isize - chars_deleted as isize,
          )
        } else {
          new_text.reserve_exact(line_ending.len() + indent.len());
          new_text.push_str(line_ending);
          new_text.push_str(&indent);
          (
            first_trailing_whitespace_char,
            pos,
            new_text.chars().count() as isize - chars_deleted as isize,
          )
        }
      } else {
        new_text.push_str(line_ending);
        (line_start, line_start, new_text.chars().count() as isize)
      };

    let new_range = if pos > range.anchor {
      Range::new(
        (range.anchor as isize + global_offs) as usize,
        (range.head as isize + local_offs + global_offs) as usize,
      )
    } else {
      Range::new(
        (range.anchor as isize + local_offs + global_offs) as usize,
        (range.head as isize + local_offs + global_offs) as usize,
      )
    };

    ranges.push(new_range);
    global_offs += new_text.chars().count() as isize - chars_deleted as isize;

    replacements.push(Replacement::replace(
      Span {
        start: from,
        end:   to,
      },
      Tendril::from(new_text.as_str()),
    ));
    new_text.clear();
  }

  let transaction = transaction_or_empty(doc.len_chars(), replacements)?;
  let cursor_ids: SmallVec<[CursorId; 1]> = selection.cursor_ids().iter().copied().collect();
  let selection = Selection::new_with_ids(ranges, cursor_ids)?;
  Ok(Edit {
    transaction,
    selection,
  })
}

pub fn open_line(
  doc: &Rope,
  selection: &Selection,
  direction: OpenLine,
  line_ending: &str,
  count: usize,
  indent_query: Option<&IndentQuery>,
  syntax: Option<&Syntax>,
  indent_style: &IndentStyle,
  tab_width: usize,
) -> Result<Edit> {
  let contents = doc;
  let text = contents.slice(..);
  let count = count.max(1);

  let mut offs: usize = 0;
  let mut ranges = SmallVec::with_capacity(selection.len());
  let indent_heuristic = IndentationHeuristic::default();
  let mut replacements = Vec::with_capacity(selection.len());

  for cursor in selection.iter() {
    let range = cursor.range;
    let curr_line_num = text.char_to_line(match direction {
      OpenLine::Below => {
        if range.is_empty() {
          range.cursor(text)
        } else {
          prev_grapheme_boundary(text, range.to())
        }
      },
      OpenLine::Above => range.from(),
    });

    let next_new_line_num = match direction {
      OpenLine::Below => curr_line_num + 1,
      OpenLine::Above => curr_line_num,
    };

    let above_next_new_line_num = next_new_line_num.saturating_sub(1);
    let continue_comment_token: Option<&String> = None;

    let (above_next_line_end_index, above_next_line_end_width) = if next_new_line_num == 0 {
      (0, 0)
    } else {
      (
        line_end_char_index(&text, above_next_new_line_num),
        line_ending.chars().count(),
      )
    };

    let line = text.line(curr_line_num);
    let indent = match line.first_non_whitespace_char() {
      Some(pos) if continue_comment_token.is_some() => line.slice(..pos).to_string(),
      _ => {
        indent_for_newline_safe(
          indent_query,
          syntax,
          &indent_heuristic,
          indent_style,
          tab_width,
          text,
          above_next_new_line_num,
          above_next_line_end_index,
          curr_line_num,
        )
      },
    };

    let indent_len = indent.chars().count();
    let mut insert = String::with_capacity(1 + indent_len);

    if direction == OpenLine::Above && next_new_line_num == 0 {
      insert.push_str(&indent);
      if let Some(token) = continue_comment_token {
        insert.push_str(token);
        insert.push(' ');
      }
      insert.push_str(line_ending);
    } else {
      insert.push_str(line_ending);
      insert.push_str(&indent);
      if let Some(token) = continue_comment_token {
        insert.push_str(token);
        insert.push(' ');
      }
    }

    let insert = insert.repeat(count);

    let pos = offs + above_next_line_end_index + above_next_line_end_width;
    let comment_len = continue_comment_token
      .map(|token| token.len() + 1)
      .unwrap_or_default();
    for i in 0..count {
      ranges.push(Range::point(
        pos
          + (i * (line_ending.chars().count() + indent_len + comment_len))
          + indent_len
          + comment_len,
      ));
    }

    offs += insert.chars().count();
    replacements.push(Replacement::insert(above_next_line_end_index, insert));
  }

  let transaction = transaction_or_empty(doc.len_chars(), replacements)?;
  let cursor_ids: SmallVec<[CursorId; 1]> = selection.cursor_ids().iter().copied().collect();
  let selection = if cursor_ids.len() == ranges.len() {
    Selection::new_with_ids(ranges, cursor_ids)?
  } else {
    Selection::new(ranges)?
  };
  Ok(Edit {
    transaction,
    selection,
  })
}

pub fn delete_backward(doc: &Rope, selection: &Selection, count: usize) -> Result<Edit> {
  let text = doc.slice(..);
  delete_by_selection(doc, selection, |range| {
    if range.is_empty() {
      let end = range.head;
      (nth_prev_grapheme_boundary(text, end, count.max(1)), end)
    } else {
      (range.from(), range.to())
    }
  })
}

pub fn delete_forward(doc: &Rope, selection: &Selection, count: usize) -> Result<Edit> {
  let text = doc.slice(..);
  delete_by_selection(doc, selection, |range| {
    if range.is_empty() {
      let start = range.head;
      (start, nth_next_grapheme_boundary(text, start, count.max(1)))
    } else {
      (range.from(), range.to())
    }
  })
}

pub fn delete_selection(doc: &Rope, selection: &Selection) -> Result<Edit> {
  let text = doc.slice(..);
  delete_by_selection(doc, selection, |range| {
    if range.is_empty() {
      (
        range.from(),
        nth_next_grapheme_boundary(text, range.from(), 1),
      )
    } else {
      (range.from(), range.to())
    }
  })
}

pub fn delete_word_backward(doc: &Rope, selection: &Selection, count: usize) -> Result<Edit> {
  let text = doc.slice(..);
  delete_by_selection(doc, selection, |range| {
    let end = range.head;
    let target = movement::move_prev_word_start(text, Range::point(end), count.max(1));
    (target.from(), end)
  })
}

pub fn delete_word_forward(doc: &Rope, selection: &Selection, count: usize) -> Result<Edit> {
  let text = doc.slice(..);
  delete_by_selection(doc, selection, |range| {
    let start = range.head;
    let target = movement::move_next_word_end(text, Range::point(start), count.max(1));
    (start, target.to())
  })
}

pub fn kill_to_line_start(doc: &Rope, selection: &Selection) -> Result<Edit> {
  let text = doc.slice(..);
  delete_by_selection(doc, selection, |range| {
    let cursor = range.head;
    let line = text.char_to_line(cursor);
    let line_start = text.line_to_char(line);
    if cursor == line_start && line > 0 {
      let prev_end = line_end_char_index(&text, line - 1);
      (prev_end, cursor)
    } else {
      (line_start, cursor)
    }
  })
}

pub fn kill_to_line_end(doc: &Rope, selection: &Selection) -> Result<Edit> {
  let text = doc.slice(..);
  delete_by_selection(doc, selection, |range| {
    let cursor = range.head;
    let line = text.char_to_line(cursor);
    let line_end = line_end_char_index(&text, line);
    if cursor == line_end && line + 1 < text.len_lines() {
      (cursor, text.line_to_char(line + 1))
    } else {
      (cursor, line_end)
    }
  })
}

pub fn replace_selection(doc: &Rope, selection: &Selection, replacement: &str) -> Result<Edit> {
  replace_selection_with(
    doc,
    selection,
    Some(Tendril::from(replacement)),
    InsertSelection::SelectInserted,
  )
}

pub fn switch_case(doc: &Rope, selection: &Selection) -> Result<Edit> {
  replace_selected_text(doc, selection, |text| {
    text
      .chars()
      .flat_map(|ch| {
        if ch.is_lowercase() {
          CaseSwitcher::Upper(ch.to_uppercase())
        } else if ch.is_uppercase() {
          CaseSwitcher::Lower(ch.to_lowercase())
        } else {
          CaseSwitcher::Keep(Some(ch))
        }
      })
      .collect()
  })
}

pub fn switch_to_uppercase(doc: &Rope, selection: &Selection) -> Result<Edit> {
  replace_selected_text(doc, selection, |text| text.to_uppercase().into())
}

pub fn switch_to_lowercase(doc: &Rope, selection: &Selection) -> Result<Edit> {
  replace_selected_text(doc, selection, |text| text.to_lowercase().into())
}

pub fn indent(doc: &Rope, selection: &Selection, indent_unit: &str, count: usize) -> Result<Edit> {
  let text = doc.slice(..);
  let indent_text = Tendril::from(indent_unit.repeat(count.max(1)));
  let mut replacements = Vec::new();
  for (start, end) in selection.line_ranges(text) {
    for line in start..=end {
      if text.line(line).chars().all(|ch| ch.is_whitespace()) {
        continue;
      }
      let pos = text.line_to_char(line);
      replacements.push(Replacement::insert(pos, indent_text.clone()));
    }
  }
  edit_and_map_selection(doc, selection, replacements)
}

pub fn surround_add(
  doc: &Rope,
  selection: &Selection,
  open: impl Into<Tendril>,
  close: impl Into<Tendril>,
) -> Result<Edit> {
  let open = open.into();
  let close = close.into();
  let surround_len = open.chars().count() + close.chars().count();
  let mut replacements = Vec::with_capacity(selection.len() * 2);
  let mut cursors: SmallVec<[CursorRange; 1]> = SmallVec::with_capacity(selection.len());
  let mut offset = 0usize;

  for cursor in selection.iter() {
    let range = cursor.range;
    replacements.push(Replacement::insert(range.from(), open.clone()));
    replacements.push(Replacement::insert(range.to(), close.clone()));
    cursors.push(CursorRange {
      id:    cursor.id,
      range: Range::new(offset + range.from(), offset + range.to() + surround_len)
        .with_direction(range.direction()),
    });
    offset += surround_len;
  }

  build_edit(doc.len_chars(), replacements, cursors)
}

pub fn surround_delete(
  doc: &Rope,
  selection: &Selection,
  syntax: Option<&Syntax>,
  surround_ch: Option<char>,
  count: usize,
) -> Result<Edit> {
  let text = doc.slice(..);
  let mut positions = surround::get_surround_pos(syntax, text, selection, surround_ch, count)?;
  positions.sort_unstable();

  let replacements = positions
    .into_iter()
    .flat_map(|(open, close)| {
      let open_end = next_grapheme_boundary(text, open);
      let close_end = next_grapheme_boundary(text, close);
      [
        Replacement::delete(Span {
          start: open,
          end:   open_end,
        }),
        Replacement::delete(Span {
          start: close,
          end:   close_end,
        }),
      ]
    })
    .collect();

  edit_and_map_selection(doc, selection, replacements)
}

pub fn surround_replace(
  doc: &Rope,
  selection: &Selection,
  syntax: Option<&Syntax>,
  target: char,
  open: impl Into<Tendril>,
  close: impl Into<Tendril>,
  count: usize,
) -> Result<Edit> {
  let text = doc.slice(..);
  let positions = surround::get_surround_pos(syntax, text, selection, Some(target), count)?;
  let open = open.into();
  let close = close.into();
  let mut replacements = Vec::with_capacity(positions.len() * 2);

  for (open_pos, close_pos) in positions {
    let open_end = next_grapheme_boundary(text, open_pos);
    let close_end = next_grapheme_boundary(text, close_pos);
    replacements.push(Replacement::replace(
      Span {
        start: open_pos,
        end:   open_end,
      },
      open.clone(),
    ));
    replacements.push(Replacement::replace(
      Span {
        start: close_pos,
        end:   close_end,
      },
      close.clone(),
    ));
  }

  replacements.sort_by_key(|replacement| replacement.span.start);
  edit_and_map_selection(doc, selection, replacements)
}

pub fn paste(
  doc: &Rope,
  selection: &Selection,
  values: &[String],
  after: bool,
  normal_mode: bool,
  line_ending: &str,
) -> Result<Option<Edit>> {
  if values.is_empty() {
    return Ok(None);
  }

  let text = doc.slice(..);
  let linewise = values
    .iter()
    .any(|value| get_line_ending_of_str(value).is_some());

  let normalize_line_endings = |value: &str| {
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars().peekable();
    while let Some(ch) = chars.next() {
      match ch {
        '\r' => {
          if chars.peek() == Some(&'\n') {
            chars.next();
          }
          out.push_str(line_ending);
        },
        '\n' => out.push_str(line_ending),
        _ => out.push(ch),
      }
    }
    Tendril::from(out.as_str())
  };

  let last = normalize_line_endings(values.last().expect("values is non-empty"));
  let mut values = values
    .iter()
    .map(|value| normalize_line_endings(value))
    .chain(std::iter::repeat(last));
  let mut offset = 0usize;
  let mut replacements = Vec::with_capacity(selection.len());
  let mut cursors: SmallVec<[CursorRange; 1]> = SmallVec::with_capacity(selection.len());

  for cursor in selection.iter() {
    let range = cursor.range;
    let pos = if linewise {
      if after {
        let line = range.line_range(text).1;
        text.line_to_char((line + 1).min(text.len_lines()))
      } else {
        text.line_to_char(text.char_to_line(range.from()))
      }
    } else if after {
      if normal_mode && range.is_empty() {
        next_grapheme_boundary(text, range.to())
      } else {
        range.to()
      }
    } else {
      range.from()
    };

    let value = values.next().expect("infinite values iterator");
    let value_len = value.chars().count();
    let anchor = offset + pos;
    replacements.push(Replacement::insert(pos, value));
    cursors.push(CursorRange {
      id:    cursor.id,
      range: Range::new(anchor, anchor + value_len).with_direction(range.direction()),
    });
    offset += value_len;
  }

  Ok(Some(build_edit(doc.len_chars(), replacements, cursors)?))
}

pub fn unindent(
  doc: &Rope,
  selection: &Selection,
  indent_width: usize,
  tab_width: usize,
) -> Result<Edit> {
  let text = doc.slice(..);
  let mut replacements = Vec::new();
  for (start, end) in selection.line_ranges(text) {
    for line_idx in start..=end {
      let line = text.line(line_idx);
      let mut width = 0usize;
      let mut pos = 0usize;
      for ch in line.chars() {
        match ch {
          ' ' => width += 1,
          '\t' => width = (width / tab_width + 1) * tab_width,
          _ => break,
        }
        pos += 1;
        if width >= indent_width {
          break;
        }
      }
      if pos > 0 {
        let start = text.line_to_char(line_idx);
        replacements.push(Replacement::delete(Span {
          start,
          end: start + pos,
        }));
      }
    }
  }
  edit_and_map_selection(doc, selection, replacements)
}

pub fn yank_selection(doc: &Rope, selection: &Selection) -> Vec<Tendril> {
  selection
    .fragments(doc.slice(..))
    .map(|fragment| Tendril::from(fragment.into_owned()))
    .collect()
}

pub fn yank_main_selection(doc: &Rope, selection: &Selection) -> Vec<Tendril> {
  selection
    .ranges()
    .first()
    .map(|range| vec![Tendril::from(range.fragment(doc.slice(..)).into_owned())])
    .unwrap_or_default()
}

pub fn yank_deleted_text(doc: &Rope, selection: &Selection) -> Vec<Tendril> {
  let text = doc.slice(..);
  selection
    .ranges()
    .iter()
    .map(|range| {
      let (from, to) = if range.is_empty() {
        (
          range.from(),
          nth_next_grapheme_boundary(text, range.from(), 1),
        )
      } else {
        (range.from(), range.to())
      };
      Tendril::from(text.slice(from..to).to_string())
    })
    .collect()
}

#[derive(Debug, Clone, Copy)]
enum InsertSelection {
  PointAfterText,
  SelectInserted,
}

fn replace_selection_with(
  doc: &Rope,
  selection: &Selection,
  text: Option<Tendril>,
  selection_mode: InsertSelection,
) -> Result<Edit> {
  replace_selection_with_len(doc.len_chars(), selection, text, selection_mode)
}

fn replace_selection_with_len(
  len: usize,
  selection: &Selection,
  text: Option<Tendril>,
  selection_mode: InsertSelection,
) -> Result<Edit> {
  let inserted_len = text.as_ref().map(|text| text.chars().count()).unwrap_or(0);
  let mut replacements = Vec::with_capacity(selection.ranges().len());
  let mut cursors: SmallVec<[CursorRange; 1]> = SmallVec::with_capacity(selection.ranges().len());
  let mut offset: isize = 0;

  for cursor in selection.iter() {
    let start = cursor.range.from();
    let end = cursor.range.to();
    let new_start = (start as isize + offset) as usize;
    let new_end = new_start + inserted_len;
    replacements.push(Replacement {
      span: Span { start, end },
      text: text.clone(),
    });
    cursors.push(CursorRange {
      id:    cursor.id,
      range: match selection_mode {
        InsertSelection::PointAfterText => Range::point(new_end),
        InsertSelection::SelectInserted => {
          Range::new(new_start, new_end).with_direction(cursor.range.direction())
        },
      },
    });
    offset += inserted_len as isize - end.saturating_sub(start) as isize;
  }

  build_edit(len, replacements, cursors)
}

fn edit_and_map_selection(
  doc: &Rope,
  selection: &Selection,
  replacements: Vec<Replacement>,
) -> Result<Edit> {
  let transaction = transaction_or_empty(doc.len_chars(), replacements)?;
  let selection = selection.clone().map(transaction.changes())?;
  Ok(Edit {
    transaction,
    selection,
  })
}

fn delete_by_selection(
  doc: &Rope,
  selection: &Selection,
  mut span_for_range: impl FnMut(Range) -> (usize, usize),
) -> Result<Edit> {
  delete_by_selection_len(doc.len_chars(), selection, |range| span_for_range(range))
}

fn delete_by_selection_len(
  len: usize,
  selection: &Selection,
  mut span_for_range: impl FnMut(Range) -> (usize, usize),
) -> Result<Edit> {
  let mut replacements = Vec::with_capacity(selection.ranges().len());
  let mut cursors: SmallVec<[CursorRange; 1]> = SmallVec::with_capacity(selection.ranges().len());
  let mut offset: isize = 0;

  for cursor in selection.iter() {
    let (start, end) = span_for_range(*cursor.range);
    replacements.push(Replacement::delete(Span { start, end }));
    let pos = (start as isize + offset) as usize;
    cursors.push(CursorRange {
      id:    cursor.id,
      range: Range::point(pos),
    });
    offset -= end.saturating_sub(start) as isize;
  }

  build_edit(len, replacements, cursors)
}

fn replace_selected_text(
  doc: &Rope,
  selection: &Selection,
  mut f: impl FnMut(&str) -> Tendril,
) -> Result<Edit> {
  let text = doc.slice(..);
  let mut replacements = Vec::with_capacity(selection.ranges().len());
  let mut cursors: SmallVec<[CursorRange; 1]> = SmallVec::with_capacity(selection.ranges().len());
  let mut offset: isize = 0;

  for cursor in selection.iter() {
    let from = cursor.range.from();
    let to = if cursor.range.is_empty() {
      nth_next_grapheme_boundary(text, from, 1)
    } else {
      cursor.range.to()
    };
    let replacement = f(&text.slice(from..to).to_string());
    let replacement_len = replacement.chars().count();
    let new_from = (from as isize + offset) as usize;
    replacements.push(Replacement::replace(
      Span {
        start: from,
        end:   to,
      },
      replacement,
    ));
    cursors.push(CursorRange {
      id:    cursor.id,
      range: Range::new(new_from, new_from + replacement_len)
        .with_direction(cursor.range.direction()),
    });
    offset += replacement_len as isize - to.saturating_sub(from) as isize;
  }

  build_edit(doc.len_chars(), replacements, cursors)
}

fn build_edit(
  len: usize,
  replacements: Vec<Replacement>,
  cursors: SmallVec<[CursorRange; 1]>,
) -> Result<Edit> {
  let transaction = transaction_or_empty(len, replacements)?;
  let selection = Selection::from_cursors(cursors)?;
  Ok(Edit {
    transaction,
    selection,
  })
}

fn transaction_or_empty(
  len: usize,
  replacements: Vec<Replacement>,
) -> crate::transaction::Result<Transaction> {
  if replacements.is_empty() {
    return Ok(Transaction::empty(len));
  }
  Transaction::from_replacements(len, replacements, ReplacementOverlapPolicy::Reject)
}

fn indent_for_newline_safe(
  indent_query: Option<&IndentQuery>,
  syntax: Option<&Syntax>,
  indent_heuristic: &IndentationHeuristic,
  indent_style: &IndentStyle,
  tab_width: usize,
  text: RopeSlice,
  line_before: usize,
  line_before_end_pos: usize,
  current_line: usize,
) -> String {
  match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
    indent_for_newline(
      indent_query,
      syntax,
      indent_heuristic,
      indent_style,
      tab_width,
      text,
      line_before,
      line_before_end_pos,
      current_line,
    )
  })) {
    Ok(indent) => indent,
    Err(_) => {
      let line = text.line(current_line);
      line
        .first_non_whitespace_char()
        .map(|pos| line.slice(..pos).to_string())
        .unwrap_or_default()
    },
  }
}

enum CaseSwitcher {
  Upper(std::char::ToUppercase),
  Lower(std::char::ToLowercase),
  Keep(Option<char>),
}

impl Iterator for CaseSwitcher {
  type Item = char;

  fn next(&mut self) -> Option<Self::Item> {
    match self {
      CaseSwitcher::Upper(chars) => chars.next(),
      CaseSwitcher::Lower(chars) => chars.next(),
      CaseSwitcher::Keep(ch) => ch.take(),
    }
  }
}

#[cfg(test)]
mod tests {
  use ropey::Rope;

  use super::*;

  #[test]
  fn insert_text_replaces_each_selection_and_preserves_cursor_ids() {
    let doc = Rope::from("one two");
    let selection = Selection::single(0, 3);
    let id = selection.cursor_ids()[0];

    let edit = insert_text(&doc, &selection, "ONE").unwrap();
    let next = edit.transaction.apply_to(&doc).unwrap();

    assert_eq!(next.to_string(), "ONE two");
    assert_eq!(edit.selection.cursor_ids(), &[id]);
    assert_eq!(edit.selection.ranges(), &[Range::point(3)]);
  }

  #[test]
  fn insert_text_uses_auto_pairs_for_typed_chars() {
    let doc = Rope::from("");
    let selection = Selection::point(0);

    let edit = insert_text(&doc, &selection, "(").unwrap();
    let next = edit.transaction.apply_to(&doc).unwrap();

    assert_eq!(next.to_string(), "()");
    assert_eq!(edit.selection.ranges(), &[Range::point(1)]);
  }

  #[test]
  fn insert_newline_expands_inside_auto_pair() {
    let doc = Rope::from("()");
    let selection = Selection::point(1);

    let edit = insert_newline(&doc, &selection, InsertNewlineOptions::new("\n", "", "  ")).unwrap();
    let next = edit.transaction.apply_to(&doc).unwrap();

    assert_eq!(next.to_string(), "(\n  \n)");
    assert_eq!(edit.selection.ranges(), &[Range::point(4)]);
  }

  #[test]
  fn delete_backward_deletes_previous_grapheme() {
    let doc = Rope::from("ab");
    let selection = Selection::point(2);

    let edit = delete_backward(&doc, &selection, 1).unwrap();
    let next = edit.transaction.apply_to(&doc).unwrap();

    assert_eq!(next.to_string(), "a");
    assert_eq!(edit.selection.ranges(), &[Range::point(1)]);
  }

  #[test]
  fn open_line_inserts_at_line_end_and_lands_on_opened_line() {
    let doc = Rope::from("foo\nbar");
    let selection = Selection::point(1);

    let edit = open_line(
      &doc,
      &selection,
      OpenLine::Below,
      "\n",
      1,
      None,
      None,
      &IndentStyle::Spaces(2),
      4,
    )
    .unwrap();
    let next = edit.transaction.apply_to(&doc).unwrap();

    assert_eq!(next.to_string(), "foo\n\nbar");
    assert_eq!(edit.selection.ranges(), &[Range::point(4)]);
  }

  #[test]
  fn open_line_copies_indent() {
    let doc = Rope::from("  foo\nbar");
    let selection = Selection::point(2);

    let edit = open_line(
      &doc,
      &selection,
      OpenLine::Below,
      "\n",
      1,
      None,
      None,
      &IndentStyle::Spaces(2),
      4,
    )
    .unwrap();
    let next = edit.transaction.apply_to(&doc).unwrap();

    assert_eq!(next.to_string(), "  foo\n  \nbar");
    assert_eq!(edit.selection.ranges(), &[Range::point(8)]);
  }

  #[test]
  fn open_above_first_line_keeps_cursor_before_inserted_newline() {
    let doc = Rope::from("  foo");
    let selection = Selection::point(2);

    let edit = open_line(
      &doc,
      &selection,
      OpenLine::Above,
      "\n",
      1,
      None,
      None,
      &IndentStyle::Spaces(2),
      4,
    )
    .unwrap();
    let next = edit.transaction.apply_to(&doc).unwrap();

    assert_eq!(next.to_string(), "  \n  foo");
    assert_eq!(edit.selection.ranges(), &[Range::point(2)]);
  }

  #[test]
  fn open_below_non_empty_selection_uses_last_selected_grapheme_line() {
    let doc = Rope::from("foo\nbar");
    let selection = Selection::single(0, 4);

    let edit = open_line(
      &doc,
      &selection,
      OpenLine::Below,
      "\n",
      1,
      None,
      None,
      &IndentStyle::Spaces(2),
      4,
    )
    .unwrap();
    let next = edit.transaction.apply_to(&doc).unwrap();

    assert_eq!(next.to_string(), "foo\n\nbar");
    assert_eq!(edit.selection.ranges(), &[Range::point(4)]);
  }

  #[test]
  fn indent_and_unindent_are_edit_planners() {
    let doc = Rope::from("a\n  b\n");
    let selection = Selection::single(0, doc.len_chars());

    let edit = indent(&doc, &selection, "  ", 1).unwrap();
    let indented = edit.transaction.apply_to(&doc).unwrap();
    assert_eq!(indented.to_string(), "  a\n    b\n");

    let edit = unindent(&indented, &edit.selection, 2, 4).unwrap();
    let unindented = edit.transaction.apply_to(&indented).unwrap();
    assert_eq!(unindented.to_string(), "a\n  b\n");
  }
}
