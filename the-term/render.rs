//! Rendering - converts RenderPlan to ratatui draw calls.

use ratatui::{
  prelude::*,
  widgets::Clear,
};
use the_default::{
  CommandPaletteLayout,
  command_palette_default_selected,
  command_palette_filtered_indices,
  render_plan,
};
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
  theme::highlight_to_color,
};

fn lib_color_to_ratatui(color: the_lib::render::graphics::Color) -> Color {
  use the_lib::render::graphics::Color as LibColor;
  match color {
    LibColor::Reset => Color::Reset,
    LibColor::Black => Color::Black,
    LibColor::Red => Color::Red,
    LibColor::Green => Color::Green,
    LibColor::Yellow => Color::Yellow,
    LibColor::Blue => Color::Blue,
    LibColor::Magenta => Color::Magenta,
    LibColor::Cyan => Color::Cyan,
    LibColor::Gray => Color::DarkGray,
    LibColor::LightRed => Color::LightRed,
    LibColor::LightGreen => Color::LightGreen,
    LibColor::LightYellow => Color::LightYellow,
    LibColor::LightBlue => Color::LightBlue,
    LibColor::LightMagenta => Color::LightMagenta,
    LibColor::LightCyan => Color::LightCyan,
    LibColor::LightGray => Color::Gray,
    LibColor::White => Color::White,
    LibColor::Rgb(r, g, b) => Color::Rgb(r, g, b),
    LibColor::Indexed(idx) => Color::Indexed(idx),
  }
}

fn fill_rect(buf: &mut Buffer, rect: Rect, style: Style) {
  if rect.width == 0 || rect.height == 0 {
    return;
  }
  let line = " ".repeat(rect.width as usize);
  for y in rect.y..rect.y + rect.height {
    buf.set_string(rect.x, y, &line, style);
  }
}

fn draw_command_palette(f: &mut Frame, area: Rect, ctx: &mut Ctx) {
  let state = &ctx.command_palette;
  if !state.is_open {
    return;
  }

  let style = &ctx.command_palette_style;
  match style.layout {
    CommandPaletteLayout::Bottom => draw_command_palette_bottom(f, area, ctx),
    CommandPaletteLayout::Top => draw_command_palette_top(f, area, ctx),
    CommandPaletteLayout::Floating => draw_command_palette_floating(f, area, ctx),
    CommandPaletteLayout::Custom => {},
  }
}

fn draw_command_palette_bottom(f: &mut Frame, area: Rect, ctx: &mut Ctx) {
  let state = &ctx.command_palette;
  let theme = ctx.command_palette_style.theme;
  let filtered = command_palette_filtered_indices(state);

  let row_height: u16 = 1;
  let divider_height: u16 = 1;
  let input_height: u16 = 1;
  let list_rows = filtered.len() as u16;
  let panel_height = list_rows
    .saturating_add(divider_height)
    .saturating_add(input_height)
    .min(area.height);

  if panel_height == 0 {
    return;
  }

  let panel_y = area.y + area.height.saturating_sub(panel_height);
  let panel = Rect::new(area.x, panel_y, area.width, panel_height);

  let buf = f.buffer_mut();
  fill_rect(
    buf,
    panel,
    Style::default().bg(lib_color_to_ratatui(theme.panel_bg)),
  );

  let input_row = panel_y + panel_height - 1;
  let divider_row = input_row.saturating_sub(1);
  let list_start = panel_y;

  let placeholder = "Execute a command...";
  let (input_text, input_style) = if state.query.is_empty() {
    (
      format!(":{placeholder}"),
      Style::default().fg(lib_color_to_ratatui(theme.placeholder)),
    )
  } else {
    (
      format!(":{}", state.query),
      Style::default().fg(lib_color_to_ratatui(theme.text)),
    )
  };

  buf.set_string(panel.x + 1, input_row, input_text, input_style);

  if divider_row >= panel_y {
    let divider_style = Style::default().fg(lib_color_to_ratatui(theme.divider));
    let line = "─".repeat(panel.width as usize);
    buf.set_string(panel.x, divider_row, &line, divider_style);
  }

  let selected_item = state
    .selected
    .or_else(|| command_palette_default_selected(state));
  let selected_row = selected_item.and_then(|sel| {
    filtered.iter().position(|&idx| idx == sel)
  });

  for (row_idx, item_idx) in filtered.iter().enumerate() {
    let row_y = list_start + row_idx as u16;
    if row_y >= divider_row {
      break;
    }

    if selected_row == Some(row_idx) {
      fill_rect(
        buf,
        Rect::new(panel.x, row_y, panel.width, row_height),
        Style::default().bg(lib_color_to_ratatui(theme.selected_bg)),
      );
    }

    let row_style = if selected_row == Some(row_idx) {
      Style::default().fg(lib_color_to_ratatui(theme.selected_text))
    } else {
      Style::default().fg(lib_color_to_ratatui(theme.text))
    };
    buf.set_string(
      panel.x + 1,
      row_y,
      &state.items[*item_idx].title,
      row_style,
    );
  }

  // Place cursor after ':' + query.
  let cursor_col = panel
    .x
    .saturating_add(1)
    .saturating_add(1 + state.query.chars().count() as u16);
  if cursor_col < panel.x + panel.width {
    f.set_cursor(cursor_col, input_row);
  }
}

fn draw_command_palette_top(f: &mut Frame, area: Rect, ctx: &mut Ctx) {
  let state = &ctx.command_palette;
  let theme = ctx.command_palette_style.theme;
  let filtered = command_palette_filtered_indices(state);

  let row_height: u16 = 1;
  let divider_height: u16 = 1;
  let input_height: u16 = 1;
  let list_rows = filtered.len() as u16;
  let panel_height = list_rows
    .saturating_add(divider_height)
    .saturating_add(input_height)
    .min(area.height);

  if panel_height == 0 {
    return;
  }

  let panel = Rect::new(area.x, area.y, area.width, panel_height);
  let input_row = panel.y;
  let divider_row = input_row.saturating_add(1);
  let list_start = divider_row.saturating_add(divider_height);

  let buf = f.buffer_mut();
  fill_rect(
    buf,
    panel,
    Style::default().bg(lib_color_to_ratatui(theme.panel_bg)),
  );

  let placeholder = "Execute a command...";
  let (input_text, input_style) = if state.query.is_empty() {
    (
      format!(":{placeholder}"),
      Style::default().fg(lib_color_to_ratatui(theme.placeholder)),
    )
  } else {
    (
      format!(":{}", state.query),
      Style::default().fg(lib_color_to_ratatui(theme.text)),
    )
  };

  buf.set_string(panel.x + 1, input_row, input_text, input_style);

  if divider_row < panel.y + panel.height {
    let divider_style = Style::default().fg(lib_color_to_ratatui(theme.divider));
    let line = "─".repeat(panel.width as usize);
    buf.set_string(panel.x, divider_row, &line, divider_style);
  }

  let selected_item = state
    .selected
    .or_else(|| command_palette_default_selected(state));
  let selected_row = selected_item.and_then(|sel| {
    filtered.iter().position(|&idx| idx == sel)
  });

  for (row_idx, item_idx) in filtered.iter().enumerate() {
    let row_y = list_start + row_idx as u16;
    if row_y >= panel.y + panel.height {
      break;
    }

    if selected_row == Some(row_idx) {
      fill_rect(
        buf,
        Rect::new(panel.x, row_y, panel.width, row_height),
        Style::default().bg(lib_color_to_ratatui(theme.selected_bg)),
      );
    }

    let row_style = if selected_row == Some(row_idx) {
      Style::default().fg(lib_color_to_ratatui(theme.selected_text))
    } else {
      Style::default().fg(lib_color_to_ratatui(theme.text))
    };
    buf.set_string(
      panel.x + 1,
      row_y,
      &state.items[*item_idx].title,
      row_style,
    );
  }

  let cursor_col = panel
    .x
    .saturating_add(1)
    .saturating_add(1 + state.query.chars().count() as u16);
  if cursor_col < panel.x + panel.width {
    f.set_cursor(cursor_col, input_row);
  }
}

fn draw_command_palette_floating(f: &mut Frame, area: Rect, ctx: &mut Ctx) {
  let state = &ctx.command_palette;
  let theme = ctx.command_palette_style.theme;
  let mut filtered = command_palette_filtered_indices(state);

  let padding_x: u16 = 2;
  let padding_y: u16 = 1;
  let header_height: u16 = 1;
  let divider_height: u16 = 1;
  let row_height: u16 = 1;
  let min_width: u16 = 24;

  let max_rows = {
    let available = area
      .height
      .saturating_sub(padding_y * 2 + header_height + divider_height);
    (available / row_height) as usize
  };

  if max_rows == 0 {
    return;
  }

  if filtered.len() > max_rows {
    filtered.truncate(max_rows);
  }

  let max_title = filtered
    .iter()
    .map(|&idx| state.items[idx].title.len() as u16)
    .max()
    .unwrap_or(0);

  let max_width = area.width.saturating_sub(4).max(min_width);
  let panel_width = (max_title + padding_x * 2).max(min_width).min(max_width);
  let panel_height =
    padding_y * 2 + header_height + divider_height + row_height * filtered.len() as u16;

  let panel_x = area.x + (area.width.saturating_sub(panel_width)) / 2;
  let panel_y = area.y + 1;
  let panel = Rect::new(panel_x, panel_y, panel_width, panel_height);

  let buf = f.buffer_mut();
  fill_rect(
    buf,
    panel,
    Style::default().bg(lib_color_to_ratatui(theme.panel_bg)),
  );

  let placeholder = "Execute a command...";
  let (input_text, input_style) = if state.query.is_empty() {
    (
      placeholder.to_string(),
      Style::default().fg(lib_color_to_ratatui(theme.placeholder)),
    )
  } else {
    (
      state.query.clone(),
      Style::default().fg(lib_color_to_ratatui(theme.text)),
    )
  };

  let input_row = panel_y + padding_y;
  let input_col = panel_x + padding_x;
  buf.set_string(input_col, input_row, input_text, input_style);

  let divider_row = panel_y + padding_y + header_height;
  let divider_style = Style::default().fg(lib_color_to_ratatui(theme.divider));
  let line = "─".repeat(panel_width as usize);
  buf.set_string(panel_x, divider_row, &line, divider_style);

  let list_start = divider_row + divider_height;
  let selected_item = state
    .selected
    .or_else(|| command_palette_default_selected(state));
  let selected_row = selected_item.and_then(|sel| {
    filtered.iter().position(|&idx| idx == sel)
  });

  for (row_idx, item_idx) in filtered.iter().enumerate() {
    let row_y = list_start + row_idx as u16;

    if selected_row == Some(row_idx) {
      fill_rect(
        buf,
        Rect::new(panel_x + 1, row_y, panel_width.saturating_sub(2), row_height),
        Style::default().bg(lib_color_to_ratatui(theme.selected_bg)),
      );
    }

    let row_style = if selected_row == Some(row_idx) {
      Style::default().fg(lib_color_to_ratatui(theme.selected_text))
    } else {
      Style::default().fg(lib_color_to_ratatui(theme.text))
    };

    buf.set_string(
      panel_x + padding_x,
      row_y,
      &state.items[*item_idx].title,
      row_style,
    );
  }

  let cursor_col = panel_x
    .saturating_add(padding_x)
    .saturating_add(state.query.chars().count() as u16);
  if cursor_col < panel_x + panel_width {
    f.set_cursor(cursor_col, input_row);
  }
}

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
pub fn render(f: &mut Frame, ctx: &mut Ctx) {
  let plan = render_plan(ctx);

  let area = f.size();
  f.render_widget(Clear, area);

  {
    let buf = f.buffer_mut();

    // Draw text lines with syntax colors
    for line in &plan.lines {
      let y = area.y + line.row;
      if y >= area.y + area.height {
        continue;
      }
      for span in &line.spans {
        let x = area.x + span.col;
        if x >= area.x + area.width {
          continue;
        }
        let fg = span.highlight.map(highlight_to_color);
        let style = if let Some(fg) = fg {
          Style::default().fg(fg)
        } else {
          Style::default()
        };
        buf.set_string(x, y, span.text.as_str(), style);
      }
    }

    // Draw secondary cursors
    for cursor in plan.cursors.iter().skip(1) {
      let x = area.x + cursor.pos.col as u16;
      let y = area.y + cursor.pos.row as u16;
      if x < area.x + area.width && y < area.y + area.height {
        buf.set_string(x, y, "|", Style::default().fg(Color::DarkGray));
      }
    }
  }

  // Draw command palette (client-rendered, data+intent)
  let palette_open = ctx.command_palette.is_open;
  draw_command_palette(f, area, ctx);

  // Draw cursor last so it sits above any text, unless palette owns it.
  if !palette_open {
    if let Some(cursor) = plan.cursors.first() {
      let x = area.x + cursor.pos.col as u16;
      let y = area.y + cursor.pos.row as u16;
      if x < area.x + area.width && y < area.y + area.height {
        f.set_cursor(x, y);
      }
    }
  }
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
