use ropey::RopeSlice;
use tree_house::tree_sitter::{
  InputEdit,
  Point,
};

use crate::transaction::{
  ChangeSet,
  Operation,
};

pub fn generate_edits(old_text: RopeSlice, changeset: &ChangeSet) -> Vec<InputEdit> {
  use Operation::*;

  let mut old_pos = 0;
  let mut edits = Vec::new();

  if changeset.changes.is_empty() {
    return edits;
  }

  let mut iter = changeset.changes.iter().peekable();
  while let Some(change) = iter.next() {
    let len = match change {
      Delete(n) | Retain(n) => *n,
      Insert(_) => 0,
    };
    let mut old_end = old_pos + len;

    match change {
      Retain(_) => {},
      Delete(_) => {
        let start_byte = old_text.char_to_byte(old_pos) as u32;
        let old_end_byte = old_text.char_to_byte(old_end) as u32;
        edits.push(InputEdit {
          start_byte,
          old_end_byte,
          new_end_byte: start_byte,
          start_point: Point::ZERO,
          old_end_point: Point::ZERO,
          new_end_point: Point::ZERO,
        });
      },
      Insert(text) => {
        let start_byte = old_text.char_to_byte(old_pos) as u32;

        if let Some(Delete(len)) = iter.peek() {
          old_end = old_pos + len;
          let old_end_byte = old_text.char_to_byte(old_end) as u32;
          iter.next();
          edits.push(InputEdit {
            start_byte,
            old_end_byte,
            new_end_byte: start_byte + text.len() as u32,
            start_point: Point::ZERO,
            old_end_point: Point::ZERO,
            new_end_point: Point::ZERO,
          });
        } else {
          edits.push(InputEdit {
            start_byte,
            old_end_byte: start_byte,
            new_end_byte: start_byte + text.len() as u32,
            start_point: Point::ZERO,
            old_end_point: Point::ZERO,
            new_end_point: Point::ZERO,
          });
        }
      },
    }

    old_pos = old_end;
  }

  edits
}

#[cfg(test)]
mod test {
  use ropey::Rope;

  use super::*;
  use crate::transaction::{
    Replacement,
    ReplacementOverlapPolicy,
    Span,
    Transaction,
  };

  fn span(start: usize, end: usize) -> Span {
    Span { start, end }
  }

  #[test]
  fn generate_edits_converts_replacements_and_deletions() {
    let doc = Rope::from("hello world!\ntest 123");
    let transaction = Transaction::from_replacements(
      doc.len_chars(),
      [
        Replacement::replace(span(6, 11), "test"),
        Replacement::delete(span(12, 17)),
      ],
      ReplacementOverlapPolicy::Reject,
    )
    .unwrap();

    let edits = generate_edits(doc.slice(..), transaction.changes());
    assert_eq!(edits, vec![
      InputEdit {
        start_byte:    6,
        old_end_byte:  11,
        new_end_byte:  10,
        start_point:   Point::ZERO,
        old_end_point: Point::ZERO,
        new_end_point: Point::ZERO,
      },
      InputEdit {
        start_byte:    12,
        old_end_byte:  17,
        new_end_byte:  12,
        start_point:   Point::ZERO,
        old_end_point: Point::ZERO,
        new_end_point: Point::ZERO,
      },
    ]);
  }

  #[test]
  fn generate_edits_converts_insertions() {
    let doc = Rope::from("fn test() {}");
    let transaction = Transaction::insert_at(doc.len_chars(), 8, "a: u32").unwrap();
    let edits = generate_edits(doc.slice(..), transaction.changes());

    assert_eq!(edits, vec![InputEdit {
      start_byte:    8,
      old_end_byte:  8,
      new_end_byte:  14,
      start_point:   Point::ZERO,
      old_end_point: Point::ZERO,
      new_end_point: Point::ZERO,
    }]);
  }

  #[test]
  fn generate_edits_returns_empty_for_identity_changeset() {
    let doc = Rope::from("hello");
    let transaction = Transaction::empty(doc.len_chars());

    assert!(generate_edits(doc.slice(..), transaction.changes()).is_empty());
  }
}
