//! Test utilities for terminal emulation testing.
//!
//! This module provides helpers for creating test terminals without spawning
//! real PTY processes, enabling deterministic unit testing.

use alacritty_terminal::{
  event::VoidListener,
  grid::Dimensions,
  index::{
    Column,
    Line,
  },
  term::{
    Config as TermConfig,
    Term,
    test::TermSize,
  },
  vte::ansi::{
    Processor,
    StdSyncHandler,
  },
};

use crate::renderer::ColorScheme;

/// Create a test terminal with given dimensions.
///
/// Uses `VoidListener` to avoid real event handling, making tests
/// deterministic and fast.
pub fn test_term(cols: usize, rows: usize) -> Term<VoidListener> {
  let size = TermSize::new(cols, rows);
  Term::new(TermConfig::default(), &size, VoidListener)
}

/// Feed raw bytes to the terminal as if they were output from a program.
///
/// This uses alacritty's VTE processor to handle escape sequences.
pub fn feed_term(term: &mut Term<VoidListener>, data: &[u8]) {
  let mut processor = Processor::<StdSyncHandler>::new();
  processor.advance(term, data);
}

/// Feed a string to the terminal.
///
/// Convenience wrapper around `feed_term` for string data.
pub fn feed_str(term: &mut Term<VoidListener>, s: &str) {
  feed_term(term, s.as_bytes());
}

/// Get the character at a specific position in the terminal grid.
///
/// Position is 0-indexed (col, row).
pub fn char_at(term: &Term<VoidListener>, col: usize, row: usize) -> char {
  term.grid()[Line(row as i32)][Column(col)].c
}

/// Get the string content of a row, trimmed of trailing spaces.
///
/// Row is 0-indexed.
pub fn row_content(term: &Term<VoidListener>, row: usize) -> String {
  let grid = term.grid();
  let cols = grid.columns();
  let mut s = String::with_capacity(cols);

  for col in 0..cols {
    s.push(grid[Line(row as i32)][Column(col)].c);
  }

  s.trim_end().to_string()
}

/// Get the cursor position as (column, line).
///
/// Both values are 0-indexed.
pub fn cursor_pos(term: &Term<VoidListener>) -> (usize, usize) {
  let cursor = term.grid().cursor.point;
  (cursor.column.0, cursor.line.0 as usize)
}

/// Get a default color scheme for testing.
pub fn test_colors() -> ColorScheme {
  ColorScheme::default()
}

/// Get the cell flags at a specific position.
pub fn cell_flags(
  term: &Term<VoidListener>,
  col: usize,
  row: usize,
) -> alacritty_terminal::term::cell::Flags {
  term.grid()[Line(row as i32)][Column(col)].flags
}
