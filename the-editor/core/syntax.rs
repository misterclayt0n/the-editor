//! TODO mostly

use core::fmt;
use std::{
  num::NonZeroU32,
  ops,
};

/// Indicates which highlight should be applied to a region of the source code.
///
/// This type is represented as a non-max u32 - an u32 which cannot be
/// `u32::MAX`. This is checked at runtime with assertions in `Highlight::new`.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Highlight(NonZeroU32);

impl Highlight {
  pub const MAX: u32 = u32::MAX - 1;

  pub const fn new(inner: u32) -> Self {
    assert!(inner != u32::MAX);
    Self(unsafe { NonZeroU32::new_unchecked(inner ^ u32::MAX) })
  }

  pub const fn get(&self) -> u32 {
    self.0.get() ^ u32::MAX
  }

  pub const fn idx(&self) -> usize {
    self.get() as usize
  }
}

impl fmt::Debug for Highlight {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_tuple("Highlight").field(&self.get()).finish()
  }
}

/// A set of "overlay" highlights and ranges they apply to.
///
/// As overlays, the styles for the given `Highlight`s are merged on top of the
/// syntax highlights.
#[derive(Debug)]
pub enum OverlayHighlights {
  /// All highlights use a single `Highlight`.
  ///
  /// Note that, currently, all ranges are assumed to be non-overlapping. This
  /// could change in the future though.
  Homogeneous {
    highlight: Highlight,
    ranges:    Vec<ops::Range<usize>>,
  },
  /// A collection of different highlights for given ranges.
  ///
  /// Note that the ranges **must be non-overlapping**.
  Heterogenous {
    highlights: Vec<(Highlight, ops::Range<usize>)>,
  },
}

impl OverlayHighlights {
  pub fn single(highlight: Highlight, range: ops::Range<usize>) -> Self {
    Self::Homogeneous {
      highlight,
      ranges: vec![range],
    }
  }

  fn is_empty(&self) -> bool {
    match self {
      Self::Homogeneous { ranges, .. } => ranges.is_empty(),
      Self::Heterogenous { highlights } => highlights.is_empty(),
    }
  }
}
