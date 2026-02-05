//! Terminal abstraction using ratatui + crossterm backend.

use std::io::{
  self,
  Stdout,
};

use crossterm::{
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
  backend::CrosstermBackend,
  prelude::Rect,
};

pub struct Terminal {
  terminal: RatatuiTerminal<CrosstermBackend<Stdout>>,
}

impl Terminal {
  pub fn new() -> Result<Self> {
    let stdout = io::stdout();
    let backend = CrosstermBackend::new(stdout);
    let terminal = RatatuiTerminal::new(backend)?;
    Ok(Self { terminal })
  }

  pub fn enter_raw_mode(&mut self) -> Result<()> {
    enable_raw_mode()?;
    execute!(self.terminal.backend_mut(), EnterAlternateScreen)?;
    Ok(())
  }

  pub fn leave_raw_mode(&mut self) -> Result<()> {
    execute!(self.terminal.backend_mut(), LeaveAlternateScreen)?;
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

  pub fn resize(&mut self, width: u16, height: u16) -> Result<()> {
    self.terminal.resize(Rect::new(0, 0, width, height))?;
    Ok(())
  }

  pub fn size(&self) -> Result<Rect> {
    Ok(self.terminal.size()?)
  }
}
