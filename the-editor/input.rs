//! Unified input processing system
//!
//! This module provides a clean, unified interface for handling all input
//! events, converting raw keyboard/text events into a single stream of
//! processed events.

use the_editor_renderer::{
  InputEvent,
  Key,
  KeyPress,
  MouseEvent,
  ScrollDelta,
};

use crate::keymap::{
  KeyBinding,
  Mode,
};

/// Unified key event after processing and normalization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnifiedKey {
  /// A character that should be inserted or used in commands.
  Character(char),
  /// A special non-character key.
  Special(SpecialKey),
  /// A modified character (e.g., Ctrl+A, Shift+Space).
  Modified {
    key:   char,
    shift: bool,
    ctrl:  bool,
    alt:   bool,
  },
  /// A modified special key (e.g., Alt+Backspace, Ctrl+Delete).
  ModifiedSpecial {
    key:   SpecialKey,
    shift: bool,
    ctrl:  bool,
    alt:   bool,
  },
  /// Escape key (special handling in many contexts).
  Escape,
}

/// Special keys that don't produce characters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpecialKey {
  Enter,
  NumpadEnter,
  Tab,
  Backspace,
  Delete,
  Insert,
  Home,
  End,
  PageUp,
  PageDown,
  Up,
  Down,
  Left,
  Right,
  F1,
  F2,
  F3,
  F4,
  F5,
  F6,
  F7,
  F8,
  F9,
  F10,
  F11,
  F12,
}

/// Processed input event ready for consumption.
#[derive(Debug, Clone)]
pub enum ProcessedInput {
  /// A key event that should be handled.
  Key(UnifiedKey),
  /// Mouse input (passed through mostly unchanged).
  Mouse(MouseEvent),
  /// Scroll input.
  Scroll(ScrollDelta),
}

/// Input processing state machine.
#[derive(Debug, Clone)]
pub struct InputProcessor {
  /// Current editor mode.
  mode:         Mode,
  /// Whether we're waiting for a character (e.g., after 'r' command).
  pending_char: bool,
  /// Accumulated modifiers from keyboard events.
  shift_held:   bool,
  ctrl_held:    bool,
  alt_held:     bool,
}

impl InputProcessor {
  pub fn new(mode: Mode) -> Self {
    Self {
      mode,
      pending_char: false,
      shift_held: false,
      ctrl_held: false,
      alt_held: false,
    }
  }

  pub fn set_mode(&mut self, mode: Mode) {
    self.mode = mode;
  }

  pub fn set_pending_char(&mut self, pending: bool) {
    self.pending_char = pending;
  }

  /// Process a raw input event into zero or more processed events.
  pub fn process(&mut self, event: InputEvent) -> Vec<ProcessedInput> {
    match event {
      InputEvent::Keyboard(key_press) => self.process_keyboard(key_press),
      InputEvent::Text(text) => self.process_text(text),
      InputEvent::Mouse(mouse) => vec![ProcessedInput::Mouse(mouse)],
      InputEvent::Scroll(delta) => vec![ProcessedInput::Scroll(delta)],
    }
  }

  fn process_keyboard(&mut self, key_press: KeyPress) -> Vec<ProcessedInput> {
    // Only process key presses, not releases.
    if !key_press.pressed {
      return vec![];
    }

    // Update modifier state.
    self.shift_held = key_press.shift;
    self.ctrl_held = key_press.ctrl;
    self.alt_held = key_press.alt;

    // Check if special keys have modifiers - if so, treat them as modified keys
    let has_modifiers = key_press.ctrl || key_press.alt || key_press.shift;

    // DEBUG: Log raw key press
    log::debug!(
      "InputProcessor: key={:?}, shift={}, ctrl={}, alt={}, has_modifiers={}",
      key_press.code,
      key_press.shift,
      key_press.ctrl,
      key_press.alt,
      has_modifiers
    );

    // Convert to unified key.
    let unified = match key_press.code {
      Key::Escape => Some(UnifiedKey::Escape),
      // Special keys with modifiers need special handling
      Key::Enter if has_modifiers => {
        Some(UnifiedKey::ModifiedSpecial {
          key:   SpecialKey::Enter,
          shift: key_press.shift,
          ctrl:  key_press.ctrl,
          alt:   key_press.alt,
        })
      },
      Key::NumpadEnter if has_modifiers => {
        Some(UnifiedKey::ModifiedSpecial {
          key:   SpecialKey::NumpadEnter,
          shift: key_press.shift,
          ctrl:  key_press.ctrl,
          alt:   key_press.alt,
        })
      },
      Key::Tab if has_modifiers => {
        Some(UnifiedKey::ModifiedSpecial {
          key:   SpecialKey::Tab,
          shift: key_press.shift,
          ctrl:  key_press.ctrl,
          alt:   key_press.alt,
        })
      },
      Key::Backspace if has_modifiers => {
        Some(UnifiedKey::ModifiedSpecial {
          key:   SpecialKey::Backspace,
          shift: key_press.shift,
          ctrl:  key_press.ctrl,
          alt:   key_press.alt,
        })
      },
      Key::Delete if has_modifiers => {
        Some(UnifiedKey::ModifiedSpecial {
          key:   SpecialKey::Delete,
          shift: key_press.shift,
          ctrl:  key_press.ctrl,
          alt:   key_press.alt,
        })
      },
      // Unmodified special keys
      Key::Enter => Some(UnifiedKey::Special(SpecialKey::Enter)),
      Key::NumpadEnter => Some(UnifiedKey::Special(SpecialKey::NumpadEnter)),
      Key::Tab => Some(UnifiedKey::Special(SpecialKey::Tab)),
      Key::Backspace => Some(UnifiedKey::Special(SpecialKey::Backspace)),
      Key::Delete => Some(UnifiedKey::Special(SpecialKey::Delete)),
      Key::Insert => Some(UnifiedKey::Special(SpecialKey::Insert)),
      Key::Home => Some(UnifiedKey::Special(SpecialKey::Home)),
      Key::End => Some(UnifiedKey::Special(SpecialKey::End)),
      Key::PageUp => Some(UnifiedKey::Special(SpecialKey::PageUp)),
      Key::PageDown => Some(UnifiedKey::Special(SpecialKey::PageDown)),
      Key::Up => Some(UnifiedKey::Special(SpecialKey::Up)),
      Key::Down => Some(UnifiedKey::Special(SpecialKey::Down)),
      Key::Left => Some(UnifiedKey::Special(SpecialKey::Left)),
      Key::Right => Some(UnifiedKey::Special(SpecialKey::Right)),
      Key::F1 => Some(UnifiedKey::Special(SpecialKey::F1)),
      Key::F2 => Some(UnifiedKey::Special(SpecialKey::F2)),
      Key::F3 => Some(UnifiedKey::Special(SpecialKey::F3)),
      Key::F4 => Some(UnifiedKey::Special(SpecialKey::F4)),
      Key::F5 => Some(UnifiedKey::Special(SpecialKey::F5)),
      Key::F6 => Some(UnifiedKey::Special(SpecialKey::F6)),
      Key::F7 => Some(UnifiedKey::Special(SpecialKey::F7)),
      Key::F8 => Some(UnifiedKey::Special(SpecialKey::F8)),
      Key::F9 => Some(UnifiedKey::Special(SpecialKey::F9)),
      Key::F10 => Some(UnifiedKey::Special(SpecialKey::F10)),
      Key::F11 => Some(UnifiedKey::Special(SpecialKey::F11)),
      Key::F12 => Some(UnifiedKey::Special(SpecialKey::F12)),
      Key::Char(ch) if key_press.ctrl || key_press.alt || key_press.shift => {
        // Modified character.
        Some(UnifiedKey::Modified {
          key:   ch,
          shift: key_press.shift,
          ctrl:  key_press.ctrl,
          alt:   key_press.alt,
        })
      },
      Key::Char(ch) => {
        // Regular character.
        if self.mode == Mode::Insert || self.pending_char {
          Some(UnifiedKey::Character(ch))
        } else {
          // Let text event handle this in normal mode.
          None
        }
      },
      Key::Other => {
        // Filter out modifier-only events.
        None
      },
    };

    unified
      .map(|key| vec![ProcessedInput::Key(key)])
      .unwrap_or_default()
  }

  fn process_text(&mut self, text: String) -> Vec<ProcessedInput> {
    // Text events are the primary source for characters.
    let mut events = Vec::new();

    for ch in text.chars() {
      // In insert mode or when pending char, all text goes through.
      if self.mode == Mode::Insert || self.pending_char {
        events.push(ProcessedInput::Key(UnifiedKey::Character(ch)));
      } else {
        // In normal/select mode, text events are commands.
        // NOTE: Check if it might be a modified key based on our modifier state.
        if self.ctrl_held || self.alt_held {
          events.push(ProcessedInput::Key(UnifiedKey::Modified {
            key:   ch,
            shift: self.shift_held,
            ctrl:  self.ctrl_held,
            alt:   self.alt_held,
          }));
        } else {
          events.push(ProcessedInput::Key(UnifiedKey::Character(ch)));
        }
      }
    }

    events
  }
}

/// Convert UnifiedKey to KeyBinding for keymap lookups
impl UnifiedKey {
  pub fn to_key_binding(&self) -> Option<KeyBinding> {
    match self {
      UnifiedKey::Character(ch) => Some(KeyBinding::new(Key::Char(*ch))),
      UnifiedKey::Modified {
        key,
        shift,
        ctrl,
        alt,
      } => Some(KeyBinding::new(Key::Char(*key)).with_modifiers(*shift, *ctrl, *alt)),
      UnifiedKey::Escape => Some(KeyBinding::new(Key::Escape)),
      UnifiedKey::Special(special) => {
        let key = match special {
          SpecialKey::Enter => Key::Enter,
          SpecialKey::NumpadEnter => Key::NumpadEnter,
          SpecialKey::Tab => Key::Tab,
          SpecialKey::Backspace => Key::Backspace,
          SpecialKey::Delete => Key::Delete,
          SpecialKey::Insert => Key::Insert,
          SpecialKey::Home => Key::Home,
          SpecialKey::End => Key::End,
          SpecialKey::PageUp => Key::PageUp,
          SpecialKey::PageDown => Key::PageDown,
          SpecialKey::Up => Key::Up,
          SpecialKey::Down => Key::Down,
          SpecialKey::Left => Key::Left,
          SpecialKey::Right => Key::Right,
          SpecialKey::F1 => Key::F1,
          SpecialKey::F2 => Key::F2,
          SpecialKey::F3 => Key::F3,
          SpecialKey::F4 => Key::F4,
          SpecialKey::F5 => Key::F5,
          SpecialKey::F6 => Key::F6,
          SpecialKey::F7 => Key::F7,
          SpecialKey::F8 => Key::F8,
          SpecialKey::F9 => Key::F9,
          SpecialKey::F10 => Key::F10,
          SpecialKey::F11 => Key::F11,
          SpecialKey::F12 => Key::F12,
        };
        Some(KeyBinding::new(key))
      },
      UnifiedKey::ModifiedSpecial {
        key,
        shift,
        ctrl,
        alt,
      } => {
        let key_code = match key {
          SpecialKey::Enter => Key::Enter,
          SpecialKey::NumpadEnter => Key::NumpadEnter,
          SpecialKey::Tab => Key::Tab,
          SpecialKey::Backspace => Key::Backspace,
          SpecialKey::Delete => Key::Delete,
          SpecialKey::Insert => Key::Insert,
          SpecialKey::Home => Key::Home,
          SpecialKey::End => Key::End,
          SpecialKey::PageUp => Key::PageUp,
          SpecialKey::PageDown => Key::PageDown,
          SpecialKey::Up => Key::Up,
          SpecialKey::Down => Key::Down,
          SpecialKey::Left => Key::Left,
          SpecialKey::Right => Key::Right,
          SpecialKey::F1 => Key::F1,
          SpecialKey::F2 => Key::F2,
          SpecialKey::F3 => Key::F3,
          SpecialKey::F4 => Key::F4,
          SpecialKey::F5 => Key::F5,
          SpecialKey::F6 => Key::F6,
          SpecialKey::F7 => Key::F7,
          SpecialKey::F8 => Key::F8,
          SpecialKey::F9 => Key::F9,
          SpecialKey::F10 => Key::F10,
          SpecialKey::F11 => Key::F11,
          SpecialKey::F12 => Key::F12,
        };
        let binding = KeyBinding::new(key_code).with_modifiers(*shift, *ctrl, *alt);
        log::debug!(
          "UnifiedKey::to_key_binding (ModifiedSpecial): key={:?}, shift={}, ctrl={}, alt={}",
          key_code,
          shift,
          ctrl,
          alt
        );
        Some(binding)
      },
    }
  }
}

/// State machine for handling multi-key sequences and callbacks.
pub enum InputState {
  /// Normal input processing.
  Normal,
  /// Insert mode - typing characters.
  Insert,
  /// Select/Visual mode.
  Select,
  // /// Waiting for next key in a sequence.
  // PendingSequence { keys: Vec<KeyBinding> },
  /// Waiting for a character (e.g., replace command)
  PendingChar,
  /// Command mode with active prompt.
  CommandMode,
}

// impl InputState {
//   pub fn is_pending_char(&self) -> bool {
//     matches!(self, InputState::PendingChar)
//   }
// }

/// Main input handler with explicit state management.
pub struct InputHandler {
  processor: InputProcessor,
  state:     InputState,
}

impl InputHandler {
  pub fn new(mode: Mode) -> Self {
    let state = match mode {
      Mode::Normal => InputState::Normal,
      Mode::Insert => InputState::Insert,
      Mode::Select => InputState::Select,
      Mode::Command => InputState::CommandMode,
    };
    Self {
      processor: InputProcessor::new(mode),
      state,
    }
  }

  pub fn set_mode(&mut self, mode: Mode) {
    self.processor.set_mode(mode);
    // Update state based on mode (unless we're waiting for a character).
    if !matches!(self.state, InputState::PendingChar) {
      self.state = match mode {
        Mode::Normal => InputState::Normal,
        Mode::Insert => InputState::Insert,
        Mode::Select => InputState::Select,
        Mode::Command => InputState::CommandMode,
      };
    }
  }

  pub fn set_pending_char(&mut self) {
    self.state = InputState::PendingChar;
    self.processor.set_pending_char(true);
  }

  pub fn clear_pending_char(&mut self) {
    self.state = InputState::Normal;
    self.processor.set_pending_char(false);
  }

  pub fn handle_input(&mut self, event: InputEvent) -> InputResult {
    let processed = self.processor.process(event);

    let mut result = InputResult::default();

    for input in processed {
      match input {
        ProcessedInput::Key(key) => {
          // Handle based on current state.
          match &mut self.state {
            InputState::PendingChar => {
              self.clear_pending_char();

              match key {
                UnifiedKey::Escape => {
                  // Cancel the pending operation.
                  result.cancelled = true;
                },
                UnifiedKey::Character(ch) => {
                  // Return the character for the callback.
                  result.pending_char = Some(ch);
                  result.consumed = true;
                },
                UnifiedKey::Special(SpecialKey::Enter)
                | UnifiedKey::Special(SpecialKey::NumpadEnter) => {
                  // Some commands treat Enter specially.
                  result.pending_char = Some('\n');
                  result.consumed = true;
                },
                _ => {
                  // Other keys cancel.
                  result.cancelled = true;
                },
              }
            },
            // InputState::PendingSequence { keys } => {
            //   if let Some(binding) = key.to_key_binding() {
            //     keys.push(binding);
            //     result.keys = Some(keys.clone());
            //     result.consumed = true;
            //   }
            // },
            InputState::Normal | InputState::Select => {
              if let Some(binding) = key.to_key_binding() {
                result.keys = Some(vec![binding]);
              }
            },
            InputState::CommandMode => {
              // Command mode needs special handling.
              if let Some(binding) = key.to_key_binding() {
                result.command_key = Some(binding);
              }
            },
            InputState::Insert => {
              // In insert mode, characters should be inserted.
              if let UnifiedKey::Character(ch) = key {
                result.insert_char = Some(ch);
              } else if let Some(binding) = key.to_key_binding() {
                result.keys = Some(vec![binding]);
              }
            },
          }
        },
        ProcessedInput::Mouse(mouse) => {
          result.mouse = Some(mouse);
        },
        ProcessedInput::Scroll(delta) => {
          result.scroll = Some(delta);
        },
      }
    }

    result
  }
}

/// Result of input handling.
#[derive(Debug, Default)]
pub struct InputResult {
  /// Keys to be looked up in keymap.
  pub keys:         Option<Vec<KeyBinding>>,
  /// Mouse event to handle.
  pub mouse:        Option<MouseEvent>,
  /// Scroll event to handle.
  pub scroll:       Option<ScrollDelta>,
  /// Character for pending callback (e.g., replace command).
  pub pending_char: Option<char>,
  /// Character to insert in insert mode.
  pub insert_char:  Option<char>,
  /// Key for command mode.
  pub command_key:  Option<KeyBinding>,
  /// Whether the input was consumed by a callback.
  pub consumed:     bool,
  /// Whether a pending operation was cancelled.
  pub cancelled:    bool,
}

#[cfg(test)]
mod tests {
  use the_editor_renderer::{
    InputEvent,
    Key,
    KeyPress,
    MouseButton,
  };

  use super::*;

  fn create_key_press(code: Key, shift: bool, ctrl: bool, alt: bool) -> KeyPress {
    KeyPress {
      code,
      pressed: true,
      shift,
      ctrl,
      alt,
      super_: false,
    }
  }

  #[test]
  fn test_modifier_only_events_ignored() {
    let mut handler = InputHandler::new(Mode::Normal);

    // Modifier-only events (Key::Other) should be ignored.
    let event = InputEvent::Keyboard(create_key_press(Key::Other, true, false, false));
    let result = handler.handle_input(event);

    assert!(result.keys.is_none());
    assert!(!result.consumed);
  }

  #[test]
  fn test_capital_letters_in_normal_mode() {
    let mut handler = InputHandler::new(Mode::Normal);

    // Capital 'B' should come through as text event.
    let event = InputEvent::Text("B".to_string());
    let result = handler.handle_input(event);

    // Should produce a key binding for 'B'.
    assert!(result.keys.is_some());
    let keys = result.keys.unwrap();
    assert_eq!(keys.len(), 1);
    assert_eq!(keys[0].code, Key::Char('B'));
  }

  #[test]
  fn test_replace_command_with_capital() {
    let mut handler = InputHandler::new(Mode::Normal);

    // Simulate 'r' command setting up pending char.
    handler.set_pending_char();

    // Then capital 'B' as text event.
    let event = InputEvent::Text("B".to_string());
    let result = handler.handle_input(event);

    // Should return the character for the callback.
    assert_eq!(result.pending_char, Some('B'));
    assert!(result.consumed);
    assert!(!result.cancelled);
  }

  #[test]
  fn test_escape_cancels_pending_char() {
    let mut handler = InputHandler::new(Mode::Normal);

    // Set pending char (like after 'r' command).
    handler.set_pending_char();

    // Escape should cancel.
    let event = InputEvent::Keyboard(create_key_press(Key::Escape, false, false, false));
    let result = handler.handle_input(event);

    assert!(result.cancelled);
    assert!(result.pending_char.is_none());
  }

  #[test]
  fn test_insert_mode_character_insertion() {
    let mut handler = InputHandler::new(Mode::Insert);

    // Regular character in insert mode.
    let event = InputEvent::Text("a".to_string());
    let result = handler.handle_input(event);

    // Should produce insert_char.
    assert_eq!(result.insert_char, Some('a'));
    assert!(result.keys.is_none());
  }

  #[test]
  fn test_insert_mode_special_keys() {
    let mut handler = InputHandler::new(Mode::Insert);

    // Escape in insert mode should produce key binding.
    let event = InputEvent::Keyboard(create_key_press(Key::Escape, false, false, false));
    let result = handler.handle_input(event);

    assert!(result.keys.is_some());
    let keys = result.keys.unwrap();
    assert_eq!(keys[0].code, Key::Escape);
    assert!(result.insert_char.is_none());
  }

  #[test]
  fn test_mode_switching() {
    let mut handler = InputHandler::new(Mode::Normal);

    // Switch to insert mode.
    handler.set_mode(Mode::Insert);

    // Character should be for insertion.
    let event = InputEvent::Text("x".to_string());
    let result = handler.handle_input(event);
    assert_eq!(result.insert_char, Some('x'));

    // Switch back to normal.
    handler.set_mode(Mode::Normal);

    // Character should be for command.
    let event = InputEvent::Text("x".to_string());
    let result = handler.handle_input(event);
    assert!(result.keys.is_some());
    assert!(result.insert_char.is_none());
  }

  #[test]
  fn test_command_mode_keys() {
    let mut handler = InputHandler::new(Mode::Command);

    // Keys in command mode should go to command_key.
    let event = InputEvent::Text("q".to_string());
    let result = handler.handle_input(event);

    assert!(result.command_key.is_some());
    assert_eq!(result.command_key.unwrap().code, Key::Char('q'));
  }

  #[test]
  fn test_space_with_shift_modifier() {
    let mut handler = InputHandler::new(Mode::Normal);

    // Space + Shift (like for replace_selections_with_clipboard).
    let event = InputEvent::Keyboard(create_key_press(Key::Char(' '), true, false, false));
    let result = handler.handle_input(event);

    // Should produce a key binding with shift.
    assert!(result.keys.is_some());
    let keys = result.keys.unwrap();
    assert_eq!(keys[0].code, Key::Char(' '));
    assert!(keys[0].shift);
  }

  #[test]
  fn test_ctrl_modified_keys() {
    let mut handler = InputHandler::new(Mode::Normal);

    // Ctrl+A should be handled as modified key.
    let event = InputEvent::Keyboard(create_key_press(Key::Char('a'), false, true, false));
    let result = handler.handle_input(event);

    assert!(result.keys.is_some());
    let keys = result.keys.unwrap();
    assert_eq!(keys[0].code, Key::Char('a'));
    assert!(keys[0].ctrl);
  }

  #[test]
  fn test_dead_key_filtering() {
    let mut processor = InputProcessor::new(Mode::Normal);

    // Dead keys (Key::Other) should be filtered.
    let event = InputEvent::Keyboard(create_key_press(Key::Other, false, false, false));
    let processed = processor.process(event);

    assert!(processed.is_empty());
  }

  #[test]
  fn test_text_event_in_normal_mode_with_pending() {
    let mut handler = InputHandler::new(Mode::Normal);
    handler.set_pending_char();

    // Text event with pending char should return the character.
    let event = InputEvent::Text("'".to_string());
    let result = handler.handle_input(event);

    assert_eq!(result.pending_char, Some('\''));
    assert!(result.consumed);
  }

  #[test]
  fn test_enter_as_newline_for_replace() {
    let mut handler = InputHandler::new(Mode::Normal);
    handler.set_pending_char();

    // Enter should produce newline for replace command.
    let event = InputEvent::Keyboard(create_key_press(Key::Enter, false, false, false));
    let result = handler.handle_input(event);

    assert_eq!(result.pending_char, Some('\n'));
    assert!(result.consumed);
  }

  #[test]
  fn test_scroll_event_passthrough() {
    let mut handler = InputHandler::new(Mode::Normal);

    let event = InputEvent::Scroll(ScrollDelta::Lines { x: 0.0, y: -3.0 });
    let result = handler.handle_input(event);

    assert!(result.scroll.is_some());
    if let Some(ScrollDelta::Lines { y, .. }) = result.scroll {
      assert_eq!(y, -3.0);
    }
  }

  #[test]
  fn test_mouse_event_passthrough() {
    let mut handler = InputHandler::new(Mode::Normal);

    let mouse = MouseEvent {
      position: (100.0, 200.0),
      button:   Some(MouseButton::Left),
      pressed:  true,
    };
    let event = InputEvent::Mouse(mouse.clone());
    let result = handler.handle_input(event);

    assert!(result.mouse.is_some());
    let result_mouse = result.mouse.unwrap();
    assert_eq!(result_mouse.position, (100.0, 200.0));
  }

  #[test]
  fn test_alt_backspace_in_insert_mode() {
    let mut handler = InputHandler::new(Mode::Insert);

    // Alt+Backspace should produce a key binding with alt modifier.
    let event = InputEvent::Keyboard(create_key_press(Key::Backspace, false, false, true));
    let result = handler.handle_input(event);

    assert!(result.keys.is_some());
    let keys = result.keys.unwrap();
    assert_eq!(keys[0].code, Key::Backspace);
    assert!(keys[0].alt);
    assert!(!keys[0].ctrl);
    assert!(!keys[0].shift);
  }

  #[test]
  fn test_ctrl_w_in_insert_mode() {
    let mut handler = InputHandler::new(Mode::Insert);

    // Ctrl+W should also produce a key binding with ctrl modifier.
    let event = InputEvent::Keyboard(create_key_press(Key::Char('w'), false, true, false));
    let result = handler.handle_input(event);

    assert!(result.keys.is_some());
    let keys = result.keys.unwrap();
    assert_eq!(keys[0].code, Key::Char('w'));
    assert!(keys[0].ctrl);
    assert!(!keys[0].alt);
    assert!(!keys[0].shift);
  }
}
