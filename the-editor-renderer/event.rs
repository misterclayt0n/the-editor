//! Input event types and handling
//!
//! This module defines the input events that can be handled by applications.

/// Keyboard key codes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
  /// Escape key
  Escape,
  /// Enter/Return key
  Enter,
  /// Space bar
  Space,
  /// Backspace key
  Backspace,
  /// Up arrow key
  Up,
  /// Down arrow key
  Down,
  /// Left arrow key
  Left,
  /// Right arrow key
  Right,
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

/// Input event types that can be handled by the application
#[derive(Debug, Clone)]
pub enum InputEvent {
  /// Keyboard key press or release
  Keyboard(KeyPress),
  /// Mouse button or motion event
  Mouse(MouseEvent),
  /// Text input (for typing)
  Text(String),
}
