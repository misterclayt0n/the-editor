//! Terminal client for the-editor.

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

use clap::Parser;
use crossterm::event::{
  self,
  Event,
};
use eyre::Result;

use crate::{
  ctx::Ctx,
  render::{
    LoopPerfInfo,
    RenderReason,
    RenderReasonMask,
  },
};

#[derive(Debug, Parser)]
#[command(name = "the-editor")]
#[command(about = "Terminal client for the-editor")]
struct Cli {
  /// Path to file to open
  file: Option<String>,
}

fn main() -> Result<()> {
  let cli = Cli::parse();
  let file_path = cli.file.as_deref();

  // Initialize application state for runtime startup
  let defaults = the_default::default_defaults();
  let mut ctx = Ctx::new_with_defaults(file_path, &defaults)?;
  ctx.start_background_services();
  let mut terminal = terminal::Terminal::new()?;

  terminal.enter_raw_mode(ctx.defaults.term.mouse.unwrap_or(true))?;

  // Initial render
  ctx.needs_render = false;
  let initial_render_reasons = RenderReasonMask::from_reason(RenderReason::Startup);
  let initial_render_start = Instant::now();
  let mut initial_debug = render::RenderDebugInfo::default();
  let initial_backend = terminal.draw(|f| {
    initial_debug = render::render(f, &mut ctx, initial_render_reasons);
  })?;
  let initial_after_draw = Instant::now();
  let initial_cursor_backend = terminal.apply_editor_cursor(ctx.term_cursor_mode)?;
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
    initial_debug,
    initial_backend,
    initial_cursor_backend,
  );

  // Event loop
  let mut render_reasons = RenderReasonMask::empty();
  loop {
    if ctx.should_quit {
      break;
    }

    let loop_start = Instant::now();
    let mut loop_perf = LoopPerfInfo::default();

    let input_start = Instant::now();
    if event::poll(Duration::from_millis(16))? {
      match event::read()? {
        Event::Key(key) => {
          input::handle_key(&mut ctx, key);
          render_reasons.insert(RenderReason::KeyInput);
        },
        Event::Mouse(mouse) => {
          input::handle_mouse(&mut ctx, mouse);
          render_reasons.insert(RenderReason::MouseInput);
        },
        Event::Resize(w, h) => {
          ctx.resize(w, h);
          terminal.resize(w, h)?;
          ctx.needs_render = true;
          render_reasons.insert(RenderReason::Resize);
        },
        Event::FocusGained => {
          ctx.handle_terminal_focus_gained();
          render_reasons.insert(RenderReason::FocusChange);
        },
        Event::FocusLost => {
          ctx.handle_terminal_focus_lost();
          render_reasons.insert(RenderReason::FocusChange);
        },
        _ => {},
      }
    }
    loop_perf.input_ms = input_start.elapsed().as_secs_f64() * 1000.0;

    let wake_start = Instant::now();
    loop {
      match ctx.render_wake_rx.try_recv() {
        Ok(()) => {
          ctx.needs_render = true;
          render_reasons.insert(RenderReason::Wake);
        },
        Err(TryRecvError::Empty) => break,
        Err(TryRecvError::Disconnected) => break,
      }
    }
    loop_perf.wake_ms = wake_start.elapsed().as_secs_f64() * 1000.0;

    let syntax_start = Instant::now();
    if ctx.poll_syntax_parse_results() {
      ctx.needs_render = true;
      render_reasons.insert(RenderReason::SyntaxParse);
    }
    loop_perf.syntax_ms = syntax_start.elapsed().as_secs_f64() * 1000.0;

    let global_search_start = Instant::now();
    if ctx.poll_global_search() {
      ctx.needs_render = true;
      render_reasons.insert(RenderReason::GlobalSearch);
    }
    loop_perf.global_search_ms = global_search_start.elapsed().as_secs_f64() * 1000.0;

    let vcs_diff_picker_start = Instant::now();
    if ctx.poll_vcs_diff_picker() {
      ctx.needs_render = true;
      render_reasons.insert(RenderReason::Wake);
    }
    loop_perf.global_search_ms += vcs_diff_picker_start.elapsed().as_secs_f64() * 1000.0;

    let lsp_completion_start = Instant::now();
    if ctx.poll_lsp_completion_auto_trigger() {
      ctx.needs_render = true;
      render_reasons.insert(RenderReason::LspCompletion);
    }
    loop_perf.lsp_completion_ms = lsp_completion_start.elapsed().as_secs_f64() * 1000.0;

    let lsp_signature_start = Instant::now();
    if ctx.poll_lsp_signature_help_auto_trigger() {
      ctx.needs_render = true;
      render_reasons.insert(RenderReason::LspSignature);
    }
    loop_perf.lsp_signature_ms = lsp_signature_start.elapsed().as_secs_f64() * 1000.0;

    let lsp_events_start = Instant::now();
    if ctx.poll_lsp_events() {
      ctx.needs_render = true;
      render_reasons.insert(RenderReason::LspEvents);
    }
    loop_perf.lsp_events_ms = lsp_events_start.elapsed().as_secs_f64() * 1000.0;

    let lsp_file_watch_start = Instant::now();
    if ctx.poll_lsp_file_watch() {
      ctx.needs_render = true;
      render_reasons.insert(RenderReason::LspFileWatch);
    }
    loop_perf.lsp_file_watch_ms = lsp_file_watch_start.elapsed().as_secs_f64() * 1000.0;

    let vcs_watch_start = Instant::now();
    if ctx.poll_vcs_watch() {
      ctx.needs_render = true;
      render_reasons.insert(RenderReason::VcsWatch);
    }
    loop_perf.vcs_watch_ms = vcs_watch_start.elapsed().as_secs_f64() * 1000.0;

    let active_file_vcs_dispatch_start = Instant::now();
    let _ = ctx.poll_active_file_vcs_refresh_dispatch(Instant::now());
    loop_perf.active_file_vcs_dispatch_ms =
      active_file_vcs_dispatch_start.elapsed().as_secs_f64() * 1000.0;

    let active_file_vcs_apply_start = Instant::now();
    if ctx.poll_active_file_vcs_refresh_results() {
      ctx.needs_render = true;
      render_reasons.insert(RenderReason::ActiveFileVcsRefresh);
    }
    loop_perf.active_file_vcs_apply_ms =
      active_file_vcs_apply_start.elapsed().as_secs_f64() * 1000.0;

    let file_tree_watch_start = Instant::now();
    if ctx.poll_file_tree_watch() {
      ctx.needs_render = true;
      render_reasons.insert(RenderReason::FileTreeWatch);
    }
    loop_perf.file_tree_watch_ms = file_tree_watch_start.elapsed().as_secs_f64() * 1000.0;

    let file_tree_vcs_dispatch_start = Instant::now();
    let _ = ctx.poll_file_tree_vcs_refresh_dispatch(Instant::now());
    loop_perf.file_tree_vcs_dispatch_ms =
      file_tree_vcs_dispatch_start.elapsed().as_secs_f64() * 1000.0;

    let file_tree_vcs_apply_start = Instant::now();
    if ctx.poll_file_tree_vcs_refresh_results() {
      ctx.needs_render = true;
      render_reasons.insert(RenderReason::FileTreeVcsRefresh);
    }
    loop_perf.file_tree_vcs_apply_ms = file_tree_vcs_apply_start.elapsed().as_secs_f64() * 1000.0;

    let statusline_start = Instant::now();
    if ctx.tick_lsp_statusline() {
      ctx.needs_render = true;
      render_reasons.insert(RenderReason::StatuslineTick);
    }
    loop_perf.statusline_ms = statusline_start.elapsed().as_secs_f64() * 1000.0;
    loop_perf.total_ms = loop_start.elapsed().as_secs_f64() * 1000.0;
    ctx.flush_message_log();
    render::log_loop_perf(render_reasons, loop_perf, ctx.needs_render);

    // Render if needed
    if ctx.needs_render {
      ctx.needs_render = false;
      let active_render_reasons = render_reasons;
      render_reasons = RenderReasonMask::empty();
      let render_start = Instant::now();
      let mut render_debug = render::RenderDebugInfo::default();
      let draw_backend = terminal.draw(|f| {
        render_debug = render::render(f, &mut ctx, active_render_reasons);
      })?;
      let after_draw = Instant::now();
      let cursor_backend = terminal.apply_editor_cursor(ctx.term_cursor_mode)?;
      let total_ms = render_start.elapsed().as_secs_f64() * 1000.0;
      let draw_ms = after_draw.duration_since(render_start).as_secs_f64() * 1000.0;
      let cursor_ms = total_ms - draw_ms;
      render::log_present_perf(
        &ctx,
        "update",
        draw_ms,
        cursor_ms,
        total_ms,
        render_debug,
        draw_backend,
        cursor_backend,
      );
    }
  }

  ctx.shutdown_background_services();
  terminal.leave_raw_mode()?;
  Ok(())
}
