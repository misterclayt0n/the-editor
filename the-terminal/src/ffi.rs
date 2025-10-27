//! Low-level FFI bindings to libghostty-vt C API.
//!
//! These are the raw C function declarations from libghostty-vt.
//! They are unsafe and should be wrapped by safe types in the terminal module.

use std::ffi::c_uint;

/// Opaque terminal structure from libghostty
#[repr(C)]
pub struct GhosttyTerminal {
    _private: [u8; 0],
}

/// Represents a single cell in the terminal grid
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct GhosttyCell {
    pub codepoint: u32,
    pub cluster: u32,
    pub style: u64,
    pub hyperlink_id: u32,
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

unsafe extern "C" {
    /// Initialize a new terminal with the given options.
    ///
    /// Returns a pointer to a new Terminal instance that must be freed with
    /// ghostty_terminal_free.
    pub fn ghostty_terminal_new(opts: *const GhosttyTerminalOptions) -> *mut GhosttyTerminal;

    /// Free a terminal instance.
    pub fn ghostty_terminal_free(term: *mut GhosttyTerminal);

    /// Print a UTF-8 string to the terminal.
    ///
    /// The string will be processed as VT escape sequences and regular characters.
    pub fn ghostty_terminal_print_string(term: *mut GhosttyTerminal, s: *const u8, len: usize) -> bool;

    /// Get the width (columns) of the terminal.
    pub fn ghostty_terminal_cols(term: *const GhosttyTerminal) -> c_uint;

    /// Get the height (rows) of the terminal.
    pub fn ghostty_terminal_rows(term: *const GhosttyTerminal) -> c_uint;

    /// Get a cell from the terminal grid at the given position.
    ///
    /// Returns a GhosttyCell. If the position is out of bounds, returns a cell
    /// with default values.
    pub fn ghostty_terminal_get_cell(
        term: *const GhosttyTerminal,
        pt: GhosttyPoint,
    ) -> GhosttyCell;

    /// Get the current cursor position.
    pub fn ghostty_terminal_cursor_pos(term: *const GhosttyTerminal) -> GhosttyPoint;

    /// Resize the terminal to new dimensions.
    ///
    /// Returns true on success, false on failure.
    pub fn ghostty_terminal_resize(term: *mut GhosttyTerminal, cols: c_uint, rows: c_uint) -> bool;
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
