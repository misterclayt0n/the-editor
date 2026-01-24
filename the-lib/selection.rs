//! Cursor positions and multi-cursor selections.
//!
//! This module provides [`Range`] for representing a single cursor/selection
//! and [`Selection`] for managing multiple cursors (multi-cursor editing).
//!
//! # Range Model
//!
//! A [`Range`] has two positions: `anchor` and `head`. The `head` is where the
//! cursor visually appears, while the `anchor` is the other end of the
//! selection. When `anchor == head`, the range is a point (no selection).
//!
//! ```text
//! anchor=2, head=7: "he[llo w]orld"  (forward selection)
//! anchor=7, head=2: "he]llo w[orld"  (backward selection)
//! anchor=5, head=5: "hello|world"    (point/cursor)
//! ```
//!
//! The `from()` and `to()` methods return the range bounds regardless of
//! direction, while `direction()` tells you which way the selection extends.
//!
//! # 1-Width Cursor Model
//!
//! This module uses a "1-width" cursor model where even a point cursor occupies
//! the space of one grapheme. The `cursor()` method returns the left edge of
//! the block cursor:
//!
//! ```ignore
//! let range = Range::new(5, 7);
//! let cursor_pos = range.cursor(text); // Returns 6 (prev grapheme from head)
//! ```
//!
//! # Multi-Cursor Selection
//!
//! A [`Selection`] contains one or more [`Range`]s. Ranges are kept normalized:
//!
//! - Sorted by position
//! - No overlapping ranges (overlaps are merged)
//! - Always at least one range
//!
//! ```ignore
//! use the_lib::selection::Selection;
//!
//! // Create a multi-cursor selection
//! let selection = Selection::new(vec![
//!     Range::point(5),
//!     Range::point(15),
//!     Range::point(25),
//! ])?;
//!
//! // Iterate over all cursors
//! for range in selection.iter() {
//!     println!("Cursor at {}", range.head);
//! }
//!
//! // Iterate cursors with stable ids
//! for (id, range) in selection.iter_with_ids() {
//!     println!("{id:?} -> {}", range.head);
//! }
//! ```
//!
//! # Mapping Through Changes
//!
//! When document changes are applied, selections need to be updated:
//!
//! ```ignore
//! let new_selection = selection.map(transaction.changes())?;
//! ```
//!
//! For single ranges, use [`Range::map`]. For multiple ranges (multi-cursor),
//! use [`Selection::map`] which is more efficient.
//!
//! # Cursor Identity
//!
//! Each range in a selection carries a stable [`CursorId`]. When selections are
//! mapped or normalized, ids are preserved for surviving ranges. New ranges
//! (e.g. from splits) get fresh ids unless the original range's head is
//! contained within the new range, in which case the original id is retained.
//!
//! # Grapheme Alignment
//!
//! Selections should be aligned to grapheme boundaries. Use
//! `ensure_grapheme_boundary_*` functions from `the_core::grapheme` to ensure
//! positions don't split graphemes.
//!
//! # Error Handling
//!
//! Operations return [`Result<T, SelectionError>`]:
//!
//! - **EmptySelection** - Selection must have at least one range
//! - **RangeIndexOutOfBounds** - Accessed range index doesn't exist
//! - **RemoveLastRange** - Cannot remove the only range
//! - **NoRanges** - A transform operation produced no ranges

use std::{
  borrow::Cow,
  iter,
  num::NonZeroU64,
  sync::atomic::{AtomicU64, Ordering},
};

use ropey::RopeSlice;
use smallvec::{
  SmallVec,
  smallvec,
};
use the_core::{
  grapheme::{
    ensure_grapheme_boundary_next,
    ensure_grapheme_boundary_prev,
    next_grapheme_boundary,
    prev_grapheme_boundary,
  },
  line_ending::get_line_ending,
};
use the_stdx::{
  range::is_subset,
  rope::{
    self,
    RopeSliceExt,
  },
};
use thiserror::Error;
use tree_house::tree_sitter::Node;

use crate::{
  movement::Direction,
  transaction::{
    Assoc,
    ChangeSet,
    TransactionError,
  },
};

pub type Result<T> = std::result::Result<T, SelectionError>;

#[derive(Debug, Error, PartialEq, Eq)]
#[non_exhaustive]
pub enum SelectionError {
  #[error("selection must contain at least one range")]
  EmptySelection,
  #[error("range index {index} out of bounds for selection of length {len}")]
  RangeIndexOutOfBounds { index: usize, len: usize },
  #[error("cursor id count {ids} does not match range count {ranges}")]
  CursorIdCountMismatch { ids: usize, ranges: usize },
  #[error("cursor id {id} not found in selection")]
  CursorIdNotFound { id: u64 },
  #[error("cannot remove the last range from a selection")]
  RemoveLastRange,
  #[error("selection transform produced no ranges")]
  NoRanges,
  #[error(transparent)]
  Transaction(#[from] TransactionError),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CursorId(NonZeroU64);

impl CursorId {
  pub fn new(id: NonZeroU64) -> Self {
    Self(id)
  }

  pub fn fresh() -> Self {
    static NEXT_ID: AtomicU64 = AtomicU64::new(1);
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed).max(1);
    Self(NonZeroU64::new(id).expect("cursor id must be non-zero"))
  }

  pub fn get(self) -> u64 {
    self.0.get()
  }
}

#[derive(Debug, Clone, Copy)]
pub enum CursorPick {
  First,
  Last,
  Index(usize),
  Id(CursorId),
  Nearest(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Range {
  pub anchor:         usize,
  pub head:           usize,
  pub old_visual_pos: Option<(u32, u32)>,
}

impl Range {
  pub fn new(anchor: usize, head: usize) -> Self {
    Self {
      anchor,
      head,
      old_visual_pos: None,
    }
  }

  pub fn from_node(node: Node, text: RopeSlice, direction: Direction) -> Self {
    let from = text.byte_to_char(node.start_byte() as usize);
    let to = text.byte_to_char(node.end_byte() as usize);
    Range::new(from, to).with_direction(direction)
  }

  // Helpers.
  //

  #[inline]
  pub fn point(head: usize) -> Self {
    Self::new(head, head)
  }

  // TODO
  // pub fn from_node(node: Node, text: RopeSlice, direction: Direction) -> Self

  /// Start of the range
  #[inline]
  #[must_use]
  pub fn from(&self) -> usize {
    std::cmp::min(self.anchor, self.head)
  }

  /// End of the range
  #[inline]
  #[must_use]
  pub fn to(&self) -> usize {
    std::cmp::max(self.anchor, self.head)
  }

  /// Length of the range.
  #[inline]
  #[must_use]
  pub fn len(&self) -> usize {
    self.to() - self.from()
  }

  /// When the head and anchor are in the same position, we have no range.
  #[inline]
  #[must_use]
  pub fn is_empty(&self) -> bool {
    self.anchor == self.head
  }

  #[inline]
  pub fn contains_range(&self, other: &Self) -> bool {
    self.from() <= other.from() && self.to() >= other.to()
  }

  #[inline]
  pub fn contains(&self, pos: usize) -> bool {
    self.from() <= pos && pos < self.to()
  }

  /// Gets the left-side position of the block cursor.
  #[must_use]
  #[inline]
  pub fn cursor(self, text: RopeSlice) -> usize {
    if self.head > self.anchor {
      prev_grapheme_boundary(text, self.head)
    } else {
      self.head
    }
  }

  /// The line number the cursor is chilling on.
  #[inline]
  #[must_use]
  pub fn cursor_line(&self, slice: RopeSlice) -> usize {
    slice.char_to_line(self.cursor(slice))
  }

  /// Returns true if this [Range] covers a single grapheme in the given text.
  pub fn is_single_grapheme(&self, slice: RopeSlice) -> bool {
    let mut graphemes = slice.slice(self.from()..self.to()).graphemes();
    let first = graphemes.next_grapheme();
    let second = graphemes.next_grapheme();
    first.is_some() && second.is_none()
  }

  /// Converts this [Range] into an in order byte range, with no regard for
  /// direction.
  pub fn into_byte_range(self, slice: &RopeSlice) -> (usize, usize) {
    (
      slice.char_to_byte(self.from()),
      slice.char_to_byte(self.to()),
    )
  }

  #[inline]
  #[must_use]
  pub fn line_range(&self, slice: RopeSlice) -> (usize, usize) {
    let from = self.from();
    let to = if self.is_empty() {
      self.to() // NOTE: Could be `to()` or `from()` since we are at the same position.
    } else {
      prev_grapheme_boundary(slice, self.to()).max(from)
    };

    (slice.char_to_line(from), slice.char_to_line(to))
  }

  #[inline]
  #[must_use]
  pub fn direction(&self) -> Direction {
    if self.head < self.anchor {
      Direction::Backward
    } else {
      Direction::Forward
    }
  }

  /// Flips the direction of the selection
  #[inline]
  #[must_use]
  pub fn flip(&self) -> Self {
    // NOTE: We're returning Self directly here instead of Self::new() for clarity.
    Self {
      anchor:         self.head,
      head:           self.anchor,
      old_visual_pos: self.old_visual_pos,
    }
  }

  /// Returns the selection if we're going the same way as `direction`,
  /// else, flip it.
  #[inline]
  #[must_use]
  pub fn with_direction(self, direction: Direction) -> Self {
    if self.direction() == direction {
      self
    } else {
      self.flip()
    }
  }

  /// Check if two `Ranges` overlap
  pub fn overlaps(&self, other: &Self) -> bool {
    // NOTE: Just got this from helix, not gonna argue.
    //
    // "To my eye, it's non-obvious why this works, but I arrived
    // at it after transforming the slower version that explicitly
    // enumerated more cases.  The unit tests are thorough."
    self.from() == other.from() || (self.to() > other.from() && other.to() > self.from())
  }

  // Range operations.
  //

  /// Place the left side of the block cursor at `char_idx`, optionally
  /// extending the range.
  ///
  /// This follows the "1-width" semantics, and does a combination of anchor and
  /// head moves to behave as if both the front and back of the range are
  /// 1-width blocks.
  ///
  /// This method assumes that the range and `char_idx` are already properly
  /// grapheme-aligned.
  pub fn put_cursor(self, slice: RopeSlice, char_idx: usize, extend: bool) -> Range {
    if extend {
      let anchor = if self.head >= self.anchor && char_idx < self.anchor {
        next_grapheme_boundary(slice, self.anchor)
      } else if self.head < self.anchor && char_idx >= self.anchor {
        prev_grapheme_boundary(slice, self.anchor)
      } else {
        self.anchor
      };

      if anchor <= char_idx {
        Range::new(anchor, next_grapheme_boundary(slice, char_idx))
      } else {
        Range::new(anchor, char_idx)
      }
    } else {
      Range::point(char_idx)
    }
  }

  /// Map a range through a set of changes. Returns a new range representing
  /// the same position after the changes are applied.
  /// NOTE: This function runs at O(N) (N = number of changes) and can therefore
  /// cause performance problems for a large number of ranges (this means
  /// multicursors basically, so now we would have O(MN), where M = number of
  /// cursors).
  /// In this case, instead, we can use [Selection::map] or
  /// [ChangeSet::update_positions]
  pub fn map(mut self, changes: &ChangeSet) -> Result<Self> {
    use std::cmp::Ordering;
    if changes.is_empty() {
      return Ok(self);
    }

    let positions_to_map = match self.anchor.cmp(&self.head) {
      Ordering::Equal => {
        [
          (&mut self.anchor, Assoc::AfterSticky),
          (&mut self.head, Assoc::AfterSticky),
        ]
      },
      Ordering::Less => {
        [
          (&mut self.anchor, Assoc::AfterSticky),
          (&mut self.head, Assoc::BeforeSticky),
        ]
      },
      Ordering::Greater => {
        [
          (&mut self.head, Assoc::AfterSticky),
          (&mut self.anchor, Assoc::BeforeSticky),
        ]
      },
    };

    changes.update_positions(positions_to_map.into_iter())?;
    self.old_visual_pos = None;
    Ok(self)
  }

  /// Extend the range to cover at least `from` `to`.
  #[must_use]
  pub fn extend(&self, from: usize, to: usize) -> Self {
    debug_assert!(from <= to);

    if self.anchor <= self.head {
      Self {
        anchor:         self.anchor.min(from),
        head:           self.head.max(to),
        old_visual_pos: None,
      }
    } else {
      Self {
        anchor:         self.anchor.max(to),
        head:           self.head.min(from),
        old_visual_pos: None,
      }
    }
  }

  /// Returns a `Range` that encompasses both input ranges.
  ///
  /// This is much like [Self::extend()] but tries to negotiate the anchor/head
  /// ordering between the two input ranges.
  pub fn merge(&self, other: Self) -> Self {
    if self.anchor > self.head && other.anchor > other.head {
      Self {
        anchor:         self.anchor.max(other.anchor),
        head:           self.head.min(other.head),
        old_visual_pos: None,
      }
    } else {
      Self {
        anchor:         self.from().min(other.from()),
        head:           self.to().max(other.to()),
        old_visual_pos: None,
      }
    }
  }

  // "At" functions.
  //

  // NOTE: Should I do a function that just return a `Tendril` instead of what I
  // have setup here?

  /// Returns the text inside this range given the text of the whole buffer.
  ///
  /// The returned `Cow` is a reference if the range of text is inside a single
  /// chunk of the rope. Otherwise, a copy of the text is returned. Consider
  /// using `slice` instead if you don't need a `Cow` or `String` to avoid
  /// copying.
  #[inline]
  pub fn fragment<'a, 'b: 'a>(&'a self, text: RopeSlice<'b>) -> Cow<'b, str> {
    self.slice(text).into()
  }

  /// Returns the text inside this range given the text of the whole buffer.
  ///
  /// The returned value is a reference to the passed slice. This method never
  /// copies any contents.
  #[inline]
  pub fn slice<'a, 'b: 'a>(&'a self, text: RopeSlice<'b>) -> RopeSlice<'b> {
    text.slice(self.from()..self.to())
  }

  // Alignment
  //

  /// Compute a possibly new range from this range, with it's ends
  /// shifted as needed to align with grapheme boundaries.
  ///
  /// NOTE: Zero-width ranges will always stay zero-width, and non-zero-width
  /// ranges will never collapse to zero-width.
  #[must_use]
  pub fn grapheme_aligned(&self, slice: RopeSlice) -> Self {
    use std::cmp::Ordering;

    let (new_anchor, new_head) = match self.anchor.cmp(&self.head) {
      Ordering::Equal => {
        let pos = ensure_grapheme_boundary_prev(slice, self.anchor);
        (pos, pos)
      },
      Ordering::Less => {
        (
          ensure_grapheme_boundary_prev(slice, self.anchor),
          ensure_grapheme_boundary_next(slice, self.head),
        )
      },
      Ordering::Greater => {
        (
          ensure_grapheme_boundary_next(slice, self.anchor),
          ensure_grapheme_boundary_prev(slice, self.head),
        )
      },
    };

    Range {
      anchor:         new_anchor,
      head:           new_head,
      old_visual_pos: if new_anchor == self.anchor {
        self.old_visual_pos
      } else {
        None
      },
    }
  }

  /// Compute a possibly new range from this range, attempting to ensure
  /// a minimum range width of 1 char by shifting the head in the forward
  /// direction as needed.
  ///
  /// This method will never shift the anchor, and will only shift the
  /// head in the forward direction.  Therefore, this method can fail
  /// at ensuring the minimum width if and only if the passed range is
  /// both zero-width and at the end of the `RopeSlice`.
  ///
  /// If the input range is grapheme-boundary aligned, the returned range
  /// will also be.  Specifically, if the head needs to shift to achieve
  /// the minimum width, it will shift to the next grapheme boundary.
  #[must_use]
  #[inline]
  pub fn min_width_1(&self, slice: RopeSlice) -> Self {
    if self.anchor == self.head {
      Range {
        anchor:         self.anchor,
        head:           next_grapheme_boundary(slice, self.head),
        old_visual_pos: self.old_visual_pos,
      }
    } else {
      *self
    }
  }
}

impl From<(usize, usize)> for Range {
  fn from(value: (usize, usize)) -> Self {
    Self::new(value.0, value.1)
  }
}

impl From<Range> for the_stdx::range::Range {
  fn from(range: Range) -> Self {
    Self {
      start: range.from(),
      end:   range.to(),
    }
  }
}

/// A selection is one or more ranges.
/// INVARIANT: A selection can never be empty (always contain at least one
/// range).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selection {
  ranges:        SmallVec<[Range; 1]>,
  cursor_ids:    SmallVec<[CursorId; 1]>,
}

impl Selection {
  pub fn new(ranges: SmallVec<[Range; 1]>) -> Result<Self> {
    if ranges.is_empty() {
      return Err(SelectionError::EmptySelection);
    }
    let cursor_ids = ranges.iter().map(|_| CursorId::fresh()).collect();
    Ok(Self::new_with_ids_unchecked(ranges, cursor_ids).normalize())
  }

  pub fn new_with_ids(
    ranges: SmallVec<[Range; 1]>,
    cursor_ids: SmallVec<[CursorId; 1]>,
  ) -> Result<Self> {
    if ranges.is_empty() {
      return Err(SelectionError::EmptySelection);
    }
    if ranges.len() != cursor_ids.len() {
      return Err(SelectionError::CursorIdCountMismatch {
        ids:    cursor_ids.len(),
        ranges: ranges.len(),
      });
    }

    Ok(Self::new_with_ids_unchecked(ranges, cursor_ids).normalize())
  }

  pub(crate) fn new_with_ids_unchecked(
    ranges: SmallVec<[Range; 1]>,
    cursor_ids: SmallVec<[CursorId; 1]>,
  ) -> Self {
    Self {
      ranges,
      cursor_ids,
    }
  }

  pub fn point(pos: usize) -> Self {
    Self::new_with_ids_unchecked(smallvec![Range::point(pos)], smallvec![CursorId::fresh()])
  }

  pub fn ranges(&self) -> &[Range] {
    &self.ranges
  }

  pub fn cursor_ids(&self) -> &[CursorId] {
    &self.cursor_ids
  }

  pub fn iter_with_ids(&self) -> impl Iterator<Item = (CursorId, &Range)> {
    self.cursor_ids
      .iter()
      .copied()
      .zip(self.ranges.iter())
  }

  pub fn index_of(&self, id: CursorId) -> Option<usize> {
    self.cursor_ids.iter().position(|cursor_id| *cursor_id == id)
  }

  pub fn range_by_id(&self, id: CursorId) -> Option<&Range> {
    self.index_of(id).and_then(|idx| self.ranges.get(idx))
  }

  pub fn range_at(&self, idx: usize) -> Result<Range> {
    self.ranges.get(idx).copied().ok_or_else(|| {
      SelectionError::RangeIndexOutOfBounds {
        index: idx,
        len:   self.ranges.len(),
      }
    })
  }

  pub fn range_mut(&mut self, idx: usize) -> Result<&mut Range> {
    let len = self.ranges.len();
    self.ranges.get_mut(idx).ok_or_else(|| {
      SelectionError::RangeIndexOutOfBounds {
        index: idx,
        len,
      }
    })
  }

  pub fn cursor_id_at(&self, idx: usize) -> Result<CursorId> {
    self.cursor_ids.get(idx).copied().ok_or_else(|| {
      SelectionError::RangeIndexOutOfBounds {
        index: idx,
        len:   self.cursor_ids.len(),
      }
    })
  }

  pub fn pick(&self, pick: CursorPick) -> Result<(CursorId, Range)> {
    match pick {
      CursorPick::First => Ok((self.cursor_ids[0], self.ranges[0])),
      CursorPick::Last => {
        let idx = self.ranges.len() - 1;
        Ok((self.cursor_ids[idx], self.ranges[idx]))
      },
      CursorPick::Index(idx) => self.range_at(idx).map(|range| (self.cursor_ids[idx], range)),
      CursorPick::Id(id) => self
        .index_of(id)
        .map(|idx| (self.cursor_ids[idx], self.ranges[idx]))
        .ok_or(SelectionError::CursorIdNotFound { id: id.get() }),
      CursorPick::Nearest(pos) => {
        let (idx, _) = self
          .ranges
          .iter()
          .enumerate()
          .map(|(idx, range)| {
            let dist = pos.abs_diff(range.head);
            (idx, dist)
          })
          .min_by_key(|(_, dist)| *dist)
          .expect("selection is non-empty");
        Ok((self.cursor_ids[idx], self.ranges[idx]))
      },
    }
  }

  /// Total length of all ranges.
  #[inline]
  #[must_use]
  pub fn len(&self) -> usize {
    self.ranges().iter().map(Range::len).sum()
  }

  /// Check if the selection is empty (all ranges are collapsed).
  #[inline]
  #[must_use]
  pub fn is_empty(&self) -> bool {
    self.ranges().iter().all(|range| range.is_empty())
  }

  pub fn into_single(self) -> Self {
    if self.ranges.len() == 1 {
      self
    } else {
      Self::new_with_ids_unchecked(smallvec![self.ranges[0]], smallvec![self.cursor_ids[0]])
    }
  }

  /// Adds a new range to the selection.
  pub fn push(mut self, range: Range) -> Self {
    self.ranges.push(range);
    self.cursor_ids.push(CursorId::fresh());
    self.normalize()
  }

  pub fn push_with_id(mut self, range: Range, cursor_id: CursorId) -> Self {
    self.ranges.push(range);
    self.cursor_ids.push(cursor_id);
    self.normalize()
  }

  pub fn collapse(self, pick: CursorPick) -> Result<Self> {
    let (cursor_id, range) = self.pick(pick)?;
    Ok(Selection::new_with_ids_unchecked(
      smallvec![range],
      smallvec![cursor_id],
    ))
  }

  pub fn remove(mut self, idx: usize) -> Result<Self> {
    if self.ranges.len() == 1 {
      return Err(SelectionError::RemoveLastRange);
    }
    if idx >= self.ranges.len() {
      return Err(SelectionError::RangeIndexOutOfBounds {
        index: idx,
        len:   self.ranges.len(),
      });
    }

    self.ranges.remove(idx);
    self.cursor_ids.remove(idx);

    Ok(self)
  }

  pub fn replace(mut self, idx: usize, range: Range) -> Result<Self> {
    if idx >= self.ranges.len() {
      return Err(SelectionError::RangeIndexOutOfBounds {
        index: idx,
        len:   self.ranges.len(),
      });
    }
    self.ranges[idx] = range;
    Ok(self.normalize())
  }

  /// Map selections over a set of changes. Useful for adjusting the selection
  /// position after applying changes to a document.
  pub fn map(self, changes: &ChangeSet) -> Result<Self> {
    Ok(self.map_no_normalize(changes)?.normalize())
  }

  /// Map selections over a set of changes. Useful for adjusting the selection
  /// position after applying changes to a document. Doesn't normalize the
  /// selection
  pub fn map_no_normalize(mut self, changes: &ChangeSet) -> Result<Self> {
    if changes.is_empty() {
      return Ok(self);
    }

    let positions_to_map = self.ranges.iter_mut().flat_map(|range| {
      use std::cmp::Ordering;
      range.old_visual_pos = None;
      match range.anchor.cmp(&range.head) {
        Ordering::Equal => {
          [
            (&mut range.anchor, Assoc::AfterSticky),
            (&mut range.head, Assoc::AfterSticky),
          ]
        },
        Ordering::Less => {
          [
            (&mut range.anchor, Assoc::AfterSticky),
            (&mut range.head, Assoc::BeforeSticky),
          ]
        },
        Ordering::Greater => {
          [
            (&mut range.head, Assoc::AfterSticky),
            (&mut range.anchor, Assoc::BeforeSticky),
          ]
        },
      }
    });
    changes.update_positions(positions_to_map)?;
    Ok(self)
  }

  /// Returns an iterator over the line ranges of each range in the selection.
  ///
  /// Adjacent and overlapping line ranges of the [Range]s in the selection are
  /// merged.
  pub fn line_ranges<'a>(&'a self, slice: RopeSlice<'a>) -> LineRangeIter<'a> {
    let mut ranges: Vec<Range> = self.ranges.iter().copied().collect();
    ranges.sort_unstable_by_key(Range::from);
    LineRangeIter {
      ranges: ranges.into_iter().peekable(),
      slice,
    }
  }

  #[must_use]
  /// Constructs a selection holding a single range.
  pub fn single(anchor: usize, head: usize) -> Self {
    Self::new_with_ids_unchecked(
      smallvec![Range::new(anchor, head)],
      smallvec![CursorId::fresh()],
    )
  }

  /// Normalizes a [Selection]
  ///
  /// Ranges are sorted by [Range::from] with overlapping ranges merged.
  fn normalize(mut self) -> Self {
    if self.ranges.len() < 2 {
      return self;
    }
    let mut pairs: SmallVec<[(Range, CursorId); 1]> = self
      .ranges
      .into_iter()
      .zip(self.cursor_ids.into_iter())
      .collect();
    pairs.sort_by_key(|(range, _)| range.from());

    let mut ranges: SmallVec<[Range; 1]> = SmallVec::with_capacity(pairs.len());
    let mut cursor_ids: SmallVec<[CursorId; 1]> = SmallVec::with_capacity(pairs.len());

    for (range, cursor_id) in pairs {
      if let Some(prev_range) = ranges.last_mut() {
        if prev_range.overlaps(&range) {
          *prev_range = prev_range.merge(range);
          continue;
        }
      }
      ranges.push(range);
      cursor_ids.push(cursor_id);
    }

    self.ranges = ranges;
    self.cursor_ids = cursor_ids;
    self
  }

  pub fn merge_ranges(self) -> Self {
    let first = self.ranges.first().unwrap();
    let last = self.ranges.last().unwrap();
    let id = self.cursor_ids.first().copied().unwrap();
    Selection::new_with_ids_unchecked(smallvec![first.merge(*last)], smallvec![id])
  }

  /// Merges all ranges that are consecutive.
  pub fn merge_consecutive_ranges(mut self) -> Self {
    self = self.normalize();
    let mut pairs: SmallVec<[(Range, CursorId); 1]> = self
      .ranges
      .into_iter()
      .zip(self.cursor_ids.into_iter())
      .collect();

    let mut ranges: SmallVec<[Range; 1]> = SmallVec::with_capacity(pairs.len());
    let mut cursor_ids: SmallVec<[CursorId; 1]> = SmallVec::with_capacity(pairs.len());

    for (range, cursor_id) in pairs.drain(..) {
      if let Some(prev_range) = ranges.last_mut() {
        if prev_range.to() == range.from() {
          *prev_range = prev_range.merge(range);
          continue;
        }
      }
      ranges.push(range);
      cursor_ids.push(cursor_id);
    }

    self.ranges = ranges;
    self.cursor_ids = cursor_ids;
    self
  }

  /// Apply a transformation to all ranges and return a new Selection.
  pub fn transform<F>(mut self, mut f: F) -> Self
  where
    F: FnMut(Range) -> Range,
  {
    for range in self.ranges.iter_mut() {
      *range = f(*range)
    }

    self.normalize()
  }

  pub fn transform_iter<F, I>(mut self, mut f: F) -> Result<Self>
  where
    F: FnMut(Range) -> I,
    I: Iterator<Item = Range>,
  {
    let mut ranges = SmallVec::new();
    let mut cursor_ids = SmallVec::new();

    for (range, cursor_id) in self.ranges.into_iter().zip(self.cursor_ids.into_iter()) {
      let head = range.head;
      let produced: SmallVec<[Range; 1]> = f(range).collect();
      if produced.is_empty() {
        continue;
      }

      let mut assigned = false;
      for (idx, produced_range) in produced.iter().enumerate() {
        let use_id = if !assigned && range_contains_inclusive(produced_range, head) {
          assigned = true;
          cursor_id
        } else {
          CursorId::fresh()
        };
        ranges.push(*produced_range);
        cursor_ids.push(use_id);
        if idx == 0 && !assigned && produced.len() == 1 {
          assigned = true;
        }
      }

      if !assigned {
        // Fall back to keeping the id on the first produced range.
        cursor_ids[ranges.len() - produced.len()] = cursor_id;
      }
    }

    if ranges.is_empty() {
      return Err(SelectionError::NoRanges);
    }
    self.ranges = ranges;
    self.cursor_ids = cursor_ids;
    Ok(self.normalize())
  }

  /// Invariants here are:
  /// 1. All ranges are grapheme aligned.
  /// 2. All ranges are at least 1 character wide, unless at the very end of the
  ///    document.
  /// 3. Ranges are non-overlapping.
  /// 4. Ranges are sorted by their position in the text.
  pub fn ensure_invariants(self, slice: RopeSlice) -> Self {
    self
      .transform(|r| r.min_width_1(slice).grapheme_aligned(slice))
      .normalize()
  }

  /// Transforms the selection into all of the left-side head positions, using
  /// block cursor semantics.
  pub fn cursors(self, slice: RopeSlice) -> Self {
    self.transform(|range| Range::point(range.cursor(slice)))
  }

  pub fn fragments<'a>(
    &'a self,
    slice: RopeSlice<'a>,
  ) -> impl DoubleEndedIterator<Item = Cow<'a, str>> + ExactSizeIterator<Item = Cow<'a, str>> {
    self.ranges.iter().map(move |range| range.fragment(slice))
  }

  pub fn slices<'a>(
    &'a self,
    slice: RopeSlice<'a>,
  ) -> impl DoubleEndedIterator<Item = RopeSlice<'a>> + ExactSizeIterator<Item = RopeSlice<'a>> + 'a
  {
    self.ranges.iter().map(move |range| range.slice(slice))
  }

  #[inline(always)]
  pub fn iter(&self) -> std::slice::Iter<'_, Range> {
    self.ranges.iter()
  }

  pub fn range_bounds(&self) -> impl Iterator<Item = the_stdx::range::Range> + '_ {
    self.ranges.iter().map(|&range| range.into())
  }

  pub fn contains(&self, other: &Selection) -> bool {
    is_subset::<true>(self.range_bounds(), other.range_bounds())
  }
}

impl<'a> IntoIterator for &'a Selection {
  type Item = &'a Range;
  type IntoIter = std::slice::Iter<'a, Range>;

  fn into_iter(self) -> std::slice::Iter<'a, Range> {
    self.ranges.iter()
  }
}

impl IntoIterator for Selection {
  type Item = Range;
  type IntoIter = smallvec::IntoIter<[Range; 1]>;

  fn into_iter(self) -> smallvec::IntoIter<[Range; 1]> {
    self.ranges.into_iter()
  }
}

impl From<Range> for Selection {
  fn from(range: Range) -> Self {
    Self::new_with_ids_unchecked(smallvec![range], smallvec![CursorId::fresh()])
  }
}

pub struct LineRangeIter<'a> {
  ranges: iter::Peekable<std::vec::IntoIter<Range>>,
  slice:  RopeSlice<'a>,
}

impl Iterator for LineRangeIter<'_> {
  type Item = (usize, usize);

  fn next(&mut self) -> Option<Self::Item> {
    let (start, mut end) = self.ranges.next()?.line_range(self.slice);
    while let Some((next_start, next_end)) =
      self.ranges.peek().map(|range| range.line_range(self.slice))
    {
      // Merge overlapping and adjacent ranges.
      if next_start <= end.saturating_add(1) {
        end = next_end;
        self.ranges.next();
      } else {
        break;
      }
    }

    Some((start, end))
  }
}

pub fn keep_or_remove_matches(
  text: RopeSlice,
  selection: &Selection,
  regex: &rope::Regex,
  remove: bool,
) -> Result<Option<Selection>> {
  let mut ranges = SmallVec::with_capacity(selection.ranges.len());
  let mut cursor_ids = SmallVec::with_capacity(selection.ranges.len());

  for (cursor_id, range) in selection.iter_with_ids() {
    if regex.is_match(text.regex_input_at(range.from()..range.to())) ^ remove {
      ranges.push(*range);
      cursor_ids.push(cursor_id);
    }
  }

  if ranges.is_empty() {
    return Ok(None);
  }
  Ok(Some(Selection::new_with_ids(ranges, cursor_ids)?))
}

// TODO: support to split on capture #N instead of whole match
pub fn select_on_matches(
  text: RopeSlice,
  selection: &Selection,
  regex: &rope::Regex,
) -> Result<Option<Selection>> {
  let mut ranges = SmallVec::with_capacity(selection.ranges.len());
  let mut cursor_ids = SmallVec::with_capacity(selection.ranges.len());

  for (cursor_id, sel) in selection.iter_with_ids() {
    let head = sel.head;
    let mut produced = 0;
    let mut assigned = false;
    let start_idx = ranges.len();

    for mat in regex.find_iter(text.regex_input_at(sel.from()..sel.to())) {
      let start = text.byte_to_char(mat.start());
      let end = text.byte_to_char(mat.end());

      let range = Range::new(start, end).with_direction(sel.direction());
      // Make sure the match is not right outside of the selection.
      // These invalid matches can come from using RegEx anchors like `^`, `$`
      if range == Range::point(sel.to()) {
        continue;
      }

      let use_id = if !assigned && range_contains_inclusive(&range, head) {
        assigned = true;
        cursor_id
      } else {
        CursorId::fresh()
      };
      ranges.push(range);
      cursor_ids.push(use_id);
      produced += 1;
    }

    if produced > 0 && !assigned {
      cursor_ids[start_idx] = cursor_id;
    }
  }

  if ranges.is_empty() {
    return Ok(None);
  }
  Ok(Some(Selection::new_with_ids(ranges, cursor_ids)?))
}

pub fn split_on_newline(text: RopeSlice, selection: &Selection) -> Result<Selection> {
  let mut ranges = SmallVec::with_capacity(selection.ranges.len());
  let mut cursor_ids = SmallVec::with_capacity(selection.ranges.len());

  for (cursor_id, sel) in selection.iter_with_ids() {
    let head = sel.head;
    let mut produced = 0;
    let mut assigned = false;
    let start_idx = ranges.len();

    // Special case: zero-width selection.
    if sel.from() == sel.to() {
      ranges.push(*sel);
      cursor_ids.push(cursor_id);
      continue;
    }

    let sel_start = sel.from();
    let sel_end = sel.to();

    let mut start = sel_start;

    for line in sel.slice(text).lines() {
      let Some(line_ending) = get_line_ending(&line) else {
        break;
      };
      let line_end = start + line.len_chars();
      let range =
        Range::new(start, line_end - line_ending.len_chars()).with_direction(sel.direction());
      let use_id = if !assigned && range_contains_inclusive(&range, head) {
        assigned = true;
        cursor_id
      } else {
        CursorId::fresh()
      };
      ranges.push(range);
      cursor_ids.push(use_id);
      produced += 1;
      start = line_end;
    }

    if start < sel_end {
      let range = Range::new(start, sel_end).with_direction(sel.direction());
      let use_id = if !assigned && range_contains_inclusive(&range, head) {
        assigned = true;
        cursor_id
      } else {
        CursorId::fresh()
      };
      ranges.push(range);
      cursor_ids.push(use_id);
      produced += 1;
    }

    if produced > 0 && !assigned {
      cursor_ids[start_idx] = cursor_id;
    }
  }

  Selection::new_with_ids(ranges, cursor_ids)
}

pub fn split_on_matches(
  text: RopeSlice,
  selection: &Selection,
  regex: &the_stdx::rope::Regex,
) -> Result<Selection> {
  let mut ranges = SmallVec::with_capacity(selection.ranges.len());
  let mut cursor_ids = SmallVec::with_capacity(selection.ranges.len());

  for (cursor_id, sel) in selection.iter_with_ids() {
    let head = sel.head;
    let mut produced = 0;
    let mut assigned = false;
    let start_idx = ranges.len();

    // Special case: zero-width selection.
    if sel.from() == sel.to() {
      ranges.push(*sel);
      cursor_ids.push(cursor_id);
      continue;
    }

    let sel_start = sel.from();
    let sel_end = sel.to();
    let mut start = sel_start;

    for mat in regex.find_iter(text.regex_input_at(sel_start..sel_end)) {
      let end = text.byte_to_char(mat.start());
      let range = Range::new(start, end).with_direction(sel.direction());
      let use_id = if !assigned && range_contains_inclusive(&range, head) {
        assigned = true;
        cursor_id
      } else {
        CursorId::fresh()
      };
      ranges.push(range);
      cursor_ids.push(use_id);
      produced += 1;
      start = text.byte_to_char(mat.end());
    }

    if start < sel_end {
      let range = Range::new(start, sel_end).with_direction(sel.direction());
      let use_id = if !assigned && range_contains_inclusive(&range, head) {
        assigned = true;
        cursor_id
      } else {
        CursorId::fresh()
      };
      ranges.push(range);
      cursor_ids.push(use_id);
      produced += 1;
    }

    if produced > 0 && !assigned {
      cursor_ids[start_idx] = cursor_id;
    }
  }

  Selection::new_with_ids(ranges, cursor_ids)
}

fn range_contains_inclusive(range: &Range, pos: usize) -> bool {
  range.from() <= pos && pos <= range.to()
}

#[cfg(test)]
mod test {
  use std::num::NonZeroU64;

  use ropey::Rope;

  use super::*;

  #[test]
  fn test_new_empty() {
    let err = Selection::new(smallvec![]).unwrap_err();
    assert_eq!(err, SelectionError::EmptySelection);
  }

  #[test]
  fn test_new_with_ids_mismatch() {
    let err = Selection::new_with_ids(
      smallvec![Range::point(1), Range::point(2)],
      smallvec![CursorId::new(NonZeroU64::new(1).unwrap())],
    )
    .unwrap_err();
    assert_eq!(
      err,
      SelectionError::CursorIdCountMismatch {
        ids: 1,
        ranges: 2
      }
    );
  }

  #[test]
  fn test_pick_by_id() {
    let id = CursorId::new(NonZeroU64::new(7).unwrap());
    let sel = Selection::new_with_ids(
      smallvec![Range::point(1), Range::point(4)],
      smallvec![CursorId::new(NonZeroU64::new(1).unwrap()), id],
    )
    .unwrap();

    let (picked_id, range) = sel.pick(CursorPick::Id(id)).unwrap();
    assert_eq!(picked_id, id);
    assert_eq!(range, Range::point(4));
  }

  #[test]
  fn test_create_normalizes_and_merges() {
    let sel = Selection::new(
      smallvec![
        Range::new(10, 12),
        Range::new(6, 7),
        Range::new(4, 5),
        Range::new(3, 4),
        Range::new(0, 6),
        Range::new(7, 8),
        Range::new(9, 13),
        Range::new(13, 14),
      ],
    )
    .unwrap();

    let res = sel
      .ranges
      .into_iter()
      .map(|range| format!("{}/{}", range.anchor, range.head))
      .collect::<Vec<String>>()
      .join(",");

    assert_eq!(res, "0/6,6/7,7/8,9/13,13/14");
  }

  #[test]
  fn test_create_merges_adjacent_points() {
    let sel = Selection::new(
      smallvec![
        Range::new(10, 12),
        Range::new(12, 12),
        Range::new(12, 12),
        Range::new(10, 10),
        Range::new(8, 10),
      ],
    )
    .unwrap();

    let res = sel
      .ranges
      .into_iter()
      .map(|range| format!("{}/{}", range.anchor, range.head))
      .collect::<Vec<String>>()
      .join(",");

    assert_eq!(res, "8/10,10/12,12/12");
  }

  #[test]
  fn test_contains() {
    let range = Range::new(10, 12);

    assert!(!range.contains(9));
    assert!(range.contains(10));
    assert!(range.contains(11));
    assert!(!range.contains(12));
    assert!(!range.contains(13));

    let range = Range::new(9, 6);
    assert!(!range.contains(9));
    assert!(range.contains(7));
    assert!(range.contains(6));
  }

  #[test]
  fn test_overlaps() {
    fn overlaps(a: (usize, usize), b: (usize, usize)) -> bool {
      Range::new(a.0, a.1).overlaps(&Range::new(b.0, b.1))
    }

    // Two non-zero-width ranges, no overlap.
    assert!(!overlaps((0, 3), (3, 6)));
    assert!(!overlaps((0, 3), (6, 3)));
    assert!(!overlaps((3, 0), (3, 6)));
    assert!(!overlaps((3, 0), (6, 3)));
    assert!(!overlaps((3, 6), (0, 3)));
    assert!(!overlaps((3, 6), (3, 0)));
    assert!(!overlaps((6, 3), (0, 3)));
    assert!(!overlaps((6, 3), (3, 0)));

    // Two non-zero-width ranges, overlap.
    assert!(overlaps((0, 4), (3, 6)));
    assert!(overlaps((0, 4), (6, 3)));
    assert!(overlaps((4, 0), (3, 6)));
    assert!(overlaps((4, 0), (6, 3)));
    assert!(overlaps((3, 6), (0, 4)));
    assert!(overlaps((3, 6), (4, 0)));
    assert!(overlaps((6, 3), (0, 4)));
    assert!(overlaps((6, 3), (4, 0)));

    // Zero-width and non-zero-width range, no overlap.
    assert!(!overlaps((0, 3), (3, 3)));
    assert!(!overlaps((3, 0), (3, 3)));
    assert!(!overlaps((3, 3), (0, 3)));
    assert!(!overlaps((3, 3), (3, 0)));

    // Zero-width and non-zero-width range, overlap.
    assert!(overlaps((1, 4), (1, 1)));
    assert!(overlaps((4, 1), (1, 1)));
    assert!(overlaps((1, 1), (1, 4)));
    assert!(overlaps((1, 1), (4, 1)));

    assert!(overlaps((1, 4), (3, 3)));
    assert!(overlaps((4, 1), (3, 3)));
    assert!(overlaps((3, 3), (1, 4)));
    assert!(overlaps((3, 3), (4, 1)));

    // Two zero-width ranges, no overlap.
    assert!(!overlaps((0, 0), (1, 1)));
    assert!(!overlaps((1, 1), (0, 0)));

    // Two zero-width ranges, overlap.
    assert!(overlaps((1, 1), (1, 1)));
  }

  #[test]
  fn test_grapheme_aligned() {
    let r = Rope::from_str("\r\nHi\r\n");
    let s = r.slice(..);

    // Zero-width.
    assert_eq!(Range::new(0, 0).grapheme_aligned(s), Range::new(0, 0));
    assert_eq!(Range::new(1, 1).grapheme_aligned(s), Range::new(0, 0));
    assert_eq!(Range::new(2, 2).grapheme_aligned(s), Range::new(2, 2));
    assert_eq!(Range::new(3, 3).grapheme_aligned(s), Range::new(3, 3));
    assert_eq!(Range::new(4, 4).grapheme_aligned(s), Range::new(4, 4));
    assert_eq!(Range::new(5, 5).grapheme_aligned(s), Range::new(4, 4));
    assert_eq!(Range::new(6, 6).grapheme_aligned(s), Range::new(6, 6));

    // Forward.
    assert_eq!(Range::new(0, 1).grapheme_aligned(s), Range::new(0, 2));
    assert_eq!(Range::new(1, 2).grapheme_aligned(s), Range::new(0, 2));
    assert_eq!(Range::new(2, 3).grapheme_aligned(s), Range::new(2, 3));
    assert_eq!(Range::new(3, 4).grapheme_aligned(s), Range::new(3, 4));
    assert_eq!(Range::new(4, 5).grapheme_aligned(s), Range::new(4, 6));
    assert_eq!(Range::new(5, 6).grapheme_aligned(s), Range::new(4, 6));

    assert_eq!(Range::new(0, 2).grapheme_aligned(s), Range::new(0, 2));
    assert_eq!(Range::new(1, 3).grapheme_aligned(s), Range::new(0, 3));
    assert_eq!(Range::new(2, 4).grapheme_aligned(s), Range::new(2, 4));
    assert_eq!(Range::new(3, 5).grapheme_aligned(s), Range::new(3, 6));
    assert_eq!(Range::new(4, 6).grapheme_aligned(s), Range::new(4, 6));

    // Reverse.
    assert_eq!(Range::new(1, 0).grapheme_aligned(s), Range::new(2, 0));
    assert_eq!(Range::new(2, 1).grapheme_aligned(s), Range::new(2, 0));
    assert_eq!(Range::new(3, 2).grapheme_aligned(s), Range::new(3, 2));
    assert_eq!(Range::new(4, 3).grapheme_aligned(s), Range::new(4, 3));
    assert_eq!(Range::new(5, 4).grapheme_aligned(s), Range::new(6, 4));
    assert_eq!(Range::new(6, 5).grapheme_aligned(s), Range::new(6, 4));

    assert_eq!(Range::new(2, 0).grapheme_aligned(s), Range::new(2, 0));
    assert_eq!(Range::new(3, 1).grapheme_aligned(s), Range::new(3, 0));
    assert_eq!(Range::new(4, 2).grapheme_aligned(s), Range::new(4, 2));
    assert_eq!(Range::new(5, 3).grapheme_aligned(s), Range::new(6, 3));
    assert_eq!(Range::new(6, 4).grapheme_aligned(s), Range::new(6, 4));
  }

  #[test]
  fn test_min_width_1() {
    let r = Rope::from_str("\r\nHi\r\n");
    let s = r.slice(..);

    // Zero-width.
    assert_eq!(Range::new(0, 0).min_width_1(s), Range::new(0, 2));
    assert_eq!(Range::new(1, 1).min_width_1(s), Range::new(1, 2));
    assert_eq!(Range::new(2, 2).min_width_1(s), Range::new(2, 3));
    assert_eq!(Range::new(3, 3).min_width_1(s), Range::new(3, 4));
    assert_eq!(Range::new(4, 4).min_width_1(s), Range::new(4, 6));
    assert_eq!(Range::new(5, 5).min_width_1(s), Range::new(5, 6));
    assert_eq!(Range::new(6, 6).min_width_1(s), Range::new(6, 6));

    // Forward.
    assert_eq!(Range::new(0, 1).min_width_1(s), Range::new(0, 1));
    assert_eq!(Range::new(1, 2).min_width_1(s), Range::new(1, 2));
    assert_eq!(Range::new(2, 3).min_width_1(s), Range::new(2, 3));
    assert_eq!(Range::new(3, 4).min_width_1(s), Range::new(3, 4));
    assert_eq!(Range::new(4, 5).min_width_1(s), Range::new(4, 5));
    assert_eq!(Range::new(5, 6).min_width_1(s), Range::new(5, 6));

    // Reverse.
    assert_eq!(Range::new(1, 0).min_width_1(s), Range::new(1, 0));
    assert_eq!(Range::new(2, 1).min_width_1(s), Range::new(2, 1));
    assert_eq!(Range::new(3, 2).min_width_1(s), Range::new(3, 2));
    assert_eq!(Range::new(4, 3).min_width_1(s), Range::new(4, 3));
    assert_eq!(Range::new(5, 4).min_width_1(s), Range::new(5, 4));
    assert_eq!(Range::new(6, 5).min_width_1(s), Range::new(6, 5));
  }

  #[test]
  fn test_select_on_matches() {
    let r = Rope::from_str("Nobody expects the Spanish inquisition");
    let s = r.slice(..);

    let selection = Selection::single(0, r.len_chars());
    let result =
      select_on_matches(s, &selection, &rope::Regex::new(r"[A-Z][a-z]*").unwrap()).unwrap();
    assert_selection_ranges(result, &[Range::new(0, 6), Range::new(19, 26)]);

    let r = Rope::from_str("This\nString\n\ncontains multiple\nlines");
    let s = r.slice(..);

    let start_of_line = rope::RegexBuilder::new()
      .syntax(rope::Config::new().multi_line(true))
      .build(r"^")
      .unwrap();
    let end_of_line = rope::RegexBuilder::new()
      .syntax(rope::Config::new().multi_line(true))
      .build(r"$")
      .unwrap();

    // line without ending
    let result = select_on_matches(s, &Selection::single(0, 4), &start_of_line).unwrap();
    assert_selection_ranges(result, &[Range::point(0)]);
    assert!(select_on_matches(s, &Selection::single(0, 4), &end_of_line)
      .unwrap()
      .is_none());
    // line with ending
    let result = select_on_matches(s, &Selection::single(0, 5), &start_of_line).unwrap();
    assert_selection_ranges(result, &[Range::point(0)]);
    let result = select_on_matches(s, &Selection::single(0, 5), &end_of_line).unwrap();
    assert_selection_ranges(result, &[Range::new(4, 4)]);
    // line with start of next line
    let result = select_on_matches(s, &Selection::single(0, 6), &start_of_line).unwrap();
    assert_selection_ranges(result, &[Range::point(0), Range::point(5)]);
    let result = select_on_matches(s, &Selection::single(0, 6), &end_of_line).unwrap();
    assert_selection_ranges(result, &[Range::new(4, 4)]);

    // multiple lines
    let result = select_on_matches(
      s,
      &Selection::single(0, s.len_chars()),
      &rope::RegexBuilder::new()
        .syntax(rope::Config::new().multi_line(true))
        .build(r"^[a-z ]*$")
        .unwrap(),
    )
    .unwrap();
    assert_selection_ranges(
      result,
      &[Range::point(12), Range::new(13, 30), Range::new(31, 36)],
    );
  }

  #[test]
  fn test_line_range() {
    let r = Rope::from_str("\r\nHi\r\nthere!");
    let s = r.slice(..);

    // Zero-width ranges.
    assert_eq!(Range::new(0, 0).line_range(s), (0, 0));
    assert_eq!(Range::new(1, 1).line_range(s), (0, 0));
    assert_eq!(Range::new(2, 2).line_range(s), (1, 1));
    assert_eq!(Range::new(3, 3).line_range(s), (1, 1));

    // Forward ranges.
    assert_eq!(Range::new(0, 1).line_range(s), (0, 0));
    assert_eq!(Range::new(0, 2).line_range(s), (0, 0));
    assert_eq!(Range::new(0, 3).line_range(s), (0, 1));
    assert_eq!(Range::new(1, 2).line_range(s), (0, 0));
    assert_eq!(Range::new(2, 3).line_range(s), (1, 1));
    assert_eq!(Range::new(3, 8).line_range(s), (1, 2));
    assert_eq!(Range::new(0, 12).line_range(s), (0, 2));

    // Reverse ranges.
    assert_eq!(Range::new(1, 0).line_range(s), (0, 0));
    assert_eq!(Range::new(2, 0).line_range(s), (0, 0));
    assert_eq!(Range::new(3, 0).line_range(s), (0, 1));
    assert_eq!(Range::new(2, 1).line_range(s), (0, 0));
    assert_eq!(Range::new(3, 2).line_range(s), (1, 1));
    assert_eq!(Range::new(8, 3).line_range(s), (1, 2));
    assert_eq!(Range::new(12, 0).line_range(s), (0, 2));
  }

  #[test]
  fn test_cursor() {
    let r = Rope::from_str("\r\nHi\r\nthere!");
    let s = r.slice(..);

    // Zero-width ranges.
    assert_eq!(Range::new(0, 0).cursor(s), 0);
    assert_eq!(Range::new(2, 2).cursor(s), 2);
    assert_eq!(Range::new(3, 3).cursor(s), 3);

    // Forward ranges.
    assert_eq!(Range::new(0, 2).cursor(s), 0);
    assert_eq!(Range::new(0, 3).cursor(s), 2);
    assert_eq!(Range::new(3, 6).cursor(s), 4);

    // Reverse ranges.
    assert_eq!(Range::new(2, 0).cursor(s), 0);
    assert_eq!(Range::new(6, 2).cursor(s), 2);
    assert_eq!(Range::new(6, 3).cursor(s), 3);
  }

  #[test]
  fn test_put_cursor() {
    let r = Rope::from_str("\r\nHi\r\nthere!");
    let s = r.slice(..);

    // Zero-width ranges.
    assert_eq!(Range::new(0, 0).put_cursor(s, 0, true), Range::new(0, 2));
    assert_eq!(Range::new(0, 0).put_cursor(s, 2, true), Range::new(0, 3));
    assert_eq!(Range::new(2, 3).put_cursor(s, 4, true), Range::new(2, 6));
    assert_eq!(Range::new(2, 8).put_cursor(s, 4, true), Range::new(2, 6));
    assert_eq!(Range::new(8, 8).put_cursor(s, 4, true), Range::new(9, 4));

    // Forward ranges.
    assert_eq!(Range::new(3, 6).put_cursor(s, 0, true), Range::new(4, 0));
    assert_eq!(Range::new(3, 6).put_cursor(s, 2, true), Range::new(4, 2));
    assert_eq!(Range::new(3, 6).put_cursor(s, 3, true), Range::new(3, 4));
    assert_eq!(Range::new(3, 6).put_cursor(s, 4, true), Range::new(3, 6));
    assert_eq!(Range::new(3, 6).put_cursor(s, 6, true), Range::new(3, 7));
    assert_eq!(Range::new(3, 6).put_cursor(s, 8, true), Range::new(3, 9));

    // Reverse ranges.
    assert_eq!(Range::new(6, 3).put_cursor(s, 0, true), Range::new(6, 0));
    assert_eq!(Range::new(6, 3).put_cursor(s, 2, true), Range::new(6, 2));
    assert_eq!(Range::new(6, 3).put_cursor(s, 3, true), Range::new(6, 3));
    assert_eq!(Range::new(6, 3).put_cursor(s, 4, true), Range::new(6, 4));
    assert_eq!(Range::new(6, 3).put_cursor(s, 6, true), Range::new(4, 7));
    assert_eq!(Range::new(6, 3).put_cursor(s, 8, true), Range::new(4, 9));
  }

  #[test]
  fn test_split_on_matches() {
    let text = Rope::from(" abcd efg wrs   xyz 123 456");

    let selection = Selection::new(smallvec![Range::new(0, 9), Range::new(11, 20)]).unwrap();

    let result = split_on_matches(
      text.slice(..),
      &selection,
      &rope::Regex::new(r"\s+").unwrap(),
    )
    .unwrap();

    assert_eq!(result.ranges(), &[
      // TODO: rather than this behavior, maybe we want it
      // to be based on which side is the anchor?
      //
      // We get a leading zero-width range when there's
      // a leading match because ranges are inclusive on
      // the left.  Imagine, for example, if the entire
      // selection range were matched: you'd still want
      // at least one range to remain after the split.
      Range::new(0, 0),
      Range::new(1, 5),
      Range::new(6, 9),
      Range::new(11, 13),
      Range::new(16, 19),
      // In contrast to the comment above, there is no
      // _trailing_ zero-width range despite the trailing
      // match, because ranges are exclusive on the right.
    ]);

    assert_eq!(result.fragments(text.slice(..)).collect::<Vec<_>>(), &[
      "", "abcd", "efg", "rs", "xyz"
    ]);
  }

  #[test]
  fn test_merge_consecutive_ranges() {
    let selection = Selection::new(
      smallvec![
        Range::new(0, 1),
        Range::new(1, 10),
        Range::new(15, 20),
        Range::new(25, 26),
        Range::new(26, 30)
      ],
    )
    .unwrap();

    let result = selection.merge_consecutive_ranges();

    assert_eq!(result.ranges(), &[
      Range::new(0, 10),
      Range::new(15, 20),
      Range::new(25, 30)
    ]);

    let selection = Selection::new(smallvec![Range::new(0, 1)]).unwrap();
    let result = selection.merge_consecutive_ranges();

    assert_eq!(result.ranges(), &[Range::new(0, 1)]);

    let selection = Selection::new(
      smallvec![
        Range::new(0, 1),
        Range::new(1, 5),
        Range::new(5, 8),
        Range::new(8, 10),
        Range::new(10, 15),
        Range::new(18, 25)
      ],
    )
    .unwrap();

    let result = selection.merge_consecutive_ranges();

    assert_eq!(result.ranges(), &[Range::new(0, 15), Range::new(18, 25)]);
  }

  #[test]
  fn test_selection_contains() {
    fn contains(a: Vec<(usize, usize)>, b: Vec<(usize, usize)>) -> bool {
      let sel_a = Selection::new(a.iter().map(|a| Range::new(a.0, a.1)).collect()).unwrap();
      let sel_b = Selection::new(b.iter().map(|b| Range::new(b.0, b.1)).collect()).unwrap();
      sel_a.contains(&sel_b)
    }

    // exact match
    assert!(contains(vec!((1, 1)), vec!((1, 1))));

    // larger set contains smaller
    assert!(contains(vec!((1, 1), (2, 2), (3, 3)), vec!((2, 2))));

    // multiple matches
    assert!(contains(vec!((1, 1), (2, 2)), vec!((1, 1), (2, 2))));

    // smaller set can't contain bigger
    assert!(!contains(vec!((1, 1)), vec!((1, 1), (2, 2))));

    assert!(contains(
      vec!((1, 1), (2, 4), (5, 6), (7, 9), (10, 13)),
      vec!((3, 4), (7, 9))
    ));
    assert!(!contains(vec!((1, 1), (5, 6)), vec!((1, 6))));

    // multiple ranges of other are all contained in some ranges of self,
    assert!(contains(
      vec!((1, 4), (7, 10)),
      vec!((1, 2), (3, 4), (7, 9))
    ));
  }

  fn assert_selection_ranges(selection: Option<Selection>, expected: &[Range]) {
    let selection = selection.expect("expected selection");
    assert_eq!(selection.ranges(), expected);
  }
}
