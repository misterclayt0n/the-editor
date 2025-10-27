//! Safe Rust wrapper around libghostty-vt terminal emulation.

use std::ffi::c_void;

use crate::ffi::{
    self, GhosttyPoint, GhosttyTerminal, GhosttyTerminalOptions, ResponseCallback,
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
        let success = unsafe {
            ffi::ghostty_terminal_write(self.inner, data.as_ptr(), data.len())
        };

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
        let success = unsafe {
            ffi::ghostty_terminal_print_string(self.inner, s.as_ptr(), s.len())
        };

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

        let c = unsafe { ffi::ghostty_terminal_get_cell(self.inner, pt) };

        Cell {
            codepoint: c.codepoint,
            cluster: c.cluster,
            style: c.style,
            hyperlink_id: c.hyperlink_id,
        }
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
        let success = unsafe {
            ffi::ghostty_terminal_resize(self.inner, cols as u32, rows as u32)
        };

        if !success {
            return Err(anyhow::anyhow!("Failed to resize terminal"));
        }

        Ok(())
    }

    /// Get a view of the entire terminal grid.
    pub fn grid(&self) -> Grid<'_> {
        Grid {
            terminal: self,
            rows: self.rows(),
            cols: self.cols(),
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

/// Represents a single cell in the terminal grid.
#[derive(Debug, Clone, Copy)]
pub struct Cell {
    pub codepoint: u32,
    pub cluster: u32,
    pub style: u64,
    pub hyperlink_id: u32,
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

/// A view into the terminal grid for iteration.
pub struct Grid<'a> {
    terminal: &'a Terminal,
    rows: u16,
    cols: u16,
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
        self.row_cells(row)
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
