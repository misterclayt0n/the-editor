//! Input event types and handling
//!
//! This module defines the input events that can be handled by applications.

/// Keyboard key codes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Key {
  /// A character-producing key (single Unicode scalar)
  Char(char),
  /// Escape key
  Escape,
  /// Enter/Return key
  Enter,
  /// Keypad Enter key
  NumpadEnter,
  /// Backspace key
  Backspace,
  /// Tab key
  Tab,
  /// Delete key
  Delete,
  /// Home key
  Home,
  /// End key
  End,
  /// Page up key
  PageUp,
  /// Page down key
  PageDown,
  /// Up arrow key
  Up,
  /// Down arrow key
  Down,
  /// Left arrow key
  Left,
  /// Right arrow key
  Right,
  /// Insert key
  Insert,
  /// Function key F1
  F1,
  /// Function key F2
  F2,
  /// Function key F3
  F3,
  /// Function key F4
  F4,
  /// Function key F5
  F5,
  /// Function key F6
  F6,
  /// Function key F7
  F7,
  /// Function key F8
  F8,
  /// Function key F9
  F9,
  /// Function key F10
  F10,
  /// Function key F11
  F11,
  /// Function key F12
  F12,
  /// Any other key not specifically handled
  Other,
}

/// Keyboard press event with modifiers
#[derive(Debug, Clone)]
pub struct KeyPress {
  /// The key that was pressed or released
  pub code:    Key,
  /// True if the key was pressed, false if released
  pub pressed: bool,
  /// True if Shift was held during the event
  pub shift:   bool,
  /// True if Ctrl/Cmd was held during the event
  pub ctrl:    bool,
  /// True if Alt/Option was held during the event
  pub alt:     bool,
  /// True if Super/Win/Cmd was held during the event
  pub super_:  bool,
}

/// Mouse button identifiers
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
  /// Left mouse button
  Left,
  /// Right mouse button
  Right,
  /// Middle mouse button (scroll wheel click)
  Middle,
}

/// Mouse input event
#[derive(Debug, Clone)]
pub struct MouseEvent {
  /// Cursor position in window coordinates (x, y)
  pub position: (f32, f32),
  /// Button involved in the event (None for motion events)
  pub button:   Option<MouseButton>,
  /// True if the button was pressed, false if released
  pub pressed:  bool,
}

/// Mouse scroll delta reported by the windowing backend.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ScrollDelta {
  /// Scroll expressed in logical lines.
  Lines { x: f32, y: f32 },
  /// Scroll expressed in physical pixels.
  Pixels { x: f32, y: f32 },
}

/// Input event types that can be handled by the application
#[derive(Debug, Clone)]
pub enum InputEvent {
  /// Keyboard key press or release
  Keyboard(KeyPress),
  /// Mouse button or motion event
  Mouse(MouseEvent),
  /// Text input (for typing)
  Text(String),
  /// Mouse wheel or trackpad scrolling
  Scroll(ScrollDelta),
}
