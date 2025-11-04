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

  /// Set the foreground color reported for OSC queries.
  pub fn set_foreground_color(&mut self, r: u8, g: u8, b: u8) {
    unsafe { ffi::ghostty_terminal_set_foreground_color(self.inner, r, g, b) };
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

  /// Pin a row for zero-copy iteration.
  ///
  /// Returns a Pin handle that provides direct access to cell data without
  /// copying. The caller must drop the Pin when done (via RAII).
  ///
  /// # Arguments
  /// * `row` - Row index (0-based, viewport coordinates)
  ///
  /// # Returns
  /// Some(Pin) if row is valid, None if out of bounds
  ///
  /// # Example
  /// ```no_run
  /// # use the_terminal::Terminal;
  /// # let term = Terminal::new(80, 24).unwrap();
  /// if let Some(pin) = term.pin_row(0) {
  ///   for col in 0..pin.cell_count() {
  ///     if let Some(cell_ext) = pin.get_cell_ext(&term, col) {
  ///       // Use cell_ext without copying
  ///     }
  ///   }
  /// }
  /// ```
  pub fn pin_row(&self, row: u16) -> Option<Pin> {
    let pin_ptr = unsafe { ffi::ghostty_terminal_pin_row(self.inner, row as u32) };

    if pin_ptr.is_null() {
      return None;
    }

    Some(Pin { ptr: pin_ptr })
  }

  /// Create a row iterator for the terminal viewport.
  ///
  /// Returns an iterator that yields (row_index, is_dirty) tuples for each
  /// row from top to bottom. This is an alternative to `get_dirty_rows()`
  /// that avoids allocating a Vec, but requires holding the terminal lock
  /// during iteration.
  ///
  /// **When to use**:
  /// - Processing rows inline while lock is held (fast operations only)
  /// - Alternative rendering patterns that need row-by-row dirty checks
  ///
  /// **When NOT to use**:
  /// - Snapshot-based rendering (use `get_dirty_rows()` instead)
  /// - Long processing per row (would block PTY thread)
  ///
  /// **Note**: The current snapshot-based rendering in `terminal.rs`
  /// uses `get_dirty_rows()` because it unlocks BEFORE rendering. Using
  /// the iterator would require holding the lock during rendering, which
  /// would block the PTY thread for milliseconds. The Vec allocation
  /// (~1-20µs) is acceptable vs blocking the PTY.
  ///
  /// # Example
  /// ```no_run
  /// # use the_terminal::Terminal;
  /// # fn main() -> anyhow::Result<()> {
  /// let terminal = Terminal::new(80, 24)?;
  /// let iter = terminal.row_iterator()?;
  ///
  /// // IMPORTANT: Lock held during entire iteration!
  /// for (row, is_dirty) in iter {
  ///   if is_dirty {
  ///     // Fast processing only! Don't do I/O or slow operations here.
  ///     if let Some(pin) = terminal.pin_row(row as u16) {
  ///       // Quick row processing...
  ///     }
  ///   }
  /// }
  /// # Ok(())
  /// # }
  /// ```
  pub fn row_iterator(&self) -> Option<RowIterator> {
    let iter_ptr = unsafe { ffi::ghostty_terminal_row_iterator_new(self.inner) };

    if iter_ptr.is_null() {
      return None;
    }

    Some(RowIterator {
      ptr:      iter_ptr,
      terminal: self.inner,
    })
  }

  /// Query a terminal mode state.
  ///
  /// Checks if a specific terminal mode is enabled. This is used to query
  /// various terminal behavior flags such as cursor visibility, application
  /// cursor keys, bracketed paste mode, etc.
  ///
  /// # Arguments
  /// * `mode_value` - Numeric mode identifier (see terminal spec)
  /// * `ansi` - If true, query ANSI mode space; if false, query DEC private
  ///   mode
  ///
  /// # Returns
  /// true if the mode is enabled, false if disabled or mode doesn't exist
  ///
  /// # Example
  /// ```no_run
  /// # use the_terminal::Terminal;
  /// # let terminal = Terminal::new(80, 24).unwrap();
  /// // Check if cursor is visible (DEC mode 25)
  /// let visible = terminal.get_mode(25, false);
  /// ```
  pub fn get_mode(&self, mode_value: u16, ansi: bool) -> bool {
    unsafe { ffi::ghostty_terminal_get_mode(self.inner, mode_value, ansi) }
  }

  /// Check if the cursor is visible.
  ///
  /// This is a convenience wrapper around `get_mode()` that specifically
  /// checks the DECTCEM (DEC mode 25) cursor visibility state.
  ///
  /// Applications can hide the cursor with CSI ?25l and show it with CSI ?25h.
  /// Many TUI applications hide the cursor during operation and show it again
  /// when exiting or in specific UI states.
  ///
  /// # Returns
  /// true if cursor should be rendered, false if it should be hidden
  ///
  /// # Example
  /// ```no_run
  /// # use the_terminal::Terminal;
  /// # let terminal = Terminal::new(80, 24).unwrap();
  /// if terminal.is_cursor_visible() {
  ///   // Render cursor
  /// }
  /// ```
  pub fn is_cursor_visible(&self) -> bool {
    self.get_mode(25, false) // DEC mode 25 = cursor_visible (DECTCEM)
  }

  /// Check if the viewport is at the bottom of the scrollback.
  ///
  /// This is critical for cursor rendering - ghostty only renders the cursor
  /// when the viewport is at the bottom. This prevents rendering the cursor
  /// when scrolled back in history.
  ///
  /// Use this in combination with `is_cursor_visible()` to determine if the
  /// cursor should be rendered:
  /// ```no_run
  /// # use the_terminal::Terminal;
  /// # let terminal = Terminal::new(80, 24).unwrap();
  /// if terminal.is_cursor_visible() && terminal.is_viewport_at_bottom() {
  ///   // Render cursor
  /// }
  /// ```
  ///
  /// # Returns
  /// true if viewport is at bottom, false otherwise
  pub fn is_viewport_at_bottom(&self) -> bool {
    unsafe { ffi::ghostty_terminal_is_viewport_at_bottom(self.inner) }
  }

  /// Get the terminal's default background color.
  ///
  /// Returns the background color used for cells that don't have an explicit
  /// background color set. This is the "base" color of the terminal that can
  /// be changed by applications via OSC 11 sequences.
  ///
  /// Use this color for cells where `cell.bg.is_none()`.
  ///
  /// # Returns
  /// Option<Rgb> - The default background color, or None if not set
  ///
  /// # Example
  /// ```no_run
  /// # use the_terminal::Terminal;
  /// # let terminal = Terminal::new(80, 24).unwrap();
  /// let default_bg = terminal.get_default_background().unwrap();
  /// // Use default_bg.r, default_bg.g, default_bg.b
  /// ```
  pub fn get_default_background(&self) -> Option<Rgb> {
    let color = unsafe { ffi::ghostty_terminal_get_default_background(self.inner) };

    if color.is_set {
      Some(Rgb {
        r: color.r,
        g: color.g,
        b: color.b,
      })
    } else {
      None
    }
  }
}

/// RAII wrapper for a pinned terminal row.
///
/// Provides zero-copy access to cell data. The pin is automatically freed
/// when dropped.
pub struct Pin {
  ptr: *mut ffi::GhosttyPin,
}

impl Pin {
  /// Get the number of cells in this pinned row.
  ///
  /// This is typically the terminal width in columns.
  pub fn cell_count(&self, terminal: &Terminal) -> usize {
    let mut count: usize = 0;
    unsafe {
      let cells_ptr = ffi::ghostty_terminal_pin_cells(terminal.inner, self.ptr, &mut count);
      if cells_ptr.is_null() {
        return 0;
      }
    }
    count
  }

  /// Get a cell's extended information (colors, attributes) at a specific
  /// index.
  ///
  /// This resolves colors and attributes on-demand, avoiding the cost of
  /// resolving data for cells that won't be rendered.
  ///
  /// # Arguments
  /// * `terminal` - Reference to the terminal (needed for palette access)
  /// * `col` - Column index (0 to cell_count()-1)
  ///
  /// # Returns
  /// Some(Cell) if index is valid, None otherwise
  pub fn get_cell_ext(&self, terminal: &Terminal, col: usize) -> Option<Cell> {
    let mut cell_ext = std::mem::MaybeUninit::<ffi::GhosttyCellExt>::uninit();

    let success = unsafe {
      ffi::ghostty_terminal_pin_populate_cell_ext(
        terminal.inner,
        self.ptr,
        col,
        cell_ext.as_mut_ptr(),
      )
    };

    if !success {
      return None;
    }

    let cell_ext = unsafe { cell_ext.assume_init() };
    Some(Cell::from(cell_ext))
  }
}

impl Drop for Pin {
  fn drop(&mut self) {
    unsafe {
      ffi::ghostty_terminal_pin_free(self.ptr);
    }
  }
}

/// Row iterator for efficient terminal viewport traversal.
///
/// This iterator yields (row_index, is_dirty) tuples for each row in the
/// viewport from top to bottom. It matches ghostty's rendering approach
/// by checking dirty status inline without allocating a dirty row list.
///
/// # Example
/// ```no_run
/// # use the_terminal::Terminal;
/// let terminal = Terminal::new(80, 24)?;
/// let iter = terminal.row_iterator()?;
/// for (row, is_dirty) in iter {
///   if is_dirty {
///     // Pin and render this row
///     let pin = terminal.pin_row(row as u16)?;
///     // ... render cells from pin ...
///   }
/// }
/// # Ok::<(), anyhow::Error>(())
/// ```
pub struct RowIterator {
  ptr:      *mut ffi::GhosttyRowIterator,
  terminal: *const ffi::GhosttyTerminal,
}

impl Iterator for RowIterator {
  type Item = (u32, bool); // (row_index, is_dirty)

  fn next(&mut self) -> Option<Self::Item> {
    let mut row_index: u32 = 0;
    let mut is_dirty: bool = false;

    let has_next = unsafe {
      ffi::ghostty_terminal_row_iterator_next(
        self.terminal,
        self.ptr,
        &mut row_index,
        &mut is_dirty,
      )
    };

    if has_next {
      Some((row_index, is_dirty))
    } else {
      None
    }
  }
}

impl Drop for RowIterator {
  fn drop(&mut self) {
    unsafe {
      ffi::ghostty_terminal_row_iterator_free(self.ptr);
    }
  }
}

// SAFETY: RowIterator holds pointers but they're managed correctly via Drop
unsafe impl Send for RowIterator {}

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
  pub selected:     bool, // True if this cell is in the current selection
}

/// A snapshot of the terminal screen for zero-copy rendering.
///
/// This structure contains ONLY metadata about terminal state - no cell data.
/// Cell data is accessed directly during rendering using pin-based iteration
/// to achieve true zero-copy performance.
///
/// This implements Ghostty's "clone-and-release" pattern: snapshot metadata
/// under lock (microseconds), then render without lock using pins.
#[derive(Debug, Clone, PartialEq)]
pub struct ScreenSnapshot {
  /// Cursor position (row, col)
  pub cursor_pos:         (u16, u16),
  /// Terminal dimensions (rows, cols)
  pub size:               (u16, u16),
  /// Dirty rows that need re-rendering
  pub dirty_rows:         Vec<u32>,
  /// True if full render is needed
  pub needs_full_rebuild: bool,
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
      selected: ext.selected,
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

impl ScreenSnapshot {
  /// Create a snapshot of the terminal screen metadata.
  ///
  /// This captures ONLY cursor position, size, and dirty rows - NO cell data.
  /// Cell data is accessed later during rendering using pin-based iteration.
  ///
  /// Lock hold time: ~1-10 microseconds (just copying metadata).
  ///
  /// # Arguments
  /// * `terminal` - Reference to the terminal to snapshot
  ///
  /// # Returns
  /// A new ScreenSnapshot containing only metadata
  pub fn from_terminal(terminal: &Terminal) -> Self {
    let size = (terminal.rows(), terminal.cols());
    let cursor_pos = terminal.cursor_pos();
    let needs_full_rebuild = terminal.needs_full_rebuild();
    let dirty_rows = if needs_full_rebuild {
      Vec::new() // Empty vec signals full render
    } else {
      terminal.get_dirty_rows()
    };

    Self {
      cursor_pos,
      size,
      dirty_rows,
      needs_full_rebuild,
    }
  }

  /// Get terminal dimensions from snapshot
  pub fn size(&self) -> (u16, u16) {
    self.size
  }

  /// Check if full render is needed
  pub fn is_full_render(&self) -> bool {
    self.dirty_rows.is_empty()
  }

  /// Get the number of dirty rows that need rendering
  pub fn dirty_row_count(&self) -> usize {
    if self.dirty_rows.is_empty() {
      self.size.0 as usize // Full render
    } else {
      self.dirty_rows.len()
    }
  }

  /// Calculate rendering efficiency metric (0.0 = full render, 1.0 = no
  /// changes) Useful for deciding whether to do incremental vs full render
  pub fn render_efficiency(&self) -> f32 {
    if self.dirty_rows.is_empty() {
      0.0 // Full render needed
    } else {
      1.0 - (self.dirty_rows.len() as f32 / self.size.0.max(1) as f32)
    }
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
