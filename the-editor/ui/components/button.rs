use the_editor_renderer::{Color, MouseButton, Renderer, TextSection};

use crate::{
  core::{
    graphics::{CursorKind, Rect},
    position::Position,
  },
  editor::Editor,
  ui::{
    compositor::{Component, Context, Event, EventResult, Surface},
    theme_color_to_renderer_color,
  },
};

/// A simple RAD-style button with outline, hover glow, and click feedback.
pub struct Button {
  // Content
  label: String,

  // Appearance
  base_color: Color, // Outline and text color base; glow derives from this
  color_override: bool,

  // State
  visible: bool,
  hovered: bool,
  pressed: bool,
  hover_cursor_px: Option<(f32, f32)>, // cursor position in pixels relative to button top-left

  // Click state animation (0.0 = not pressed, 1.0 = fully pressed)
  anim_t: f32,

  // Behavior
  on_click: Option<Box<dyn FnMut() + 'static>>,

  // Cached font metrics for mouse handling
  cached_char_width: f32,
  cached_line_height: f32,

  // Position and size in the compositor
  rect: Rect,
  // Last rendered area (for mouse hit testing)
  last_rendered_area: Rect,
}

#[derive(Debug, Clone, Copy)]
struct ButtonPalette {
  text: Color,
  outline: Color,
  fill: Option<Color>,
  accent_text: Color,
  accent_outline: Color,
  accent_fill: Option<Color>,
  hover_glow: Color,
  press_glow: Color,
}

impl Button {
  pub fn new(label: impl Into<String>) -> Self {
    Self {
      label: label.into(),
      base_color: Color::new(0.45, 0.47, 0.50, 1.0), // neutral gray by default
      color_override: false,
      visible: true,
      hovered: false,
      pressed: false,
      hover_cursor_px: None,
      anim_t: 0.0,
      on_click: None,
      cached_char_width: 12.0, // Default fallback values
      cached_line_height: 20.0,
      rect: Rect::new(0, 0, 10, 2), // Default size
      last_rendered_area: Rect::new(0, 0, 10, 2),
    }
  }

  // --- Builder API -------------------------------------------------------

  /// Set the button text (builder-style)
  pub fn text(mut self, label: impl Into<String>) -> Self {
    self.label = label.into();
    self
  }

  /// Set the outline/text base color (builder-style)
  pub fn color(mut self, color: Color) -> Self {
    self.base_color = color;
    self.color_override = true;
    self
  }

  /// Set the on-click callback (builder-style)
  pub fn on_click<F: FnMut() + 'static>(mut self, f: F) -> Self {
    self.on_click = Some(Box::new(f));
    self
  }

  /// Set the button position and size (builder-style)
  pub fn with_rect(mut self, rect: Rect) -> Self {
    self.rect = rect;
    self.last_rendered_area = rect;
    self
  }

  /// Set visibility (builder-style)
  pub fn visible(mut self, visible: bool) -> Self {
    self.visible = visible;
    self
  }

  // --- Runtime setters ---------------------------------------------------

  /// Update the button text at runtime
  pub fn set_text(&mut self, label: impl Into<String>) {
    self.label = label.into();
  }

  /// Update the base color at runtime
  pub fn set_color(&mut self, color: Color) {
    self.base_color = color;
    self.color_override = true;
  }

  /// Update the click callback at runtime
  pub fn set_on_click<F: FnMut() + 'static>(&mut self, f: F) {
    self.on_click = Some(Box::new(f));
  }

  /// Toggle visibility
  pub fn toggle_visible(&mut self) {
    self.visible = !self.visible;
  }

  /// Check if visible
  pub fn is_visible(&self) -> bool {
    self.visible
  }

  fn rect_to_pixels(rect: Rect, renderer: &Renderer) -> (f32, f32, f32, f32) {
    // Use actual font metrics from the renderer
    let char_w = renderer.cell_width();
    let line_h = renderer.cell_height();
    let x = rect.x as f32 * char_w;
    let y = rect.y as f32 * line_h;
    let w = rect.width as f32 * char_w;
    let h = rect.height as f32 * line_h;
    (x, y, w, h)
  }

  fn resolve_palette(&self, cx: &Context) -> ButtonPalette {
    if self.color_override {
      let base = self.base_color;
      let glow = Self::glow_rgb_from_base(base);
      return ButtonPalette {
        text: base,
        outline: base,
        fill: None,
        accent_text: glow,
        accent_outline: glow,
        accent_fill: None,
        hover_glow: glow,
        press_glow: glow,
      };
    }

    let theme = &cx.editor.theme;

    let base_style = theme.try_get_exact("ui.button");
    let base_text = base_style
      .and_then(|style| style.fg)
      .map(theme_color_to_renderer_color)
      .unwrap_or(self.base_color);
    let base_fill = base_style
      .and_then(|style| style.bg)
      .map(theme_color_to_renderer_color);

    let highlight_style = theme.try_get_exact("ui.button.highlight");
    let accent_text = highlight_style
      .and_then(|style| style.fg)
      .map(theme_color_to_renderer_color)
      .unwrap_or_else(|| Self::glow_rgb_from_base(base_text));
    let accent_text = Color::new(
      accent_text.r,
      accent_text.g,
      accent_text.b,
      if accent_text.a == 0.0 {
        1.0
      } else {
        accent_text.a
      },
    );
    let accent_fill = highlight_style
      .and_then(|style| style.bg)
      .map(theme_color_to_renderer_color)
      .or_else(|| {
        base_fill.map(|fill| {
          let lifted = Self::mix(fill, accent_text, 0.35);
          Color::new(lifted.r, lifted.g, lifted.b, 1.0)
        })
      });
    let accent_outline = highlight_style
      .and_then(|style| style.fg)
      .map(theme_color_to_renderer_color)
      .unwrap_or(accent_text);
    let accent_outline = Color::new(
      accent_outline.r,
      accent_outline.g,
      accent_outline.b,
      if accent_outline.a == 0.0 {
        1.0
      } else {
        accent_outline.a
      },
    );

    let selection_style = theme.try_get("ui.selection");
    let selection_glow_style = theme.try_get("ui.selection.glow");
    let selection_glow = selection_glow_style
      .and_then(|style| style.bg.or(style.fg))
      .map(theme_color_to_renderer_color)
      .or_else(|| {
        selection_style
          .and_then(|style| style.bg.or(style.fg))
          .map(theme_color_to_renderer_color)
      })
      .unwrap_or(accent_text);
    let selection_glow = Color::new(
      selection_glow.r,
      selection_glow.g,
      selection_glow.b,
      if selection_glow.a == 0.0 {
        1.0
      } else {
        selection_glow.a
      },
    );

    ButtonPalette {
      text: base_text,
      outline: base_text,
      fill: base_fill,
      accent_text,
      accent_outline,
      accent_fill,
      hover_glow: selection_glow,
      press_glow: selection_glow,
    }
  }

  #[allow(clippy::too_many_arguments)]
  fn draw_outline_button(
    &self,
    renderer: &mut Renderer,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    rounded: f32,
    click_t: f32, // 0.0 = not clicking, 1.0 = fully clicked
    base_outline: Color,
    hover_outline: Color,
    hover_glow_color: Color,
    press_glow_color: Color,
  ) {
    let mut outline = if self.hovered {
      hover_outline
    } else {
      base_outline
    };
    outline.a = 0.95;

    if self.hovered {
      // Directional border thickness only when hovered: top > sides > bottom
      let bottom_thickness = (h * 0.035).clamp(0.6, 1.4);
      let side_thickness = (bottom_thickness * 1.55).min(bottom_thickness + 1.8);
      let top_thickness = (bottom_thickness * 2.3).min(bottom_thickness + 2.6);

      renderer.draw_rounded_rect_stroke_fade(
        x,
        y,
        w,
        h,
        rounded,
        top_thickness,
        side_thickness,
        bottom_thickness,
        outline,
      );
    } else {
      // Default idle border remains uniform
      renderer.draw_rounded_rect_stroke(x, y, w, h, rounded, 1.0, outline);
    }

    // Bottom glow (only appears on click)
    if click_t > 0.0 {
      let bottom_center_y = y + h + 1.5; // slightly below bottom edge
      let bottom_glow_strength = click_t * 0.12; // only on click, reduced intensity
      let bottom_glow = Color::new(
        press_glow_color.r,
        press_glow_color.g,
        press_glow_color.b,
        bottom_glow_strength,
      );
      let bottom_radius = (w * 0.45).max(h * 0.42);
      renderer.draw_rounded_rect_glow(
        x,
        y,
        w,
        h,
        rounded,
        x + w * 0.5,
        bottom_center_y,
        bottom_radius,
        bottom_glow,
      );
    }

    if self.hovered {
      let hover_strength = (1.0 - click_t * 0.9).max(0.0); // almost fully disappear on click
      Self::draw_hover_layers(
        renderer,
        x,
        y,
        w,
        h,
        rounded,
        hover_glow_color,
        hover_strength,
      );
    }
  }

  #[allow(clippy::too_many_arguments)]
  pub(crate) fn draw_hover_layers(
    renderer: &mut Renderer,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    rounded: f32,
    highlight_color: Color,
    strength: f32,
  ) {
    if strength <= 0.0 {
      return;
    }

    let max_depth = (h * 0.33).max(1.0);
    let top_center_x = x + w * 0.5;
    let width_base = (w * 0.5).max(max_depth);
    let layers = [
      (-0.12, 0.9, 0.16),
      (0.12, 1.12, 0.11),
      (0.24, 1.3, 0.07),
      (0.33, 1.48, 0.035),
    ];

    for (depth_ratio, radius_scale, alpha_scale) in layers {
      let center_y = y + max_depth * depth_ratio;
      let radius = (width_base * radius_scale).max(max_depth * (0.75 + depth_ratio.abs() * 0.4));
      renderer.draw_rounded_rect_glow(
        x,
        y,
        w,
        h,
        rounded,
        top_center_x,
        center_y,
        radius,
        Color::new(
          highlight_color.r,
          highlight_color.g,
          highlight_color.b,
          alpha_scale * strength,
        ),
      );
    }
  }

  #[allow(clippy::too_many_arguments)]
  fn draw_hover_glow(
    &self,
    renderer: &mut Renderer,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    click_t: f32,
    highlight_color: Color,
  ) {
    if !self.hovered {
      return;
    }

    let Some((cx, cy)) = self.hover_cursor_px else {
      return;
    };

    let center_x = x + cx;
    let center_y = y + cy;

    // Subtle mouse-follow glow, fades during click
    let glow_radius = (w.max(h)) * 1.2;
    let glow_strength = 0.042 * (1.0 - click_t * 0.7);
    let glow = Color::new(
      highlight_color.r,
      highlight_color.g,
      highlight_color.b,
      glow_strength,
    );
    renderer.draw_rounded_rect_glow(
      x,
      y,
      w,
      h,
      (h * 0.5).min(10.0),
      center_x,
      center_y,
      glow_radius,
      glow,
    );
  }

  fn update_click_state(&mut self, dt: f32) -> f32 {
    // Target state: 1.0 when pressed, 0.0 when not pressed
    let target = if self.pressed { 1.0 } else { 0.0 };

    // Animate toward target
    let anim_speed = 12.0; // Quick response
    if (self.anim_t - target).abs() < 0.01 {
      self.anim_t = target;
    } else if self.anim_t < target {
      self.anim_t = (self.anim_t + dt * anim_speed).min(target);
    } else {
      self.anim_t = (self.anim_t - dt * anim_speed).max(target);
    }

    // Use ease-in-out for smooth transition
    let t = self.anim_t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t) // smoothstep
  }

  // --- Color helpers -----------------------------------------------------

  fn luminance(c: Color) -> f32 {
    // Relative luminance approximation in sRGB space
    0.2126 * c.r + 0.7152 * c.g + 0.0722 * c.b
  }

  fn mix(a: Color, b: Color, t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    Color::new(
      a.r * (1.0 - t) + b.r * t,
      a.g * (1.0 - t) + b.g * t,
      a.b * (1.0 - t) + b.b * t,
      1.0,
    )
  }

  /// Derive a glow RGB color from the base color by increasing contrast toward
  /// white
  fn glow_rgb_from_base(base: Color) -> Color {
    let lum = Self::luminance(base);
    // Darker colors get more lift; lighter colors get a subtle lift.
    let t = if lum < 0.35 {
      0.75
    } else if lum < 0.65 {
      0.55
    } else {
      0.35
    };
    Self::mix(base, Color::WHITE, t)
  }
}

impl Default for Button {
  fn default() -> Self {
    Self::new("Button")
  }
}

impl Component for Button {
  fn handle_event(&mut self, event: &Event, _cx: &mut Context) -> EventResult {
    match event {
      Event::Mouse(mouse) => {
        // Use the last rendered area for hit testing
        let rect = self.last_rendered_area;
        // Convert rect to pixel-space box using cached font metrics
        let x = rect.x as f32 * self.cached_char_width;
        let y = rect.y as f32 * self.cached_line_height;
        let w = rect.width as f32 * self.cached_char_width;
        let h = rect.height as f32 * self.cached_line_height;
        let (mx, my) = mouse.position;
        let inside = mx >= x && mx <= x + w && my >= y && my <= y + h;

        // Track hover + cursor pos relative to button
        let prev_hovered = self.hovered;
        if inside {
          self.hovered = true;
          self.hover_cursor_px = Some((mx - x, my - y));
        } else {
          self.hovered = false;
          self.hover_cursor_px = None;
        }

        // Handle press/release only on left button
        if let Some(MouseButton::Left) = mouse.button {
          if inside && mouse.pressed {
            self.pressed = true;
            return EventResult::Consumed(None);
          } else if self.pressed && !mouse.pressed {
            // Release
            self.pressed = false;
            // Fire callback only if release occurs inside the button
            if inside && let Some(cb) = self.on_click.as_mut() {
              (cb)();
            }
            // Return empty callback for now - animation will progress on any event
            // The issue is that we need events to trigger redraws
            return EventResult::Consumed(Some(Box::new(|_compositor, _cx| {
              // Animation will progress whenever any event causes a redraw
              // This includes mouse movements, key presses, etc.
            })));
          }
        }

        // Request redraw when leaving/entering hover, or for hover motion
        if inside || (prev_hovered != self.hovered) {
          EventResult::Consumed(None)
        } else {
          EventResult::Ignored(None)
        }
      },
      _ => EventResult::Ignored(None),
    }
  }

  fn render(&mut self, _area: Rect, renderer: &mut Surface, cx: &mut Context) {
    if !self.visible {
      return;
    }

    // Always use our internal rect for buttons
    // Buttons are positioned absolutely, not relative to the provided area
    let rect = self.rect;

    // Store the rendered area for mouse hit testing
    self.last_rendered_area = rect;

    // Cache font metrics for mouse handling
    self.cached_char_width = renderer.cell_width();
    self.cached_line_height = renderer.cell_height();

    let (x, y, w, h) = Self::rect_to_pixels(rect, renderer);
    let radius = (h * 0.5).min(10.0);

    // Update click state and get current progress
    let click_t = self.update_click_state(cx.dt);

    let palette = self.resolve_palette(cx);
    let state_fill = if self.pressed {
      palette.accent_fill.or(palette.fill).map(|fill| {
        if let Some(base) = palette.fill {
          let mixed = Self::mix(fill, base, 0.4);
          Color::new(
            mixed.r,
            mixed.g,
            mixed.b,
            if fill.a == 0.0 {
              base.a.max(0.4)
            } else {
              fill.a
            },
          )
        } else {
          fill
        }
      })
    } else if self.hovered {
      palette.accent_fill.or(palette.fill).map(|fill| {
        Color::new(
          fill.r,
          fill.g,
          fill.b,
          if fill.a == 0.0 { 0.85 } else { fill.a },
        )
      })
    } else {
      palette.fill
    };

    if let Some(mut fill) = state_fill {
      if fill.a == 0.0 {
        fill.a = 1.0;
      }
      renderer.draw_rounded_rect(x, y, w, h, radius, fill);
    }

    // Base + outline (with click inversion effect)
    self.draw_outline_button(
      renderer,
      x,
      y,
      w,
      h,
      radius,
      click_t,
      palette.outline,
      palette.accent_outline,
      palette.hover_glow,
      palette.press_glow,
    );

    // Hover glow following cursor (weakens during click)
    self.draw_hover_glow(renderer, x, y, w, h, click_t, palette.hover_glow);

    // Label centered
    let text_color = if self.pressed {
      Self::mix(palette.accent_text, palette.text, 0.45)
    } else if self.hovered {
      palette.accent_text
    } else {
      palette.text
    };
    let font_size = (h * 0.5).clamp(12.0, 20.0);
    // Position is top-left; center the text inside the button.
    let text = self.label.clone();
    // Estimate text width: ~0.6 * font_size per character as a rough width
    let est_char_w = font_size * 0.6;
    let text_w = est_char_w * (text.chars().count() as f32);
    let tx = x + (w - text_w) * 0.5;
    let ty = y + (h - font_size) * 0.5; // top-left positioning
    renderer.draw_text(TextSection::simple(tx, ty, text, font_size, text_color));
  }

  fn cursor(&self, _area: Rect, _editor: &Editor) -> (Option<Position>, CursorKind) {
    (None, CursorKind::Hidden)
  }

  fn required_size(&mut self, _viewport: (u16, u16)) -> Option<(u16, u16)> {
    // Return the button's preferred size (10x2 by default)
    Some((10, 2))
  }

  fn should_update(&self) -> bool {
    // Update when animating click state transition
    let target = if self.pressed { 1.0 } else { 0.0 };
    (self.anim_t - target).abs() > 0.01
  }
}
