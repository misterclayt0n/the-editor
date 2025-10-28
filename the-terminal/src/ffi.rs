//! Low-level FFI bindings to libghostty-vt C API.
//!
//! These are the raw C function declarations from libghostty-vt.
//! They are unsafe and should be wrapped by safe types in the terminal module.

use std::ffi::{
  c_uint,
  c_void,
};

/// Opaque terminal structure from libghostty
#[repr(C)]
pub struct GhosttyTerminal {
  _private: [u8; 0],
}

/// Represents a single cell in the terminal grid
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct GhosttyCell {
  pub codepoint:    u32,
  pub cluster:      u32,
  pub style:        u64,
  pub hyperlink_id: u32,
}

/// Color with presence flag
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct GhosttyColor {
  pub r:      u8,
  pub g:      u8,
  pub b:      u8,
  pub a:      u8,
  pub is_set: bool,
}

impl Default for GhosttyColor {
  fn default() -> Self {
    Self {
      r:      0,
      g:      0,
      b:      0,
      a:      0,
      is_set: false,
    }
  }
}

/// Extended cell information including resolved colors and attributes
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct GhosttyCellExt {
  pub codepoint:    u32,
  pub cluster:      u32,
  pub style:        u64,
  pub hyperlink_id: u32,
  pub fg:           GhosttyColor,
  pub bg:           GhosttyColor,
  pub underline:    GhosttyColor,
  pub flags:        u32,
  pub width:        u8,
}

impl Default for GhosttyCellExt {
  fn default() -> Self {
    Self {
      codepoint:    0,
      cluster:      0,
      style:        0,
      hyperlink_id: 0,
      fg:           GhosttyColor::default(),
      bg:           GhosttyColor::default(),
      underline:    GhosttyColor::default(),
      flags:        0,
      width:        0,
    }
  }
}

/// Terminal size options
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct GhosttyTerminalOptions {
  pub cols: c_uint,
  pub rows: c_uint,
}

/// A point in the terminal grid (row, col)
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct GhosttyPoint {
  pub row: i32,
  pub col: i32,
}

/// Callback function type for terminal responses
///
/// This callback is invoked when the terminal needs to send data back to the
/// PTY, such as cursor position reports or other terminal queries.
pub type ResponseCallback = extern "C" fn(ctx: *mut c_void, data: *const u8, len: usize);

unsafe extern "C" {
  /// Initialize a new terminal with the given options.
  ///
  /// Returns a pointer to a new Terminal instance that must be freed with
  /// ghostty_terminal_free.
  pub fn ghostty_terminal_new(opts: *const GhosttyTerminalOptions) -> *mut GhosttyTerminal;

  /// Free a terminal instance.
  pub fn ghostty_terminal_free(term: *mut GhosttyTerminal);

  /// Print a UTF-8 string to the terminal WITHOUT parsing escape sequences.
  ///
  /// DEPRECATED: Use ghostty_terminal_write for PTY output.
  /// This function only renders literal text.
  pub fn ghostty_terminal_print_string(
    term: *mut GhosttyTerminal,
    s: *const u8,
    len: usize,
  ) -> bool;

  /// Write raw bytes to the terminal, parsing VT100/ANSI escape sequences.
  ///
  /// This is the correct function to use for PTY output. It will parse
  /// escape sequences and update the terminal state accordingly.
  pub fn ghostty_terminal_write(term: *mut GhosttyTerminal, data: *const u8, len: usize) -> bool;

  /// Get the width (columns) of the terminal.
  pub fn ghostty_terminal_cols(term: *const GhosttyTerminal) -> c_uint;

  /// Get the height (rows) of the terminal.
  pub fn ghostty_terminal_rows(term: *const GhosttyTerminal) -> c_uint;

  /// Get a cell from the terminal grid at the given position.
  ///
  /// Returns a GhosttyCell. If the position is out of bounds, returns a cell
  /// with default values.
  pub fn ghostty_terminal_get_cell(term: *const GhosttyTerminal, pt: GhosttyPoint) -> GhosttyCell;

  /// Retrieve extended cell information (colors, attributes).
  pub fn ghostty_terminal_get_cell_ext(
    term: *const GhosttyTerminal,
    pt: GhosttyPoint,
    out: *mut GhosttyCellExt,
  ) -> bool;

  /// Copy an entire row worth of extended cell data into the provided buffer.
  /// Returns the number of valid entries written.
  pub fn ghostty_terminal_copy_row_cells_ext(
    term: *const GhosttyTerminal,
    row: c_uint,
    out_cells: *mut GhosttyCellExt,
    max_len: usize,
  ) -> usize;

  /// Get the current cursor position.
  pub fn ghostty_terminal_cursor_pos(term: *const GhosttyTerminal) -> GhosttyPoint;

  /// Resize the terminal to new dimensions.
  ///
  /// Returns true on success, false on failure.
  pub fn ghostty_terminal_resize(term: *mut GhosttyTerminal, cols: c_uint, rows: c_uint) -> bool;

  /// Set the callback for terminal responses.
  ///
  /// This enables bidirectional communication between the terminal and PTY.
  /// The callback will be invoked when the terminal needs to send responses
  /// (e.g., cursor position reports, color queries).
  pub fn ghostty_terminal_set_callback(
    term: *mut GhosttyTerminal,
    callback: ResponseCallback,
    ctx: *mut c_void,
  );

  /// Update cached cell pixel dimensions for query responses.
  pub fn ghostty_terminal_set_cell_pixel_size(term: *mut GhosttyTerminal, width: u16, height: u16);

  /// Update cached background color for OSC queries.
  pub fn ghostty_terminal_set_background_color(term: *mut GhosttyTerminal, r: u8, g: u8, b: u8);

  /// Check if terminal needs a full rebuild.
  ///
  /// Returns true if terminal-level or screen-level dirty flags are set,
  /// indicating operations like eraseDisplay, resize, or mode changes that
  /// require a full screen rebuild (not just dirty rows).
  ///
  /// This must be checked BEFORE get_dirty_rows() to properly handle full
  /// screen operations like those triggered by nushell's prompt redraws.
  pub fn ghostty_terminal_needs_full_rebuild(term: *const GhosttyTerminal) -> bool;

  /// Get dirty rows in the terminal.
  ///
  /// Returns a pointer to an array of u32 row indices that need re-rendering.
  /// The caller must free the array using ghostty_terminal_free_dirty_rows().
  ///
  /// # Safety
  /// The returned pointer must be freed with ghostty_terminal_free_dirty_rows()
  /// with the same count value.
  pub fn ghostty_terminal_get_dirty_rows(
    term: *const GhosttyTerminal,
    out_count: *mut usize,
  ) -> *mut u32;

  /// Free the dirty rows array returned by ghostty_terminal_get_dirty_rows().
  pub fn ghostty_terminal_free_dirty_rows(rows: *mut u32, count: usize);

  /// Clear all dirty bits in the terminal.
  ///
  /// This should be called after rendering all dirty rows to reset the dirty
  /// state.
  pub fn ghostty_terminal_clear_dirty(term: *mut GhosttyTerminal);
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_ffi_basic() {
    unsafe {
      let opts = GhosttyTerminalOptions { cols: 80, rows: 24 };
      let term = ghostty_terminal_new(&opts);
      assert!(!term.is_null());

      let cols = ghostty_terminal_cols(term);
      assert_eq!(cols, 80);

      let rows = ghostty_terminal_rows(term);
      assert_eq!(rows, 24);

      ghostty_terminal_free(term);
    }
  }
}
