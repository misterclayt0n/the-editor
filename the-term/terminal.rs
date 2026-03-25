//! Terminal abstraction using ratatui + crossterm backend.

use std::io::{
  self,
  Stdout,
};

use crossterm::{
  cursor::{
    Hide,
    MoveTo,
    SetCursorStyle,
    Show,
  },
  event::{
    DisableMouseCapture,
    EnableMouseCapture,
    KeyboardEnhancementFlags,
    PopKeyboardEnhancementFlags,
    PushKeyboardEnhancementFlags,
  },
  execute,
  terminal::{
    EnterAlternateScreen,
    LeaveAlternateScreen,
    disable_raw_mode,
    enable_raw_mode,
  },
};
use eyre::Result;
use ratatui::{
  Terminal as RatatuiTerminal,
  prelude::Rect,
};
use the_lib::render::graphics::CursorKind as LibCursorKind;

use crate::{
  ctx::TermCursorMode,
  undercurl_backend::UndercurlCrosstermBackend,
};

pub struct Terminal {
  terminal:                      RatatuiTerminal<UndercurlCrosstermBackend<Stdout>>,
  keyboard_enhancements_enabled: bool,
}

impl Terminal {
  pub fn new() -> Result<Self> {
    let stdout = io::stdout();
    let backend = UndercurlCrosstermBackend::new(stdout);
    let terminal = RatatuiTerminal::new(backend)?;
    Ok(Self {
      terminal,
      keyboard_enhancements_enabled: false,
    })
  }

  pub fn enter_raw_mode(&mut self, enable_mouse: bool) -> Result<()> {
    enable_raw_mode()?;
    if enable_mouse {
      execute!(
        self.terminal.backend_mut(),
        EnterAlternateScreen,
        EnableMouseCapture,
        Hide
      )?;
    } else {
      execute!(self.terminal.backend_mut(), EnterAlternateScreen, Hide)?;
    }
    let enhancement_flags = KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
      | KeyboardEnhancementFlags::REPORT_EVENT_TYPES;
    if execute!(
      self.terminal.backend_mut(),
      PushKeyboardEnhancementFlags(enhancement_flags)
    )
    .is_ok()
    {
      self.keyboard_enhancements_enabled = true;
    }
    Ok(())
  }

  pub fn leave_raw_mode(&mut self) -> Result<()> {
    if self.keyboard_enhancements_enabled {
      let _ = execute!(self.terminal.backend_mut(), PopKeyboardEnhancementFlags);
      self.keyboard_enhancements_enabled = false;
    }
    execute!(
      self.terminal.backend_mut(),
      SetCursorStyle::DefaultUserShape,
      DisableMouseCapture,
      LeaveAlternateScreen,
      Show
    )?;
    disable_raw_mode()?;
    Ok(())
  }

  pub fn draw<F>(&mut self, f: F) -> Result<()>
  where
    F: for<'a> FnOnce(&mut ratatui::Frame<'a>),
  {
    self.terminal.draw(f)?;
    Ok(())
  }

  pub fn apply_editor_cursor(&mut self, cursor: TermCursorMode) -> Result<()> {
    match cursor {
      TermCursorMode::Hardware(cursor) => {
        let (x, y, kind) = (cursor.x, cursor.y, cursor.kind);
        let shape = match kind {
          LibCursorKind::Bar => SetCursorStyle::SteadyBar,
          LibCursorKind::Underline => SetCursorStyle::SteadyUnderScore,
          LibCursorKind::Block => SetCursorStyle::SteadyBlock,
          LibCursorKind::Hollow | LibCursorKind::Hidden => SetCursorStyle::DefaultUserShape,
        };
        execute!(self.terminal.backend_mut(), shape, MoveTo(x, y), Show)?;
      },
      TermCursorMode::Hidden => {
        execute!(self.terminal.backend_mut(), Hide)?;
      },
    }
    Ok(())
  }

  pub fn resize(&mut self, width: u16, height: u16) -> Result<()> {
    self.terminal.resize(Rect::new(0, 0, width, height))?;
    Ok(())
  }

  pub fn size(&self) -> Result<Rect> {
    Ok(self.terminal.size()?)
  }
}
