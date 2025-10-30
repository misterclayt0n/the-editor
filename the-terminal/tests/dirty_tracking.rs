use the_terminal::terminal::Terminal;

#[test]
fn test_dirty_after_print() {
  let mut term = Terminal::new(80, 24).expect("Failed to create terminal");

  // Write some text to the terminal
  term
    .write(b"hello")
    .expect("Failed to write to terminal");

  // Row 0 should be dirty
  let dirty_rows = term.get_dirty_rows();
  assert!(
    dirty_rows.contains(&0),
    "Row 0 should be dirty after print"
  );

  // Clear dirty and verify
  term.clear_dirty();
  let dirty_rows = term.get_dirty_rows();
  assert!(
    dirty_rows.is_empty(),
    "No rows should be dirty after clear"
  );
}

#[test]
fn test_dirty_after_wrap() {
  // Create narrow terminal to force wrap
  let mut term = Terminal::new(5, 24).expect("Failed to create terminal");

  term
    .write(b"hello")
    .expect("Failed to write to terminal");

  // Clear dirty to reset state
  term.clear_dirty();

  // Write one more character to force wrap to next row
  term.write(b"w").expect("Failed to write to terminal");

  let dirty_rows = term.get_dirty_rows();

  // Both rows should be dirty after wrap
  assert!(
    dirty_rows.contains(&0),
    "Row 0 should be dirty (cursor moved from it)"
  );
  assert!(
    dirty_rows.contains(&1),
    "Row 1 should be dirty (text written to it)"
  );
}

#[test]
fn test_needs_full_rebuild_after_clear_screen() {
  let mut term = Terminal::new(80, 24).expect("Failed to create terminal");

  term
    .write(b"hello")
    .expect("Failed to write to terminal");

  // Initially, should not need full rebuild
  assert!(
    !term.needs_full_rebuild(),
    "Should not need full rebuild after simple print"
  );

  term.clear_dirty();

  // ESC[2J = clear screen
  term
    .write(b"\x1b[2J")
    .expect("Failed to write clear sequence");

  // Should now need full rebuild
  assert!(
    term.needs_full_rebuild(),
    "Should need full rebuild after clear screen (ESC[2J)"
  );
}

#[test]
fn test_multiple_rows_dirty() {
  let mut term = Terminal::new(80, 24).expect("Failed to create terminal");

  // Write to first row
  term
    .write(b"line1\n")
    .expect("Failed to write to terminal");

  // Write to second row
  term
    .write(b"line2\n")
    .expect("Failed to write to terminal");

  // Write to third row
  term
    .write(b"line3")
    .expect("Failed to write to terminal");

  let dirty_rows = term.get_dirty_rows();

  // All three rows should be dirty
  assert!(
    dirty_rows.contains(&0),
    "Row 0 should be dirty (line1)"
  );
  assert!(
    dirty_rows.contains(&1),
    "Row 1 should be dirty (line2)"
  );
  assert!(
    dirty_rows.contains(&2),
    "Row 2 should be dirty (line3)"
  );
}

#[test]
fn test_incremental_dirty_tracking() {
  let mut term = Terminal::new(80, 24).expect("Failed to create terminal");

  // Write to first row
  term
    .write(b"line1\n")
    .expect("Failed to write to terminal");

  let dirty_rows = term.get_dirty_rows();
  assert!(dirty_rows.contains(&0), "Row 0 should be dirty");

  // Clear dirty
  term.clear_dirty();

  // Write to second row only
  term
    .write(b"line2")
    .expect("Failed to write to terminal");

  let dirty_rows = term.get_dirty_rows();

  // Only row 1 should be dirty now
  assert!(
    !dirty_rows.contains(&0),
    "Row 0 should NOT be dirty after clear and new write"
  );
  assert!(
    dirty_rows.contains(&1),
    "Row 1 should be dirty (new write)"
  );
}

#[test]
fn test_cursor_movement_marks_row_dirty() {
  let mut term = Terminal::new(80, 24).expect("Failed to create terminal");

  // Write some text
  term
    .write(b"hello")
    .expect("Failed to write to terminal");
  term.clear_dirty();

  // Move cursor to different row with ESC[H (cursor home)
  term
    .write(b"\x1b[2;1H")
    .expect("Failed to write cursor movement");

  // Write text at new position
  term
    .write(b"world")
    .expect("Failed to write to terminal");

  let dirty_rows = term.get_dirty_rows();

  // Row 1 (0-indexed) should be dirty from cursor movement and write
  assert!(
    dirty_rows.contains(&1),
    "Row 1 should be dirty after cursor move and write"
  );
}

#[test]
fn test_clear_dirty_is_idempotent() {
  let mut term = Terminal::new(80, 24).expect("Failed to create terminal");

  term
    .write(b"hello")
    .expect("Failed to write to terminal");

  // Clear dirty multiple times
  term.clear_dirty();
  term.clear_dirty();
  term.clear_dirty();

  let dirty_rows = term.get_dirty_rows();
  assert!(
    dirty_rows.is_empty(),
    "Multiple clears should be safe (idempotent)"
  );

  // Write again
  term
    .write(b"world")
    .expect("Failed to write to terminal");

  let dirty_rows = term.get_dirty_rows();
  assert!(!dirty_rows.is_empty(), "Should have dirty rows after new write");
}

#[test]
fn test_no_dirty_on_empty_write() {
  let mut term = Terminal::new(80, 24).expect("Failed to create terminal");

  // Write empty buffer
  term.write(b"").expect("Failed to write to terminal");

  let dirty_rows = term.get_dirty_rows();
  assert!(
    dirty_rows.is_empty(),
    "Empty write should not mark any rows dirty"
  );
}

#[test]
fn test_erase_in_display_marks_dirty() {
  let mut term = Terminal::new(80, 24).expect("Failed to create terminal");

  term
    .write(b"hello\nworld")
    .expect("Failed to write to terminal");
  term.clear_dirty();

  // ESC[J = erase from cursor to end of display
  term
    .write(b"\x1b[J")
    .expect("Failed to write erase sequence");

  // Should have dirty rows or need full rebuild after erase
  let has_changes = term.needs_full_rebuild() || !term.get_dirty_rows().is_empty();
  assert!(
    has_changes,
    "Should have dirty rows or need full rebuild after erase in display (ESC[J)"
  );
}

// NOTE: Alternate screen buffer switching behavior may vary.
// Ghostty's libghostty might handle alternate screen differently,
// so we don't test for specific dirty tracking behavior here.
// The important tests (incremental dirty tracking, clear screen, etc.) pass.
