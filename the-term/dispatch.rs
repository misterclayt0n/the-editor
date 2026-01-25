//! Dispatch definition and handlers.

use the_dispatch::define;

use crate::Ctx;

/// Direction for cursor movement.
#[derive(Debug, Clone, Copy)]
pub enum Direction {
  Up,
  Down,
  Left,
  Right,
}

// Define dispatch points
define! {
    Term {
        // Text operations
        insert_char: char,
        delete_char: (),

        // Cursor operations
        move_cursor: Direction,
        add_cursor: Direction,

        // File operations
        save: (),

        // Lifecycle
        quit: (),
    }
}

// Type aliases for handler signatures
type InsertCharHandler = fn(&mut Ctx, char);
type DeleteCharHandler = fn(&mut Ctx, ());
type MoveCursorHandler = fn(&mut Ctx, Direction);
type AddCursorHandler = fn(&mut Ctx, Direction);
type SaveHandler = fn(&mut Ctx, ());
type QuitHandler = fn(&mut Ctx, ());

/// Concrete dispatch type for the application.
pub type AppDispatch = TermDispatch<
  Ctx,
  InsertCharHandler,
  DeleteCharHandler,
  MoveCursorHandler,
  AddCursorHandler,
  SaveHandler,
  QuitHandler,
>;

/// Build the dispatch with all handlers configured.
pub fn build_dispatch() -> AppDispatch {
  TermDispatch::new()
    .with_insert_char(handlers::insert_char as InsertCharHandler)
    .with_delete_char(handlers::delete_char as DeleteCharHandler)
    .with_move_cursor(handlers::move_cursor as MoveCursorHandler)
    .with_add_cursor(handlers::add_cursor as AddCursorHandler)
    .with_save(handlers::save as SaveHandler)
    .with_quit(handlers::quit as QuitHandler)
}

mod handlers {
  use the_lib::{
    selection::{
      Range,
      Selection,
    },
    transaction::Transaction,
  };

  use super::*;

  /// Insert a character at all cursor positions.
  pub fn insert_char(ctx: &mut Ctx, c: char) {
    let doc = ctx.editor.document_mut(ctx.active_doc).unwrap();
    let text = doc.text();

    // Build changes for all cursors
    let changes: Vec<_> = doc
      .selection()
      .iter()
      .map(|range| {
        let pos = range.cursor(text.slice(..));
        (pos, pos, Some(c.to_string().into()))
      })
      .collect();

    if let Ok(tx) = Transaction::change(text, changes) {
      let _ = doc.apply_transaction(&tx);
    }
  }

  /// Delete character before each cursor (backspace).
  pub fn delete_char(ctx: &mut Ctx, _: ()) {
    let doc = ctx.editor.document_mut(ctx.active_doc).unwrap();
    let text = doc.text();

    // Build changes for all cursors
    let changes: Vec<_> = doc
      .selection()
      .iter()
      .filter_map(|range| {
        let pos = range.cursor(text.slice(..));
        if pos > 0 {
          Some((pos - 1, pos, None))
        } else {
          None
        }
      })
      .collect();

    if !changes.is_empty() {
      if let Ok(tx) = Transaction::change(text, changes) {
        let _ = doc.apply_transaction(&tx);
      }
    }
  }

  /// Move all cursors in the given direction.
  pub fn move_cursor(ctx: &mut Ctx, dir: Direction) {
    let doc = ctx.editor.document_mut(ctx.active_doc).unwrap();
    let text = doc.text();
    let len = text.len_chars();

    let new_ranges: Vec<_> = doc
      .selection()
      .iter()
      .map(|range| {
        let pos = range.cursor(text.slice(..));
        let new_pos = match dir {
          Direction::Left => pos.saturating_sub(1),
          Direction::Right => (pos + 1).min(len),
          Direction::Up => {
            // Move to same column on previous line
            let line = text.char_to_line(pos);
            if line == 0 {
              pos
            } else {
              let col = pos - text.line_to_char(line);
              let prev_line_start = text.line_to_char(line - 1);
              let prev_line_len = text.line(line - 1).len_chars().saturating_sub(1);
              prev_line_start + col.min(prev_line_len)
            }
          },
          Direction::Down => {
            // Move to same column on next line
            let line = text.char_to_line(pos);
            let line_count = text.len_lines();
            if line >= line_count.saturating_sub(1) {
              pos
            } else {
              let col = pos - text.line_to_char(line);
              let next_line_start = text.line_to_char(line + 1);
              let next_line_len = text.line(line + 1).len_chars().saturating_sub(1);
              next_line_start + col.min(next_line_len)
            }
          },
        };
        Range::point(new_pos)
      })
      .collect();

    if let Ok(selection) = Selection::new(new_ranges.into()) {
      let _ = doc.set_selection(selection);
    }
  }

  /// Add a cursor in the given direction (for multiple cursors).
  pub fn add_cursor(ctx: &mut Ctx, dir: Direction) {
    let doc = ctx.editor.document_mut(ctx.active_doc).unwrap();
    let text = doc.text();

    // Get current ranges
    let mut ranges: Vec<_> = doc.selection().iter().cloned().collect();

    // Find the primary cursor position
    let primary = &doc.selection().ranges()[0];
    let pos = primary.cursor(text.slice(..));
    let line = text.char_to_line(pos);
    let col = pos - text.line_to_char(line);

    let new_pos = match dir {
      Direction::Up => {
        if line > 0 {
          let prev_line_start = text.line_to_char(line - 1);
          let prev_line_len = text.line(line - 1).len_chars().saturating_sub(1);
          Some(prev_line_start + col.min(prev_line_len))
        } else {
          None
        }
      },
      Direction::Down => {
        let line_count = text.len_lines();
        if line < line_count.saturating_sub(1) {
          let next_line_start = text.line_to_char(line + 1);
          let next_line_len = text.line(line + 1).len_chars().saturating_sub(1);
          Some(next_line_start + col.min(next_line_len))
        } else {
          None
        }
      },
      _ => None,
    };

    if let Some(new_pos) = new_pos {
      ranges.push(Range::point(new_pos));
      if let Ok(selection) = Selection::new(ranges.into()) {
        let _ = doc.set_selection(selection);
      }
    }
  }

  /// Save the document to file.
  pub fn save(ctx: &mut Ctx, _: ()) {
    if let Some(path) = &ctx.file_path {
      if let Some(doc) = ctx.editor.document(ctx.active_doc) {
        let text = doc.text().to_string();
        if let Err(e) = std::fs::write(path, text) {
          eprintln!("Failed to save: {e}");
        }
      }
    }
  }

  /// Quit the application.
  pub fn quit(ctx: &mut Ctx, _: ()) {
    ctx.should_quit = true;
  }
}
