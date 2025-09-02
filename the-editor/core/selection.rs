use ropey::RopeSlice;

use crate::core::{
  grapheme::{
    next_grapheme_boundary,
    prev_grapheme_boundary,
  },
  movement::Direction,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Range {
  head:           usize,
  anchor:         usize,
  old_visual_pos: Option<(usize, usize)>,
}

impl Range {
  pub fn new(head: usize, anchor: usize) -> Self {
    Self {
      head,
      anchor,
      old_visual_pos: None,
    }
  }

  pub fn point(head: usize) -> Self {
    Self {
      head,
      anchor: head,
      old_visual_pos: None,
    }
  }

  /// Start of the range
  #[inline]
  #[must_use]
  pub fn from(&self) -> usize {
    std::cmp::min(self.head, self.anchor)
  }

  /// End of the range
  #[inline]
  #[must_use]
  pub fn to(&self) -> usize {
    std::cmp::max(self.head, self.anchor)
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
}

/// A selection is one or more ranges.
/// INVARIANT: A selection can never be empty (always contain at least one primary range).
pub struct Selection {
  ranges: Vec<[Range; 1]>,
  primary_index: usize,
}

impl Selection {
  pub fn new(ranges: Vec<[Range; 1]>, primary_index: usize) -> Self {
    assert!(!ranges.is_empty());
    assert!(primary_index < ranges.len());

    // let selection = Self {
      // ranges,
      // primary_index
    // }

    // selection.normalize()

    Self {
      ranges,
      primary_index
    }
  }
}
