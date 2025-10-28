//! Terminal view component for displaying PTY shells in the editor
//!
//! This component integrates the terminal emulator from `the-terminal` crate
//! into the editor's UI system, allowing users to spawn and interact with
//! shell processes.
//!
//! # Architecture
//!
//! TerminalView implements the Component trait and can be managed by the
//! Compositor. It wraps a TerminalSession which combines:
//! - Terminal (VT100 emulation from ghostty-vt via FFI)
//! - PtySession (process management and I/O)
//!
//! # Integration Points
//!
//! To fully integrate terminals into the editor:
//! 1. Add a TerminalManager to the App struct
//! 2. Create keybindings to spawn/switch terminals
//! 3. Add terminal to compositor with proper layout
//! 4. Implement terminal input mode for better UX
//!
//! # Current Limitations
//!
//! - TerminalView is not yet integrated into the main compositor
//! - No command system for spawning terminals
//! - No way to switch between multiple terminals
//! - Terminal rendering is basic (no styling/colors yet)

use std::cell::RefCell;

use the_editor_renderer::{
  Color,
  TextSection,
  TextSegment,
};
use the_terminal::TerminalSession;

use crate::{
  core::{
    graphics::{
      CursorKind,
      Rect,
    },
    position::Position,
  },
  editor::Editor,
  keymap::KeyBinding,
  ui::compositor::{
    Component,
    Context,
    Event,
    EventResult,
    Surface,
  },
};

/// A terminal view component that displays and manages a PTY session
pub struct TerminalView {
  /// The PTY terminal session (wrapped in RefCell for interior mutability)
  session: RefCell<TerminalSession>,

  /// Unique identifier for this terminal
  id: u32,

  /// Whether this terminal needs redrawing
  dirty: bool,

  /// Cache for last rendered dimensions
  last_cols: u16,
  last_rows: u16,
}

impl TerminalView {
  /// Create a new terminal view with specified dimensions
  ///
  /// # Arguments
  /// * `cols` - Terminal width in columns
  /// * `rows` - Terminal height in rows
  /// * `shell` - Shell to execute (None uses $SHELL or /bin/bash)
  /// * `id` - Unique identifier for this terminal
  ///
  /// # Errors
  /// Returns an error if terminal session cannot be created.
  pub fn new(cols: u16, rows: u16, shell: Option<&str>, id: u32) -> anyhow::Result<Self> {
    let session = TerminalSession::new(rows, cols, shell)?;

    Ok(Self {
      session: RefCell::new(session),
      id,
      dirty: true,
      last_cols: cols,
      last_rows: rows,
    })
  }

  /// Send input to the terminal (keyboard)
  pub fn send_input(&self, bytes: Vec<u8>) -> anyhow::Result<()> {
    self.session.borrow().send_input(bytes)
  }

  /// Check if the terminal shell is still alive
  pub fn is_alive(&self) -> bool {
    self.session.borrow_mut().is_alive()
  }

  /// Get the terminal's unique ID
  pub fn id(&self) -> u32 {
    self.id
  }

  /// Get current terminal dimensions
  pub fn size(&self) -> (u16, u16) {
    self.session.borrow().size()
  }

  /// Convert a KeyBinding to bytes for PTY
  ///
  /// This handles special keys and VT100 escape sequences.
  fn key_to_bytes(key: &KeyBinding) -> Vec<u8> {
    use the_editor_renderer::Key;

    let key_code = key.code;
    let ctrl = key.ctrl;
    let alt = key.alt;
    let shift = key.shift;

    // Handle special keys with escape sequences
    match key_code {
      Key::Up => {
        if ctrl {
          b"\x1b[1;5A".to_vec()
        } else if alt {
          b"\x1b[1;3A".to_vec()
        } else {
          b"\x1b[A".to_vec()
        }
      },
      Key::Down => {
        if ctrl {
          b"\x1b[1;5B".to_vec()
        } else if alt {
          b"\x1b[1;3B".to_vec()
        } else {
          b"\x1b[B".to_vec()
        }
      },
      Key::Left => {
        if ctrl {
          b"\x1b[1;5D".to_vec()
        } else if alt {
          b"\x1b[1;3D".to_vec()
        } else {
          b"\x1b[D".to_vec()
        }
      },
      Key::Right => {
        if ctrl {
          b"\x1b[1;5C".to_vec()
        } else if alt {
          b"\x1b[1;3C".to_vec()
        } else {
          b"\x1b[C".to_vec()
        }
      },
      Key::Home => b"\x1b[H".to_vec(),
      Key::End => b"\x1b[F".to_vec(),
      Key::PageUp => b"\x1b[5~".to_vec(),
      Key::PageDown => b"\x1b[6~".to_vec(),
      Key::Tab => {
        if shift {
          b"\x1b[Z".to_vec() // Shift+Tab
        } else {
          b"\t".to_vec()
        }
      },
      Key::Backspace => b"\x7f".to_vec(),
      Key::Delete => b"\x1b[3~".to_vec(),
      Key::Enter => b"\r".to_vec(),
      Key::Escape => b"\x1b".to_vec(),
      Key::Char(c) => {
        let mut bytes = Vec::new();

        if ctrl {
          // Ctrl+key produces control character
          match c {
            'a'..='z' => bytes.push((c as u8) - b'a' + 1),
            'A'..='Z' => bytes.push((c as u8) - b'A' + 1),
            '[' => bytes.push(0x1B),
            _ => bytes.extend_from_slice(c.to_string().as_bytes()),
          }
        } else if alt {
          // Alt+key produces ESC key
          bytes.push(0x1B);
          bytes.extend_from_slice(c.to_string().as_bytes());
        } else {
          bytes.extend_from_slice(c.to_string().as_bytes());
        }

        bytes
      },
      _ => Vec::new(), // Other keys ignored for now
    }
  }
}

impl Component for TerminalView {
  fn handle_event(&mut self, event: &Event, _ctx: &mut Context) -> EventResult {
    match event {
      Event::Key(key) => {
        // Mark as dirty so we redraw
        self.dirty = true;

        // Convert key to bytes and send to PTY
        let bytes = Self::key_to_bytes(key);
        if !bytes.is_empty() {
          if let Err(e) = self.send_input(bytes) {
            log::error!("Failed to send input to terminal: {}", e);
          }
        }

        EventResult::Consumed(None)
      },
      _ => EventResult::Ignored(None),
    }
  }

  fn should_update(&self) -> bool {
    // Always redraw while the shell is alive (PTY output may arrive at any time)
    // This ensures we continuously poll for output and render it
    self.is_alive() || self.dirty
  }

  fn render(&mut self, area: Rect, surface: &mut Surface, _ctx: &mut Context) {
    // Update terminal with any pending PTY output
    self.session.borrow_mut().update();
    self.dirty = false;

    // Get font metrics from renderer
    let cell_width = surface.cell_width();
    let cell_height = surface.cell_height();

    // Calculate terminal dimensions based on available area
    let new_cols = (area.width as f32 / cell_width).floor() as u16;
    let new_rows = (area.height as f32 / cell_height).floor() as u16;

    // Resize terminal if dimensions changed
    if new_cols != self.last_cols || new_rows != self.last_rows {
      if new_cols > 0 && new_rows > 0 {
        if let Err(e) = self.session.borrow_mut().resize(new_rows, new_cols) {
          log::error!("Failed to resize terminal: {}", e);
        } else {
          self.last_cols = new_cols;
          self.last_rows = new_rows;
        }
      }
    }

    let session_borrow = self.session.borrow();
    let grid = session_borrow.terminal().grid();
    let (term_rows, term_cols) = (grid.rows(), grid.cols());

    // Render grid cells
    // Clamp rendering to the calculated dimensions to avoid overflow
    // if the terminal hasn't resized yet
    let render_rows = term_rows.min(new_rows);
    let render_cols = term_cols.min(new_cols);

    // Batch contiguous runs of identical color within each row.
    let line_height = cell_height;

    for row in 0..render_rows {
      let row_y = area.y as f32 + (row as f32 * line_height);
      let mut run_text = String::with_capacity(render_cols as usize);
      let mut run_color: Option<(u8, u8, u8)> = None;
      let mut run_start_col = 0u16;

      let flush_run = |surface: &mut Surface,
                       start_col: u16,
                       color: Option<(u8, u8, u8)>,
                       buffer: &mut String| {
        if buffer.is_empty() {
          return;
        }
        while buffer.ends_with(' ') {
          buffer.pop();
        }
        if buffer.is_empty() {
          return;
        }
        let Some((r, g, b)) = color else {
          buffer.clear();
          return;
        };

        let fg_color = Color::rgba(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0);
        let text = std::mem::take(buffer);
        let x = area.x as f32 + (start_col as f32 * cell_width);
        let mut section = TextSection::new(x, row_y);
        section = section.add_text(
          TextSegment::new(text)
            .with_color(fg_color)
            .with_size(cell_height),
        );
        surface.draw_text(section);
        buffer.reserve(render_cols as usize);
      };

      for col in 0..render_cols {
        let cell = grid.get(row, col);
        let mut ch = cell.character().unwrap_or(' ');
        if ch == '\0' {
          ch = ' ';
        }

        let rgb = (cell.fg.r, cell.fg.g, cell.fg.b);

        if run_color.map(|current| current != rgb).unwrap_or(true) {
          flush_run(surface, run_start_col, run_color, &mut run_text);
          run_color = Some(rgb);
          run_start_col = col;
        }

        let cell_width = cell.width;
        let is_wide_continuation = cell_width == 0;
        if is_wide_continuation {
          continue;
        }

        run_text.push(ch);

        let glyph_width = usize::from(cell_width.max(1));
        if glyph_width > 1 {
          for _ in 1..glyph_width {
            run_text.push(' ');
          }
        }
      }

      flush_run(surface, run_start_col, run_color, &mut run_text);
    }

    // Render cursor if visible
    let (cursor_row, cursor_col) = session_borrow.terminal().cursor_pos();
    if cursor_row < term_rows && cursor_col < term_cols {
      let cursor_x = area.x as f32 + (cursor_col as f32 * cell_width);
      let cursor_y = area.y as f32 + (cursor_row as f32 * cell_height);
      let cursor_width = cell_width;
      let cursor_height = cell_height;

      // Draw cursor as a semi-transparent rectangle
      surface.draw_rect(
        cursor_x,
        cursor_y,
        cursor_width,
        cursor_height,
        Color::new(0.8, 0.8, 0.8, 0.5),
      );
    }
  }

  fn cursor(&self, area: Rect, _ctx: &Editor) -> (Option<Position>, CursorKind) {
    let session_borrow = self.session.borrow();
    let (cursor_row, cursor_col) = session_borrow.terminal().cursor_pos();

    if cursor_row < area.height && cursor_col < area.width {
      let pos = Position::new(
        area.y as usize + cursor_row as usize,
        area.x as usize + cursor_col as usize,
      );
      (Some(pos), CursorKind::Block)
    } else {
      (None, CursorKind::Hidden)
    }
  }

  fn required_size(&mut self, _viewport: (u16, u16)) -> Option<(u16, u16)> {
    // Return None to indicate we can fill any size
    // The terminal will dynamically resize based on the area given in render()
    None
  }

  fn type_name(&self) -> &'static str {
    "TerminalView"
  }

  fn id(&self) -> Option<&'static str> {
    None
  }

  fn is_animating(&self) -> bool {
    false // Terminal doesn't animate, but PTY may produce output
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[tokio::test]
  async fn test_terminal_view_creation() {
    // Note: This will fail without a shell, but shows the API
    // Actual tests should use integration tests with proper environment
    let _result = TerminalView::new(80, 24, None, 0);
    // Result depends on system shell availability
  }

  #[test]
  fn test_key_to_bytes_arrow_keys() {
    use the_editor_renderer::Key;

    let key = KeyBinding {
      code:  Key::Up,
      shift: false,
      ctrl:  false,
      alt:   false,
    };
    let bytes = TerminalView::key_to_bytes(&key);
    assert_eq!(bytes, b"\x1b[A");

    let key = KeyBinding {
      code:  Key::Down,
      shift: false,
      ctrl:  false,
      alt:   false,
    };
    let bytes = TerminalView::key_to_bytes(&key);
    assert_eq!(bytes, b"\x1b[B");
  }

  #[test]
  fn test_key_to_bytes_ctrl_c() {
    use the_editor_renderer::Key;

    let key = KeyBinding {
      code:  Key::Char('c'),
      shift: false,
      ctrl:  true,
      alt:   false,
    };
    let bytes = TerminalView::key_to_bytes(&key);
    assert_eq!(bytes, vec![3]); // Ctrl+C = 0x03
  }

  #[test]
  fn test_key_to_bytes_regular_char() {
    use the_editor_renderer::Key;

    let key = KeyBinding {
      code:  Key::Char('a'),
      shift: false,
      ctrl:  false,
      alt:   false,
    };
    let bytes = TerminalView::key_to_bytes(&key);
    assert_eq!(bytes, b"a");
  }
}
