//! Terminal events for communication between PTY I/O and the editor.

use crate::TerminalId;

/// Events emitted by terminal instances.
#[derive(Debug, Clone)]
pub enum TerminalEvent {
  /// Terminal output changed, needs redraw.
  Wakeup(TerminalId),

  /// Terminal title changed (via OSC escape sequence).
  Title {
    id:    TerminalId,
    title: String,
  },

  /// Bell character received.
  Bell(TerminalId),

  /// Child process exited.
  Exit {
    id:     TerminalId,
    status: Option<i32>,
  },

  /// Terminal requests clipboard content (paste).
  ClipboardLoad {
    id: TerminalId,
  },

  /// Terminal provides clipboard content (copy).
  ClipboardStore {
    id:      TerminalId,
    content: String,
  },

  /// Cursor visibility changed.
  CursorVisibility {
    id:      TerminalId,
    visible: bool,
  },

  /// Mouse cursor shape changed.
  MouseCursorShape {
    id:    TerminalId,
    shape: MouseCursorShape,
  },
}

/// Mouse cursor shapes that the terminal can request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseCursorShape {
  Default,
  Text,
  Pointer,
}
