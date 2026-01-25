//! Input handling - maps key events to dispatch calls.

use crossterm::event::{
  KeyCode,
  KeyEvent,
  KeyModifiers,
};

use crate::{
  Ctx,
  dispatch::{
    Direction,
    TermApi,
  },
};

/// Orchestration function - maps keyboard input to dispatch calls.
pub fn handle_key<D: TermApi<Ctx>>(dispatch: &D, ctx: &mut Ctx, key: KeyEvent) {
  let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
  let alt = key.modifiers.contains(KeyModifiers::ALT);

  match key.code {
    // Text input (only when no modifiers)
    KeyCode::Char(c) if !ctrl && !alt => {
      dispatch.insert_char(ctx, c);
      ctx.needs_render = true;
    },

    // Enter key - insert newline
    KeyCode::Enter if !ctrl && !alt => {
      dispatch.insert_char(ctx, '\n');
      ctx.needs_render = true;
    },

    // Backspace - delete character
    KeyCode::Backspace => {
      dispatch.delete_char(ctx, ());
      ctx.needs_render = true;
    },

    // Arrow keys - movement
    KeyCode::Left if !alt => {
      dispatch.move_cursor(ctx, Direction::Left);
      ctx.needs_render = true;
    },
    KeyCode::Right if !alt => {
      dispatch.move_cursor(ctx, Direction::Right);
      ctx.needs_render = true;
    },
    KeyCode::Up if !alt => {
      dispatch.move_cursor(ctx, Direction::Up);
      ctx.needs_render = true;
    },
    KeyCode::Down if !alt => {
      dispatch.move_cursor(ctx, Direction::Down);
      ctx.needs_render = true;
    },

    // Alt+Arrow - add cursor (multiple cursors)
    KeyCode::Up if alt => {
      dispatch.add_cursor(ctx, Direction::Up);
      ctx.needs_render = true;
    },
    KeyCode::Down if alt => {
      dispatch.add_cursor(ctx, Direction::Down);
      ctx.needs_render = true;
    },

    // Ctrl+S - save
    KeyCode::Char('s') if ctrl => {
      dispatch.save(ctx, ());
    },

    // Ctrl+Q - quit
    KeyCode::Char('q') if ctrl => {
      dispatch.quit(ctx, ());
    },

    _ => {},
  }
}
