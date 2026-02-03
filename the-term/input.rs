//! Input handling - maps key events to dispatch calls.

use crossterm::event::{
  KeyCode,
  KeyEvent as CrosstermKeyEvent,
  KeyEventKind,
  KeyModifiers,
};
use the_default::{
  DefaultContext,
  Mode,
  ui_event as dispatch_ui_event,
};
use the_lib::render::{
  UiEvent,
  UiEventKind,
  UiKey,
  UiKeyEvent,
  UiModifiers,
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
pub fn handle_key(ctx: &mut Ctx, event: CrosstermKeyEvent) {
  // Only handle key press events, not release or repeat
  if event.kind != KeyEventKind::Press {
    return;
  }

  if ctx.mode() == Mode::Command {
    if let Some(mut key) = to_ui_key(event.code) {
      if matches!(event.code, KeyCode::Tab | KeyCode::BackTab) {
        key = if event.modifiers.contains(KeyModifiers::SHIFT) || event.code == KeyCode::BackTab {
          UiKey::Up
        } else {
          UiKey::Down
        };
      }

      let ui_event = UiEvent {
        target: None,
        kind: UiEventKind::Key(UiKeyEvent {
          key,
          modifiers: to_ui_modifiers(event.modifiers),
        }),
      };
      let _ = dispatch_ui_event(ctx, ui_event);
      return;
    }
  }

  let modifiers = to_modifiers(event.modifiers, event.code);
  let Some(key) = to_key(event.code) else {
    return;
  };

  let key_event = KeyEvent { key, modifiers };

  ctx.dispatch().pre_on_keypress(ctx, key_event);
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

fn to_ui_key(code: KeyCode) -> Option<UiKey> {
  match code {
    KeyCode::Char(c) => Some(UiKey::Char(c)),
    KeyCode::Enter => Some(UiKey::Enter),
    KeyCode::Tab => Some(UiKey::Tab),
    KeyCode::BackTab => Some(UiKey::Tab),
    KeyCode::Esc => Some(UiKey::Escape),
    KeyCode::Backspace => Some(UiKey::Backspace),
    KeyCode::Delete => Some(UiKey::Delete),
    KeyCode::Home => Some(UiKey::Home),
    KeyCode::End => Some(UiKey::End),
    KeyCode::PageUp => Some(UiKey::PageUp),
    KeyCode::PageDown => Some(UiKey::PageDown),
    KeyCode::Left => Some(UiKey::Left),
    KeyCode::Right => Some(UiKey::Right),
    KeyCode::Up => Some(UiKey::Up),
    KeyCode::Down => Some(UiKey::Down),
    _ => None,
  }
}

fn to_modifiers(modifiers: KeyModifiers, code: KeyCode) -> Modifiers {
  let mut out = Modifiers::empty();
  if modifiers.contains(KeyModifiers::CONTROL) {
    out.insert(Modifiers::CTRL);
  }
  if modifiers.contains(KeyModifiers::ALT) {
    out.insert(Modifiers::ALT);
  }
  if modifiers.contains(KeyModifiers::SHIFT) {
    // Don't include SHIFT for characters that are inherently shifted
    // (uppercase letters, symbols produced by shift+number, etc.)
    // The shift is already represented in the character itself.
    let dominated_by_char = matches!(code, KeyCode::Char(c) if c.is_uppercase() || "~!@#$%^&*()_+{}|:\"<>?".contains(c));
    if !dominated_by_char {
      out.insert(Modifiers::SHIFT);
    }
  }
  out
}

fn to_ui_modifiers(modifiers: KeyModifiers) -> UiModifiers {
  UiModifiers {
    ctrl: modifiers.contains(KeyModifiers::CONTROL),
    alt: modifiers.contains(KeyModifiers::ALT),
    shift: modifiers.contains(KeyModifiers::SHIFT),
    meta: modifiers.contains(KeyModifiers::SUPER),
  }
}
