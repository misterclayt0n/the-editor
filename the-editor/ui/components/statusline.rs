use the_editor_renderer::{
  Color,
  TextSection,
};

use crate::{
  core::graphics::Rect,
  keymap::Mode,
  ui::compositor::{
    Component,
    Context,
    Surface,
  },
};

// Visual constants
const STATUS_BAR_HEIGHT: f32 = 28.0;
const SEGMENT_PADDING_X: f32 = 12.0;
const SEGMENT_SPACING: f32 = 16.0;
const FONT_SIZE: f32 = 13.0;

/// StatusLine component with RAD Debugger aesthetics
/// Emacs-style: shows mode and buffer as plain text with color coding
pub struct StatusLine {
  visible:            bool,
  target_visible:     bool, // Animation target
  anim_t:             f32,  // Animation progress 0.0 -> 1.0
  status_bar_y:       f32,  // Current animated Y position
  // Horizontal slide for prompt
  slide_offset:       f32,  // Current horizontal offset
  should_slide:       bool, // Whether we should be slid for prompt
  slide_anim_t:       f32,  // Slide animation progress 0.0 -> 1.0
  // Status message animation
  status_msg_anim_t:  f32,            // Fade-in animation for status messages
  status_msg_slide_x: f32,            // Horizontal slide position
  last_status_msg:    Option<String>, // Track last message to detect changes
}

impl StatusLine {
  pub fn new() -> Self {
    Self {
      visible:            true,
      target_visible:     true,
      anim_t:             1.0, // Start fully visible
      status_bar_y:       0.0, // Will be calculated on first render
      slide_offset:       0.0,
      should_slide:       false,
      slide_anim_t:       1.0, // Start at rest
      status_msg_anim_t:  0.0, // Start invisible
      status_msg_slide_x: 0.0,
      last_status_msg:    None,
    }
  }

  pub fn toggle(&mut self) {
    self.target_visible = !self.target_visible;
    // Reset animation to start
    self.anim_t = 0.0;
  }

  pub fn is_visible(&self) -> bool {
    self.visible
  }

  /// Slide statusline content to make room for prompt
  pub fn slide_for_prompt(&mut self, slide: bool) {
    self.should_slide = slide;
    self.slide_anim_t = 0.0; // Start animation
  }

  /// Get mode string
  fn mode_text(mode: Mode) -> &'static str {
    match mode {
      Mode::Normal => "NORMAL",
      Mode::Insert => "INSERT",
      Mode::Select => "SELECT",
      Mode::Command => "COMMAND",
    }
  }

  /// Get theme key for current mode
  fn mode_theme_key(mode: Mode) -> &'static str {
    match mode {
      Mode::Normal => "ui.statusline",
      Mode::Insert => "ui.statusline.insert",
      Mode::Select => "ui.statusline.select",
      Mode::Command => "ui.statusline",
    }
  }

  /// Measure text width without drawing
  fn measure_text(text: &str) -> f32 {
    let est_char_w = FONT_SIZE * 0.6;
    est_char_w * (text.chars().count() as f32)
  }

  /// Draw plain text
  fn draw_text(surface: &mut Surface, x: f32, y: f32, text: &str, color: Color) -> f32 {
    let text_w = Self::measure_text(text);
    let text_y = y + (STATUS_BAR_HEIGHT - FONT_SIZE) * 0.5;
    surface.draw_text(TextSection::simple(x, text_y, text, FONT_SIZE, color));
    text_w
  }
}

impl Default for StatusLine {
  fn default() -> Self {
    Self::new()
  }
}

impl Component for StatusLine {
  fn render(&mut self, _area: Rect, surface: &mut Surface, cx: &mut Context) {
    let theme = &cx.editor.theme;
    let mode = cx.editor.mode();

    // Get theme colors for current mode
    let statusline_style = theme.get(Self::mode_theme_key(mode));
    let inactive_style = theme.get("ui.statusline.inactive");

    let bg_color = statusline_style
      .bg
      .map(crate::ui::theme_color_to_renderer_color)
      .unwrap_or(Color::new(0.12, 0.12, 0.13, 1.0));

    let mode_color = statusline_style
      .fg
      .map(crate::ui::theme_color_to_renderer_color)
      .unwrap_or(Color::new(0.85, 0.85, 0.9, 1.0));

    let text_color = inactive_style
      .fg
      .map(crate::ui::theme_color_to_renderer_color)
      .unwrap_or(Color::new(0.6, 0.6, 0.6, 0.9));

    // Update vertical animation (time-based) - 5x faster
    const ANIM_SPEED: f32 = 0.12; // Animation speed per frame (at 60fps)
    if self.anim_t < 1.0 {
      // 5x faster for snappier animations
      let speed = ANIM_SPEED * 420.0; // Speed per second
      self.anim_t = (self.anim_t + speed * cx.dt).min(1.0);
    }

    // Calculate eased animation value (smooth ease-out)
    let eased_t = 1.0 - (1.0 - self.anim_t) * (1.0 - self.anim_t);

    // Update horizontal slide animation (time-based) - 5x faster
    // Calculate target offset based on current viewport width
    const PROMPT_WIDTH_PERCENT: f32 = 0.25; // Match prompt's 25% width
    let target_offset = if self.should_slide {
      surface.width() as f32 * PROMPT_WIDTH_PERCENT + 16.0 // Add spacing after prompt box
    } else {
      0.0
    };

    const SLIDE_SPEED: f32 = 0.15; // Slightly faster for slide (at 60fps)
    if self.slide_anim_t < 1.0 {
      let speed = SLIDE_SPEED * 420.0; // 7x faster
      self.slide_anim_t = (self.slide_anim_t + speed * cx.dt).min(1.0);
    }

    // Calculate eased slide value (smooth ease-out)
    let eased_slide = 1.0 - (1.0 - self.slide_anim_t) * (1.0 - self.slide_anim_t);
    self.slide_offset = self.slide_offset + (target_offset - self.slide_offset) * eased_slide;

    // Calculate Y position with animation
    let base_y = surface.height() as f32 - STATUS_BAR_HEIGHT;
    let hidden_y = surface.height() as f32; // Off-screen below

    let bar_y = if self.target_visible {
      // Sliding up from bottom
      hidden_y + (base_y - hidden_y) * eased_t
    } else {
      // Sliding down to bottom
      base_y + (hidden_y - base_y) * eased_t
    };

    self.status_bar_y = bar_y;

    // Early exit if fully hidden
    if !self.target_visible && self.anim_t >= 1.0 {
      return;
    }

    // Draw background bar across full width
    surface.draw_rect(
      0.0,
      bar_y,
      surface.width() as f32,
      STATUS_BAR_HEIGHT,
      bg_color,
    );

    let view = cx.editor.tree.get(cx.editor.tree.focus);
    let doc = cx.editor.documents.get(&view.doc).unwrap();

    // Left side: MODE | FILE | % | SELECTION
    // Apply horizontal slide offset
    let mut x = SEGMENT_PADDING_X + self.slide_offset;

    // Mode text
    let mode_text = Self::mode_text(mode);
    let mode_width = Self::draw_text(surface, x, bar_y, mode_text, mode_color);
    x += mode_width + SEGMENT_SPACING;

    // Buffer name
    let path = doc.path();
    let modified = doc.is_modified();
    let name = path
      .and_then(|p| p.file_name())
      .and_then(|n| n.to_str())
      .unwrap_or("[No Name]");
    let display_name = if modified {
      format!("{}*", name)
    } else {
      name.to_string()
    };
    let buffer_width = Self::draw_text(surface, x, bar_y, &display_name, text_color);
    x += buffer_width + SEGMENT_SPACING;

    // File percentage (emacs style)
    let text = doc.text();
    let selection = doc.selection(view.id);
    let cursor_line = text.char_to_line(selection.primary().cursor(text.slice(..)));
    let total_lines = text.len_lines();
    let percentage = if total_lines > 0 {
      (cursor_line + 1) * 100 / total_lines
    } else {
      0
    };
    let percent_text = format!("{}%", percentage);
    let percent_width = Self::draw_text(surface, x, bar_y, &percent_text, text_color);
    x += percent_width + SEGMENT_SPACING;

    // Selection count
    let selection_count = selection.ranges().len();
    let selection_text = if selection_count == 1 {
      "1 sel".to_string()
    } else {
      format!("{}/{} sel", selection.primary_index() + 1, selection_count)
    };
    let sel_width = Self::draw_text(surface, x, bar_y, &selection_text, text_color);
    x += sel_width + SEGMENT_SPACING;

    // Status message (with fade-in and slide animation)
    if let Some((status_msg, severity)) = cx.editor.get_status() {
      let anim_enabled = cx.editor.config().status_msg_anim_enabled;

      // Detect message changes to restart animation
      let current_msg = status_msg.to_string();
      if self.last_status_msg.as_ref() != Some(&current_msg) {
        self.last_status_msg = Some(current_msg);
        self.status_msg_anim_t = 0.0; // Reset animation
        if anim_enabled {
          self.status_msg_slide_x = -30.0; // Start 30px to the left
        } else {
          self.status_msg_slide_x = 0.0;
        }
      }

      // Update animation (time-based) - 5x faster
      const STATUS_ANIM_SPEED: f32 = 0.15; // At 60fps
      if self.status_msg_anim_t < 1.0 {
        let speed = STATUS_ANIM_SPEED * 420.0; // 7x faster
        self.status_msg_anim_t = (self.status_msg_anim_t + speed * cx.dt).min(1.0);
      }

      // Calculate eased animation (smooth ease-out)
      let eased = 1.0 - (1.0 - self.status_msg_anim_t) * (1.0 - self.status_msg_anim_t);

      // Lerp the slide position if animation is enabled (time-based) - 5x faster
      if anim_enabled {
        let target_x = 0.0;
        let dx = target_x - self.status_msg_slide_x;
        // 5x faster for snappier slide
        let lerp_factor = 0.25_f32;
        let lerp_t = 1.0 - (1.0 - lerp_factor).powf(cx.dt * 420.0);
        self.status_msg_slide_x += dx * lerp_t;
      } else {
        self.status_msg_slide_x = 0.0;
      }

      // Get color based on severity
      use crate::core::diagnostics::Severity;
      let msg_color = match severity {
        Severity::Error => {
          let error_style = theme.get("error");
          error_style
            .fg
            .map(crate::ui::theme_color_to_renderer_color)
            .unwrap_or(Color::new(0.9, 0.3, 0.3, 1.0))
        },
        Severity::Warning => {
          let warning_style = theme.get("warning");
          warning_style
            .fg
            .map(crate::ui::theme_color_to_renderer_color)
            .unwrap_or(Color::new(0.9, 0.7, 0.3, 1.0))
        },
        Severity::Info => {
          let info_style = theme.get("info");
          info_style
            .fg
            .map(crate::ui::theme_color_to_renderer_color)
            .unwrap_or(Color::new(0.4, 0.7, 0.9, 1.0))
        },
        Severity::Hint => {
          let hint_style = theme.get("hint");
          hint_style
            .fg
            .map(crate::ui::theme_color_to_renderer_color)
            .unwrap_or(Color::new(0.5, 0.5, 0.5, 1.0))
        },
      };

      // Apply animation to color alpha
      let animated_color = Color::new(msg_color.r, msg_color.g, msg_color.b, msg_color.a * eased);

      // Draw at animated position
      let anim_x = x + self.status_msg_slide_x;
      Self::draw_text(surface, anim_x, bar_y, status_msg.as_ref(), animated_color);
    } else {
      // Clear last message when there's no status
      if self.last_status_msg.is_some() {
        self.last_status_msg = None;
        self.status_msg_anim_t = 0.0;
        self.status_msg_slide_x = 0.0;
      }
    }

    // Right side: LSP | GIT BRANCH (right-aligned)
    let mut right_segments = Vec::new();

    // LSP status - show active language servers
    if !doc.language_servers.is_empty() {
      let lsp_names: Vec<_> = doc.language_servers.keys().map(|s| s.as_str()).collect();
      let lsp_text = lsp_names.join(",");
      right_segments.push(lsp_text);
    }

    // Git branch
    if let Some(branch) = doc.version_control_head() {
      right_segments.push(format!("{}", branch.as_ref()));
    }

    // Render right-aligned segments (also offset)
    if !right_segments.is_empty() {
      let combined_text = right_segments.join(" | ");
      let text_width = Self::measure_text(&combined_text);
      let right_x = surface.width() as f32 - text_width - SEGMENT_PADDING_X + self.slide_offset;
      Self::draw_text(surface, right_x, bar_y, &combined_text, text_color);
    }
  }

  fn should_update(&self) -> bool {
    // Keep updating while any animation is running
    self.anim_t < 1.0 || self.slide_anim_t < 1.0 || self.status_msg_anim_t < 1.0
  }
}
