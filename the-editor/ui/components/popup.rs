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
  ui::compositor::{
    Callback,
    Component,
    Context,
    Event,
    EventResult,
    Surface,
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

/// Layout limits expressed in terminal cells.
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
  bias:            PositionBias,
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
      bias: PositionBias::Below,
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

  pub fn position_bias(mut self, bias: PositionBias) -> Self {
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

  fn anchor_position(
    &self,
    ctx: &Context,
    viewport: RectPx,
    cell_w: f32,
    cell_h: f32,
  ) -> Option<(f32, f32)> {
    let anchor = self.anchor.or_else(|| ctx.editor.cursor().0)?;
    let x = viewport.x + (anchor.col as f32 * cell_w);
    let y = viewport.y + (anchor.row as f32 * cell_h);
    Some((x, y))
  }

  fn compute_outer_rect(
    &self,
    viewport_rect: RectPx,
    content_size: PopupSize,
    cell_w: f32,
    cell_h: f32,
    anchor: Option<(f32, f32)>,
  ) -> RectPx {
    let padding = self.style.padding;
    let min_outer_width = (self.limits.min_width as f32 * cell_w).min(viewport_rect.width);
    let min_outer_height = (self.limits.min_height as f32 * cell_h).min(viewport_rect.height);
    let max_outer_width = (self.limits.max_width as f32 * cell_w)
      .max(min_outer_width)
      .min(viewport_rect.width);
    let max_outer_height = (self.limits.max_height as f32 * cell_h)
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

    let (x, y) = match anchor {
      Some((ax, ay)) => {
        let px = clamp_x(ax - outer_width / 2.0);

        let gap = cell_h.max(16.0);
        match self.bias {
          PositionBias::Below => {
            let candidate = ay + gap;
            let bottom = candidate + outer_height;
            if bottom <= viewport_rect.bottom() {
              (px, candidate)
            } else {
              let above = ay - gap - outer_height;
              if above >= viewport_rect.y {
                (px, above)
              } else {
                (px, clamp_y(viewport_rect.bottom() - outer_height))
              }
            }
          },
          PositionBias::Above => {
            let candidate = ay - gap - outer_height;
            if candidate >= viewport_rect.y {
              (px, candidate)
            } else {
              let below = ay + gap;
              if below + outer_height <= viewport_rect.bottom() {
                (px, below)
              } else {
                (px, viewport_rect.y)
              }
            }
          },
        }
      },
      None => {
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
    let cell_w = surface.cell_width();
    let cell_h = surface.cell_height();
    let _viewport_px = Self::viewport_rect(viewport, cell_w, cell_h);

    let padding = self.style.padding * 2.0;
    let max_inner_width =
      (self.limits.max_width.min(viewport.width) as f32 * cell_w - padding).max(0.0);
    let max_inner_height =
      (self.limits.max_height.min(viewport.height) as f32 * cell_h - padding).max(0.0);

    let constraints = PopupConstraints {
      max_width:  max_inner_width,
      max_height: max_inner_height,
    };

    self.contents.measure(surface, ctx, constraints)
  }
}

impl<T: PopupContent + 'static> Component for PopupShell<T> {
  fn render(&mut self, area: Rect, surface: &mut Surface, ctx: &mut Context) {
    let content_size = self.measure_with_surface(surface, ctx, area);

    self.animation.update(ctx.dt);
    let eased = *self.animation.current();

    let cell_w = surface.cell_width();
    let cell_h = surface.cell_height();
    let viewport_px = Self::viewport_rect(area, cell_w, cell_h);
    let anchor_px = self.anchor_position(ctx, viewport_px, cell_w, cell_h);

    let mut outer_rect =
      self.compute_outer_rect(viewport_px, content_size, cell_w, cell_h, anchor_px);

    let slide = if matches!(self.bias, PositionBias::Above) {
      -(1.0 - eased) * 8.0
    } else {
      (1.0 - eased) * 8.0
    };
    outer_rect.y =
      (outer_rect.y + slide).clamp(viewport_px.y, viewport_px.bottom() - outer_rect.height);
    let inner_rect = outer_rect.inset(self.style.padding);

    self.last_outer_rect = Some(outer_rect);
    self.last_outer_size = Some((
      (outer_rect.width / cell_w).ceil() as u16,
      (outer_rect.height / cell_h).ceil() as u16,
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
