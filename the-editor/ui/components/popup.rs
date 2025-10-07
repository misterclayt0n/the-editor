use the_editor_renderer::Key;

use crate::{
  core::{
    graphics::Rect,
    position::Position,
  },
  ui::compositor::{
    Component,
    Context,
    Event,
    EventResult,
    Surface,
  },
};

/// Position bias for popup placement
#[derive(Clone, Copy, Debug)]
pub enum PositionBias {
  /// Place popup above the anchor
  Above,
  /// Place popup below the anchor
  Below,
}

/// A generic popup wrapper that positions and manages a child component
pub struct Popup<T: Component> {
  /// The wrapped component
  contents:   T,
  /// Position bias (above or below cursor)
  bias:       PositionBias,
  /// Whether the popup should auto-close when focus is lost
  auto_close: bool,
  /// The area occupied by the popup
  area:       Rect,
  /// Component ID for replacement in compositor
  id:         &'static str,
}

impl<T: Component> Popup<T> {
  /// Create a new popup wrapping a component
  pub fn new(id: &'static str, contents: T) -> Self {
    Self {
      contents,
      bias: PositionBias::Above,
      auto_close: false,
      area: Rect::default(),
      id,
    }
  }

  /// Set whether the popup should auto-close
  pub fn auto_close(mut self, auto_close: bool) -> Self {
    self.auto_close = auto_close;
    self
  }

  /// Set the position bias
  pub fn position_bias(mut self, bias: PositionBias) -> Self {
    self.bias = bias;
    self
  }

  /// Calculate the popup position based on cursor and viewport
  fn calculate_position(
    &mut self,
    viewport: Rect,
    cursor_pos: Option<Position>,
    required_size: Option<(u16, u16)>,
  ) -> Rect {
    let (width, height) = required_size.unwrap_or((40, 10));

    // If no cursor position, center in viewport
    let Some(cursor) = cursor_pos else {
      return Rect::new(
        viewport.x + (viewport.width.saturating_sub(width)) / 2,
        viewport.y + (viewport.height.saturating_sub(height)) / 2,
        width,
        height,
      );
    };

    // Position relative to cursor
    let cursor_x = cursor.col as u16;
    let cursor_y = cursor.row as u16;

    // Try to position based on bias
    let mut x = cursor_x.saturating_sub(width / 2); // Center horizontally on cursor
    let mut y = match self.bias {
      PositionBias::Above => cursor_y.saturating_sub(height + 1), // 1 line above cursor
      PositionBias::Below => cursor_y + 2,                         // 2 lines below cursor
    };

    // Clamp to viewport
    if x + width > viewport.width {
      x = viewport.width.saturating_sub(width);
    }
    if y + height > viewport.height {
      // If doesn't fit below, try above
      match self.bias {
        PositionBias::Below => {
          let new_y = cursor_y.saturating_sub(height + 1);
          if new_y + height <= viewport.height {
            y = new_y;
          }
        },
        PositionBias::Above => {
          // If doesn't fit above, show below anyway
          y = cursor_y + 2;
          // Clamp to viewport
          if y + height > viewport.height {
            y = viewport.height.saturating_sub(height);
          }
        },
      }
    }

    Rect::new(viewport.x + x, viewport.y + y, width, height)
  }
}

impl<T: Component> Component for Popup<T> {
  fn render(&mut self, area: Rect, surface: &mut Surface, ctx: &mut Context) {
    // Get cursor position from current view
    let (cursor_pos, _cursor_kind) = ctx.editor.cursor();

    // Get required size from contents
    let required_size = self.contents.required_size((area.width, area.height));

    // Calculate popup position
    self.area = self.calculate_position(area, cursor_pos, required_size);

    // Render the contents
    self.contents.render(self.area, surface, ctx);
  }

  fn handle_event(&mut self, event: &Event, ctx: &mut Context) -> EventResult {
    // Handle escape to close
    if let Event::Key(key) = event {
      if matches!(key.code, Key::Escape) && self.auto_close {
        return EventResult::Consumed(Some(Box::new(|compositor, _ctx| {
          compositor.pop();
        })));
      }
    }

    // Forward events to contents
    match self.contents.handle_event(event, ctx) {
      EventResult::Consumed(callback) => EventResult::Consumed(callback),
      EventResult::Ignored(_) => {
        // If auto-close and event not handled by contents, close on any key
        if self.auto_close {
          if let Event::Key(_) = event {
            return EventResult::Consumed(Some(Box::new(|compositor, _ctx| {
              compositor.pop();
            })));
          }
        }
        EventResult::Ignored(None)
      },
    }
  }

  fn required_size(&mut self, viewport: (u16, u16)) -> Option<(u16, u16)> {
    self.contents.required_size(viewport)
  }

  fn id(&self) -> Option<&'static str> {
    Some(self.id)
  }

  fn is_animating(&self) -> bool {
    self.contents.is_animating()
  }
}
