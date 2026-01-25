//! Proof-of-life terminal client for the-editor.
//!
//! This is a minimal terminal client that validates the-lib's infrastructure:
//! - Document creation and editing via Transactions
//! - RenderPlan generation and display
//! - Syntax highlighting via tree-sitter
//! - Multiple cursor support

mod ctx;
mod dispatch;
mod input;
mod render;
mod terminal;

use std::time::Duration;

use crossterm::event::{
  self,
  Event,
};
use eyre::Result;

use crate::{
  ctx::Ctx,
  dispatch::build_dispatch,
};

fn main() -> Result<()> {
  // Parse command line arguments
  let args: Vec<String> = std::env::args().collect();
  let file_path = args.get(1).map(|s| s.as_str());

  // Initialize application state
  let mut ctx = Ctx::new(file_path)?;
  let dispatch = build_dispatch();
  let mut terminal = terminal::Terminal::new()?;

  terminal.enter_raw_mode()?;

  // Initial render
  render::render(&mut ctx, &mut terminal)?;

  // Event loop
  loop {
    if ctx.should_quit {
      break;
    }

    if event::poll(Duration::from_millis(100))? {
      match event::read()? {
        Event::Key(key) => {
          input::handle_key(&dispatch, &mut ctx, key);
        },
        Event::Resize(w, h) => {
          ctx.resize(w, h);
          ctx.needs_render = true;
        },
        _ => {},
      }
    }

    // Render if needed
    if ctx.needs_render {
      render::ensure_cursor_visible(&mut ctx);
      render::render(&mut ctx, &mut terminal)?;
      ctx.needs_render = false;
    }
  }

  terminal.leave_raw_mode()?;
  Ok(())
}
