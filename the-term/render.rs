//! Rendering - converts RenderPlan to terminal draw calls.

use crossterm::style::Color;
use eyre::Result;
use the_default::render_plan;
use the_lib::render::{
  NoHighlights,
  RenderPlan,
  RenderStyles,
  SyntaxHighlightAdapter,
  build_plan,
  text_annotations::TextAnnotations,
};
use the_lib::selection::Range;

use crate::{
  Ctx,
  terminal::Terminal,
  theme::highlight_to_color,
};

pub fn build_render_plan(ctx: &mut Ctx) -> RenderPlan {
  build_render_plan_with_styles(ctx, RenderStyles::default())
}

pub fn build_render_plan_with_styles(ctx: &mut Ctx, styles: RenderStyles) -> RenderPlan {
  let view = ctx.editor.view();

  // Set up text formatting
  ctx.text_format.viewport_width = view.viewport.width;
  let text_fmt = &ctx.text_format;

  // Set up annotations
  let mut annotations = TextAnnotations::default();
  if !ctx.inline_annotations.is_empty() {
    let _ = annotations.add_inline_annotations(&ctx.inline_annotations, None);
  }
  if !ctx.overlay_annotations.is_empty() {
    let _ = annotations.add_overlay(&ctx.overlay_annotations, None);
  }

  let (doc, render_cache) = ctx.editor.document_and_cache();

  // Build the render plan (with or without syntax highlighting)
  if let (Some(loader), Some(syntax)) = (&ctx.loader, doc.syntax()) {
    // Calculate line range for highlighting
    let line_range = view.scroll.row..(view.scroll.row + view.viewport.height as usize);

    // Create syntax highlight adapter
    let mut adapter = SyntaxHighlightAdapter::new(
      doc.text().slice(..),
      syntax,
      loader.as_ref(),
      &mut ctx.highlight_cache,
      line_range,
      doc.version(),
      1, // syntax version (simplified)
    );

    build_plan(
      doc,
      view,
      text_fmt,
      &mut annotations,
      &mut adapter,
      render_cache,
      styles,
    )
  } else {
    // No syntax highlighting available
    let mut highlights = NoHighlights;
    build_plan(
      doc,
      view,
      text_fmt,
      &mut annotations,
      &mut highlights,
      render_cache,
      styles,
    )
  }
}

/// Render the current document state to the terminal.
pub fn render(ctx: &mut Ctx, terminal: &mut Terminal) -> Result<()> {
  let plan = render_plan(ctx);

  // Clear and draw
  terminal.clear()?;

  // Draw text lines with syntax colors
  for line in &plan.lines {
    for span in &line.spans {
      let fg = span.highlight.map(highlight_to_color);
      terminal.draw_str(line.row, span.col, &span.text, fg, None)?;
    }
  }

  // Draw cursors
  if let Some(cursor) = plan.cursors.first() {
    terminal.set_cursor(cursor.pos.row as u16, cursor.pos.col as u16)?;
  } else {
    terminal.hide_cursor()?;
  }

  // Draw secondary cursors (for multiple cursor support)
  for cursor in plan.cursors.iter().skip(1) {
    // Draw a marker at secondary cursor positions
    terminal.draw_str(
      cursor.pos.row as u16,
      cursor.pos.col as u16,
      "|",
      Some(Color::DarkGrey),
      None,
    )?;
  }

  terminal.flush()?;
  Ok(())
}

/// Ensure cursor is visible by adjusting scroll if needed.
pub fn ensure_cursor_visible(ctx: &mut Ctx) {
  let doc = ctx.editor.document();
  let text = doc.text();
  let max = text.len_chars();

  // Get primary cursor position
  let Some(range) = doc.selection().ranges().get(0).copied() else {
    return;
  };
  let clamped = Range::new(range.anchor.min(max), range.head.min(max));
  let cursor_pos = clamped.cursor(text.slice(..));
  let cursor_line = text.char_to_line(cursor_pos);
  let cursor_col = cursor_pos - text.line_to_char(cursor_line);

  let view = ctx.editor.view();
  let viewport_height = view.viewport.height as usize;
  let viewport_width = view.viewport.width as usize;

  // Vertical scrolling
  if cursor_line < view.scroll.row {
    ctx.editor.view_mut().scroll.row = cursor_line;
  } else if cursor_line >= view.scroll.row + viewport_height {
    ctx.editor.view_mut().scroll.row = cursor_line - viewport_height + 1;
  }

  // Horizontal scrolling
  if cursor_col < view.scroll.col {
    ctx.editor.view_mut().scroll.col = cursor_col;
  } else if cursor_col >= view.scroll.col + viewport_width {
    ctx.editor.view_mut().scroll.col = cursor_col - viewport_width + 1;
  }
}
