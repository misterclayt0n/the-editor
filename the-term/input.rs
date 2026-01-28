//! Input handling - maps key events to dispatch calls.

use crossterm::event::{
  KeyCode,
  KeyEvent as CrosstermKeyEvent,
  KeyModifiers,
};
use the_default::{
  DefaultApi,
  KeyOutcome,
  KeyPipelineApi,
  handle_command,
  handle_key as dispatch_handle_key,
};

use crate::{
  Ctx,
  dispatch::{
    Key,
    KeyEvent,
    Modifiers,
  },
};

/// Orchestration function - maps keyboard input to dispatch calls.
pub fn handle_key<D, P>(dispatch: &D, pipeline: &mut P, ctx: &mut Ctx, event: CrosstermKeyEvent)
where
  D: DefaultApi<Ctx>,
  P: KeyPipelineApi<Ctx>,
{
  let modifiers = to_modifiers(event.modifiers);
  let Some(key) = to_key(event.code) else {
    return;
  };

  let key_event = KeyEvent { key, modifiers };

  match pipeline.pre(ctx, key_event) {
    KeyOutcome::Command(command) => {
      handle_command(dispatch, ctx, command);
      return;
    },
    KeyOutcome::Commands(commands) => {
      for command in commands {
        handle_command(dispatch, ctx, command);
      }
      return;
    },
    KeyOutcome::Handled => return,
    KeyOutcome::Continue => {},
  }

  match pipeline.on(ctx, key_event) {
    KeyOutcome::Command(command) => {
      handle_command(dispatch, ctx, command);
      return;
    },
    KeyOutcome::Commands(commands) => {
      for command in commands {
        handle_command(dispatch, ctx, command);
      }
      return;
    },
    KeyOutcome::Handled => return,
    KeyOutcome::Continue => {},
  }

  dispatch_handle_key(dispatch, ctx, key_event);

  match pipeline.post(ctx, key_event) {
    KeyOutcome::Command(command) => {
      handle_command(dispatch, ctx, command);
    },
    KeyOutcome::Commands(commands) => {
      for command in commands {
        handle_command(dispatch, ctx, command);
      }
    },
    KeyOutcome::Handled | KeyOutcome::Continue => {},
  }
}

fn to_key(code: KeyCode) -> Option<Key> {
  match code {
    KeyCode::Char(c) => Some(Key::Char(c)),
    KeyCode::Enter => Some(Key::Enter),
    KeyCode::Tab => Some(Key::Tab),
    KeyCode::BackTab => Some(Key::Tab),
    KeyCode::Esc => Some(Key::Escape),
    KeyCode::Backspace => Some(Key::Backspace),
    KeyCode::Delete => Some(Key::Delete),
    KeyCode::Insert => Some(Key::Insert),
    KeyCode::Home => Some(Key::Home),
    KeyCode::End => Some(Key::End),
    KeyCode::PageUp => Some(Key::PageUp),
    KeyCode::PageDown => Some(Key::PageDown),
    KeyCode::Left => Some(Key::Left),
    KeyCode::Right => Some(Key::Right),
    KeyCode::Up => Some(Key::Up),
    KeyCode::Down => Some(Key::Down),
    KeyCode::F(1) => Some(Key::F1),
    KeyCode::F(2) => Some(Key::F2),
    KeyCode::F(3) => Some(Key::F3),
    KeyCode::F(4) => Some(Key::F4),
    KeyCode::F(5) => Some(Key::F5),
    KeyCode::F(6) => Some(Key::F6),
    KeyCode::F(7) => Some(Key::F7),
    KeyCode::F(8) => Some(Key::F8),
    KeyCode::F(9) => Some(Key::F9),
    KeyCode::F(10) => Some(Key::F10),
    KeyCode::F(11) => Some(Key::F11),
    KeyCode::F(12) => Some(Key::F12),
    _ => None,
  }
}

fn to_modifiers(modifiers: KeyModifiers) -> Modifiers {
  let mut out = Modifiers::empty();
  if modifiers.contains(KeyModifiers::CONTROL) {
    out.insert(Modifiers::CTRL);
  }
  if modifiers.contains(KeyModifiers::ALT) {
    out.insert(Modifiers::ALT);
  }
  if modifiers.contains(KeyModifiers::SHIFT) {
    out.insert(Modifiers::SHIFT);
  }
  out
}
