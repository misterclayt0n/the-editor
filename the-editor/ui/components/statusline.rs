use the_editor_renderer::{
  Color,
  Renderer,
  TextSection,
};

use crate::{
  core::graphics::Rect,
  editor::{
    StatusLineConfig,
    StatusLineElement,
  },
  keymap::Mode,
  ui::{
    Component,
    UI_FONT_SIZE,
  },
};

const STATUS_BAR_HEIGHT: f32 = 30.0;

/// StatusLine component that displays editor status information at the bottom
/// of the screen
pub struct StatusLine {
  // Configuration
  config: StatusLineConfig,

  // State
  visible: bool,

  // Status information
  mode:        Mode,
  cursor_line: usize,
  cursor_col:  usize,
  total_lines: usize,
  start_line:  usize,
  file_name:   Option<String>,
  is_modified: bool,

  // Command mode support - the statusline should not render when command prompt is shown
  show_command_prompt: bool,
}

impl StatusLine {
  pub fn new(config: StatusLineConfig) -> Self {
    Self {
      config,
      visible: true,
      mode: Mode::Normal,
      cursor_line: 0,
      cursor_col: 0,
      total_lines: 1,
      start_line: 0,
      file_name: None,
      is_modified: false,
      show_command_prompt: false,
    }
  }

  /// Update the editor state information
  pub fn update_state(
    &mut self,
    mode: Mode,
    cursor_line: usize,
    cursor_col: usize,
    total_lines: usize,
    start_line: usize,
    file_name: Option<String>,
    is_modified: bool,
    show_command_prompt: bool,
  ) {
    self.mode = mode;
    self.cursor_line = cursor_line;
    self.cursor_col = cursor_col;
    self.total_lines = total_lines;
    self.start_line = start_line;
    self.file_name = file_name;
    self.is_modified = is_modified;
    self.show_command_prompt = show_command_prompt;
  }

  /// Get the mode string representation
  fn mode_str(&self) -> &'static str {
    match self.mode {
      Mode::Normal => "NORMAL",
      Mode::Insert => "INSERT",
      Mode::Select => "VISUAL",
      Mode::Command => "COMMAND",
    }
  }

  /// Render a status line element
  fn render_element(&self, element: &StatusLineElement) -> String {
    match element {
      StatusLineElement::Mode => self.mode_str().to_string(),
      StatusLineElement::Position => {
        format!(
          "Ln {}/{} Col {}",
          self.cursor_line + 1,
          self.total_lines,
          self.cursor_col + 1
        )
      },
      StatusLineElement::TotalLineNumbers => self.total_lines.to_string(),
      StatusLineElement::FileName => {
        self
          .file_name
          .clone()
          .unwrap_or_else(|| "[No Name]".to_string())
      },
      StatusLineElement::FileBaseName => {
        self
          .file_name
          .as_ref()
          .and_then(|name| std::path::Path::new(name).file_name())
          .and_then(|name| name.to_str())
          .unwrap_or("[No Name]")
          .to_string()
      },
      StatusLineElement::FileModificationIndicator => {
        if self.is_modified { "[+]" } else { "" }.to_string()
      },
      StatusLineElement::Separator => self.config.separator.clone(),
      StatusLineElement::Spacer => " ".to_string(),
      StatusLineElement::PositionPercentage => {
        let percentage = if self.total_lines > 0 {
          ((self.cursor_line + 1) * 100) / self.total_lines
        } else {
          0
        };
        format!("{}%", percentage)
      },
      // TODO: Implement other elements as needed
      _ => "".to_string(),
    }
  }

  /// Render a list of status line elements
  fn render_elements(&self, elements: &[StatusLineElement]) -> String {
    elements
      .iter()
      .map(|element| self.render_element(element))
      .filter(|s| !s.is_empty())
      .collect::<Vec<_>>()
      .join("")
  }

  /// Get the status text to display
  fn get_status_text(&self) -> String {
    // Simple implementation for now - just show the basic info
    // This maintains compatibility with the existing hardcoded format
    format!(
      "{} | Ln {}/{} Col {} | Top {}",
      self.mode_str(),
      self.cursor_line + 1,
      self.total_lines,
      self.cursor_col + 1,
      self.start_line + 1,
    )
  }
}

impl Component for StatusLine {
  fn render(&mut self, renderer: &mut Renderer, rect: Rect) {
    if !self.visible || self.show_command_prompt {
      // Don't render statusline when command prompt is shown
      return;
    }

    // Calculate position - status line should be at bottom
    // Convert rect coordinates to pixel coordinates using the same approach as the
    // button component
    let _char_w = 12.0f32; // May be used later for positioning
    let line_h = 20.0f32;
    let x = 10.0; // Fixed left margin like original
    let y = renderer.height() as f32 - STATUS_BAR_HEIGHT;
    // let y = rect.y as f32 * line_h;
    let status_text = self.get_status_text();

    // Render the status text
    renderer.draw_text(TextSection::simple(
      x,
      y,
      status_text,
      UI_FONT_SIZE,
      Color::rgb(0.6, 0.6, 0.7),
    ));
  }

  fn preferred_size(&self) -> Option<(u16, u16)> {
    // Status line should span the full width and be 1 line tall
    None // Let the layout manager determine size
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
}
