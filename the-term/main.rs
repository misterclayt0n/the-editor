//! Proof-of-life terminal client for the-editor.
//!
//! This is a minimal terminal client that validates the-lib's infrastructure:
//! - Document creation and editing via Transactions
//! - RenderPlan generation and display
//! - Syntax highlighting via tree-sitter
//! - Multiple cursor support

mod config_cli;
mod ctx;
mod dispatch;
mod input;
mod render;
mod terminal;
mod theme;

use std::time::Duration;

use clap::Parser;
use crossterm::event::{
  self,
  Event,
};
use eyre::Result;

use crate::{
  ctx::Ctx,
  dispatch::build_dispatch,
};

#[derive(Debug, Parser)]
#[command(name = "the-editor")]
#[command(about = "Proof-of-life terminal client for the-editor")]
struct Cli {
  /// Install a default config crate in ~/.config/the-editor
  #[arg(long)]
  install_config: bool,

  /// Build the editor using ~/.config/the-editor if present
  #[arg(long)]
  build_config: bool,

  /// Path to file to open
  file: Option<String>,
}

fn main() -> Result<()> {
  let cli = Cli::parse();
  if cli.install_config {
    config_cli::install_config_template()?;
    return Ok(());
  }

  if cli.build_config {
    config_cli::build_config_binary()?;
    return Ok(());
  }

  let file_path = cli.file.as_deref();

  // Initialize application state
  let mut ctx = Ctx::new(file_path)?;
  ctx.keymaps = the_config::build_keymaps();
  let dispatch = build_dispatch::<Ctx>();
  ctx.set_dispatch(&dispatch);
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
          input::handle_key(&mut ctx, key);
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
