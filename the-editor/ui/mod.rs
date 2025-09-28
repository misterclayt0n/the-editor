use std::collections::HashMap;

use the_editor_renderer::{
  Color,
  KeyPress,
  MouseEvent,
  Renderer,
};

use crate::{
  core::graphics::Rect,
  editor::Editor,
  ui::job::Jobs,
};

pub mod components;
pub mod compositor;
pub mod editor_view;
pub mod job;

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


