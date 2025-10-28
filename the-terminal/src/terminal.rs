//! Safe Rust wrapper around libghostty-vt terminal emulation.

use std::{
  ffi::c_void,
  mem::MaybeUninit,
};

use crate::ffi::{
  self,
  GhosttyPoint,
  GhosttyTerminal,
  GhosttyTerminalOptions,
  ResponseCallback,
};

/// A safe wrapper around a ghostty terminal instance.
pub struct Terminal {
  inner: *mut GhosttyTerminal,
}

impl Terminal {
  /// Create a new terminal with the specified dimensions.
  ///
  /// # Arguments
  /// * `cols` - Terminal width in columns
  /// * `rows` - Terminal height in rows
  ///
  /// # Errors
  /// Returns an error if terminal creation fails.
  pub fn new(cols: u16, rows: u16) -> anyhow::Result<Self> {
    let opts = GhosttyTerminalOptions {
      cols: cols as u32,
      rows: rows as u32,
    };

    let inner = unsafe { ffi::ghostty_terminal_new(&opts) };

    if inner.is_null() {
      return Err(anyhow::anyhow!("Failed to create terminal"));
    }

    Ok(Self { inner })
  }

  /// Write raw bytes to the terminal, parsing VT100/ANSI escape sequences.
  ///
  /// This is the correct method to use for PTY output. It will parse
  /// escape sequences (colors, cursor movement, etc.) and update the
  /// terminal state accordingly.
  ///
  /// # Arguments
  /// * `data` - Raw bytes from PTY (may contain escape sequences)
  ///
  /// # Errors
  /// Returns an error if the write fails.
  pub fn write(&mut self, data: &[u8]) -> anyhow::Result<()> {
    let success = unsafe { ffi::ghostty_terminal_write(self.inner, data.as_ptr(), data.len()) };

    if !success {
      return Err(anyhow::anyhow!("Failed to write to terminal"));
    }

    Ok(())
  }

  /// Write a UTF-8 string to the terminal WITHOUT parsing escape sequences.
  ///
  /// DEPRECATED: Use `write()` for PTY output.
  /// This function only renders literal text and should not be used
  /// for PTY output as it will display escape sequences as literal characters.
  ///
  /// # Arguments
  /// * `s` - UTF-8 string to write
  ///
  /// # Errors
  /// Returns an error if the write fails.
  pub fn print_string(&mut self, s: &str) -> anyhow::Result<()> {
    let success = unsafe { ffi::ghostty_terminal_print_string(self.inner, s.as_ptr(), s.len()) };

    if !success {
      return Err(anyhow::anyhow!("Failed to print string to terminal"));
    }

    Ok(())
  }

  /// Get the width of the terminal in columns.
  pub fn cols(&self) -> u16 {
    unsafe { ffi::ghostty_terminal_cols(self.inner) as u16 }
  }

  /// Get the height of the terminal in rows.
  pub fn rows(&self) -> u16 {
    unsafe { ffi::ghostty_terminal_rows(self.inner) as u16 }
  }

  /// Get a cell from the terminal grid.
  ///
  /// # Arguments
  /// * `row` - Row coordinate (0-indexed)
  /// * `col` - Column coordinate (0-indexed)
  pub fn get_cell(&self, row: u16, col: u16) -> Cell {
    let pt = GhosttyPoint {
      row: row as i32,
      col: col as i32,
    };

    let mut cell_ext = MaybeUninit::<ffi::GhosttyCellExt>::uninit();
    unsafe {
      ffi::ghostty_terminal_get_cell_ext(self.inner, pt, cell_ext.as_mut_ptr());
    }
    let cell_ext = unsafe { cell_ext.assume_init() };

    Cell::from(cell_ext)
  }

  /// Get the current cursor position.
  pub fn cursor_pos(&self) -> (u16, u16) {
    let pos = unsafe { ffi::ghostty_terminal_cursor_pos(self.inner) };
    (pos.row as u16, pos.col as u16)
  }

  /// Resize the terminal to new dimensions.
  ///
  /// # Arguments
  /// * `cols` - New width in columns
  /// * `rows` - New height in rows
  ///
  /// # Errors
  /// Returns an error if the resize operation fails.
  pub fn resize(&mut self, cols: u16, rows: u16) -> anyhow::Result<()> {
    let success = unsafe { ffi::ghostty_terminal_resize(self.inner, cols as u32, rows as u32) };

    if !success {
      return Err(anyhow::anyhow!("Failed to resize terminal"));
    }

    Ok(())
  }

  /// Inform the terminal of the cell size in pixels for window queries.
  pub fn set_cell_pixel_size(&mut self, width_px: u16, height_px: u16) {
    unsafe { ffi::ghostty_terminal_set_cell_pixel_size(self.inner, width_px, height_px) };
  }

  /// Set the background color reported for OSC queries.
  pub fn set_background_color(&mut self, r: u8, g: u8, b: u8) {
    unsafe { ffi::ghostty_terminal_set_background_color(self.inner, r, g, b) };
  }

  /// Get a view of the entire terminal grid.
  pub fn grid(&self) -> Grid<'_> {
    Grid {
      terminal: self,
      rows:     self.rows(),
      cols:     self.cols(),
    }
  }

  /// Copy extended cell data for a specific row into the provided buffer.
  ///
  /// Returns the number of valid cells written, capped at `buffer.len()`.
  pub fn copy_row_ext(&self, row: u16, buffer: &mut [ffi::GhosttyCellExt]) -> usize {
    if buffer.is_empty() {
      return 0;
    }

    unsafe {
      ffi::ghostty_terminal_copy_row_cells_ext(
        self.inner,
        row as u32,
        buffer.as_mut_ptr(),
        buffer.len(),
      )
    }
  }

  /// Set a callback for terminal responses (e.g., cursor position reports).
  ///
  /// This enables bidirectional communication: the terminal can send responses
  /// back to the PTY when it receives queries from the shell.
  ///
  /// # Safety
  /// The callback must be thread-safe and must not call back into the Terminal.
  /// The context pointer will be passed to the callback on every invocation.
  pub unsafe fn set_response_callback_raw(&mut self, callback: ResponseCallback, ctx: *mut c_void) {
    unsafe {
      ffi::ghostty_terminal_set_callback(self.inner, callback, ctx);
    }
  }

  /// Check if the terminal needs a full rebuild.
  ///
  /// Returns true if terminal-level or screen-level dirty flags are set,
  /// indicating operations like `eraseDisplay` (clear screen), resize,
  /// or mode changes that require rendering all rows.
  ///
  /// CRITICAL: This must be checked BEFORE `get_dirty_rows()` to properly
  /// handle full-screen operations. When this returns true, you should render
  /// all rows and call `clear_dirty()`, rather than using `get_dirty_rows()`.
  ///
  /// This is the root cause fix for nushell performance issues - nushell sends
  /// frequent `eraseDisplay` sequences which set terminal-level dirty flags,
  /// not row-level dirty bits.
  ///
  /// # Example
  /// ```no_run
  /// # use the_terminal::Terminal;
  /// # let mut term = Terminal::new(80, 24).unwrap();
  /// if term.needs_full_rebuild() {
  ///   // Render all rows
  ///   term.clear_dirty();
  /// } else {
  ///   // Render only dirty rows
  ///   let dirty_rows = term.get_dirty_rows();
  ///   term.clear_dirty();
  /// }
  /// ```
  pub fn needs_full_rebuild(&self) -> bool {
    unsafe { ffi::ghostty_terminal_needs_full_rebuild(self.inner) }
  }

  /// Get the list of dirty rows that need re-rendering.
  ///
  /// Returns a Vec of row indices that have changed since the last call to
  /// `clear_dirty()`. This allows for efficient incremental rendering by only
  /// updating rows that have actually changed.
  ///
  /// IMPORTANT: Check `needs_full_rebuild()` FIRST. If it returns true,
  /// you should render all rows instead of using this method.
  ///
  /// # Example
  /// ```no_run
  /// # use the_terminal::Terminal;
  /// # let mut term = Terminal::new(80, 24).unwrap();
  /// let dirty_rows = term.get_dirty_rows();
  /// for row in dirty_rows {
  ///   // Re-render only this row
  /// }
  /// term.clear_dirty();
  /// ```
  pub fn get_dirty_rows(&self) -> Vec<u32> {
    let mut count: usize = 0;
    let rows_ptr = unsafe { ffi::ghostty_terminal_get_dirty_rows(self.inner, &mut count) };

    if rows_ptr.is_null() || count == 0 {
      return Vec::new();
    }

    // Copy the array into a Vec
    let slice = unsafe { std::slice::from_raw_parts(rows_ptr, count) };
    let result = slice.to_vec();

    // Free the array allocated by Zig
    unsafe {
      ffi::ghostty_terminal_free_dirty_rows(rows_ptr, count);
    }

    result
  }

  /// Clear all dirty bits in the terminal.
  ///
  /// This should be called after rendering all dirty rows to reset the dirty
  /// state. After calling this, `get_dirty_rows()` will return an empty Vec
  /// until new changes occur.
  pub fn clear_dirty(&mut self) {
    unsafe {
      ffi::ghostty_terminal_clear_dirty(self.inner);
    }
  }
}

impl Drop for Terminal {
  fn drop(&mut self) {
    unsafe {
      if !self.inner.is_null() {
        ffi::ghostty_terminal_free(self.inner);
      }
    }
  }
}

// SAFETY: Terminal is safe to send across threads as long as ghostty
// doesn't maintain thread-local state (which it shouldn't for a library).
unsafe impl Send for Terminal {}
unsafe impl Sync for Terminal {}

/// Simple RGB color stored as 8-bit components.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgb {
  pub r: u8,
  pub g: u8,
  pub b: u8,
}

impl Rgb {
  pub const WHITE: Self = Self {
    r: 255,
    g: 255,
    b: 255,
  };

  pub fn from_color(color: ffi::GhosttyColor) -> Option<Self> {
    if color.is_set {
      Some(Self {
        r: color.r,
        g: color.g,
        b: color.b,
      })
    } else {
      None
    }
  }
}

/// Bitflags describing cell attributes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CellFlags(pub u32);

impl CellFlags {
  pub const BOLD: u32 = 1 << 0;
  pub const ITALIC: u32 = 1 << 1;
  pub const FAINT: u32 = 1 << 2;
  pub const INVERSE: u32 = 1 << 3;
  pub const BLINK: u32 = 1 << 4;
  pub const STRIKETHROUGH: u32 = 1 << 5;
  pub const OVERLINE: u32 = 1 << 6;
  pub const UNDERLINE_ANY: u32 = 1 << 7;

  pub fn contains(self, mask: u32) -> bool {
    (self.0 & mask) != 0
  }
}

/// Represents a single cell in the terminal grid.
#[derive(Debug, Clone, Copy)]
pub struct Cell {
  pub codepoint:    u32,
  pub cluster:      u32,
  pub style:        u64,
  pub hyperlink_id: u32,
  pub fg:           Rgb,
  pub bg:           Option<Rgb>,
  pub underline:    Option<Rgb>,
  pub flags:        CellFlags,
  pub width:        u8,
}

impl Cell {
  /// Get the character represented by this cell's codepoint.
  pub fn character(&self) -> Option<char> {
    char::from_u32(self.codepoint)
  }

  /// Check if this cell is empty (space or null).
  pub fn is_empty(&self) -> bool {
    self.codepoint == 0 || self.codepoint == 32 // space
  }
}

impl From<ffi::GhosttyCellExt> for Cell {
  fn from(ext: ffi::GhosttyCellExt) -> Self {
    let fg = Rgb::from_color(ext.fg).unwrap_or(Rgb::WHITE);
    let bg = Rgb::from_color(ext.bg);
    let underline = Rgb::from_color(ext.underline);

    Self {
      codepoint: ext.codepoint,
      cluster: ext.cluster,
      style: ext.style,
      hyperlink_id: ext.hyperlink_id,
      fg,
      bg,
      underline,
      flags: CellFlags(ext.flags),
      width: ext.width,
    }
  }
}

/// A view into the terminal grid for iteration.
pub struct Grid<'a> {
  terminal: &'a Terminal,
  rows:     u16,
  cols:     u16,
}

impl<'a> Grid<'a> {
  /// Iterate over all cells in the grid.
  pub fn cells(&self) -> impl Iterator<Item = (u16, u16, Cell)> + 'a {
    let rows = self.rows;
    let cols = self.cols;
    let terminal = self.terminal;

    (0..rows).flat_map(move |row| {
      (0..cols).map(move |col| {
        let cell = terminal.get_cell(row, col);
        (row, col, cell)
      })
    })
  }

  /// Iterate over cells in a specific row.
  pub fn row_cells(&self, row: u16) -> impl Iterator<Item = (u16, Cell)> + 'a {
    let terminal = self.terminal;
    (0..self.cols).map(move |col| {
      let cell = terminal.get_cell(row, col);
      (col, cell)
    })
  }

  /// Get the number of rows in the grid.
  pub fn rows(&self) -> u16 {
    self.rows
  }

  /// Get the number of columns in the grid.
  pub fn cols(&self) -> u16 {
    self.cols
  }

  /// Get a cell at the specified position.
  pub fn get(&self, row: u16, col: u16) -> Cell {
    self.terminal.get_cell(row, col)
  }

  /// Get the plain text content of a row as a string.
  pub fn row_string(&self, row: u16) -> String {
    self
      .row_cells(row)
      .filter_map(|(_, cell)| cell.character())
      .collect()
  }

  /// Get the entire grid as plain text.
  pub fn as_string(&self) -> String {
    (0..self.rows)
      .map(|row| self.row_string(row))
      .collect::<Vec<_>>()
      .join("\n")
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_terminal_creation() {
    let term = Terminal::new(80, 24).expect("Failed to create terminal");
    assert_eq!(term.cols(), 80);
    assert_eq!(term.rows(), 24);
  }

  #[test]
  fn test_print_string() {
    let mut term = Terminal::new(80, 24).expect("Failed to create terminal");
    term.print_string("hello").expect("Failed to print string");
  }

  #[test]
  fn test_grid_iteration() {
    let mut term = Terminal::new(80, 24).expect("Failed to create terminal");
    term.print_string("hello").expect("Failed to print string");

    let grid = term.grid();
    let mut count = 0;
    for (_, _, cell) in grid.cells() {
      if !cell.is_empty() {
        count += 1;
      }
    }
    assert!(count > 0, "Should have non-empty cells after printing");
  }

  #[test]
  fn test_row_string() {
    let mut term = Terminal::new(80, 24).expect("Failed to create terminal");
    term.print_string("test").expect("Failed to print string");

    let grid = term.grid();
    let row_0 = grid.row_string(0);
    assert!(
      row_0.contains("test"),
      "Row 0 should contain 'test', got: {}",
      row_0
    );
  }
}
