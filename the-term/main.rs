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
mod docs_panel;
mod input;
mod picker_layout;
mod render;
mod terminal;
mod theme;
mod undercurl_backend;

use std::{
  sync::mpsc::TryRecvError,
  time::{
    Duration,
    Instant,
  },
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
  ctx.start_background_services();
  let mut terminal = terminal::Terminal::new()?;

  terminal.enter_raw_mode()?;

  // Initial render
  ctx.needs_render = false;
  let initial_render_start = Instant::now();
  terminal.draw(|f| render::render(f, &mut ctx))?;
  let initial_after_draw = Instant::now();
  terminal.apply_editor_cursor(
    ctx
      .term_hardware_cursor
      .map(|cursor| (cursor.x, cursor.y, cursor.kind)),
  )?;
  let initial_total_ms = initial_render_start.elapsed().as_secs_f64() * 1000.0;
  let initial_draw_ms = initial_after_draw
    .duration_since(initial_render_start)
    .as_secs_f64()
    * 1000.0;
  let initial_cursor_ms = initial_total_ms - initial_draw_ms;
  render::log_present_perf(
    &ctx,
    "initial",
    initial_draw_ms,
    initial_cursor_ms,
    initial_total_ms,
  );

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

    if ctx.poll_syntax_parse_results() {
      ctx.needs_render = true;
    }

    if ctx.poll_global_search() {
      ctx.needs_render = true;
    }

    if ctx.poll_lsp_completion_auto_trigger() {
      ctx.needs_render = true;
    }

    if ctx.poll_lsp_signature_help_auto_trigger() {
      ctx.needs_render = true;
    }

    if ctx.poll_lsp_events() {
      ctx.needs_render = true;
    }

    if ctx.poll_lsp_file_watch() {
      ctx.needs_render = true;
    }
    if ctx.tick_lsp_statusline() {
      ctx.needs_render = true;
    }
    if ctx.tick_vcs_statusline() {
      ctx.needs_render = true;
    }
    ctx.flush_message_log();

    // Render if needed
    if ctx.needs_render {
      ctx.needs_render = false;
      let render_start = Instant::now();
      terminal.draw(|f| render::render(f, &mut ctx))?;
      let after_draw = Instant::now();
      terminal.apply_editor_cursor(
        ctx
          .term_hardware_cursor
          .map(|cursor| (cursor.x, cursor.y, cursor.kind)),
      )?;
      let total_ms = render_start.elapsed().as_secs_f64() * 1000.0;
      let draw_ms = after_draw.duration_since(render_start).as_secs_f64() * 1000.0;
      let cursor_ms = total_ms - draw_ms;
      render::log_present_perf(&ctx, "update", draw_ms, cursor_ms, total_ms);
    }
  }

  ctx.shutdown_background_services();
  terminal.leave_raw_mode()?;
  Ok(())
}
