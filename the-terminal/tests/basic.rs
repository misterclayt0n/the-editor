//! Basic integration tests for the terminal emulation.

use the_terminal::Terminal;

#[test]
fn test_terminal_creation() {
  let term = Terminal::new(80, 24).expect("Failed to create terminal");
  assert_eq!(term.cols(), 80);
  assert_eq!(term.rows(), 24);
}

#[test]
fn test_print_and_read() {
  let mut term = Terminal::new(80, 24).expect("Failed to create terminal");

  // Print "hello" to the terminal
  term.print_string("hello").expect("Failed to print string");

  // Read back the grid and verify content
  let grid = term.grid();
  let row_0 = grid.row_string(0);

  // The row should contain "hello" at the beginning
  assert!(
    row_0.starts_with("hello"),
    "Row 0 should start with 'hello', got: '{}'",
    row_0
  );
}

#[test]
fn test_cell_access() {
  let mut term = Terminal::new(80, 24).expect("Failed to create terminal");
  term.print_string("A").expect("Failed to print");

  let cell = term.get_cell(0, 0);
  assert_eq!(cell.codepoint, 'A' as u32);
  assert!(cell.character() == Some('A'));
}

#[test]
fn test_cursor_position() {
  let mut term = Terminal::new(80, 24).expect("Failed to create terminal");
  term.print_string("test").expect("Failed to print");

  let (row, col) = term.cursor_pos();
  // Cursor should be at column 4 after printing "test" (4 chars)
  assert_eq!(row, 0);
  assert_eq!(col, 4);
}

#[test]
fn test_grid_iteration() {
  let mut term = Terminal::new(80, 24).expect("Failed to create terminal");
  term.print_string("hi").expect("Failed to print");

  let grid = term.grid();
  let mut cell_count = 0;

  for (_, _, cell) in grid.cells() {
    if !cell.is_empty() {
      cell_count += 1;
    }
  }

  // Should have at least the 2 characters we printed
  assert!(
    cell_count >= 2,
    "Expected at least 2 non-empty cells, got {}",
    cell_count
  );
}

#[test]
fn test_multiline_content() {
  let mut term = Terminal::new(80, 24).expect("Failed to create terminal");

  // Print text with newlines
  term.print_string("line1\nline2").expect("Failed to print");

  let grid = term.grid();
  let row_0 = grid.row_string(0);
  let row_1 = grid.row_string(1);

  assert!(row_0.contains("line1"), "Row 0 should contain 'line1'");
  assert!(row_1.contains("line2"), "Row 1 should contain 'line2'");
}

#[test]
fn test_grid_as_string() {
  let mut term = Terminal::new(80, 24).expect("Failed to create terminal");
  term.print_string("test").expect("Failed to print");

  let grid = term.grid();
  let content = grid.as_string();

  // The entire grid as a string should contain our text
  assert!(
    content.contains("test"),
    "Grid content should contain 'test'"
  );
}
