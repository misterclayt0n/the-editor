//! Terminal emulation wrapper around alacritty_terminal.

use std::sync::Arc;

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
  sync::FairMutex,
  term::{
    Config as TermConfig,
    Term,
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
  id:     TerminalId,
  sender: mpsc::UnboundedSender<TerminalEvent>,
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
        // This shouldn't happen in normal operation
        log::warn!("Unexpected PtyWrite event: {}", text);
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
    config: TerminalConfig,
    event_sender: mpsc::UnboundedSender<TerminalEvent>,
  ) -> anyhow::Result<Self> {
    let event_proxy = EventProxy { id, sender: event_sender.clone() };

    // Determine shell
    let shell = config.shell.clone().unwrap_or_else(|| {
      std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string())
    });

    // Create terminal config
    let term_config = TermConfig::default();

    // Create window size
    let window_size = WindowSize {
      num_cols:    cols,
      num_lines:   rows,
      cell_width:  1, // Will be set properly by renderer
      cell_height: 1,
    };

    // Create PTY options
    let pty_config = PtyOptions {
      shell:             Some(tty::Shell::new(shell, vec![])),
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
        sender: event_sender.clone(),
      },
      pty,
      false, // hold
      false, // ref_test
    )?;

    // Start the event loop
    let event_loop_sender = event_loop.channel();
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
  pub fn resize(&mut self, cols: u16, rows: u16) {
    if cols == self.cols && rows == self.rows {
      return;
    }

    self.cols = cols;
    self.rows = rows;

    let size = WindowSize {
      num_cols:    cols,
      num_lines:   rows,
      cell_width:  1,
      cell_height: 1,
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

    CursorInfo {
      col:   content.cursor.point.column.0 as u16,
      row:   content.cursor.point.line.0 as u16,
      shape: match content.cursor.shape {
        alacritty_terminal::vte::ansi::CursorShape::Block => CursorShape::Block,
        alacritty_terminal::vte::ansi::CursorShape::Underline => CursorShape::Underline,
        alacritty_terminal::vte::ansi::CursorShape::Beam => CursorShape::Beam,
        _ => CursorShape::Block,
      },
      // Cursor visibility is managed by mode in alacritty_terminal 0.25
      visible: true,
    }
  }

  /// Scroll the terminal viewport.
  pub fn scroll(&self, delta: i32) {
    let mut term = self.term.lock();
    term.scroll_display(alacritty_terminal::grid::Scroll::Delta(delta));
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
