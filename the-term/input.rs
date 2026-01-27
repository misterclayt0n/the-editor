//! Input handling - maps key events to dispatch calls.

use crossterm::event::{
  KeyCode,
  KeyEvent as CrosstermKeyEvent,
  KeyModifiers,
};

use crate::{
  Ctx,
  dispatch::{
    AppDispatch,
    Key,
    KeyEvent,
    Modifiers,
    handle_key as dispatch_handle_key,
  },
};

/// Orchestration function - maps keyboard input to dispatch calls.
pub fn handle_key(dispatch: &mut AppDispatch, ctx: &mut Ctx, event: CrosstermKeyEvent) {
  let modifiers = to_modifiers(event.modifiers);
  let Some(key) = to_key(event.code) else {
    return;
  };

  dispatch_handle_key(dispatch, ctx, KeyEvent { key, modifiers });
}

fn to_key(code: KeyCode) -> Option<Key> {
  match code {
    KeyCode::Char(c) => Some(Key::Char(c)),
    KeyCode::Enter => Some(Key::Enter),
    KeyCode::Backspace => Some(Key::Backspace),
    KeyCode::Left => Some(Key::Left),
    KeyCode::Right => Some(Key::Right),
    KeyCode::Up => Some(Key::Up),
    KeyCode::Down => Some(Key::Down),
    _ => None,
  }
}

fn to_modifiers(modifiers: KeyModifiers) -> Modifiers {
  let mut out = Modifiers::empty();
  if modifiers.contains(KeyModifiers::CONTROL) {
    out |= Modifiers::CTRL;
  }
  if modifiers.contains(KeyModifiers::ALT) {
    out |= Modifiers::ALT;
  }
  out
}
