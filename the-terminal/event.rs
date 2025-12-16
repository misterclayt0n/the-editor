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

#[cfg(test)]
mod tests {
  use super::*;
  use crate::TerminalId;
  use std::num::NonZeroUsize;

  fn make_id(n: usize) -> TerminalId {
    TerminalId(NonZeroUsize::new(n).unwrap())
  }

  #[test]
  fn test_terminal_event_wakeup() {
    let id = make_id(1);
    let event = TerminalEvent::Wakeup(id);

    match event {
      TerminalEvent::Wakeup(eid) => assert_eq!(eid, id),
      _ => panic!("Expected Wakeup event"),
    }
  }

  #[test]
  fn test_terminal_event_title() {
    let id = make_id(1);
    let event = TerminalEvent::Title { id, title: "MyTerminal".to_string() };

    match event {
      TerminalEvent::Title { id: eid, title } => {
        assert_eq!(eid, id);
        assert_eq!(title, "MyTerminal");
      }
      _ => panic!("Expected Title event"),
    }
  }

  #[test]
  fn test_terminal_event_bell() {
    let id = make_id(2);
    let event = TerminalEvent::Bell(id);

    match event {
      TerminalEvent::Bell(eid) => assert_eq!(eid, id),
      _ => panic!("Expected Bell event"),
    }
  }

  #[test]
  fn test_terminal_event_exit_with_status() {
    let id = make_id(1);
    let event = TerminalEvent::Exit { id, status: Some(0) };

    match event {
      TerminalEvent::Exit { id: eid, status } => {
        assert_eq!(eid, id);
        assert_eq!(status, Some(0));
      }
      _ => panic!("Expected Exit event"),
    }
  }

  #[test]
  fn test_terminal_event_exit_no_status() {
    let id = make_id(1);
    let event = TerminalEvent::Exit { id, status: None };

    match event {
      TerminalEvent::Exit { id: eid, status } => {
        assert_eq!(eid, id);
        assert_eq!(status, None);
      }
      _ => panic!("Expected Exit event"),
    }
  }

  #[test]
  fn test_terminal_event_clipboard_store() {
    let id = make_id(1);
    let event = TerminalEvent::ClipboardStore { id, content: "copied text".to_string() };

    match event {
      TerminalEvent::ClipboardStore { id: eid, content } => {
        assert_eq!(eid, id);
        assert_eq!(content, "copied text");
      }
      _ => panic!("Expected ClipboardStore event"),
    }
  }

  #[test]
  fn test_terminal_event_clipboard_load() {
    let id = make_id(1);
    let event = TerminalEvent::ClipboardLoad { id };

    match event {
      TerminalEvent::ClipboardLoad { id: eid } => assert_eq!(eid, id),
      _ => panic!("Expected ClipboardLoad event"),
    }
  }

  #[test]
  fn test_mouse_cursor_shapes_distinct() {
    assert_ne!(MouseCursorShape::Default, MouseCursorShape::Text);
    assert_ne!(MouseCursorShape::Default, MouseCursorShape::Pointer);
    assert_ne!(MouseCursorShape::Text, MouseCursorShape::Pointer);
  }

  #[test]
  fn test_terminal_event_cursor_visibility() {
    let id = make_id(1);
    let event = TerminalEvent::CursorVisibility { id, visible: true };

    match event {
      TerminalEvent::CursorVisibility { id: eid, visible } => {
        assert_eq!(eid, id);
        assert!(visible);
      }
      _ => panic!("Expected CursorVisibility event"),
    }
  }

  #[test]
  fn test_terminal_event_mouse_cursor_shape() {
    let id = make_id(1);
    let event = TerminalEvent::MouseCursorShape { id, shape: MouseCursorShape::Pointer };

    match event {
      TerminalEvent::MouseCursorShape { id: eid, shape } => {
        assert_eq!(eid, id);
        assert_eq!(shape, MouseCursorShape::Pointer);
      }
      _ => panic!("Expected MouseCursorShape event"),
    }
  }
}
