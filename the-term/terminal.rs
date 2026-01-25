//! Terminal abstraction over crossterm.

use std::io::{
  self,
  Stdout,
  Write,
};

use crossterm::{
  cursor::{
    self,
    MoveTo,
    SetCursorStyle,
  },
  execute,
  queue,
  style::{
    Color,
    Print,
    ResetColor,
    SetBackgroundColor,
    SetForegroundColor,
  },
  terminal::{
    self,
    Clear,
    ClearType,
    EnterAlternateScreen,
    LeaveAlternateScreen,
  },
};
use eyre::Result;

pub struct Terminal {
  stdout: Stdout,
  size:   (u16, u16),
}

impl Terminal {
  pub fn new() -> Result<Self> {
    let stdout = io::stdout();
    let size = terminal::size()?;
    Ok(Self { stdout, size })
  }

  pub fn enter_raw_mode(&mut self) -> Result<()> {
    terminal::enable_raw_mode()?;
    execute!(
      self.stdout,
      EnterAlternateScreen,
      Clear(ClearType::All),
      cursor::Hide
    )?;
    Ok(())
  }

  pub fn leave_raw_mode(&mut self) -> Result<()> {
    execute!(self.stdout, cursor::Show, LeaveAlternateScreen, ResetColor)?;
    terminal::disable_raw_mode()?;
    Ok(())
  }

  pub fn size(&self) -> (u16, u16) {
    self.size
  }

  pub fn set_size(&mut self, width: u16, height: u16) {
    self.size = (width, height);
  }

  pub fn clear(&mut self) -> Result<()> {
    queue!(self.stdout, Clear(ClearType::All))?;
    Ok(())
  }

  pub fn draw_str(
    &mut self,
    row: u16,
    col: u16,
    s: &str,
    fg: Option<Color>,
    bg: Option<Color>,
  ) -> Result<()> {
    queue!(self.stdout, MoveTo(col, row))?;

    if let Some(fg) = fg {
      queue!(self.stdout, SetForegroundColor(fg))?;
    }
    if let Some(bg) = bg {
      queue!(self.stdout, SetBackgroundColor(bg))?;
    }

    queue!(self.stdout, Print(s))?;

    if fg.is_some() || bg.is_some() {
      queue!(self.stdout, ResetColor)?;
    }

    Ok(())
  }

  pub fn set_cursor(&mut self, row: u16, col: u16) -> Result<()> {
    queue!(
      self.stdout,
      cursor::Show,
      MoveTo(col, row),
      SetCursorStyle::SteadyBlock
    )?;
    Ok(())
  }

  pub fn hide_cursor(&mut self) -> Result<()> {
    queue!(self.stdout, cursor::Hide)?;
    Ok(())
  }

  pub fn flush(&mut self) -> Result<()> {
    self.stdout.flush()?;
    Ok(())
  }
}
