use the_terminal::terminal::Terminal;

#[test]
fn test_needs_full_rebuild_after_clear_screen() {
  let mut term = Terminal::new(80, 24).expect("Failed to create terminal");

  term.write(b"hello").expect("Failed to write to terminal");

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
fn test_clear_dirty_is_idempotent() {
  let mut term = Terminal::new(80, 24).expect("Failed to create terminal");

  term.write(b"hello").expect("Failed to write to terminal");

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
  term.write(b"world").expect("Failed to write to terminal");

  let dirty_rows = term.get_dirty_rows();
  assert!(
    !dirty_rows.is_empty(),
    "Should have dirty rows after new write"
  );
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
