use std::collections::HashMap;

use the_editor_renderer::{
  Color,
  KeyPress,
  MouseEvent,
  Renderer,
};

use crate::core::graphics::Rect;

pub mod components;

// UI Font constants - used across all UI components for consistency
pub const UI_FONT_SIZE: f32 = 14.0;
// Font width calculated based on the monospace font at UI_FONT_SIZE
// Monospace fonts typically have a width-to-height ratio of ~0.6
pub const UI_FONT_WIDTH: f32 = UI_FONT_SIZE * 0.6; // ~8.4 pixels for 14pt font

/// Convert theme color to renderer color
pub fn theme_color_to_renderer_color(theme_color: crate::core::graphics::Color) -> Color {
  use crate::core::graphics::Color as ThemeColor;
  match theme_color {
    ThemeColor::Reset => Color::BLACK,
    ThemeColor::Black => Color::BLACK,
    ThemeColor::Red => Color::RED,
    ThemeColor::Green => Color::GREEN,
    ThemeColor::Yellow => Color::rgb(1.0, 1.0, 0.0),
    ThemeColor::Blue => Color::BLUE,
    ThemeColor::Magenta => Color::rgb(1.0, 0.0, 1.0),
    ThemeColor::Cyan => Color::rgb(0.0, 1.0, 1.0),
    ThemeColor::Gray => Color::GRAY,
    ThemeColor::LightRed => Color::rgb(1.0, 0.5, 0.5),
    ThemeColor::LightGreen => Color::rgb(0.5, 1.0, 0.5),
    ThemeColor::LightYellow => Color::rgb(1.0, 1.0, 0.5),
    ThemeColor::LightBlue => Color::rgb(0.5, 0.5, 1.0),
    ThemeColor::LightMagenta => Color::rgb(1.0, 0.5, 1.0),
    ThemeColor::LightCyan => Color::rgb(0.5, 1.0, 1.0),
    ThemeColor::LightGray => Color::rgb(0.75, 0.75, 0.75),
    ThemeColor::White => Color::WHITE,
    ThemeColor::Rgb(r, g, b) => Color::rgb(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0),
    ThemeColor::Indexed(i) => {
      // Convert 8-bit indexed colors to approximate RGB values
      match i {
        0 => Color::BLACK,
        1 => Color::RED,
        2 => Color::GREEN,
        3 => Color::rgb(1.0, 1.0, 0.0), // yellow
        4 => Color::BLUE,
        5 => Color::rgb(1.0, 0.0, 1.0), // magenta
        6 => Color::rgb(0.0, 1.0, 1.0), // cyan
        7 => Color::WHITE,
        8 => Color::GRAY,
        9 => Color::rgb(1.0, 0.5, 0.5),  // light red
        10 => Color::rgb(0.5, 1.0, 0.5), // light green
        11 => Color::rgb(1.0, 1.0, 0.5), // light yellow
        12 => Color::rgb(0.5, 0.5, 1.0), // light blue
        13 => Color::rgb(1.0, 0.5, 1.0), // light magenta
        14 => Color::rgb(0.5, 1.0, 1.0), // light cyan
        15 => Color::WHITE,
        // For extended colors (16-255), use a simple grayscale approximation
        _ => {
          let gray = (i as f32 - 16.0) / 239.0;
          Color::rgb(gray, gray, gray)
        },
      }
    },
  }
}

/// Core trait for UI components.
pub trait Component {
  /// Render the component using the renderer.
  fn render(&mut self, renderer: &mut Renderer, rect: Rect);

  /// Handle input events, returns true if the event was consumed.
  fn handle_input(&mut self, _key: &KeyPress) -> bool {
    false
  }

  /// Handle mouse events (position/mouse buttons).
  /// Default does nothing and returns false (not consumed).
  fn handle_mouse(&mut self, _mouse: &MouseEvent, _rect: Rect) -> bool {
    false
  }

  /// Get the preferred size for this component.
  fn preferred_size(&self) -> Option<(u16, u16)> {
    None
  }

  /// Whether this component is currently visible.
  fn is_visible(&self) -> bool {
    true
  }

  /// Set visibility of this component.
  fn set_visible(&mut self, visible: bool);

  /// Enable downcasting to concrete types
  fn as_any_mut(&mut self) -> &mut dyn std::any::Any;

  /// Whether the component is currently animating and needs redraws.
  fn is_animating(&self) -> bool {
    false
  }
}

/// Component manager handles layout and rendering of UI components.
pub struct ComponentManager {
  components: HashMap<String, Box<dyn Component>>,
  layout:     Layout,
  positions:  HashMap<String, OverlayPosition>,
  last_rects: HashMap<String, Rect>,
}

impl ComponentManager {
  pub fn new() -> Self {
    Self {
      components: HashMap::new(),
      layout:     Layout::default(),
      positions:  HashMap::new(),
      last_rects: HashMap::new(),
    }
  }

  /// Returns true if any visible component is animating.
  pub fn is_animating(&self) -> bool {
    self
      .components
      .values()
      .any(|c| c.is_visible() && c.is_animating())
  }

  /// Add a component with a unique ID.
  pub fn add_component(&mut self, id: String, component: Box<dyn Component>) {
    self.components.insert(id, component);
  }

  /// Remove a component.
  pub fn remove_component(&mut self, id: &str) -> Option<Box<dyn Component>> {
    self.positions.remove(id);
    self.last_rects.remove(id);
    self.components.remove(id)
  }

  /// Get a mutable reference to a component.
  pub fn get_component_mut(&mut self, id: &str) -> Option<&mut Box<dyn Component>> {
    self.components.get_mut(id)
  }

  /// Toggle visibility of a component.
  pub fn toggle_component(&mut self, id: &str) {
    if let Some(component) = self.components.get_mut(id) {
      let visible = component.is_visible();
      component.set_visible(!visible);
    }
  }

  /// Check if a component is visible.
  pub fn is_component_visible(&self, id: &str) -> bool {
    self.components.get(id).map_or(false, |c| c.is_visible())
  }

  /// Render all visible components.
  pub fn render(&mut self, renderer: &mut Renderer, editor_rect: Rect) {
    // NOTE: This is a simple overlay layout
    // Components are rendered on top of the editor.
    for (id, component) in self.components.iter_mut() {
      if component.is_visible() {
        let pos = self
          .positions
          .get(id)
          .copied()
          .unwrap_or(self.layout.overlay_position);
        let rect = self
          .layout
          .calculate_rect_with(component.as_ref(), editor_rect, pos);
        self.last_rects.insert(id.clone(), rect);
        component.render(renderer, rect);
      }
    }
  }

  /// Handle input for all components.
  pub fn handle_input(&mut self, key: &KeyPress) -> bool {
    for component in self.components.values_mut() {
      if component.is_visible() && component.handle_input(key) {
        return true;
      }
    }
    false
  }

  /// Forward mouse events to components. Returns true if any component consumes
  /// it.
  pub fn handle_mouse(&mut self, mouse: &MouseEvent) -> bool {
    for (id, component) in self.components.iter_mut() {
      if component.is_visible() {
        if let Some(rect) = self.last_rects.get(id).copied() {
          if component.handle_mouse(mouse, rect) {
            return true;
          }
        }
      }
    }
    false
  }

  /// Set the overlay position for a specific component id.
  pub fn set_component_position(&mut self, id: &str, position: OverlayPosition) {
    self.positions.insert(id.to_string(), position);
  }
}

impl Default for ComponentManager {
  fn default() -> Self {
    Self::new()
  }
}

/// Layout manager for positioning components.
#[derive(Debug, Clone)]
pub struct Layout {
  pub overlay_position: OverlayPosition,
}

impl Default for Layout {
  fn default() -> Self {
    Self {
      overlay_position: OverlayPosition::TopRight,
    }
  }
}

#[derive(Debug, Clone, Copy)]
pub enum OverlayPosition {
  TopLeft,
  TopRight,
  BottomLeft,
  BottomRight,
  Center,
  StatusLine, // Full width at bottom
}

impl Layout {
  /// Calculate the rectangle for a component within the available area
  pub fn calculate_rect(&self, component: &dyn Component, available: Rect) -> Rect {
    let (width, height) = component.preferred_size().unwrap_or((40, 10));
    let width = width.min(available.width);
    let height = height.min(available.height);

    match self.overlay_position {
      OverlayPosition::TopLeft => Rect::new(available.x, available.y, width, height),
      OverlayPosition::TopRight => {
        Rect::new(
          available.x + available.width.saturating_sub(width),
          available.y,
          width,
          height,
        )
      },
      OverlayPosition::BottomLeft => {
        Rect::new(
          available.x,
          available.y + available.height.saturating_sub(height),
          width,
          height,
        )
      },
      OverlayPosition::BottomRight => {
        Rect::new(
          available.x + available.width.saturating_sub(width),
          available.y + available.height.saturating_sub(height),
          width,
          height,
        )
      },
      OverlayPosition::Center => {
        let center_x = available.x + available.width / 2;
        let center_y = available.y + available.height / 2;
        Rect::new(
          center_x.saturating_sub(width / 2),
          center_y.saturating_sub(height / 2),
          width,
          height,
        )
      },
      OverlayPosition::StatusLine => {
        // Status line spans full width at bottom
        // The available area includes the status line area, so position at the last row
        Rect::new(
          available.x,
          available.y + available.height.saturating_sub(1), // Last row
          available.width,
          1, // 1 row tall
        )
      },
    }
  }

  /// Calculate rect using an explicit overlay position (overrides default).
  pub fn calculate_rect_with(
    &self,
    component: &dyn Component,
    available: Rect,
    position: OverlayPosition,
  ) -> Rect {
    let (width, height) = component.preferred_size().unwrap_or((40, 10));
    let width = width.min(available.width);
    let height = height.min(available.height);

    match position {
      OverlayPosition::TopLeft => Rect::new(available.x, available.y, width, height),
      OverlayPosition::TopRight => {
        Rect::new(
          available.x + available.width.saturating_sub(width),
          available.y,
          width,
          height,
        )
      },
      OverlayPosition::BottomLeft => {
        Rect::new(
          available.x,
          available.y + available.height.saturating_sub(height),
          width,
          height,
        )
      },
      OverlayPosition::BottomRight => {
        Rect::new(
          available.x + available.width.saturating_sub(width),
          available.y + available.height.saturating_sub(height),
          width,
          height,
        )
      },
      OverlayPosition::Center => {
        let center_x = available.x + available.width / 2;
        let center_y = available.y + available.height / 2;
        Rect::new(
          center_x.saturating_sub(width / 2),
          center_y.saturating_sub(height / 2),
          width,
          height,
        )
      },
      OverlayPosition::StatusLine => {
        // Status line spans full width at bottom
        // The available area includes the status line area, so position at the last row
        Rect::new(
          available.x,
          available.y + available.height.saturating_sub(1), // Last row
          available.width,
          1, // 1 row tall
        )
      },
    }
  }
}
