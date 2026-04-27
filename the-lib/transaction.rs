use std::{
  borrow::Cow,
  iter::once,
};

use ropey::{
  Rope,
  RopeBuilder,
};
use thiserror::Error;

use crate::{
  Tendril,
  chars::char_is_word,
};

pub type Result<T> = std::result::Result<T, TransactionError>;

/// Normalized text extent used for document mutation.
///
/// Do not confuse this with `selection::Range`, which is directional
/// (`anchor`/`head`) selection state. A `Span` is always a document extent
/// (`start`/`end`) with no direction.
pub type Span = the_stdx::range::Range<usize>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Replacement {
  pub span: Span,
  pub text: Option<Tendril>,
}

impl Replacement {
  pub fn insert(at: usize, text: impl Into<Tendril>) -> Self {
    Self {
      span: Span {
        start: at,
        end:   at,
      },
      text: Some(text.into()),
    }
  }
  pub fn delete(span: Span) -> Self {
    Self { span, text: None }
  }
  pub fn replace(span: Span, text: impl Into<Tendril>) -> Self {
    Self {
      span,
      text: Some(text.into()),
    }
  }
  pub fn is_insert(&self) -> bool {
    self.span.start == self.span.end && self.text.is_some()
  }
  pub fn is_delete(&self) -> bool {
    self.span.start < self.span.end && self.text.is_none()
  }
  pub fn is_replace(&self) -> bool {
    self.span.start < self.span.end && self.text.is_some()
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplacementOverlapPolicy {
  /// Return an error if any replacement overlaps an earlier replacement.
  Reject,
  /// Preserve caller order for conflict resolution and ignore a replacement
  /// when it overlaps an earlier accepted replacement.
  ///
  /// Accepted replacements are sorted only after overlap resolution so they can
  /// be lowered to a canonical [`ChangeSet`].
  IgnoreLater,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeletionOverlapPolicy {
  Reject,
  IgnoreLater,
  Merge,
}

/// Result of mapping a position through changes with delete tracking enabled.
///
/// When using delete-tracking `Assoc` variants (`TrackDel`, `TrackDelSticky`,
/// `TrackDelBefore`, `TrackDelAfter`), this type provides information about
/// whether the position fell within deleted text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MappedPos {
  /// Position mapped successfully to a new location (not in deleted text).
  Pos(usize),

  /// Position was inside deleted text.
  Deleted {
    /// The position where the deletion occurred in the new document.
    pos:    usize,
    /// Original offset within the deleted range (0 = at start of deletion).
    offset: usize,
    /// Total length of the deletion in characters.
    len:    usize,
  },
}

impl MappedPos {
  /// Returns the mapped position, regardless of whether it was deleted.
  ///
  /// For `Pos(n)`, returns `n`.
  /// For `Deleted { pos, .. }`, returns `pos`.
  #[inline]
  pub fn position(&self) -> usize {
    match self {
      MappedPos::Pos(pos) => *pos,
      MappedPos::Deleted { pos, .. } => *pos,
    }
  }

  /// Returns `true` if the position was inside deleted text.
  #[inline]
  pub fn is_deleted(&self) -> bool {
    matches!(self, MappedPos::Deleted { .. })
  }

  /// Unwraps the position, panicking if it was deleted.
  ///
  /// # Panics
  /// Panics if the position was inside deleted text.
  #[inline]
  pub fn unwrap(self) -> usize {
    match self {
      MappedPos::Pos(pos) => pos,
      MappedPos::Deleted { .. } => panic!("called `MappedPos::unwrap()` on a `Deleted` value"),
    }
  }

  /// Returns the position if not deleted, or `None` if deleted.
  #[inline]
  pub fn ok(self) -> Option<usize> {
    match self {
      MappedPos::Pos(pos) => Some(pos),
      MappedPos::Deleted { .. } => None,
    }
  }
}

impl From<usize> for MappedPos {
  fn from(pos: usize) -> Self {
    MappedPos::Pos(pos)
  }
}

#[derive(Debug, Error, PartialEq, Eq)]
#[non_exhaustive]
pub enum TransactionError {
  #[error("document length mismatch: expected {expected}, got {actual}")]
  LengthMismatch { expected: usize, actual: usize },
  #[error(
    "changeset compose length mismatch: left output {left_len_after}, right input {right_len}"
  )]
  ComposeLengthMismatch {
    left_len_after: usize,
    right_len:      usize,
  },
  #[error("invalid span: start {start} is after end {end}")]
  InvalidSpan { start: usize, end: usize },
  #[error("span {start}..{end} is out of bounds for document length {len}")]
  SpanOutOfBounds {
    start: usize,
    end:   usize,
    len:   usize,
  },
  #[error("replacement span {start}..{end} overlaps previous end {prev_end}")]
  OverlappingReplacement {
    prev_end: usize,
    start:    usize,
    end:      usize,
  },
  #[error("positions {positions:?} are out of bounds for changeset length {len}")]
  PositionsOutOfBounds {
    positions: Vec<usize>,
    len:       usize,
  },
  #[error(
    "delete-tracking mapping mode cannot be used with batch update_positions; use map_pos_tracked \
     instead"
  )]
  TrackingAssocInBatchUpdate,
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

  // Delete tracking variants
  /// Track deletions - when a position falls inside deleted text, returns
  /// `MappedPos::Deleted` with information about the deletion rather than
  /// just collapsing to the deletion start.
  ///
  /// Maps to the start of the deletion but signals that the position was
  /// inside deleted content.
  TrackDel,

  /// Like `TrackDel` but preserves the relative offset within the deletion.
  /// Useful for undo/redo where you want to restore cursor position after
  /// re-inserting the deleted text.
  TrackDelSticky,

  /// Return deletion info if the character BEFORE this position was deleted.
  /// Useful for tracking whether adjacent content was removed.
  ///
  /// Example: Position 5 in "hello" - if 'o' (at index 4) is deleted,
  /// this signals that deletion affected the position.
  TrackDelBefore,

  /// Return deletion info if the character AFTER this position was deleted.
  /// Useful for tracking whether content immediately following was removed.
  ///
  /// Example: Position 4 in "hello" - if 'o' (at index 4) is deleted,
  /// this signals that deletion affected the position.
  TrackDelAfter,

  /// Like `TrackDelSticky` but moves after insertions (like `AfterSticky`).
  /// Use for the "expanding" edge of a forward selection (the anchor).
  ///
  /// Combines deletion tracking with `AfterSticky` insert behavior.
  TrackDelAfterSticky,

  /// Like `TrackDelSticky` but stays before insertions (like `BeforeSticky`).
  /// Use for the "contracting" edge of a forward selection (the head).
  ///
  /// Combines deletion tracking with `BeforeSticky` insert behavior.
  TrackDelBeforeSticky,
}

impl Assoc {
  /// Whether to stick to gaps.
  fn stays_at_gaps(self) -> bool {
    !matches!(self, Self::BeforeWord | Self::AfterWord)
  }

  fn insert_offset(self, s: &str) -> usize {
    let chars = s.chars().count();

    match self {
      Assoc::After | Assoc::AfterSticky | Assoc::TrackDelAfterSticky => chars,
      Assoc::AfterWord => s.chars().take_while(|&c| char_is_word(c)).count(),
      Assoc::Before | Assoc::BeforeSticky | Assoc::TrackDelBeforeSticky => 0,
      Assoc::BeforeWord => chars - s.chars().rev().take_while(|&c| char_is_word(c)).count(),
      // Other delete tracking variants behave like Before for insertions
      Assoc::TrackDel | Assoc::TrackDelSticky | Assoc::TrackDelBefore | Assoc::TrackDelAfter => 0,
    }
  }

  pub fn sticky(self) -> bool {
    matches!(
      self,
      Assoc::BeforeSticky
        | Assoc::AfterSticky
        | Assoc::TrackDelSticky
        | Assoc::TrackDelAfterSticky
        | Assoc::TrackDelBeforeSticky
    )
  }

  /// Returns `true` if this association mode tracks deletions.
  ///
  /// When tracking deletions, `map_pos_tracked` returns `MappedPos::Deleted`
  /// instead of just collapsing positions to the deletion boundary.
  pub fn tracks_deletion(self) -> bool {
    matches!(
      self,
      Assoc::TrackDel
        | Assoc::TrackDelSticky
        | Assoc::TrackDelBefore
        | Assoc::TrackDelAfter
        | Assoc::TrackDelAfterSticky
        | Assoc::TrackDelBeforeSticky
    )
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
  pub fn new_len(len: usize) -> Self {
    Self {
      changes: Vec::new(),
      len,
      len_after: len,
    }
  }

  pub fn changes(&self) -> &[Operation] {
    &self.changes
  }

  /// Returns the expected document length for this changeset
  pub fn len(&self) -> usize {
    self.len
  }

  /// Returns the document length after this changeset is applied.
  pub fn len_after(&self) -> usize {
    self.len_after
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
            // Reject delete-tracking Assoc variants - they require MappedPos return.
            // Users should use map_pos_tracked() for positions that need deletion info.
            if assoc.tracks_deletion() {
              return Err(TransactionError::TrackingAssocInBatchUpdate);
            }
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

  /// Map a position through the changes with delete tracking support.
  ///
  /// Unlike [`map_pos`], this method returns a [`MappedPos`] that can indicate
  /// whether the position fell within deleted text. This is useful for:
  ///
  /// - Detecting when a cursor position was "destroyed" by a deletion
  /// - Preserving relative offsets for undo/redo operations
  /// - Tracking whether adjacent content was deleted
  ///
  /// # Delete Tracking Modes
  ///
  /// - [`Assoc::TrackDel`]: Returns `MappedPos::Deleted` if position is inside
  ///   deleted text
  /// - [`Assoc::TrackDelSticky`]: Same as `TrackDel` but preserves offset
  ///   within deletion
  /// - [`Assoc::TrackDelBefore`]: Returns `MappedPos::Deleted` if char before
  ///   position was deleted
  /// - [`Assoc::TrackDelAfter`]: Returns `MappedPos::Deleted` if char after
  ///   position was deleted
  ///
  /// Non-tracking `Assoc` variants work identically to [`map_pos`].
  ///
  /// # Example
  ///
  /// ```ignore
  /// use the_lib::transaction::{ChangeSet, Assoc, MappedPos, Operation};
  ///
  /// let cs = ChangeSet {
  ///     changes: vec![Operation::Retain(4), Operation::Delete(4), Operation::Retain(4)],
  ///     len: 12,
  ///     len_after: 8,
  /// };
  ///
  /// // Position inside deletion with TrackDel
  /// let result = cs.map_pos_tracked(5, Assoc::TrackDel).unwrap();
  /// assert!(result.is_deleted());
  /// ```
  pub fn map_pos_tracked(&self, pos: usize, assoc: Assoc) -> Result<MappedPos> {
    use Operation::*;

    if pos > self.len {
      return Err(single_position_out_of_bounds(pos, self.len));
    }
    if self.changes.is_empty() {
      return Ok(MappedPos::Pos(pos));
    }

    let mut old_pos = 0;
    let mut new_pos = 0;
    let mut iter = self.changes.iter().peekable();

    while let Some(change) = iter.next() {
      let len = operation_input_len(change);
      let old_end = old_pos + len;

      match change {
        Retain(_) => {
          if pos < old_end {
            return Ok(MappedPos::Pos(new_pos + (pos - old_pos)));
          }
          new_pos += len;
        },
        Delete(del_len) => {
          if pos < old_end {
            return Ok(map_inside_deletion(pos - old_pos, new_pos, *del_len, assoc));
          }
          if let Some(mapped) = map_deletion_boundary(pos, old_end, new_pos, *del_len, assoc) {
            return Ok(mapped);
          }
        },
        Insert(s) => {
          let ins_len = s.chars().count();

          if let Some(Delete(del_len)) = iter.peek() {
            iter.next();
            let del_len = *del_len;
            let old_end = old_pos + del_len;

            if pos < old_end {
              return Ok(map_inside_replacement(
                pos - old_pos,
                new_pos,
                del_len,
                ins_len,
                s,
                assoc,
              ));
            }
            if let Some(mapped) =
              map_replacement_boundary(pos, old_end, new_pos, del_len, ins_len, assoc)
            {
              return Ok(mapped);
            }

            old_pos = old_end;
            new_pos += ins_len;
            continue;
          }

          if pos == old_pos {
            return Ok(MappedPos::Pos(new_pos + assoc.insert_offset(s)));
          }

          new_pos += ins_len;
        },
      }

      old_pos = old_end;
    }

    if pos == old_pos {
      return Ok(MappedPos::Pos(new_pos));
    }

    Err(single_position_out_of_bounds(pos, self.len))
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
  type Item = Replacement;

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
          return Some(Replacement::delete(Span {
            start,
            end: self.pos,
          }));
        },
        Insert(s) => {
          let start = self.pos;
          // a subsequent delete means a replace, consume it
          if let Some(Delete(len)) = self.iter.peek() {
            self.iter.next();

            self.pos += len;
            return Some(Replacement::replace(
              Span {
                start,
                end: self.pos,
              },
              s.clone(),
            ));
          } else {
            return Some(Replacement::insert(start, s.clone()));
          }
        },
      }
    }
  }
}

fn operation_input_len(change: &Operation) -> usize {
  match change {
    Operation::Delete(n) | Operation::Retain(n) => *n,
    Operation::Insert(_) => 0,
  }
}

fn single_position_out_of_bounds(pos: usize, len: usize) -> TransactionError {
  TransactionError::PositionsOutOfBounds {
    positions: vec![pos],
    len,
  }
}

fn deletion_tracks_offset(offset: usize, len: usize, assoc: Assoc) -> bool {
  match assoc {
    Assoc::TrackDel
    | Assoc::TrackDelSticky
    | Assoc::TrackDelAfterSticky
    | Assoc::TrackDelBeforeSticky
    | Assoc::TrackDelBefore => offset > 0,
    Assoc::TrackDelAfter => offset < len,
    _ => false,
  }
}

fn map_inside_deletion(offset: usize, new_pos: usize, del_len: usize, assoc: Assoc) -> MappedPos {
  if assoc.tracks_deletion() && deletion_tracks_offset(offset, del_len, assoc) {
    MappedPos::Deleted {
      pos: new_pos,
      offset,
      len: del_len,
    }
  } else {
    MappedPos::Pos(new_pos)
  }
}

fn map_deletion_boundary(
  pos: usize,
  old_end: usize,
  new_pos: usize,
  del_len: usize,
  assoc: Assoc,
) -> Option<MappedPos> {
  (pos == old_end && assoc == Assoc::TrackDelBefore).then_some(MappedPos::Deleted {
    pos: new_pos,
    offset: del_len,
    len: del_len,
  })
}

fn map_inside_replacement(
  offset: usize,
  new_pos: usize,
  del_len: usize,
  ins_len: usize,
  inserted: &str,
  assoc: Assoc,
) -> MappedPos {
  if assoc.tracks_deletion() {
    return map_inside_deletion(offset, new_pos, del_len, assoc);
  }
  if offset == 0 && assoc.stays_at_gaps() {
    return MappedPos::Pos(new_pos);
  }
  if del_len == ins_len && assoc.sticky() {
    return MappedPos::Pos(new_pos + offset);
  }
  MappedPos::Pos(new_pos + assoc.insert_offset(inserted))
}

fn map_replacement_boundary(
  pos: usize,
  old_end: usize,
  new_pos: usize,
  del_len: usize,
  ins_len: usize,
  assoc: Assoc,
) -> Option<MappedPos> {
  (pos == old_end && assoc == Assoc::TrackDelBefore).then_some(MappedPos::Deleted {
    pos: new_pos + ins_len,
    offset: del_len,
    len: del_len,
  })
}

fn spans_overlap(a: &Span, b: &Span) -> bool {
  a.start < b.end && b.start < a.end
}

fn validate_change_bounds(from: usize, to: usize, len: usize) -> Result<()> {
  if from > to {
    return Err(TransactionError::InvalidSpan {
      start: from,
      end:   to,
    });
  }
  if to > len {
    return Err(TransactionError::SpanOutOfBounds {
      start: from,
      end: to,
      len,
    });
  }
  Ok(())
}

impl From<ChangeSet> for Transaction {
  fn from(changes: ChangeSet) -> Self {
    Self { changes }
  }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Transaction {
  changes: ChangeSet,
}

impl Transaction {
  pub fn empty(len: usize) -> Self {
    Self {
      changes: ChangeSet::new_len(len),
    }
  }

  pub fn from_replacements(
    len: usize,
    replacements: impl IntoIterator<Item = Replacement>,
    policy: ReplacementOverlapPolicy,
  ) -> Result<Self> {
    let mut accepted: Vec<Replacement> = Vec::new();
    for replacement in replacements {
      validate_change_bounds(replacement.span.start, replacement.span.end, len)?;
      if let Some(prev) = accepted
        .iter()
        .find(|prev| spans_overlap(&prev.span, &replacement.span))
      {
        match policy {
          ReplacementOverlapPolicy::Reject => {
            return Err(TransactionError::OverlappingReplacement {
              prev_end: prev.span.end,
              start:    replacement.span.start,
              end:      replacement.span.end,
            });
          },
          // Caller order is preserved for conflict resolution: if this
          // replacement overlaps an earlier accepted replacement, this later
          // one is ignored. Accepted replacements are sorted only after this
          // decision so they can be lowered to a canonical ChangeSet.
          ReplacementOverlapPolicy::IgnoreLater => continue,
        }
      }
      accepted.push(replacement);
    }
    accepted.sort_by_key(|replacement| (replacement.span.start, replacement.span.end));
    let mut changeset = ChangeSet::with_capacity(accepted.len() * 2 + 1);
    let mut pos = 0;
    for Replacement { span, text } in accepted {
      changeset.retain(span.start - pos);
      match text {
        Some(text) => {
          changeset.insert(text);
          changeset.delete(span.end - span.start);
        },
        None => changeset.delete(span.end - span.start),
      }
      pos = span.end;
    }
    changeset.retain(len - pos);
    Ok(Self { changes: changeset })
  }

  pub fn replace(len: usize, span: Span, text: impl Into<Tendril>) -> Result<Self> {
    Self::from_replacements(
      len,
      [Replacement::replace(span, text)],
      ReplacementOverlapPolicy::Reject,
    )
  }
  pub fn insert_at(len: usize, at: usize, text: impl Into<Tendril>) -> Result<Self> {
    Self::from_replacements(
      len,
      [Replacement::insert(at, text)],
      ReplacementOverlapPolicy::Reject,
    )
  }
  pub fn delete(len: usize, span: Span) -> Result<Self> {
    Self::delete_spans(len, [span], DeletionOverlapPolicy::Reject)
  }
  pub fn delete_spans(
    len: usize,
    spans: impl IntoIterator<Item = Span>,
    policy: DeletionOverlapPolicy,
  ) -> Result<Self> {
    let mut spans: Vec<_> = spans.into_iter().collect();
    spans.sort_by_key(|span| (span.start, span.end));
    let mut out: Vec<Span> = Vec::with_capacity(spans.len());
    for span in spans {
      validate_change_bounds(span.start, span.end, len)?;
      if let Some(last) = out.last_mut() {
        if span.start < last.end
          || (policy == DeletionOverlapPolicy::Merge && span.start <= last.end)
        {
          match policy {
            DeletionOverlapPolicy::Reject => {
              return Err(TransactionError::OverlappingReplacement {
                prev_end: last.end,
                start:    span.start,
                end:      span.end,
              });
            },
            DeletionOverlapPolicy::IgnoreLater => continue,
            DeletionOverlapPolicy::Merge => {
              last.end = last.end.max(span.end);
              continue;
            },
          }
        }
      }
      out.push(span);
    }
    Self::from_replacements(
      len,
      out.into_iter().map(Replacement::delete),
      ReplacementOverlapPolicy::Reject,
    )
  }

  pub fn changes(&self) -> &ChangeSet {
    &self.changes
  }
  pub fn apply(&self, doc: &mut Rope) -> Result<()> {
    self.changes.apply(doc)
  }
  pub fn apply_to(&self, doc: &Rope) -> Result<Rope> {
    self.changes.apply_to(doc)
  }
  pub fn invert(&self, original: &Rope) -> Result<Self> {
    Ok(Self {
      changes: self.changes.invert(original)?,
    })
  }
  pub fn compose(self, other: Self) -> Result<Self> {
    Ok(Self {
      changes: self.changes.compose(other.changes)?,
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
    let transaction = Transaction::from_replacements(
      doc.len_chars(),
      vec![
        Replacement::delete(Span { start: 1, end: 1 }),
        Replacement::replace(
          Span {
            start: 6,
            end:   11,
          },
          "void",
        ),
        Replacement::delete(Span {
          start: 12,
          end:   17,
        }),
      ],
      ReplacementOverlapPolicy::Reject,
    )
    .unwrap();
    transaction.apply(&mut doc).unwrap();
    assert_eq!(doc, Rope::from_str("hello void! 123"));
  }

  #[test]
  fn changes_iter() {
    let doc = Rope::from("hello world!\ntest 123");
    let changes = vec![(6, 11, Some("void".into())), (12, 17, None)];
    let transaction = Transaction::from_replacements(
      doc.len_chars(),
      changes.clone().into_iter().map(|(from, to, text)| {
        Replacement {
          span: Span {
            start: from,
            end:   to,
          },
          text,
        }
      }),
      ReplacementOverlapPolicy::Reject,
    )
    .unwrap();
    assert_eq!(
      transaction.changes_iter().collect::<Vec<_>>(),
      changes
        .into_iter()
        .map(|(from, to, text)| {
          Replacement {
            span: Span {
              start: from,
              end:   to,
            },
            text,
          }
        })
        .collect::<Vec<_>>()
    );
  }

  #[test]
  fn combine_with_empty() {
    let empty = Rope::from("");
    let a = ChangeSet::new_len(empty.len_chars());

    let mut b = ChangeSet::new_len(empty.len_chars());
    b.insert("a".into());

    let changes = a.compose(b).unwrap();

    use Operation::*;
    assert_eq!(changes.changes, &[Insert("a".into())]);
  }

  #[test]
  fn combine_with_utf8() {
    const TEST_CASE: &str = "Hello, これはヘリックスエディターです！";

    let empty = Rope::from("");
    let a = ChangeSet::new_len(empty.len_chars());

    let mut b = ChangeSet::new_len(empty.len_chars());
    b.insert(TEST_CASE.into());

    let changes = a.compose(b).unwrap();

    use Operation::*;
    assert_eq!(changes.changes, &[Insert(TEST_CASE.into())]);
    assert_eq!(changes.len_after, TEST_CASE.chars().count());
  }

  #[test]
  fn compose_insert_longer_than_delete() {
    // Test the branch at line ~509-514 where Insert.len() > Delete.len()
    // This is the TODO that was marked as needing test coverage.
    use Operation::*;

    // First changeset: insert "hello" (5 chars) into empty document
    let a = ChangeSet {
      changes:   vec![Insert("hello".into())],
      len:       0,
      len_after: 5,
    };

    // Second changeset: delete first 3 chars of the 5-char document
    let b = ChangeSet {
      changes:   vec![Delete(3), Retain(2)],
      len:       5,
      len_after: 2,
    };

    let composed = a.compose(b).unwrap();

    // Composing these should result in just inserting "lo" (chars 3-4 of "hello")
    assert_eq!(composed.len, 0);
    assert_eq!(composed.len_after, 2);
    assert_eq!(composed.changes, &[Insert("lo".into())]);

    // Verify by applying to empty document
    let mut doc = Rope::from("");
    composed.apply(&mut doc).unwrap();
    assert_eq!(doc.to_string(), "lo");
  }

  #[test]
  fn compose_insert_longer_than_delete_multibyte() {
    // Same test but with multibyte UTF-8 characters
    use Operation::*;

    // Insert "世界hello" (7 chars: 2 CJK + 5 ASCII)
    let a = ChangeSet {
      changes:   vec![Insert("世界hello".into())],
      len:       0,
      len_after: 7,
    };

    // Delete first 4 chars ("世界he"), keep last 3 ("llo")
    let b = ChangeSet {
      changes:   vec![Delete(4), Retain(3)],
      len:       7,
      len_after: 3,
    };

    let composed = a.compose(b).unwrap();

    assert_eq!(composed.len, 0);
    assert_eq!(composed.len_after, 3);
    assert_eq!(composed.changes, &[Insert("llo".into())]);
  }

  #[test]
  fn apply_to_matches_in_place() {
    let doc = Rope::from("hello world!");
    let transaction = Transaction::from_replacements(
      doc.len_chars(),
      vec![
        Replacement::replace(
          Span {
            start: 6,
            end:   11,
          },
          "void",
        ),
        Replacement::insert(12, "!!"),
      ],
      ReplacementOverlapPolicy::Reject,
    )
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
    let changes = ChangeSet::new_len(doc.len_chars());
    let invert = changes.invert(&doc).unwrap();

    let updated = invert.apply_to(&doc).unwrap();
    assert_eq!(updated, doc);
    assert_eq!(invert.len(), doc.len_chars());
  }

  #[test]
  fn apply_errors_on_length_mismatch() {
    let doc = Rope::from("hello");
    let changes = ChangeSet::new_len(doc.len_chars());
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

  // Delete tracking tests

  #[test]
  fn map_pos_tracked_basic_delete() {
    use Operation::*;

    // Delete characters 4-7 (indices) from a 12-char document
    // Original: positions 0-11, delete range [4, 8) means chars at 4,5,6,7
    let cs = ChangeSet {
      changes:   vec![Retain(4), Delete(4), Retain(4)],
      len:       12,
      len_after: 8,
    };

    // Position before deletion - should map normally
    let result = cs.map_pos_tracked(2, Assoc::TrackDel).unwrap();
    assert_eq!(result, MappedPos::Pos(2));
    assert!(!result.is_deleted());

    // Position at deletion start boundary - NOT considered "inside" deletion
    // (cursor is between char 3 and char 4, at the left edge)
    let result = cs.map_pos_tracked(4, Assoc::TrackDel).unwrap();
    assert_eq!(result, MappedPos::Pos(4));
    assert!(!result.is_deleted());

    // Position inside deletion - should signal deletion
    let result = cs.map_pos_tracked(5, Assoc::TrackDel).unwrap();
    assert!(result.is_deleted());
    assert_eq!(result.position(), 4);
    if let MappedPos::Deleted { pos, offset, len } = result {
      assert_eq!(pos, 4);
      assert_eq!(offset, 1); // 5 - 4 = 1 char into deletion
      assert_eq!(len, 4);
    } else {
      panic!("Expected Deleted variant");
    }

    // Position at deletion end boundary - NOT inside deletion
    // (cursor is between char 7 and char 8, at the right edge)
    let result = cs.map_pos_tracked(8, Assoc::TrackDel).unwrap();
    assert_eq!(result, MappedPos::Pos(4));
    assert!(!result.is_deleted());

    // Position after deletion - should map with offset adjustment
    let result = cs.map_pos_tracked(10, Assoc::TrackDel).unwrap();
    assert_eq!(result, MappedPos::Pos(6)); // 10 - 4 deleted = 6
    assert!(!result.is_deleted());
  }

  #[test]
  fn map_pos_tracked_sticky_preserves_offset() {
    use Operation::*;

    let cs = ChangeSet {
      changes:   vec![Retain(2), Delete(4), Retain(2)],
      len:       8,
      len_after: 4,
    };

    // Position at deletion start boundary - NOT deleted
    let result = cs.map_pos_tracked(2, Assoc::TrackDelSticky).unwrap();
    assert!(!result.is_deleted());
    assert_eq!(result.position(), 2);

    // Offset 1 into deletion (position 3)
    let result = cs.map_pos_tracked(3, Assoc::TrackDelSticky).unwrap();
    assert!(result.is_deleted());
    if let MappedPos::Deleted { offset, len, .. } = result {
      assert_eq!(offset, 1);
      assert_eq!(len, 4);
    }

    // Offset 3 into deletion (position 5)
    let result = cs.map_pos_tracked(5, Assoc::TrackDelSticky).unwrap();
    assert!(result.is_deleted());
    if let MappedPos::Deleted { offset, .. } = result {
      assert_eq!(offset, 3);
    }
  }

  #[test]
  fn map_pos_tracked_del_before() {
    use Operation::*;

    let cs = ChangeSet {
      changes:   vec![Retain(4), Delete(4), Retain(4)],
      len:       12,
      len_after: 8,
    };

    // Position 4 (at deletion start) - no char before was deleted
    let result = cs.map_pos_tracked(4, Assoc::TrackDelBefore).unwrap();
    assert!(!result.is_deleted());

    // Position 5 (inside deletion) - char before (pos 4) was deleted
    let result = cs.map_pos_tracked(5, Assoc::TrackDelBefore).unwrap();
    assert!(result.is_deleted());

    // Position 8 (at deletion end) - char before (pos 7) was deleted
    let result = cs.map_pos_tracked(8, Assoc::TrackDelBefore).unwrap();
    assert!(result.is_deleted());
  }

  #[test]
  fn map_pos_tracked_del_after() {
    use Operation::*;

    let cs = ChangeSet {
      changes:   vec![Retain(4), Delete(4), Retain(4)],
      len:       12,
      len_after: 8,
    };

    // Position 4 (at deletion start) - char after (at index 4) WAS deleted
    // TrackDelAfter signals if the char at this position was deleted
    let result = cs.map_pos_tracked(4, Assoc::TrackDelAfter).unwrap();
    assert!(result.is_deleted());
    assert_eq!(result.position(), 4);

    // Position 7 (inside deletion, offset=3) - char at this position was deleted
    let result = cs.map_pos_tracked(7, Assoc::TrackDelAfter).unwrap();
    assert!(result.is_deleted());

    // Position 8 (at deletion end) - not inside deletion range
    let result = cs.map_pos_tracked(8, Assoc::TrackDelAfter).unwrap();
    assert!(!result.is_deleted());
    assert_eq!(result.position(), 4);
  }

  #[test]
  fn map_pos_tracked_replacement() {
    use Operation::*;

    // Replace 4 chars with 4 chars (same size replacement)
    let cs = ChangeSet {
      changes:   vec![Retain(2), Insert("XXXX".into()), Delete(4), Retain(2)],
      len:       8,
      len_after: 8,
    };

    // Position inside replaced text should signal deletion
    let result = cs.map_pos_tracked(3, Assoc::TrackDel).unwrap();
    assert!(result.is_deleted());
    if let MappedPos::Deleted { pos, offset, len } = result {
      assert_eq!(pos, 2);
      assert_eq!(offset, 1); // 1 char into the replaced region
      assert_eq!(len, 4);
    }

    // Position at replacement start boundary - NOT deleted (consistent with pure
    // deletions)
    let result = cs.map_pos_tracked(2, Assoc::TrackDel).unwrap();
    assert!(!result.is_deleted());
    assert_eq!(result.position(), 2);

    // TrackDelAfter at replacement start - IS deleted (char at position was
    // replaced)
    let result = cs.map_pos_tracked(2, Assoc::TrackDelAfter).unwrap();
    assert!(result.is_deleted());
  }

  #[test]
  fn map_pos_tracked_pure_insert() {
    use Operation::*;

    // Pure insertion at position 4
    let cs = ChangeSet {
      changes:   vec![Retain(4), Insert("!!".into()), Retain(4)],
      len:       8,
      len_after: 10,
    };

    // Position at insert point - should not be deleted
    let result = cs.map_pos_tracked(4, Assoc::TrackDel).unwrap();
    assert!(!result.is_deleted());
    assert_eq!(result.position(), 4); // TrackDel behaves like Before for inserts

    // Position after insert - should map with offset
    let result = cs.map_pos_tracked(5, Assoc::TrackDel).unwrap();
    assert!(!result.is_deleted());
    assert_eq!(result.position(), 7); // 5 + 2 inserted = 7
  }

  #[test]
  fn map_pos_tracked_non_tracking_variants() {
    use Operation::*;

    let cs = ChangeSet {
      changes:   vec![Retain(4), Delete(4), Retain(4)],
      len:       12,
      len_after: 8,
    };

    // Non-tracking variants should return Pos even for deleted positions
    let result = cs.map_pos_tracked(5, Assoc::Before).unwrap();
    assert_eq!(result, MappedPos::Pos(4));
    assert!(!result.is_deleted());

    let result = cs.map_pos_tracked(5, Assoc::After).unwrap();
    assert_eq!(result, MappedPos::Pos(4));
    assert!(!result.is_deleted());
  }

  #[test]
  fn map_pos_tracked_out_of_bounds() {
    use Operation::*;

    let cs = ChangeSet {
      changes:   vec![Retain(4), Delete(4)],
      len:       8,
      len_after: 4,
    };

    let err = cs.map_pos_tracked(10, Assoc::TrackDel).unwrap_err();
    assert!(matches!(err, TransactionError::PositionsOutOfBounds { .. }));
  }

  #[test]
  fn update_positions_rejects_tracking_variants() {
    use Operation::*;

    let cs = ChangeSet {
      changes:   vec![Retain(4), Delete(4), Retain(4)],
      len:       12,
      len_after: 8,
    };

    // Non-tracking variants should work
    let mut pos = 5usize;
    cs.update_positions(std::iter::once((&mut pos, Assoc::Before)))
      .unwrap();
    assert_eq!(pos, 4);

    // Tracking variants should error
    let mut pos = 5usize;
    let err = cs
      .update_positions(std::iter::once((&mut pos, Assoc::TrackDel)))
      .unwrap_err();
    assert!(matches!(err, TransactionError::TrackingAssocInBatchUpdate));
  }

  #[test]
  fn mapped_pos_helpers() {
    let pos = MappedPos::Pos(42);
    assert_eq!(pos.position(), 42);
    assert!(!pos.is_deleted());
    assert_eq!(pos.ok(), Some(42));
    assert_eq!(pos.unwrap(), 42);

    let deleted = MappedPos::Deleted {
      pos:    10,
      offset: 2,
      len:    5,
    };
    assert_eq!(deleted.position(), 10);
    assert!(deleted.is_deleted());
    assert_eq!(deleted.ok(), None);
  }

  #[test]
  fn assoc_tracks_deletion() {
    assert!(!Assoc::Before.tracks_deletion());
    assert!(!Assoc::After.tracks_deletion());
    assert!(!Assoc::BeforeSticky.tracks_deletion());
    assert!(!Assoc::AfterSticky.tracks_deletion());
    assert!(!Assoc::BeforeWord.tracks_deletion());
    assert!(!Assoc::AfterWord.tracks_deletion());

    assert!(Assoc::TrackDel.tracks_deletion());
    assert!(Assoc::TrackDelSticky.tracks_deletion());
    assert!(Assoc::TrackDelBefore.tracks_deletion());
    assert!(Assoc::TrackDelAfter.tracks_deletion());
    assert!(Assoc::TrackDelAfterSticky.tracks_deletion());
    assert!(Assoc::TrackDelBeforeSticky.tracks_deletion());
  }

  #[test]
  fn assoc_sticky_includes_trackdelsticky() {
    assert!(Assoc::TrackDelSticky.sticky());
    assert!(Assoc::TrackDelAfterSticky.sticky());
    assert!(Assoc::TrackDelBeforeSticky.sticky());
    assert!(!Assoc::TrackDel.sticky());
    assert!(!Assoc::TrackDelBefore.sticky());
    assert!(!Assoc::TrackDelAfter.sticky());
  }

  #[test]
  fn track_del_after_before_sticky_insert_behavior() {
    use Operation::*;

    // Pure insertion at position 4 (insert "!!" between chars)
    let cs = ChangeSet {
      changes:   vec![Retain(4), Insert("!!".into()), Retain(4)],
      len:       8,
      len_after: 10,
    };

    // TrackDelAfterSticky should move AFTER the insertion (like AfterSticky)
    let result = cs.map_pos_tracked(4, Assoc::TrackDelAfterSticky).unwrap();
    assert!(!result.is_deleted());
    assert_eq!(result.position(), 6); // 4 + 2 inserted chars

    // TrackDelBeforeSticky should stay BEFORE the insertion (like BeforeSticky)
    let result = cs.map_pos_tracked(4, Assoc::TrackDelBeforeSticky).unwrap();
    assert!(!result.is_deleted());
    assert_eq!(result.position(), 4); // stays at 4

    // Regular TrackDel stays before (like Before)
    let result = cs.map_pos_tracked(4, Assoc::TrackDel).unwrap();
    assert!(!result.is_deleted());
    assert_eq!(result.position(), 4);
  }

  #[test]
  fn map_pos_tracked_empty_changeset() {
    let doc = Rope::from("hello");
    let cs = ChangeSet::new_len(doc.len_chars());

    let result = cs.map_pos_tracked(3, Assoc::TrackDel).unwrap();
    assert_eq!(result, MappedPos::Pos(3));
  }

  #[test]
  fn delete_tracking_undo_roundtrip() {
    use Operation::*;

    // Simulate: delete "world" from "hello world"
    let _doc = Rope::from("hello world");
    let cs = ChangeSet {
      changes:   vec![Retain(6), Delete(5)],
      len:       11,
      len_after: 6,
    };

    // Cursor was at position 8 (inside "world")
    let result = cs.map_pos_tracked(8, Assoc::TrackDelSticky).unwrap();
    assert!(result.is_deleted());

    // The offset should be preserved
    if let MappedPos::Deleted { pos, offset, len } = result {
      assert_eq!(pos, 6); // Position after "hello "
      assert_eq!(offset, 2); // 2 chars into "world"
      assert_eq!(len, 5); // "world" is 5 chars

      // After undo, we could restore position as: deletion_start + offset
      let restored = 6 + offset; // = 8, the original position
      assert_eq!(restored, 8);
    }
  }
}
