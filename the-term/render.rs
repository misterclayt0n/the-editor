//! Rendering - converts RenderPlan to ratatui draw calls.

use std::{
  collections::BTreeMap,
  path::Path,
};

use ratatui::{
  prelude::*,
  style::Modifier,
  text::{
    Line,
    Span,
  },
  widgets::{
    Block,
    Borders,
    Clear,
    Paragraph,
    Widget,
  },
};
use ropey::Rope;
use the_default::{
  FilePickerPreview,
  command_palette_filtered_indices,
  file_picker_icon_glyph,
  render_plan,
  set_picker_visible_rows,
  ui_tree,
};
use the_lib::{
  diagnostics::DiagnosticSeverity,
  render::{
    LayoutIntent,
    NoHighlights,
    RenderDiagnosticGutterStyles,
    RenderDiffGutterStyles,
    RenderPlan,
    RenderStyles,
    SyntaxHighlightAdapter,
    UiAlign,
    UiAlignPair,
    UiAxis,
    UiColor,
    UiColorToken,
    UiConstraints,
    UiContainer,
    UiEmphasis,
    UiInput,
    UiInsets,
    UiLayer,
    UiLayout,
    UiList,
    UiListItem,
    UiNode,
    UiPanel,
    UiStatusBar,
    UiStyle,
    UiStyledSpan,
    UiText,
    UiTooltip,
    UiTree,
    apply_diagnostic_gutter_markers,
    apply_diff_gutter_markers,
    build_plan,
    graphics::{
      Modifier as LibModifier,
      Style as LibStyle,
      UnderlineStyle as LibUnderlineStyle,
    },
    gutter_width_for_document,
    text_annotations::TextAnnotations,
    ui_theme::resolve_ui_tree,
  },
  selection::Range,
  syntax::{
    Highlight,
    Syntax,
  },
};

use crate::{
  Ctx,
  picker_layout::{
    CompletionDocsLayout,
    FilePickerLayout,
    compute_file_picker_layout,
    compute_scrollbar_metrics,
  },
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
    gutter: theme.try_get("ui.linenr").unwrap_or_default(),
    gutter_active: theme
      .try_get("ui.linenr.selected")
      .or_else(|| theme.try_get("ui.linenr"))
      .unwrap_or_default(),
  }
}

fn render_diagnostic_styles_from_theme(
  theme: &the_lib::render::theme::Theme,
) -> RenderDiagnosticGutterStyles {
  RenderDiagnosticGutterStyles {
    error:   theme
      .try_get("error")
      .or_else(|| theme.try_get("diagnostic.error"))
      .or_else(|| theme.try_get("ui.linenr"))
      .unwrap_or_default(),
    warning: theme
      .try_get("warning")
      .or_else(|| theme.try_get("diagnostic.warning"))
      .or_else(|| theme.try_get("ui.linenr"))
      .unwrap_or_default(),
    info:    theme
      .try_get("info")
      .or_else(|| theme.try_get("diagnostic.info"))
      .or_else(|| theme.try_get("ui.linenr"))
      .unwrap_or_default(),
    hint:    theme
      .try_get("hint")
      .or_else(|| theme.try_get("diagnostic.hint"))
      .or_else(|| theme.try_get("ui.linenr"))
      .unwrap_or_default(),
  }
}

fn render_diff_styles_from_theme(theme: &the_lib::render::theme::Theme) -> RenderDiffGutterStyles {
  RenderDiffGutterStyles {
    added:    theme
      .try_get("diff.plus")
      .or_else(|| theme.try_get("ui.linenr"))
      .unwrap_or_default(),
    modified: theme
      .try_get("diff.delta")
      .or_else(|| theme.try_get("ui.linenr"))
      .unwrap_or_default(),
    removed:  theme
      .try_get("diff.minus")
      .or_else(|| theme.try_get("ui.linenr"))
      .unwrap_or_default(),
  }
}

fn active_diagnostics_by_line(ctx: &Ctx) -> BTreeMap<usize, DiagnosticSeverity> {
  let Some(state) = ctx.lsp_document.as_ref().filter(|state| state.opened) else {
    return BTreeMap::new();
  };
  let Some(document) = ctx.diagnostics.document(&state.uri) else {
    return BTreeMap::new();
  };

  let mut out = BTreeMap::new();
  for diagnostic in &document.diagnostics {
    let line = diagnostic.range.start.line as usize;
    let severity = diagnostic.severity.unwrap_or(DiagnosticSeverity::Warning);
    match out.get(&line).copied() {
      Some(prev) if diagnostic_severity_rank(prev) >= diagnostic_severity_rank(severity) => {},
      _ => {
        out.insert(line, severity);
      },
    }
  }
  out
}

fn diagnostic_severity_rank(severity: DiagnosticSeverity) -> u8 {
  match severity {
    DiagnosticSeverity::Error => 4,
    DiagnosticSeverity::Warning => 3,
    DiagnosticSeverity::Information => 2,
    DiagnosticSeverity::Hint => 1,
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

fn draw_fuzzy_match_line(
  buf: &mut Buffer,
  x: u16,
  y: u16,
  text: &str,
  max_chars: usize,
  base_style: Style,
  fuzzy_style: Style,
  match_indices: &[usize],
) {
  if max_chars == 0 {
    return;
  }

  let mut next_match_iter = match_indices.iter().copied();
  let mut next_match = next_match_iter.next();
  for (char_index, ch) in text.chars().enumerate() {
    if char_index >= max_chars {
      break;
    }

    let mut style = base_style;
    if next_match.is_some_and(|idx| idx == char_index) {
      style = style.patch(fuzzy_style);
      next_match = next_match_iter.next();
    }

    let mut utf8 = [0u8; 4];
    let symbol = ch.encode_utf8(&mut utf8);
    buf.set_stringn(x.saturating_add(char_index as u16), y, symbol, 1, style);
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
  buf.set_string(
    rect.x + rect.width - 1,
    rect.y + rect.height - 1,
    "┘",
    border,
  );

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
    the_lib::render::UiAlign::Center => rect.x + (rect.width.saturating_sub(width)) / 2,
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
    the_lib::render::UiAlign::Center => rect.y + (rect.height.saturating_sub(height)) / 2,
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
          let wrapped = ((line_len as usize + max_width as usize - 1) / max_width as usize).max(1);
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
      let is_completion_list = list.style.role.as_deref() == Some("completion");
      let has_icons =
        is_completion_list && list.items.iter().any(|item| item.leading_icon.is_some());
      let icon_width: usize = if has_icons { 2 } else { 0 };
      let mut has_detail = false;
      for item in &list.items {
        let mut w = item.title.chars().count() + icon_width;
        if let Some(shortcut) = item.shortcut.as_ref() {
          w = w.saturating_add(shortcut.chars().count() + 3);
        }
        if let Some(detail) = item
          .subtitle
          .as_deref()
          .filter(|s| !s.is_empty())
          .or_else(|| item.description.as_deref().filter(|s| !s.is_empty()))
        {
          if is_completion_list {
            w = w.saturating_add(detail.chars().count() + 2);
          } else {
            has_detail = true;
            w = w.max(detail.chars().count());
          }
        }
        width = width.max(w);
      }
      let width = if list.fill_width {
        max_width
      } else {
        width.min(max_width as usize).max(1) as u16
      };
      let base_height = if is_completion_list {
        1
      } else if has_detail {
        2
      } else {
        1
      };
      let row_height = base_height;
      let mut count = list.items.len().max(1);
      if let Some(max_visible) = list.max_visible {
        count = count.min(max_visible.max(1));
      }
      let count = count as u16;
      let total_height = count.saturating_mul(row_height as u16);
      (width, total_height)
    },
    UiNode::Container(container) => {
      match &container.layout {
        UiLayout::Stack { axis, gap } => {
          match axis {
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
              apply_constraints(
                width,
                height.max(1),
                &container.constraints,
                max_width,
                u16::MAX,
              )
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
              apply_constraints(
                width.max(1),
                height.max(1),
                &container.constraints,
                max_width,
                u16::MAX,
              )
            },
          }
        },
        UiLayout::Split { axis, .. } => {
          match axis {
            UiAxis::Vertical => {
              let width = max_width.saturating_add(container.constraints.padding.horizontal());
              let height =
                container.children.len().max(1) as u16 + container.constraints.padding.vertical();
              apply_constraints(
                width.max(1),
                height.max(1),
                &container.constraints,
                max_width,
                u16::MAX,
              )
            },
            UiAxis::Horizontal => {
              let width = max_width.saturating_add(container.constraints.padding.horizontal());
              let height = 1 + container.constraints.padding.vertical();
              apply_constraints(
                width.max(1),
                height.max(1),
                &container.constraints,
                max_width,
                u16::MAX,
              )
            },
          }
        },
      }
    },
    UiNode::Panel(panel) => {
      let max_width =
        max_content_width_for_intent(panel.intent.clone(), Rect::new(0, 0, max_width, 1), 0, 0);
      let (child_w, child_h) = measure_node(&panel.child, max_width);
      let width = child_w.saturating_add(panel.constraints.padding.horizontal());
      let height = child_h.saturating_add(panel.constraints.padding.vertical());
      apply_constraints(
        width.max(1),
        height.max(1),
        &panel.constraints,
        max_width,
        u16::MAX,
      )
    },
    UiNode::Tooltip(tooltip) => {
      let width = tooltip
        .content
        .chars()
        .count()
        .saturating_add(2)
        .min(max_width as usize) as u16;
      (width.max(2), 3)
    },
    UiNode::StatusBar(_) => (max_width, 1),
  }
}

fn layout_children<'a>(container: &'a UiContainer, rect: Rect) -> Vec<(Rect, &'a UiNode)> {
  let mut placements = Vec::new();
  let rect = inset_rect(rect, container.constraints.padding);

  match &container.layout {
    UiLayout::Stack { axis, gap } => {
      match axis {
        UiAxis::Vertical => {
          let mut y = rect.y;
          for child in &container.children {
            let (child_w, h) = measure_node(child, rect.width);
            let height = h
              .min(rect.height.saturating_sub(y.saturating_sub(rect.y)))
              .max(1);
            if height == 0 {
              break;
            }
            let (x, width) =
              align_horizontal(rect, child_w, container.constraints.align.horizontal);
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
            let width = w
              .min(rect.width.saturating_sub(x.saturating_sub(rect.x)))
              .max(1);
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
      }
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
            let (x, width) =
              align_horizontal(rect, rect.width, container.constraints.align.horizontal);
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
            let (y, height) =
              align_vertical(rect, rect.height, container.constraints.align.vertical);
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

#[derive(Clone)]
struct StyledTextRun {
  text:  String,
  style: Style,
}

#[derive(Clone, Copy)]
struct CompletionDocsStyles {
  base:    Style,
  heading: [Style; 6],
  bullet:  Style,
  quote:   Style,
  code:    Style,
  link:    Style,
  rule:    Style,
}

impl CompletionDocsStyles {
  fn default(base: Style) -> Self {
    let heading = [
      base.add_modifier(Modifier::BOLD),
      base.add_modifier(Modifier::BOLD),
      base.add_modifier(Modifier::BOLD),
      base.add_modifier(Modifier::BOLD),
      base.add_modifier(Modifier::BOLD),
      base.add_modifier(Modifier::BOLD),
    ];
    Self {
      base,
      heading,
      bullet: base.add_modifier(Modifier::BOLD),
      quote: base.add_modifier(Modifier::DIM),
      code: base.add_modifier(Modifier::DIM),
      link: base.add_modifier(Modifier::UNDERLINED),
      rule: base.add_modifier(Modifier::DIM),
    }
  }
}

fn theme_style_or(ctx: &Ctx, scope: &str, fallback: Style) -> Style {
  ctx
    .ui_theme
    .try_get(scope)
    .map(lib_style_to_ratatui)
    .map(|style| fallback.patch(style))
    .unwrap_or(fallback)
}

fn completion_docs_styles(ctx: &Ctx, base: Style) -> CompletionDocsStyles {
  let mut styles = CompletionDocsStyles::default(base);
  styles.heading = [
    theme_style_or(ctx, "markup.heading.1", styles.heading[0]),
    theme_style_or(ctx, "markup.heading.2", styles.heading[1]),
    theme_style_or(ctx, "markup.heading.3", styles.heading[2]),
    theme_style_or(ctx, "markup.heading.4", styles.heading[3]),
    theme_style_or(ctx, "markup.heading.5", styles.heading[4]),
    theme_style_or(ctx, "markup.heading.6", styles.heading[5]),
  ];
  styles.bullet = theme_style_or(ctx, "markup.list.unnumbered", styles.bullet);
  styles.quote = theme_style_or(ctx, "markup.quote", styles.quote);
  styles.code = theme_style_or(ctx, "markup.raw.inline", styles.code);
  styles.link = theme_style_or(ctx, "markup.link.text", styles.link);
  styles.rule = theme_style_or(ctx, "punctuation.special", styles.rule);
  styles
}

fn push_styled_run(runs: &mut Vec<StyledTextRun>, text: String, style: Style) {
  if text.is_empty() {
    return;
  }
  if let Some(last) = runs.last_mut()
    && last.style == style
  {
    last.text.push_str(&text);
    return;
  }
  runs.push(StyledTextRun { text, style });
}

fn parse_markdown_link(chars: &[char], start: usize) -> Option<(usize, String)> {
  if chars.get(start).copied() != Some('[') {
    return None;
  }
  let mut close_bracket = start + 1;
  while close_bracket < chars.len() && chars[close_bracket] != ']' {
    close_bracket += 1;
  }
  if close_bracket >= chars.len() || chars.get(close_bracket + 1).copied() != Some('(') {
    return None;
  }
  let mut close_paren = close_bracket + 2;
  while close_paren < chars.len() && chars[close_paren] != ')' {
    close_paren += 1;
  }
  if close_paren >= chars.len() {
    return None;
  }
  let label: String = chars[start + 1..close_bracket].iter().collect();
  Some((close_paren + 1, label))
}

fn parse_inline_markdown_runs(
  text: &str,
  styles: &CompletionDocsStyles,
  base: Style,
) -> Vec<StyledTextRun> {
  let chars: Vec<char> = text.chars().collect();
  let mut runs = Vec::new();
  let mut buf = String::new();
  let mut idx = 0usize;
  let mut bold = false;
  let mut italic = false;

  let flush = |runs: &mut Vec<StyledTextRun>, buf: &mut String, bold: bool, italic: bool| {
    if buf.is_empty() {
      return;
    }
    let mut style = base;
    if bold {
      style = style.add_modifier(Modifier::BOLD);
    }
    if italic {
      style = style.add_modifier(Modifier::ITALIC);
    }
    push_styled_run(runs, std::mem::take(buf), style);
  };

  while idx < chars.len() {
    if chars[idx] == '`' {
      flush(&mut runs, &mut buf, bold, italic);
      let mut end = idx + 1;
      while end < chars.len() && chars[end] != '`' {
        end += 1;
      }
      if end < chars.len() {
        let literal: String = chars[idx + 1..end].iter().collect();
        push_styled_run(&mut runs, literal, styles.code);
        idx = end + 1;
        continue;
      }
      buf.push(chars[idx]);
      idx += 1;
      continue;
    }

    if let Some((next, label)) = parse_markdown_link(&chars, idx) {
      flush(&mut runs, &mut buf, bold, italic);
      let mut style = base;
      if bold {
        style = style.add_modifier(Modifier::BOLD);
      }
      if italic {
        style = style.add_modifier(Modifier::ITALIC);
      }
      push_styled_run(&mut runs, label, style.patch(styles.link));
      idx = next;
      continue;
    }

    if idx + 1 < chars.len() && chars[idx] == '*' && chars[idx + 1] == '*' {
      flush(&mut runs, &mut buf, bold, italic);
      bold = !bold;
      idx += 2;
      continue;
    }

    if chars[idx] == '*' {
      flush(&mut runs, &mut buf, bold, italic);
      italic = !italic;
      idx += 1;
      continue;
    }

    buf.push(chars[idx]);
    idx += 1;
  }

  flush(&mut runs, &mut buf, bold, italic);
  runs
}

fn parse_numbered_list_prefix(line: &str) -> Option<(String, &str)> {
  let bytes = line.as_bytes();
  let mut idx = 0usize;
  while idx < bytes.len() && bytes[idx].is_ascii_digit() {
    idx += 1;
  }
  if idx == 0 || idx + 1 >= bytes.len() || bytes[idx] != b'.' || bytes[idx + 1] != b' ' {
    return None;
  }
  let marker = line[..=idx].to_string();
  let rest = &line[idx + 2..];
  Some((marker, rest))
}

fn parse_heading(line: &str) -> Option<(usize, &str)> {
  let mut level = 0usize;
  for ch in line.chars() {
    if ch == '#' {
      level += 1;
    } else {
      break;
    }
  }
  if level == 0 || level > 6 {
    return None;
  }
  line
    .strip_prefix(&"#".repeat(level))
    .and_then(|rest| rest.strip_prefix(' '))
    .map(|rest| (level, rest))
}

fn is_markdown_rule(line: &str) -> bool {
  let chars: Vec<char> = line.chars().filter(|ch| !ch.is_whitespace()).collect();
  if chars.len() < 3 {
    return false;
  }
  chars.iter().all(|ch| matches!(ch, '-' | '_' | '*'))
}

fn parse_markdown_fence_language(trimmed_line: &str) -> Option<String> {
  let fence = trimmed_line.strip_prefix("```")?;
  let token = fence
    .trim()
    .split(|ch: char| ch.is_whitespace() || matches!(ch, ',' | '{' | '}'))
    .next()
    .unwrap_or_default()
    .trim_matches('.');
  (!token.is_empty()).then(|| token.to_string())
}

fn highlighted_code_block_lines(
  code_lines: &[String],
  styles: &CompletionDocsStyles,
  ctx: Option<&Ctx>,
  language: Option<&str>,
) -> Vec<Vec<StyledTextRun>> {
  if code_lines.is_empty() {
    return vec![Vec::new()];
  }

  let Some(ctx) = ctx else {
    return code_lines
      .iter()
      .map(|line| {
        vec![StyledTextRun {
          text:  line.clone(),
          style: styles.code,
        }]
      })
      .collect();
  };
  let Some(loader) = ctx.loader.as_deref() else {
    return code_lines
      .iter()
      .map(|line| {
        vec![StyledTextRun {
          text:  line.clone(),
          style: styles.code,
        }]
      })
      .collect();
  };
  let Some(language) = language.and_then(|marker| {
    loader
      .language_for_name(marker)
      .or_else(|| loader.language_for_scope(marker))
      .or_else(|| loader.language_for_filename(Path::new(&format!("tmp.{marker}"))))
  }) else {
    return code_lines
      .iter()
      .map(|line| {
        vec![StyledTextRun {
          text:  line.clone(),
          style: styles.code,
        }]
      })
      .collect();
  };

  let joined = code_lines.join("\n");
  let rope = Rope::from_str(&joined);
  let Ok(syntax) = Syntax::new(rope.slice(..), language, loader) else {
    return code_lines
      .iter()
      .map(|line| {
        vec![StyledTextRun {
          text:  line.clone(),
          style: styles.code,
        }]
      })
      .collect();
  };

  let mut highlights = syntax.collect_highlights(rope.slice(..), loader, 0..rope.len_bytes());
  highlights.sort_by_key(|(_highlight, range)| (range.start, std::cmp::Reverse(range.end)));

  let mut rendered = Vec::with_capacity(code_lines.len());
  let mut line_start_byte = 0usize;

  for (idx, line) in code_lines.iter().enumerate() {
    let mut runs = Vec::new();
    let mut piece = String::new();
    let mut active_style = styles.code;
    let mut byte_idx = line_start_byte;

    for ch in line.chars() {
      let style = preview_highlight_at(&highlights, byte_idx)
        .map(|highlight| {
          styles
            .code
            .patch(lib_style_to_ratatui(ctx.ui_theme.highlight(highlight)))
        })
        .unwrap_or(styles.code);
      if style != active_style && !piece.is_empty() {
        push_styled_run(&mut runs, std::mem::take(&mut piece), active_style);
      }
      active_style = style;
      piece.push(ch);
      byte_idx = byte_idx.saturating_add(ch.len_utf8());
    }
    push_styled_run(&mut runs, piece, active_style);
    if runs.is_empty() {
      runs.push(StyledTextRun {
        text:  String::new(),
        style: styles.code,
      });
    }
    rendered.push(runs);

    line_start_byte = line_start_byte.saturating_add(line.len());
    if idx + 1 < code_lines.len() {
      line_start_byte = line_start_byte.saturating_add(1);
    }
  }

  rendered
}

fn completion_docs_markdown_lines(
  markdown: &str,
  styles: &CompletionDocsStyles,
  ctx: Option<&Ctx>,
) -> Vec<Vec<StyledTextRun>> {
  let mut lines = Vec::new();
  let mut in_code_block = false;
  let mut code_block_language: Option<String> = None;
  let mut code_block_lines: Vec<String> = Vec::new();

  for raw_line in markdown.lines() {
    let normalized = raw_line.replace('\t', "  ");
    let trimmed = normalized.trim_start();

    if trimmed.starts_with("```") {
      if in_code_block {
        lines.extend(highlighted_code_block_lines(
          &code_block_lines,
          styles,
          ctx,
          code_block_language.as_deref(),
        ));
        code_block_lines.clear();
        code_block_language = None;
        in_code_block = false;
      } else {
        code_block_language = parse_markdown_fence_language(trimmed);
        in_code_block = true;
      }
      continue;
    }

    if in_code_block {
      code_block_lines.push(normalized);
      continue;
    }

    if trimmed.is_empty() {
      lines.push(Vec::new());
      continue;
    }

    if is_markdown_rule(trimmed) {
      lines.push(vec![StyledTextRun {
        text:  "───".to_string(),
        style: styles.rule,
      }]);
      continue;
    }

    if let Some((level, heading)) = parse_heading(trimmed) {
      let style = styles.heading[level.saturating_sub(1)];
      let runs = parse_inline_markdown_runs(heading, styles, style);
      lines.push(runs);
      continue;
    }

    if let Some(stripped) = trimmed
      .strip_prefix("- ")
      .or_else(|| trimmed.strip_prefix("* "))
      .or_else(|| trimmed.strip_prefix("+ "))
    {
      let mut runs = Vec::new();
      push_styled_run(&mut runs, "• ".to_string(), styles.bullet);
      runs.extend(parse_inline_markdown_runs(stripped, styles, styles.base));
      lines.push(runs);
      continue;
    }

    if let Some((marker, rest)) = parse_numbered_list_prefix(trimmed) {
      let mut runs = Vec::new();
      push_styled_run(&mut runs, format!("{marker} "), styles.bullet);
      runs.extend(parse_inline_markdown_runs(rest, styles, styles.base));
      lines.push(runs);
      continue;
    }

    if let Some(quoted) = trimmed.strip_prefix('>') {
      let mut runs = Vec::new();
      push_styled_run(&mut runs, "│ ".to_string(), styles.quote);
      runs.extend(parse_inline_markdown_runs(
        quoted.trim_start(),
        styles,
        styles.quote,
      ));
      lines.push(runs);
      continue;
    }

    lines.push(parse_inline_markdown_runs(trimmed, styles, styles.base));
  }

  if in_code_block {
    lines.extend(highlighted_code_block_lines(
      &code_block_lines,
      styles,
      ctx,
      code_block_language.as_deref(),
    ));
  }

  if lines.is_empty() {
    lines.push(Vec::new());
  }
  lines
}

fn wrap_styled_runs(runs: &[StyledTextRun], width: usize) -> Vec<Vec<StyledTextRun>> {
  if width == 0 {
    return Vec::new();
  }
  if runs.is_empty() {
    return vec![Vec::new()];
  }

  let mut wrapped = Vec::new();
  let mut current = Vec::new();
  let mut col = 0usize;

  for run in runs {
    let mut piece = String::new();
    for ch in run.text.chars() {
      if col >= width {
        if !piece.is_empty() {
          push_styled_run(&mut current, std::mem::take(&mut piece), run.style);
        }
        wrapped.push(current);
        current = Vec::new();
        col = 0;
      }
      piece.push(ch);
      col += 1;
    }
    if !piece.is_empty() {
      push_styled_run(&mut current, piece, run.style);
    }
  }

  if current.is_empty() {
    wrapped.push(Vec::new());
  } else {
    wrapped.push(current);
  }
  wrapped
}

fn completion_docs_rows_with_context(
  markdown: &str,
  styles: &CompletionDocsStyles,
  width: usize,
  ctx: Option<&Ctx>,
) -> Vec<Vec<StyledTextRun>> {
  let mut rows = Vec::new();
  for line in completion_docs_markdown_lines(markdown, styles, ctx) {
    rows.extend(wrap_styled_runs(&line, width));
  }
  if rows.is_empty() {
    rows.push(Vec::new());
  }
  rows
}

fn completion_docs_rows(
  markdown: &str,
  styles: &CompletionDocsStyles,
  width: usize,
) -> Vec<Vec<StyledTextRun>> {
  completion_docs_rows_with_context(markdown, styles, width, None)
}

#[derive(Debug, Clone, Copy)]
struct CompletionDocsRenderMetrics {
  content_width:  usize,
  total_rows:     usize,
  visible_rows:   usize,
  show_scrollbar: bool,
}

fn completion_docs_render_metrics(
  markdown: &str,
  styles: &CompletionDocsStyles,
  rect: Rect,
) -> CompletionDocsRenderMetrics {
  if rect.width == 0 || rect.height == 0 {
    return CompletionDocsRenderMetrics {
      content_width:  0,
      total_rows:     0,
      visible_rows:   0,
      show_scrollbar: false,
    };
  }

  let mut content_width = rect.width as usize;
  let mut rows = completion_docs_rows(markdown, styles, content_width);
  let mut show_scrollbar = rows.len() > rect.height as usize && rect.width > 1;
  if show_scrollbar {
    content_width = rect.width.saturating_sub(1) as usize;
    rows = completion_docs_rows(markdown, styles, content_width);
    show_scrollbar = rows.len() > rect.height as usize && rect.width > 1;
  }

  CompletionDocsRenderMetrics {
    content_width,
    total_rows: rows.len(),
    visible_rows: rect.height as usize,
    show_scrollbar,
  }
}

fn draw_styled_row(
  buf: &mut Buffer,
  x: u16,
  y: u16,
  width: usize,
  row: &[StyledTextRun],
  base_style: Style,
) {
  if width == 0 {
    return;
  }
  buf.set_string(x, y, " ".repeat(width), base_style);

  let mut col = 0usize;
  for run in row {
    for ch in run.text.chars() {
      if col >= width {
        return;
      }
      let mut symbol = [0u8; 4];
      let symbol = ch.encode_utf8(&mut symbol);
      buf.set_stringn(x + col as u16, y, symbol, 1, run.style);
      col += 1;
    }
  }
}

fn draw_completion_docs_text(buf: &mut Buffer, rect: Rect, ctx: &Ctx, text: &UiText) {
  if rect.width == 0 || rect.height == 0 {
    return;
  }

  let (text_style, ..) = ui_style_colors(&text.style);
  let base_style = apply_ui_emphasis(text_style, text.style.emphasis);
  let styles = completion_docs_styles(ctx, base_style);
  let metrics = completion_docs_render_metrics(&text.content, &styles, rect);
  let content_width = metrics.content_width;
  let rows = completion_docs_rows_with_context(&text.content, &styles, content_width, Some(ctx));
  let total_rows = metrics.total_rows;
  let visible_rows = metrics.visible_rows;
  let max_scroll = total_rows.saturating_sub(visible_rows);
  let docs_scroll = if text
    .id
    .as_deref()
    .is_some_and(|id| id.starts_with("term_command_palette_docs"))
  {
    0
  } else {
    ctx.completion_menu.docs_scroll
  };
  let scroll = docs_scroll.min(max_scroll);

  for row_idx in 0..visible_rows {
    let y = rect.y + row_idx as u16;
    if let Some(row) = rows.get(scroll + row_idx) {
      draw_styled_row(buf, rect.x, y, content_width, row, base_style);
    } else {
      draw_styled_row(buf, rect.x, y, content_width, &[], base_style);
    }
  }

  if metrics.show_scrollbar {
    let track_x = rect.x + rect.width - 1;
    let track_height = rect.height;
    let thumb_height = ((visible_rows as f32 / total_rows as f32) * track_height as f32)
      .round()
      .clamp(1.0, track_height as f32) as u16;
    let max_thumb_offset = track_height.saturating_sub(thumb_height);
    let thumb_offset = if max_scroll == 0 || max_thumb_offset == 0 {
      0
    } else {
      ((scroll as f32 / max_scroll as f32) * max_thumb_offset as f32).round() as u16
    };
    let scroll_color = text
      .style
      .border
      .as_ref()
      .and_then(resolve_ui_color)
      .or_else(|| text.style.accent.as_ref().and_then(resolve_ui_color))
      .or(base_style.fg);

    for row in 0..track_height {
      let is_thumb = row >= thumb_offset && row < thumb_offset + thumb_height;
      if !is_thumb {
        continue;
      }
      let mut style = Style::default();
      if let Some(color) = scroll_color {
        style = style.fg(color);
      }
      buf.set_string(track_x, rect.y + row, "█", style);
    }
  }
}

fn draw_ui_text(buf: &mut Buffer, rect: Rect, ctx: &Ctx, text: &UiText) {
  if rect.width == 0 || rect.height == 0 {
    return;
  }
  if text.style.role.as_deref() == Some("completion_docs") {
    draw_completion_docs_text(buf, rect, ctx, text);
    return;
  }
  let (text_style, ..) = ui_style_colors(&text.style);
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
  let (text_style, ..) = ui_style_colors(&input.style);
  let placeholder_color = input
    .style
    .accent
    .as_ref()
    .and_then(resolve_ui_color)
    .or(text_style.fg);
  let (value, style) = if input.value.is_empty() {
    let placeholder = input.placeholder.as_deref().unwrap_or("...");
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

  let is_focused = focus.map(|f| f.id == input.id).unwrap_or(focus.is_none());
  if is_focused && cursor_out.is_none() {
    let cursor_pos = focus.and_then(|f| f.cursor).unwrap_or(input.cursor);
    let cursor_x = rect
      .x
      .saturating_add(cursor_pos as u16)
      .min(rect.x + rect.width - 1);
    *cursor_out = Some((cursor_x, rect.y));
  }
}

fn draw_ui_list(buf: &mut Buffer, rect: Rect, list: &UiList, _cursor_out: &mut Option<(u16, u16)>) {
  if rect.width == 0 || rect.height == 0 {
    return;
  }
  let (text_style, ..) = ui_style_colors(&list.style);
  let base_text_color = text_style.fg;
  let selected_text_color = list
    .style
    .border
    .as_ref()
    .and_then(resolve_ui_color)
    .or(base_text_color);
  let selected_bg_color = list.style.accent.as_ref().and_then(resolve_ui_color);
  let scroll_color = list
    .style
    .border
    .as_ref()
    .and_then(resolve_ui_color)
    .or(base_text_color);
  let is_completion_list = list.style.role.as_deref() == Some("completion");
  let has_icons = is_completion_list && list.items.iter().any(|item| item.leading_icon.is_some());
  let icon_col_width: u16 = if has_icons { 2 } else { 0 };
  let has_detail = list.items.iter().any(|item| {
    item.subtitle.as_ref().map_or(false, |s| !s.is_empty())
      || item.description.as_ref().map_or(false, |s| !s.is_empty())
  });
  let base_height: usize = if is_completion_list {
    1
  } else if has_detail {
    2
  } else {
    1
  };
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
  let total_items = list.virtual_total.unwrap_or(list.items.len());
  let virtual_mode = list.virtual_total.is_some();
  let mut scroll_offset = if virtual_mode {
    list
      .virtual_start
      .min(total_items.saturating_sub(visible_items))
  } else {
    list.scroll.min(total_items.saturating_sub(visible_items))
  };
  let selected = list.selected;
  if !virtual_mode {
    if let Some(sel) = selected {
      if sel < scroll_offset {
        scroll_offset = sel;
      } else if sel >= scroll_offset + visible_items {
        scroll_offset = sel + 1 - visible_items;
      }
    }
  }
  let mut draw_item = |row_idx: usize, absolute_idx: usize, item: &the_lib::render::UiListItem| {
    let y = rect.y + (row_idx * row_height) as u16;
    let is_selected = selected == Some(absolute_idx);
    let row_right_padding = if total_items > visible_items { 2 } else { 1 };

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
    let leading_pad = if is_completion_list { 0 } else { 1 };
    let base_content_x = rect.x + leading_pad;
    let available_width = rect.width.saturating_sub(leading_pad + row_right_padding) as usize;
    if !shortcut.is_empty() && shortcut.len() + 2 < available_width {
      let shortcut_width = shortcut.len() + 1;
      truncate_in_place(&mut title, available_width.saturating_sub(shortcut_width));
      let shortcut_x = rect.x
        + rect
          .width
          .saturating_sub(shortcut.len() as u16 + row_right_padding);
      buf.set_string(shortcut_x, y, shortcut, row_style);
    } else {
      truncate_in_place(&mut title, available_width);
    }
    if is_completion_list {
      if has_icons {
        if let Some(icon) = item.leading_icon.as_deref() {
          let icon_style = if is_selected {
            row_style
          } else if let Some(ref color) = item.leading_color {
            resolve_ui_color(color)
              .map(|c| Style::default().fg(c))
              .unwrap_or(row_style)
          } else {
            row_style
          };
          buf.set_string(base_content_x, y, icon, icon_style);
        }
      }

      let label_x = base_content_x + icon_col_width;
      let label_available = rect
        .width
        .saturating_sub(leading_pad + icon_col_width + row_right_padding)
        as usize;

      let detail = item
        .subtitle
        .as_deref()
        .filter(|detail| !detail.is_empty())
        .or_else(|| {
          item
            .description
            .as_deref()
            .filter(|detail| !detail.is_empty())
        });
      if let Some(detail) = detail {
        let content_right = rect.x + rect.width.saturating_sub(row_right_padding);
        let mut detail_text = detail.to_string();
        truncate_in_place(&mut detail_text, label_available.saturating_sub(4));
        let detail_width = detail_text.chars().count() as u16;
        let detail_x = content_right.saturating_sub(detail_width);
        let mut title_width = detail_x.saturating_sub(label_x).saturating_sub(1) as usize;
        if title_width == 0 {
          title_width = 1;
        }
        truncate_in_place(&mut title, title_width);
        buf.set_string(label_x, y, title, row_style);
        let detail_style = if is_selected {
          row_style
        } else {
          row_style.add_modifier(Modifier::DIM)
        };
        if detail_x > label_x {
          buf.set_string(detail_x, y, detail_text, detail_style);
        }
      } else {
        truncate_in_place(&mut title, label_available);
        buf.set_string(label_x, y, title, row_style);
      }
    } else {
      buf.set_string(base_content_x, y, title, row_style);
    }

    if !is_completion_list && base_height > 1 {
      let detail = item
        .subtitle
        .as_deref()
        .filter(|detail: &&str| !detail.is_empty())
        .or_else(|| {
          item
            .description
            .as_deref()
            .filter(|detail: &&str| !detail.is_empty())
        });
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
  };

  if virtual_mode {
    for (row_idx, item) in list.items.iter().take(visible_items).enumerate() {
      draw_item(row_idx, scroll_offset + row_idx, item);
    }
  } else {
    for (row_idx, item) in list
      .items
      .iter()
      .skip(scroll_offset)
      .take(visible_items)
      .enumerate()
    {
      draw_item(row_idx, row_idx + scroll_offset, item);
    }
  }

  if total_items > visible_items {
    let track_x = rect.x + rect.width - 1;
    let track_height = rect.height;
    let thumb_height = ((visible_items as f32 / total_items as f32) * track_height as f32)
      .ceil()
      .max(1.0) as u16;
    let max_scroll = total_items.saturating_sub(visible_items);
    let thumb_offset = if max_scroll == 0 {
      0
    } else {
      ((scroll_offset as f32 / max_scroll as f32) * (track_height - thumb_height) as f32).round()
        as u16
    };
    for i in 0..track_height {
      let y = rect.y + i;
      let is_thumb = i >= thumb_offset && i < thumb_offset + thumb_height;
      if !is_thumb {
        continue;
      }
      let mut style = Style::default();
      if let Some(color) = scroll_color {
        style = style.fg(color);
      }
      buf.set_string(track_x, y, "█", style);
    }
  }
}

fn draw_ui_node(
  buf: &mut Buffer,
  rect: Rect,
  ctx: &Ctx,
  node: &UiNode,
  focus: Option<&the_lib::render::UiFocus>,
  editor_cursor: Option<(u16, u16)>,
  cursor_out: &mut Option<(u16, u16)>,
) {
  if rect.width == 0 || rect.height == 0 {
    return;
  }
  match node {
    UiNode::Text(text) => draw_ui_text(buf, rect, ctx, text),
    UiNode::Input(input) => draw_ui_input(buf, rect, ctx, input, focus, cursor_out),
    UiNode::List(list) => draw_ui_list(buf, rect, list, cursor_out),
    UiNode::Divider(_) => {
      let line = "─".repeat(rect.width as usize);
      let style = Style::default().add_modifier(Modifier::DIM);
      buf.set_string(rect.x, rect.y, line, style);
    },
    UiNode::Spacer(_) => {},
    UiNode::Container(container) => {
      let placements = layout_children(container, rect);
      for (child_rect, child) in placements {
        draw_ui_node(
          buf,
          child_rect,
          ctx,
          child,
          focus,
          editor_cursor,
          cursor_out,
        );
      }
    },
    UiNode::Panel(panel) => {
      draw_ui_panel(buf, rect, ctx, panel, focus, editor_cursor, cursor_out);
    },
    UiNode::Tooltip(tooltip) => {
      draw_ui_tooltip(buf, rect, ctx, tooltip);
    },
    UiNode::StatusBar(status) => {
      draw_ui_status_bar(buf, rect, ctx, status);
    },
  }
}

fn draw_file_picker_panel(
  buf: &mut Buffer,
  area: Rect,
  ctx: &Ctx,
  panel: &UiPanel,
  focus: Option<&the_lib::render::UiFocus>,
  cursor_out: &mut Option<(u16, u16)>,
) {
  let picker = &ctx.file_picker;
  if !picker.active || area.width < 4 || area.height < 4 {
    return;
  }

  let Some(layout) = ctx
    .file_picker_layout
    .or_else(|| compute_file_picker_layout(area, picker))
  else {
    return;
  };
  if layout.panel.width == 0 || layout.panel.height == 0 {
    return;
  }

  let (text_style, mut fill_style, border_style) = ui_style_colors(&panel.style);
  if fill_style.bg.is_none() {
    let fallback_bg = ctx
      .ui_theme
      .try_get("ui.file_picker")
      .and_then(|style| style.bg)
      .or_else(|| {
        ctx
          .ui_theme
          .try_get("ui.background")
          .and_then(|style| style.bg)
      })
      .map(lib_color_to_ratatui);
    if let Some(bg) = fallback_bg {
      fill_style = fill_style.bg(bg);
    }
  }

  fill_rect(buf, layout.panel, fill_style);

  if layout.panel_inner.width < 3 || layout.panel_inner.height < 3 {
    return;
  }

  draw_file_picker_list_pane(
    buf,
    &layout,
    picker,
    text_style,
    fill_style,
    border_style,
    &ctx.ui_theme,
    focus,
    cursor_out,
  );

  if layout.show_preview {
    draw_file_picker_preview_pane(
      buf,
      &layout,
      picker,
      text_style,
      fill_style,
      border_style,
      &ctx.ui_theme,
    );
  }
}

fn draw_file_picker_list_pane(
  buf: &mut Buffer,
  layout: &FilePickerLayout,
  picker: &the_default::FilePickerState,
  text_style: Style,
  fill_style: Style,
  border_style: Style,
  theme: &the_lib::render::theme::Theme,
  focus: Option<&the_lib::render::UiFocus>,
  cursor_out: &mut Option<(u16, u16)>,
) {
  let rect = layout.list_pane;
  let block = Block::default()
    .borders(Borders::ALL)
    .border_style(border_style)
    .style(fill_style);
  block.render(rect, buf);

  let inner = layout.list_inner;
  if inner.width == 0 || inner.height == 0 {
    return;
  }

  let prompt_area = layout.list_prompt;
  let prompt = if picker.query.is_empty() {
    "find file".to_string()
  } else {
    picker.query.clone()
  };
  let prompt_style = if picker.query.is_empty() {
    text_style.add_modifier(Modifier::DIM)
  } else {
    text_style
  };
  Paragraph::new(prompt)
    .style(prompt_style)
    .render(prompt_area, buf);

  let count = format!("{}/{}", picker.matched_count(), picker.total_count());
  let count_style = text_style.add_modifier(Modifier::DIM);
  buf.set_stringn(
    prompt_area.x.saturating_add(
      prompt_area
        .width
        .saturating_sub(count.chars().count() as u16),
    ),
    prompt_area.y,
    &count,
    prompt_area.width as usize,
    count_style,
  );

  if let Some(error) = picker.error.as_ref().filter(|err| !err.is_empty()) {
    let error_area = Rect::new(
      prompt_area.x,
      prompt_area.y,
      prompt_area
        .width
        .saturating_sub(count.chars().count() as u16 + 1),
      1,
    );
    let mut error_text = format!("! {error}");
    truncate_in_place(&mut error_text, error_area.width as usize);
    buf.set_string(error_area.x, error_area.y, error_text, count_style);
  }

  let is_focused = focus
    .map(|focus| focus.id == "file_picker_input")
    .unwrap_or(true);
  if is_focused && cursor_out.is_none() {
    let cursor_col = picker.query[..picker.cursor.min(picker.query.len())]
      .chars()
      .count() as u16;
    let x = prompt_area
      .x
      .saturating_add(cursor_col)
      .min(prompt_area.x + prompt_area.width.saturating_sub(1));
    *cursor_out = Some((x, prompt_area.y));
  }

  let separator_y = prompt_area.y.saturating_add(1);
  if separator_y < inner.y.saturating_add(inner.height) {
    let separator = "─".repeat(inner.width as usize);
    buf.set_string(
      inner.x,
      separator_y,
      separator,
      border_style.add_modifier(Modifier::DIM),
    );
  }

  if inner.height < 3 {
    return;
  }

  let list_area = layout.list_content;
  if list_area.width == 0 || list_area.height == 0 {
    return;
  }

  let total_matches = picker.matched_count();
  if total_matches == 0 {
    Paragraph::new("<No matches>")
      .style(text_style.add_modifier(Modifier::DIM))
      .render(list_area, buf);
    return;
  }

  let visible_rows = list_area.height as usize;
  if visible_rows == 0 {
    return;
  }
  let scroll_offset = layout.list_scroll_offset;
  let end = scroll_offset
    .saturating_add(visible_rows)
    .min(total_matches);
  let selected_scope = theme
    .try_get("ui.file_picker.list.selected")
    .or_else(|| theme.try_get("ui.menu.selected"));
  let selected_bg = selected_scope
    .and_then(|style| style.bg)
    .map(lib_color_to_ratatui);
  let selected_fg = selected_scope
    .and_then(|style| style.fg)
    .map(lib_color_to_ratatui)
    .or(text_style.fg);
  let scrollbar_style = border_style.add_modifier(Modifier::DIM);
  let fuzzy_highlight_style =
    lib_style_to_ratatui(theme.try_get("special").unwrap_or_default()).add_modifier(Modifier::BOLD);
  let mut match_indices = Vec::new();
  for row_idx in scroll_offset..end {
    let Some(item) = picker.matched_item_with_match_indices(row_idx, &mut match_indices) else {
      continue;
    };
    let y = list_area.y + (row_idx - scroll_offset) as u16;
    let is_selected = picker.selected == Some(row_idx);
    let is_hovered = picker.hovered == Some(row_idx);
    let mut style = text_style;
    if is_selected && let Some(bg) = selected_bg {
      fill_rect(
        buf,
        Rect::new(list_area.x, y, list_area.width, 1),
        Style::default().bg(bg),
      );
    }
    if is_selected && let Some(fg) = selected_fg {
      style = style.fg(fg);
    }
    if item.is_dir {
      style = style.add_modifier(Modifier::BOLD);
    }
    if is_hovered {
      style = style.add_modifier(Modifier::UNDERLINED);
    }

    let icon = file_picker_icon_glyph(item.icon.as_str(), item.is_dir);
    let icon_x = list_area.x.saturating_add(1);
    buf.set_string(icon_x, y, icon, style);

    let icon_width = icon.chars().count() as u16;
    let text_x = icon_x.saturating_add(icon_width.saturating_add(1));
    let content_width = list_area
      .width
      .saturating_sub(1 + icon_width.saturating_add(1)) as usize;
    if content_width == 0 {
      continue;
    }
    draw_fuzzy_match_line(
      buf,
      text_x,
      y,
      item.display.as_str(),
      content_width,
      style,
      fuzzy_highlight_style,
      &match_indices,
    );
  }

  if let Some(track) = layout.list_scrollbar_track
    && let Some(metrics) =
      compute_scrollbar_metrics(track, total_matches, visible_rows, scroll_offset)
  {
    for idx in 0..track.height {
      let y = track.y + idx;
      let is_thumb = idx >= metrics.thumb_offset
        && idx < metrics.thumb_offset.saturating_add(metrics.thumb_height);
      if !is_thumb {
        continue;
      }
      buf.set_string(track.x, y, "█", scrollbar_style);
    }
  }
}

fn draw_file_picker_preview_pane(
  buf: &mut Buffer,
  layout: &FilePickerLayout,
  picker: &the_default::FilePickerState,
  text_style: Style,
  fill_style: Style,
  border_style: Style,
  theme: &the_lib::render::theme::Theme,
) {
  let Some(rect) = layout.preview_pane else {
    return;
  };

  let block = Block::default()
    .borders(Borders::ALL)
    .border_style(border_style)
    .style(fill_style);
  block.render(rect, buf);
  let Some(content) = layout.preview_content else {
    return;
  };
  if content.width == 0 || content.height == 0 {
    return;
  }

  let scroll_offset = layout.preview_scroll_offset;
  let visible_rows = content.height as usize;
  let total_lines = picker.preview_line_count();

  match &picker.preview {
    FilePickerPreview::Empty => {},
    FilePickerPreview::Source(source) => {
      draw_file_picker_source_preview(buf, content, source, text_style, theme, scroll_offset);
    },
    FilePickerPreview::Text(text) | FilePickerPreview::Message(text) => {
      draw_file_picker_plain_preview(buf, content, text, text_style, scroll_offset);
    },
  }

  if let Some(track) = layout.preview_scrollbar
    && let Some(metrics) =
      compute_scrollbar_metrics(track, total_lines, visible_rows, scroll_offset)
  {
    let scrollbar_style = border_style.add_modifier(Modifier::DIM);
    for idx in 0..track.height {
      let y = track.y + idx;
      let is_thumb = idx >= metrics.thumb_offset
        && idx < metrics.thumb_offset.saturating_add(metrics.thumb_height);
      if !is_thumb {
        continue;
      }
      buf.set_string(track.x, y, "█", scrollbar_style);
    }
  }
}

fn draw_file_picker_source_preview(
  buf: &mut Buffer,
  area: Rect,
  source: &the_default::FilePickerSourcePreview,
  text_style: Style,
  theme: &the_lib::render::theme::Theme,
  scroll_offset: usize,
) {
  if area.width == 0 || area.height == 0 {
    return;
  }

  let lines_len = source.lines.len().max(1);
  let line_number_width = lines_len.to_string().len();
  let gutter_style = text_style.add_modifier(Modifier::DIM);

  for row in 0..area.height as usize {
    let y = area.y + row as u16;
    let line_idx = scroll_offset.saturating_add(row);
    if line_idx >= source.lines.len() {
      if source.truncated && line_idx == source.lines.len() {
        buf.set_stringn(area.x, y, "…", area.width as usize, gutter_style);
      }
      continue;
    }

    let line_number = line_idx + 1;
    let gutter = format!("{line_number:>line_number_width$} ");
    let gutter_width = gutter.chars().count() as u16;
    buf.set_stringn(area.x, y, &gutter, area.width as usize, gutter_style);

    if gutter_width >= area.width {
      continue;
    }

    let line = &source.lines[line_idx];
    if line.is_empty() {
      continue;
    }

    let line_start = source.line_starts[line_idx];
    let line_spans = preview_line_spans(line, line_start, &source.highlights, text_style, theme);

    Paragraph::new(Line::from(line_spans)).render(
      Rect::new(
        area.x + gutter_width,
        y,
        area.width.saturating_sub(gutter_width),
        1,
      ),
      buf,
    );
  }
}

fn draw_file_picker_plain_preview(
  buf: &mut Buffer,
  area: Rect,
  text: &str,
  text_style: Style,
  scroll_offset: usize,
) {
  if area.width == 0 || area.height == 0 {
    return;
  }

  for (row, line) in text
    .lines()
    .skip(scroll_offset)
    .take(area.height as usize)
    .enumerate()
  {
    buf.set_stringn(
      area.x,
      area.y + row as u16,
      line,
      area.width as usize,
      text_style,
    );
  }
}

fn preview_line_spans<'a>(
  line: &'a str,
  line_start: usize,
  highlights: &[(Highlight, std::ops::Range<usize>)],
  text_style: Style,
  theme: &the_lib::render::theme::Theme,
) -> Vec<Span<'a>> {
  if line.is_empty() {
    return Vec::new();
  }

  if highlights.is_empty() {
    return vec![Span::styled(line, text_style)];
  }

  let line_end = line_start.saturating_add(line.len());
  let mut boundaries = vec![line_start, line_end];
  for (_highlight, range) in highlights {
    if range.end <= line_start || range.start >= line_end {
      continue;
    }
    boundaries.push(range.start.max(line_start));
    boundaries.push(range.end.min(line_end));
  }
  boundaries.sort_unstable();
  boundaries.dedup();

  let mut spans = Vec::new();
  for pair in boundaries.windows(2) {
    let absolute_start = pair[0];
    let absolute_end = pair[1];
    if absolute_end <= absolute_start {
      continue;
    }

    let local_start = clamp_boundary(line, absolute_start.saturating_sub(line_start), false);
    let local_end = clamp_boundary(line, absolute_end.saturating_sub(line_start), true);
    if local_end <= local_start {
      continue;
    }

    let sample_byte = absolute_start + (absolute_end - absolute_start) / 2;
    let style = preview_highlight_at(highlights, sample_byte)
      .map(|highlight| text_style.patch(lib_style_to_ratatui(theme.highlight(highlight))))
      .unwrap_or(text_style);

    spans.push(Span::styled(&line[local_start..local_end], style));
  }

  if spans.is_empty() {
    spans.push(Span::styled(line, text_style));
  }
  spans
}

fn clamp_boundary(text: &str, idx: usize, round_up: bool) -> usize {
  let mut idx = idx.min(text.len());
  if text.is_char_boundary(idx) {
    return idx;
  }
  if round_up {
    while idx < text.len() && !text.is_char_boundary(idx) {
      idx += 1;
    }
    return idx;
  }
  while idx > 0 && !text.is_char_boundary(idx) {
    idx -= 1;
  }
  idx
}

fn preview_highlight_at(
  highlights: &[(Highlight, std::ops::Range<usize>)],
  byte_idx: usize,
) -> Option<Highlight> {
  let mut active = None;
  for (highlight, range) in highlights {
    if byte_idx < range.start {
      break;
    }
    if byte_idx < range.end {
      active = Some(*highlight);
    }
  }
  active
}

fn max_content_width_for_intent(
  intent: LayoutIntent,
  area: Rect,
  border: u16,
  padding_h: u16,
) -> u16 {
  let full = area.width.saturating_sub(border * 2 + padding_h).max(1);
  match intent {
    LayoutIntent::Floating | LayoutIntent::Custom(_) => {
      let cap = area.width.saturating_mul(2) / 3;
      full.min(cap.max(20))
    },
    _ => full,
  }
}

fn panel_is_completion(panel: &UiPanel) -> bool {
  panel.id == "completion" || panel.style.role.as_deref() == Some("completion")
}

fn panel_is_completion_docs(panel: &UiPanel) -> bool {
  panel.id == "completion_docs" || panel.style.role.as_deref() == Some("completion_docs")
}

fn panel_is_term_command_palette_list(panel: &UiPanel) -> bool {
  panel.id == "term_command_palette_list"
}

fn panel_is_term_command_palette_docs(panel: &UiPanel) -> bool {
  panel.id == "term_command_palette_docs"
}

fn term_command_palette_panel_rect(area: Rect, panel_width: u16, panel_height: u16) -> Rect {
  let width = panel_width.min(area.width).max(1);
  let height = panel_height.min(area.height).max(1);
  let max_y = area.y.saturating_add(area.height.saturating_sub(height));
  let y = max_y;
  Rect::new(area.x, y, width, height)
}

fn completion_panel_rect(
  area: Rect,
  panel_width: u16,
  panel_height: u16,
  editor_cursor: Option<(u16, u16)>,
) -> Rect {
  let width = panel_width.min(area.width).max(1);
  let height = panel_height.min(area.height).max(1);
  let center_x = area.x + (area.width.saturating_sub(width)) / 2;
  let center_y = area.y + (area.height.saturating_sub(height)) / 2;
  let Some((cursor_x, cursor_y)) = editor_cursor else {
    return Rect::new(center_x, center_y, width, height);
  };
  if area.width == 0 || area.height == 0 {
    return Rect::new(center_x, center_y, width, height);
  }

  let max_x = area.x + area.width.saturating_sub(width);
  let max_y = area.y + area.height.saturating_sub(height);
  let cursor_x = cursor_x.clamp(area.x, area.x + area.width.saturating_sub(1));
  let cursor_y = cursor_y.clamp(area.y, area.y + area.height.saturating_sub(1));

  let mut x = cursor_x.saturating_sub(1);
  x = x.clamp(area.x, max_x);

  let below_start = cursor_y.saturating_add(1).max(area.y);
  let below_space = area
    .y
    .saturating_add(area.height)
    .saturating_sub(below_start);
  let above_space = cursor_y.saturating_sub(area.y);
  let place_below = below_space >= height || below_space >= above_space;

  let y = if place_below {
    below_start.min(max_y)
  } else {
    cursor_y.saturating_sub(height).max(area.y)
  };

  Rect::new(x, y, width, height)
}

fn completion_docs_panel_rect(
  area: Rect,
  panel_width: u16,
  panel_height: u16,
  completion_rect: Rect,
) -> Option<Rect> {
  if area.width == 0 || area.height == 0 {
    return None;
  }
  let desired_width = panel_width.min(area.width).max(1);
  let desired_height = panel_height.min(area.height).max(1);
  let gap = 1u16;
  let min_side_width = 24u16;

  let area_right = area.x.saturating_add(area.width);
  let area_bottom = area.y.saturating_add(area.height);
  let right_x = completion_rect
    .x
    .saturating_add(completion_rect.width)
    .saturating_add(gap);
  let right_available_width = area_right.saturating_sub(right_x);
  let left_end = completion_rect.x.saturating_sub(gap);
  let left_available_width = left_end.saturating_sub(area.x);

  if right_available_width >= min_side_width || left_available_width >= min_side_width {
    let place_right = right_available_width >= min_side_width
      && (right_available_width >= left_available_width || left_available_width < min_side_width);
    let available_width = if place_right {
      right_available_width
    } else {
      left_available_width
    };
    let width = desired_width.min(available_width).max(1);
    let height = desired_height.min(area.height).max(1);
    let max_y = area_bottom.saturating_sub(height);
    let y = completion_rect.y.clamp(area.y, max_y);
    let x = if place_right {
      right_x
    } else {
      left_end.saturating_sub(width)
    };
    return Some(Rect::new(x, y, width, height));
  }

  None
}

fn panel_box_size(panel: &UiPanel, area: Rect) -> (u16, u16) {
  let boxed = panel.style.border.is_some();
  let border: u16 = if boxed { 1 } else { 0 };
  let padding = panel.constraints.padding;
  let padding_h = padding.horizontal();
  let padding_v = padding.vertical();
  let title_height = panel.title.is_some() as u16;

  let (min_panel_width, min_panel_height) = if panel_is_completion(panel) {
    (1, 1)
  } else {
    (10, 3)
  };

  let max_content_width =
    max_content_width_for_intent(panel.intent.clone(), area, border, padding_h);
  let (child_w, child_h) = measure_node(&panel.child, max_content_width);
  let panel_width = child_w
    .saturating_add(border * 2 + padding_h)
    .min(area.width)
    .max(min_panel_width);
  let panel_height = child_h
    .saturating_add(border * 2 + padding_v + title_height)
    .min(area.height)
    .max(min_panel_height);

  apply_constraints(
    panel_width,
    panel_height,
    &panel.constraints,
    area.width,
    area.height,
  )
}

fn panel_content_rect(rect: Rect, panel: &UiPanel) -> Rect {
  let mut content = inner_rect(rect);
  if panel.title.is_some() {
    content = Rect::new(
      content.x,
      content.y.saturating_add(1),
      content.width,
      content.height.saturating_sub(1),
    );
  }
  inset_rect(content, panel.constraints.padding)
}

fn selected_completion_docs_text(ctx: &Ctx) -> Option<&str> {
  ctx
    .completion_menu
    .selected
    .and_then(|idx| ctx.completion_menu.items.get(idx))
    .and_then(|item| item.documentation.as_deref())
    .map(str::trim)
    .filter(|docs| !docs.is_empty())
}

fn completion_docs_layout_for_panel(
  ctx: &Ctx,
  panel: &UiPanel,
  panel_rect: Rect,
) -> Option<CompletionDocsLayout> {
  let docs = selected_completion_docs_text(ctx)?;
  let content = panel_content_rect(panel_rect, panel);
  if content.width == 0 || content.height == 0 {
    return None;
  }

  let base_style = ui_style_colors(&panel.style).0;
  let styles = completion_docs_styles(ctx, base_style);
  let metrics = completion_docs_render_metrics(docs, &styles, content);
  let scrollbar_track = metrics.show_scrollbar.then(|| {
    Rect::new(
      content.x + content.width.saturating_sub(1),
      content.y,
      1,
      content.height,
    )
  });

  Some(CompletionDocsLayout {
    panel: panel_rect,
    content: if scrollbar_track.is_some() {
      Rect::new(
        content.x,
        content.y,
        content.width.saturating_sub(1),
        content.height,
      )
    } else {
      content
    },
    scrollbar_track,
    visible_rows: metrics.visible_rows,
    total_rows: metrics.total_rows,
  })
}

fn draw_ui_panel(
  buf: &mut Buffer,
  area: Rect,
  ctx: &Ctx,
  panel: &UiPanel,
  focus: Option<&the_lib::render::UiFocus>,
  editor_cursor: Option<(u16, u16)>,
  cursor_out: &mut Option<(u16, u16)>,
) {
  if panel.id == "file_picker" || panel.style.role.as_deref() == Some("file_picker") {
    draw_file_picker_panel(buf, area, ctx, panel, focus, cursor_out);
    return;
  }

  let boxed = panel.style.border.is_some();
  let border: u16 = if boxed { 1 } else { 0 };
  let padding_h = panel.constraints.padding.horizontal();
  let padding_v = panel.constraints.padding.vertical();
  let max_content_width =
    max_content_width_for_intent(panel.intent.clone(), area, border, padding_h);
  let (_, child_h) = measure_node(&panel.child, max_content_width);
  let (mut panel_width, panel_height) = panel_box_size(panel, area);

  if panel_is_completion(panel)
    && matches!(
      panel.intent,
      LayoutIntent::Custom(_) | LayoutIntent::Floating
    )
  {
    let rect = completion_panel_rect(area, panel_width, panel_height, editor_cursor);
    draw_panel_in_rect(
      buf,
      rect,
      ctx,
      panel,
      BorderEdge::Top,
      false,
      focus,
      cursor_out,
    );
    return;
  }

  match panel.intent.clone() {
    LayoutIntent::Bottom => {
      let mut height = if boxed {
        panel_height.min(area.height).max(4)
      } else {
        let divider = flat_panel_divider(panel);
        let mut height = child_h.saturating_add(padding_v + divider).min(area.height);
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
      draw_panel_in_rect(
        buf,
        rect,
        ctx,
        panel,
        BorderEdge::Top,
        !panel_is_statusline(panel),
        focus,
        cursor_out,
      );
    },
    LayoutIntent::Top => {
      let mut height = if boxed {
        panel_height.min(area.height).max(4)
      } else {
        let divider = flat_panel_divider(panel);
        let mut height = child_h.saturating_add(padding_v + divider).min(area.height);
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
      draw_panel_in_rect(
        buf,
        rect,
        ctx,
        panel,
        BorderEdge::Bottom,
        !panel_is_statusline(panel),
        focus,
        cursor_out,
      );
    },
    LayoutIntent::SidebarLeft => {
      panel_width = (area.width / 3).max(panel_width.min(area.width));
      let rect = Rect::new(area.x, area.y, panel_width, area.height);
      draw_panel_in_rect(
        buf,
        rect,
        ctx,
        panel,
        BorderEdge::Top,
        false,
        focus,
        cursor_out,
      );
    },
    LayoutIntent::SidebarRight => {
      panel_width = (area.width / 3).max(panel_width.min(area.width));
      let rect = Rect::new(
        area.x + area.width - panel_width,
        area.y,
        panel_width,
        area.height,
      );
      draw_panel_in_rect(
        buf,
        rect,
        ctx,
        panel,
        BorderEdge::Top,
        false,
        focus,
        cursor_out,
      );
    },
    LayoutIntent::Fullscreen => {
      let rect = area;
      draw_panel_in_rect(
        buf,
        rect,
        ctx,
        panel,
        BorderEdge::Top,
        false,
        focus,
        cursor_out,
      );
    },
    LayoutIntent::Custom(_) | LayoutIntent::Floating => {
      let (x, width) = align_horizontal(area, panel_width, panel.constraints.align.horizontal);
      let (y, height) = align_vertical(area, panel_height, panel.constraints.align.vertical);
      let rect = Rect::new(x, y, width, height);
      draw_panel_in_rect(
        buf,
        rect,
        ctx,
        panel,
        BorderEdge::Top,
        false,
        focus,
        cursor_out,
      );
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
    content = Rect::new(
      content.x,
      content.y + 1,
      content.width,
      content.height.saturating_sub(1),
    );
  }

  let content = inset_rect(content, panel.constraints.padding);
  draw_ui_node(buf, content, ctx, &panel.child, focus, None, cursor_out);
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
    return 0;
  }
  match panel.intent {
    LayoutIntent::Top | LayoutIntent::Bottom => 1,
    _ => 0,
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
      BorderEdge::Top => {
        Rect::new(
          rect.x,
          rect.y + 1,
          rect.width,
          rect.height.saturating_sub(1),
        )
      },
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
  let width = (text.chars().count() as u16)
    .saturating_add(2)
    .min(area.width)
    .max(2);
  let height = 3u16.min(area.height).max(1);

  let rect = match tooltip.placement.clone() {
    LayoutIntent::Bottom => Rect::new(area.x, area.y + area.height - height, width, height),
    LayoutIntent::Top => Rect::new(area.x, area.y, width, height),
    LayoutIntent::SidebarLeft => Rect::new(area.x, area.y, width, height),
    LayoutIntent::SidebarRight => Rect::new(area.x + area.width - width, area.y, width, height),
    LayoutIntent::Fullscreen => Rect::new(area.x, area.y, width, height),
    LayoutIntent::Custom(_) | LayoutIntent::Floating => {
      Rect::new(
        area.x + (area.width.saturating_sub(width)) / 2,
        area.y + (area.height.saturating_sub(height)) / 2,
        width,
        height,
      )
    },
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
  if let Some(icon_token) = status.left_icon.as_deref() {
    let glyph = file_picker_icon_glyph(icon_token, false);
    left = match left.split_once("  ") {
      Some((mode, file)) if !file.is_empty() => format!("{mode}  {glyph}  {file}"),
      _ if left.is_empty() => glyph.to_string(),
      _ => format!("{glyph} {left}"),
    };
  }
  truncate_in_place(&mut left, rect.width as usize);
  let left_width = left.chars().count() as u16;

  if !status.right_segments.is_empty() {
    // Styled segments path: render each segment with its own style.
    let separator = "  ";
    let sep_len = separator.chars().count() as u16;

    // Compute total right width.
    let mut total_right: u16 = 0;
    for (i, seg) in status.right_segments.iter().enumerate() {
      total_right = total_right.saturating_add(seg.text.chars().count() as u16);
      if i > 0 {
        total_right = total_right.saturating_add(sep_len);
      }
    }

    // Collision: if left + right >= width, cap left.
    let left_width = if left_width.saturating_add(total_right) >= rect.width {
      let available = rect.width.saturating_sub(total_right.saturating_add(1));
      truncate_in_place(&mut left, available as usize);
      left.chars().count() as u16
    } else {
      left_width
    };

    buf.set_string(rect.x, rect.y, &left, text_style);

    // Render segments right-to-left.
    let mut rx = rect.x.saturating_add(rect.width);
    for (i, seg) in status.right_segments.iter().enumerate().rev() {
      let seg_style = styled_span_style(seg, text_style);
      let text_w = seg.text.chars().count() as u16;
      rx = rx.saturating_sub(text_w);
      if rx >= rect.x.saturating_add(left_width) {
        buf.set_string(rx, rect.y, &seg.text, seg_style);
      }
      if i > 0 {
        rx = rx.saturating_sub(sep_len);
        if rx >= rect.x.saturating_add(left_width) {
          buf.set_string(rx, rect.y, separator, text_style);
        }
      }
    }
  } else {
    // Fallback: plain text path (backward compat).
    let mut center = status.center.clone();
    let mut right = status.right.clone();
    truncate_in_place(&mut right, rect.width as usize);
    truncate_in_place(&mut center, rect.width as usize);

    let mut left_width = left_width;
    let mut right_width = right.chars().count() as u16;
    if left_width.saturating_add(right_width) >= rect.width {
      let available_right = rect.width.saturating_sub(left_width.saturating_add(1));
      truncate_in_place(&mut right, available_right as usize);
      right_width = right.chars().count() as u16;
    }
    if left_width.saturating_add(right_width) >= rect.width {
      let available_left = rect.width.saturating_sub(right_width.saturating_add(1));
      truncate_in_place(&mut left, available_left as usize);
      left_width = left.chars().count() as u16;
    }

    buf.set_string(rect.x, rect.y, &left, text_style);
    if !right.is_empty() {
      let rx = rect.x + rect.width.saturating_sub(right_width);
      buf.set_string(rx, rect.y, right, text_style);
    }
    if !center.is_empty() {
      let center_start = rect.x + left_width.saturating_add(1);
      let center_end = rect
        .x
        .saturating_add(rect.width)
        .saturating_sub(right_width.saturating_add(1));
      if center_end > center_start {
        let center_width = center_end.saturating_sub(center_start);
        truncate_in_place(&mut center, center_width as usize);
        let center_text_width = center.chars().count() as u16;
        let cx = center_start + center_width.saturating_sub(center_text_width) / 2;
        buf.set_string(cx, rect.y, center, text_style);
      }
    }
  }
}

fn styled_span_style(span: &UiStyledSpan, base: Style) -> Style {
  match &span.style {
    None => base,
    Some(s) => {
      let mut style = base;
      if let Some(ref fg) = s.fg {
        if let Some(color) = resolve_ui_color(fg) {
          style = style.fg(color);
        }
      }
      apply_ui_emphasis(style, s.emphasis)
    },
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
    let max_content_width =
      max_content_width_for_intent(panel.intent.clone(), area, border, padding_h);
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
    let height = child_h.saturating_add(divider + padding_v).min(area.height);
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
  draw_divider: bool,
  focus: Option<&the_lib::render::UiFocus>,
  cursor_out: &mut Option<(u16, u16)>,
) {
  if rect.width == 0 || rect.height == 0 {
    return;
  }
  if panel.style.border.is_some() {
    draw_box_with_title(buf, rect, ctx, panel, focus, cursor_out);
  } else {
    let content = draw_flat_panel(buf, rect, ctx, panel, edge, draw_divider);
    draw_ui_node(buf, content, ctx, &panel.child, focus, None, cursor_out);
  }
}

fn draw_ui_overlays(
  buf: &mut Buffer,
  area: Rect,
  ctx: &mut Ctx,
  ui: &UiTree,
  editor_cursor: Option<(u16, u16)>,
  cursor_out: &mut Option<(u16, u16)>,
) {
  ctx.completion_docs_layout = None;
  let mut top_offset: u16 = 0;
  let mut bottom_offset: u16 = 0;
  let focus = ui.focus.as_ref();
  let layers = [
    the_lib::render::UiLayer::Background,
    the_lib::render::UiLayer::Overlay,
    the_lib::render::UiLayer::Tooltip,
  ];
  for layer in layers {
    let layer_nodes: Vec<&UiNode> = ui
      .overlays
      .iter()
      .filter(|node| node_layer(node) == layer)
      .collect();
    let mut index = 0usize;
    while index < layer_nodes.len() {
      let node = layer_nodes[index];
      match node {
        UiNode::Panel(panel) => {
          let term_command_docs_pair = layer_nodes.get(index + 1).and_then(|next| {
            match *next {
              UiNode::Panel(next_panel) if panel_is_term_command_palette_docs(next_panel) => {
                Some(next_panel)
              },
              _ => None,
            }
          });
          let completion_docs_pair = layer_nodes.get(index + 1).and_then(|next| {
            match *next {
              UiNode::Panel(next_panel) if panel_is_completion_docs(next_panel) => Some(next_panel),
              _ => None,
            }
          });
          if panel_is_term_command_palette_list(panel) {
            let available_height = area.height.saturating_sub(top_offset + bottom_offset);
            if available_height > 0 {
              let overlay_area =
                Rect::new(area.x, area.y + top_offset, area.width, available_height);
              let (list_width, list_height) = panel_box_size(panel, overlay_area);
              let list_rect =
                term_command_palette_panel_rect(overlay_area, list_width, list_height);
              draw_panel_in_rect(
                buf,
                list_rect,
                ctx,
                panel,
                BorderEdge::Top,
                false,
                focus,
                cursor_out,
              );

              if let Some(docs_panel) = term_command_docs_pair {
                let (docs_width, docs_height) = panel_box_size(docs_panel, overlay_area);
                let docs_rect =
                  completion_docs_panel_rect(overlay_area, docs_width, docs_height, list_rect);
                if let Some(docs_rect) = docs_rect {
                  draw_panel_in_rect(
                    buf,
                    docs_rect,
                    ctx,
                    docs_panel,
                    BorderEdge::Top,
                    false,
                    focus,
                    cursor_out,
                  );
                }
              }
            }
            index += if term_command_docs_pair.is_some() {
              2
            } else {
              1
            };
            continue;
          }
          if panel_is_completion(panel)
            && matches!(
              panel.intent,
              LayoutIntent::Custom(_) | LayoutIntent::Floating
            )
            && completion_docs_pair.is_some()
          {
            let available_height = area.height.saturating_sub(top_offset + bottom_offset);
            if available_height > 0 {
              let overlay_area =
                Rect::new(area.x, area.y + top_offset, area.width, available_height);
              let (completion_width, completion_height) = panel_box_size(panel, overlay_area);
              let completion_rect = completion_panel_rect(
                overlay_area,
                completion_width,
                completion_height,
                editor_cursor,
              );
              draw_panel_in_rect(
                buf,
                completion_rect,
                ctx,
                panel,
                BorderEdge::Top,
                false,
                focus,
                cursor_out,
              );

              if let Some(docs_panel) = completion_docs_pair {
                let (docs_width, docs_height) = panel_box_size(docs_panel, overlay_area);
                let docs_rect = completion_docs_panel_rect(
                  overlay_area,
                  docs_width,
                  docs_height,
                  completion_rect,
                );
                if let Some(docs_rect) = docs_rect {
                  draw_panel_in_rect(
                    buf,
                    docs_rect,
                    ctx,
                    docs_panel,
                    BorderEdge::Top,
                    false,
                    focus,
                    cursor_out,
                  );
                  ctx.completion_docs_layout =
                    completion_docs_layout_for_panel(ctx, docs_panel, docs_rect);
                }
              }
            }
            index += 2;
            continue;
          }

          match panel.intent.clone() {
            LayoutIntent::Bottom => {
              if matches!(layer, the_lib::render::UiLayer::Tooltip) {
                draw_ui_panel(buf, area, ctx, panel, focus, editor_cursor, cursor_out);
                index += 1;
                continue;
              }
              let available_height = area.height.saturating_sub(top_offset + bottom_offset);
              if available_height == 0 {
                index += 1;
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
              draw_panel_in_rect(
                buf,
                rect,
                ctx,
                panel,
                BorderEdge::Top,
                !panel_is_statusline(panel),
                focus,
                cursor_out,
              );
            },
            LayoutIntent::Top => {
              if matches!(layer, the_lib::render::UiLayer::Tooltip) {
                draw_ui_panel(buf, area, ctx, panel, focus, editor_cursor, cursor_out);
                index += 1;
                continue;
              }
              let available_height = area.height.saturating_sub(top_offset + bottom_offset);
              if available_height == 0 {
                index += 1;
                continue;
              }
              let rect_area = Rect::new(area.x, area.y + top_offset, area.width, available_height);
              let panel_height = panel_height_for_area(panel, rect_area);
              let rect = Rect::new(area.x, area.y + top_offset, area.width, panel_height);
              top_offset = top_offset.saturating_add(panel_height);
              draw_panel_in_rect(
                buf,
                rect,
                ctx,
                panel,
                BorderEdge::Bottom,
                !panel_is_statusline(panel),
                focus,
                cursor_out,
              );
            },
            _ => {
              let available_height = area.height.saturating_sub(top_offset + bottom_offset);
              if available_height == 0 {
                continue;
              }
              let overlay_area =
                Rect::new(area.x, area.y + top_offset, area.width, available_height);
              draw_ui_panel(
                buf,
                overlay_area,
                ctx,
                panel,
                focus,
                editor_cursor,
                cursor_out,
              )
            },
          }
        },
        _ => draw_ui_node(buf, area, ctx, node, focus, editor_cursor, cursor_out),
      }
      index += 1;
    }
  }
  if ctx.completion_docs_layout.is_none() {
    ctx.completion_docs_drag = None;
  }
}

fn is_search_prompt_overlay(node: &UiNode) -> bool {
  matches!(node, UiNode::Panel(panel) if panel.id.starts_with("search_prompt"))
}

fn is_command_palette_overlay(node: &UiNode) -> bool {
  matches!(node, UiNode::Panel(panel) if panel.id.starts_with("command_palette"))
}

fn status_bar_from_overlay_mut(node: &mut UiNode) -> Option<&mut UiStatusBar> {
  match node {
    UiNode::Panel(panel) if panel.id == "statusline" => {
      if let UiNode::StatusBar(status) = panel.child.as_mut() {
        Some(status)
      } else {
        None
      }
    },
    UiNode::StatusBar(status) if status.id.as_deref() == Some("statusline") => Some(status),
    _ => None,
  }
}

fn command_palette_prompt_query_and_cursor(ctx: &Ctx) -> (&str, usize) {
  let raw = ctx.command_prompt.input.as_str();
  if let Some(stripped) = raw.strip_prefix(':') {
    (stripped, ctx.command_prompt.cursor.saturating_sub(1))
  } else {
    (raw, ctx.command_prompt.cursor)
  }
}

fn command_palette_statusline_text(query: &str, cursor: usize) -> String {
  let mut cursor = cursor.min(query.len());
  while cursor > 0 && !query.is_char_boundary(cursor) {
    cursor -= 1;
  }
  if !query.is_char_boundary(cursor) {
    cursor = 0;
  }
  let (before, after) = query.split_at(cursor);
  format!("CMD {before}█{after}")
}

fn term_command_palette_filtered_selection(
  state: &the_default::CommandPaletteState,
) -> Option<(Vec<usize>, Option<usize>)> {
  let filtered = command_palette_filtered_indices(state);
  if filtered.is_empty() {
    return None;
  }
  let selected = state
    .selected
    .and_then(|current| filtered.iter().position(|&idx| idx == current));
  Some((filtered, selected))
}

fn search_statusline_text(query: &str, cursor: usize) -> String {
  let mut cursor = cursor.min(query.len());
  while cursor > 0 && !query.is_char_boundary(cursor) {
    cursor -= 1;
  }
  if !query.is_char_boundary(cursor) {
    cursor = 0;
  }
  let (before, after) = query.split_at(cursor);
  format!("FIND {before}█{after}")
}

fn build_term_command_palette_list_overlay(ctx: &Ctx) -> Option<UiNode> {
  let state = &ctx.command_palette;
  if !state.is_open {
    return None;
  }

  let (query, _) = command_palette_prompt_query_and_cursor(ctx);
  let mut filtered_state = state.clone();
  filtered_state.query = query.to_string();
  let (filtered, selected) = term_command_palette_filtered_selection(&filtered_state)?;
  const MAX_VISIBLE_ITEMS: usize = 10;
  let items: Vec<UiListItem> = filtered
    .iter()
    .filter_map(|index| state.items.get(*index))
    .map(|item| {
      UiListItem {
        title:         item.title.clone(),
        subtitle:      item.subtitle.clone().or_else(|| item.shortcut.clone()),
        description:   None,
        shortcut:      None,
        badge:         item.badge.clone(),
        leading_icon:  item.leading_icon.clone(),
        leading_color: item.leading_color.map(UiColor::Value),
        symbols:       item.symbols.clone(),
        match_indices: None,
        emphasis:      item.emphasis,
        action:        None,
      }
    })
    .collect();
  if items.is_empty() {
    return None;
  }

  let mut list = UiList::new("command_palette_list", items);
  list.fill_width = false;
  list.selected = selected;
  list.scroll = 0;
  list.max_visible = Some(MAX_VISIBLE_ITEMS);
  list.style = list.style.with_role("completion");
  list.style.accent = Some(UiColor::Token(UiColorToken::SelectedBg));
  list.style.border = Some(UiColor::Token(UiColorToken::SelectedText));

  let mut container = UiContainer::column("term_command_palette_container", 0, vec![UiNode::List(
    list,
  )]);
  container.style = container.style.with_role("completion");

  let mut panel = UiPanel::new(
    "term_command_palette_list",
    LayoutIntent::Custom("term_command_palette_list".to_string()),
    UiNode::Container(container),
  );
  panel.style = panel.style.with_role("completion");
  panel.style.border = None;
  panel.layer = UiLayer::Overlay;
  panel.constraints = UiConstraints {
    min_width: None,
    max_width: Some(64),
    min_height: Some(1),
    max_height: Some((MAX_VISIBLE_ITEMS as u16).saturating_add(4)),
    padding: UiInsets {
      left:   1,
      right:  1,
      top:    0,
      bottom: 0,
    },
    align: UiAlignPair {
      horizontal: UiAlign::Start,
      vertical:   UiAlign::End,
    },
    ..UiConstraints::default()
  };

  Some(UiNode::Panel(panel))
}

fn build_term_command_palette_docs_overlay(ctx: &Ctx) -> Option<UiNode> {
  let state = &ctx.command_palette;
  if !state.is_open {
    return None;
  }
  let (query, _) = command_palette_prompt_query_and_cursor(ctx);
  let mut filtered_state = state.clone();
  filtered_state.query = query.to_string();
  let (filtered, selected_filtered) = term_command_palette_filtered_selection(&filtered_state)?;
  let selected_index = *filtered.get(selected_filtered.unwrap_or(0))?;
  let item = state.items.get(selected_index)?;

  let mut docs = String::new();
  if let Some(description) = item.description.as_deref().map(str::trim)
    && !description.is_empty()
  {
    docs.push_str(description);
  }
  if !item.aliases.is_empty() {
    if !docs.is_empty() {
      docs.push_str("\n\n");
    }
    docs.push_str("aliases: ");
    docs.push_str(item.aliases.join(", ").as_str());
  }
  if docs.is_empty() {
    return None;
  }

  let mut docs_text = UiText::new("term_command_palette_docs_text", docs);
  docs_text.style = docs_text.style.with_role("completion_docs");
  docs_text.clip = false;

  let mut docs_container = UiContainer::column("term_command_palette_docs_container", 0, vec![
    UiNode::Text(docs_text),
  ]);
  docs_container.style = docs_container.style.with_role("completion_docs");

  let mut docs_panel = UiPanel::new(
    "term_command_palette_docs",
    LayoutIntent::Custom("term_command_palette_docs".to_string()),
    UiNode::Container(docs_container),
  );
  docs_panel.style = docs_panel.style.with_role("completion_docs");
  docs_panel.style.border = None;
  docs_panel.layer = UiLayer::Overlay;
  docs_panel.constraints = UiConstraints {
    min_width:  Some(28),
    max_width:  Some(84),
    min_height: None,
    max_height: Some(18),
    padding:    UiInsets {
      left:   1,
      right:  1,
      top:    1,
      bottom: 1,
    },
    align:      UiAlignPair {
      horizontal: UiAlign::Start,
      vertical:   UiAlign::End,
    },
  };

  Some(UiNode::Panel(docs_panel))
}

fn adapt_ui_tree_for_term(ctx: &Ctx, ui: &mut UiTree) {
  if ctx.command_palette.is_open {
    ui.overlays.retain(|node| !is_command_palette_overlay(node));
    let (query, cursor) = command_palette_prompt_query_and_cursor(ctx);
    if let Some(status) = ui.overlays.iter_mut().find_map(status_bar_from_overlay_mut) {
      status.left = command_palette_statusline_text(query, cursor);
      status.left_icon = None;
    }
    if let Some(list_overlay) = build_term_command_palette_list_overlay(ctx) {
      ui.overlays.push(list_overlay);
      if let Some(docs_overlay) = build_term_command_palette_docs_overlay(ctx) {
        ui.overlays.push(docs_overlay);
      }
    }
    return;
  }

  if !ctx.search_prompt.active {
    return;
  }

  ui.overlays.retain(|node| !is_search_prompt_overlay(node));
  if ui
    .focus
    .as_ref()
    .is_some_and(|focus| focus.id.starts_with("search_prompt"))
  {
    ui.focus = None;
  }

  if let Some(status) = ui.overlays.iter_mut().find_map(status_bar_from_overlay_mut) {
    status.left =
      search_statusline_text(ctx.search_prompt.query.as_str(), ctx.search_prompt.cursor);
    status.left_icon = None;
  }
}

pub fn build_render_plan(ctx: &mut Ctx) -> RenderPlan {
  let styles = render_styles_from_theme(&ctx.ui_theme);
  build_render_plan_with_styles(ctx, styles)
}

pub fn build_render_plan_with_styles(ctx: &mut Ctx, styles: RenderStyles) -> RenderPlan {
  let view = ctx.editor.view();
  let gutter_width = gutter_width_for_document(
    ctx.editor.document(),
    view.viewport.width,
    &ctx.gutter_config,
  );
  let diagnostics_by_line = active_diagnostics_by_line(ctx);
  let diagnostic_styles = render_diagnostic_styles_from_theme(&ctx.ui_theme);
  let diff_styles = render_diff_styles_from_theme(&ctx.ui_theme);
  let diff_signs = ctx.gutter_diff_signs.clone();

  // Set up text formatting
  ctx.text_format.viewport_width = view.viewport.width.saturating_sub(gutter_width).max(1);
  let text_fmt = &ctx.text_format;

  // Set up annotations
  let mut annotations = TextAnnotations::default();
  if !ctx.inline_annotations.is_empty() {
    let _ = annotations.add_inline_annotations(&ctx.inline_annotations, None);
  }
  if !ctx.overlay_annotations.is_empty() {
    let _ = annotations.add_overlay(&ctx.overlay_annotations, None);
  }

  let allow_cache_refresh = ctx.syntax_highlight_refresh_allowed();
  let (doc, render_cache) = ctx.editor.document_and_cache();

  // Build the render plan (with or without syntax highlighting)
  let mut plan = if let (Some(loader), Some(syntax)) = (&ctx.loader, doc.syntax()) {
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
      doc.syntax_version(),
      allow_cache_refresh,
    );

    build_plan(
      doc,
      view,
      text_fmt,
      &ctx.gutter_config,
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
      &ctx.gutter_config,
      &mut annotations,
      &mut highlights,
      render_cache,
      styles,
    )
  };

  apply_diagnostic_gutter_markers(&mut plan, &diagnostics_by_line, diagnostic_styles);
  apply_diff_gutter_markers(&mut plan, &diff_signs, diff_styles);
  plan
}

/// Render the current document state to the terminal.
pub fn render(f: &mut Frame, ctx: &mut Ctx) {
  let area = f.size();
  sync_file_picker_viewport(ctx, area);
  let suppress_editor_cursor = ctx.command_palette.is_open || ctx.search_prompt.active;

  let mut ui = ui_tree(ctx);
  adapt_ui_tree_for_term(ctx, &mut ui);
  resolve_ui_tree(&mut ui, &ctx.ui_theme);
  apply_ui_viewport(ctx, &ui, f.size());
  ensure_cursor_visible(ctx);
  let plan = render_plan(ctx);

  f.render_widget(Clear, area);

  let ui_cursor = {
    let buf = f.buffer_mut();
    let mut cursor_out = None;
    let content_x = area.x.saturating_add(plan.content_offset_x);
    let editor_cursor = plan.cursors.first().map(|cursor| {
      (
        content_x + cursor.pos.col as u16,
        area.y + cursor.pos.row as u16,
      )
    });
    let base_text_style = lib_style_to_ratatui(ctx.ui_theme.try_get("ui.text").unwrap_or_default());
    if let Some(bg) = ctx
      .ui_theme
      .try_get("ui.background")
      .and_then(|style| style.bg)
    {
      fill_rect(buf, area, Style::default().bg(lib_color_to_ratatui(bg)));
    }

    if plan.content_offset_x > 0 {
      for line in &plan.gutter_lines {
        let y = area.y + line.row;
        if y >= area.y + area.height {
          continue;
        }
        for span in &line.spans {
          let x = area.x + span.col;
          if x >= content_x {
            continue;
          }
          let max_width = content_x.saturating_sub(x) as usize;
          if max_width == 0 {
            continue;
          }
          let text = if is_diff_gutter_marker(span.text.as_str()) {
            "▏"
          } else {
            span.text.as_str()
          };
          buf.set_stringn(x, y, text, max_width, lib_style_to_ratatui(span.style));
        }
      }
    }

    for selection in &plan.selections {
      let rect = Rect::new(
        content_x + selection.rect.x,
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
        let x = content_x + span.col;
        if x >= area.x + area.width {
          continue;
        }
        let style = span
          .highlight
          .map(|highlight| {
            base_text_style.patch(lib_style_to_ratatui(ctx.ui_theme.highlight(highlight)))
          })
          .unwrap_or(base_text_style);
        buf.set_string(x, y, span.text.as_str(), style);
      }
    }

    // Draw cursors
    for cursor in &plan.cursors {
      let x = content_x + cursor.pos.col as u16;
      let y = area.y + cursor.pos.row as u16;
      if x < area.x + area.width && y < area.y + area.height {
        let style = lib_style_to_ratatui(cursor.style);
        let cell = buf.get_mut(x, y);
        let merged = cell.style().patch(style);
        cell.set_style(merged);
      }
    }

    // Draw UI root and overlays.
    draw_ui_node(
      buf,
      area,
      ctx,
      &ui.root,
      ui.focus.as_ref(),
      editor_cursor,
      &mut cursor_out,
    );
    draw_ui_overlays(buf, area, ctx, &ui, editor_cursor, &mut cursor_out);
    cursor_out
  };

  if let Some((x, y)) = ui_cursor {
    f.set_cursor(x, y);
  } else if !suppress_editor_cursor {
    if let Some(cursor) = plan.cursors.first() {
      let x = area.x + plan.content_offset_x + cursor.pos.col as u16;
      let y = area.y + cursor.pos.row as u16;
      if x < area.x + area.width && y < area.y + area.height {
        f.set_cursor(x, y);
      }
    }
  }
}

fn is_diff_gutter_marker(text: &str) -> bool {
  matches!(text.trim(), "+" | "~" | "-")
}

fn sync_file_picker_viewport(ctx: &mut Ctx, area: Rect) {
  if !ctx.file_picker.active {
    ctx.file_picker_layout = None;
    ctx.file_picker_drag = None;
    return;
  }

  let Some(layout) = compute_file_picker_layout(area, &ctx.file_picker) else {
    set_picker_visible_rows(&mut ctx.file_picker, 1);
    ctx.file_picker.clamp_preview_scroll(1);
    ctx.file_picker_layout = None;
    ctx.file_picker_drag = None;
    return;
  };

  set_picker_visible_rows(&mut ctx.file_picker, layout.list_visible_rows());
  ctx
    .file_picker
    .clamp_preview_scroll(layout.preview_visible_rows());
  ctx.file_picker_layout = compute_file_picker_layout(area, &ctx.file_picker);
  if ctx.file_picker_layout.is_none() {
    ctx.file_picker_drag = None;
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
  let gutter_width = gutter_width_for_document(doc, view.viewport.width, &ctx.gutter_config);
  let viewport_width = view.viewport.width.saturating_sub(gutter_width).max(1) as usize;

  if ctx.text_format.soft_wrap {
    let mut changed = false;
    let mut new_scroll = view.scroll;

    if let Some(new_row) = the_lib::view::scroll_row_to_keep_visible(
      cursor_line,
      view.scroll.row,
      viewport_height,
      ctx.scrolloff,
    ) {
      new_scroll.row = new_row;
      changed = true;
    }

    if view.scroll.col != 0 {
      new_scroll.col = 0;
      changed = true;
    }

    if changed {
      ctx.editor.view_mut().scroll = new_scroll;
    }
    return;
  }

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

#[cfg(test)]
mod tests {
  use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Style,
  };
  use the_default::{
    CommandPaletteItem,
    CommandPaletteState,
  };
  use the_lib::render::{
    LayoutIntent,
    UiConstraints,
    UiContainer,
    UiInsets,
    UiList,
    UiListItem,
    UiNode,
    UiPanel,
  };

  use super::{
    CompletionDocsStyles,
    StyledTextRun,
    completion_docs_panel_rect,
    completion_docs_rows,
    completion_panel_rect,
    draw_ui_list,
    panel_box_size,
    term_command_palette_filtered_selection,
  };

  fn flatten_rows(rows: &[Vec<StyledTextRun>]) -> Vec<String> {
    rows
      .iter()
      .map(|row| {
        row
          .iter()
          .map(|run| run.text.as_str())
          .collect::<Vec<_>>()
          .join("")
      })
      .collect()
  }

  #[test]
  fn completion_panel_rect_places_below_cursor_when_space_exists() {
    let area = Rect::new(0, 0, 100, 30);
    let rect = completion_panel_rect(area, 32, 8, Some((40, 10)));
    assert_eq!(rect.y, 11);
    assert_eq!(rect.width, 32);
    assert_eq!(rect.height, 8);
  }

  #[test]
  fn completion_panel_rect_flips_above_when_below_is_tight() {
    let area = Rect::new(0, 0, 80, 12);
    let rect = completion_panel_rect(area, 30, 8, Some((20, 10)));
    assert!(rect.y < 10);
    assert_eq!(rect.height, 8);
  }

  #[test]
  fn completion_panel_rect_clamps_to_viewport_bounds() {
    let area = Rect::new(5, 3, 20, 10);
    let rect = completion_panel_rect(area, 18, 9, Some((500, 500)));
    assert!(rect.x >= area.x);
    assert!(rect.y >= area.y);
    assert!(rect.x + rect.width <= area.x + area.width);
    assert!(rect.y + rect.height <= area.y + area.height);
  }

  #[test]
  fn completion_docs_panel_rect_prefers_right_side() {
    let area = Rect::new(0, 0, 100, 30);
    let completion_rect = Rect::new(20, 9, 30, 8);
    let docs_rect = completion_docs_panel_rect(area, 24, 10, completion_rect).expect("docs rect");
    assert_eq!(docs_rect.x, 51);
    assert_eq!(docs_rect.y, completion_rect.y);
  }

  #[test]
  fn completion_docs_panel_rect_flips_left_when_right_is_tight() {
    let area = Rect::new(0, 0, 70, 20);
    let completion_rect = Rect::new(45, 4, 24, 8);
    let docs_rect = completion_docs_panel_rect(area, 20, 8, completion_rect).expect("docs rect");
    assert_eq!(docs_rect.x, 24);
    assert_eq!(docs_rect.y, completion_rect.y);
  }

  #[test]
  fn completion_docs_panel_rect_hides_when_viewport_is_narrow() {
    let area = Rect::new(0, 0, 72, 22);
    let completion_rect = Rect::new(4, 5, 46, 10);
    let docs_rect = completion_docs_panel_rect(area, 40, 9, completion_rect);
    assert!(docs_rect.is_none());
  }

  #[test]
  fn completion_docs_panel_rect_hides_when_side_space_is_unavailable() {
    let area = Rect::new(0, 0, 80, 24);
    let completion_rect = Rect::new(2, 6, 76, 10);
    let docs_rect = completion_docs_panel_rect(area, 36, 9, completion_rect);
    assert!(docs_rect.is_none());
  }

  #[test]
  fn term_command_palette_selection_stays_empty_without_explicit_selection() {
    let state = CommandPaletteState {
      is_open:     true,
      query:       String::new(),
      selected:    None,
      items:       vec![
        CommandPaletteItem::new("help"),
        CommandPaletteItem::new("quit"),
      ],
      max_results: 10,
    };

    let (filtered, selected) =
      term_command_palette_filtered_selection(&state).expect("filtered selection");
    assert_eq!(filtered, vec![0, 1]);
    assert_eq!(selected, None);
  }

  #[test]
  fn completion_list_scrollbar_renders_thumb_without_track_line() {
    let items = (0..10)
      .map(|idx| UiListItem::new(format!("item-{idx}")))
      .collect();
    let mut list = UiList::new("completion_list", items);
    list.style = list.style.with_role("completion");
    list.selected = Some(0);
    list.scroll = 0;
    list.max_visible = Some(3);

    let rect = Rect::new(0, 0, 24, 3);
    let mut buf = Buffer::empty(rect);
    let mut cursor = None;
    draw_ui_list(&mut buf, rect, &list, &mut cursor);

    let track_x = rect.x + rect.width - 1;
    let selected_row_cell = buf.get(track_x, rect.y);
    assert_eq!(selected_row_cell.symbol(), "█");

    let next_row_cell = buf.get(track_x, rect.y + 1);
    assert_eq!(next_row_cell.symbol(), " ");
  }

  #[test]
  fn completion_panel_size_allows_single_row_without_minimum_height() {
    let mut list = UiList::new("completion_list", vec![UiListItem::new("std")]);
    list.fill_width = false;
    list.style = list.style.with_role("completion");

    let mut container = UiContainer::column("completion_container", 0, vec![UiNode::List(list)]);
    container.style = container.style.with_role("completion");

    let mut panel = UiPanel::new(
      "completion",
      LayoutIntent::Custom("completion".to_string()),
      UiNode::Container(container),
    );
    panel.style = panel.style.with_role("completion");
    panel.style.border = None;
    panel.constraints = UiConstraints::panel();
    panel.constraints.padding = UiInsets {
      left:   0,
      right:  0,
      top:    0,
      bottom: 0,
    };
    panel.constraints.min_width = None;

    let area = Rect::new(0, 0, 80, 20);
    let (width, height) = panel_box_size(&panel, area);

    assert_eq!(width, 3);
    assert_eq!(height, 1);
  }

  #[test]
  fn completion_docs_rows_parse_markdown_basics() {
    let styles = CompletionDocsStyles::default(Style::default());
    let rows = completion_docs_rows(
      "# Title\n- item\n[Result](https://example.com)\n```rs\nfn test() {}\n```",
      &styles,
      80,
    );
    let non_empty: Vec<_> = flatten_rows(&rows)
      .into_iter()
      .filter(|line| !line.trim().is_empty())
      .collect();
    assert_eq!(non_empty, vec![
      "Title".to_string(),
      "• item".to_string(),
      "Result".to_string(),
      "fn test() {}".to_string(),
    ]);
  }

  #[test]
  fn completion_docs_rows_wrap_long_lines() {
    let styles = CompletionDocsStyles::default(Style::default());
    let rows = completion_docs_rows("abcdef", &styles, 3);
    assert_eq!(flatten_rows(&rows), vec![
      "abc".to_string(),
      "def".to_string()
    ]);
  }
}
