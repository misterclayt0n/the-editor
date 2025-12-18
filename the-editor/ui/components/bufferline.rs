use std::collections::HashMap;

use the_editor_renderer::{
  Color,
  TextSection,
};
use the_terminal::TerminalId;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BufferKind {
  Document(DocumentId),
  Terminal(TerminalId),
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
#[derive(Debug, Clone, Copy)]
pub struct TabAnimationState {
  /// Hover state (0.0 = not hovered, 1.0 = fully hovered)
  pub hover_t:         f32,
  /// Close button hover state
  pub close_hover_t:   f32,
  /// Press/click state for the tab
  pub pressed_t:       f32,
  /// Press/click state for the close button
  pub close_pressed_t: f32,
  /// Active/selected tab state
  pub active_t:        f32,
  /// Alive state for close animation (1.0 = visible, 0.0 = closed)
  pub alive_t:         f32,
  /// Flag indicating this tab is closing (triggers animation even at alive_t=1.0)
  pub is_closing:      bool,
  /// Cached tab width for close animation (so we know width during fade-out)
  pub cached_width:    f32,
  /// Cached tab position (start_x) for close animation
  pub cached_start_x:  f32,
  /// Tab index when last alive (for ordering closing tabs)
  pub cached_index:    usize,
}

impl Default for TabAnimationState {
  fn default() -> Self {
    Self {
      hover_t:         0.0,
      close_hover_t:   0.0,
      pressed_t:       0.0,
      close_pressed_t: 0.0,
      active_t:        0.0,
      alive_t:         0.0, // Start at 0 so new tabs animate in
      is_closing:      false,
      cached_width:    0.0,
      cached_start_x:  0.0,
      cached_index:    0,
    }
  }
}

impl TabAnimationState {
  /// Update animation state using exponential decay
  pub fn update(
    &mut self,
    dt: f32,
    is_hovered: bool,
    is_close_hovered: bool,
    is_pressed: bool,
    is_close_pressed: bool,
    is_active: bool,
    is_alive: bool, // false when tab is closing
  ) {
    // Exponential decay formula: rate = 1 - 2^(-60 * dt)
    let rate = 1.0 - 2.0_f32.powf(-60.0 * dt);
    // Slower rate for alive animation (more deliberate close)
    let alive_rate = 1.0 - 2.0_f32.powf(-30.0 * dt);

    let hover_target = if is_hovered { 1.0 } else { 0.0 };
    let close_hover_target = if is_close_hovered { 1.0 } else { 0.0 };
    let press_target = if is_pressed { 1.0 } else { 0.0 };
    let close_press_target = if is_close_pressed { 1.0 } else { 0.0 };
    let active_target = if is_active { 1.0 } else { 0.0 };
    let alive_target = if is_alive { 1.0 } else { 0.0 };

    self.hover_t += (hover_target - self.hover_t) * rate;
    self.close_hover_t += (close_hover_target - self.close_hover_t) * rate;
    self.pressed_t += (press_target - self.pressed_t) * rate;
    self.close_pressed_t += (close_press_target - self.close_pressed_t) * rate;
    self.active_t += (active_target - self.active_t) * rate;
    self.alive_t += (alive_target - self.alive_t) * alive_rate;

    // Snap to target when close enough
    if (self.hover_t - hover_target).abs() < 0.01 {
      self.hover_t = hover_target;
    }
    if (self.close_hover_t - close_hover_target).abs() < 0.01 {
      self.close_hover_t = close_hover_target;
    }
    if (self.pressed_t - press_target).abs() < 0.01 {
      self.pressed_t = press_target;
    }
    if (self.close_pressed_t - close_press_target).abs() < 0.01 {
      self.close_pressed_t = close_press_target;
    }
    if (self.active_t - active_target).abs() < 0.01 {
      self.active_t = active_target;
    }
    if (self.alive_t - alive_target).abs() < 0.01 {
      self.alive_t = alive_target;
    }
  }

  /// Returns true if any animation is in progress
  pub fn is_animating(&self) -> bool {
    self.hover_t > 0.01
      || self.close_hover_t > 0.01
      || self.pressed_t > 0.01
      || self.close_pressed_t > 0.01
      || (self.active_t > 0.01 && self.active_t < 0.99)
      // alive_t animation: opening (0->1) or closing (1->0)
      || (self.alive_t > 0.005 && self.alive_t < 0.995)
      // is_closing flag ensures we animate even when alive_t is still at 1.0
      || self.is_closing
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

fn format_terminal_title(working_dir: Option<&std::path::Path>) -> String {
  match working_dir {
    Some(path) => {
      if let Ok(home) = std::env::var("HOME") {
        if let Ok(stripped) = path.strip_prefix(&home) {
          return format!("~/{}", stripped.display());
        }
      }
      path.display().to_string()
    },
    None => "terminal".to_string(),
  }
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
  close_pressed_index: Option<usize>,
  add_button_hovered: bool,
  add_button_pressed: bool,
  tabs: &mut Vec<BufferTab>,
  animation_states: &mut HashMap<BufferKind, TabAnimationState>,
  add_button_state: &mut AddButtonState,
  scroll_offset: f32,
  mouse_pos: Option<(f32, f32)>,
  dt: f32,
  alive_t: f32, // Height animation for show/hide (0.0 = hidden/collapsed, 1.0 = fully visible)
) -> RenderResult {
  tabs.clear();

  // Helper to apply alpha
  let with_alpha = |color: Color, alpha: f32| -> Color {
    Color::new(color.r, color.g, color.b, alpha.clamp(0.0, 1.0))
  };

  let saved_font = surface.save_font_state();
  let ui_font_family = surface.current_font_family().to_owned();
  surface.configure_font(&ui_font_family, UI_FONT_SIZE);

  let base_cell_height = surface.cell_height().max(UI_FONT_SIZE + 4.0);
  let full_cell_height = (base_cell_height + 10.0).max(UI_FONT_SIZE + 16.0);
  // For slide animation: visible_height is how much space the bufferline takes in layout
  let visible_height = full_cell_height * alive_t;
  let tab_height = (full_cell_height - 6.0).max(UI_FONT_SIZE + 8.0);
  // Content slides up as alive_t decreases: alive_t=1 -> normal, alive_t=0 -> fully slid up
  let y_slide_offset = -full_cell_height * (1.0 - alive_t);
  let tab_top = origin_y + y_slide_offset + (full_cell_height - tab_height) * 0.5;
  let text_y = tab_top + (tab_height - UI_FONT_SIZE) * 0.5;
  // cell_height is the visible space; we clip to this but render full-size content
  let cell_height = visible_height;

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

  // Push scissor rect to clip bufferline content during slide animation
  surface.push_scissor_rect(origin_x, origin_y, viewport_width, visible_height.max(0.0));

  // Draw background (full height for slide effect)
  surface.draw_rect(origin_x, origin_y + y_slide_offset, viewport_width, full_cell_height, base_bg);

  // Draw subtle separators
  let separator_color = base_bg.lerp(Color::WHITE, 0.05);
  surface.draw_rect(origin_x, origin_y + y_slide_offset, viewport_width, 1.0, separator_color);
  surface.draw_rect(
    origin_x,
    origin_y + y_slide_offset + full_cell_height - 1.0,
    viewport_width,
    1.0,
    separator_color,
  );

  // Get focused view content (document or terminal)
  let focused_content = editor
    .focused_view_id()
    .and_then(|view_id| editor.tree.try_get(view_id).map(|view| view.content));

  // Layout constants
  let icon_size = (UI_FONT_SIZE * 1.0) as u32;
  let icon_padding = 6.0;
  let text_padding = 8.0;
  let text_close_gap = 8.0; // Gap between text and close button
  let close_btn_width = UI_FONT_SIZE * 2.0; // Full-height close button width
  let tab_spacing = 2.0;
  let min_tab_width = text_padding * 2.0 + icon_size as f32 + 20.0;

  let left_margin = 4.0;
  let add_button_reserved = tab_height + 12.0;
  let available_width = viewport_width - left_margin - add_button_reserved;

  // Collect tab items: documents first, then visible terminals
  struct TabItem {
    kind:        BufferKind,
    title:       String,
    icon:        file_icons::FileIcon,
    is_modified: bool,
  }

  let mut tab_items: Vec<TabItem> = Vec::new();

  // Add document tabs
  for doc in editor.documents() {
    let title = display_name(doc).to_string();
    let icon = file_icons::icon_for_file(&title);
    tab_items.push(TabItem {
      kind: BufferKind::Document(doc.id()),
      title,
      icon,
      is_modified: doc.is_modified(),
    });
  }

  // Add visible terminal tabs
  for terminal in editor.visible_terminals() {
    let info = terminal.picker_info();
    let title = format_terminal_title(info.working_directory.as_deref());
    tab_items.push(TabItem {
      kind: BufferKind::Terminal(info.id),
      title,
      icon: file_icons::terminal_icon(),
      is_modified: false,
    });
  }

  // Collect all tab kinds for closing animation check
  let active_kinds: std::collections::HashSet<_> = tab_items.iter().map(|t| t.kind).collect();

  // Mark closing tabs and update their animation state
  for (kind, anim) in animation_states.iter_mut() {
    if !active_kinds.contains(kind) {
      anim.is_closing = true;
      anim.update(dt, false, false, false, false, false, false);
    }
  }

  // Calculate natural width for each tab - sized exactly to content, no maximum
  let tab_widths: Vec<f32> = tab_items
    .iter()
    .map(|item| {
      let display_text = if item.is_modified {
        format!("{} •", item.title)
      } else {
        item.title.clone()
      };
      let text_width = surface.measure_text(&display_text, UI_FONT_SIZE);
      (text_padding + icon_size as f32 + icon_padding + text_width + text_close_gap + close_btn_width)
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

  // Find active tab index based on focused content
  let active_tab_index = tab_items.iter().position(|item| {
    match (item.kind, focused_content) {
      (BufferKind::Document(doc_id), Some(crate::core::view::ViewContent::Document(focused_id))) => {
        doc_id == focused_id
      },
      (BufferKind::Terminal(term_id), Some(crate::core::view::ViewContent::Terminal(focused_id))) => {
        term_id == focused_id
      },
      _ => false,
    }
  });

  // Start cursor at left margin minus scroll offset
  let mut cursor_x = origin_x + left_margin - scroll_offset;
  let clip_left = origin_x + left_margin;
  let clip_right = origin_x + viewport_width - add_button_reserved;

  // Push scissor rect to clip tab content to the visible area
  surface.push_scissor_rect(clip_left, origin_y, clip_right - clip_left, cell_height);

  for (tab_index, item) in tab_items.iter().enumerate() {
    let is_active = Some(tab_index) == active_tab_index;
    let is_hovered = Some(tab_index) == hover_index;
    let is_pressed = Some(tab_index) == pressed_index;
    let is_close_hovered = Some(tab_index) == close_hover_index;
    let is_close_pressed = Some(tab_index) == close_pressed_index;

    // Get or create animation state for this tab
    let anim = animation_states.entry(item.kind).or_default();
    anim.update(dt, is_hovered, is_close_hovered, is_pressed, is_close_pressed, is_active, true);

    // Apply alive_t to width for open/close animation
    let base_width = tab_widths[tab_index];
    let draw_width = base_width * anim.alive_t;
    let tab_start = cursor_x;
    let tab_end = cursor_x + draw_width;

    // Cache position info for close animation (use base width, not animated)
    anim.cached_width = base_width;
    anim.cached_start_x = tab_start;
    anim.cached_index = tab_index;

    // Store tab bounds for hit testing (use animated width)
    tabs.push(BufferTab {
      kind: item.kind,
      start_x: tab_start,
      end_x: tab_end,
      close_start_x: tab_end - close_btn_width * anim.alive_t,
      close_end_x: tab_end,
    });

    // Skip rendering if tab is too small or completely outside visible area
    let is_visible = draw_width > 1.0 && tab_end > clip_left && tab_start < clip_right;
    if !is_visible {
      cursor_x = tab_end + tab_spacing;
      continue;
    }

    // Calculate clipped dimensions for rectangles (text is clipped by scissor rect)
    let clipped_start_x = cursor_x.max(clip_left);
    let clipped_end_x = tab_end.min(clip_right);
    let clipped_width = (clipped_end_x - clipped_start_x).max(0.0);

    // Get tab title and icon from pre-computed item
    let name_str = &item.title;
    let icon = item.icon;

    // Determine colors based on state - using active_t for smooth transitions
    // Fade content based on alive_t for open/close animation
    let content_opacity = anim.alive_t;
    let blend = anim.active_t.max(anim.hover_t);
    let text_color_base = inactive_text.lerp(active_text.lerp(Color::WHITE, 0.1), blend);
    let text_color = with_alpha(text_color_base, text_color_base.a * content_opacity);
    let bg_alpha = (0.15 * anim.active_t + anim.hover_t * 0.12 * (1.0 - anim.active_t)) * content_opacity;

    // Draw tab background (subtle fill on hover/active) - use clipped dimensions
    if bg_alpha > 0.0 && clipped_width > 0.0 {
      let bg_color = with_alpha(active_accent, bg_alpha);
      surface.draw_rounded_rect(clipped_start_x, tab_top, clipped_width, tab_height, 3.0, bg_color);
    }

    // Draw border/outline with directional thickness - use clipped dimensions
    // Using active_t for smooth transitions, faded by alive_t
    if (anim.hover_t > 0.1 || anim.active_t > 0.1) && clipped_width > 0.0 && content_opacity > 0.1 {
      let border_strength =
        (0.6 * anim.active_t + anim.hover_t * 0.8 * (1.0 - anim.active_t * 0.5)) * content_opacity;
      let outline_color = with_alpha(
        button_base.lerp(active_border, anim.active_t),
        border_strength,
      );

      let bottom_thickness = (tab_height * 0.035).clamp(0.6, 1.4);
      let side_thickness = (bottom_thickness * 1.55).min(bottom_thickness + 1.8);
      let top_thickness = (bottom_thickness * 2.3).min(bottom_thickness + 2.6);

      let thickness_blend = anim.hover_t.max(anim.active_t);
      surface.draw_rounded_rect_stroke_fade(
        clipped_start_x,
        tab_top,
        clipped_width,
        tab_height,
        3.0,
        top_thickness * thickness_blend.max(anim.active_t * 0.5),
        side_thickness * thickness_blend.max(anim.active_t * 0.3),
        bottom_thickness * thickness_blend.max(anim.active_t * 0.2),
        outline_color,
      );
    }

    // Close button dimensions (full height, positioned at right edge)
    let close_x = tab_end - close_btn_width;
    let close_y = tab_top;
    let close_height = tab_height;

    // Draw mouse-following glow (raddebugger style)
    // When close button is hovered, only glow the close button area
    // When tab is hovered (but not close), glow the tab area excluding close button
    if anim.hover_t > 0.01 && clipped_width > 0.0 && content_opacity > 0.1 {
      if let Some((mouse_x, mouse_y)) = mouse_pos {
        let glow_alpha = 0.06 * anim.hover_t * (1.0 - anim.pressed_t * 0.5) * content_opacity;
        let glow_color = Color::new(
          button_highlight.r,
          button_highlight.g,
          button_highlight.b,
          glow_alpha,
        );

        if anim.close_hover_t > 0.5 {
          // Hovering close button - glow only the close button area
          let close_clipped_x = close_x.max(clip_left);
          let close_clipped_end = (close_x + close_btn_width).min(clip_right);
          let close_clipped_width = (close_clipped_end - close_clipped_x).max(0.0);
          if close_clipped_width > 0.0 {
            let glow_radius = tab_height * 1.5;
            surface.draw_rounded_rect_glow(
              close_clipped_x,
              tab_top,
              close_clipped_width,
              tab_height,
              3.0,
              mouse_x,
              mouse_y,
              glow_radius,
              glow_color,
            );
          }
        } else {
          // Hovering tab (not close button) - glow the tab area
          let glow_radius = (tab_height * 2.5).min(clipped_width * 0.6);
          surface.draw_rounded_rect_glow(
            clipped_start_x,
            tab_top,
            clipped_width,
            tab_height,
            3.0,
            mouse_x,
            mouse_y,
            glow_radius,
            glow_color,
          );
        }
      }
    }

    // Draw press glow - use clipped dimensions
    if anim.pressed_t > 0.0 && clipped_width > 0.0 && content_opacity > 0.1 {
      let glow_alpha = 0.15 * anim.pressed_t * content_opacity;
      let press_glow = Color::new(
        button_highlight.r,
        button_highlight.g,
        button_highlight.b,
        glow_alpha,
      );
      // Glow centered on mouse or tab center
      let (glow_x, glow_y) = mouse_pos.unwrap_or((
        clipped_start_x + clipped_width * 0.5,
        tab_top + tab_height * 0.5,
      ));
      let glow_radius = (tab_height * 2.0).min(clipped_width * 0.5);
      surface.draw_rounded_rect_glow(
        clipped_start_x,
        tab_top,
        clipped_width,
        tab_height,
        3.0,
        glow_x,
        glow_y,
        glow_radius,
        press_glow,
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

    // Full title with modified indicator - no truncation ever
    let display_text = if item.is_modified {
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

    // Draw close button - visible when tab is large enough
    let close_visible =
      content_opacity > 0.3 && close_x >= clip_left && close_x + close_btn_width <= clip_right;

    if close_visible {
      // Draw close button background on hover (full area)
      if anim.close_hover_t > 0.0 || anim.close_pressed_t > 0.0 {
        let hover_alpha = anim.close_hover_t * 0.2 * content_opacity;
        let press_alpha = anim.close_pressed_t * 0.1 * content_opacity;
        let close_bg = with_alpha(button_base, hover_alpha + press_alpha);
        surface.draw_rect(close_x, close_y, close_btn_width, close_height, close_bg);
      }

      // Raddebugger-style click animation: top dark shadow + bottom light highlight
      if anim.close_pressed_t > 0.01 {
        let shadow_height = (close_height * 0.35 * anim.close_pressed_t).min(close_height * 0.4);

        // Top dark shadow (pressed-in effect)
        let shadow_alpha = 0.2 * anim.close_pressed_t * content_opacity;
        surface.draw_rect(close_x, close_y, close_btn_width, shadow_height, with_alpha(Color::BLACK, shadow_alpha));

        // Bottom light highlight (reflection)
        let light_alpha = 0.08 * anim.close_pressed_t * content_opacity;
        surface.draw_rect(
          close_x,
          close_y + close_height - shadow_height,
          close_btn_width,
          shadow_height,
          with_alpha(Color::WHITE, light_alpha),
        );
      }

      // Draw × character - fades with content
      let base_alpha = 0.4 * content_opacity;
      let hover_boost = anim.close_hover_t * 0.6 * content_opacity;
      let x_color = with_alpha(
        inactive_text.lerp(active_text, anim.close_hover_t * 0.5),
        base_alpha + hover_boost,
      );
      let x_font_size = UI_FONT_SIZE * 1.2;
      let x_text_width = surface.measure_text("×", x_font_size);
      // Center the × in the full button area
      let x_x = close_x + (close_btn_width - x_text_width) * 0.5;
      let x_y = close_y + (close_height - x_font_size) * 0.5;
      surface.draw_text(TextSection::simple(x_x, x_y, "×", x_font_size, x_color));
    }

    cursor_x = tab_end + tab_spacing;
  }

  // Pop scissor rect now that tabs are done
  surface.pop_scissor_rect();

  // Push scissor rect for closing tabs overlay
  surface.push_scissor_rect(clip_left, origin_y, clip_right - clip_left, cell_height);

  // Render closing tabs (already updated earlier in the function)
  for (_id, anim) in animation_states.iter() {
    if anim.is_closing && anim.alive_t > 0.01 {
      // Render the closing tab with shrinking width at its last known position
      let animated_width = anim.cached_width * anim.alive_t;
      if animated_width > 0.5 {
        let tab_start = anim.cached_start_x;
        let tab_end = tab_start + animated_width;

        // Skip if outside visible area
        if tab_end > clip_left && tab_start < clip_right {
          let clipped_start_x = tab_start.max(clip_left);
          let clipped_end_x = tab_end.min(clip_right);
          let clipped_width = (clipped_end_x - clipped_start_x).max(0.0);

          // Draw shrinking tab background matching live tabs
          if clipped_width > 0.0 {
            let content_opacity = anim.alive_t;
            // Use a visible background color that matches the active tab style
            let bg_alpha = (0.2 + 0.1 * content_opacity).min(0.3);
            let bg_color = with_alpha(active_accent, bg_alpha);
            surface.draw_rounded_rect(clipped_start_x, tab_top, clipped_width, tab_height, 3.0, bg_color);

            // Draw border for visibility (same style as active tabs)
            let border_alpha = 0.6 * content_opacity;
            let border_color = with_alpha(active_border, border_alpha);
            surface.draw_rounded_rect_stroke(
              clipped_start_x,
              tab_top,
              clipped_width,
              tab_height,
              3.0,
              1.5,
              border_color,
            );
          }
        }
      }
    }
  }

  surface.pop_scissor_rect();

  // Remove animation states only when animation is complete (alive_t near 0)
  animation_states.retain(|kind, anim| {
    active_kinds.contains(kind) || anim.alive_t > 0.01
  });

  // Draw add (+) button
  add_button_state.update(dt, add_button_hovered, add_button_pressed);

  let add_btn_size = tab_height;
  let add_btn_x = cursor_x + 4.0;
  let add_btn_y = tab_top;

  let add_button_rect = if add_btn_x + add_btn_size < origin_x + viewport_width {
    // Draw add button background on hover (full area)
    if add_button_state.hover_t > 0.0 || add_button_state.pressed_t > 0.0 {
      let hover_alpha = add_button_state.hover_t * 0.2;
      let press_alpha = add_button_state.pressed_t * 0.1;
      let add_bg = with_alpha(button_base, hover_alpha + press_alpha);
      surface.draw_rect(add_btn_x, add_btn_y, add_btn_size, add_btn_size, add_bg);
    }

    // Mouse-following glow on hover (same as X button)
    if add_button_state.hover_t > 0.01 {
      if let Some((mouse_x, mouse_y)) = mouse_pos {
        let glow_alpha = 0.06 * add_button_state.hover_t * (1.0 - add_button_state.pressed_t * 0.5);
        let glow_color = Color::new(button_highlight.r, button_highlight.g, button_highlight.b, glow_alpha);
        let glow_radius = add_btn_size * 1.5;
        surface.draw_rounded_rect_glow(
          add_btn_x,
          add_btn_y,
          add_btn_size,
          add_btn_size,
          3.0,
          mouse_x,
          mouse_y,
          glow_radius,
          glow_color,
        );
      }
    }

    // Raddebugger-style click animation: top dark shadow + bottom light highlight
    if add_button_state.pressed_t > 0.01 {
      let shadow_height = (add_btn_size * 0.35 * add_button_state.pressed_t).min(add_btn_size * 0.4);

      // Top dark shadow (pressed-in effect)
      let shadow_alpha = 0.2 * add_button_state.pressed_t;
      surface.draw_rect(add_btn_x, add_btn_y, add_btn_size, shadow_height, with_alpha(Color::BLACK, shadow_alpha));

      // Bottom light highlight (reflection)
      let light_alpha = 0.08 * add_button_state.pressed_t;
      surface.draw_rect(
        add_btn_x,
        add_btn_y + add_btn_size - shadow_height,
        add_btn_size,
        shadow_height,
        with_alpha(Color::WHITE, light_alpha),
      );
    }

    // Draw + icon
    let base_alpha = 0.4;
    let hover_boost = add_button_state.hover_t * 0.6;
    let plus_size = UI_FONT_SIZE * 1.2;
    let plus_text_width = surface.measure_text("+", plus_size);
    // Center the + in the button area
    let plus_x = add_btn_x + (add_btn_size - plus_text_width) * 0.5;
    let plus_y = add_btn_y + (add_btn_size - plus_size) * 0.5;
    let plus_color = with_alpha(
      inactive_text.lerp(active_text, add_button_state.hover_t * 0.5),
      base_alpha + hover_boost,
    );
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

  // Pop the main bufferline scissor rect (for slide animation clipping)
  surface.pop_scissor_rect();

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
  animation_states: &HashMap<BufferKind, TabAnimationState>,
  add_button_state: &AddButtonState,
) -> bool {
  add_button_state.is_animating() || animation_states.values().any(|s| s.is_animating())
}
