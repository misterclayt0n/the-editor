//! Provides [Range] type expanding on [RangeBounds].

use std::ops::{
  self,
  RangeBounds,
};

/// A range of `char`s within the text.
#[derive(Debug, Clone, Copy, PartialOrd, Ord, PartialEq, Eq)]
pub struct Range<T = usize> {
  pub start: T,
  pub end:   T,
}

impl<T: PartialOrd> Range<T> {
  pub fn contains(&self, other: Self) -> bool {
    self.start <= other.start && other.end <= self.end
  }
  pub fn is_empty(&self) -> bool {
    self.end <= self.start
  }
}

impl<T> RangeBounds<T> for Range<T> {
  fn start_bound(&self) -> ops::Bound<&T> {
    ops::Bound::Included(&self.start)
  }

  fn end_bound(&self) -> ops::Bound<&T> {
    ops::Bound::Excluded(&self.end)
  }
}

/// Returns true if all ranges yielded by `sub_set` are contained by
/// `super_set`. This is essentially an optimized implementation of
/// `sub_set.all(|rb| super_set.any(|ra| ra.contains(rb)))` that runs in O(m+n)
/// instead of O(mn) (and in many cases faster).
///
/// Both iterators must uphold a the following invariants:
/// * ranges must not overlap (but they can be adjacent)
/// * ranges must be sorted
pub fn is_subset<const ALLOW_EMPTY: bool>(
  mut super_set: impl Iterator<Item = Range>,
  sub_set: impl Iterator<Item = Range>,
) -> bool {
  let mut super_range = super_set.next();
  for sub_range in sub_set {
    loop {
      match super_range {
        None => return false,
        Some(s) if s.end <= sub_range.start && !(ALLOW_EMPTY && s.start == sub_range.start) => {
          super_range = super_set.next();
        },
        Some(s) if s.contains(sub_range) => break,
        Some(_) => return false,
      }
    }
  }

  true
}

/// Similar to is_subset but requires each element of `super_set` to be matched
pub fn is_exact_subset(
  mut super_set: impl Iterator<Item = Range>,
  sub_set: impl Iterator<Item = Range>,
) -> bool {
  let mut sup = super_set.next();
  let mut matched = false;

  for sub in sub_set {
    loop {
      match sup {
        None => return false,
        Some(s) if s.end <= sub.start => {
          if !matched {
            return false;
          }

          sup = super_set.next();
          matched = false;
        },
        Some(s) if s.contains(sub) => {
          matched = true;
          break;
        },
        Some(_) => return false,
      }
    }
  }

  matched && super_set.next().is_none()
}

#[cfg(test)]
mod tests {
  use super::*;

  fn r(start: usize, end: usize) -> Range {
    Range { start, end }
  }

  #[test]
  fn range_contains_another_range_when_bounds_enclose_it() {
    assert!(r(0, 10).contains(r(0, 10)));
    assert!(r(0, 10).contains(r(2, 8)));
    assert!(!r(2, 10).contains(r(0, 5)));
    assert!(!r(0, 8).contains(r(5, 10)));
  }

  #[test]
  fn range_is_empty_when_end_is_not_after_start() {
    assert!(r(5, 5).is_empty());
    assert!(r(7, 3).is_empty());
    assert!(!r(3, 4).is_empty());
  }

  #[test]
  fn is_subset_accepts_sub_ranges_inside_sorted_super_ranges() {
    let super_set = [r(0, 10), r(20, 30)];
    let sub_set = [r(2, 5), r(22, 28)];

    assert!(is_subset::<false>(
      super_set.into_iter(),
      sub_set.into_iter()
    ));
  }

  #[test]
  fn is_subset_rejects_sub_range_that_crosses_a_gap() {
    let super_set = [r(0, 10), r(20, 30)];
    let sub_set = [r(8, 22)];

    assert!(!is_subset::<false>(
      super_set.into_iter(),
      sub_set.into_iter()
    ));
  }

  #[test]
  fn is_subset_treats_empty_ranges_according_to_flag() {
    let super_set = [r(0, 3), r(5, 10)];
    let sub_set = [r(5, 5)];

    assert!(is_subset::<true>(
      super_set.into_iter(),
      sub_set.into_iter()
    ));
    assert!(!is_subset::<false>(
      super_set.into_iter(),
      sub_set.into_iter()
    ));
  }

  #[test]
  fn is_exact_subset_requires_each_super_range_to_match() {
    let super_set = [r(0, 10), r(20, 30)];

    assert!(is_exact_subset(
      super_set.into_iter(),
      [r(2, 5), r(22, 28)].into_iter(),
    ));

    assert!(!is_exact_subset(
      super_set.into_iter(),
      [r(2, 5)].into_iter(),
    ));
  }

  #[test]
  fn is_exact_subset_accepts_multiple_sub_ranges_inside_one_super_range() {
    let super_set = [r(0, 20)];
    let sub_set = [r(1, 5), r(10, 15), r(16, 19)];

    assert!(is_exact_subset(super_set.into_iter(), sub_set.into_iter()));
  }
}
