use std::{
  borrow::Cow,
  iter,
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
  #[error("primary index {index} out of bounds for selection of length {len}")]
  PrimaryIndexOutOfBounds { index: usize, len: usize },
  #[error("range index {index} out of bounds for selection of length {len}")]
  RangeIndexOutOfBounds { index: usize, len: usize },
  #[error("cannot remove the last range from a selection")]
  RemoveLastRange,
  #[error("selection transform produced no ranges")]
  NoRanges,
  #[error(transparent)]
  Transaction(#[from] TransactionError),
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
/// primary range).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selection {
  ranges:        SmallVec<[Range; 1]>,
  primary_index: usize,
}

impl Selection {
  pub fn new(ranges: SmallVec<[Range; 1]>, primary_index: usize) -> Result<Self> {
    if ranges.is_empty() {
      return Err(SelectionError::EmptySelection);
    }
    if primary_index >= ranges.len() {
      return Err(SelectionError::PrimaryIndexOutOfBounds {
        index: primary_index,
        len:   ranges.len(),
      });
    }

    Ok(Self::new_unchecked(ranges, primary_index).normalize())
  }

  pub(crate) fn new_unchecked(
    ranges: SmallVec<[Range; 1]>,
    primary_index: usize,
  ) -> Self {
    Self {
      ranges,
      primary_index,
    }
  }

  pub fn point(pos: usize) -> Self {
    Self::new_unchecked(smallvec![Range::point(pos)], 0)
  }

  pub fn primary(&self) -> Range {
    self.ranges[self.primary_index]
  }

  pub fn primary_mut(&mut self) -> &mut Range {
    &mut self.ranges[self.primary_index]
  }

  pub fn primary_index(&self) -> usize {
    self.primary_index
  }

  pub fn set_primary_index(&mut self, idx: usize) -> Result<()> {
    if idx >= self.ranges.len() {
      return Err(SelectionError::PrimaryIndexOutOfBounds {
        index: idx,
        len:   self.ranges.len(),
      });
    }
    self.primary_index = idx;
    Ok(())
  }

  pub fn ranges(&self) -> &[Range] {
    &self.ranges
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
      Self::new_unchecked(smallvec![self.ranges[self.primary_index]], 0)
    }
  }

  /// Adds a new range to the selection and makes it the primary range.
  pub fn push(mut self, range: Range) -> Self {
    self.ranges.push(range);
    self.primary_index = self.ranges().len() - 1;
    self.normalize()
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
    if idx < self.primary_index || self.primary_index == self.ranges.len() {
      self.primary_index -= 1;
    }

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
    Self::new_unchecked(smallvec![Range::new(anchor, head)], 0)
  }

  /// Normalizes a [Selection]
  ///
  /// Ranges are sorted by [Range::from] with overlapping ranges merged.
  fn normalize(mut self) -> Self {
    if self.ranges.len() < 2 {
      return self;
    }
    let mut primary = self.ranges[self.primary_index];
    self.ranges.sort_unstable_by_key(Range::from);

    self.ranges.dedup_by(|curr_range, prev_range| {
      if prev_range.overlaps(curr_range) {
        let new_range = curr_range.merge(*prev_range);
        if prev_range == &primary || curr_range == &primary {
          primary = new_range;
        }
        *prev_range = new_range;
        true
      } else {
        false
      }
    });

    self.primary_index = self
      .ranges
      .iter()
      .position(|&range| range == primary)
      .unwrap();

    self
  }

  pub fn merge_ranges(self) -> Self {
    let first = self.ranges.first().unwrap();
    let last = self.ranges.last().unwrap();
    Selection::new_unchecked(smallvec![first.merge(*last)], 0)
  }

  /// Merges all ranges that are consecutive.
  pub fn merge_consecutive_ranges(mut self) -> Self {
    self = self.normalize();
    let mut primary = self.ranges[self.primary_index];

    self.ranges.dedup_by(|curr_range, prev_range| {
      if prev_range.to() == curr_range.from() {
        let new_range = curr_range.merge(*prev_range);
        if prev_range == &primary || curr_range == &primary {
          primary = new_range;
        }
        *prev_range = new_range;
        true
      } else {
        false
      }
    });

    self.primary_index = self
      .ranges
      .iter()
      .position(|&range| range == primary)
      .unwrap();

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

  pub fn transform_iter<F, I>(mut self, f: F) -> Result<Self>
  where
    F: FnMut(Range) -> I,
    I: Iterator<Item = Range>,
  {
    self.ranges = self.ranges.into_iter().flat_map(f).collect();
    if self.ranges.is_empty() {
      return Err(SelectionError::NoRanges);
    }
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
    Self::new_unchecked(smallvec![range], 0)
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
  let primary_idx = selection.primary_index();
  let mut result = SmallVec::with_capacity(selection.ranges.len());
  let mut new_primary_idx = None;
  let mut last_before_primary = None;

  for (idx, range) in selection.iter().enumerate() {
    if regex.is_match(text.regex_input_at(range.from()..range.to())) ^ remove {
      let new_idx = result.len();
      result.push(*range);
      if idx == primary_idx {
        new_primary_idx = Some(new_idx);
      }
      if idx < primary_idx {
        last_before_primary = Some(new_idx);
      }
    }
  }

  if result.is_empty() {
    return Ok(None);
  }
  let primary_idx = new_primary_idx.or(last_before_primary).unwrap_or(0);
  Ok(Some(Selection::new(result, primary_idx)?))
}

// TODO: support to split on capture #N instead of whole match
pub fn select_on_matches(
  text: RopeSlice,
  selection: &Selection,
  regex: &rope::Regex,
) -> Result<Option<Selection>> {
  let primary_idx = selection.primary_index();
  let mut result = SmallVec::with_capacity(selection.ranges.len());
  let mut primary_match_idx = None;
  let mut first_primary_match_idx = None;
  let mut last_before_primary = None;

  for (idx, sel) in selection.iter().enumerate() {
    let head = sel.head;
    for mat in regex.find_iter(text.regex_input_at(sel.from()..sel.to())) {
      let start = text.byte_to_char(mat.start());
      let end = text.byte_to_char(mat.end());

      let range = Range::new(start, end).with_direction(sel.direction());
      // Make sure the match is not right outside of the selection.
      // These invalid matches can come from using RegEx anchors like `^`, `$`
      if range == Range::point(sel.to()) {
        continue;
      }

      let new_idx = result.len();
      result.push(range);
      if idx < primary_idx {
        last_before_primary = Some(new_idx);
      } else if idx == primary_idx {
        if first_primary_match_idx.is_none() {
          first_primary_match_idx = Some(new_idx);
        }
        if start <= head && head <= end {
          primary_match_idx = Some(new_idx);
        }
      }
    }
  }

  if result.is_empty() {
    return Ok(None);
  }

  let primary_idx = primary_match_idx
    .or(first_primary_match_idx)
    .or(last_before_primary)
    .unwrap_or(0);
  Ok(Some(Selection::new(result, primary_idx)?))
}

pub fn split_on_newline(text: RopeSlice, selection: &Selection) -> Result<Selection> {
  let mut result = SmallVec::with_capacity(selection.ranges.len());
  let primary_idx = selection.primary_index();
  let mut new_primary_idx = None;

  let range_contains_inclusive = |range: &Range, pos: usize| {
    range.from() <= pos && pos <= range.to()
  };

  for (idx, sel) in selection.iter().enumerate() {
    let is_primary = idx == primary_idx;
    let head = sel.head;

    // Special case: zero-width selection.
    if sel.from() == sel.to() {
      let new_idx = result.len();
      result.push(*sel);
      if is_primary {
        new_primary_idx = Some(new_idx);
      }
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
      let range = Range::new(start, line_end - line_ending.len_chars())
        .with_direction(sel.direction());
      let new_idx = result.len();
      result.push(range);
      if is_primary && new_primary_idx.is_none() && range_contains_inclusive(&range, head) {
        new_primary_idx = Some(new_idx);
      }
      start = line_end;
    }

    if start < sel_end {
      let range = Range::new(start, sel_end).with_direction(sel.direction());
      let new_idx = result.len();
      result.push(range);
      if is_primary && new_primary_idx.is_none() && range_contains_inclusive(&range, head) {
        new_primary_idx = Some(new_idx);
      }
    }
  }

  let primary_idx = new_primary_idx.unwrap_or(0);
  Selection::new(result, primary_idx)
}

pub fn split_on_matches(
  text: RopeSlice,
  selection: &Selection,
  regex: &the_stdx::rope::Regex,
) -> Result<Selection> {
  let mut result = SmallVec::with_capacity(selection.ranges.len());
  let primary_idx = selection.primary_index();
  let mut new_primary_idx = None;

  let range_contains_inclusive = |range: &Range, pos: usize| {
    range.from() <= pos && pos <= range.to()
  };

  for (idx, sel) in selection.iter().enumerate() {
    let is_primary = idx == primary_idx;
    let head = sel.head;

    // Special case: zero-width selection.
    if sel.from() == sel.to() {
      let new_idx = result.len();
      result.push(*sel);
      if is_primary {
        new_primary_idx = Some(new_idx);
      }
      continue;
    }

    let sel_start = sel.from();
    let sel_end = sel.to();
    let mut start = sel_start;

    for mat in regex.find_iter(text.regex_input_at(sel_start..sel_end)) {
      let end = text.byte_to_char(mat.start());
      let range = Range::new(start, end).with_direction(sel.direction());
      let new_idx = result.len();
      result.push(range);
      if is_primary && new_primary_idx.is_none() && range_contains_inclusive(&range, head) {
        new_primary_idx = Some(new_idx);
      }
      start = text.byte_to_char(mat.end());
    }

    if start < sel_end {
      let range = Range::new(start, sel_end).with_direction(sel.direction());
      let new_idx = result.len();
      result.push(range);
      if is_primary && new_primary_idx.is_none() && range_contains_inclusive(&range, head) {
        new_primary_idx = Some(new_idx);
      }
    }
  }

  let primary_idx = new_primary_idx.unwrap_or(0);
  Selection::new(result, primary_idx)
}

#[cfg(test)]
mod test {
  use ropey::Rope;

  use super::*;

  #[test]
  fn test_new_empty() {
    let err = Selection::new(smallvec![], 0).unwrap_err();
    assert_eq!(err, SelectionError::EmptySelection);
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
      0,
    )
    .unwrap();

    let res = sel
      .ranges
      .into_iter()
      .map(|range| format!("{}/{}", range.anchor, range.head))
      .collect::<Vec<String>>()
      .join(",");

    assert_eq!(res, "0/6,6/7,7/8,9/13,13/14");

    // it correctly calculates a new primary index
    let sel = Selection::new(
      smallvec![Range::new(0, 2), Range::new(1, 5), Range::new(4, 7)],
      2,
    )
    .unwrap();

    let res = sel
      .ranges
      .into_iter()
      .map(|range| format!("{}/{}", range.anchor, range.head))
      .collect::<Vec<String>>()
      .join(",");

    assert_eq!(res, "0/7");
    assert_eq!(sel.primary_index, 0);
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
      0,
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
    assert_eq!(
      select_on_matches(s, &selection, &rope::Regex::new(r"[A-Z][a-z]*").unwrap()).unwrap(),
      Some(
        Selection::new(smallvec![Range::new(0, 6), Range::new(19, 26)], 0).unwrap()
      )
    );

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
    assert_eq!(
      select_on_matches(s, &Selection::single(0, 4), &start_of_line).unwrap(),
      Some(Selection::single(0, 0))
    );
    assert_eq!(
      select_on_matches(s, &Selection::single(0, 4), &end_of_line).unwrap(),
      None
    );
    // line with ending
    assert_eq!(
      select_on_matches(s, &Selection::single(0, 5), &start_of_line).unwrap(),
      Some(Selection::single(0, 0))
    );
    assert_eq!(
      select_on_matches(s, &Selection::single(0, 5), &end_of_line).unwrap(),
      Some(Selection::single(4, 4))
    );
    // line with start of next line
    assert_eq!(
      select_on_matches(s, &Selection::single(0, 6), &start_of_line).unwrap(),
      Some(Selection::new(smallvec![Range::point(0), Range::point(5)], 0).unwrap())
    );
    assert_eq!(
      select_on_matches(s, &Selection::single(0, 6), &end_of_line).unwrap(),
      Some(Selection::single(4, 4))
    );

    // multiple lines
    assert_eq!(
      select_on_matches(
        s,
        &Selection::single(0, s.len_chars()),
        &rope::RegexBuilder::new()
          .syntax(rope::Config::new().multi_line(true))
          .build(r"^[a-z ]*$")
          .unwrap()
      )
      .unwrap(),
      Some(
        Selection::new(
          smallvec![Range::point(12), Range::new(13, 30), Range::new(31, 36)],
          2
        )
        .unwrap()
      )
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

    let selection =
      Selection::new(smallvec![Range::new(0, 9), Range::new(11, 20)], 0).unwrap();

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
      4,
    )
    .unwrap();

    let result = selection.merge_consecutive_ranges();

    assert_eq!(result.ranges(), &[
      Range::new(0, 10),
      Range::new(15, 20),
      Range::new(25, 30)
    ]);
    assert_eq!(result.primary_index, 2);

    let selection = Selection::new(smallvec![Range::new(0, 1)], 0).unwrap();
    let result = selection.merge_consecutive_ranges();

    assert_eq!(result.ranges(), &[Range::new(0, 1)]);
    assert_eq!(result.primary_index, 0);

    let selection = Selection::new(
      smallvec![
        Range::new(0, 1),
        Range::new(1, 5),
        Range::new(5, 8),
        Range::new(8, 10),
        Range::new(10, 15),
        Range::new(18, 25)
      ],
      3,
    )
    .unwrap();

    let result = selection.merge_consecutive_ranges();

    assert_eq!(result.ranges(), &[Range::new(0, 15), Range::new(18, 25)]);
    assert_eq!(result.primary_index, 0);
  }

  #[test]
  fn test_selection_contains() {
    fn contains(a: Vec<(usize, usize)>, b: Vec<(usize, usize)>) -> bool {
      let sel_a = Selection::new(a.iter().map(|a| Range::new(a.0, a.1)).collect(), 0).unwrap();
      let sel_b = Selection::new(b.iter().map(|b| Range::new(b.0, b.1)).collect(), 0).unwrap();
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
}
