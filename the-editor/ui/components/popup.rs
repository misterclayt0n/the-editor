use the_editor_renderer::Color;

use crate::{
  core::{
    animation::{
      self,
      AnimationHandle,
    },
    graphics::Rect,
    position::Position,
  },
  ui::{
    UI_FONT_SIZE,
    UI_FONT_WIDTH,
    compositor::{
      Callback,
      Component,
      Context,
      Event,
      EventResult,
      Surface,
    },
    popup_positioning::{
      CursorPosition,
      calculate_cursor_position,
      constrain_popup_height,
      position_popup_near_cursor,
    },
  },
};

/// Position bias for popup placement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PositionBias {
  /// Prefer positioning the popup above the anchor.
  Above,
  /// Prefer positioning the popup below the anchor.
  Below,
}

/// Layout limits expressed in UI character units.
#[derive(Clone, Copy, Debug)]
pub struct PopupLimits {
  pub min_width:  u16,
  pub max_width:  u16,
  pub min_height: u16,
  pub max_height: u16,
}

impl Default for PopupLimits {
  fn default() -> Self {
    Self {
      min_width:  12,
      max_width:  120,
      min_height: 4,
      max_height: 26,
    }
  }
}

/// Visual styling for popup chrome.
#[derive(Clone, Copy, Debug)]
pub struct PopupStyle {
  pub padding:          f32,
  pub corner_radius:    f32,
  pub border_thickness: f32,
}

impl Default for PopupStyle {
  fn default() -> Self {
    Self {
      padding:          12.0,
      corner_radius:    6.0,
      border_thickness: 1.0,
    }
  }
}

/// Measurement constraints for popup content in physical pixels.
#[derive(Clone, Copy, Debug)]
pub struct PopupConstraints {
  pub max_width:  f32,
  pub max_height: f32,
}

/// Reported size for popup content in physical pixels.
#[derive(Clone, Copy, Debug)]
pub struct PopupSize {
  pub width:  f32,
  pub height: f32,
}

impl PopupSize {
  pub const ZERO: Self = Self {
    width:  0.0,
    height: 0.0,
  };
}

/// Rectangle in physical pixels.
#[derive(Clone, Copy, Debug)]
pub struct RectPx {
  pub x:      f32,
  pub y:      f32,
  pub width:  f32,
  pub height: f32,
}

impl RectPx {
  pub fn inset(&self, padding: f32) -> Self {
    let pad2 = padding * 2.0;
    Self {
      x:      self.x + padding,
      y:      self.y + padding,
      width:  (self.width - pad2).max(0.0),
      height: (self.height - pad2).max(0.0),
    }
  }

  pub fn right(&self) -> f32 {
    self.x + self.width
  }

  pub fn bottom(&self) -> f32 {
    self.y + self.height
  }
}

/// Rendering context handed to popup contents during drawing.
pub struct PopupFrame<'a> {
  surface: &'a mut Surface,
  outer:   RectPx,
  inner:   RectPx,
  alpha:   f32,
}

impl<'a> PopupFrame<'a> {
  fn new(surface: &'a mut Surface, outer: RectPx, inner: RectPx, alpha: f32) -> Self {
    Self {
      surface,
      outer,
      inner,
      alpha,
    }
  }

  pub fn surface(&mut self) -> &mut Surface {
    self.surface
  }

  pub fn outer(&self) -> RectPx {
    self.outer
  }

  pub fn inner(&self) -> RectPx {
    self.inner
  }

  pub fn alpha(&self) -> f32 {
    self.alpha
  }

  pub fn inner_origin(&self) -> (f32, f32) {
    (self.inner.x, self.inner.y)
  }
}

/// Behaviour required from popup content.
pub trait PopupContent {
  /// Measure intrinsic content size (excluding shell padding) within
  /// constraints.
  fn measure(
    &mut self,
    surface: &Surface,
    ctx: &mut Context,
    constraints: PopupConstraints,
  ) -> PopupSize;

  /// Render the content inside the provided frame.
  fn render(&mut self, frame: &mut PopupFrame<'_>, ctx: &mut Context);

  /// Handle events before the shell applies auto-close behaviour.
  fn handle_event(&mut self, _event: &Event, _ctx: &mut Context) -> EventResult {
    EventResult::Ignored(None)
  }

  /// Report whether the content is animating.
  fn is_animating(&self) -> bool {
    false
  }
}

/// Generic popup wrapper that handles anchoring, layout, chrome, and clipping.
pub struct PopupShell<T: PopupContent> {
  id:              &'static str,
  contents:        T,
  anchor:          Option<Position>,
  bias:            Option<PositionBias>,
  auto_close:      bool,
  limits:          PopupLimits,
  style:           PopupStyle,
  animation:       AnimationHandle<f32>,
  last_outer_size: Option<(u16, u16)>,
  last_outer_rect: Option<RectPx>,
}

impl<T: PopupContent> PopupShell<T> {
  pub fn new(id: &'static str, contents: T) -> Self {
    let (duration, easing) = animation::presets::POPUP;
    Self {
      id,
      contents,
      anchor: None,
      bias: None,
      auto_close: true,
      limits: PopupLimits::default(),
      style: PopupStyle::default(),
      animation: AnimationHandle::new(0.0, 1.0, duration, easing),
      last_outer_size: None,
      last_outer_rect: None,
    }
  }

  pub fn with_anchor(mut self, anchor: Option<Position>) -> Self {
    self.anchor = anchor;
    self
  }

  pub fn position_bias(mut self, bias: Option<PositionBias>) -> Self {
    self.bias = bias;
    self
  }

  pub fn auto_close(mut self, auto_close: bool) -> Self {
    self.auto_close = auto_close;
    self
  }

  pub fn with_limits(mut self, limits: PopupLimits) -> Self {
    self.limits = limits;
    self
  }

  pub fn with_style(mut self, style: PopupStyle) -> Self {
    self.style = style;
    self
  }

  pub fn content(&self) -> &T {
    &self.contents
  }

  pub fn content_mut(&mut self) -> &mut T {
    &mut self.contents
  }

  pub fn set_anchor(&mut self, anchor: Option<Position>) {
    self.anchor = anchor;
  }

  pub fn last_outer_rect(&self) -> Option<RectPx> {
    self.last_outer_rect
  }

  pub fn alpha(&self) -> f32 {
    *self.animation.current()
  }

  fn close_callback(id: &'static str) -> Callback {
    Box::new(move |compositor, _ctx| {
      compositor.remove(id);
    })
  }

  fn viewport_rect(area: Rect, cell_w: f32, cell_h: f32) -> RectPx {
    RectPx {
      x:      area.x as f32 * cell_w,
      y:      area.y as f32 * cell_h,
      width:  area.width as f32 * cell_w,
      height: area.height as f32 * cell_h,
    }
  }

  fn anchor_position(&self, ctx: &Context, surface: &mut Surface) -> Option<CursorPosition> {
    // Use shared cursor position calculation for consistent positioning
    // with completer and signature help
    calculate_cursor_position(ctx, surface)
  }

  fn compute_outer_rect(
    &self,
    viewport_rect: RectPx,
    content_size: PopupSize,
    ui_cell_w: f32,
    ui_cell_h: f32,
    cursor: Option<CursorPosition>,
    slide_offset: f32,
    scale: f32,
    surface_width: f32,
    surface_height: f32,
    min_y: f32,
  ) -> RectPx {
    let padding = self.style.padding;
    let min_outer_width = (self.limits.min_width as f32 * ui_cell_w)
      .max(0.0)
      .min(viewport_rect.width);
    let min_outer_height = (self.limits.min_height as f32 * ui_cell_h)
      .max(0.0)
      .min(viewport_rect.height);
    let max_outer_width = (self.limits.max_width as f32 * ui_cell_w)
      .max(min_outer_width)
      .min(viewport_rect.width);
    let max_outer_height = (self.limits.max_height as f32 * ui_cell_h)
      .max(min_outer_height)
      .min(viewport_rect.height);

    let mut outer_width = (content_size.width + padding * 2.0)
      .clamp(min_outer_width, max_outer_width)
      .min(viewport_rect.width);
    let mut outer_height = (content_size.height + padding * 2.0)
      .clamp(min_outer_height, max_outer_height)
      .min(viewport_rect.height);

    if !outer_width.is_finite() || !outer_height.is_finite() {
      outer_width = min_outer_width.max(240.0).min(max_outer_width);
      outer_height = min_outer_height.max(160.0).min(max_outer_height);
    }

    let (x, y) = match cursor {
      Some(cursor_pos) => {
        // Use shared positioning logic
        // For hover, align with cursor column (like completion), not centered
        // Pass bias to respect preferred positioning side
        // Use surface dimensions (not viewport_rect) because cursor position is in
        // screen coordinates
        let popup_pos = position_popup_near_cursor(
          cursor_pos,
          outer_width,
          outer_height,
          surface_width,
          surface_height,
          min_y,
          slide_offset,
          scale,
          self.bias,
        );
        (popup_pos.x, popup_pos.y)
      },
      None => {
        // Center in viewport if no cursor
        let clamp_x = |value: f32| {
          let max_x = (viewport_rect.right() - outer_width).max(viewport_rect.x);
          let min_x = viewport_rect.x.min(max_x);
          value.clamp(min_x, max_x)
        };
        let clamp_y = |value: f32| {
          let max_y = (viewport_rect.bottom() - outer_height).max(viewport_rect.y);
          let min_y = viewport_rect.y.min(max_y);
          value.clamp(min_y, max_y)
        };
        let px = clamp_x(viewport_rect.x + (viewport_rect.width - outer_width) / 2.0);
        let py = clamp_y(viewport_rect.y + (viewport_rect.height - outer_height) / 2.0);
        (px, py)
      },
    };

    RectPx {
      x,
      y,
      width: outer_width,
      height: outer_height,
    }
  }

  fn measure_with_surface(
    &mut self,
    surface: &Surface,
    ctx: &mut Context,
    viewport: Rect,
  ) -> PopupSize {
    let doc_cell_w = surface.cell_width();
    let doc_cell_h = surface.cell_height();
    let viewport_px = Self::viewport_rect(viewport, doc_cell_w, doc_cell_h);
    let ui_cell_w = UI_FONT_WIDTH.max(1.0);
    let ui_cell_h = (UI_FONT_SIZE + 4.0).max(1.0);

    let padding = self.style.padding * 2.0;
    let max_inner_width =
      ((self.limits.max_width as f32 * ui_cell_w).min(viewport_px.width) - padding).max(0.0);
    let max_inner_height =
      ((self.limits.max_height as f32 * ui_cell_h).min(viewport_px.height) - padding).max(0.0);

    let constraints = PopupConstraints {
      max_width:  max_inner_width,
      max_height: max_inner_height,
    };

    self.contents.measure(surface, ctx, constraints)
  }
}

impl<T: PopupContent + 'static> Component for PopupShell<T> {
  fn render(&mut self, area: Rect, surface: &mut Surface, ctx: &mut Context) {
    let mut content_size = self.measure_with_surface(surface, ctx, area);

    self.animation.update(ctx.dt);
    let eased = *self.animation.current();

    let font_state = surface.save_font_state();
    let doc_cell_w = font_state.cell_width.max(1.0);
    let doc_cell_h = font_state.cell_height.max(1.0);
    let viewport_px = Self::viewport_rect(area, doc_cell_w, doc_cell_h);
    let cursor = self.anchor_position(ctx, surface);

    surface.configure_font(&font_state.family, UI_FONT_SIZE);
    let ui_cell_w = surface.cell_width().max(UI_FONT_WIDTH.max(1.0));
    let ui_cell_h = surface.cell_height().max((UI_FONT_SIZE + 4.0).max(1.0));

    // Get surface dimensions for positioning (cursor position is in screen
    // coordinates)
    let surface_width = surface.width() as f32;
    let surface_height = surface.height() as f32;

    // Constrain popup height based on available space (same logic as signature
    // helper)
    if let Some(cursor_pos) = cursor {
      let padding = self.style.padding;
      let min_popup_height = (self.limits.min_height as f32 * ui_cell_h)
        .max(padding * 2.0)
        .min(surface_height);

      // Constrain content height to fit available space
      // Pass bias to respect preferred positioning side
      // Use surface_height (not viewport_px.height) because cursor position is in
      // screen coordinates
      // min_y is the bufferline height (top boundary where popups cannot be placed)
      let min_y = ctx.editor.viewport_pixel_offset.1;
      let constrained_height = constrain_popup_height(
        cursor_pos,
        content_size.height + padding * 2.0,
        min_popup_height,
        surface_height,
        min_y,
        self.bias,
      );

      // Adjust content_size height to fit within constrained space
      content_size.height = (constrained_height - padding * 2.0).max(0.0);
    }

    // Calculate animation slide offset and scale
    let slide_offset = if matches!(self.bias, Some(PositionBias::Above)) {
      -(1.0 - eased) * 8.0
    } else {
      (1.0 - eased) * 8.0
    };
    let scale = 0.95 + (eased * 0.05); // 95% -> 100%

    // min_y is the bufferline height (top boundary where popups cannot be placed)
    let min_y = ctx.editor.viewport_pixel_offset.1;
    let outer_rect = self.compute_outer_rect(
      viewport_px,
      content_size,
      ui_cell_w,
      ui_cell_h,
      cursor,
      slide_offset,
      scale,
      surface_width,
      surface_height,
      min_y,
    );

    // Positioning function already handles viewport clamping, so we don't need
    // to clamp again here. Clamping here would override the positioning decision
    // (e.g., if positioned above due to bias, clamping might push it back down).
    let inner_rect = outer_rect.inset(self.style.padding);

    self.last_outer_rect = Some(outer_rect);
    self.last_outer_size = Some((
      (outer_rect.width / doc_cell_w).ceil() as u16,
      (outer_rect.height / doc_cell_h).ceil() as u16,
    ));

    let theme = &ctx.editor.theme;
    let popup_style = theme.get("ui.popup");
    let mut bg_color = popup_style
      .bg
      .map(crate::ui::theme_color_to_renderer_color)
      .unwrap_or(Color::new(0.1, 0.1, 0.14, 1.0));
    let mut border_color = popup_style
      .fg
      .map(crate::ui::theme_color_to_renderer_color)
      .unwrap_or(Color::new(0.3, 0.3, 0.35, 0.8));

    bg_color.a *= eased;
    border_color.a *= eased;

    surface.with_overlay_region(
      outer_rect.x,
      outer_rect.y,
      outer_rect.width,
      outer_rect.height,
      |surface| {
        surface.push_scissor_rect(
          outer_rect.x,
          outer_rect.y,
          outer_rect.width,
          outer_rect.height,
        );

        surface.draw_rounded_rect(
          outer_rect.x,
          outer_rect.y,
          outer_rect.width,
          outer_rect.height,
          self.style.corner_radius,
          bg_color,
        );

        if self.style.border_thickness > 0.0 {
          surface.draw_rounded_rect_stroke(
            outer_rect.x,
            outer_rect.y,
            outer_rect.width,
            outer_rect.height,
            self.style.corner_radius,
            self.style.border_thickness,
            border_color,
          );
        }

        let mut frame = PopupFrame::new(surface, outer_rect, inner_rect, eased);
        self.contents.render(&mut frame, ctx);
        drop(frame);

        surface.pop_scissor_rect();
      },
    );

    surface.restore_font_state(font_state);
  }

  fn handle_event(&mut self, event: &Event, ctx: &mut Context) -> EventResult {
    match self.contents.handle_event(event, ctx) {
      EventResult::Consumed(cb) => EventResult::Consumed(cb),
      EventResult::Ignored(cb) => {
        if let Some(callback) = cb {
          EventResult::Ignored(Some(callback))
        } else if self.auto_close && matches!(event, Event::Key(_)) {
          EventResult::Ignored(Some(Self::close_callback(self.id)))
        } else {
          EventResult::Ignored(None)
        }
      },
    }
  }

  fn required_size(&mut self, _viewport: (u16, u16)) -> Option<(u16, u16)> {
    self.last_outer_size
  }

  fn id(&self) -> Option<&'static str> {
    Some(self.id)
  }

  fn is_animating(&self) -> bool {
    !self.animation.is_complete() || self.contents.is_animating()
  }
}
