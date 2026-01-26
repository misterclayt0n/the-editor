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
    movement::{
      Direction as MoveDir,
      Movement,
      move_horizontally,
      move_vertically,
    },
    render::{
      text_annotations::TextAnnotations,
      text_format::TextFormat,
    },
    selection::Selection,
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

  /// Move all cursors in the given direction using the_lib movement functions.
  pub fn move_cursor(ctx: &mut Ctx, dir: Direction) {
    let doc = ctx.editor.document_mut(ctx.active_doc).unwrap();
    let slice = doc.text().slice(..);
    let text_fmt = TextFormat::default();
    let mut annotations = TextAnnotations::default();

    // Use Selection::transform to apply movement to each range
    let selection = doc.selection().clone().transform(|range| {
      match dir {
        Direction::Left => {
          move_horizontally(
            slice,
            range,
            MoveDir::Backward,
            1,
            Movement::Move,
            &text_fmt,
            &mut annotations,
          )
        },
        Direction::Right => {
          move_horizontally(
            slice,
            range,
            MoveDir::Forward,
            1,
            Movement::Move,
            &text_fmt,
            &mut annotations,
          )
        },
        Direction::Up => {
          move_vertically(
            slice,
            range,
            MoveDir::Backward,
            1,
            Movement::Move,
            &text_fmt,
            &mut annotations,
          )
        },
        Direction::Down => {
          move_vertically(
            slice,
            range,
            MoveDir::Forward,
            1,
            Movement::Move,
            &text_fmt,
            &mut annotations,
          )
        },
      }
    });

    // Drop annotations before mutably borrowing doc for set_selection
    drop(annotations);
    let _ = doc.set_selection(selection);
  }

  /// Add a cursor in the given direction (for multiple cursors).
  ///
  /// This creates a new cursor on the line above/below, preserving the column.
  pub fn add_cursor(ctx: &mut Ctx, dir: Direction) {
    let doc = ctx.editor.document_mut(ctx.active_doc).unwrap();
    let slice = doc.text().slice(..);
    let text_fmt = TextFormat::default();
    let mut annotations = TextAnnotations::default();

    // Get current ranges
    let mut ranges: Vec<_> = doc.selection().iter().cloned().collect();

    // Compute new cursor position from primary cursor
    let primary = ranges[0];
    let new_range = match dir {
      Direction::Up => {
        move_vertically(
          slice,
          primary,
          MoveDir::Backward,
          1,
          Movement::Move,
          &text_fmt,
          &mut annotations,
        )
      },
      Direction::Down => {
        move_vertically(
          slice,
          primary,
          MoveDir::Forward,
          1,
          Movement::Move,
          &text_fmt,
          &mut annotations,
        )
      },
      // Left/Right don't add cursors
      _ => return,
    };

    // Drop annotations before mutably borrowing doc for set_selection
    drop(annotations);

    // Only add if the new position is different from primary
    if new_range.cursor(slice) != primary.cursor(slice) {
      ranges.push(new_range);
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
