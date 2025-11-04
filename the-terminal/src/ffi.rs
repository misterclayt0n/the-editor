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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
  pub selected:     bool,
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
      selected:     false,
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

/// Opaque pin handle for zero-copy row iteration
#[repr(C)]
pub struct GhosttyPin {
  _private: [u8; 0],
}

/// Opaque row iterator type.
#[repr(C)]
pub struct GhosttyRowIterator {
  _private: [u8; 0],
}

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
  /// Update cached foreground color for OSC queries.
  pub fn ghostty_terminal_set_foreground_color(term: *mut GhosttyTerminal, r: u8, g: u8, b: u8);
  /// Update the terminal color palette entry.
  pub fn ghostty_terminal_set_palette_color(
    term: *mut GhosttyTerminal,
    index: u16,
    r: u8,
    g: u8,
    b: u8,
  ) -> bool;

  /// Scroll the terminal viewport by a delta in rows (negative scrolls up).
  pub fn ghostty_terminal_scroll_viewport_delta(
    term: *mut GhosttyTerminal,
    delta_rows: i32,
  ) -> bool;

  /// Scroll the terminal viewport to the top of scrollback.
  pub fn ghostty_terminal_scroll_viewport_top(term: *mut GhosttyTerminal) -> bool;

  /// Scroll the terminal viewport to the bottom (active area).
  pub fn ghostty_terminal_scroll_viewport_bottom(term: *mut GhosttyTerminal) -> bool;

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

  // ===== PIN-BASED ZERO-COPY ITERATION =====

  /// Pin a specific row in the terminal viewport for zero-copy access.
  ///
  /// Returns an opaque pin handle that provides direct access to cell data.
  /// The caller MUST call ghostty_terminal_pin_free() when done.
  ///
  /// # Arguments
  /// * `term` - Terminal instance
  /// * `row` - Row index (0-based, viewport coordinates)
  ///
  /// # Returns
  /// Opaque pin handle, or null if row is out of bounds
  ///
  /// # Safety
  /// The pin must be freed with ghostty_terminal_pin_free().
  /// The pin is only valid while the terminal is alive.
  pub fn ghostty_terminal_pin_row(term: *const GhosttyTerminal, row: c_uint) -> *mut GhosttyPin;

  /// Get direct pointer to cell array from a pinned row.
  ///
  /// Returns a pointer to the internal cell array. The pointer is valid
  /// until ghostty_terminal_pin_free() is called.
  ///
  /// # Arguments
  /// * `term` - Terminal instance
  /// * `pin` - Pin handle from ghostty_terminal_pin_row()
  /// * `out_count` - Output parameter for number of cells
  ///
  /// # Returns
  /// Pointer to internal cell array (ghostty_vt.page.Cell), or null if invalid
  ///
  /// # Safety
  /// The returned pointer is only valid until ghostty_terminal_pin_free().
  /// Do NOT dereference the cells directly - use
  /// ghostty_terminal_pin_populate_cell_ext().
  pub fn ghostty_terminal_pin_cells(
    term: *const GhosttyTerminal,
    pin: *mut GhosttyPin,
    out_count: *mut usize,
  ) -> *const c_void;

  /// Populate a CCellExt from a pin's internal cell.
  ///
  /// This converts a ghostty internal cell to the FFI-safe CCellExt struct,
  /// resolving colors and attributes.
  ///
  /// # Arguments
  /// * `term` - Terminal instance
  /// * `pin` - Pin handle from ghostty_terminal_pin_row()
  /// * `cell_index` - Index into the cell array (0 to count-1)
  /// * `out_cell` - Output CCellExt struct
  ///
  /// # Returns
  /// true on success, false if indices are invalid
  pub fn ghostty_terminal_pin_populate_cell_ext(
    term: *const GhosttyTerminal,
    pin: *mut GhosttyPin,
    cell_index: usize,
    out_cell: *mut GhosttyCellExt,
  ) -> bool;

  /// Free a pin handle.
  ///
  /// MUST be called for every pin returned by ghostty_terminal_pin_row().
  ///
  /// # Safety
  /// The pin must not be used after calling this function.
  pub fn ghostty_terminal_pin_free(pin: *mut GhosttyPin);

  // ==========================================================================
  // ROW ITERATOR (GHOSTTY PATTERN)
  // ==========================================================================

  /// Create a row iterator for the terminal viewport.
  ///
  /// Returns an iterator that yields rows from top to bottom. Each row
  /// includes its index and dirty flag, enabling efficient incremental
  /// rendering.
  ///
  /// The iterator MUST be freed with ghostty_terminal_row_iterator_free().
  ///
  /// Returns null if terminal is invalid or iterator creation fails.
  pub fn ghostty_terminal_row_iterator_new(term: *const GhosttyTerminal)
  -> *mut GhosttyRowIterator;

  /// Get the next row from the iterator.
  ///
  /// Advances the iterator and returns the next row's index and dirty status.
  /// Returns false when iteration is complete.
  ///
  /// # Arguments
  /// * `term` - Terminal handle (for validation)
  /// * `iter` - Iterator handle from ghostty_terminal_row_iterator_new()
  /// * `out_row_index` - Receives the row index (0-based from top)
  /// * `out_is_dirty` - Receives true if the row needs re-rendering
  ///
  /// # Returns
  /// true if a row was yielded, false if iteration is complete
  pub fn ghostty_terminal_row_iterator_next(
    term: *const GhosttyTerminal,
    iter: *mut GhosttyRowIterator,
    out_row_index: *mut u32,
    out_is_dirty: *mut bool,
  ) -> bool;

  /// Free a row iterator.
  ///
  /// MUST be called for every iterator returned by
  /// ghostty_terminal_row_iterator_new().
  ///
  /// # Safety
  /// The iterator must not be used after calling this function.
  pub fn ghostty_terminal_row_iterator_free(iter: *mut GhosttyRowIterator);

  // ==========================================================================
  // TERMINAL MODES
  // ==========================================================================

  /// Query a terminal mode state.
  ///
  /// Checks if a specific terminal mode is enabled, such as cursor visibility,
  /// application cursor keys, bracketed paste, etc.
  ///
  /// # Arguments
  /// * `term` - Terminal handle
  /// * `mode_value` - Numeric mode identifier (e.g., 25 for cursor_visible)
  /// * `ansi` - If true, query ANSI mode space; if false, query DEC private
  ///   mode
  ///
  /// # Returns
  /// true if mode is enabled, false if disabled or mode doesn't exist
  ///
  /// # Common Modes
  /// * DEC mode 25 (ansi=false): cursor_visible (DECTCEM)
  /// * DEC mode 1 (ansi=false): application_cursor_keys (DECCKM)
  /// * ANSI mode 2004 (ansi=true): bracketed_paste
  pub fn ghostty_terminal_get_mode(
    term: *const GhosttyTerminal,
    mode_value: u16,
    ansi: bool,
  ) -> bool;

  /// Get the terminal's default background color.
  ///
  /// Returns the background color used for cells without explicit background
  /// colors. Applications can change this via OSC 11 sequences.
  ///
  /// # Returns
  /// RGB color as a GhosttyColor struct
  pub fn ghostty_terminal_get_default_background(term: *const GhosttyTerminal) -> GhosttyColor;

  /// Check if the viewport is at the bottom of the scrollback.
  ///
  /// This is critical for cursor rendering - ghostty only renders the cursor
  /// when the viewport is at the bottom. This prevents rendering the cursor
  /// when scrolled back in history.
  ///
  /// # Returns
  /// true if viewport is at bottom, false otherwise
  pub fn ghostty_terminal_is_viewport_at_bottom(term: *const GhosttyTerminal) -> bool;
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
