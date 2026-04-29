use std::{
  borrow::Cow,
  collections::HashSet,
  iter,
  num::NonZeroU64,
  sync::atomic::{
    AtomicU64,
    Ordering,
  },
};

use ropey::RopeSlice;
use smallvec::{
  SmallVec,
  smallvec,
};
use the_stdx::rope::{
  self,
  RopeSliceExt,
};
use thiserror::Error;

use crate::{
  grapheme::{
    ensure_grapheme_boundary_next,
    ensure_grapheme_boundary_prev,
    next_grapheme_boundary,
    prev_grapheme_boundary,
  },
  line_ending::get_line_ending,
  movement::Direction,
  syntax::tree_sitter::Node,
  transaction::{
    Assoc,
    ChangeSet,
    MappedPos,
    TransactionError,
  },
};

pub type Result<T> = std::result::Result<T, SelectionError>;

#[derive(Debug, Error, PartialEq, Eq)]
#[non_exhaustive]
pub enum SelectionError {
  #[error("selection must contain at least one cursor range")]
  EmptySelection,
  #[error("range index {index} out of bounds for selection of length {len}")]
  RangeIndexOutOfBounds { index: usize, len: usize },
  #[error("cursor id count {ids} does not match range count {ranges}")]
  CursorIdCountMismatch { ids: usize, ranges: usize },
  #[error("cursor id {id} appears more than once in selection")]
  DuplicateCursorId { id: u64 },
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
pub enum MatchCapture {
  Whole,
  Group(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CursorRange {
  pub id:    CursorId,
  pub range: Range,
}

#[derive(Debug, Clone, Copy)]
pub struct CursorRangeRef<'a> {
  pub id:    CursorId,
  pub range: &'a Range,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Range {
  pub anchor: usize,
  pub head:   usize,
}

/// Result of mapping a [`Range`] through changes with delete tracking.
///
/// Provides information about whether the range's anchor and/or head
/// positions fell within deleted text during the mapping operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MappedRange {
  /// The mapped range.
  pub range: Range,

  /// Information about the anchor position mapping.
  /// `Some` if the anchor was in deleted text, `None` otherwise.
  pub anchor_deleted: Option<DeleteInfo>,

  /// Information about the head position mapping.
  /// `Some` if the head was in deleted text, `None` otherwise.
  pub head_deleted: Option<DeleteInfo>,
}

/// Information about a position that was inside deleted text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeleteInfo {
  /// The offset within the deleted range where the position was.
  pub offset: usize,

  /// The total length of the deletion.
  pub len: usize,
}

impl MappedRange {
  /// Returns `true` if either the anchor or head was in deleted text.
  #[inline]
  pub fn was_deleted(&self) -> bool {
    self.anchor_deleted.is_some() || self.head_deleted.is_some()
  }

  /// Returns `true` if the entire range was deleted (both anchor and head).
  #[inline]
  pub fn fully_deleted(&self) -> bool {
    self.anchor_deleted.is_some() && self.head_deleted.is_some()
  }

  /// Unwraps the inner range, discarding deletion information.
  #[inline]
  pub fn into_range(self) -> Range {
    self.range
  }
}

impl From<Range> for MappedRange {
  fn from(range: Range) -> Self {
    MappedRange {
      range,
      anchor_deleted: None,
      head_deleted: None,
    }
  }
}

/// Result of mapping a [`Selection`] through changes with delete tracking.
///
/// Contains both the updated selection and information about which ranges
/// had positions that fell within deleted text.
#[derive(Debug, Clone)]
pub struct MappedSelection {
  /// The updated selection after mapping through changes.
  pub selection: Selection,

  /// Mapping information for each original range, in the same order.
  /// Note: After normalization, the selection may have fewer ranges
  /// than `mapped_ranges` due to merging.
  pub mapped_ranges: SmallVec<[MappedRange; 1]>,
}

impl MappedSelection {
  /// Returns `true` if any range had a position in deleted text.
  pub fn any_deleted(&self) -> bool {
    self.mapped_ranges.iter().any(|mr| mr.was_deleted())
  }

  /// Returns `true` if all ranges were fully deleted.
  pub fn all_fully_deleted(&self) -> bool {
    self.mapped_ranges.iter().all(|mr| mr.fully_deleted())
  }

  /// Unwraps the selection, discarding deletion information.
  pub fn into_selection(self) -> Selection {
    self.selection
  }
}

impl Range {
  pub fn new(anchor: usize, head: usize) -> Self {
    Self { anchor, head }
  }

  pub fn from_node(node: Node, text: RopeSlice, direction: Direction) -> Self {
    let from = text.byte_to_char(node.start_byte() as usize);
    let to = text.byte_to_char(node.end_byte() as usize);
    Range::new(from, to).with_direction(direction)
  }

  #[inline]
  pub fn point(head: usize) -> Self {
    Self::new(head, head)
  }

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
      return prev_grapheme_boundary(text, self.head);
    }
    self.head
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
    let to = match self.is_empty() {
      true => self.to(),
      false => prev_grapheme_boundary(slice, self.to()).max(from),
    };

    (slice.char_to_line(from), slice.char_to_line(to))
  }

  #[inline]
  #[must_use]
  pub fn direction(&self) -> Direction {
    if self.head < self.anchor {
      return Direction::Backward;
    }
    Direction::Forward
  }

  /// Flips the direction of the selection
  #[inline]
  #[must_use]
  pub fn flip(&self) -> Self {
    // NOTE: We're returning Self directly here instead of Self::new() for clarity.
    Self {
      anchor: self.head,
      head:   self.anchor,
    }
  }

  /// Returns the selection if we're going the same way as `direction`,
  /// else, flip it.
  #[inline]
  #[must_use]
  pub fn with_direction(self, direction: Direction) -> Self {
    if self.direction() == direction {
      return self;
    }
    self.flip()
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
    if !extend {
      return Range::point(char_idx);
    }

    let anchor = match (self.head >= self.anchor, char_idx < self.anchor) {
      (true, true) => next_grapheme_boundary(slice, self.anchor),
      (false, false) => prev_grapheme_boundary(slice, self.anchor),
      _ => self.anchor,
    };

    if anchor <= char_idx {
      return Range::new(anchor, next_grapheme_boundary(slice, char_idx));
    }

    Range::new(anchor, char_idx)
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
    Ok(self)
  }

  /// Map a range through changes with delete tracking.
  ///
  /// Unlike [`map`], this method returns a [`MappedRange`] that includes
  /// information about whether the anchor and/or head positions fell within
  /// deleted text.
  ///
  /// This is useful for:
  /// - Detecting when a cursor position was "destroyed" by a deletion
  /// - Preserving information for undo/redo operations
  /// - Tracking selection changes through edits
  ///
  /// # Example
  ///
  /// ```ignore
  /// let mapped = range.map_tracked(&changes)?;
  /// if mapped.fully_deleted() {
  ///     // The entire range was inside deleted text
  /// }
  /// ```
  pub fn map_tracked(self, changes: &ChangeSet) -> Result<MappedRange> {
    use std::cmp::Ordering;

    if changes.is_empty() {
      return Ok(MappedRange::from(self));
    }

    // Determine which Assoc to use based on direction, mirroring Range::map
    // behavior. For forward selection (anchor < head): anchor expands
    // (AfterSticky), head contracts (BeforeSticky) For backward selection
    // (anchor > head): anchor contracts, head expands For point (anchor ==
    // head): both expand (AfterSticky behavior)
    let (anchor_assoc, head_assoc) = match self.anchor.cmp(&self.head) {
      Ordering::Equal => (Assoc::TrackDelAfterSticky, Assoc::TrackDelAfterSticky),
      Ordering::Less => (Assoc::TrackDelAfterSticky, Assoc::TrackDelBeforeSticky),
      Ordering::Greater => (Assoc::TrackDelBeforeSticky, Assoc::TrackDelAfterSticky),
    };

    let anchor_result = changes.map_pos_tracked(self.anchor, anchor_assoc)?;
    let head_result = changes.map_pos_tracked(self.head, head_assoc)?;

    let anchor_deleted = match anchor_result {
      MappedPos::Deleted { offset, len, .. } => Some(DeleteInfo { offset, len }),
      MappedPos::Pos(_) => None,
    };

    let head_deleted = match head_result {
      MappedPos::Deleted { offset, len, .. } => Some(DeleteInfo { offset, len }),
      MappedPos::Pos(_) => None,
    };

    let range = Range {
      anchor: anchor_result.position(),
      head:   head_result.position(),
    };

    Ok(MappedRange {
      range,
      anchor_deleted,
      head_deleted,
    })
  }

  /// Extend the range to cover at least `from` `to`.
  #[must_use]
  pub fn extend(&self, from: usize, to: usize) -> Self {
    debug_assert!(from <= to);

    if self.anchor <= self.head {
      return Self::new(self.anchor.min(from), self.head.max(to));
    }
    Self::new(self.anchor.max(to), self.head.min(from))
  }

  /// Returns a `Range` that encompasses both input ranges.
  ///
  /// This is much like [Self::extend()] but tries to negotiate the anchor/head
  /// ordering between the two input ranges.
  pub fn merge(&self, other: Self) -> Self {
    if self.anchor > self.head && other.anchor > other.head {
      return Self::new(self.anchor.max(other.anchor), self.head.min(other.head));
    }
    Self::new(self.from().min(other.from()), self.to().max(other.to()))
  }

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
      anchor: new_anchor,
      head:   new_head,
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
      return Range::new(self.anchor, next_grapheme_boundary(slice, self.head));
    }
    *self
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
  ranges:     SmallVec<[Range; 1]>,
  cursor_ids: SmallVec<[CursorId; 1]>,
}

impl Selection {
  pub fn new(ranges: SmallVec<[Range; 1]>) -> Result<Self> {
    if ranges.is_empty() {
      return Err(SelectionError::EmptySelection);
    }
    let cursor_ids = ranges.iter().map(|_| CursorId::fresh()).collect();
    Ok(Self::new_with_ids_unchecked(ranges, cursor_ids).normalize())
  }

  pub fn from_cursors(cursors: SmallVec<[CursorRange; 1]>) -> Result<Self> {
    if cursors.is_empty() {
      return Err(SelectionError::EmptySelection);
    }
    let ranges = cursors.iter().map(|cursor| cursor.range).collect();
    let cursor_ids = cursors.iter().map(|cursor| cursor.id).collect();
    Self::new_with_ids(ranges, cursor_ids)
  }

  pub(crate) fn new_with_ids(
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
    ensure_unique_ids(&cursor_ids)?;

    Ok(Self::new_with_ids_unchecked(ranges, cursor_ids).normalize())
  }

  pub(crate) fn new_with_ids_unchecked(
    ranges: SmallVec<[Range; 1]>,
    cursor_ids: SmallVec<[CursorId; 1]>,
  ) -> Self {
    Self { ranges, cursor_ids }
  }

  pub fn point(pos: usize) -> Self {
    Self::new_with_ids_unchecked(smallvec![Range::point(pos)], smallvec![CursorId::fresh()])
  }

  /// Low-level escape hatch for callers that need raw ranges.
  ///
  /// Prefer the cursor-entry-centric [`Selection::iter`] API for normal use;
  /// ranges and cursor IDs are conceptually paired.
  pub fn ranges(&self) -> &[Range] {
    &self.ranges
  }

  /// Low-level escape hatch for callers that need raw cursor IDs.
  ///
  /// Prefer the cursor-entry-centric [`Selection::iter`] API for normal use;
  /// ranges and cursor IDs are conceptually paired.
  pub fn cursor_ids(&self) -> &[CursorId] {
    &self.cursor_ids
  }

  pub fn iter(&self) -> impl Iterator<Item = CursorRangeRef<'_>> {
    self
      .cursor_ids
      .iter()
      .copied()
      .zip(self.ranges.iter())
      .map(|(id, range)| CursorRangeRef { id, range })
  }

  pub fn get(&self, idx: usize) -> Result<CursorRangeRef<'_>> {
    self
      .ranges
      .get(idx)
      .map(|range| {
        CursorRangeRef {
          id: self.cursor_ids[idx],
          range,
        }
      })
      .ok_or_else(|| {
        SelectionError::RangeIndexOutOfBounds {
          index: idx,
          len:   self.ranges.len(),
        }
      })
  }

  pub fn by_id(&self, id: CursorId) -> Option<CursorRangeRef<'_>> {
    self.index_of(id).map(|idx| {
      CursorRangeRef {
        id,
        range: &self.ranges[idx],
      }
    })
  }

  pub fn index_of(&self, id: CursorId) -> Option<usize> {
    self
      .cursor_ids
      .iter()
      .position(|cursor_id| *cursor_id == id)
  }

  pub fn range_by_id(&self, id: CursorId) -> Option<&Range> {
    self.index_of(id).and_then(|idx| self.ranges.get(idx))
  }

  fn range_at(&self, idx: usize) -> Result<Range> {
    self.ranges.get(idx).copied().ok_or_else(|| {
      SelectionError::RangeIndexOutOfBounds {
        index: idx,
        len:   self.ranges.len(),
      }
    })
  }

  pub fn range_mut(&mut self, idx: usize) -> Result<&mut Range> {
    let len = self.ranges.len();
    self
      .ranges
      .get_mut(idx)
      .ok_or_else(|| SelectionError::RangeIndexOutOfBounds { index: idx, len })
  }

  pub fn pick(&self, pick: CursorPick) -> Result<CursorRange> {
    match pick {
      CursorPick::First => {
        Ok(CursorRange {
          id:    self.cursor_ids[0],
          range: self.ranges[0],
        })
      },
      CursorPick::Last => {
        let idx = self.ranges.len() - 1;
        Ok(CursorRange {
          id:    self.cursor_ids[idx],
          range: self.ranges[idx],
        })
      },
      CursorPick::Index(idx) => {
        self.range_at(idx).map(|range| {
          CursorRange {
            id: self.cursor_ids[idx],
            range,
          }
        })
      },
      CursorPick::Id(id) => {
        self
          .index_of(id)
          .map(|idx| {
            CursorRange {
              id:    self.cursor_ids[idx],
              range: self.ranges[idx],
            }
          })
          .ok_or(SelectionError::CursorIdNotFound { id: id.get() })
      },
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
        Ok(CursorRange {
          id:    self.cursor_ids[idx],
          range: self.ranges[idx],
        })
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
      return self;
    }

    Self::new_with_ids_unchecked(smallvec![self.ranges[0]], smallvec![self.cursor_ids[0]])
  }

  /// Adds a new range to the selection.
  pub fn push(mut self, range: Range) -> Self {
    self.ranges.push(range);
    self.cursor_ids.push(CursorId::fresh());
    self.normalize()
  }

  pub fn push_with_id(mut self, range: Range, cursor_id: CursorId) -> Result<Self> {
    if self
      .cursor_ids
      .iter()
      .any(|existing| *existing == cursor_id)
    {
      return Err(SelectionError::DuplicateCursorId {
        id: cursor_id.get(),
      });
    }
    self.ranges.push(range);
    self.cursor_ids.push(cursor_id);
    Ok(self.normalize())
  }

  pub fn collapse(self, pick: CursorPick) -> Result<Self> {
    let cursor = self.pick(pick)?;
    Ok(Selection::new_with_ids_unchecked(
      smallvec![cursor.range],
      smallvec![cursor.id],
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

  /// Map selections over changes with delete tracking.
  ///
  /// Returns a [`MappedSelection`] containing both the updated selection and
  /// information about which ranges had their anchor/head positions deleted.
  ///
  /// This is useful for detecting when cursors were "destroyed" by deletions
  /// and may need special handling (e.g., repositioning or removal).
  pub fn map_tracked(self, changes: &ChangeSet) -> Result<MappedSelection> {
    let mut mapped_ranges = SmallVec::with_capacity(self.ranges.len());
    let mut ranges = SmallVec::with_capacity(self.ranges.len());

    for range in &self.ranges {
      let mapped = range.map_tracked(changes)?;
      ranges.push(mapped.range);
      mapped_ranges.push(mapped);
    }

    let selection = Selection::new_with_ids_unchecked(ranges, self.cursor_ids).normalize();

    Ok(MappedSelection {
      selection,
      mapped_ranges,
    })
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
    Self::new_with_ids_unchecked(smallvec![Range::new(anchor, head)], smallvec![
      CursorId::fresh()
    ])
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

    pairs.sort_by_key(|(range, _)| (range.from(), range.to()));

    let mut ranges: SmallVec<[Range; 1]> = SmallVec::with_capacity(pairs.len());
    let mut cursor_ids: SmallVec<[CursorId; 1]> = SmallVec::with_capacity(pairs.len());

    for (range, cursor_id) in pairs {
      let Some(prev) = ranges.last().copied() else {
        ranges.push(range);
        cursor_ids.push(cursor_id);
        continue;
      };

      if !prev.overlaps(&range) {
        ranges.push(range);
        cursor_ids.push(cursor_id);
        continue;
      }

      let merged = prev.merge(range);
      let prev_id = *cursor_ids.last().expect("range must have cursor id");

      *ranges.last_mut().expect("range must exist") = merged;
      *cursor_ids.last_mut().expect("cursor id must exist") =
        merge_cursor_id(prev, prev_id, range, cursor_id, merged);
    }

    self.ranges = ranges;
    self.cursor_ids = cursor_ids;
    self
  }

  pub fn merge_ranges(self) -> Self {
    let merged = self
      .ranges
      .iter()
      .copied()
      .reduce(|acc, range| acc.merge(range))
      .expect("selection is non-empty");
    let (id, _) = self
      .ranges
      .iter()
      .copied()
      .zip(self.cursor_ids.iter().copied())
      .map(|(range, cursor_id)| {
        let dist = merged.head.abs_diff(range.head);
        (cursor_id, dist)
      })
      .min_by_key(|(_, dist)| *dist)
      .expect("selection is non-empty");
    Selection::new_with_ids_unchecked(smallvec![merged], smallvec![id])
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
          let prev = *prev_range;
          let merged = prev.merge(range);
          let keep_id =
            merge_cursor_id(prev, *cursor_ids.last().unwrap(), range, cursor_id, merged);
          *prev_range = merged;
          *cursor_ids.last_mut().unwrap() = keep_id;
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

  pub fn transform_entries<F>(mut self, mut f: F) -> Result<Self>
  where
    F: FnMut(CursorRange) -> CursorRange,
  {
    let mut ranges = SmallVec::with_capacity(self.ranges.len());
    let mut cursor_ids = SmallVec::with_capacity(self.cursor_ids.len());
    for (range, id) in self.ranges.into_iter().zip(self.cursor_ids.into_iter()) {
      let cursor = f(CursorRange { id, range });
      ranges.push(cursor.range);
      cursor_ids.push(cursor.id);
    }
    ensure_unique_ids(&cursor_ids)?;
    self.ranges = ranges;
    self.cursor_ids = cursor_ids;
    Ok(self.normalize())
  }

  pub fn flat_map_entries<I>(mut self, mut f: impl FnMut(CursorRange) -> I) -> Result<Self>
  where
    I: Iterator<Item = CursorRange>,
  {
    let mut ranges = SmallVec::new();
    let mut cursor_ids = SmallVec::new();
    for (range, id) in self.ranges.into_iter().zip(self.cursor_ids.into_iter()) {
      for cursor in f(CursorRange { id, range }) {
        ranges.push(cursor.range);
        cursor_ids.push(cursor.id);
      }
    }
    if ranges.is_empty() {
      return Err(SelectionError::NoRanges);
    }
    ensure_unique_ids(&cursor_ids)?;
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

  pub fn range_bounds(&self) -> impl Iterator<Item = the_stdx::range::Range> + '_ {
    self.ranges.iter().map(|&range| range.into())
  }

  pub fn contains(&self, other: &Selection) -> bool {
    other.ranges.iter().all(|needle| {
      self
        .ranges
        .iter()
        .any(|haystack| haystack.contains_range(needle))
    })
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

  for cursor in selection.iter() {
    if regex.is_match(text.regex_input_at(cursor.range.from()..cursor.range.to())) ^ remove {
      ranges.push(*cursor.range);
      cursor_ids.push(cursor.id);
    }
  }

  if ranges.is_empty() {
    return Ok(None);
  }
  Ok(Some(Selection::new_with_ids(ranges, cursor_ids)?))
}

pub fn select_on_matches(
  text: RopeSlice,
  selection: &Selection,
  regex: &rope::Regex,
) -> Result<Option<Selection>> {
  select_on_matches_with(text, selection, regex, MatchCapture::Whole)
}

fn push_range_preserving_cursor_id(
  ranges: &mut SmallVec<[Range; 1]>,
  cursor_ids: &mut SmallVec<[CursorId; 1]>,
  range: Range,
  cursor: CursorRangeRef<'_>,
  assigned: &mut bool,
) {
  let id = if !*assigned && range_contains_inclusive(&range, cursor.range.head) {
    *assigned = true;
    cursor.id
  } else {
    CursorId::fresh()
  };

  ranges.push(range);
  cursor_ids.push(id);
}

pub fn select_on_matches_with(
  text: RopeSlice,
  selection: &Selection,
  regex: &rope::Regex,
  capture: MatchCapture,
) -> Result<Option<Selection>> {
  let mut ranges = SmallVec::with_capacity(selection.ranges.len());
  let mut cursor_ids = SmallVec::with_capacity(selection.ranges.len());

  for cursor in selection.iter() {
    let sel = cursor.range;
    let start_idx = ranges.len();
    let mut assigned = false;

    let matches: SmallVec<[Range; 1]> = match capture {
      MatchCapture::Whole => {
        regex
          .find_iter(text.regex_input_at(sel.from()..sel.to()))
          .map(|mat| Range::new(text.byte_to_char(mat.start()), text.byte_to_char(mat.end())))
          .collect()
      },

      MatchCapture::Group(index) => {
        regex
          .captures_iter(text.regex_input_at(sel.from()..sel.to()))
          .filter_map(|caps| caps.get_group(index))
          .map(|span| Range::new(text.byte_to_char(span.start), text.byte_to_char(span.end)))
          .collect()
      },
    };

    for range in matches {
      let range = range.with_direction(sel.direction());

      if range == Range::point(sel.to()) {
        continue;
      }

      push_range_preserving_cursor_id(&mut ranges, &mut cursor_ids, range, cursor, &mut assigned);
    }

    if ranges.len() > start_idx && !assigned {
      cursor_ids[start_idx] = cursor.id;
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

  for cursor in selection.iter() {
    let sel = cursor.range;

    if sel.is_empty() {
      ranges.push(*sel);
      cursor_ids.push(cursor.id);
      continue;
    }

    let start_idx = ranges.len();
    let mut assigned = false;
    let mut start = sel.from();
    let end = sel.to();

    for line in sel.slice(text).lines() {
      let Some(line_ending) = get_line_ending(&line) else {
        break;
      };

      let line_end = start + line.len_chars();
      let range =
        Range::new(start, line_end - line_ending.len_chars()).with_direction(sel.direction());

      push_range_preserving_cursor_id(&mut ranges, &mut cursor_ids, range, cursor, &mut assigned);

      start = line_end;
    }

    if start < end {
      push_range_preserving_cursor_id(
        &mut ranges,
        &mut cursor_ids,
        Range::new(start, end).with_direction(sel.direction()),
        cursor,
        &mut assigned,
      );
    }

    if ranges.len() > start_idx && !assigned {
      cursor_ids[start_idx] = cursor.id;
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

  for cursor in selection.iter() {
    let sel = cursor.range;

    if sel.is_empty() {
      ranges.push(*sel);
      cursor_ids.push(cursor.id);
      continue;
    }

    let start_idx = ranges.len();
    let mut assigned = false;
    let mut start = sel.from();
    let end = sel.to();

    for mat in regex.find_iter(text.regex_input_at(start..end)) {
      let split = text.byte_to_char(mat.start());

      if start != split {
        push_range_preserving_cursor_id(
          &mut ranges,
          &mut cursor_ids,
          Range::new(start, split).with_direction(sel.direction()),
          cursor,
          &mut assigned,
        );
      }

      start = text.byte_to_char(mat.end());
    }

    if start < end {
      push_range_preserving_cursor_id(
        &mut ranges,
        &mut cursor_ids,
        Range::new(start, end).with_direction(sel.direction()),
        cursor,
        &mut assigned,
      );
    }

    if ranges.len() == start_idx {
      ranges.push(Range::point(sel.head));
      cursor_ids.push(cursor.id);
    } else if !assigned {
      cursor_ids[start_idx] = cursor.id;
    }
  }

  Selection::new_with_ids(ranges, cursor_ids)
}

fn range_contains_inclusive(range: &Range, pos: usize) -> bool {
  range.from() <= pos && pos <= range.to()
}

fn ensure_unique_ids(ids: &[CursorId]) -> Result<()> {
  match ids.len() {
    0 | 1 => Ok(()),
    2..=8 => {
      for (idx, id) in ids.iter().enumerate() {
        if ids[..idx].contains(id) {
          return Err(SelectionError::DuplicateCursorId { id: id.get() });
        }
      }

      Ok(())
    },
    _ => {
      let mut seen = HashSet::with_capacity(ids.len());

      for id in ids {
        if !seen.insert(*id) {
          return Err(SelectionError::DuplicateCursorId { id: id.get() });
        }
      }

      Ok(())
    },
  }
}

fn merge_cursor_id(
  prev: Range,
  prev_id: CursorId,
  next: Range,
  next_id: CursorId,
  merged: Range,
) -> CursorId {
  if prev_id == next_id {
    return prev_id;
  }

  let prev_dist = merged.head.abs_diff(prev.head);
  let next_dist = merged.head.abs_diff(next.head);
  if prev_dist <= next_dist {
    return prev_id;
  }

  next_id
}

#[cfg(test)]
mod test {
  use std::num::NonZeroU64;

  use ropey::Rope;

  use super::*;

  fn id(n: u64) -> CursorId {
    CursorId::new(NonZeroU64::new(n).unwrap())
  }

  fn cursor(cursor_id: u64, anchor: usize, head: usize) -> CursorRange {
    CursorRange {
      id:    id(cursor_id),
      range: Range::new(anchor, head),
    }
  }

  #[test]
  fn constructors_reject_empty_and_duplicate_cursor_entries() {
    assert_eq!(
      Selection::new(smallvec![]).unwrap_err(),
      SelectionError::EmptySelection
    );
    assert_eq!(
      Selection::from_cursors(smallvec![]).unwrap_err(),
      SelectionError::EmptySelection
    );
    assert_eq!(
      Selection::from_cursors(smallvec![cursor(1, 1, 1), cursor(1, 2, 2)]).unwrap_err(),
      SelectionError::DuplicateCursorId { id: 1 },
    );
  }

  #[test]
  fn from_cursors_normalizes_ranges_and_preserves_the_closest_surviving_id() {
    let selection =
      Selection::from_cursors(smallvec![cursor(1, 7, 9), cursor(2, 1, 4), cursor(3, 3, 8),])
        .unwrap();

    let cursors = selection
      .iter()
      .map(|cursor| (cursor.id.get(), *cursor.range))
      .collect::<Vec<_>>();

    assert_eq!(cursors, vec![(1, Range::new(1, 9))]);
  }

  #[test]
  fn iter_get_by_id_and_pick_expose_cursor_entries() {
    let selection = Selection::from_cursors(smallvec![
      cursor(3, 5, 5),
      cursor(1, 1, 2),
      cursor(2, 8, 10),
    ])
    .unwrap();

    let iterated = selection
      .iter()
      .map(|cursor| (cursor.id.get(), *cursor.range))
      .collect::<Vec<_>>();
    assert_eq!(iterated, vec![
      (1, Range::new(1, 2)),
      (3, Range::new(5, 5)),
      (2, Range::new(8, 10)),
    ]);

    let second = selection.get(1).unwrap();
    assert_eq!(second.id.get(), 3);
    assert_eq!(*second.range, Range::new(5, 5));

    let by_id = selection.by_id(id(2)).unwrap();
    assert_eq!(*by_id.range, Range::new(8, 10));

    assert_eq!(selection.pick(CursorPick::First).unwrap().id.get(), 1);
    assert_eq!(selection.pick(CursorPick::Last).unwrap().id.get(), 2);
    assert_eq!(selection.pick(CursorPick::Nearest(6)).unwrap().id.get(), 3);
  }

  #[test]
  fn contains_requires_every_other_range_to_fit_inside_some_self_range() {
    let outer = Selection::new(smallvec![Range::new(1, 4), Range::new(7, 10)]).unwrap();
    let inner = Selection::new(smallvec![
      Range::new(1, 2),
      Range::new(3, 4),
      Range::new(7, 9)
    ])
    .unwrap();
    let crossing_gap = Selection::new(smallvec![Range::new(1, 6)]).unwrap();

    assert!(outer.contains(&inner));
    assert!(!outer.contains(&crossing_gap));
  }

  #[test]
  fn transform_entries_preserves_ids() {
    let selection = Selection::from_cursors(smallvec![cursor(1, 0, 1), cursor(2, 4, 5)]).unwrap();

    let transformed = selection
      .transform_entries(|cursor| {
        CursorRange {
          id:    cursor.id,
          range: Range::new(cursor.range.anchor + 1, cursor.range.head + 1),
        }
      })
      .unwrap();

    let cursors = transformed
      .iter()
      .map(|cursor| (cursor.id.get(), *cursor.range))
      .collect::<Vec<_>>();

    assert_eq!(cursors, vec![(1, Range::new(1, 2)), (2, Range::new(5, 6))]);
  }

  #[test]
  fn transform_entries_rejects_duplicate_ids() {
    let selection = Selection::from_cursors(smallvec![cursor(1, 0, 1), cursor(2, 4, 5)]).unwrap();

    let err = selection
      .transform_entries(|cursor| {
        CursorRange {
          id:    id(1),
          range: cursor.range,
        }
      })
      .unwrap_err();

    assert_eq!(err, SelectionError::DuplicateCursorId { id: 1 });
  }

  #[test]
  fn flat_map_entries_can_split_a_cursor_and_reject_empty_output() {
    let split = Selection::from_cursors(smallvec![cursor(1, 0, 4)])
      .unwrap()
      .flat_map_entries(|cursor| {
        vec![
          CursorRange {
            id:    cursor.id,
            range: Range::new(0, 2),
          },
          CursorRange {
            id:    id(2),
            range: Range::new(2, 4),
          },
        ]
        .into_iter()
      })
      .unwrap();

    let cursors = split
      .iter()
      .map(|cursor| (cursor.id.get(), *cursor.range))
      .collect::<Vec<_>>();
    assert_eq!(cursors, vec![(1, Range::new(0, 2)), (2, Range::new(2, 4))]);

    let err = Selection::from_cursors(smallvec![cursor(1, 0, 4)])
      .unwrap()
      .flat_map_entries(|_| std::iter::empty())
      .unwrap_err();
    assert_eq!(err, SelectionError::NoRanges);
  }

  #[test]
  fn map_preserves_cursor_ids_when_text_is_inserted() {
    let selection = Selection::from_cursors(smallvec![cursor(1, 2, 2), cursor(2, 4, 6)]).unwrap();
    let tx = crate::transaction::Transaction::insert_at(8, 3, "!!").unwrap();

    let mapped = selection.map(tx.changes()).unwrap();
    let cursors = mapped
      .iter()
      .map(|cursor| (cursor.id.get(), *cursor.range))
      .collect::<Vec<_>>();

    assert_eq!(cursors, vec![(1, Range::point(2)), (2, Range::new(6, 8))]);
  }

  #[test]
  fn map_tracked_reports_which_cursors_were_deleted() {
    let selection = Selection::from_cursors(smallvec![cursor(1, 2, 2), cursor(2, 6, 6)]).unwrap();
    let tx =
      crate::transaction::Transaction::delete(12, the_stdx::range::Range { start: 4, end: 8 })
        .unwrap();

    let mapped = selection.map_tracked(tx.changes()).unwrap();

    assert!(mapped.any_deleted());
    assert!(!mapped.all_fully_deleted());
    assert!(!mapped.mapped_ranges[0].was_deleted());
    assert!(mapped.mapped_ranges[1].was_deleted());
    assert_eq!(
      mapped
        .selection
        .iter()
        .map(|cursor| cursor.id.get())
        .collect::<Vec<_>>(),
      vec![1, 2]
    );
  }

  #[test]
  fn transform_entries_returns_a_normalized_selection() {
    let selection = Selection::from_cursors(smallvec![cursor(1, 0, 2), cursor(2, 4, 6)]).unwrap();

    let transformed = selection
      .transform_entries(|cursor| {
        match cursor.id.get() {
          1 => {
            CursorRange {
              id:    cursor.id,
              range: cursor.range,
            }
          },
          2 => {
            CursorRange {
              id:    cursor.id,
              range: Range::new(1, 5),
            }
          },
          _ => unreachable!(),
        }
      })
      .unwrap();

    let cursors = transformed
      .iter()
      .map(|cursor| (cursor.id.get(), *cursor.range))
      .collect::<Vec<_>>();

    assert_eq!(cursors, vec![(2, Range::new(0, 5))]);
  }

  #[test]
  fn merge_ranges_keeps_the_cursor_id_closest_to_the_merged_head() {
    let selection = Selection::from_cursors(smallvec![
      cursor(1, 0, 2),
      cursor(2, 4, 6),
      cursor(3, 8, 10),
    ])
    .unwrap();

    let merged = selection.merge_ranges();
    let cursor = merged.pick(CursorPick::First).unwrap();

    assert_eq!(cursor.id.get(), 3);
    assert_eq!(cursor.range, Range::new(0, 10));
  }

  #[test]
  fn merge_consecutive_ranges_keeps_the_id_of_the_range_closest_to_the_merged_head() {
    let selection = Selection::from_cursors(smallvec![
      cursor(1, 0, 2),
      cursor(2, 2, 5),
      cursor(3, 8, 10),
    ])
    .unwrap();

    let merged = selection.merge_consecutive_ranges();
    let cursors = merged
      .iter()
      .map(|cursor| (cursor.id.get(), *cursor.range))
      .collect::<Vec<_>>();

    assert_eq!(cursors, vec![(2, Range::new(0, 5)), (3, Range::new(8, 10))]);
  }

  #[test]
  fn range_line_range_reports_the_lines_touched_by_forward_and_point_ranges() {
    let text = Rope::from_str("a\nb\nc");
    let slice = text.slice(..);

    assert_eq!(Range::point(2).line_range(slice), (1, 1));
    assert_eq!(Range::new(0, 3).line_range(slice), (0, 1));
    assert_eq!(Range::new(2, 5).line_range(slice), (1, 2));
  }

  #[test]
  fn range_overlaps_distinguishes_touching_ranges_from_shared_points() {
    assert!(!Range::new(0, 3).overlaps(&Range::new(3, 6)));
    assert!(Range::new(1, 4).overlaps(&Range::new(3, 3)));
    assert!(Range::new(1, 1).overlaps(&Range::new(1, 1)));
  }

  #[test]
  fn range_helpers_are_stable_on_simple_ascii_text() {
    let text = Rope::from_str("abcd");
    let slice = text.slice(..);

    assert_eq!(Range::new(1, 3).grapheme_aligned(slice), Range::new(1, 3));
    assert_eq!(Range::new(2, 2).min_width_1(slice), Range::new(2, 3));
    assert_eq!(Range::new(1, 3).cursor(slice), 2);
  }

  #[test]
  fn put_cursor_extends_and_collapses_on_simple_ascii_text() {
    let text = Rope::from_str("abcd");
    let slice = text.slice(..);

    assert_eq!(
      Range::new(1, 3).put_cursor(slice, 0, true),
      Range::new(2, 0)
    );
    assert_eq!(
      Range::new(1, 3).put_cursor(slice, 2, true),
      Range::new(1, 3)
    );
    assert_eq!(
      Range::new(1, 3).put_cursor(slice, 2, false),
      Range::point(2)
    );
  }

  #[test]
  fn select_on_matches_can_return_capture_groups() {
    let text = Rope::from_str("foo1 foo22 bar333");
    let selection = Selection::single(0, text.len_chars());

    let result = select_on_matches_with(
      text.slice(..),
      &selection,
      &rope::Regex::new(r"foo([0-9]+)").unwrap(),
      MatchCapture::Group(1),
    )
    .unwrap()
    .unwrap();

    assert_eq!(result.ranges(), &[Range::new(3, 4), Range::new(8, 10)]);
  }
}
