//! Comment toggling helpers for line and block comments.
//!
//! This module provides utilities to locate comment tokens and build
//! transactions that toggle comments over selections. Functions return
//! `Result<Transaction>` to surface range or transaction errors.
//!
//! # Example: toggle line comments
//!
//! ```no_run
//! use ropey::Rope;
//! use the_lib::comment::toggle_line_comments;
//! use the_lib::selection::Selection;
//!
//! let mut doc = Rope::from("line\nline");
//! let selection = Selection::single(0, doc.len_chars());
//! let tx = toggle_line_comments(&doc, &selection, Some("//")).unwrap();
//! tx.apply(&mut doc).unwrap();
//! assert_eq!(doc, "// line\n// line");
//! ```

use ropey::{Rope, RopeSlice};
use smallvec::SmallVec;
use the_stdx::rope::RopeSliceExt;
use thiserror::Error;

use crate::{
  Tendril,
  selection::{Range, Selection},
  syntax::config::BlockCommentToken,
  selection::SelectionError,
  transaction::{Change, Transaction, TransactionError},
};

pub const DEFAULT_COMMENT_TOKEN: &str = "#";

#[derive(Debug, Error)]
pub enum CommentError {
  #[error(transparent)]
  Transaction(#[from] TransactionError),
  #[error(transparent)]
  Selection(#[from] SelectionError),
}

type Result<T> = std::result::Result<T, CommentError>;

/// Returns the longest matching comment token of the given line (if it exists).
pub fn get_comment_token<'a, S: AsRef<str>>(
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

/// Given text, a comment token, and a set of line indices, returns the
/// following:
/// - Whether the given lines should be considered commented
///     - If any of the lines are uncommented, all lines are considered as such.
/// - The lines to change for toggling comments
///     - This is all provided lines excluding blanks lines.
/// - The column of the comment tokens
///     - Column of existing tokens, if the lines are commented; column to place
///       tokens at otherwise.
/// - The margin to the right of the comment tokens
///     - Defaults to `1`. If any existing comment token is not followed by a
///       space, changes to `0`.
fn find_line_comment(
  token: &str,
  text: RopeSlice,
  lines: impl IntoIterator<Item = usize>,
) -> (bool, Vec<usize>, usize, usize) {
  let mut commented = true;
  let mut saw_non_blank = false;
  let mut to_change = Vec::new();
  let mut min = usize::MAX; // minimum col for first_non_whitespace_char
  let mut margin = 1;
  let token_len = token.chars().count();

  for line in lines {
    let line_slice = text.line(line);
    if let Some(pos) = line_slice.first_non_whitespace_char() {
      saw_non_blank = true;
      min = std::cmp::min(min, pos);

      // as soon as one of the non-blank lines doesn't have a comment, the whole block
      // is considered uncommented.
      if !line_slice.slice(pos..).starts_with(token) {
        commented = false;
      }

      // determine margin of 0 or 1 for uncommenting; if any existing comment
      // token is not followed by a space, a margin of 0 is used for all lines.
      if line_slice.slice(pos..).starts_with(token)
        && !matches!(line_slice.get_char(pos + token_len), Some(c) if c == ' ')
      {
        margin = 0;
      }

      // blank lines don't get pushed.
      to_change.push(line);
    }
  }

  if !saw_non_blank {
    commented = false;
    min = 0;
    margin = 1;
  }

  (commented, to_change, min, margin)
}

#[must_use]
pub fn toggle_line_comments(
  doc: &Rope,
  selection: &Selection,
  token: Option<&str>,
) -> Result<Transaction> {
  let text = doc.slice(..);

  let token = token.unwrap_or(DEFAULT_COMMENT_TOKEN);

  let mut lines: Vec<usize> = Vec::with_capacity(selection.len());

  let mut min_next_line = 0;
  for selection in selection {
    let (start, end) = selection.line_range(text);
    let start = start.clamp(min_next_line, text.len_lines());
    let end = (end + 1).min(text.len_lines());

    lines.extend(start..end);
    min_next_line = end;
  }

  let (commented, to_change, min, margin) = find_line_comment(token, text, lines);
  let comment = if margin == 0 {
    Tendril::from(token)
  } else {
    Tendril::from(format!("{} ", token))
  };
  let token_len = token.chars().count();

  let mut changes: Vec<Change> = Vec::with_capacity(to_change.len());

  for line in to_change {
    let pos = text.line_to_char(line) + min;

    if !commented {
      // comment line
      changes.push((pos, pos, Some(comment.clone())));
    } else {
      // uncomment line
      changes.push((pos, pos + token_len + margin, None));
    }
  }

  Ok(Transaction::change(doc, changes.into_iter())?)
}

#[derive(Debug, PartialEq, Eq)]
pub enum CommentChange {
  Commented {
    range: Range,
    start_pos: usize,
    end_pos: usize,
    start_margin: bool,
    end_margin: bool,
    start_token: String,
    end_token: String,
  },
  Uncommented {
    range: Range,
    start_pos: usize,
    end_pos: usize,
    start_token: String,
    end_token: String,
  },
  Whitespace {
    range: Range,
  },
}

pub fn find_block_comments(
  tokens: &[BlockCommentToken],
  text: RopeSlice,
  selection: &Selection,
) -> (bool, Vec<CommentChange>) {
  let prepared = prepare_block_tokens(tokens);
  find_block_comments_prepared(&prepared, text, selection)
}

#[derive(Debug, Clone)]
struct BlockToken {
  start:     String,
  end:       String,
  start_len: usize,
  end_len:   usize,
}

fn prepare_block_tokens(tokens: &[BlockCommentToken]) -> Vec<BlockToken> {
  let mut prepared: Vec<BlockToken> = tokens
    .iter()
    .cloned()
    .map(|token| BlockToken {
      start_len: token.start.chars().count(),
      end_len: token.end.chars().count(),
      start: token.start,
      end: token.end,
    })
    .collect();

  prepared.sort_by(|a, b| {
    if a.start_len == b.start_len {
      b.end_len.cmp(&a.end_len)
    } else {
      b.start_len.cmp(&a.start_len)
    }
  });

  if prepared.is_empty() {
    prepared.push(BlockToken {
      start:     BlockCommentToken::default().start,
      end:       BlockCommentToken::default().end,
      start_len: 2,
      end_len:   2,
    });
  }

  prepared
}

fn find_block_comments_prepared(
  tokens: &[BlockToken],
  text: RopeSlice,
  selection: &Selection,
) -> (bool, Vec<CommentChange>) {
  let mut commented = true;
  let mut only_whitespace = true;
  let mut comment_changes = Vec::with_capacity(selection.len());
  let default_tokens = tokens.first().cloned().unwrap();
  let mut start_token = default_tokens.start.clone();
  let mut end_token = default_tokens.end.clone();

  for range in selection {
    let selection_slice = range.slice(text);
    if let (Some(start_pos), Some(end_pos)) = (
      selection_slice.first_non_whitespace_char(),
      selection_slice.last_non_whitespace_char(),
    ) {
      let mut line_commented = false;
      let mut after_start = 0;
      let mut before_end = 0;
      let len = (end_pos + 1) - start_pos;

      for token in tokens {
        after_start = start_pos + token.start_len;
        before_end = end_pos.saturating_sub(token.end_len);

        if len >= token.start_len + token.end_len {
          let start_fragment = selection_slice.slice(start_pos..after_start);
          let end_fragment = selection_slice.slice(before_end + 1..end_pos + 1);

          // block commented with these tokens
          if start_fragment == token.start.as_str() && end_fragment == token.end.as_str() {
            start_token = token.start.to_string();
            end_token = token.end.to_string();
            line_commented = true;
            break;
          }
        }
      }

      if !line_commented {
        comment_changes.push(CommentChange::Uncommented {
          range: *range,
          start_pos,
          end_pos,
          start_token: default_tokens.start.clone(),
          end_token: default_tokens.end.clone(),
        });
        commented = false;
      } else {
        comment_changes.push(CommentChange::Commented {
          range: *range,
          start_pos,
          end_pos,
          start_margin: selection_slice.get_char(after_start) == Some(' '),
          end_margin: after_start != before_end
            && (selection_slice.get_char(before_end) == Some(' ')),
          start_token: start_token.to_string(),
          end_token: end_token.to_string(),
        });
      }
      only_whitespace = false;
    } else {
      comment_changes.push(CommentChange::Whitespace { range: *range });
    }
  }
  if only_whitespace {
    commented = false;
  }
  (commented, comment_changes)
}

#[must_use]
pub fn create_block_comment_transaction(
  doc: &Rope,
  selection: &Selection,
  commented: bool,
  comment_changes: Vec<CommentChange>,
) -> Result<(Transaction, SmallVec<[Range; 1]>)> {
  let mut changes: Vec<Change> = Vec::with_capacity(selection.len() * 2);
  let mut ranges: SmallVec<[Range; 1]> = SmallVec::with_capacity(selection.len());
  let mut offs = 0;
  for change in comment_changes {
    if commented {
      if let CommentChange::Commented {
        range,
        start_pos,
        end_pos,
        start_token,
        end_token,
        start_margin,
        end_margin,
      } = change
      {
        let start_len = start_token.chars().count();
        let end_len = end_token.chars().count();
        let from = range.from();
        changes.push((
          from + start_pos,
          from + start_pos + start_len + start_margin as usize,
          None,
        ));
        changes.push((
          from + end_pos - end_len - end_margin as usize + 1,
          from + end_pos + 1,
          None,
        ));
      }
    } else {
      // uncommented so manually map ranges through changes
      match change {
        CommentChange::Uncommented {
          range,
          start_pos,
          end_pos,
          start_token,
          end_token,
        } => {
          let from = range.from();
          let start_len = start_token.chars().count();
          let end_len = end_token.chars().count();
          changes.push((
            from + start_pos,
            from + start_pos,
            Some(Tendril::from(format!("{} ", start_token))),
          ));
          changes.push((
            from + end_pos + 1,
            from + end_pos + 1,
            Some(Tendril::from(format!(" {}", end_token))),
          ));

          let offset = start_len + end_len + 2;
          ranges.push(
            Range::new(from + offs, from + offs + end_pos + 1 + offset)
              .with_direction(range.direction()),
          );
          offs += offset;
        },
        CommentChange::Commented { range, .. } | CommentChange::Whitespace { range } => {
          ranges.push(Range::new(range.from() + offs, range.to() + offs));
        },
      }
    }
  }
  Ok((Transaction::change(doc, changes.into_iter())?, ranges))
}

#[must_use]
pub fn toggle_block_comments(
  doc: &Rope,
  selection: &Selection,
  tokens: &[BlockCommentToken],
) -> Result<Transaction> {
  let text = doc.slice(..);
  let (commented, comment_changes) = find_block_comments(tokens, text, selection);
  let (mut transaction, ranges) =
    create_block_comment_transaction(doc, selection, commented, comment_changes)?;
  if !commented {
    transaction = transaction.with_selection(Selection::new(ranges, selection.primary_index())?);
  }
  Ok(transaction)
}

pub fn split_lines_of_selection(text: RopeSlice, selection: &Selection) -> Selection {
  let mut ranges = SmallVec::new();
  for range in selection.ranges() {
    let (line_start, line_end) = range.line_range(text.slice(..));
    let mut pos = text.line_to_char(line_start);
    for line in text.slice(pos..text.line_to_char(line_end + 1)).lines() {
      let start = pos;
      pos += line.len_chars();
      ranges.push(Range::new(start, pos));
    }
  }
  Selection::new(ranges, 0).unwrap_or_else(|_| selection.clone())
}

#[cfg(test)]
mod test {
  use super::*;

  mod find_line_comment {
    use super::*;

    #[test]
    fn not_commented() {
      // four lines, two space indented, except for line 1 which is blank.
      let doc = Rope::from("  1\n\n  2\n  3");

      let text = doc.slice(..);

      let res = find_line_comment("//", text, 0..3);
      // (commented = false, to_change = [line 0, line 2], min = col 2, margin = 1)
      assert_eq!(res, (false, vec![0, 2], 2, 1));
    }

    #[test]
    fn is_commented() {
      // three lines where the second line is empty.
      let doc = Rope::from("// hello\n\n// there");

      let res = find_line_comment("//", doc.slice(..), 0..3);

      // (commented = true, to_change = [line 0, line 2], min = col 0, margin = 1)
      assert_eq!(res, (true, vec![0, 2], 0, 1));
    }
  }

  // TODO: account for uncommenting with uneven comment indentation
  mod toggle_line_comment {
    use super::*;

    #[test]
    fn comment() {
      // four lines, two space indented, except for line 1 which is blank.
      let mut doc = Rope::from("  1\n\n  2\n  3");
      // select whole document
      let selection = Selection::single(0, doc.len_chars() - 1);

      let transaction = toggle_line_comments(&doc, &selection, None).unwrap();
      transaction.apply(&mut doc).unwrap();

      assert_eq!(doc, "  # 1\n\n  # 2\n  # 3");
    }

    #[test]
    fn uncomment() {
      let mut doc = Rope::from("  # 1\n\n  # 2\n  # 3");
      let mut selection = Selection::single(0, doc.len_chars() - 1);

      let transaction = toggle_line_comments(&doc, &selection, None).unwrap();
      transaction.apply(&mut doc).unwrap();
      selection = selection.map(transaction.changes()).unwrap();

      assert_eq!(doc, "  1\n\n  2\n  3");
      let _ = selection; // to ignore the selection unused warning
    }

    #[test]
    fn uncomment_0_margin_comments() {
      let mut doc = Rope::from("  #1\n\n  #2\n  #3");
      let mut selection = Selection::single(0, doc.len_chars() - 1);

      let transaction = toggle_line_comments(&doc, &selection, None).unwrap();
      transaction.apply(&mut doc).unwrap();
      selection = selection.map(transaction.changes()).unwrap();

      assert_eq!(doc, "  1\n\n  2\n  3");
      let _ = selection; // to ignore the selection unused warning
    }

    #[test]
    fn uncomment_0_margin_comments_with_no_space() {
      let mut doc = Rope::from("#");
      let mut selection = Selection::single(0, doc.len_chars() - 1);

      let transaction = toggle_line_comments(&doc, &selection, None).unwrap();
      transaction.apply(&mut doc).unwrap();
      selection = selection.map(transaction.changes()).unwrap();
      assert_eq!(doc, "");
      let _ = selection; // to ignore the selection unused warning
    }
  }

  #[test]
  fn test_find_block_comments() {
    // three lines 5 characters.
    let mut doc = Rope::from("1\n2\n3");
    // select whole document
    let selection = Selection::single(0, doc.len_chars());

    let text = doc.slice(..);

    let res = find_block_comments(&[BlockCommentToken::default()], text, &selection);

    assert_eq!(
      res,
      (
        false,
        vec![CommentChange::Uncommented {
          range: Range::new(0, 5),
          start_pos: 0,
          end_pos: 4,
          start_token: "/*".to_string(),
          end_token: "*/".to_string(),
        }]
      )
    );

    // comment
    let transaction = toggle_block_comments(&doc, &selection, &[BlockCommentToken::default()]).unwrap();
    transaction.apply(&mut doc).unwrap();

    assert_eq!(doc, "/* 1\n2\n3 */");

    // uncomment
    let selection = Selection::single(0, doc.len_chars());
    let transaction = toggle_block_comments(&doc, &selection, &[BlockCommentToken::default()]).unwrap();
    transaction.apply(&mut doc).unwrap();
    assert_eq!(doc, "1\n2\n3");

    // don't panic when there is just a space in comment
    doc = Rope::from("/* */");
    let selection = Selection::single(0, doc.len_chars());
    let transaction = toggle_block_comments(&doc, &selection, &[BlockCommentToken::default()]).unwrap();
    transaction.apply(&mut doc).unwrap();
    assert_eq!(doc, "");
  }

  /// Test, if `get_comment_tokens` works, even if the content of the file
  /// includes chars, whose byte size unequal the amount of chars
  #[test]
  fn test_get_comment_with_char_boundaries() {
    let rope = Rope::from("··");
    let tokens = ["//", "///"];

    assert_eq!(
      super::get_comment_token(rope.slice(..), tokens.as_slice(), 0),
      None
    );
  }

  /// Test for `get_comment_token`.
  ///
  /// Assuming the comment tokens are stored as `["///", "//"]`,
  /// `get_comment_token` should still return `///` instead of `//` if the
  /// user is in a doc-comment section.
  #[test]
  fn test_use_longest_comment() {
    let text = Rope::from("    /// amogus");
    let tokens = ["///", "//"];

    assert_eq!(
      super::get_comment_token(text.slice(..), tokens.as_slice(), 0),
      Some("///")
    );
  }
}
