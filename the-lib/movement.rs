//! Movement direction for cursor and selection operations.
//!
//! This module provides the [`Direction`] enum used throughout the library
//! to indicate the direction of movement or selection extension.
//!
//! # Usage
//!
//! ```ignore
//! use the_lib::movement::Direction;
//! use the_lib::selection::Range;
//!
//! let range = Range::new(5, 10);
//!
//! // Check selection direction
//! assert_eq!(range.direction(), Direction::Forward);
//!
//! // Create a range with specific direction
//! let backward = range.with_direction(Direction::Backward);
//! assert_eq!(backward.anchor, 10);
//! assert_eq!(backward.head, 5);
//! ```

/// The direction of cursor movement or selection extension.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Direction {
  /// Moving toward the end of the document (increasing positions).
  Forward,
  /// Moving toward the start of the document (decreasing positions).
  Backward,
}
