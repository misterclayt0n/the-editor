//! Terminal emulation wrapper around alacritty_terminal.

use std::sync::{
  Arc,
  OnceLock,
};

use alacritty_terminal::{
  event::{
    Event as AlacrittyEvent,
    EventListener,
    WindowSize,
  },
  event_loop::{
    EventLoop,
    EventLoopSender,
    Msg,
  },
  index::{
    Column,
    Direction as AlacDirection,
    Line,
    Point as AlacPoint,
  },
  selection::{
    Selection,
    SelectionType,
  },
  sync::FairMutex,
  term::{
    Config as TermConfig,
    Term,
    TermMode,
    test::TermSize,
  },
  tty::{
    self,
    Options as PtyOptions,
  },
};
use tokio::sync::mpsc;

use crate::{
  TerminalConfig,
  TerminalEvent,
  TerminalId,
  renderer::{
    ColorScheme,
    CursorInfo,
    CursorShape,
    RenderCell,
    extract_cells,
  },
};

/// Event proxy that forwards alacritty events to our channel.
struct EventProxy {
  id:                TerminalId,
  sender:            mpsc::UnboundedSender<TerminalEvent>,
  event_loop_sender: Arc<OnceLock<EventLoopSender>>,
}

impl EventListener for EventProxy {
  fn send_event(&self, event: AlacrittyEvent) {
    let terminal_event = match event {
      AlacrittyEvent::Wakeup => TerminalEvent::Wakeup(self.id),
      AlacrittyEvent::Bell => TerminalEvent::Bell(self.id),
      AlacrittyEvent::Exit => TerminalEvent::Exit { id: self.id, status: None },
      AlacrittyEvent::Title(title) => TerminalEvent::Title { id: self.id, title },
      AlacrittyEvent::ClipboardLoad(_, _) => TerminalEvent::ClipboardLoad { id: self.id },
      AlacrittyEvent::ClipboardStore(_, content) => {
        TerminalEvent::ClipboardStore { id: self.id, content }
      }
      AlacrittyEvent::CursorBlinkingChange => return,
      AlacrittyEvent::MouseCursorDirty => return,
      AlacrittyEvent::ResetTitle => {
        TerminalEvent::Title { id: self.id, title: String::new() }
      }
      AlacrittyEvent::TextAreaSizeRequest(_) => return,
      AlacrittyEvent::ColorRequest(_, _) => return,
      AlacrittyEvent::PtyWrite(text) => {
        // Write response back to PTY (for terminal queries like device attributes)
        if let Some(sender) = self.event_loop_sender.get() {
          let _ = sender.send(Msg::Input(text.into_bytes().into()));
        }
        return;
      }
      AlacrittyEvent::ChildExit(status) => {
        TerminalEvent::Exit { id: self.id, status: Some(status) }
      }
    };

    let _ = self.sender.send(terminal_event);
  }
}

/// A terminal instance.
pub struct Terminal {
  /// Unique identifier.
  pub id: TerminalId,

  /// The terminal emulator state (shared with event loop).
  term: Arc<FairMutex<Term<EventProxy>>>,

  /// Sender to communicate with the PTY event loop.
  event_loop_sender: EventLoopSender,

  /// Current title (set via OSC escape sequences).
  title: String,

  /// Whether the child process has exited.
  exited: bool,

  /// Exit status if exited.
  exit_status: Option<i32>,

  /// Terminal dimensions in cells.
  cols: u16,
  rows: u16,

  /// Configuration.
  #[allow(dead_code)]
  config: TerminalConfig,
}

impl Terminal {
  /// Create and spawn a new terminal.
  pub fn spawn(
    id: TerminalId,
    cols: u16,
    rows: u16,
    cell_width: u16,
    cell_height: u16,
    config: TerminalConfig,
    event_sender: mpsc::UnboundedSender<TerminalEvent>,
  ) -> anyhow::Result<Self> {
    // Create shared OnceLock for event_loop_sender (set after EventLoop creation)
    let event_loop_sender_cell = Arc::new(OnceLock::new());

    let event_proxy = EventProxy {
      id,
      sender:            event_sender.clone(),
      event_loop_sender: Arc::clone(&event_loop_sender_cell),
    };

    // Determine shell
    let shell = config.shell.clone().unwrap_or_else(|| {
      std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string())
    });

    // Create terminal config
    let term_config = TermConfig::default();

    // Create window size with actual cell dimensions for proper PTY sizing
    let window_size = WindowSize {
      num_cols: cols,
      num_lines: rows,
      cell_width,
      cell_height,
    };

    // Create PTY options
    // Pass -i flag for interactive mode - most shells need this to work properly
    // in a pseudo-terminal context (bash, zsh, fish, nu all support -i)
    let pty_config = PtyOptions {
      shell:             Some(tty::Shell::new(shell, vec!["-i".to_string()])),
      working_directory: config.working_directory.clone(),
      ..Default::default()
    };

    // Create the PTY
    let pty = tty::new(&pty_config, window_size, id.0.get() as u64)?;

    // Create the terminal
    let term = Term::new(term_config, &TermSize::new(cols as usize, rows as usize), event_proxy);
    let term = Arc::new(FairMutex::new(term));

    // Create the event loop
    let event_loop = EventLoop::new(
      Arc::clone(&term),
      EventProxy {
        id,
        sender:            event_sender.clone(),
        event_loop_sender: Arc::clone(&event_loop_sender_cell),
      },
      pty,
      false, // hold
      false, // ref_test
    )?;

    // Start the event loop
    let event_loop_sender = event_loop.channel();
    // Set the sender in the OnceLock so EventProxy can use it for PtyWrite responses
    let _ = event_loop_sender_cell.set(event_loop_sender.clone());
    let _handle = event_loop.spawn();

    Ok(Self {
      id,
      term,
      event_loop_sender,
      title: String::new(),
      exited: false,
      exit_status: None,
      cols,
      rows,
      config,
    })
  }

  /// Get the terminal title.
  pub fn title(&self) -> &str {
    if self.title.is_empty() {
      "Terminal"
    } else {
      &self.title
    }
  }

  /// Set the terminal title.
  pub fn set_title(&mut self, title: String) {
    self.title = title;
  }

  /// Check if the terminal process has exited.
  pub fn is_exited(&self) -> bool {
    self.exited
  }

  /// Mark the terminal as exited.
  pub fn mark_exited(&mut self, status: Option<i32>) {
    self.exited = true;
    self.exit_status = status;
  }

  /// Get the exit status if exited.
  pub fn exit_status(&self) -> Option<i32> {
    self.exit_status
  }

  /// Resize the terminal.
  pub fn resize(&mut self, cols: u16, rows: u16, cell_width: u16, cell_height: u16) {
    if cols == self.cols && rows == self.rows {
      return;
    }

    self.cols = cols;
    self.rows = rows;

    let size = WindowSize {
      num_cols: cols,
      num_lines: rows,
      cell_width,
      cell_height,
    };

    // Resize terminal state
    {
      let mut term = self.term.lock();
      term.resize(TermSize::new(cols as usize, rows as usize));
    }

    // Notify PTY event loop of resize
    let _ = self.event_loop_sender.send(Msg::Resize(size));
  }

  /// Write input bytes to the terminal.
  pub fn write(&self, data: &[u8]) {
    let _ = self.event_loop_sender.send(Msg::Input(data.to_vec().into()));
  }

  /// Write a string to the terminal.
  pub fn write_str(&self, s: &str) {
    self.write(s.as_bytes());
  }

  /// Get terminal dimensions.
  pub fn dimensions(&self) -> (u16, u16) {
    (self.cols, self.rows)
  }

  /// Extract renderable cells for the current terminal state.
  pub fn render_cells(&self, colors: &ColorScheme) -> Vec<RenderCell> {
    let term = self.term.lock();
    let content = term.renderable_content();
    extract_cells(content, colors, self.cols as usize, self.rows as usize)
  }

  /// Get cursor information.
  pub fn cursor_info(&self) -> CursorInfo {
    let term = self.term.lock();
    let content = term.renderable_content();

    // Handle hidden cursor - alacritty sets shape to Hidden when SHOW_CURSOR mode is off
    let (shape, visible) = match content.cursor.shape {
      alacritty_terminal::vte::ansi::CursorShape::Block => (CursorShape::Block, true),
      alacritty_terminal::vte::ansi::CursorShape::Underline => (CursorShape::Underline, true),
      alacritty_terminal::vte::ansi::CursorShape::Beam => (CursorShape::Beam, true),
      alacritty_terminal::vte::ansi::CursorShape::HollowBlock => (CursorShape::Block, true),
      alacritty_terminal::vte::ansi::CursorShape::Hidden => (CursorShape::Block, false),
    };

    CursorInfo {
      col: content.cursor.point.column.0 as u16,
      row: content.cursor.point.line.0 as u16,
      shape,
      visible,
    }
  }

  /// Scroll the terminal viewport.
  pub fn scroll(&self, delta: i32) {
    let mut term = self.term.lock();
    term.scroll_display(alacritty_terminal::grid::Scroll::Delta(delta));
  }

  /// Scroll the terminal viewport to the bottom (current prompt).
  pub fn scroll_to_bottom(&self) {
    let mut term = self.term.lock();
    term.scroll_display(alacritty_terminal::grid::Scroll::Bottom);
  }

  /// Check if the terminal is in mouse mode (for mouse-aware programs like vim, less, tmux).
  pub fn mouse_mode(&self) -> bool {
    let term = self.term.lock();
    term.mode().contains(TermMode::MOUSE_MODE)
  }

  /// Check if SGR mouse mode is enabled (extended mouse reporting).
  pub fn sgr_mouse_mode(&self) -> bool {
    let term = self.term.lock();
    term.mode().contains(TermMode::SGR_MOUSE)
  }

  /// Get the current display offset (scroll position in history).
  pub fn display_offset(&self) -> usize {
    let term = self.term.lock();
    term.grid().display_offset()
  }

  /// Start a new selection at the given cell position.
  /// `selection_type`: Simple (character), Semantic (word), or Lines (line)
  pub fn start_selection(&self, col: u16, row: i32, selection_type: SelectionType) {
    let mut term = self.term.lock();
    let point = AlacPoint::new(Line(row), Column(col as usize));
    let selection = Selection::new(selection_type, point, AlacDirection::Left);
    term.selection = Some(selection);
  }

  /// Update the current selection to the given cell position.
  pub fn update_selection(&self, col: u16, row: i32) {
    let mut term = self.term.lock();
    if let Some(ref mut selection) = term.selection {
      let point = AlacPoint::new(Line(row), Column(col as usize));
      selection.update(point, AlacDirection::Right);
    }
  }

  /// Clear the current selection.
  pub fn clear_selection(&self) {
    let mut term = self.term.lock();
    term.selection = None;
  }

  /// Check if there is an active selection.
  pub fn has_selection(&self) -> bool {
    let term = self.term.lock();
    term.selection.is_some()
  }

  /// Get the selected text, if any.
  pub fn selection_text(&self) -> Option<String> {
    let term = self.term.lock();
    term.selection_to_string()
  }

  /// Get the selection range for rendering purposes.
  /// Returns (start_col, start_row, end_col, end_row) if there's a selection.
  pub fn selection_range(&self) -> Option<((u16, i32), (u16, i32))> {
    let term = self.term.lock();
    term.selection.as_ref().and_then(|sel| {
      let range = sel.to_range(&term)?;
      Some((
        (range.start.column.0 as u16, range.start.line.0),
        (range.end.column.0 as u16, range.end.line.0),
      ))
    })
  }

  // Note: Raw terminal access is available through render_cells() and cursor_info().
  // Direct term() access is intentionally not exposed to keep EventProxy private.
}

impl Drop for Terminal {
  fn drop(&mut self) {
    // Signal the event loop to shut down
    let _ = self.event_loop_sender.send(Msg::Shutdown);
  }
}

#[cfg(test)]
mod tests {
  use crate::test_utils::{
    cell_flags,
    char_at,
    cursor_pos,
    feed_str,
    row_content,
    test_term,
  };
  use alacritty_terminal::term::cell::Flags;

  // ============================================================
  // Cursor Movement Tests (CSI sequences)
  // ============================================================

  mod cursor_movement {
    use super::*;

    #[test]
    fn test_cursor_starts_at_origin() {
      let term = test_term(10, 10);
      assert_eq!(cursor_pos(&term), (0, 0));
    }

    #[test]
    fn test_cursor_advances_with_text() {
      let mut term = test_term(10, 10);
      feed_str(&mut term, "ABC");
      assert_eq!(cursor_pos(&term), (3, 0));
    }

    #[test]
    fn test_cursor_newline() {
      let mut term = test_term(10, 10);
      // LF (\n) moves down but doesn't reset column
      // Need CR+LF (\r\n) to go to start of next line
      feed_str(&mut term, "ABC\r\n");
      assert_eq!(cursor_pos(&term), (0, 1));
    }

    #[test]
    fn test_lf_preserves_column() {
      let mut term = test_term(10, 10);
      // LF alone doesn't reset column
      feed_str(&mut term, "ABC\n");
      assert_eq!(cursor_pos(&term), (3, 1));
    }

    #[test]
    fn test_cursor_up_csi_a() {
      let mut term = test_term(10, 10);
      feed_str(&mut term, "\n\n\n"); // Move to line 3
      assert_eq!(cursor_pos(&term), (0, 3));

      feed_str(&mut term, "\x1b[2A"); // CSI 2 A - move up 2 lines
      assert_eq!(cursor_pos(&term), (0, 1));
    }

    #[test]
    fn test_cursor_down_csi_b() {
      let mut term = test_term(10, 10);
      assert_eq!(cursor_pos(&term), (0, 0));

      feed_str(&mut term, "\x1b[3B"); // CSI 3 B - move down 3 lines
      assert_eq!(cursor_pos(&term), (0, 3));
    }

    #[test]
    fn test_cursor_forward_csi_c() {
      let mut term = test_term(10, 10);
      assert_eq!(cursor_pos(&term), (0, 0));

      feed_str(&mut term, "\x1b[5C"); // CSI 5 C - move right 5 columns
      assert_eq!(cursor_pos(&term), (5, 0));
    }

    #[test]
    fn test_cursor_back_csi_d() {
      let mut term = test_term(10, 10);
      feed_str(&mut term, "ABCDE"); // Cursor at column 5
      assert_eq!(cursor_pos(&term), (5, 0));

      feed_str(&mut term, "\x1b[3D"); // CSI 3 D - move left 3 columns
      assert_eq!(cursor_pos(&term), (2, 0));
    }

    #[test]
    fn test_cursor_position_csi_h() {
      let mut term = test_term(20, 20);

      // CSI row;col H (1-indexed in escape sequence)
      feed_str(&mut term, "\x1b[5;10H");
      assert_eq!(cursor_pos(&term), (9, 4)); // 0-indexed: col 9, row 4
    }

    #[test]
    fn test_cursor_home_csi_h_no_args() {
      let mut term = test_term(20, 20);
      feed_str(&mut term, "\x1b[5;10H"); // Move somewhere

      // CSI H with no args goes to home (1,1 -> 0,0)
      feed_str(&mut term, "\x1b[H");
      assert_eq!(cursor_pos(&term), (0, 0));
    }

    #[test]
    fn test_cursor_up_clamped_at_top() {
      let mut term = test_term(10, 5);

      // Try to move up from top - should stay at 0
      feed_str(&mut term, "\x1b[100A");
      let (_, row) = cursor_pos(&term);
      assert_eq!(row, 0);
    }

    #[test]
    fn test_cursor_left_clamped_at_start() {
      let mut term = test_term(10, 5);

      // Try to move left from column 0 - should stay at 0
      feed_str(&mut term, "\x1b[100D");
      let (col, _) = cursor_pos(&term);
      assert_eq!(col, 0);
    }

    #[test]
    fn test_carriage_return() {
      let mut term = test_term(20, 5);
      feed_str(&mut term, "Hello");
      assert_eq!(cursor_pos(&term), (5, 0));

      feed_str(&mut term, "\r");
      assert_eq!(cursor_pos(&term), (0, 0));
    }
  }

  // ============================================================
  // Line/Screen Clearing Tests
  // ============================================================

  mod clearing {
    use super::*;

    #[test]
    fn test_erase_to_end_of_line_csi_0k() {
      let mut term = test_term(10, 5);
      feed_str(&mut term, "ABCDEFGHIJ");
      feed_str(&mut term, "\x1b[1;6H"); // Position at column 5 (0-indexed)
      feed_str(&mut term, "\x1b[0K"); // Erase from cursor to end of line

      let line = row_content(&term, 0);
      assert_eq!(line, "ABCDE");
    }

    #[test]
    fn test_erase_to_beginning_of_line_csi_1k() {
      let mut term = test_term(10, 5);
      feed_str(&mut term, "ABCDEFGHIJ");
      feed_str(&mut term, "\x1b[1;6H"); // Position at column 5
      feed_str(&mut term, "\x1b[1K"); // Erase from beginning to cursor

      // Characters 0-5 should be spaces, rest intact
      let c5 = char_at(&term, 5, 0);
      let c6 = char_at(&term, 6, 0);
      // Position 5 is cleared (inclusive), position 6+ remains
      assert_eq!(c5, ' ');
      assert_eq!(c6, 'G');
    }

    #[test]
    fn test_erase_entire_line_csi_2k() {
      let mut term = test_term(10, 5);
      feed_str(&mut term, "ABCDEFGHIJ");
      feed_str(&mut term, "\x1b[1;6H"); // Position somewhere in line
      feed_str(&mut term, "\x1b[2K"); // Erase entire line

      let line = row_content(&term, 0);
      assert_eq!(line, "");
    }

    #[test]
    fn test_erase_to_end_of_screen_csi_0j() {
      let mut term = test_term(10, 5);
      feed_str(&mut term, "Line1\nLine2\nLine3");
      feed_str(&mut term, "\x1b[2;1H"); // Go to line 2 (0-indexed: row 1)
      feed_str(&mut term, "\x1b[0J"); // Erase from cursor to end of screen

      let line1 = row_content(&term, 0);
      let line2 = row_content(&term, 1);
      let line3 = row_content(&term, 2);

      assert_eq!(line1, "Line1"); // Unchanged
      assert_eq!(line2, ""); // Erased
      assert_eq!(line3, ""); // Erased
    }

    #[test]
    fn test_erase_entire_screen_csi_2j() {
      let mut term = test_term(10, 5);
      feed_str(&mut term, "Line1\nLine2\nLine3");
      feed_str(&mut term, "\x1b[2J"); // Erase entire screen

      for row in 0..5 {
        assert_eq!(row_content(&term, row), "");
      }
    }

    #[test]
    fn test_erase_default_is_to_end() {
      let mut term = test_term(10, 5);
      feed_str(&mut term, "ABCDEFGHIJ");
      feed_str(&mut term, "\x1b[1;6H"); // Position at column 5
      feed_str(&mut term, "\x1b[K"); // Erase (no param = 0 = to end)

      let line = row_content(&term, 0);
      assert_eq!(line, "ABCDE");
    }
  }

  // ============================================================
  // Text Attribute Tests (SGR)
  // ============================================================

  mod text_attributes {
    use super::*;

    #[test]
    fn test_sgr_bold() {
      let mut term = test_term(10, 5);
      feed_str(&mut term, "\x1b[1mB\x1b[0m");

      assert!(cell_flags(&term, 0, 0).contains(Flags::BOLD));
    }

    #[test]
    fn test_sgr_dim() {
      let mut term = test_term(10, 5);
      feed_str(&mut term, "\x1b[2mD\x1b[0m");

      assert!(cell_flags(&term, 0, 0).contains(Flags::DIM));
    }

    #[test]
    fn test_sgr_italic() {
      let mut term = test_term(10, 5);
      feed_str(&mut term, "\x1b[3mI\x1b[0m");

      assert!(cell_flags(&term, 0, 0).contains(Flags::ITALIC));
    }

    #[test]
    fn test_sgr_underline() {
      let mut term = test_term(10, 5);
      feed_str(&mut term, "\x1b[4mU\x1b[0m");

      assert!(cell_flags(&term, 0, 0).contains(Flags::UNDERLINE));
    }

    #[test]
    fn test_sgr_inverse() {
      let mut term = test_term(10, 5);
      feed_str(&mut term, "\x1b[7mV\x1b[0m");

      assert!(cell_flags(&term, 0, 0).contains(Flags::INVERSE));
    }

    #[test]
    fn test_sgr_strikethrough() {
      let mut term = test_term(10, 5);
      feed_str(&mut term, "\x1b[9mS\x1b[0m");

      assert!(cell_flags(&term, 0, 0).contains(Flags::STRIKEOUT));
    }

    #[test]
    fn test_sgr_reset_clears_attributes() {
      let mut term = test_term(10, 5);
      // Bold+Italic+Underline X, then reset, then Y
      feed_str(&mut term, "\x1b[1;3;4mX\x1b[0mY");

      let x_flags = cell_flags(&term, 0, 0);
      let y_flags = cell_flags(&term, 1, 0);

      assert!(x_flags.contains(Flags::BOLD));
      assert!(x_flags.contains(Flags::ITALIC));
      assert!(x_flags.contains(Flags::UNDERLINE));

      assert!(!y_flags.contains(Flags::BOLD));
      assert!(!y_flags.contains(Flags::ITALIC));
      assert!(!y_flags.contains(Flags::UNDERLINE));
    }

    #[test]
    fn test_sgr_combined_attributes() {
      let mut term = test_term(10, 5);
      // Set multiple attributes with single SGR
      feed_str(&mut term, "\x1b[1;4;7mC\x1b[0m");

      let flags = cell_flags(&term, 0, 0);
      assert!(flags.contains(Flags::BOLD));
      assert!(flags.contains(Flags::UNDERLINE));
      assert!(flags.contains(Flags::INVERSE));
    }
  }

  // ============================================================
  // Unicode and Special Character Tests
  // ============================================================

  mod unicode {
    use super::*;

    #[test]
    fn test_unicode_accented_characters() {
      let mut term = test_term(20, 5);
      feed_str(&mut term, "caf\u{00E9}"); // café with é

      assert_eq!(char_at(&term, 0, 0), 'c');
      assert_eq!(char_at(&term, 1, 0), 'a');
      assert_eq!(char_at(&term, 2, 0), 'f');
      assert_eq!(char_at(&term, 3, 0), '\u{00E9}'); // é
    }

    #[test]
    fn test_box_drawing_characters() {
      let mut term = test_term(20, 5);
      // Box drawing: ┌─┐
      feed_str(&mut term, "\u{250C}\u{2500}\u{2510}");

      assert_eq!(char_at(&term, 0, 0), '\u{250C}'); // Top-left corner
      assert_eq!(char_at(&term, 1, 0), '\u{2500}'); // Horizontal line
      assert_eq!(char_at(&term, 2, 0), '\u{2510}'); // Top-right corner
    }

    #[test]
    fn test_powerline_symbols() {
      let mut term = test_term(20, 5);
      // Powerline arrows
      feed_str(&mut term, "\u{E0B0}\u{E0B2}");

      assert_eq!(char_at(&term, 0, 0), '\u{E0B0}');
      assert_eq!(char_at(&term, 1, 0), '\u{E0B2}');
    }

    #[test]
    fn test_mixed_ascii_unicode() {
      let mut term = test_term(20, 5);
      feed_str(&mut term, "A\u{03B1}B\u{03B2}C"); // AαBβC

      assert_eq!(char_at(&term, 0, 0), 'A');
      assert_eq!(char_at(&term, 1, 0), '\u{03B1}'); // alpha
      assert_eq!(char_at(&term, 2, 0), 'B');
      assert_eq!(char_at(&term, 3, 0), '\u{03B2}'); // beta
      assert_eq!(char_at(&term, 4, 0), 'C');
    }
  }

  // ============================================================
  // Terminal Mode Tests
  // ============================================================

  mod terminal_modes {
    use super::*;

    #[test]
    fn test_alternate_screen_buffer() {
      let mut term = test_term(10, 5);
      feed_str(&mut term, "MainScreen");

      // Switch to alternate screen (DECSET 1049)
      feed_str(&mut term, "\x1b[?1049h");

      // Write to alternate screen
      feed_str(&mut term, "AltScreen");

      // Switch back to main screen (DECRST 1049)
      feed_str(&mut term, "\x1b[?1049l");

      // Main screen content should be restored
      let line = row_content(&term, 0);
      assert!(line.contains("MainScreen"));
    }
  }

  // ============================================================
  // Tab Stop Tests
  // ============================================================

  mod tab_stops {
    use super::*;

    #[test]
    fn test_horizontal_tab() {
      let mut term = test_term(20, 5);
      feed_str(&mut term, "A\tB");

      // Tab should advance to next tab stop (default every 8 columns)
      assert_eq!(char_at(&term, 0, 0), 'A');
      // B should be at position 8 (first tab stop after column 1)
      let b_pos = (0..20).find(|&col| char_at(&term, col, 0) == 'B');
      assert!(b_pos.is_some());
      assert_eq!(b_pos.unwrap(), 8);
    }

    #[test]
    fn test_multiple_tabs() {
      let mut term = test_term(30, 5);
      feed_str(&mut term, "A\t\tB");

      // First tab to 8, second tab to 16
      let b_pos = (0..30).find(|&col| char_at(&term, col, 0) == 'B');
      assert!(b_pos.is_some());
      assert_eq!(b_pos.unwrap(), 16);
    }
  }

  // ============================================================
  // Line Wrapping Tests
  // ============================================================

  mod line_wrapping {
    use super::*;

    #[test]
    fn test_auto_wrap_at_edge() {
      let mut term = test_term(5, 3);
      feed_str(&mut term, "ABCDEFGH");

      // First 5 chars on line 0
      assert_eq!(row_content(&term, 0), "ABCDE");
      // Rest wrap to line 1
      assert_eq!(row_content(&term, 1), "FGH");
    }

    #[test]
    fn test_no_wrap_with_escape_sequence() {
      let mut term = test_term(10, 3);
      // Text with color change shouldn't affect wrapping
      feed_str(&mut term, "ABC\x1b[31mDEF\x1b[0mGHI");

      let line = row_content(&term, 0);
      assert_eq!(line, "ABCDEFGHI");
    }
  }

  // ============================================================
  // OSC Command Tests
  // ============================================================

  mod osc_commands {
    use super::*;

    #[test]
    fn test_osc_title_does_not_affect_content() {
      let mut term = test_term(20, 5);
      // OSC 0 ; title BEL - set window title
      feed_str(&mut term, "\x1b]0;MyTitle\x07");
      feed_str(&mut term, "Content");

      // Title sequence should not appear in content
      let line = row_content(&term, 0);
      assert_eq!(line, "Content");
    }

    #[test]
    fn test_osc_hyperlink_text_visible() {
      let mut term = test_term(40, 5);
      // OSC 8 hyperlink
      feed_str(&mut term, "\x1b]8;;https://example.com\x1b\\Link\x1b]8;;\x1b\\");

      // The text "Link" should be visible
      let line = row_content(&term, 0);
      assert!(line.contains("Link"));
    }
  }
}
