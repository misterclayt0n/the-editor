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
mod picker_layout;
mod render;
mod terminal;
mod theme;

use std::{
  sync::mpsc::TryRecvError,
  time::Duration,
};

use clap::{
  Parser,
  Subcommand,
};
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
  #[command(subcommand)]
  command: Option<Command>,

  /// Path to file to open
  file: Option<String>,
}

#[derive(Debug, Subcommand)]
enum Command {
  /// Manage editor configuration
  Config {
    #[command(subcommand)]
    command: ConfigCommand,
  },
}

#[derive(Debug, Subcommand)]
enum ConfigCommand {
  /// Install a default config crate in ~/.config/the-editor
  Install,
  /// Build the editor using ~/.config/the-editor if present
  Build,
}

fn main() -> Result<()> {
  let cli = Cli::parse();
  if let Some(command) = cli.command {
    match command {
      Command::Config { command } => {
        match command {
          ConfigCommand::Install => {
            config_cli::install_config_template()?;
            return Ok(());
          },
          ConfigCommand::Build => {
            config_cli::build_config_binary()?;
            return Ok(());
          },
        }
      },
    }
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
  ctx.needs_render = false;
  terminal.draw(|f| render::render(f, &mut ctx))?;

  // Event loop
  loop {
    if ctx.should_quit {
      break;
    }

    if event::poll(Duration::from_millis(16))? {
      match event::read()? {
        Event::Key(key) => {
          input::handle_key(&mut ctx, key);
        },
        Event::Mouse(mouse) => {
          input::handle_mouse(&mut ctx, mouse);
        },
        Event::Resize(w, h) => {
          ctx.resize(w, h);
          terminal.resize(w, h)?;
          ctx.needs_render = true;
        },
        _ => {},
      }
    }

    loop {
      match ctx.file_picker_wake_rx.try_recv() {
        Ok(()) => {
          ctx.needs_render = true;
        },
        Err(TryRecvError::Empty) => break,
        Err(TryRecvError::Disconnected) => break,
      }
    }

    // Render if needed
    if ctx.needs_render {
      ctx.needs_render = false;
      terminal.draw(|f| render::render(f, &mut ctx))?;
    }
  }

  terminal.leave_raw_mode()?;
  Ok(())
}
