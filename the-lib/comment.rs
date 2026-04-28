use ropey::{
  Rope,
  RopeSlice,
};
use smallvec::SmallVec;
use the_stdx::rope::RopeSliceExt;
use thiserror::Error;

use crate::{
  Tendril,
  edit::Edit,
  selection::{
    Range,
    Selection,
    SelectionError,
  },
  syntax::BlockCommentToken,
  transaction::{
    Replacement,
    ReplacementOverlapPolicy,
    Span,
    Transaction,
    TransactionError,
  },
};

const DEFAULT_COMMENT_TOKEN: &str = "#";

#[derive(Debug, Error)]
pub enum CommentError {
  #[error(transparent)]
  Transaction(#[from] TransactionError),
  #[error(transparent)]
  Selection(#[from] SelectionError),
}

type Result<T> = std::result::Result<T, CommentError>;

pub fn toggle_line_comments(
  doc: &Rope,
  selection: &Selection,
  token: Option<&str>,
) -> Result<Edit> {
  let text = doc.slice(..);
  let token = token.unwrap_or(DEFAULT_COMMENT_TOKEN);
  let lines = selected_lines(text, selection);
  let (commented, lines_to_change, min_column, margin) = analyze_line_comments(token, text, lines);
  let replacements =
    line_comment_replacements(token, text, commented, min_column, margin, lines_to_change);
  let transaction = transaction_from_replacements(doc, replacements)?;
  let selection = selection.clone().map(transaction.changes())?;

  Ok(Edit {
    transaction,
    selection,
  })
}

pub fn toggle_block_comments(
  doc: &Rope,
  selection: &Selection,
  tokens: &[BlockCommentToken],
) -> Result<Edit> {
  let text = doc.slice(..);
  let tokens = prepare_block_tokens(tokens);
  let (commented, changes) = analyze_block_comments(&tokens, text, selection);
  let replacements = block_comment_replacements(&changes, commented);
  let transaction = transaction_from_replacements(doc, replacements)?;
  let selection = if commented {
    selection.clone().map(transaction.changes())?
  } else {
    block_comment_selection_after_comment(selection, &changes)?
  };

  Ok(Edit {
    transaction,
    selection,
  })
}

fn selected_lines(text: RopeSlice, selection: &Selection) -> Vec<usize> {
  let mut lines = Vec::with_capacity(selection.len());
  let mut min_next_line = 0;

  for cursor in selection.iter() {
    let (start, end) = cursor.range.line_range(text);
    let start = start.clamp(min_next_line, text.len_lines());
    let end = (end + 1).min(text.len_lines());
    lines.extend(start..end);
    min_next_line = end;
  }

  lines
}

fn analyze_line_comments(
  token: &str,
  text: RopeSlice,
  lines: impl IntoIterator<Item = usize>,
) -> (bool, Vec<(usize, usize)>, usize, usize) {
  let mut commented = true;
  let mut saw_non_blank = false;
  let mut lines_to_change = Vec::new();
  let mut min_column = usize::MAX;
  let mut margin = 1;
  let token_len = token.chars().count();

  for line in lines {
    let line_slice = text.line(line);
    let Some(first_non_whitespace) = line_slice.first_non_whitespace_char() else {
      continue;
    };

    saw_non_blank = true;
    min_column = min_column.min(first_non_whitespace);

    if !line_slice.slice(first_non_whitespace..).starts_with(token) {
      commented = false;
    }

    if line_slice.slice(first_non_whitespace..).starts_with(token)
      && !matches!(
        line_slice.get_char(first_non_whitespace + token_len),
        Some(' ')
      )
    {
      margin = 0;
    }

    lines_to_change.push((line, first_non_whitespace));
  }

  if !saw_non_blank {
    return (false, Vec::new(), 0, 1);
  }

  (commented, lines_to_change, min_column, margin)
}

fn line_comment_replacements(
  token: &str,
  text: RopeSlice,
  commented: bool,
  min_column: usize,
  margin: usize,
  lines_to_change: Vec<(usize, usize)>,
) -> Vec<Replacement> {
  let comment_text = if margin == 0 {
    Tendril::from(token)
  } else {
    Tendril::from(format!("{token} "))
  };
  let token_len = token.chars().count();

  lines_to_change
    .into_iter()
    .map(|(line, comment_col)| {
      let line_start = text.line_to_char(line);
      if commented {
        let start = line_start + comment_col;
        Replacement::delete(Span {
          start,
          end: start + token_len + margin,
        })
      } else {
        Replacement::insert(line_start + min_column, comment_text.clone())
      }
    })
    .collect()
}

fn transaction_from_replacements(
  doc: &Rope,
  replacements: Vec<Replacement>,
) -> Result<Transaction> {
  Ok(Transaction::from_replacements(
    doc.len_chars(),
    replacements,
    ReplacementOverlapPolicy::Reject,
  )?)
}

#[cfg(test)]
fn get_comment_token<'a, S: AsRef<str>>(
  text: RopeSlice,
  tokens: &'a [S],
  line_num: usize,
) -> Option<&'a str> {
  let line = text.line(line_num);
  let start = line.first_non_whitespace_char()?;

  tokens
    .iter()
    .map(AsRef::as_ref)
    .filter(|token| line.slice(start..).starts_with(token))
    .max_by_key(|token| token.len())
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum BlockCommentChange {
  Commented {
    range:        Range,
    start_pos:    usize,
    end_pos:      usize,
    start_margin: bool,
    end_margin:   bool,
    start_token:  String,
    end_token:    String,
  },
  Uncommented {
    range:       Range,
    start_pos:   usize,
    end_pos:     usize,
    start_token: String,
    end_token:   String,
  },
  Whitespace {
    range: Range,
  },
}

#[derive(Debug, Clone)]
struct PreparedBlockToken {
  start:     String,
  end:       String,
  start_len: usize,
  end_len:   usize,
}

fn prepare_block_tokens(tokens: &[BlockCommentToken]) -> Vec<PreparedBlockToken> {
  let mut prepared = tokens
    .iter()
    .cloned()
    .map(|token| {
      PreparedBlockToken {
        start_len: token.start.chars().count(),
        end_len:   token.end.chars().count(),
        start:     token.start,
        end:       token.end,
      }
    })
    .collect::<Vec<_>>();

  prepared.sort_by(|a, b| {
    if a.start_len == b.start_len {
      b.end_len.cmp(&a.end_len)
    } else {
      b.start_len.cmp(&a.start_len)
    }
  });

  if prepared.is_empty() {
    prepared.push(PreparedBlockToken {
      start:     BlockCommentToken::default().start,
      end:       BlockCommentToken::default().end,
      start_len: 2,
      end_len:   2,
    });
  }

  prepared
}

fn analyze_block_comments(
  tokens: &[PreparedBlockToken],
  text: RopeSlice,
  selection: &Selection,
) -> (bool, Vec<BlockCommentChange>) {
  let mut commented = true;
  let mut only_whitespace = true;
  let mut changes = Vec::with_capacity(selection.len());
  let default_token = tokens.first().expect("block tokens must not be empty");

  for cursor in selection.iter() {
    let range = *cursor.range;
    let slice = range.slice(text);
    let Some(start_pos) = slice.first_non_whitespace_char() else {
      changes.push(BlockCommentChange::Whitespace { range });
      continue;
    };
    let Some(end_pos) = slice.last_non_whitespace_char() else {
      changes.push(BlockCommentChange::Whitespace { range });
      continue;
    };

    only_whitespace = false;
    if let Some((token, start_margin, end_margin)) =
      matching_block_token(tokens, slice, start_pos, end_pos)
    {
      changes.push(BlockCommentChange::Commented {
        range,
        start_pos,
        end_pos,
        start_margin,
        end_margin,
        start_token: token.start.clone(),
        end_token: token.end.clone(),
      });
    } else {
      commented = false;
      changes.push(BlockCommentChange::Uncommented {
        range,
        start_pos,
        end_pos,
        start_token: default_token.start.clone(),
        end_token: default_token.end.clone(),
      });
    }
  }

  if only_whitespace {
    commented = false;
  }

  (commented, changes)
}

fn matching_block_token<'a>(
  tokens: &'a [PreparedBlockToken],
  slice: RopeSlice,
  start_pos: usize,
  end_pos: usize,
) -> Option<(&'a PreparedBlockToken, bool, bool)> {
  let len = end_pos + 1 - start_pos;

  for token in tokens {
    if len < token.start_len + token.end_len {
      continue;
    }

    let after_start = start_pos + token.start_len;
    let before_end = end_pos.saturating_sub(token.end_len);
    let start_fragment = slice.slice(start_pos..after_start);
    let end_fragment = slice.slice(before_end + 1..end_pos + 1);

    if start_fragment == token.start.as_str() && end_fragment == token.end.as_str() {
      let start_margin = slice.get_char(after_start) == Some(' ');
      let end_margin = after_start != before_end && slice.get_char(before_end) == Some(' ');
      return Some((token, start_margin, end_margin));
    }
  }

  None
}

fn block_comment_replacements(changes: &[BlockCommentChange], commented: bool) -> Vec<Replacement> {
  let mut replacements = Vec::with_capacity(changes.len() * 2);

  for change in changes {
    match (commented, change) {
      (
        true,
        BlockCommentChange::Commented {
          range,
          start_pos,
          end_pos,
          start_token,
          end_token,
          start_margin,
          end_margin,
        },
      ) => {
        let from = range.from();
        let start_len = start_token.chars().count();
        let end_len = end_token.chars().count();
        replacements.push(Replacement::delete(Span {
          start: from + start_pos,
          end:   from + start_pos + start_len + usize::from(*start_margin),
        }));
        replacements.push(Replacement::delete(Span {
          start: from + end_pos - end_len - usize::from(*end_margin) + 1,
          end:   from + end_pos + 1,
        }));
      },
      (
        false,
        BlockCommentChange::Uncommented {
          range,
          start_pos,
          end_pos,
          start_token,
          end_token,
        },
      ) => {
        let from = range.from();
        replacements.push(Replacement::insert(
          from + start_pos,
          format!("{start_token} "),
        ));
        replacements.push(Replacement::insert(
          from + end_pos + 1,
          format!(" {end_token}"),
        ));
      },
      _ => {},
    }
  }

  replacements
}

fn block_comment_selection_after_comment(
  selection: &Selection,
  changes: &[BlockCommentChange],
) -> Result<Selection> {
  let mut ranges: SmallVec<[Range; 1]> = SmallVec::with_capacity(selection.len());
  let cursor_ids: SmallVec<[crate::selection::CursorId; 1]> =
    selection.cursor_ids().iter().copied().collect();
  let mut offset = 0;

  for change in changes {
    match change {
      BlockCommentChange::Uncommented {
        range,
        start_pos,
        end_pos,
        start_token,
        end_token: _,
      } => {
        let prefix_len = start_token.chars().count() + 1;
        let start = range.from() + offset + start_pos + prefix_len;
        let end = range.from() + offset + end_pos + 1 + prefix_len;
        ranges.push(Range::new(start, end).with_direction(range.direction()));
        offset += prefix_len + 1 + change_end_token_len(change);
      },
      BlockCommentChange::Commented { range, .. } | BlockCommentChange::Whitespace { range } => {
        ranges.push(shift_range(*range, offset));
      },
    }
  }

  Ok(Selection::new_with_ids(ranges, cursor_ids)?)
}

fn change_end_token_len(change: &BlockCommentChange) -> usize {
  match change {
    BlockCommentChange::Uncommented { end_token, .. } => end_token.chars().count(),
    _ => 0,
  }
}

fn shift_range(range: Range, offset: usize) -> Range {
  Range::new(range.from() + offset, range.to() + offset).with_direction(range.direction())
}

#[cfg(test)]
mod test {
  use super::*;

  fn apply(doc: &Rope, edit: &Edit) -> String {
    edit.transaction.apply_to(doc).unwrap().to_string()
  }

  #[test]
  fn line_comments_comment_selected_lines_at_the_minimum_indentation_column() {
    let doc = Rope::from("line1\n  line2\n    line3");
    let selection = Selection::single(0, doc.len_chars() - 1);

    let edit = toggle_line_comments(&doc, &selection, Some("//")).unwrap();

    assert_eq!(apply(&doc, &edit), "// line1\n//   line2\n//     line3");
    assert_eq!(
      edit.selection,
      selection.clone().map(edit.transaction.changes()).unwrap()
    );
  }

  #[test]
  fn line_comments_uncomment_using_zero_or_one_margin_consistently() {
    let doc = Rope::from("// line1\n  //line2\n    // line3");
    let selection = Selection::single(0, doc.len_chars() - 1);

    let edit = toggle_line_comments(&doc, &selection, Some("//")).unwrap();

    assert_eq!(apply(&doc, &edit), " line1\n  line2\n     line3");
    assert_eq!(
      edit.selection,
      selection.clone().map(edit.transaction.changes()).unwrap()
    );
  }

  #[test]
  fn line_comments_skip_blank_lines_when_commenting_and_uncommenting() {
    let doc = Rope::from("// first\n\n    // second\n\n  // third");
    let selection = Selection::single(0, doc.len_chars() - 1);

    let edit = toggle_line_comments(&doc, &selection, Some("//")).unwrap();

    assert_eq!(apply(&doc, &edit), "first\n\n    second\n\n  third");
  }

  #[test]
  fn block_comments_wrap_the_original_content_and_keep_the_selection_inside_the_delimiters() {
    let doc = Rope::from("1\n2\n3");
    let selection = Selection::single(0, doc.len_chars());

    let edit = toggle_block_comments(&doc, &selection, &[BlockCommentToken::default()]).unwrap();

    assert_eq!(apply(&doc, &edit), "/* 1\n2\n3 */");
    assert_eq!(edit.selection.ranges(), &[Range::new(3, 8)]);
  }

  #[test]
  fn block_comments_unwrap_existing_comments_and_map_the_selection_back_to_the_content() {
    let doc = Rope::from("/* 1\n2\n3 */");
    let selection = Selection::single(0, doc.len_chars());

    let edit = toggle_block_comments(&doc, &selection, &[BlockCommentToken::default()]).unwrap();

    assert_eq!(apply(&doc, &edit), "1\n2\n3");
    assert_eq!(
      edit.selection,
      selection.clone().map(edit.transaction.changes()).unwrap()
    );
  }

  #[test]
  fn block_comments_leave_whitespace_only_selections_unchanged() {
    let doc = Rope::from("   ");
    let selection = Selection::single(0, doc.len_chars());

    let edit = toggle_block_comments(&doc, &selection, &[BlockCommentToken::default()]).unwrap();

    assert_eq!(apply(&doc, &edit), "   ");
    assert_eq!(edit.selection, selection);
  }

  #[test]
  fn get_comment_token_prefers_the_longest_matching_prefix() {
    let text = Rope::from("    /// amogus");
    let tokens = ["///", "//"];

    assert_eq!(get_comment_token(text.slice(..), &tokens, 0), Some("///"));
  }

  #[test]
  fn get_comment_token_handles_multibyte_text_without_false_matches() {
    let text = Rope::from("··");
    let tokens = ["//", "///"];

    assert_eq!(get_comment_token(text.slice(..), &tokens, 0), None);
  }
}
