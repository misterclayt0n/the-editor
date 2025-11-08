use the_editor_renderer::{
  Color,
  TextSection,
};

use super::button::Button;
use crate::{
  core::{
    DocumentId,
    ViewId,
    document::Document,
  },
  editor::Editor,
  ui::{
    UI_FONT_SIZE,
    compositor::Surface,
    theme_color_to_renderer_color,
  },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BufferKind {
  Document(DocumentId),
  Terminal(ViewId),
}

#[derive(Debug, Clone, Copy)]
pub struct BufferTab {
  pub kind:    BufferKind,
  pub start_x: f32,
  pub end_x:   f32,
}

fn theme_color(style: &crate::core::graphics::Style, fallback: Color) -> Color {
  style
    .fg
    .map(theme_color_to_renderer_color)
    .unwrap_or(fallback)
}

fn display_name(doc: &Document) -> std::borrow::Cow<'_, str> {
  doc.short_name()
}

fn with_alpha(color: Color, alpha: f32) -> Color {
  Color::new(color.r, color.g, color.b, alpha.clamp(0.0, 1.0))
}

fn mix(a: Color, b: Color, t: f32) -> Color {
  let t = t.clamp(0.0, 1.0);
  Color::new(
    a.r * (1.0 - t) + b.r * t,
    a.g * (1.0 - t) + b.g * t,
    a.b * (1.0 - t) + b.b * t,
    a.a * (1.0 - t) + b.a * t,
  )
}

fn glow_rgb_from_base(base: Color) -> Color {
  let lum = 0.2126 * base.r + 0.7152 * base.g + 0.0722 * base.b;
  let t = if lum < 0.35 {
    0.75
  } else if lum < 0.65 {
    0.55
  } else {
    0.35
  };
  mix(base, Color::WHITE, t)
}

pub fn render(
  editor: &Editor,
  origin_x: f32,
  origin_y: f32,
  viewport_width: f32,
  surface: &mut Surface,
  hover_index: Option<usize>,
  pressed_index: Option<usize>,
  tabs: &mut Vec<BufferTab>,
) -> f32 {
  tabs.clear();

  let saved_font = surface.save_font_state();
  let ui_font_family = surface.current_font_family().to_owned();
  surface.configure_font(&ui_font_family, UI_FONT_SIZE);

  let cell_width = surface.cell_width().max(1.0);
  let base_cell_height = surface.cell_height().max(UI_FONT_SIZE + 4.0);
  let cell_height = (base_cell_height + 4.0).max(UI_FONT_SIZE + 8.0);
  let tab_height = (cell_height - 4.0).max(UI_FONT_SIZE + 2.0);
  let tab_top = origin_y + (cell_height - tab_height) * 0.5;
  let text_y = tab_top + (tab_height - UI_FONT_SIZE) * 0.5;

  let theme = &editor.theme;
  let base_bg_style = theme
    .try_get("ui.bufferline.background")
    .unwrap_or_else(|| theme.get("ui.statusline"));
  let active_style = theme
    .try_get("ui.bufferline.active")
    .unwrap_or_else(|| theme.get("ui.statusline.active"));
  let inactive_style = theme
    .try_get("ui.bufferline")
    .unwrap_or_else(|| theme.get("ui.statusline.inactive"));

  let base_bg = base_bg_style
    .bg
    .map(theme_color_to_renderer_color)
    .or_else(|| {
      theme
        .get("ui.background")
        .bg
        .map(theme_color_to_renderer_color)
    })
    .unwrap_or(Color::new(0.12, 0.12, 0.15, 1.0));

  surface.draw_rect(origin_x, origin_y, viewport_width, cell_height, base_bg);
  let separator_color = base_bg.lerp(Color::WHITE, 0.05);
  surface.draw_rect(origin_x, origin_y, viewport_width, 1.0, separator_color);
  surface.draw_rect(
    origin_x,
    origin_y + cell_height - 1.0,
    viewport_width,
    1.0,
    separator_color,
  );

  let mut cursor_x = origin_x;
  let max_x = origin_x + viewport_width;
  let current_doc_id = editor
    .focused_view_id()
    .and_then(|view_id| editor.tree.try_get(view_id).map(|view| view.doc));
  let current_terminal_id = editor
    .tree
    .get_terminal(editor.tree.focus)
    .map(|_| editor.tree.focus);
  let button_base = theme
    .try_get_exact("ui.button")
    .and_then(|style| style.fg)
    .map(theme_color_to_renderer_color)
    .unwrap_or(Color::new(0.45, 0.47, 0.50, 1.0));
  let button_highlight = theme
    .try_get_exact("ui.button.highlight")
    .and_then(|style| style.fg)
    .map(theme_color_to_renderer_color)
    .unwrap_or_else(|| glow_rgb_from_base(button_base));

  struct TabEntry {
    label: String,
    kind:  BufferKind,
  }

  let mut entries: Vec<TabEntry> = Vec::new();

  for doc in editor.documents() {
    let mut label = format!(" {} ", display_name(doc));
    if doc.is_modified() {
      label.push_str("[+]");
    }
    label.push(' ');
    entries.push(TabEntry {
      label,
      kind: BufferKind::Document(doc.id()),
    });
  }

  for terminal_entry in editor.terminal_tab_entries() {
    let mut label = format!(" terminal #{} ", terminal_entry.terminal.id);
    label.push(' ');
    entries.push(TabEntry {
      label,
      kind: BufferKind::Terminal(terminal_entry.view_id),
    });
  }

  for (tab_index, entry) in entries.into_iter().enumerate() {
    let TabEntry { mut label, kind } = entry;

    let is_active = match kind {
      BufferKind::Document(doc_id) => current_doc_id == Some(doc_id),
      BufferKind::Terminal(view_id) => current_terminal_id == Some(view_id),
    };

    let style = if is_active {
      active_style
    } else if Some(tab_index) == hover_index {
      active_style
    } else {
      inactive_style
    };

    let fg = theme_color(&style, Color::rgb(0.85, 0.85, 0.9));

    if cursor_x >= max_x {
      break;
    }

    let remaining = max_x - cursor_x;
    let padding = (cell_width * 0.75).clamp(10.0, 16.0);
    let usable_width = (remaining - padding * 2.0).max(cell_width);
    if remaining < cell_width {
      break;
    }

    let max_chars = ((usable_width / cell_width).floor() as usize).max(1);
    if label.chars().count() > max_chars {
      label = label.chars().take(max_chars.saturating_sub(1)).collect();
      label.push('…');
    }

    let text_width = label.chars().count() as f32 * cell_width;
    let draw_width = (text_width + padding * 2.0).min(remaining);
    if draw_width < padding * 2.0 {
      break;
    }
    let end_x = (cursor_x + draw_width).min(max_x);

    let is_hovered = Some(tab_index) == hover_index;
    let is_pressed = Some(tab_index) == pressed_index;

    let mut text_color = fg;
    if is_active {
      text_color = text_color.lerp(Color::WHITE, 0.18);
    }
    if is_hovered {
      let hover_mix = if is_pressed { 0.25 } else { 0.35 };
      text_color = text_color.lerp(Color::WHITE, hover_mix);

      let outline_color = with_alpha(button_base, 0.95);
      let bottom_thickness = (tab_height * 0.035).clamp(0.6, 1.4);
      let side_thickness = (bottom_thickness * 1.55).min(bottom_thickness + 1.8);
      let top_thickness = (bottom_thickness * 2.3).min(bottom_thickness + 2.6);
      surface.draw_rounded_rect_stroke_fade(
        cursor_x,
        tab_top,
        draw_width,
        tab_height,
        0.0,
        top_thickness,
        side_thickness,
        bottom_thickness,
        outline_color,
      );

      let hover_strength = if is_pressed { 0.1 } else { 1.0 };
      if hover_strength > 0.0 {
        Button::draw_hover_layers(
          surface,
          cursor_x,
          tab_top,
          draw_width,
          tab_height,
          0.0,
          button_highlight,
          hover_strength,
        );
      }

      if is_pressed {
        let glow_alpha = 0.12;
        let bottom_glow = Color::new(
          button_highlight.r,
          button_highlight.g,
          button_highlight.b,
          glow_alpha,
        );
        let bottom_center_y = tab_top + tab_height + 1.5;
        let bottom_radius = (draw_width * 0.45).max(tab_height * 0.42);
        surface.draw_rounded_rect_glow(
          cursor_x,
          tab_top,
          draw_width,
          tab_height,
          0.0,
          cursor_x + draw_width * 0.5,
          bottom_center_y,
          bottom_radius,
          bottom_glow,
        );
      }
    }

    surface.draw_text(TextSection::simple(
      cursor_x + padding,
      text_y,
      &label,
      UI_FONT_SIZE,
      text_color,
    ));

    tabs.push(BufferTab {
      kind,
      start_x: cursor_x,
      end_x,
    });

    cursor_x = end_x;

    if cursor_x + 1.0 < max_x {
      surface.draw_rect(
        cursor_x,
        tab_top + 2.0,
        1.0,
        tab_height - 4.0,
        separator_color,
      );
      cursor_x += 1.0;
    }
  }

  surface.restore_font_state(saved_font);
  cell_height
}
