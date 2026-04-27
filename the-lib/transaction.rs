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
  use ropey::Rope;

  use super::*;

  fn span(start: usize, end: usize) -> Span {
    Span { start, end }
  }

  #[test]
  fn from_replacements_applies_sorted_non_overlapping_edits() {
    let doc = Rope::from("abcdef");
    let tx = Transaction::from_replacements(
      doc.len_chars(),
      [
        Replacement::replace(span(4, 6), "XY"),
        Replacement::insert(0, ">"),
        Replacement::delete(span(1, 3)),
      ],
      ReplacementOverlapPolicy::Reject,
    )
    .unwrap();

    assert_eq!(tx.apply_to(&doc).unwrap().to_string(), ">adXY");
  }

  #[test]
  fn from_replacements_validates_spans() {
    let err = Transaction::from_replacements(
      6,
      [Replacement::delete(span(4, 3))],
      ReplacementOverlapPolicy::Reject,
    )
    .unwrap_err();
    assert!(matches!(err, TransactionError::InvalidSpan { start: 4, end: 3 }));

    let err = Transaction::from_replacements(
      6,
      [Replacement::delete(span(5, 7))],
      ReplacementOverlapPolicy::Reject,
    )
    .unwrap_err();
    assert!(matches!(err, TransactionError::SpanOutOfBounds {
      start: 5,
      end: 7,
      len: 6,
    }));
  }

  #[test]
  fn from_replacements_rejects_overlapping_spans_by_default() {
    let err = Transaction::from_replacements(
      6,
      [
        Replacement::replace(span(1, 4), "X"),
        Replacement::replace(span(2, 5), "Y"),
      ],
      ReplacementOverlapPolicy::Reject,
    )
    .unwrap_err();

    assert!(matches!(err, TransactionError::OverlappingReplacement {
      prev_end: 4,
      start: 2,
      end: 5,
    }));
  }

  #[test]
  fn from_replacements_ignore_later_resolves_conflicts_in_caller_order() {
    let doc = Rope::from("abcdef");

    let first_wins = Transaction::from_replacements(
      doc.len_chars(),
      [
        Replacement::replace(span(1, 4), "X"),
        Replacement::replace(span(2, 5), "Y"),
      ],
      ReplacementOverlapPolicy::IgnoreLater,
    )
    .unwrap();

    let second_wins = Transaction::from_replacements(
      doc.len_chars(),
      [
        Replacement::replace(span(2, 5), "Y"),
        Replacement::replace(span(1, 4), "X"),
      ],
      ReplacementOverlapPolicy::IgnoreLater,
    )
    .unwrap();

    assert_eq!(first_wins.apply_to(&doc).unwrap().to_string(), "aXef");
    assert_eq!(second_wins.apply_to(&doc).unwrap().to_string(), "abYf");
  }

  #[test]
  fn delete_spans_merge_combines_touching_and_overlapping_spans() {
    let doc = Rope::from("0123456789");
    let tx = Transaction::delete_spans(
      doc.len_chars(),
      [span(2, 4), span(4, 6), span(5, 8)],
      DeletionOverlapPolicy::Merge,
    )
    .unwrap();

    assert_eq!(tx.apply_to(&doc).unwrap().to_string(), "0189");
  }

  #[test]
  fn compose_matches_sequential_application() {
    let doc = Rope::from("hello world");
    let first = Transaction::replace(doc.len_chars(), span(6, 11), "rust").unwrap();
    let after_first = first.apply_to(&doc).unwrap();
    let second = Transaction::from_replacements(
      after_first.len_chars(),
      [Replacement::insert(0, ">"), Replacement::replace(span(6, 10), "editor")],
      ReplacementOverlapPolicy::Reject,
    )
    .unwrap();

    let sequential = second.apply_to(&after_first).unwrap();
    let composed = first.compose(second).unwrap();

    assert_eq!(composed.apply_to(&doc).unwrap(), sequential);
    assert_eq!(sequential.to_string(), ">hello editor");
  }

  #[test]
  fn invert_restores_original_document() {
    let doc = Rope::from("世界 hello xz");
    let tx = Transaction::from_replacements(
      doc.len_chars(),
      [
        Replacement::insert(2, "!"),
        Replacement::replace(span(3, 8), "goodbye"),
      ],
      ReplacementOverlapPolicy::Reject,
    )
    .unwrap();

    let updated = tx.apply_to(&doc).unwrap();
    let reverted = tx.invert(&doc).unwrap().apply_to(&updated).unwrap();

    assert_eq!(reverted, doc);
  }

  #[test]
  fn apply_to_matches_in_place_and_leaves_original_unchanged() {
    let doc = Rope::from("hello world!");
    let tx = Transaction::from_replacements(
      doc.len_chars(),
      [Replacement::replace(span(6, 11), "void"), Replacement::insert(12, "!!")],
      ReplacementOverlapPolicy::Reject,
    )
    .unwrap();

    let mut in_place = doc.clone();
    tx.apply(&mut in_place).unwrap();

    assert_eq!(in_place, tx.apply_to(&doc).unwrap());
    assert_eq!(doc.to_string(), "hello world!");
  }

  #[test]
  fn changes_iter_round_trips_public_replacements() {
    let tx = Transaction::from_replacements(
      20,
      [
        Replacement::replace(span(12, 17), "void"),
        Replacement::delete(span(18, 20)),
        Replacement::insert(6, "::"),
      ],
      ReplacementOverlapPolicy::Reject,
    )
    .unwrap();

    assert_eq!(
      tx.changes_iter().collect::<Vec<_>>(),
      vec![
        Replacement::insert(6, "::"),
        Replacement::replace(span(12, 17), "void"),
        Replacement::delete(span(18, 20)),
      ],
    );
  }

  #[test]
  fn update_positions_matches_individual_mapping_even_when_unsorted() {
    let tx = Transaction::from_replacements(
      8,
      [Replacement::insert(2, "!!"), Replacement::replace(span(5, 7), "Z")],
      ReplacementOverlapPolicy::Reject,
    )
    .unwrap();
    let cs = tx.changes();

    let expected = [
      cs.map_pos(8, Assoc::After).unwrap(),
      cs.map_pos(2, Assoc::Before).unwrap(),
      cs.map_pos(6, Assoc::After).unwrap(),
      cs.map_pos(0, Assoc::Before).unwrap(),
    ];

    let (mut a, mut b, mut c, mut d) = (8usize, 2usize, 6usize, 0usize);
    cs.update_positions(
      [
        (&mut a, Assoc::After),
        (&mut b, Assoc::Before),
        (&mut c, Assoc::After),
        (&mut d, Assoc::Before),
      ]
      .into_iter(),
    )
    .unwrap();

    assert_eq!([a, b, c, d], expected);
  }

  #[test]
  fn update_positions_rejects_tracking_assoc() {
    let tx = Transaction::delete(8, span(2, 6)).unwrap();
    let mut pos = 3usize;

    let err = tx
      .changes()
      .update_positions(std::iter::once((&mut pos, Assoc::TrackDel)))
      .unwrap_err();

    assert!(matches!(err, TransactionError::TrackingAssocInBatchUpdate));
  }

  #[test]
  fn map_pos_tracked_marks_deleted_positions_and_preserves_offsets() {
    let tx = Transaction::delete(12, span(4, 8)).unwrap();
    let cs = tx.changes();

    assert_eq!(cs.map_pos_tracked(4, Assoc::TrackDel).unwrap(), MappedPos::Pos(4));
    assert_eq!(cs.map_pos_tracked(8, Assoc::TrackDel).unwrap(), MappedPos::Pos(4));

    let inside = cs.map_pos_tracked(6, Assoc::TrackDelSticky).unwrap();
    assert_eq!(
      inside,
      MappedPos::Deleted {
        pos: 4,
        offset: 2,
        len: 4,
      }
    );

    let boundary = cs.map_pos_tracked(8, Assoc::TrackDelBefore).unwrap();
    assert_eq!(
      boundary,
      MappedPos::Deleted {
        pos: 4,
        offset: 4,
        len: 4,
      }
    );
  }

  #[test]
  fn map_pos_tracked_handles_replacements_and_insert_stickiness() {
    let replacement = Transaction::replace(8, span(2, 6), "WXYZ").unwrap();
    let replacement_cs = replacement.changes();

    assert_eq!(replacement_cs.map_pos_tracked(2, Assoc::TrackDel).unwrap(), MappedPos::Pos(2));
    assert_eq!(
      replacement_cs.map_pos_tracked(4, Assoc::TrackDelSticky).unwrap(),
      MappedPos::Deleted {
        pos: 2,
        offset: 2,
        len: 4,
      }
    );

    let insertion = Transaction::insert_at(8, 4, "!!").unwrap();
    let insertion_cs = insertion.changes();

    assert_eq!(insertion_cs.map_pos_tracked(4, Assoc::TrackDel).unwrap(), MappedPos::Pos(4));
    assert_eq!(
      insertion_cs
        .map_pos_tracked(4, Assoc::TrackDelAfterSticky)
        .unwrap(),
      MappedPos::Pos(6)
    );
    assert_eq!(
      insertion_cs
        .map_pos_tracked(4, Assoc::TrackDelBeforeSticky)
        .unwrap(),
      MappedPos::Pos(4)
    );
  }

  #[test]
  fn empty_changesets_are_identity_for_apply_and_tracked_mapping() {
    let doc = Rope::from("hello");
    let tx = Transaction::empty(doc.len_chars());

    assert_eq!(tx.apply_to(&doc).unwrap(), doc);
    assert_eq!(tx.changes().map_pos_tracked(3, Assoc::TrackDel).unwrap(), MappedPos::Pos(3));
  }
}
