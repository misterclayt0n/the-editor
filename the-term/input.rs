//! Input handling - maps key events to dispatch calls.

use crossterm::event::{
  KeyCode,
  KeyEvent as CrosstermKeyEvent,
  KeyModifiers,
};

use crate::{
  Ctx,
  dispatch::{
    Key,
    KeyEvent,
    Modifiers,
    handle_key as dispatch_handle_key,
  },
};
use the_default::Command;
use the_dispatch::{
  DispatchPlugin,
  editor::{
    KeyOutcome,
    KeyPipelineApi,
  },
};

/// Orchestration function - maps keyboard input to dispatch calls.
pub fn handle_key<D, P>(
  dispatch: &mut D,
  pipeline: &mut P,
  ctx: &mut Ctx,
  event: CrosstermKeyEvent,
)
where
  D: DispatchPlugin<Ctx, Command>,
  P: KeyPipelineApi<Ctx>,
{
  let modifiers = to_modifiers(event.modifiers);
  let Some(key) = to_key(event.code) else {
    return;
  };

  let key_event = KeyEvent { key, modifiers };

  match pipeline.pre(ctx, key_event) {
    KeyOutcome::Command(command) => {
      let _ = dispatch.dispatch(ctx, command);
      return;
    },
    KeyOutcome::Handled => return,
    KeyOutcome::Continue => {},
  }

  match pipeline.on(ctx, key_event) {
    KeyOutcome::Command(command) => {
      let _ = dispatch.dispatch(ctx, command);
      return;
    },
    KeyOutcome::Handled => return,
    KeyOutcome::Continue => {},
  }

  dispatch_handle_key(dispatch, ctx, key_event);

  match pipeline.post(ctx, key_event) {
    KeyOutcome::Command(command) => {
      let _ = dispatch.dispatch(ctx, command);
    },
    KeyOutcome::Handled | KeyOutcome::Continue => {},
  }
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
    out.insert(Modifiers::CTRL);
  }
  if modifiers.contains(KeyModifiers::ALT) {
    out.insert(Modifiers::ALT);
  }
  out
}
