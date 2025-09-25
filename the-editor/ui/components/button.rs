use the_editor_renderer::{
  Color,
  MouseButton,
  MouseEvent,
  Renderer,
  TextSection,
};

use crate::{
  core::graphics::Rect,
  ui::Component,
};

/// A simple RAD-style button with outline, hover glow, and click feedback.
pub struct Button {
  // Content
  label: String,

  // Appearance
  base_color: Color, // Outline and text color base; glow derives from this

  // State
  visible:         bool,
  hovered:         bool,
  pressed:         bool,
  hover_cursor_px: Option<(f32, f32)>, // cursor position in pixels relative to button top-left

  // Click glow animation state
  anim_active: bool,
  anim_t:      f32, // 0.0 -> 1.0

  // Behavior
  on_click: Option<Box<dyn FnMut() + 'static>>,

  // Cached font metrics for mouse handling
  cached_char_width: f32,
  cached_line_height: f32,
}

impl Button {
  pub fn new(label: impl Into<String>) -> Self {
    Self {
      label:           label.into(),
      base_color:      Color::new(0.45, 0.47, 0.50, 1.0), // neutral gray by default
      visible:         true,
      hovered:         false,
      pressed:         false,
      hover_cursor_px: None,
      anim_active:     false,
      anim_t:          0.0,
      on_click:        None,
      cached_char_width: 12.0, // Default fallback values
      cached_line_height: 20.0,
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
    self
  }

  /// Set the on-click callback (builder-style)
  pub fn on_click<F: FnMut() + 'static>(mut self, f: F) -> Self {
    self.on_click = Some(Box::new(f));
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
  }

  /// Update the click callback at runtime
  pub fn set_on_click<F: FnMut() + 'static>(&mut self, f: F) {
    self.on_click = Some(Box::new(f));
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

  fn draw_outline_button(
    &self,
    renderer: &mut Renderer,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    rounded: f32,
  ) {
    // Colors: transparent fill, outline derived from base color
    let mut outline = self.base_color;
    outline.a = 0.95;
    let border_thickness = 1.0; // thinner outline
    renderer.draw_rounded_rect_stroke(x, y, w, h, rounded, border_thickness, outline);
  }

  fn draw_hover_glow(&self, renderer: &mut Renderer, x: f32, y: f32, w: f32, h: f32) {
    if !self.hovered {
      return;
    }

    let Some((cx, cy)) = self.hover_cursor_px else {
      return;
    };

    let center_x = x + cx;
    let center_y = y + cy;

    // Smooth internal glow, clipped in shader and with border accent.
    let glow_radius = (w.max(h)) * 1.2; // A bit of larger area.
    let glow_rgb = Self::glow_rgb_from_base(self.base_color);
    let glow = Color::new(glow_rgb.r, glow_rgb.g, glow_rgb.b, 0.12);
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

  fn draw_click_flash(&mut self, renderer: &mut Renderer, x: f32, y: f32, w: f32, h: f32) {
    if !self.anim_active {
      return;
    }

    let cx = x + w * 0.5;
    let cy = y + h * 0.5;
    let base_r = (w.max(h)) * 0.9;
    // Aggressive center-out dissipate with visible pulse
    let t = self.anim_t.clamp(0.0, 1.0);
    let eased = 1.0 - (1.0 - t) * (1.0 - t);
    let radius = base_r * (0.15 + 0.85 * eased);
    let alpha = (1.0 - t).powf(0.6) * 0.22;
    let glow_rgb = Self::glow_rgb_from_base(self.base_color);
    renderer.draw_rounded_rect_glow(
      x,
      y,
      w,
      h,
      (h * 0.5).min(10.0),
      cx,
      cy,
      radius,
      Color::new(glow_rgb.r, glow_rgb.g, glow_rgb.b, alpha),
    );

    // Secondary inner pulse for more impact.
    let radius2 = base_r * (0.08 + 0.50 * eased);
    let alpha2 = (1.0 - t).powf(0.4) * 0.30;
    renderer.draw_rounded_rect_glow(
      x,
      y,
      w,
      h,
      (h * 0.5).min(10.0),
      cx,
      cy,
      radius2,
      Color::new(glow_rgb.r, glow_rgb.g, glow_rgb.b, alpha2),
    );

    // Advance animation.
    self.anim_t += 0.16; // ~6-7 frames to finish.
    if self.anim_t >= 1.0 {
      self.anim_active = false;
      self.anim_t = 0.0;
    }
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
  fn render(&mut self, renderer: &mut Renderer, rect: Rect) {
    if !self.visible {
      return;
    }

    // Cache font metrics for mouse handling
    self.cached_char_width = renderer.cell_width();
    self.cached_line_height = renderer.cell_height();

    let (x, y, w, h) = Self::rect_to_pixels(rect, renderer);
    let radius = (h * 0.5).min(10.0);

    // Base + outline
    self.draw_outline_button(renderer, x, y, w, h, radius);

    // Hover glow following cursor
    self.draw_hover_glow(renderer, x, y, w, h);

    // Click flash (one-shot)
    self.draw_click_flash(renderer, x, y, w, h);

    // Label centered
    let text_color = if self.hovered {
      // Slightly elevated contrast on hover
      let lifted = Self::mix(self.base_color, Color::WHITE, 0.70);
      Color::new(lifted.r, lifted.g, lifted.b, 1.0)
    } else {
      self.base_color
    };
    let font_size = (h * 0.5).max(12.0).min(20.0);
    // Position is top-left; center the text inside the button.
    let text = self.label.clone();
    // Estimate text width: ~0.6 * font_size per character as a rough width
    let est_char_w = font_size * 0.6;
    let text_w = est_char_w * (text.chars().count() as f32);
    let tx = x + (w - text_w) * 0.5;
    let ty = y + (h - font_size) * 0.5; // top-left positioning
    renderer.draw_text(TextSection::simple(tx, ty, text, font_size, text_color));
  }

  fn preferred_size(&self) -> Option<(u16, u16)> {
    // 16 cols wide, 2 rows tall by default
    Some((16, 2))
  }

  fn is_visible(&self) -> bool {
    self.visible
  }
  fn set_visible(&mut self, visible: bool) {
    self.visible = visible;
  }

  fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
    self
  }

  fn is_animating(&self) -> bool {
    self.anim_active
  }

  fn handle_mouse(&mut self, mouse: &MouseEvent, rect: Rect) -> bool {
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
        return true; // consume + request redraw
      } else if self.pressed && !mouse.pressed {
        // Release
        self.pressed = false;
        // Trigger center-out glow animation
        self.anim_active = true;
        self.anim_t = 0.0;
        // Fire callback only if release occurs inside the button
        if inside {
          if let Some(cb) = self.on_click.as_mut() {
            (cb)();
          }
        }
        return true;
      }
    }

    // Request redraw when leaving/entering hover, or for hover motion
    inside || (prev_hovered != self.hovered)
  }
}
