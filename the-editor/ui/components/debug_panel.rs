use the_editor_renderer::{
  Color,
  KeyPress,
  Renderer,
  TextSection,
  TextSegment,
  TextStyle,
};

use crate::{
  core::graphics::Rect,
  ui::Component,
};

/// A simple debug panel component that displays editor information
pub struct DebugPanel {
  visible: bool,
  title: String,
  lines: Vec<String>,
}

impl DebugPanel {
  pub fn new() -> Self {
    Self {
      visible: false,
      title: "Debug Panel".to_string(),
      lines: vec![
        "This is completely mocked btw, no worries here".to_string(),
        "Press Ctrl+B to toggle".to_string(),
      ],
    }
  }

  pub fn update_info(&mut self, info: Vec<String>) {
    self.lines = info;
  }
}

impl Default for DebugPanel {
  fn default() -> Self {
    Self::new()
  }
}

impl Component for DebugPanel {
  fn render(&mut self, renderer: &mut Renderer, rect: Rect) {
    if !self.visible {
      return;
    }

    let font_size = 16.0;
    let line_height = font_size * 1.4;

    // Calculate pixel positions
    let x = rect.x as f32 * 12.0; // Assuming character width of 12 pixels
    let y = rect.y as f32 * line_height;

    // Title
    let title_text = format!("┌─ {} ─┐", self.title);
    renderer.draw_text(TextSection {
      position: (x, y),
      texts: vec![TextSegment {
        content: title_text,
        style: TextStyle {
          size: font_size,
          color: Color::rgb(0.4, 0.8, 1.0), // Bright blue
        },
      }],
    });

    // Content lines
    for (line_idx, line) in self.lines.iter().enumerate() {
      if line_idx >= rect.height.saturating_sub(2) as usize {
        break; // Don't draw past the available space
      }

      let prefix = if line_idx == self.lines.len() - 1 {
        "└ "
      } else {
        "│ "
      };

      renderer.draw_text(TextSection {
        position: (x, y + ((line_idx + 1) as f32 * line_height)),
        texts: vec![
          TextSegment {
            content: prefix.to_string(),
            style: TextStyle {
              size: font_size,
              color: Color::rgb(0.4, 0.8, 1.0), // Border color
            },
          },
          TextSegment {
            content: line.clone(),
            style: TextStyle {
              size: font_size,
              color: Color::rgb(0.9, 0.9, 0.9), // Light gray text
            },
          },
        ],
      });
    }
  }

  fn handle_input(&mut self, _key: &KeyPress) -> bool {
    // This component doesn't handle input directly
    false
  }

  fn preferred_size(&self) -> Option<(u16, u16)> {
    // Calculate size based on content
    let width = self.lines.iter()
      .map(|line| line.len())
      .max()
      .unwrap_or(20)
      .max(self.title.len())
      + 4; // padding + borders

    let height = self.lines.len() + 4; // content + borders + title

    Some((width as u16, height as u16))
  }

  fn is_visible(&self) -> bool {
    self.visible
  }

  fn set_visible(&mut self, visible: bool) {
    self.visible = visible;
  }
}
