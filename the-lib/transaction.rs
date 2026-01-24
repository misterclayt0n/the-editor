//! Operational transformation primitives for document editing.
//!
//! This module provides the core types for representing and applying changes
//! to text documents: [`ChangeSet`] for low-level operations and
//! [`Transaction`] for higher-level edits that may include selection updates.
//!
//! # Architecture
//!
//! Changes are represented as a sequence of [`Operation`]s:
//!
//! - **Retain(n)** - Keep `n` characters unchanged
//! - **Delete(n)** - Remove `n` characters
//! - **Insert(s)** - Insert string `s`
//!
//! These operations are applied sequentially from the start of the document.
//! A [`ChangeSet`] is a list of operations that transforms a document of a
//! specific length into a new document.
//!
//! # Basic Usage
//!
//! ```ignore
//! use the_lib::transaction::Transaction;
//! use ropey::Rope;
//!
//! let mut doc = Rope::from("hello world");
//!
//! // Replace "world" with "rust"
//! let tx = Transaction::change(&doc, vec![
//!     (6, 11, Some("rust".into()))
//! ]).unwrap();
//!
//! tx.apply(&mut doc).unwrap();
//! assert_eq!(doc.to_string(), "hello rust");
//! ```
//!
//! # Position Mapping
//!
//! After applying changes, cursor positions need to be updated. The [`Assoc`]
//! enum controls how positions are mapped through insertions/deletions:
//!
//! - **Before** - Stay before insertions at this position
//! - **After** - Move after insertions at this position
//! - **BeforeWord/AfterWord** - Move based on word character boundaries
//! - **BeforeSticky/AfterSticky** - Maintain relative offset in same-size
//!   replacements
//!
//! ```ignore
//! use the_lib::transaction::{ChangeSet, Assoc};
//!
//! // Insert "!!" at position 4
//! let cs = ChangeSet { /* ... */ };
//!
//! // Position 4 with Before stays at 4
//! assert_eq!(cs.map_pos(4, Assoc::Before).unwrap(), 4);
//!
//! // Position 4 with After moves to 6 (after the insertion)
//! assert_eq!(cs.map_pos(4, Assoc::After).unwrap(), 6);
//! ```
//!
//! # Composition
//!
//! Two [`ChangeSet`]s can be composed together when the output length of the
//! first matches the input length of the second:
//!
//! ```ignore
//! let composed = changeset_a.compose(changeset_b)?;
//! // Applying `composed` is equivalent to applying `a` then `b`
//! ```
//!
//! # Inversion
//!
//! A [`ChangeSet`] can be inverted to create an "undo" changeset:
//!
//! ```ignore
//! let original_doc = doc.clone();
//! let inverted = changes.invert(&original_doc)?;
//!
//! changes.apply(&mut doc)?;
//! inverted.apply(&mut doc)?;
//! assert_eq!(doc, original_doc);
//! ```
//!
//! # Error Handling
//!
//! All fallible operations return [`Result<T, TransactionError>`]:
//!
//! - **LengthMismatch** - Document length doesn't match changeset expectation
//! - **ComposeLengthMismatch** - Changesets can't be composed (length mismatch)
//! - **InvalidRange** - Change range has start > end
//! - **RangeOutOfBounds** - Change range extends past document end
//! - **OverlappingRange** - Changes overlap (use `change_ignore_overlapping`
//!   instead)
//! - **PositionsOutOfBounds** - Positions to map are outside changeset range

use std::{
  borrow::Cow,
  iter::once,
};

use ropey::{
  Rope,
  RopeBuilder,
  RopeSlice,
};
use smallvec::SmallVec;
use the_core::chars::char_is_word;
use thiserror::Error;

use crate::{
  Tendril,
  selection::{
    Range,
    Selection,
  },
};

pub type Result<T> = std::result::Result<T, TransactionError>;

/// (from, to) replacement.
pub type Change = (usize, usize, Option<Tendril>);
pub type Deletion = (usize, usize);

#[derive(Debug, Error, PartialEq, Eq)]
#[non_exhaustive]
pub enum TransactionError {
  #[error("changeset length mismatch: expected {expected}, got {actual}")]
  LengthMismatch { expected: usize, actual: usize },
  #[error(
    "changeset compose length mismatch: left output {left_len_after}, right input {right_len}"
  )]
  ComposeLengthMismatch {
    left_len_after: usize,
    right_len:      usize,
  },
  #[error("invalid change range: start {from} is after end {to}")]
  InvalidRange { from: usize, to: usize },
  #[error("change range {from}..{to} is out of bounds for document length {len}")]
  RangeOutOfBounds {
    from: usize,
    to:   usize,
    len:  usize,
  },
  #[error("change range {from}..{to} overlaps previous end {prev_end}")]
  OverlappingRange {
    prev_end: usize,
    from:     usize,
    to:       usize,
  },
  #[error("positions {positions:?} are out of bounds for changeset length {len}")]
  PositionsOutOfBounds {
    positions: Vec<usize>,
    len:       usize,
  },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Operation {
  /// Move cursor by n characters.
  Retain(usize),

  /// Delete n characters.
  Delete(usize),

  /// Insert text at position.
  Insert(Tendril),
}

impl Operation {
  pub fn len_chars(&self) -> usize {
    match self {
      Operation::Retain(n) | Operation::Delete(n) => *n,
      Operation::Insert(s) => s.chars().count(),
    }
  }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Assoc {
  Before,
  After,

  /// Acts like `After` if a word character is inserted
  /// after the position, otherwise acts like `Before`
  AfterWord,

  /// Acts like `Before` if a word character is inserted
  /// before the position, otherwise acts like `After`
  BeforeWord,

  /// Acts like `Before` but if the position is within an exact replacement
  /// (exact size) the offset to the start of the replacement is kept
  BeforeSticky,

  /// Acts like `After` but if the position is within an exact replacement
  /// (exact size) the offset to the start of the replacement is kept
  AfterSticky,
}

impl Assoc {
  /// Whether to stick to gaps.
  fn stays_at_gaps(self) -> bool {
    !matches!(self, Self::BeforeWord | Self::AfterWord)
  }

  fn insert_offset(self, s: &str) -> usize {
    let chars = s.chars().count();

    match self {
      Assoc::After | Assoc::AfterSticky => chars,
      Assoc::AfterWord => s.chars().take_while(|&c| char_is_word(c)).count(),
      Assoc::Before | Assoc::BeforeSticky => 0,
      Assoc::BeforeWord => chars - s.chars().rev().take_while(|&c| char_is_word(c)).count(),
    }
  }

  pub fn sticky(self) -> bool {
    matches!(self, Assoc::BeforeSticky | Assoc::AfterSticky)
  }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ChangeSet {
  pub(crate) changes: Vec<Operation>,
  /// The required document length. Will refuse to apply changes unless it
  /// matches.
  len:                usize,
  len_after:          usize,
}

impl ChangeSet {
  pub fn with_capacity(capacity: usize) -> Self {
    Self {
      changes:   Vec::with_capacity(capacity),
      len:       0,
      len_after: 0,
    }
  }

  #[must_use]
  pub fn new(doc: RopeSlice) -> Self {
    let len = doc.len_chars();
    Self {
      changes: Vec::new(),
      len,
      len_after: len,
    }
  }

  #[doc(hidden)] // used by lsp to convert to LSP changes
  pub fn changes(&self) -> &[Operation] {
    &self.changes
  }

  /// Returns the expected document length for this changeset
  pub fn len(&self) -> usize {
    self.len
  }

  // Changeset builder operations: delete/insert/retain.
  //

  pub fn delete(&mut self, n: usize) {
    use Operation::*;

    if n == 0 {
      return;
    }

    self.len += n;

    if let Some(Delete(count)) = self.changes.last_mut() {
      *count += n;
    } else {
      self.changes.push(Delete(n))
    }
  }

  pub fn insert(&mut self, fragment: Tendril) {
    use Operation::*;

    if fragment.is_empty() {
      return;
    }

    self.len_after += fragment.chars().count();

    let new_last = match self.changes.as_mut_slice() {
      [.., Insert(prev)] | [.., Insert(prev), Delete(_)] => {
        prev.push_str(&fragment);
        return;
      },
      [.., last @ Delete(_)] => std::mem::replace(last, Insert(fragment)),
      _ => Insert(fragment),
    };

    self.changes.push(new_last);
  }

  pub fn retain(&mut self, n: usize) {
    use Operation::*;

    if n == 0 {
      return;
    }

    self.len += n;
    self.len_after += n;

    if let Some(Retain(count)) = self.changes.last_mut() {
      *count += n;
    } else {
      self.changes.push(Retain(n))
    }
  }

  /// Combine two `ChangeSet` together.
  pub fn compose(self, other: Self) -> Result<Self> {
    // The output length of the first must match the input length of the second.
    if self.len_after != other.len {
      return Err(TransactionError::ComposeLengthMismatch {
        left_len_after: self.len_after,
        right_len:      other.len,
      });
    }

    // Composing fails in weird ways if one of the sets is empty
    if self.changes.is_empty() {
      return Ok(other);
    }
    if other.changes.is_empty() {
      return Ok(self);
    }

    let len = self.changes.len();

    let mut changes_a = self.changes.into_iter();
    let mut changes_b = other.changes.into_iter();

    let mut head_a = changes_a.next();
    let mut head_b = changes_b.next();

    let mut changes = Self::with_capacity(len);

    loop {
      use std::cmp::Ordering;

      use Operation::*;
      match (head_a, head_b) {
        // we are done
        (None, None) => {
          break;
        },
        // deletion in A
        (Some(Delete(i)), b) => {
          changes.delete(i);
          head_a = changes_a.next();
          head_b = b;
        },
        // insertion in B
        (a, Some(Insert(current))) => {
          changes.insert(current);
          head_a = a;
          head_b = changes_b.next();
        },
        (None, val) | (val, None) => unreachable!("({:?})", val),
        (Some(Retain(i)), Some(Retain(j))) => {
          match i.cmp(&j) {
            Ordering::Less => {
              changes.retain(i);
              head_a = changes_a.next();
              head_b = Some(Retain(j - i));
            },
            Ordering::Equal => {
              changes.retain(i);
              head_a = changes_a.next();
              head_b = changes_b.next();
            },
            Ordering::Greater => {
              changes.retain(j);
              head_a = Some(Retain(i - j));
              head_b = changes_b.next();
            },
          }
        },
        (Some(Insert(mut s)), Some(Delete(j))) => {
          let len = s.chars().count();
          match len.cmp(&j) {
            Ordering::Less => {
              head_a = changes_a.next();
              head_b = Some(Delete(j - len));
            },
            Ordering::Equal => {
              head_a = changes_a.next();
              head_b = changes_b.next();
            },
            Ordering::Greater => {
              // TODO: cover this with a test
              // figure out the byte index of the truncated string end
              let (pos, _) = s.char_indices().nth(j).unwrap();
              s.replace_range(0..pos, "");
              head_a = Some(Insert(s));
              head_b = changes_b.next();
            },
          }
        },
        (Some(Insert(s)), Some(Retain(j))) => {
          let len = s.chars().count();
          match len.cmp(&j) {
            Ordering::Less => {
              changes.insert(s);
              head_a = changes_a.next();
              head_b = Some(Retain(j - len));
            },
            Ordering::Equal => {
              changes.insert(s);
              head_a = changes_a.next();
              head_b = changes_b.next();
            },
            Ordering::Greater => {
              // figure out the byte index of the truncated string end
              let (pos, _) = s.char_indices().nth(j).unwrap();
              let mut before = s;
              let after = before.split_off(pos);

              changes.insert(before);
              head_a = Some(Insert(after));
              head_b = changes_b.next();
            },
          }
        },
        (Some(Retain(i)), Some(Delete(j))) => {
          match i.cmp(&j) {
            Ordering::Less => {
              changes.delete(i);
              head_a = changes_a.next();
              head_b = Some(Delete(j - i));
            },
            Ordering::Equal => {
              changes.delete(j);
              head_a = changes_a.next();
              head_b = changes_b.next();
            },
            Ordering::Greater => {
              changes.delete(j);
              head_a = Some(Retain(i - j));
              head_b = changes_b.next();
            },
          }
        },
      };
    }

    debug_assert!(changes.len == self.len);

    Ok(changes)
  }

  /// Returns a new changeset that reverts this one. Useful for `undo`
  /// implementation. The document parameter expects the original document
  /// before this change was applied.
  pub fn invert(&self, original_doc: &Rope) -> Result<Self> {
    if self.changes.is_empty() {
      return Ok(ChangeSet {
        changes:   Vec::new(),
        len:       self.len_after,
        len_after: self.len,
      });
    }

    self.ensure_len(original_doc.len_chars())?;

    let mut changes = Self::with_capacity(self.changes.len());
    let mut pos = 0;

    for change in &self.changes {
      use Operation::*;
      match change {
        Retain(n) => {
          changes.retain(*n);
          pos += n;
        },
        Delete(n) => {
          let text = Cow::from(original_doc.slice(pos..pos + *n));
          changes.insert(Tendril::from(text.as_ref()));
          pos += n;
        },
        Insert(s) => {
          let chars = s.chars().count();
          changes.delete(chars);
        },
      }
    }

    Ok(changes)
  }

  fn ensure_len(&self, text_len: usize) -> Result<()> {
    if text_len != self.len {
      return Err(TransactionError::LengthMismatch {
        expected: self.len,
        actual:   text_len,
      });
    }
    Ok(())
  }

  /// Apply this changeset in-place.
  pub fn apply(&self, text: &mut Rope) -> Result<()> {
    self.ensure_len(text.len_chars())?;
    let mut pos = 0;

    for change in &self.changes {
      use Operation::*;
      match change {
        Retain(n) => pos += n,
        Delete(n) => text.remove(pos..pos + *n),
        Insert(s) => {
          text.insert(pos, s);
          pos += s.chars().count();
        },
      }
    }

    Ok(())
  }

  /// Apply this changeset to a rope and return the updated rope.
  pub fn apply_to(&self, text: &Rope) -> Result<Rope> {
    self.ensure_len(text.len_chars())?;
    if self.is_empty() {
      return Ok(text.clone());
    }

    let mut builder = RopeBuilder::new();
    let mut pos = 0;

    let append_slice = |from: usize, to: usize, builder: &mut RopeBuilder| {
      if from >= to {
        return;
      }
      let slice = text.slice(from..to);
      for chunk in slice.chunks() {
        builder.append(chunk);
      }
    };

    for change in &self.changes {
      use Operation::*;
      match change {
        Retain(n) => {
          append_slice(pos, pos + *n, &mut builder);
          pos += n;
        },
        Delete(n) => {
          pos += n;
        },
        Insert(s) => {
          builder.append(s.as_str());
        },
      }
    }

    append_slice(pos, self.len, &mut builder);

    Ok(builder.finish())
  }

  #[inline]
  pub fn is_empty(&self) -> bool {
    self.changes.is_empty() || self.changes == [Operation::Retain(self.len)]
  }

  /// Map a (mostly) *sorted* list of positions through the changes.
  ///
  /// This is equivalent to updating each position with `map_pos`:
  ///
  /// ``` no-compile
  /// for (pos, assoc) in positions {
  ///     *pos = changes.map_pos(*pos, assoc)?;
  /// }
  /// ```
  /// However this function is significantly faster for sorted lists running
  /// in `O(N+M)` instead of `O(NM)`. This function also handles unsorted/
  /// partially sorted lists. However, in that case worst case complexity is
  /// again `O(MN)`.  For lists that are often/mostly sorted (like the end of
  /// diagnostic ranges) performance is usally close to `O(N + M)`
  pub fn update_positions<'a>(
    &self,
    positions: impl Iterator<Item = (&'a mut usize, Assoc)>,
  ) -> Result<()> {
    use Operation::*;

    let mut positions = positions.peekable();

    let mut old_pos = 0;
    let mut new_pos = 0;
    let mut iter = self.changes.iter().enumerate().peekable();

    'outer: loop {
      macro_rules! map {
        ($map:expr, $i:expr) => {
          loop {
            let Some((pos, assoc)) = positions.peek_mut() else {
              return Ok(());
            };
            if **pos < old_pos {
              // Positions are not sorted, revert to the last Operation that
              // contains this position and continue iterating from there.
              // We can unwrap here since `pos` can not be negative
              // (unsigned integer) and iterating backwards to the start
              // should always move us back to the start
              for (i, change) in self.changes[..$i].iter().enumerate().rev() {
                match change {
                  Retain(i) => {
                    old_pos -= i;
                    new_pos -= i;
                  },
                  Delete(i) => {
                    old_pos -= i;
                  },
                  Insert(ins) => {
                    new_pos -= ins.chars().count();
                  },
                }
                if old_pos <= **pos {
                  iter = self.changes[i..].iter().enumerate().peekable();
                }
              }
              debug_assert!(old_pos <= **pos, "Reverse Iter across changeset works");
              continue 'outer;
            }
            #[allow(clippy::redundant_closure_call)]
            let Some(new_pos) = $map(**pos, *assoc) else {
              break;
            };
            **pos = new_pos;
            positions.next();
          }
        };
      }

      let Some((i, change)) = iter.next() else {
        map!(
          |pos, _| (old_pos == pos).then_some(new_pos),
          self.changes.len()
        );
        break;
      };

      let len = match change {
        Delete(i) | Retain(i) => *i,
        Insert(_) => 0,
      };
      let mut old_end = old_pos + len;

      match change {
        Retain(_) => {
          map!(
            |pos, _| (old_end > pos).then_some(new_pos + (pos - old_pos)),
            i
          );
          new_pos += len;
        },
        Delete(_) => {
          // in range
          map!(|pos, _| (old_end > pos).then_some(new_pos), i);
        },
        Insert(s) => {
          // a subsequent delete means a replace, consume it
          if let Some((_, Delete(len))) = iter.peek() {
            iter.next();

            old_end = old_pos + len;
            // in range of replaced text
            map!(
              |pos, assoc: Assoc| {
                (old_end > pos).then(|| {
                  // at point or tracking before
                  if pos == old_pos && assoc.stays_at_gaps() {
                    new_pos
                  } else {
                    let ins = assoc.insert_offset(s);
                    // if the deleted and inserted text have the exact same size
                    // keep the relative offset into the new text
                    if *len == ins && assoc.sticky() {
                      new_pos + (pos - old_pos)
                    } else {
                      new_pos + assoc.insert_offset(s)
                    }
                  }
                })
              },
              i
            );
          } else {
            // at insert point
            map!(
              |pos, assoc: Assoc| {
                (old_pos == pos).then(|| {
                  // return position before inserted text
                  new_pos + assoc.insert_offset(s)
                })
              },
              i
            );
          }

          new_pos += s.chars().count();
        },
      }
      old_pos = old_end;
    }
    let out_of_bounds: Vec<usize> = positions.map(|(pos, _)| *pos).collect();
    if out_of_bounds.is_empty() {
      Ok(())
    } else {
      Err(TransactionError::PositionsOutOfBounds {
        positions: out_of_bounds,
        len:       self.len,
      })
    }
  }

  /// Map a position through the changes.
  ///
  /// `assoc` indicates which side to associate the position with. `Before` will
  /// keep the position close to the character before, and will place it
  /// before insertions over that range, or at that point. `After` will move
  /// it forward, placing it at the end of such insertions.
  pub fn map_pos(&self, mut pos: usize, assoc: Assoc) -> Result<usize> {
    self.update_positions(once((&mut pos, assoc)))?;
    Ok(pos)
  }

  pub fn changes_iter(&self) -> ChangeIterator<'_> {
    ChangeIterator::new(self)
  }
}

pub struct ChangeIterator<'a> {
  iter: std::iter::Peekable<std::slice::Iter<'a, Operation>>,
  pos:  usize,
}

impl<'a> ChangeIterator<'a> {
  fn new(changeset: &'a ChangeSet) -> Self {
    let iter = changeset.changes.iter().peekable();
    Self { iter, pos: 0 }
  }
}

impl Iterator for ChangeIterator<'_> {
  type Item = Change;

  fn next(&mut self) -> Option<Self::Item> {
    use Operation::*;

    loop {
      match self.iter.next()? {
        Retain(len) => {
          self.pos += len;
        },
        Delete(len) => {
          let start = self.pos;
          self.pos += len;
          return Some((start, self.pos, None));
        },
        Insert(s) => {
          let start = self.pos;
          // a subsequent delete means a replace, consume it
          if let Some(Delete(len)) = self.iter.peek() {
            self.iter.next();

            self.pos += len;
            return Some((start, self.pos, Some(s.clone())));
          } else {
            return Some((start, start, Some(s.clone())));
          }
        },
      }
    }
  }
}

fn validate_change_bounds(from: usize, to: usize, len: usize) -> Result<()> {
  if from > to {
    return Err(TransactionError::InvalidRange { from, to });
  }
  if to > len {
    return Err(TransactionError::RangeOutOfBounds { from, to, len });
  }
  Ok(())
}

impl From<ChangeSet> for Transaction {
  fn from(changes: ChangeSet) -> Self {
    Self {
      changes,
      selection: None,
    }
  }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Transaction {
  changes:   ChangeSet,
  selection: Option<Selection>,
}

impl Transaction {
  pub fn new(doc: &Rope) -> Self {
    Self {
      changes:   ChangeSet::new(doc.slice(..)),
      selection: None,
    }
  }

  /// Changes made to the buffer.
  pub fn changes(&self) -> &ChangeSet {
    &self.changes
  }

  /// When set, explicitly updates the selection.
  pub fn selection(&self) -> Option<&Selection> {
    self.selection.as_ref()
  }

  /// Apply this transaction in-place.
  pub fn apply(&self, doc: &mut Rope) -> Result<()> {
    self.changes.apply(doc)
  }

  /// Apply this transaction to a rope and return the updated rope.
  pub fn apply_to(&self, doc: &Rope) -> Result<Rope> {
    self.changes.apply_to(doc)
  }

  /// Generate a transaction that reverts this one.
  pub fn invert(&self, original: &Rope) -> Result<Self> {
    let changes = self.changes.invert(original)?;

    Ok(Self {
      changes,
      selection: None,
    })
  }

  pub fn compose(mut self, other: Self) -> Result<Self> {
    self.changes = self.changes.compose(other.changes)?;
    // Other selection takes precedence
    self.selection = other.selection;
    Ok(self)
  }

  pub fn with_selection(mut self, selection: Selection) -> Self {
    self.selection = Some(selection);
    self
  }

  /// Generate a transaction from a set of potentially overlapping changes. The
  /// `change_ranges` iterator yield the range (of removed text) in the old
  /// document for each edit. If any change overlaps a previous range then that
  /// change is ignored. Changes are sorted by position before applying.
  ///
  /// The `process_change` callback is called for each edit that is not ignored
  /// (in the order yielded by `changes`) and should return the new text that
  /// the associated range will be replaced with.
  ///
  /// To make this function more flexible the iterator can yield additional data
  /// for each change that is passed to `process_change`
  pub fn change_ignore_overlapping<T>(
    doc: &Rope,
    change_ranges: impl IntoIterator<Item = (usize, usize, T)>,
    mut process_change: impl FnMut(usize, usize, T) -> Option<Tendril>,
  ) -> Result<Self> {
    let len = doc.len_chars();
    let mut ranges: Vec<_> = change_ranges.into_iter().collect();
    ranges.sort_by_key(|(from, to, _)| (*from, *to));

    let mut last = 0;
    let mut changes = Vec::with_capacity(ranges.len());
    for (from, to, data) in ranges {
      validate_change_bounds(from, to, len)?;
      if from < last {
        continue;
      }
      let tendril = process_change(from, to, data);
      last = to;
      changes.push((from, to, tendril));
    }
    Self::change(doc, changes)
  }

  /// Generate a transaction from a set of changes.
  pub fn change<I>(doc: &Rope, changes: I) -> Result<Self>
  where
    I: IntoIterator<Item = Change>,
  {
    let len = doc.len_chars();
    let changes = changes.into_iter();
    let (lower, upper) = changes.size_hint();
    let size = upper.unwrap_or(lower);
    let mut changeset = ChangeSet::with_capacity(2 * size + 1); // rough estimate

    let mut last = 0;
    for (from, to, tendril) in changes {
      validate_change_bounds(from, to, len)?;
      if from < last {
        return Err(TransactionError::OverlappingRange {
          prev_end: last,
          from,
          to,
        });
      }

      // Retain from last "to" to current "from"
      changeset.retain(from - last);
      let span = to - from;
      match tendril {
        Some(text) => {
          changeset.insert(text);
          changeset.delete(span);
        },
        None => changeset.delete(span),
      }
      last = to;
    }

    changeset.retain(len - last);

    Ok(Self::from(changeset))
  }

  /// Generate a transaction from a set of potentially overlapping deletions
  /// by merging overlapping deletions together.
  pub fn delete<I>(doc: &Rope, deletions: I) -> Result<Self>
  where
    I: IntoIterator<Item = Deletion>,
  {
    let len = doc.len_chars();

    let mut deletions: Vec<_> = deletions.into_iter().collect();
    deletions.sort_by_key(|(from, to)| (*from, *to));

    let mut merged = Vec::with_capacity(deletions.len());
    for (from, to) in deletions {
      validate_change_bounds(from, to, len)?;
      match merged.last_mut() {
        Some((_, last_end)) if from <= *last_end => {
          *last_end = (*last_end).max(to);
        },
        _ => merged.push((from, to)),
      }
    }

    Self::change(doc, merged.into_iter().map(|(from, to)| (from, to, None)))
  }

  pub fn insert_at_eof(mut self, text: Tendril) -> Transaction {
    self.changes.insert(text);
    self
  }

  /// Generate a transaction with a change per selection range.
  pub fn change_by_selection<F>(doc: &Rope, selection: &Selection, f: F) -> Result<Self>
  where
    F: FnMut(&Range) -> Change,
  {
    Self::change(doc, selection.iter().map(f))
  }

  pub fn change_by_selection_ignore_overlapping(
    doc: &Rope,
    selection: &Selection,
    mut change_range: impl FnMut(&Range) -> (usize, usize),
    mut create_tendril: impl FnMut(usize, usize) -> Option<Tendril>,
  ) -> Result<(Transaction, Selection)> {
    let mut ranges: SmallVec<[Range; 1]> = SmallVec::new();
    let mut cursor_ids: SmallVec<[crate::selection::CursorId; 1]> = SmallVec::new();

    let process_change = |change_start,
                          change_end,
                          (range, cursor_id): (Range, crate::selection::CursorId)| {
      ranges.push(range);
      cursor_ids.push(cursor_id);
      create_tendril(change_start, change_end)
    };
    let transaction = Self::change_ignore_overlapping(
      doc,
      selection.iter_with_ids().map(|(cursor_id, range)| {
        let (change_start, change_end) = change_range(range);
        (change_start, change_end, (*range, cursor_id))
      }),
      process_change,
    )?;

    let new_selection = if ranges.is_empty() {
      selection.clone()
    } else {
      Selection::new_with_ids_unchecked(ranges, cursor_ids)
    };

    Ok((transaction, new_selection))
  }

  /// Generate a transaction with a deletion per selection range.
  /// Compared to using `change_by_selection` directly these ranges may overlap.
  /// In that case they are merged.
  pub fn delete_by_selection<F>(doc: &Rope, selection: &Selection, f: F) -> Result<Self>
  where
    F: FnMut(&Range) -> Deletion,
  {
    Self::delete(doc, selection.iter().map(f))
  }

  /// Insert text at each selection head.
  pub fn insert(doc: &Rope, selection: &Selection, text: Tendril) -> Result<Self> {
    Self::change_by_selection(doc, selection, |range| {
      (range.head, range.head, Some(text.clone()))
    })
  }

  pub fn changes_iter(&self) -> ChangeIterator<'_> {
    self.changes.changes_iter()
  }
}

#[cfg(test)]
mod test {
  use super::*;

  #[test]
  fn composition() {
    use Operation::*;

    let a = ChangeSet {
      changes:   vec![
        Retain(5),
        Insert(" test!".into()),
        Retain(1),
        Delete(2),
        Insert("abc".into()),
      ],
      len:       8,
      len_after: 15,
    };

    let b = ChangeSet {
      changes:   vec![Delete(10), Insert("世orld".into()), Retain(5)],
      len:       15,
      len_after: 10,
    };

    let mut text = Rope::from("hello xz");

    // should probably return cloned text
    let composed = a.compose(b).unwrap();
    assert_eq!(composed.len, 8);
    composed.apply(&mut text).unwrap();
    assert_eq!(text, "世orld! abc");
  }

  #[test]
  fn invert() {
    use Operation::*;

    let changes = ChangeSet {
      changes:   vec![Retain(4), Insert("test".into()), Delete(5), Retain(3)],
      len:       12,
      len_after: 11,
    };

    let doc = Rope::from("世界3 hello xz");
    let revert = changes.invert(&doc).unwrap();

    let mut doc2 = doc.clone();
    changes.apply(&mut doc2).unwrap();

    // a revert is different
    assert_ne!(changes, revert);
    assert_ne!(doc, doc2);

    // but inverting a revert will give us the original
    assert_eq!(changes, revert.invert(&doc2).unwrap());

    // applying a revert gives us back the original
    revert.apply(&mut doc2).unwrap();
    assert_eq!(doc, doc2);
  }

  #[test]
  fn map_pos() {
    use Operation::*;

    // maps inserts
    let cs = ChangeSet {
      changes:   vec![Retain(4), Insert("!!".into()), Retain(4)],
      len:       8,
      len_after: 10,
    };

    assert_eq!(cs.map_pos(0, Assoc::Before).unwrap(), 0); // before insert region
    assert_eq!(cs.map_pos(4, Assoc::Before).unwrap(), 4); // at insert, track before
    assert_eq!(cs.map_pos(4, Assoc::After).unwrap(), 6); // at insert, track after
    assert_eq!(cs.map_pos(5, Assoc::Before).unwrap(), 7); // after insert region

    // maps deletes
    let cs = ChangeSet {
      changes:   vec![Retain(4), Delete(4), Retain(4)],
      len:       12,
      len_after: 8,
    };
    assert_eq!(cs.map_pos(0, Assoc::Before).unwrap(), 0); // at start
    assert_eq!(cs.map_pos(4, Assoc::Before).unwrap(), 4); // before a delete
    assert_eq!(cs.map_pos(5, Assoc::Before).unwrap(), 4); // inside a delete
    assert_eq!(cs.map_pos(5, Assoc::After).unwrap(), 4); // inside a delete

    // TODO: delete tracking

    // stays inbetween replacements
    let cs = ChangeSet {
      changes:   vec![
        Insert("ab".into()),
        Delete(2),
        Insert("cd".into()),
        Delete(2),
      ],
      len:       4,
      len_after: 4,
    };
    assert_eq!(cs.map_pos(2, Assoc::Before).unwrap(), 2);
    assert_eq!(cs.map_pos(2, Assoc::After).unwrap(), 2);
    // unsorted selection
    let cs = ChangeSet {
      changes:   vec![
        Insert("ab".into()),
        Delete(2),
        Insert("cd".into()),
        Delete(2),
      ],
      len:       4,
      len_after: 4,
    };
    let mut positions = [4, 2];
    cs.update_positions(positions.iter_mut().map(|pos| (pos, Assoc::After)))
      .unwrap();
    assert_eq!(positions, [4, 2]);
    // stays at word boundary
    let cs = ChangeSet {
      changes:   vec![
        Retain(2), // <space><space>
        Insert(" ab".into()),
        Retain(2), // cd
        Insert("de ".into()),
      ],
      len:       4,
      len_after: 10,
    };
    assert_eq!(cs.map_pos(2, Assoc::BeforeWord).unwrap(), 3);
    assert_eq!(cs.map_pos(4, Assoc::AfterWord).unwrap(), 9);
    let cs = ChangeSet {
      changes:   vec![
        Retain(1), // <space>
        Insert(" b".into()),
        Delete(1), // c
        Retain(1), // d
        Insert("e ".into()),
        Delete(1), // <space>
      ],
      len:       5,
      len_after: 7,
    };
    assert_eq!(cs.map_pos(1, Assoc::BeforeWord).unwrap(), 2);
    assert_eq!(cs.map_pos(3, Assoc::AfterWord).unwrap(), 5);
    let cs = ChangeSet {
      changes:   vec![
        Retain(1), // <space>
        Insert("a".into()),
        Delete(2), // <space>b
        Retain(1), // d
        Insert("e".into()),
        Delete(1), // f
        Retain(1), // <space>
      ],
      len:       5,
      len_after: 7,
    };
    assert_eq!(cs.map_pos(2, Assoc::BeforeWord).unwrap(), 1);
    assert_eq!(cs.map_pos(4, Assoc::AfterWord).unwrap(), 4);
  }

  #[test]
  fn transaction_change() {
    let mut doc = Rope::from("hello world!\ntest 123");
    let transaction = Transaction::change(
      &doc,
      // (1, 1, None) is a useless 0-width delete that gets factored out
      vec![(1, 1, None), (6, 11, Some("void".into())), (12, 17, None)],
    )
    .unwrap();
    transaction.apply(&mut doc).unwrap();
    assert_eq!(doc, Rope::from_str("hello void! 123"));
  }

  #[test]
  fn changes_iter() {
    let doc = Rope::from("hello world!\ntest 123");
    let changes = vec![(6, 11, Some("void".into())), (12, 17, None)];
    let transaction = Transaction::change(&doc, changes.clone()).unwrap();
    assert_eq!(transaction.changes_iter().collect::<Vec<_>>(), changes);
  }

  #[test]
  fn combine_with_empty() {
    let empty = Rope::from("");
    let a = ChangeSet::new(empty.slice(..));

    let mut b = ChangeSet::new(empty.slice(..));
    b.insert("a".into());

    let changes = a.compose(b).unwrap();

    use Operation::*;
    assert_eq!(changes.changes, &[Insert("a".into())]);
  }

  #[test]
  fn combine_with_utf8() {
    const TEST_CASE: &str = "Hello, これはヘリックスエディターです！";

    let empty = Rope::from("");
    let a = ChangeSet::new(empty.slice(..));

    let mut b = ChangeSet::new(empty.slice(..));
    b.insert(TEST_CASE.into());

    let changes = a.compose(b).unwrap();

    use Operation::*;
    assert_eq!(changes.changes, &[Insert(TEST_CASE.into())]);
    assert_eq!(changes.len_after, TEST_CASE.chars().count());
  }

  #[test]
  fn apply_to_matches_in_place() {
    let doc = Rope::from("hello world!");
    let transaction = Transaction::change(&doc, vec![
      (6, 11, Some("void".into())),
      (12, 12, Some("!!".into())),
    ])
    .unwrap();

    let mut in_place = doc.clone();
    transaction.apply(&mut in_place).unwrap();
    let persistent = transaction.apply_to(&doc).unwrap();

    assert_eq!(in_place, persistent);
    assert_eq!(doc, Rope::from("hello world!"));
  }

  #[test]
  fn invert_empty_changeset_is_identity() {
    let doc = Rope::from("hello");
    let changes = ChangeSet::new(doc.slice(..));
    let invert = changes.invert(&doc).unwrap();

    let updated = invert.apply_to(&doc).unwrap();
    assert_eq!(updated, doc);
    assert_eq!(invert.len(), doc.len_chars());
  }

  #[test]
  fn apply_errors_on_length_mismatch() {
    let doc = Rope::from("hello");
    let changes = ChangeSet::new(doc.slice(..));
    let mut other = Rope::from("nope");

    let err = changes.apply(&mut other).unwrap_err();
    assert!(matches!(err, TransactionError::LengthMismatch {
      expected: 5,
      actual:   4,
    }));
    let err = changes.apply_to(&other).unwrap_err();
    assert!(matches!(err, TransactionError::LengthMismatch {
      expected: 5,
      actual:   4,
    }));
    assert_eq!(other, Rope::from("nope"));
  }
}
