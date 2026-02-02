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

fn draw_box(buf: &mut Buffer, rect: Rect, border: Style, fill: Style) {
  if rect.width < 2 || rect.height < 2 {
    return;
  }

  fill_rect(buf, rect, fill);

  let top = "─".repeat((rect.width - 2) as usize);
  let bottom = top.clone();
  buf.set_string(rect.x + 1, rect.y, &top, border);
  buf.set_string(rect.x + 1, rect.y + rect.height - 1, &bottom, border);
  buf.set_string(rect.x, rect.y, "┌", border);
  buf.set_string(rect.x + rect.width - 1, rect.y, "┐", border);
  buf.set_string(rect.x, rect.y + rect.height - 1, "└", border);
  buf.set_string(rect.x + rect.width - 1, rect.y + rect.height - 1, "┘", border);

  for y in rect.y + 1..rect.y + rect.height - 1 {
    buf.set_string(rect.x, y, "│", border);
    buf.set_string(rect.x + rect.width - 1, y, "│", border);
  }
}

fn wrap_text(text: &str, max_width: usize, max_lines: usize) -> Vec<String> {
  if max_width == 0 || max_lines == 0 {
    return Vec::new();
  }

  let mut lines = Vec::new();
  let mut current = String::new();
  for word in text.split_whitespace() {
    let pending_len = if current.is_empty() {
      word.chars().count()
    } else {
      current.chars().count() + 1 + word.chars().count()
    };

    if pending_len > max_width && !current.is_empty() {
      lines.push(current);
      current = String::new();
      if lines.len() >= max_lines {
        break;
      }
    }

    if !current.is_empty() {
      current.push(' ');
    }
    current.push_str(word);
  }

  if lines.len() < max_lines && !current.is_empty() {
    lines.push(current);
  }

  if lines.len() > max_lines {
    lines.truncate(max_lines);
  }

  lines
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

  if let Some(selected_item) = state
    .selected
    .or_else(|| command_palette_default_selected(state))
    .and_then(|sel| filtered.iter().position(|&idx| idx == sel).map(|row| filtered[row]))
    .and_then(|idx| state.items.get(idx))
  {
    if let Some(description) = selected_item.description.as_ref().filter(|s| !s.is_empty()) {
      let available_height = panel.y.saturating_sub(area.y);
      let max_width = panel.width.saturating_sub(2) as usize;
      let text = format!("{} — {}", selected_item.title, description);
      let lines = wrap_text(&text, max_width, 3);
      let help_height = (lines.len() as u16).saturating_add(2);
      if help_height > 0 && help_height + 1 <= available_height {
        let help_y = panel.y - help_height - 1;
        let help_rect = Rect::new(panel.x, help_y, panel.width, help_height);
        let border_style = Style::default().fg(lib_color_to_ratatui(theme.panel_border));
        let fill_style = Style::default().bg(lib_color_to_ratatui(theme.panel_bg));
        draw_box(buf, help_rect, border_style, fill_style);

        let text_style = Style::default().fg(lib_color_to_ratatui(theme.text));
        for (idx, line) in lines.iter().enumerate() {
          buf.set_string(help_rect.x + 1, help_rect.y + 1 + idx as u16, line, text_style);
        }
      }
    }
  }

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
  let filtered = command_palette_filtered_indices(state);

  let padding_x: u16 = 2;
  let padding_y: u16 = 1;
  let header_height: u16 = 1;
  let divider_height: u16 = 1;
  let row_height: u16 = 1;
  let min_width: u16 = 48;

  let available_height = area.height.saturating_sub(2);
  let panel_height = (available_height * 2 / 3).max(8).min(available_height);
  let max_rows = panel_height
    .saturating_sub(padding_y * 2 + header_height + divider_height)
    .max(1) as usize;

  if max_rows == 0 {
    return;
  }

  let max_title = filtered
    .iter()
    .map(|&idx| state.items[idx].title.len() as u16)
    .max()
    .unwrap_or(0);
  let max_shortcut = filtered
    .iter()
    .filter_map(|&idx| state.items[idx].shortcut.as_ref())
    .map(|s| s.len() as u16)
    .max()
    .unwrap_or(0);

  let max_width = area.width.saturating_sub(4).max(min_width);
  let content_width = max_title
    .saturating_add(if max_shortcut > 0 { max_shortcut + 4 } else { 0 });
  let panel_width = (content_width + padding_x * 2 + 1)
    .max(min_width)
    .min(max_width);

  let panel_x = area.x + (area.width.saturating_sub(panel_width)) / 2;
  let panel_y = area.y + (area.height.saturating_sub(panel_height)) / 2;
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
  let visible_rows = max_rows.min(filtered.len());
  let selected_item = state
    .selected
    .or_else(|| command_palette_default_selected(state));
  let selected_row = selected_item.and_then(|sel| {
    filtered.iter().position(|&idx| idx == sel)
  });

  let scroll_offset = if let Some(sel) = selected_row {
    if sel >= visible_rows {
      sel + 1 - visible_rows
    } else {
      0
    }
  } else {
    0
  };

  let visible = filtered
    .iter()
    .skip(scroll_offset)
    .take(visible_rows);

  for (row_idx, item_idx) in visible.enumerate() {
    let row_y = list_start + row_idx as u16;
    let is_selected = selected_row == Some(row_idx + scroll_offset);

    if is_selected {
      fill_rect(
        buf,
        Rect::new(panel_x + 1, row_y, panel_width.saturating_sub(2), row_height),
        Style::default().bg(lib_color_to_ratatui(theme.selected_bg)),
      );
    }

    let row_style = if is_selected {
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

    if let Some(shortcut) = state.items[*item_idx].shortcut.as_ref() {
      let shortcut_style = if is_selected {
        Style::default().fg(lib_color_to_ratatui(theme.selected_text))
      } else {
        Style::default().fg(lib_color_to_ratatui(theme.placeholder))
      };
      let shortcut_x = panel_x
        .saturating_add(panel_width)
        .saturating_sub(padding_x + 1 + shortcut.len() as u16);
      if shortcut_x > panel_x + padding_x {
        buf.set_string(shortcut_x, row_y, shortcut, shortcut_style);
      }
    }
  }

  if filtered.len() > visible_rows {
    let track_x = panel_x + panel_width - 1;
    let track_height = visible_rows as u16;
    let thumb_height = ((visible_rows as f32 / filtered.len() as f32) * track_height as f32)
      .ceil()
      .max(1.0) as u16;
    let max_scroll = filtered.len().saturating_sub(visible_rows);
    let thumb_offset = if max_scroll == 0 {
      0
    } else {
      ((scroll_offset as f32 / max_scroll as f32) * (track_height - thumb_height) as f32)
        .round() as u16
    };
    for i in 0..track_height {
      let y = list_start + i;
      let is_thumb = i >= thumb_offset && i < thumb_offset + thumb_height;
      let symbol = if is_thumb { "█" } else { "│" };
      let style = Style::default().fg(lib_color_to_ratatui(theme.divider));
      buf.set_string(track_x, y, symbol, style);
    }
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
