use std::collections::HashMap;

use the_editor_renderer::{
  Color,
  TextSection,
};

use super::button::Button;
use crate::{
  core::{
    DocumentId,
    document::Document,
    graphics::Rect,
  },
  editor::Editor,
  ui::{
    UI_FONT_SIZE,
    compositor::Surface,
    file_icons,
    theme_color_to_renderer_color,
  },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BufferKind {
  Document(DocumentId),
}

/// Hit testing regions for a single tab
#[derive(Debug, Clone, Copy)]
pub struct BufferTab {
  pub kind:         BufferKind,
  pub start_x:      f32,
  pub end_x:        f32,
  /// Close button hit region (only valid when tab is hovered)
  pub close_start_x: f32,
  pub close_end_x:   f32,
}

/// Per-tab animation state using exponential decay
#[derive(Debug, Clone, Copy, Default)]
pub struct TabAnimationState {
  /// Hover state (0.0 = not hovered, 1.0 = fully hovered)
  pub hover_t:       f32,
  /// Close button hover state
  pub close_hover_t: f32,
  /// Press/click state
  pub pressed_t:     f32,
}

impl TabAnimationState {
  /// Update animation state using exponential decay
  pub fn update(&mut self, dt: f32, is_hovered: bool, is_close_hovered: bool, is_pressed: bool) {
    // Exponential decay formula: rate = 1 - 2^(-60 * dt)
    let rate = 1.0 - 2.0_f32.powf(-60.0 * dt);

    let hover_target = if is_hovered { 1.0 } else { 0.0 };
    let close_target = if is_close_hovered { 1.0 } else { 0.0 };
    let press_target = if is_pressed { 1.0 } else { 0.0 };

    self.hover_t += (hover_target - self.hover_t) * rate;
    self.close_hover_t += (close_target - self.close_hover_t) * rate;
    self.pressed_t += (press_target - self.pressed_t) * rate;

    // Snap to target when close enough
    if (self.hover_t - hover_target).abs() < 0.01 {
      self.hover_t = hover_target;
    }
    if (self.close_hover_t - close_target).abs() < 0.01 {
      self.close_hover_t = close_target;
    }
    if (self.pressed_t - press_target).abs() < 0.01 {
      self.pressed_t = press_target;
    }
  }

  /// Returns true if any animation is in progress
  pub fn is_animating(&self) -> bool {
    self.hover_t > 0.01 || self.close_hover_t > 0.01 || self.pressed_t > 0.01
  }
}

/// State for the add (+) button
#[derive(Debug, Clone, Copy, Default)]
pub struct AddButtonState {
  pub hover_t:   f32,
  pub pressed_t: f32,
}

impl AddButtonState {
  pub fn update(&mut self, dt: f32, is_hovered: bool, is_pressed: bool) {
    let rate = 1.0 - 2.0_f32.powf(-60.0 * dt);

    let hover_target = if is_hovered { 1.0 } else { 0.0 };
    let press_target = if is_pressed { 1.0 } else { 0.0 };

    self.hover_t += (hover_target - self.hover_t) * rate;
    self.pressed_t += (press_target - self.pressed_t) * rate;

    if (self.hover_t - hover_target).abs() < 0.01 {
      self.hover_t = hover_target;
    }
    if (self.pressed_t - press_target).abs() < 0.01 {
      self.pressed_t = press_target;
    }
  }

  pub fn is_animating(&self) -> bool {
    self.hover_t > 0.01 || self.pressed_t > 0.01
  }
}

fn with_alpha(color: Color, alpha: f32) -> Color {
  Color::new(color.r, color.g, color.b, alpha.clamp(0.0, 1.0))
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
  base.lerp(Color::WHITE, t)
}

fn display_name(doc: &Document) -> std::borrow::Cow<'_, str> {
  doc.display_name()
}

/// Render result containing height, scroll info, and add button bounds
pub struct RenderResult {
  pub height:              f32,
  pub add_button_rect:     Option<Rect>,
  pub total_content_width: f32,
  pub max_scroll:          f32,
  pub active_tab_index:    Option<usize>,
}

/// Render the bufferline with RAD-style tabs and horizontal scrolling
#[allow(clippy::too_many_arguments)]
pub fn render(
  editor: &Editor,
  origin_x: f32,
  origin_y: f32,
  viewport_width: f32,
  surface: &mut Surface,
  hover_index: Option<usize>,
  pressed_index: Option<usize>,
  close_hover_index: Option<usize>,
  add_button_hovered: bool,
  add_button_pressed: bool,
  tabs: &mut Vec<BufferTab>,
  animation_states: &mut HashMap<DocumentId, TabAnimationState>,
  add_button_state: &mut AddButtonState,
  scroll_offset: f32,
  dt: f32,
) -> RenderResult {
  tabs.clear();

  let saved_font = surface.save_font_state();
  let ui_font_family = surface.current_font_family().to_owned();
  surface.configure_font(&ui_font_family, UI_FONT_SIZE);

  let base_cell_height = surface.cell_height().max(UI_FONT_SIZE + 4.0);
  let cell_height = (base_cell_height + 4.0).max(UI_FONT_SIZE + 8.0);
  let tab_height = (cell_height - 4.0).max(UI_FONT_SIZE + 2.0);
  let tab_top = origin_y + (cell_height - tab_height) * 0.5;
  let text_y = tab_top + (tab_height - UI_FONT_SIZE) * 0.5;

  let theme = &editor.theme;

  // Derive colors from existing theme values (not bufferline-specific)
  let base_bg = theme
    .try_get("ui.statusline")
    .and_then(|s| s.bg)
    .map(theme_color_to_renderer_color)
    .or_else(|| {
      theme
        .get("ui.background")
        .bg
        .map(theme_color_to_renderer_color)
    })
    .unwrap_or(Color::new(0.12, 0.12, 0.15, 1.0));

  // Active tab: derive from selection or cursor
  let active_accent = theme
    .try_get("ui.selection")
    .and_then(|s| s.bg)
    .map(theme_color_to_renderer_color)
    .unwrap_or(Color::new(0.35, 0.4, 0.5, 1.0));

  let active_border = theme
    .try_get("ui.cursor.primary")
    .and_then(|s| s.fg)
    .map(theme_color_to_renderer_color)
    .unwrap_or(active_accent);

  // Text colors
  let active_text = theme
    .try_get("ui.statusline.active")
    .and_then(|s| s.fg)
    .map(theme_color_to_renderer_color)
    .unwrap_or(Color::rgb(0.95, 0.95, 0.98));

  let inactive_text = theme
    .try_get("ui.statusline.inactive")
    .and_then(|s| s.fg)
    .map(theme_color_to_renderer_color)
    .unwrap_or(Color::rgb(0.6, 0.62, 0.65));

  // Button/highlight colors
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

  // Draw background
  surface.draw_rect(origin_x, origin_y, viewport_width, cell_height, base_bg);

  // Draw subtle separators
  let separator_color = base_bg.lerp(Color::WHITE, 0.05);
  surface.draw_rect(origin_x, origin_y, viewport_width, 1.0, separator_color);
  surface.draw_rect(
    origin_x,
    origin_y + cell_height - 1.0,
    viewport_width,
    1.0,
    separator_color,
  );

  let current_doc_id = editor
    .focused_view_id()
    .and_then(|view_id| editor.tree.try_get(view_id).and_then(|view| view.doc()));

  // Collect documents to render
  let documents: Vec<_> = editor.documents().collect();

  // Layout constants
  let icon_size = (UI_FONT_SIZE * 1.0) as u32;
  let icon_padding = 6.0;
  let text_padding = 8.0;
  let close_button_width = UI_FONT_SIZE * 1.2;
  let tab_spacing = 2.0;
  let min_tab_width = text_padding * 2.0 + icon_size as f32 + 20.0;

  let left_margin = 4.0;
  let add_button_reserved = tab_height + 12.0;
  let available_width = viewport_width - left_margin - add_button_reserved;

  // Calculate natural width for each tab - sized exactly to content, no maximum
  let tab_widths: Vec<f32> = documents
    .iter()
    .map(|doc| {
      let name = display_name(doc);
      let display_text = if doc.is_modified() {
        format!("{} •", name)
      } else {
        name.to_string()
      };
      let text_width = surface.measure_text(&display_text, UI_FONT_SIZE);
      (text_padding + icon_size as f32 + icon_padding + text_width + close_button_width + text_padding)
        .max(min_tab_width)
    })
    .collect();

  // Calculate total content width
  let total_spacing = if tab_widths.is_empty() {
    0.0
  } else {
    (tab_widths.len() - 1) as f32 * tab_spacing
  };
  let total_content_width: f32 = tab_widths.iter().sum::<f32>() + total_spacing;

  // Calculate scroll bounds
  let max_scroll = (total_content_width - available_width).max(0.0);
  let scroll_offset = scroll_offset.clamp(0.0, max_scroll);

  // Find active tab index
  let active_tab_index = documents
    .iter()
    .position(|doc| Some(doc.id()) == current_doc_id);

  // Start cursor at left margin minus scroll offset
  let mut cursor_x = origin_x + left_margin - scroll_offset;
  let clip_left = origin_x + left_margin;
  let clip_right = origin_x + viewport_width - add_button_reserved;

  // Push scissor rect to clip tab content to the visible area
  surface.push_scissor_rect(clip_left, origin_y, clip_right - clip_left, cell_height);

  for (tab_index, doc) in documents.iter().enumerate() {
    let draw_width = tab_widths[tab_index];
    let tab_start = cursor_x;
    let tab_end = cursor_x + draw_width;

    let doc_id = doc.id();
    let is_active = Some(doc_id) == current_doc_id;
    let is_hovered = Some(tab_index) == hover_index;
    let is_pressed = Some(tab_index) == pressed_index;
    let is_close_hovered = Some(tab_index) == close_hover_index;

    // Get or create animation state for this tab
    let anim = animation_states.entry(doc_id).or_default();
    anim.update(dt, is_hovered, is_close_hovered, is_pressed);

    // Store tab bounds for hit testing (even if not visible, for scroll calculations)
    tabs.push(BufferTab {
      kind: BufferKind::Document(doc_id),
      start_x: tab_start,
      end_x: tab_end,
      close_start_x: tab_end - close_button_width - text_padding * 0.5,
      close_end_x: tab_end - text_padding * 0.5,
    });

    // Skip rendering if tab is completely outside visible area
    let is_visible = tab_end > clip_left && tab_start < clip_right;
    if !is_visible {
      cursor_x = tab_end + tab_spacing;
      continue;
    }

    // Calculate clipped dimensions for rectangles (text is clipped by scissor rect)
    let clipped_start_x = cursor_x.max(clip_left);
    let clipped_end_x = tab_end.min(clip_right);
    let clipped_width = (clipped_end_x - clipped_start_x).max(0.0);

    // Get file name and icon
    let name = display_name(doc);
    let name_str = name.as_ref();
    let icon = file_icons::icon_for_file(name_str);

    let end_x = tab_end;

    // Determine colors based on state
    let (text_color, bg_alpha) = if is_active {
      (active_text.lerp(Color::WHITE, 0.1), 0.15 + anim.hover_t * 0.1)
    } else if anim.hover_t > 0.0 {
      (
        inactive_text.lerp(active_text, anim.hover_t),
        anim.hover_t * 0.12,
      )
    } else {
      (inactive_text, 0.0)
    };

    // Draw tab background (subtle fill on hover/active) - use clipped dimensions
    if bg_alpha > 0.0 && clipped_width > 0.0 {
      let bg_color = with_alpha(active_accent, bg_alpha);
      surface.draw_rounded_rect(clipped_start_x, tab_top, clipped_width, tab_height, 3.0, bg_color);
    }

    // Draw border/outline with directional thickness when hovered - use clipped dimensions
    if (anim.hover_t > 0.1 || is_active) && clipped_width > 0.0 {
      let border_strength = if is_active {
        0.6 + anim.hover_t * 0.4
      } else {
        anim.hover_t * 0.8
      };
      let outline_color = with_alpha(
        if is_active { active_border } else { button_base },
        border_strength,
      );

      let bottom_thickness = (tab_height * 0.035).clamp(0.6, 1.4);
      let side_thickness = (bottom_thickness * 1.55).min(bottom_thickness + 1.8);
      let top_thickness = (bottom_thickness * 2.3).min(bottom_thickness + 2.6);

      surface.draw_rounded_rect_stroke_fade(
        clipped_start_x,
        tab_top,
        clipped_width,
        tab_height,
        3.0,
        top_thickness * anim.hover_t.max(if is_active { 0.5 } else { 0.0 }),
        side_thickness * anim.hover_t.max(if is_active { 0.3 } else { 0.0 }),
        bottom_thickness * anim.hover_t.max(if is_active { 0.2 } else { 0.0 }),
        outline_color,
      );
    }

    // Draw hover glow layers - use clipped dimensions
    if anim.hover_t > 0.1 && clipped_width > 0.0 {
      let hover_strength = anim.hover_t * (1.0 - anim.pressed_t * 0.9);
      if hover_strength > 0.0 {
        Button::draw_hover_layers(
          surface,
          clipped_start_x,
          tab_top,
          clipped_width,
          tab_height,
          3.0,
          button_highlight,
          hover_strength,
        );
      }
    }

    // Draw press glow - use clipped dimensions
    if anim.pressed_t > 0.0 && clipped_width > 0.0 {
      let glow_alpha = 0.12 * anim.pressed_t;
      let bottom_glow = Color::new(
        button_highlight.r,
        button_highlight.g,
        button_highlight.b,
        glow_alpha,
      );
      let bottom_center_y = tab_top + tab_height + 1.5;
      let bottom_radius = (clipped_width * 0.45).max(tab_height * 0.42);
      surface.draw_rounded_rect_glow(
        clipped_start_x,
        tab_top,
        clipped_width,
        tab_height,
        3.0,
        clipped_start_x + clipped_width * 0.5,
        bottom_center_y,
        bottom_radius,
        bottom_glow,
      );
    }

    // Draw file icon (only if visible within clip region)
    let icon_x = cursor_x + text_padding;
    let icon_y = tab_top + (tab_height - icon_size as f32) * 0.5;
    let icon_color = text_color;
    if icon_x >= clip_left && icon_x + icon_size as f32 <= clip_right {
      surface.draw_svg_icon(icon.svg_data, icon_x, icon_y, icon_size, icon_size, icon_color);
    }

    // Draw file name - full text, no truncation
    let name_x = icon_x + icon_size as f32 + icon_padding;

    // Skip text entirely if it starts past clip region
    if name_x >= clip_right {
      cursor_x = tab_end + tab_spacing;
      continue;
    }

    // Full filename with modified indicator - no truncation ever
    let display_text = if doc.is_modified() {
      format!("{} •", name_str)
    } else {
      name_str.to_string()
    };

    surface.draw_text(TextSection::simple(
      name_x,
      text_y,
      &display_text,
      UI_FONT_SIZE,
      text_color,
    ));

    // Draw close button (only when hovering and visible - implicit style)
    let close_x = end_x - close_button_width - text_padding * 0.5;
    let close_visible = close_x >= clip_left && close_x + close_button_width <= clip_right;

    if anim.hover_t > 0.1 && close_visible {
      let close_alpha = anim.hover_t * 0.8;

      // Draw close button background on hover
      if anim.close_hover_t > 0.0 {
        let close_bg = with_alpha(button_base, anim.close_hover_t * 0.3);
        surface.draw_rounded_rect(
          close_x,
          tab_top + 2.0,
          close_button_width,
          tab_height - 4.0,
          2.0,
          close_bg,
        );

        // Draw close button glow
        if anim.close_hover_t > 0.3 {
          Button::draw_hover_layers(
            surface,
            close_x,
            tab_top + 2.0,
            close_button_width,
            tab_height - 4.0,
            2.0,
            button_highlight,
            anim.close_hover_t * 0.5,
          );
        }
      }

      // Draw × character
      let close_color = with_alpha(
        text_color.lerp(Color::WHITE, anim.close_hover_t * 0.3),
        close_alpha,
      );
      let x_size = UI_FONT_SIZE * 0.7;
      let x_x = close_x + (close_button_width - x_size * 0.5) * 0.5;
      let x_y = tab_top + (tab_height - x_size) * 0.5;
      surface.draw_text(TextSection::simple(x_x, x_y, "×", x_size, close_color));
    }

    cursor_x = tab_end + tab_spacing;
  }

  // Pop scissor rect now that tabs are done
  surface.pop_scissor_rect();

  // Clean up animation states for closed documents
  let doc_ids: std::collections::HashSet<_> = documents.iter().map(|d| d.id()).collect();
  animation_states.retain(|id, _| doc_ids.contains(id));

  // Draw add (+) button
  add_button_state.update(dt, add_button_hovered, add_button_pressed);

  let add_btn_size = tab_height;
  let add_btn_x = cursor_x + 4.0;
  let add_btn_y = tab_top;

  let add_button_rect = if add_btn_x + add_btn_size < origin_x + viewport_width {
    // Draw add button background on hover
    if add_button_state.hover_t > 0.0 {
      let add_bg = with_alpha(button_base, add_button_state.hover_t * 0.2);
      surface.draw_rounded_rect(add_btn_x, add_btn_y, add_btn_size, add_btn_size, 3.0, add_bg);

      // Draw hover glow
      if add_button_state.hover_t > 0.3 {
        Button::draw_hover_layers(
          surface,
          add_btn_x,
          add_btn_y,
          add_btn_size,
          add_btn_size,
          3.0,
          button_highlight,
          add_button_state.hover_t * 0.6,
        );
      }
    }

    // Draw press glow
    if add_button_state.pressed_t > 0.0 {
      let glow_alpha = 0.15 * add_button_state.pressed_t;
      let bottom_glow = Color::new(
        button_highlight.r,
        button_highlight.g,
        button_highlight.b,
        glow_alpha,
      );
      surface.draw_rounded_rect_glow(
        add_btn_x,
        add_btn_y,
        add_btn_size,
        add_btn_size,
        3.0,
        add_btn_x + add_btn_size * 0.5,
        add_btn_y + add_btn_size + 1.0,
        add_btn_size * 0.5,
        bottom_glow,
      );
    }

    // Draw + icon
    let plus_size = UI_FONT_SIZE * 0.9;
    let plus_x = add_btn_x + (add_btn_size - plus_size * 0.4) * 0.5;
    let plus_y = add_btn_y + (add_btn_size - plus_size) * 0.5;
    let plus_color = inactive_text.lerp(active_text, add_button_state.hover_t);
    surface.draw_text(TextSection::simple(plus_x, plus_y, "+", plus_size, plus_color));

    Some(Rect {
      x:      add_btn_x as u16,
      y:      add_btn_y as u16,
      width:  add_btn_size as u16,
      height: add_btn_size as u16,
    })
  } else {
    None
  };

  surface.restore_font_state(saved_font);

  RenderResult {
    height: cell_height,
    add_button_rect,
    total_content_width,
    max_scroll,
    active_tab_index,
  }
}

/// Check if any tab animation is in progress (requires redraw)
pub fn needs_animation_update(
  animation_states: &HashMap<DocumentId, TabAnimationState>,
  add_button_state: &AddButtonState,
) -> bool {
  add_button_state.is_animating() || animation_states.values().any(|s| s.is_animating())
}
