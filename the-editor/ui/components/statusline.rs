use std::{
  collections::HashMap,
  time::Instant,
};

use the_editor_renderer::{
  Color,
  TextSection,
};

use crate::{
  core::{
    animation::breathing::BreathingAnimation,
    graphics::Rect,
  },
  keymap::Mode,
  lsp::LanguageServerId,
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
  visible:             bool,
  target_visible:      bool, // Animation target
  anim_t:              f32,  // Animation progress 0.0 -> 1.0
  status_bar_y:        f32,  // Current animated Y position
  // Horizontal slide for prompt
  slide_offset:        f32,  // Current horizontal offset
  should_slide:        bool, // Whether we should be slid for prompt
  slide_anim_t:        f32,  // Slide animation progress 0.0 -> 1.0
  // Status message animation
  status_msg_anim_t:   f32, // Fade-in animation for status messages
  status_msg_slide_x:  f32, // Horizontal slide position
  last_status_msg:     Option<String>, // Track last message to detect changes
  // LSP loading breathing animations with last-seen timestamps for stability
  lsp_breathing_anims: HashMap<LanguageServerId, (BreathingAnimation, Instant)>,
}

impl StatusLine {
  pub fn new() -> Self {
    Self {
      visible:             true,
      target_visible:      true,
      anim_t:              1.0, // Start fully visible
      status_bar_y:        0.0, // Will be calculated on first render
      slide_offset:        0.0,
      should_slide:        false,
      slide_anim_t:        1.0, // Start at rest
      status_msg_anim_t:   0.0, // Start invisible
      status_msg_slide_x:  0.0,
      last_status_msg:     None,
      lsp_breathing_anims: HashMap::new(),
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
    // Save current font state and configure UI font for statusline rendering
    let saved_font = surface.save_font_state();
    let ui_font_family = surface.current_font_family().to_owned();
    surface.configure_font(&ui_font_family, FONT_SIZE);

    let theme = &cx.editor.theme;
    let mode = cx.editor.mode();

    // Use constant statusline colors (no mode-specific changes)
    let statusline_style = theme.get("ui.statusline");

    // Background color - constant
    let bg_color = statusline_style
      .bg
      .map(crate::ui::theme_color_to_renderer_color)
      .unwrap_or(Color::new(0.12, 0.12, 0.13, 1.0));

    // Text color - constant (same for mode and all other text)
    let text_color = statusline_style
      .fg
      .map(crate::ui::theme_color_to_renderer_color)
      .unwrap_or(Color::new(0.85, 0.85, 0.9, 1.0));

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

    // Early exit if fully hidden - no need to render anything
    if !self.target_visible && self.anim_t >= 1.0 {
      self.visible = false;
      return;
    }

    self.visible = true;

    // Draw background bar across full width
    surface.draw_rect(
      0.0,
      bar_y,
      surface.width() as f32,
      STATUS_BAR_HEIGHT,
      bg_color,
    );

    // Render statusline content in overlay mode with automatic masking
    let viewport_width = surface.width() as f32;
    surface.with_overlay_region(0.0, bar_y, viewport_width, STATUS_BAR_HEIGHT, |surface| {
      // Check if focused on a terminal instead of a view
      let focus_id = cx.editor.tree.focus;
      if cx.editor.tree.get_terminal(focus_id).is_some() {
        // Render terminal statusline
        let mut x = SEGMENT_PADDING_X + self.slide_offset;
        let mode_text = "TERMINAL";
        let mode_width = Self::draw_text(surface, x, bar_y, mode_text, text_color);
        x += mode_width + SEGMENT_SPACING;

        let term_info = "Integrated Terminal";
        Self::draw_text(surface, x, bar_y, term_info, text_color);
        return;
      }

      let view = cx.editor.tree.get(focus_id);
      let doc = cx.editor.documents.get(&view.doc).unwrap();

      // Left side: MODE | FILE | % | SELECTION
      // Apply horizontal slide offset
      let mut x = SEGMENT_PADDING_X + self.slide_offset;

      // Mode text (use custom mode string if set, otherwise use mode name)
      let mode_text = cx
        .editor
        .custom_mode_str
        .as_deref()
        .unwrap_or_else(|| Self::mode_text(mode));
      let mode_width = Self::draw_text(surface, x, bar_y, mode_text, text_color);
      x += mode_width + SEGMENT_SPACING;

      // Buffer name
      let path = doc.path();
      let modified = doc.is_modified();

      // Compute display name based on workspace context
      let display_name = if let Some(path) = path {
        // Try to get workspace root
        if let Some(workspace_root) = cx.editor.diff_providers.get_workspace_root(path) {
          // File is inside a workspace - show relative path from workspace root
          if let Ok(rel_path) = path.strip_prefix(&workspace_root) {
            let path_str = rel_path.to_str().unwrap_or("[Invalid Path]");
            if modified {
              format!("{}*", path_str)
            } else {
              path_str.to_string()
            }
          } else {
            // Shouldn't happen, but fallback to full path with ~
            let folded = the_editor_stdx::path::fold_home_dir(path);
            let path_str = folded.to_str().unwrap_or("[Invalid Path]");
            if modified {
              format!("{}*", path_str)
            } else {
              path_str.to_string()
            }
          }
        } else {
          // File is outside workspace - show full path with ~ abbreviation
          let folded = the_editor_stdx::path::fold_home_dir(path);
          let path_str = folded.to_str().unwrap_or("[Invalid Path]");
          if modified {
            format!("{}*", path_str)
          } else {
            path_str.to_string()
          }
        }
      } else if let Some(kind) = doc.special_buffer_kind() {
        let mut label = kind.display_name().to_string();
        if cx.editor.is_special_buffer_running(view.doc) {
          label.push_str(" !");
        }
        if modified {
          label.push('*');
        }
        label
      } else if modified {
        "[No Name]*".to_string()
      } else {
        "[No Name]".to_string()
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
      let mut right_x = surface.width() as f32 - SEGMENT_PADDING_X + self.slide_offset;

      // Git branch (render first, right-most)
      if let Some(branch) = doc.version_control_head() {
        let branch_text = format!("{}", branch.as_ref());
        let branch_width = Self::measure_text(&branch_text);
        right_x -= branch_width;
        Self::draw_text(surface, right_x, bar_y, &branch_text, text_color);
        right_x -= Self::measure_text(" | ");
      }

      // LSP status - show active language servers with breathing animation when
      // loading
      if !doc.language_servers.is_empty() {
        let now = std::time::Instant::now();

        // Collect LSP server names and IDs
        let lsp_servers: Vec<_> = doc
          .language_servers
          .iter()
          .map(|(name, client)| (name.as_str(), client.id()))
          .collect();

        // Update breathing animations with hysteresis to prevent flickering
        // Animations persist for a grace period after progress stops to handle
        // transient states
        const ANIMATION_GRACE_PERIOD: std::time::Duration = std::time::Duration::from_millis(500);

        // Update timestamps for currently progressing servers
        for (_server_name, server_id) in &lsp_servers {
          if cx.editor.lsp_progress.is_progressing(*server_id) {
            self
              .lsp_breathing_anims
              .entry(*server_id)
              .and_modify(|(_, last_seen)| *last_seen = now)
              .or_insert_with(|| (BreathingAnimation::new(), now));
          }
        }

        // Remove animations only after grace period or if server no longer in document
        let current_server_ids: std::collections::HashSet<_> =
          lsp_servers.iter().map(|(_, id)| *id).collect();
        self.lsp_breathing_anims.retain(|id, (_, last_seen)| {
          // Keep if: server is in document AND either still progressing or within grace
          // period
          current_server_ids.contains(id)
            && now.saturating_duration_since(*last_seen) < ANIMATION_GRACE_PERIOD
        });

        // Render LSP names
        let lsp_text = lsp_servers
          .iter()
          .map(|(name, _)| *name)
          .collect::<Vec<_>>()
          .join(",");
        let lsp_width = Self::measure_text(&lsp_text);
        right_x -= lsp_width;

        // Determine the color based on whether any server is loading
        let lsp_color = if lsp_servers
          .iter()
          .any(|(_, id)| self.lsp_breathing_anims.contains_key(id))
        {
          // At least one server is loading - apply breathing effect
          // Get the first loading server's animation for simplicity
          // (in practice, all servers will breathe together for visual consistency)
          let (anim, _) = lsp_servers
            .iter()
            .find_map(|(_, id)| self.lsp_breathing_anims.get(id))
            .unwrap();

          // Get theme color for loading LSP (with fallback to statusline color)
          let base_color = theme
            .get("ui.statusline.lsp.loading")
            .fg
            .or_else(|| theme.get("ui.statusline").fg)
            .map(crate::ui::theme_color_to_renderer_color)
            .unwrap_or(text_color);

          // Apply breathing animation to alpha only
          anim.apply_to_color(base_color, now)
        } else {
          // No servers loading - use normal color
          text_color
        };

        Self::draw_text(surface, right_x, bar_y, &lsp_text, lsp_color);
      }
    }); // End overlay region

    // Restore original font state
    surface.restore_font_state(saved_font);
  }

  fn should_update(&self) -> bool {
    // Keep updating while any animation is running
    self.anim_t < 1.0
      || self.slide_anim_t < 1.0
      || self.status_msg_anim_t < 1.0
      || !self.lsp_breathing_anims.is_empty() // Keep updating while LSP is loading
  }
}
