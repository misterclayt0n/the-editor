//! Terminal client for the-editor.

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
  ffi::OsString,
  path::PathBuf,
  process,
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

use crate::ctx::Ctx;

#[derive(Debug, Parser)]
#[command(name = "the-editor")]
#[command(about = "Terminal client for the-editor")]
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
  /// Create a config crate from the template
  #[command(alias = "install")]
  Init {
    /// Config crate directory
    #[arg(long)]
    config_dir:   Option<PathBuf>,
    /// Optional package name for the created config crate
    #[arg(long)]
    package_name: Option<String>,
  },
  /// Print the resolved config crate directory
  Path {
    /// Config crate directory
    #[arg(long)]
    config_dir: Option<PathBuf>,
  },
  /// Show config path, package, target, and build harness status
  Status {
    /// Config crate directory
    #[arg(long)]
    config_dir: Option<PathBuf>,
  },
  /// Validate the selected config workflow with cargo check
  Check {
    /// Config crate directory
    #[arg(long)]
    config_dir: Option<PathBuf>,
    /// Client target to validate
    #[arg(long, value_enum, default_value_t = config_cli::ConfigTarget::Term)]
    target:     config_cli::ConfigTarget,
    /// Use release mode
    #[arg(long)]
    release:    bool,
  },
  /// Build the selected client target with the config crate
  Build {
    /// Config crate directory
    #[arg(long)]
    config_dir: Option<PathBuf>,
    /// Client target to build
    #[arg(long, value_enum, default_value_t = config_cli::ConfigTarget::Term)]
    target:     config_cli::ConfigTarget,
    /// Use release mode
    #[arg(long)]
    release:    bool,
    /// Copy the built binary to an explicit output path
    #[arg(long)]
    out:        Option<PathBuf>,
    /// Install a stable copy under the config crate directory
    #[arg(long)]
    install:    bool,
  },
  /// Build and run the selected client target with the config crate
  Run {
    /// Config crate directory
    #[arg(long)]
    config_dir: Option<PathBuf>,
    /// Client target to run
    #[arg(long, value_enum, default_value_t = config_cli::ConfigTarget::Term)]
    target:     config_cli::ConfigTarget,
    /// Use release mode
    #[arg(long)]
    release:    bool,
    /// Arguments passed through to the configured binary
    #[arg(last = true, allow_hyphen_values = true)]
    args:       Vec<OsString>,
  },
}

fn main() -> Result<()> {
  let cli = Cli::parse();
  if let Some(command) = cli.command {
    match command {
      Command::Config { command } => {
        match command {
          ConfigCommand::Init {
            config_dir,
            package_name,
          } => {
            config_cli::init_config_template(config_cli::ConfigInitOptions {
              config_dir,
              package_name,
            })?;
            return Ok(());
          },
          ConfigCommand::Path { config_dir } => {
            config_cli::print_config_path(config_cli::ConfigPathOptions { config_dir })?;
            return Ok(());
          },
          ConfigCommand::Status { config_dir } => {
            config_cli::print_config_status(config_cli::ConfigPathOptions { config_dir })?;
            return Ok(());
          },
          ConfigCommand::Check {
            config_dir,
            target,
            release,
          } => {
            config_cli::check_config_binary(config_cli::ConfigBuildOptions {
              config_dir,
              target,
              release,
              out_path: None,
              install: false,
            })?;
            return Ok(());
          },
          ConfigCommand::Build {
            config_dir,
            target,
            release,
            out,
            install,
          } => {
            config_cli::build_config_binary(config_cli::ConfigBuildOptions {
              config_dir,
              target,
              release,
              out_path: out,
              install,
            })?;
            return Ok(());
          },
          ConfigCommand::Run {
            config_dir,
            target,
            release,
            args,
          } => {
            let status = config_cli::run_config_binary(config_cli::ConfigRunOptions {
              config_dir,
              target,
              release,
              args,
            })?;
            process::exit(status.code().unwrap_or(1));
          },
        }
      },
    }
  }

  let file_path = cli.file.as_deref();

  // Initialize application state
  let preset = the_config::build_editor_preset::<Ctx>()
    .build()
    .box_dispatch();
  let mut ctx = Ctx::new_with_defaults(file_path, preset.defaults())?;
  ctx.install_preset(preset);
  ctx.start_background_services();
  let mut terminal = terminal::Terminal::new()?;

  terminal.enter_raw_mode(ctx.preset.defaults().term.mouse.unwrap_or(true))?;

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
      match ctx.render_wake_rx.try_recv() {
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
