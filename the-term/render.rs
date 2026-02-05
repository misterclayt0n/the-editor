//! Rendering - converts RenderPlan to ratatui draw calls.

use ratatui::{
  prelude::*,
  style::Modifier,
  widgets::Clear,
};
use the_default::{
  render_plan,
  ui_tree,
};
use the_lib::render::{
  LayoutIntent,
  UiAxis,
  UiContainer,
  UiEmphasis,
  UiInput,
  UiLayer,
  UiList,
  UiLayout,
  UiNode,
  UiPanel,
  UiStatusBar,
  UiText,
  UiTooltip,
  UiStyle,
  UiTree,
  graphics::{
    Modifier as LibModifier,
    Style as LibStyle,
    UnderlineStyle as LibUnderlineStyle,
  },
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

use crate::Ctx;

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

fn lib_modifier_to_ratatui(mods: LibModifier) -> Modifier {
  let mut out = Modifier::empty();
  if mods.contains(LibModifier::BOLD) {
    out.insert(Modifier::BOLD);
  }
  if mods.contains(LibModifier::DIM) {
    out.insert(Modifier::DIM);
  }
  if mods.contains(LibModifier::ITALIC) {
    out.insert(Modifier::ITALIC);
  }
  if mods.contains(LibModifier::SLOW_BLINK) {
    out.insert(Modifier::SLOW_BLINK);
  }
  if mods.contains(LibModifier::RAPID_BLINK) {
    out.insert(Modifier::RAPID_BLINK);
  }
  if mods.contains(LibModifier::REVERSED) {
    out.insert(Modifier::REVERSED);
  }
  if mods.contains(LibModifier::HIDDEN) {
    out.insert(Modifier::HIDDEN);
  }
  if mods.contains(LibModifier::CROSSED_OUT) {
    out.insert(Modifier::CROSSED_OUT);
  }
  out
}

fn lib_style_to_ratatui(style: LibStyle) -> Style {
  let mut out = Style::default();
  if let Some(fg) = style.fg {
    out = out.fg(lib_color_to_ratatui(fg));
  }
  if let Some(bg) = style.bg {
    out = out.bg(lib_color_to_ratatui(bg));
  }
  if let Some(underline) = style.underline_style {
    if !matches!(underline, LibUnderlineStyle::Reset) {
      out = out.add_modifier(Modifier::UNDERLINED);
    }
  }
  let add = lib_modifier_to_ratatui(style.add_modifier);
  let sub = lib_modifier_to_ratatui(style.sub_modifier);
  out = out.add_modifier(add);
  out = out.remove_modifier(sub);
  out
}

fn render_styles_from_theme(theme: &the_lib::render::theme::Theme) -> RenderStyles {
  let selection = theme.try_get("ui.selection").unwrap_or_default();
  let cursor = theme.try_get("ui.cursor").unwrap_or_default();
  let active_cursor = theme
    .try_get("ui.cursor.primary")
    .or_else(|| theme.try_get("ui.cursor"))
    .unwrap_or_default();
  RenderStyles {
    selection,
    cursor,
    active_cursor,
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

fn truncate_in_place(text: &mut String, max_chars: usize) {
  if max_chars == 0 {
    text.clear();
    return;
  }
  let mut count = 0usize;
  let mut cut = None;
  for (idx, _) in text.char_indices() {
    if count == max_chars {
      cut = Some(idx);
      break;
    }
    count += 1;
  }
  if let Some(cut) = cut {
    text.truncate(cut);
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

fn inner_rect(rect: Rect) -> Rect {
  if rect.width < 2 || rect.height < 2 {
    return rect;
  }
  Rect::new(rect.x + 1, rect.y + 1, rect.width - 2, rect.height - 2)
}

fn inset_rect(rect: Rect, insets: the_lib::render::UiInsets) -> Rect {
  let x = rect.x.saturating_add(insets.left);
  let y = rect.y.saturating_add(insets.top);
  let width = rect
    .width
    .saturating_sub(insets.left.saturating_add(insets.right));
  let height = rect
    .height
    .saturating_sub(insets.top.saturating_add(insets.bottom));
  Rect::new(x, y, width, height)
}

fn align_horizontal(rect: Rect, child_width: u16, align: the_lib::render::UiAlign) -> (u16, u16) {
  let width = match align {
    the_lib::render::UiAlign::Stretch => rect.width,
    _ => child_width.min(rect.width).max(1),
  };
  let x = match align {
    the_lib::render::UiAlign::Center => {
      rect.x + (rect.width.saturating_sub(width)) / 2
    },
    the_lib::render::UiAlign::End => rect.x + rect.width.saturating_sub(width),
    _ => rect.x,
  };
  (x, width)
}

fn align_vertical(rect: Rect, child_height: u16, align: the_lib::render::UiAlign) -> (u16, u16) {
  let height = match align {
    the_lib::render::UiAlign::Stretch => rect.height,
    _ => child_height.min(rect.height).max(1),
  };
  let y = match align {
    the_lib::render::UiAlign::Center => {
      rect.y + (rect.height.saturating_sub(height)) / 2
    },
    the_lib::render::UiAlign::End => rect.y + rect.height.saturating_sub(height),
    _ => rect.y,
  };
  (y, height)
}

fn resolve_ui_color(color: &the_lib::render::UiColor) -> Option<Color> {
  match color {
    the_lib::render::UiColor::Value(value) => Some(lib_color_to_ratatui(*value)),
    the_lib::render::UiColor::Token(_) => None,
  }
}

fn ui_style_colors(style: &UiStyle) -> (Style, Style, Style) {
  let text_color = style.fg.as_ref().and_then(resolve_ui_color);
  let bg_color = style.bg.as_ref().and_then(resolve_ui_color);
  let border_color = style.border.as_ref().and_then(resolve_ui_color);

  let mut text_style = Style::default();
  if let Some(color) = text_color {
    text_style = text_style.fg(color);
  }

  let mut fill_style = Style::default();
  if let Some(color) = bg_color {
    fill_style = fill_style.bg(color);
  }

  let mut border_style = Style::default();
  if let Some(color) = border_color {
    border_style = border_style.fg(color);
  }

  (text_style, fill_style, border_style)
}

fn apply_ui_emphasis(style: Style, emphasis: UiEmphasis) -> Style {
  match emphasis {
    UiEmphasis::Muted => style.add_modifier(Modifier::DIM),
    UiEmphasis::Strong => style.add_modifier(Modifier::BOLD),
    UiEmphasis::Normal => style,
  }
}

fn apply_constraints(
  mut width: u16,
  mut height: u16,
  constraints: &the_lib::render::UiConstraints,
  max_width: u16,
  max_height: u16,
) -> (u16, u16) {
  if let Some(min_w) = constraints.min_width {
    width = width.max(min_w);
  }
  if let Some(max_w) = constraints.max_width {
    width = width.min(max_w);
  }
  width = width.min(max_width).max(1);

  if let Some(min_h) = constraints.min_height {
    height = height.max(min_h);
  }
  if let Some(max_h) = constraints.max_height {
    height = height.min(max_h);
  }
  height = height.min(max_height).max(1);

  (width, height)
}

fn measure_node(node: &UiNode, max_width: u16) -> (u16, u16) {

  match node {
    UiNode::Text(text) => {
      let mut width = 0u16;
      let mut height = 0u16;
      let max_lines = text.max_lines.unwrap_or(u16::MAX) as usize;
      for line in text.content.lines() {
        let line_len = line.chars().count() as u16;
        if text.clip {
          width = width.max(line_len);
          height = height.saturating_add(1);
        } else if max_width > 0 {
          width = width.max(max_width);
          let wrapped = ((line_len as usize + max_width as usize - 1) / max_width as usize)
            .max(1);
          height = height.saturating_add(wrapped as u16);
        } else {
          width = width.max(line_len);
          height = height.saturating_add(1);
        }
        if height as usize >= max_lines {
          height = max_lines as u16;
          break;
        }
      }
      (width.min(max_width), height.max(1))
    },
    UiNode::Input(input) => {
      let mut width = input.value.chars().count();
      if let Some(placeholder) = input.placeholder.as_ref() {
        width = width.max(placeholder.chars().count());
      }
      (width.min(max_width as usize) as u16, 1)
    },
    UiNode::Divider(_) => (max_width, 1),
    UiNode::Spacer(spacer) => (max_width, spacer.size.max(1)),
    UiNode::List(list) => {
      let mut width: usize = 0;
      let mut has_detail = false;
      for item in &list.items {
        let mut w = item.title.chars().count();
        if let Some(shortcut) = item.shortcut.as_ref() {
          w = w.saturating_add(shortcut.chars().count() + 3);
        }
        if let Some(detail) = item
          .subtitle
          .as_deref()
          .filter(|s| !s.is_empty())
          .or_else(|| item.description.as_deref().filter(|s| !s.is_empty()))
        {
          has_detail = true;
          w = w.max(detail.chars().count());
        }
        width = width.max(w);
      }
      let width = if list.fill_width {
        max_width
      } else {
        width.min(max_width as usize).max(1) as u16
      };
      let base_height = if has_detail { 2 } else { 1 };
      let row_height = base_height;
      let mut count = list.items.len().max(1);
      if let Some(max_visible) = list.max_visible {
        count = count.min(max_visible.max(1));
      }
      let count = count as u16;
      let total_height = count.saturating_mul(row_height as u16);
      (width, total_height)
    },
    UiNode::Container(container) => match &container.layout {
      UiLayout::Stack { axis, gap } => match axis {
        UiAxis::Vertical => {
          let mut height = 0u16;
          let mut width = 0u16;
          for (idx, child) in container.children.iter().enumerate() {
            let (cw, ch) = measure_node(child, max_width);
            width = width.max(cw);
            height = height.saturating_add(ch);
            if idx + 1 < container.children.len() {
              height = height.saturating_add(*gap);
            }
          }
          let width = width.saturating_add(container.constraints.padding.horizontal());
          let height = height.saturating_add(container.constraints.padding.vertical());
          apply_constraints(width, height.max(1), &container.constraints, max_width, u16::MAX)
        },
        UiAxis::Horizontal => {
          let mut width = 0u16;
          let mut height = 0u16;
          for (idx, child) in container.children.iter().enumerate() {
            let (cw, ch) = measure_node(child, max_width);
            width = width.saturating_add(cw);
            height = height.max(ch);
            if idx + 1 < container.children.len() {
              width = width.saturating_add(*gap);
            }
          }
          let width = width.saturating_add(container.constraints.padding.horizontal());
          let height = height.saturating_add(container.constraints.padding.vertical());
          apply_constraints(width.max(1), height.max(1), &container.constraints, max_width, u16::MAX)
        },
      },
      UiLayout::Split { axis, .. } => match axis {
        UiAxis::Vertical => {
          let width = max_width.saturating_add(container.constraints.padding.horizontal());
          let height = container.children.len().max(1) as u16
            + container.constraints.padding.vertical();
          apply_constraints(width.max(1), height.max(1), &container.constraints, max_width, u16::MAX)
        },
        UiAxis::Horizontal => {
          let width = max_width.saturating_add(container.constraints.padding.horizontal());
          let height = 1 + container.constraints.padding.vertical();
          apply_constraints(width.max(1), height.max(1), &container.constraints, max_width, u16::MAX)
        },
      },
    },
    UiNode::Panel(panel) => {
      let max_width = max_content_width_for_intent(panel.intent.clone(), Rect::new(0, 0, max_width, 1), 0, 0);
      let (child_w, child_h) = measure_node(&panel.child, max_width);
      let width = child_w.saturating_add(panel.constraints.padding.horizontal());
      let height = child_h.saturating_add(panel.constraints.padding.vertical());
      apply_constraints(width.max(1), height.max(1), &panel.constraints, max_width, u16::MAX)
    },
    UiNode::Tooltip(tooltip) => {
      let width = tooltip.content.chars().count().saturating_add(2).min(max_width as usize) as u16;
      (width.max(2), 3)
    },
    UiNode::StatusBar(_) => (max_width, 1),
  }
}

fn layout_children<'a>(
  container: &'a UiContainer,
  rect: Rect,
) -> Vec<(Rect, &'a UiNode)> {
  let mut placements = Vec::new();
  let rect = inset_rect(rect, container.constraints.padding);

  match &container.layout {
    UiLayout::Stack { axis, gap } => match axis {
      UiAxis::Vertical => {
        let mut y = rect.y;
        for child in &container.children {
          let (child_w, h) = measure_node(child, rect.width);
          let height = h.min(rect.height.saturating_sub(y.saturating_sub(rect.y))).max(1);
          if height == 0 {
            break;
          }
          let (x, width) = align_horizontal(rect, child_w, container.constraints.align.horizontal);
          let child_rect = Rect::new(x, y, width, height);
          placements.push((child_rect, child));
          y = y.saturating_add(height).saturating_add(*gap);
          if y >= rect.y + rect.height {
            break;
          }
        }
      },
      UiAxis::Horizontal => {
        let mut x = rect.x;
        for child in &container.children {
          let (w, child_h) = measure_node(child, rect.width);
          let width = w.min(rect.width.saturating_sub(x.saturating_sub(rect.x))).max(1);
          if width == 0 {
            break;
          }
          let (y, height) = align_vertical(rect, child_h, container.constraints.align.vertical);
          let child_rect = Rect::new(x, y, width, height);
          placements.push((child_rect, child));
          x = x.saturating_add(width).saturating_add(*gap);
          if x >= rect.x + rect.width {
            break;
          }
        }
      },
    },
    UiLayout::Split { axis, ratios } => {
      let count = container.children.len().max(1);
      let mut ratios = ratios.clone();
      if ratios.len() < count {
        ratios.resize(count, 1);
      }
      let total: u16 = ratios.iter().sum();
      let total = if total == 0 { count as u16 } else { total };

      match axis {
        UiAxis::Vertical => {
          let mut y = rect.y;
          for (child, ratio) in container.children.iter().zip(ratios.iter()) {
            let height = rect
              .height
              .saturating_mul(*ratio)
              .saturating_div(total)
              .max(1);
            let (x, width) = align_horizontal(rect, rect.width, container.constraints.align.horizontal);
            let child_rect = Rect::new(x, y, width, height);
            placements.push((child_rect, child));
            y = y.saturating_add(height);
          }
        },
        UiAxis::Horizontal => {
          let mut x = rect.x;
          for (child, ratio) in container.children.iter().zip(ratios.iter()) {
            let width = rect
              .width
              .saturating_mul(*ratio)
              .saturating_div(total)
              .max(1);
            let (y, height) = align_vertical(rect, rect.height, container.constraints.align.vertical);
            let child_rect = Rect::new(x, y, width, height);
            placements.push((child_rect, child));
            x = x.saturating_add(width);
          }
        },
      }
    },
  }

  placements
}

fn draw_ui_text(buf: &mut Buffer, rect: Rect, _ctx: &Ctx, text: &UiText) {
  if rect.width == 0 || rect.height == 0 {
    return;
  }
  let (text_style, _, _) = ui_style_colors(&text.style);
  let style = apply_ui_emphasis(text_style, text.style.emphasis);
  let max_lines = text.max_lines.unwrap_or(u16::MAX) as usize;
  let mut drawn = 0usize;

  for line in text.content.lines() {
    if drawn >= max_lines {
      break;
    }

    if text.clip || rect.width == 0 {
      let y = rect.y + drawn as u16;
      if y >= rect.y + rect.height {
        break;
      }
      let mut truncated = line.to_string();
      truncate_in_place(&mut truncated, rect.width as usize);
      buf.set_string(rect.x, y, truncated, style);
      drawn += 1;
    } else {
      let mut chunk = String::new();
      for ch in line.chars() {
        if chunk.chars().count() >= rect.width as usize {
          let y = rect.y + drawn as u16;
          if y >= rect.y + rect.height {
            break;
          }
          buf.set_string(rect.x, y, chunk.clone(), style);
          drawn += 1;
          chunk.clear();
          if drawn >= max_lines {
            break;
          }
        }
        chunk.push(ch);
      }
      if !chunk.is_empty() && drawn < max_lines {
        let y = rect.y + drawn as u16;
        if y >= rect.y + rect.height {
          break;
        }
        buf.set_string(rect.x, y, chunk, style);
        drawn += 1;
      }
    }
  }
}

fn draw_ui_input(
  buf: &mut Buffer,
  rect: Rect,
  _ctx: &Ctx,
  input: &UiInput,
  focus: Option<&the_lib::render::UiFocus>,
  cursor_out: &mut Option<(u16, u16)>,
) {
  if rect.width == 0 || rect.height == 0 {
    return;
  }
  let (text_style, _, _) = ui_style_colors(&input.style);
  let placeholder_color = input
    .style
    .accent
    .as_ref()
    .and_then(resolve_ui_color)
    .or(text_style.fg);
  let (value, style) = if input.value.is_empty() {
    let placeholder = input
      .placeholder
      .as_deref()
      .unwrap_or("...");
    let mut style = Style::default();
    if let Some(color) = placeholder_color {
      style = style.fg(color);
    }
    (placeholder.to_string(), style)
  } else {
    (input.value.clone(), text_style)
  };
  let mut truncated = value;
  truncate_in_place(&mut truncated, rect.width as usize);
  buf.set_string(rect.x, rect.y, truncated, style);

  let is_focused = focus
    .map(|f| f.id == input.id)
    .unwrap_or(focus.is_none());
  if is_focused && cursor_out.is_none() {
    let cursor_pos = focus.and_then(|f| f.cursor).unwrap_or(input.cursor);
    let cursor_x = rect.x.saturating_add(cursor_pos as u16).min(rect.x + rect.width - 1);
    *cursor_out = Some((cursor_x, rect.y));
  }
}

fn draw_ui_list(
  buf: &mut Buffer,
  rect: Rect,
  _ctx: &Ctx,
  list: &UiList,
  _cursor_out: &mut Option<(u16, u16)>,
) {
  if rect.width == 0 || rect.height == 0 {
    return;
  }
  let (text_style, _, _) = ui_style_colors(&list.style);
  let base_text_color = text_style.fg;
  let selected_text_color = list
    .style
    .border
    .as_ref()
    .and_then(resolve_ui_color)
    .or(base_text_color);
  let selected_bg_color = list
    .style
    .accent
    .as_ref()
    .and_then(resolve_ui_color);
  let scroll_color = list
    .style
    .border
    .as_ref()
    .and_then(resolve_ui_color)
    .or(base_text_color);
  let has_detail = list.items.iter().any(|item| {
    item
      .subtitle
      .as_ref()
      .map_or(false, |s| !s.is_empty())
      || item
        .description
        .as_ref()
        .map_or(false, |s| !s.is_empty())
  });
  let base_height: usize = if has_detail { 2 } else { 1 };
  let row_gap: usize = 0;
  let row_height: usize = base_height + row_gap;
  let visible_rows = rect.height as usize;
  let mut visible_items = visible_rows / row_height;
  if let Some(max_visible) = list.max_visible {
    visible_items = visible_items.min(max_visible.max(1));
  }
  if visible_items == 0 {
    return;
  }
  let mut scroll_offset = list.scroll.min(list.items.len().saturating_sub(visible_items));
  let selected = list.selected;
  if let Some(sel) = selected {
    if sel < scroll_offset {
      scroll_offset = sel;
    } else if sel >= scroll_offset + visible_items {
      scroll_offset = sel + 1 - visible_items;
    }
  }
  let visible = list.items.iter().skip(scroll_offset).take(visible_items);

  for (row_idx, item) in visible.enumerate() {
    let y = rect.y + (row_idx * row_height) as u16;
    let is_selected = selected == Some(row_idx + scroll_offset);

    if is_selected {
      if let Some(bg_color) = selected_bg_color {
        fill_rect(
          buf,
          Rect::new(rect.x, y, rect.width, base_height as u16),
          Style::default().bg(bg_color),
        );
      }
    }

    let mut row_style = Style::default();
    let row_color = if is_selected {
      selected_text_color
    } else {
      base_text_color
    };
    if let Some(color) = row_color {
      row_style = row_style.fg(color);
    }
    if item.emphasis {
      row_style = row_style.add_modifier(Modifier::BOLD);
    }

    let mut title = item.title.clone();
    let shortcut = item.shortcut.clone().unwrap_or_default();
    let available_width = rect.width.saturating_sub(2) as usize;
    if !shortcut.is_empty() && shortcut.len() + 2 < available_width {
      let shortcut_width = shortcut.len() + 1;
      truncate_in_place(&mut title, available_width.saturating_sub(shortcut_width));
      let shortcut_x = rect.x + rect.width.saturating_sub(shortcut.len() as u16 + 1);
      buf.set_string(shortcut_x, y, shortcut, row_style);
    } else {
      truncate_in_place(&mut title, available_width);
    }
    buf.set_string(rect.x + 1, y, title, row_style);

    if base_height > 1 {
      let detail = item
        .subtitle
        .as_deref()
        .filter(|s| !s.is_empty())
        .or_else(|| item.description.as_deref().filter(|s| !s.is_empty()));
      if let Some(detail) = detail {
        let mut detail_text = detail.to_string();
        truncate_in_place(&mut detail_text, available_width);
        let mut detail_style = row_style;
        if !is_selected {
          detail_style = detail_style.add_modifier(Modifier::DIM);
        }
        buf.set_string(rect.x + 1, y + 1, detail_text, detail_style);
      }
    }
  }

  if list.items.len() > visible_items {
    let track_x = rect.x + rect.width - 1;
    let track_height = rect.height;
    let thumb_height = ((visible_items as f32 / list.items.len() as f32) * track_height as f32)
      .ceil()
      .max(1.0) as u16;
    let max_scroll = list.items.len().saturating_sub(visible_items);
    let thumb_offset = if max_scroll == 0 {
      0
    } else {
      ((scroll_offset as f32 / max_scroll as f32) * (track_height - thumb_height) as f32)
        .round() as u16
    };
    for i in 0..track_height {
      let y = rect.y + i;
      let is_thumb = i >= thumb_offset && i < thumb_offset + thumb_height;
      let symbol = if is_thumb { "█" } else { "│" };
      let mut style = Style::default();
      if let Some(color) = scroll_color {
        style = style.fg(color);
      }
      buf.set_string(track_x, y, symbol, style);
    }
  }
}

fn draw_ui_node(
  buf: &mut Buffer,
  rect: Rect,
  ctx: &Ctx,
  node: &UiNode,
  focus: Option<&the_lib::render::UiFocus>,
  cursor_out: &mut Option<(u16, u16)>,
) {
  if rect.width == 0 || rect.height == 0 {
    return;
  }
  match node {
    UiNode::Text(text) => draw_ui_text(buf, rect, ctx, text),
    UiNode::Input(input) => draw_ui_input(buf, rect, ctx, input, focus, cursor_out),
    UiNode::List(list) => draw_ui_list(buf, rect, ctx, list, cursor_out),
    UiNode::Divider(_) => {
      let line = "─".repeat(rect.width as usize);
      let style = Style::default().add_modifier(Modifier::DIM);
      buf.set_string(rect.x, rect.y, line, style);
    },
    UiNode::Spacer(_) => {},
    UiNode::Container(container) => {
      let placements = layout_children(container, rect);
      for (child_rect, child) in placements {
        draw_ui_node(buf, child_rect, ctx, child, focus, cursor_out);
      }
    },
    UiNode::Panel(panel) => {
      draw_ui_panel(buf, rect, ctx, panel, focus, cursor_out);
    },
    UiNode::Tooltip(tooltip) => {
      draw_ui_tooltip(buf, rect, ctx, tooltip);
    },
    UiNode::StatusBar(status) => {
      draw_ui_status_bar(buf, rect, ctx, status);
    },
  }
}

fn max_content_width_for_intent(intent: LayoutIntent, area: Rect, border: u16, padding_h: u16) -> u16 {
  let full = area
    .width
    .saturating_sub(border * 2 + padding_h)
    .max(1);
  match intent {
    LayoutIntent::Floating | LayoutIntent::Custom(_) => {
      let cap = area.width.saturating_mul(2) / 3;
      full.min(cap.max(20))
    },
    _ => full,
  }
}

fn draw_ui_panel(
  buf: &mut Buffer,
  area: Rect,
  ctx: &Ctx,
  panel: &UiPanel,
  focus: Option<&the_lib::render::UiFocus>,
  cursor_out: &mut Option<(u16, u16)>,
) {
  let boxed = panel.style.border.is_some();
  let border: u16 = if boxed { 1 } else { 0 };
  let padding = panel.constraints.padding;
  let padding_h = padding.horizontal();
  let padding_v = padding.vertical();
  let title_height = panel.title.is_some() as u16;

  let max_content_width = max_content_width_for_intent(panel.intent.clone(), area, border, padding_h);
  let (child_w, child_h) = measure_node(&panel.child, max_content_width);
  let mut panel_width = child_w
    .saturating_add(border * 2 + padding_h)
    .min(area.width)
    .max(10);
  let mut panel_height = child_h
    .saturating_add(border * 2 + padding_v + title_height)
    .min(area.height)
    .max(4);

  let (mut panel_width, panel_height) =
    apply_constraints(panel_width, panel_height, &panel.constraints, area.width, area.height);

  match panel.intent.clone() {
    LayoutIntent::Bottom => {
      let mut height = if boxed {
        panel_height.min(area.height).max(4)
      } else {
        let divider = flat_panel_divider(panel);
        let mut height = child_h
          .saturating_add(padding_v + divider)
          .min(area.height);
        if panel_is_statusline(panel) {
          height = height.max(1);
        } else {
          height = height.max(3);
        }
        height
      };
      if panel_is_statusline(panel) {
        height = height.min(area.height).max(1);
      } else {
        height = height.min(area.height).max(2);
      }
      let rect = Rect::new(area.x, area.y + area.height - height, area.width, height);
      if boxed {
        draw_box_with_title(buf, rect, ctx, panel, focus, cursor_out);
      } else {
        let content = draw_flat_panel(buf, rect, ctx, panel, BorderEdge::Top, !panel_is_statusline(panel));
        draw_ui_node(buf, content, ctx, &panel.child, focus, cursor_out);
      }
    },
    LayoutIntent::Top => {
      let mut height = if boxed {
        panel_height.min(area.height).max(4)
      } else {
        let divider = flat_panel_divider(panel);
        let mut height = child_h
          .saturating_add(padding_v + divider)
          .min(area.height);
        if panel_is_statusline(panel) {
          height = height.max(1);
        } else {
          height = height.max(3);
        }
        height
      };
      if panel_is_statusline(panel) {
        height = height.min(area.height).max(1);
      } else {
        height = height.min(area.height).max(2);
      }
      let rect = Rect::new(area.x, area.y, area.width, height);
      if boxed {
        draw_box_with_title(buf, rect, ctx, panel, focus, cursor_out);
      } else {
        let content = draw_flat_panel(buf, rect, ctx, panel, BorderEdge::Bottom, !panel_is_statusline(panel));
        draw_ui_node(buf, content, ctx, &panel.child, focus, cursor_out);
      }
    },
    LayoutIntent::SidebarLeft => {
      panel_width = (area.width / 3).max(panel_width.min(area.width));
      let rect = Rect::new(area.x, area.y, panel_width, area.height);
      draw_box_with_title(buf, rect, ctx, panel, focus, cursor_out);
    },
    LayoutIntent::SidebarRight => {
      panel_width = (area.width / 3).max(panel_width.min(area.width));
      let rect = Rect::new(area.x + area.width - panel_width, area.y, panel_width, area.height);
      draw_box_with_title(buf, rect, ctx, panel, focus, cursor_out);
    },
    LayoutIntent::Fullscreen => {
      let rect = area;
      draw_box_with_title(buf, rect, ctx, panel, focus, cursor_out);
    },
    LayoutIntent::Custom(_) | LayoutIntent::Floating => {
      let (x, width) = align_horizontal(area, panel_width, panel.constraints.align.horizontal);
      let (y, height) = align_vertical(area, panel_height, panel.constraints.align.vertical);
      let rect = Rect::new(x, y, width, height);
      draw_box_with_title(buf, rect, ctx, panel, focus, cursor_out);
    },
  }
}

fn draw_box_with_title(
  buf: &mut Buffer,
  rect: Rect,
  ctx: &Ctx,
  panel: &UiPanel,
  focus: Option<&the_lib::render::UiFocus>,
  cursor_out: &mut Option<(u16, u16)>,
) {
  let (text_style, fill_style, border_style) = ui_style_colors(&panel.style);
  draw_box(buf, rect, border_style, fill_style);

  let mut content = inner_rect(rect);
  if let Some(title) = panel.title.as_ref() {
    let mut truncated = title.clone();
    truncate_in_place(&mut truncated, content.width as usize);
    buf.set_string(content.x, content.y, truncated, text_style);
    content = Rect::new(content.x, content.y + 1, content.width, content.height.saturating_sub(1));
  }

  let content = inset_rect(content, panel.constraints.padding);
  draw_ui_node(buf, content, ctx, &panel.child, focus, cursor_out);
}

#[derive(Clone, Copy)]
enum BorderEdge {
  Top,
  Bottom,
}

fn panel_is_statusline(panel: &UiPanel) -> bool {
  panel.id == "statusline" || panel.style.role.as_deref() == Some("statusline")
}

fn flat_panel_divider(panel: &UiPanel) -> u16 {
  if panel_is_statusline(panel) {
    0
  } else {
    1
  }
}

fn draw_flat_panel(
  buf: &mut Buffer,
  rect: Rect,
  _ctx: &Ctx,
  panel: &UiPanel,
  edge: BorderEdge,
  draw_divider: bool,
) -> Rect {
  let (_, fill_style, border_style) = ui_style_colors(&panel.style);
  fill_rect(buf, rect, fill_style);

  let content = if draw_divider {
    let line = "─".repeat(rect.width as usize);
    let border_y = match edge {
      BorderEdge::Top => rect.y,
      BorderEdge::Bottom => rect.y + rect.height.saturating_sub(1),
    };
    buf.set_string(rect.x, border_y, &line, border_style);
    match edge {
      BorderEdge::Top => Rect::new(rect.x, rect.y + 1, rect.width, rect.height.saturating_sub(1)),
      BorderEdge::Bottom => Rect::new(rect.x, rect.y, rect.width, rect.height.saturating_sub(1)),
    }
  } else {
    rect
  };
  inset_rect(content, panel.constraints.padding)
}

fn node_layer(node: &UiNode) -> UiLayer {
  match node {
    UiNode::Panel(panel) => panel.layer,
    UiNode::Tooltip(_) => UiLayer::Tooltip,
    _ => UiLayer::Overlay,
  }
}

fn apply_ui_viewport(ctx: &mut Ctx, ui: &UiTree, area: Rect) {
  let mut reserved_bottom: u16 = 0;
  for node in &ui.overlays {
    let UiNode::Panel(panel) = node else {
      continue;
    };
    if panel.intent != LayoutIntent::Bottom || panel.layer == UiLayer::Tooltip {
      continue;
    }
    let available = area.height.saturating_sub(reserved_bottom);
    if available == 0 {
      break;
    }
    let rect_area = Rect::new(area.x, area.y, area.width, available);
    let height = panel_height_for_area(panel, rect_area);
    reserved_bottom = reserved_bottom.saturating_add(height);
  }

  let height = area.height.saturating_sub(reserved_bottom).max(1);
  let width = area.width.max(1);
  let view = ctx.editor.view_mut();
  if view.viewport.width != width || view.viewport.height != height {
    view.viewport = the_lib::render::graphics::Rect::new(0, 0, width, height);
  }
}

fn draw_ui_tooltip(buf: &mut Buffer, area: Rect, _ctx: &Ctx, tooltip: &UiTooltip) {
  if area.width == 0 || area.height == 0 {
    return;
  }
  let (text_style, fill_style, border_style) = ui_style_colors(&tooltip.style);
  let mut text = tooltip.content.clone();
  let max_width = area.width.saturating_sub(2).max(1) as usize;
  truncate_in_place(&mut text, max_width);
  let width = (text.chars().count() as u16).saturating_add(2).min(area.width).max(2);
  let height = 3u16.min(area.height).max(1);

  let rect = match tooltip.placement.clone() {
    LayoutIntent::Bottom => Rect::new(area.x, area.y + area.height - height, width, height),
    LayoutIntent::Top => Rect::new(area.x, area.y, width, height),
    LayoutIntent::SidebarLeft => Rect::new(area.x, area.y, width, height),
    LayoutIntent::SidebarRight => Rect::new(area.x + area.width - width, area.y, width, height),
    LayoutIntent::Fullscreen => Rect::new(area.x, area.y, width, height),
    LayoutIntent::Custom(_) | LayoutIntent::Floating => Rect::new(
      area.x + (area.width.saturating_sub(width)) / 2,
      area.y + (area.height.saturating_sub(height)) / 2,
      width,
      height,
    ),
  };

  draw_box(buf, rect, border_style, fill_style);
  let inner = inner_rect(rect);
  buf.set_string(inner.x, inner.y, text, text_style);
}

fn draw_ui_status_bar(buf: &mut Buffer, rect: Rect, _ctx: &Ctx, status: &UiStatusBar) {
  if rect.width == 0 || rect.height == 0 {
    return;
  }
  let (text_style, fill_style, _) = ui_style_colors(&status.style);
  fill_rect(buf, Rect::new(rect.x, rect.y, rect.width, 1), fill_style);

  let mut left = status.left.clone();
  let mut center = status.center.clone();
  let mut right = status.right.clone();
  truncate_in_place(&mut left, rect.width as usize);
  truncate_in_place(&mut right, rect.width as usize);
  truncate_in_place(&mut center, rect.width as usize);

  buf.set_string(rect.x, rect.y, left, text_style);
  if !center.is_empty() {
    let cx = rect.x + rect.width.saturating_sub(center.len() as u16) / 2;
    buf.set_string(cx, rect.y, center, text_style);
  }
  if !right.is_empty() {
    let rx = rect.x + rect.width.saturating_sub(right.len() as u16);
    buf.set_string(rx, rect.y, right, text_style);
  }
}

fn panel_height_for_area(panel: &UiPanel, area: Rect) -> u16 {
  let boxed = panel.style.border.is_some();
  if boxed {
    let border: u16 = 1;
    let padding = panel.constraints.padding;
    let padding_h = padding.horizontal();
    let padding_v = padding.vertical();
    let title_height = panel.title.is_some() as u16;
    let max_content_width = max_content_width_for_intent(panel.intent.clone(), area, border, padding_h);
    let (_, child_h) = measure_node(&panel.child, max_content_width);
    child_h
      .saturating_add(border * 2 + padding_v + title_height)
      .min(area.height)
      .max(4)
  } else {
    let padding_v = panel.constraints.padding.vertical();
    let max_content_width = max_content_width_for_intent(panel.intent.clone(), area, 0, 0);
    let (_, child_h) = measure_node(&panel.child, max_content_width);
    let divider = flat_panel_divider(panel);
    let height = child_h
      .saturating_add(divider + padding_v)
      .min(area.height);
    if panel_is_statusline(panel) {
      height.max(1)
    } else {
      height.max(2)
    }
  }
}

fn draw_panel_in_rect(
  buf: &mut Buffer,
  rect: Rect,
  ctx: &Ctx,
  panel: &UiPanel,
  edge: BorderEdge,
  focus: Option<&the_lib::render::UiFocus>,
  cursor_out: &mut Option<(u16, u16)>,
) {
  if rect.width == 0 || rect.height == 0 {
    return;
  }
  if panel.style.border.is_some() {
    draw_box_with_title(buf, rect, ctx, panel, focus, cursor_out);
  } else {
    let content = draw_flat_panel(buf, rect, ctx, panel, edge, !panel_is_statusline(panel));
    draw_ui_node(buf, content, ctx, &panel.child, focus, cursor_out);
  }
}

fn draw_ui_overlays(
  buf: &mut Buffer,
  area: Rect,
  ctx: &Ctx,
  ui: &UiTree,
  cursor_out: &mut Option<(u16, u16)>,
) {
  let mut top_offset: u16 = 0;
  let mut bottom_offset: u16 = 0;
  let focus = ui.focus.as_ref();
  let layers = [
    the_lib::render::UiLayer::Background,
    the_lib::render::UiLayer::Overlay,
    the_lib::render::UiLayer::Tooltip,
  ];
  for layer in layers {
    for node in ui.overlays.iter().filter(|node| node_layer(node) == layer) {
      match node {
      UiNode::Panel(panel) => match panel.intent.clone() {
          LayoutIntent::Bottom => {
            if matches!(layer, the_lib::render::UiLayer::Tooltip) {
              draw_ui_panel(buf, area, ctx, panel, focus, cursor_out);
              continue;
            }
            let available_height = area.height.saturating_sub(top_offset + bottom_offset);
            if available_height == 0 {
              continue;
            }
            let rect_area = Rect::new(area.x, area.y + top_offset, area.width, available_height);
            let panel_height = panel_height_for_area(panel, rect_area);
            let rect = Rect::new(
              area.x,
              area.y + area.height - bottom_offset - panel_height,
              area.width,
              panel_height,
            );
            bottom_offset = bottom_offset.saturating_add(panel_height);
            draw_panel_in_rect(buf, rect, ctx, panel, BorderEdge::Top, focus, cursor_out);
          },
          LayoutIntent::Top => {
            if matches!(layer, the_lib::render::UiLayer::Tooltip) {
              draw_ui_panel(buf, area, ctx, panel, focus, cursor_out);
              continue;
            }
            let available_height = area.height.saturating_sub(top_offset + bottom_offset);
            if available_height == 0 {
              continue;
            }
            let rect_area = Rect::new(area.x, area.y + top_offset, area.width, available_height);
            let panel_height = panel_height_for_area(panel, rect_area);
            let rect = Rect::new(area.x, area.y + top_offset, area.width, panel_height);
            top_offset = top_offset.saturating_add(panel_height);
            draw_panel_in_rect(buf, rect, ctx, panel, BorderEdge::Bottom, focus, cursor_out);
          },
          _ => draw_ui_panel(buf, area, ctx, panel, focus, cursor_out),
        },
        _ => draw_ui_node(buf, area, ctx, node, focus, cursor_out),
      }
    }
  }
}

pub fn build_render_plan(ctx: &mut Ctx) -> RenderPlan {
  let styles = render_styles_from_theme(&ctx.ui_theme);
  build_render_plan_with_styles(ctx, styles)
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
  let ui = ui_tree(ctx);
  apply_ui_viewport(ctx, &ui, f.size());
  ensure_cursor_visible(ctx);
  let plan = render_plan(ctx);

  let area = f.size();
  f.render_widget(Clear, area);

  let ui_cursor = {
    let buf = f.buffer_mut();
    let mut cursor_out = None;
    let base_text_style = lib_style_to_ratatui(ctx.ui_theme.try_get("ui.text").unwrap_or_default());
    if let Some(bg) = ctx.ui_theme.try_get("ui.background").and_then(|style| style.bg) {
      fill_rect(buf, area, Style::default().bg(lib_color_to_ratatui(bg)));
    }

    for selection in &plan.selections {
      let rect = Rect::new(
        area.x + selection.rect.x,
        area.y + selection.rect.y,
        selection.rect.width,
        selection.rect.height,
      );
      fill_rect(buf, rect, lib_style_to_ratatui(selection.style));
    }

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
        let style = span
          .highlight
          .map(|highlight| base_text_style.patch(lib_style_to_ratatui(ctx.ui_theme.highlight(highlight))))
          .unwrap_or(base_text_style);
        buf.set_string(x, y, span.text.as_str(), style);
      }
    }

    // Draw cursors
    for cursor in &plan.cursors {
      let x = area.x + cursor.pos.col as u16;
      let y = area.y + cursor.pos.row as u16;
      if x < area.x + area.width && y < area.y + area.height {
        let style = lib_style_to_ratatui(cursor.style);
        let cell = buf.get_mut(x, y);
        let merged = cell.style().patch(style);
        cell.set_style(merged);
      }
    }

    // Draw UI root and overlays.
    draw_ui_node(buf, area, ctx, &ui.root, ui.focus.as_ref(), &mut cursor_out);
    draw_ui_overlays(buf, area, ctx, &ui, &mut cursor_out);
    cursor_out
  };

  if let Some((x, y)) = ui_cursor {
    f.set_cursor(x, y);
  } else {
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

  if let Some(new_scroll) = the_lib::view::scroll_to_keep_visible(
    cursor_line,
    cursor_col,
    view.scroll,
    viewport_height,
    viewport_width,
    ctx.scrolloff,
  ) {
    ctx.editor.view_mut().scroll = new_scroll;
  }
}
