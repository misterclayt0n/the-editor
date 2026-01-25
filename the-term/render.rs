//! Rendering - converts RenderPlan to terminal draw calls.

use crossterm::style::Color;
use eyre::Result;
use the_lib::render::{
  NoHighlights,
  RenderCache,
  RenderStyles,
  build_plan,
  graphics::Style,
  text_annotations::TextAnnotations,
  text_format::TextFormat,
};

use crate::{
  Ctx,
  terminal::Terminal,
};

/// Render the current document state to the terminal.
pub fn render(ctx: &mut Ctx, terminal: &mut Terminal) -> Result<()> {
  let doc = ctx.editor.document(ctx.active_doc).unwrap();

  // Set up text formatting
  let mut text_fmt = TextFormat::default();
  text_fmt.viewport_width = ctx.view.viewport.width;

  // Set up annotations (none for now)
  let mut annotations = TextAnnotations::default();

  // Set up highlights (none for now - will add syntax later)
  let mut highlights = NoHighlights;

  // Render cache
  let mut cache = RenderCache::default();

  // Styles for cursor and selection
  let styles = RenderStyles {
    selection:     Style::default(),
    cursor:        Style::default(),
    active_cursor: Style::default(),
  };

  // Build the render plan
  let plan = build_plan(
    doc,
    ctx.view,
    &text_fmt,
    &mut annotations,
    &mut highlights,
    &mut cache,
    styles,
  );

  // Clear and draw
  terminal.clear()?;

  // Draw text lines
  for line in &plan.lines {
    for span in &line.spans {
      let fg = span.highlight.map(|_h| {
        // For now, just use default color
        // Will add proper highlight mapping when we add syntax
        Color::Reset
      });
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
  // In a real implementation, we'd draw these differently (e.g., different color)
  for cursor in plan.cursors.iter().skip(1) {
    // Draw a marker at secondary cursor positions
    // For now, just draw a pipe character
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
  let doc = ctx.editor.document(ctx.active_doc).unwrap();
  let text = doc.text();

  // Get primary cursor position
  let cursor_pos = doc.selection().ranges()[0].cursor(text.slice(..));
  let cursor_line = text.char_to_line(cursor_pos);
  let cursor_col = cursor_pos - text.line_to_char(cursor_line);

  let viewport_height = ctx.view.viewport.height as usize;
  let viewport_width = ctx.view.viewport.width as usize;

  // Vertical scrolling
  if cursor_line < ctx.view.scroll.row {
    ctx.view.scroll.row = cursor_line;
  } else if cursor_line >= ctx.view.scroll.row + viewport_height {
    ctx.view.scroll.row = cursor_line - viewport_height + 1;
  }

  // Horizontal scrolling
  if cursor_col < ctx.view.scroll.col {
    ctx.view.scroll.col = cursor_col;
  } else if cursor_col >= ctx.view.scroll.col + viewport_width {
    ctx.view.scroll.col = cursor_col - viewport_width + 1;
  }
}
