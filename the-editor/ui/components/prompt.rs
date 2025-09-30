use std::collections::VecDeque;

use the_editor_renderer::{
  Color,
  Key,
  KeyPress,
  Renderer,
  TextSection,
};

use crate::{
  core::{
    command_registry::CommandRegistry,
    commands::Context as CommandContext,
    graphics::{
      CursorKind,
      Rect,
    },
    position::Position,
  },
  editor::Editor,
  ui::{
    UI_FONT_SIZE,
    UI_FONT_WIDTH,
    compositor::{
      Component,
      Context,
      Event,
      EventResult,
      Surface,
    },
  },
};

/// Events that can occur in the prompt component
#[derive(Debug, Clone, PartialEq)]
pub enum PromptEvent {
  /// Validate and execute the current input
  Validate,
  /// Update the input (continue editing)
  Update,
  /// Abort/cancel the prompt
  Abort,
}

/// A prompt component for handling command input
#[derive(Debug)]
pub struct Prompt {
  /// Current input text
  input:              String,
  /// Cursor position within the input
  cursor:             usize,
  /// Command history (previous commands)
  history:            VecDeque<String>,
  /// Current position in history during navigation
  history_pos:        Option<usize>,
  /// Current completions based on input
  completions:        Vec<String>,
  /// Index of selected completion
  completion_index:   Option<usize>,
  /// Whether completion menu is visible
  show_completions:   bool,
  /// Maximum history size
  max_history:        usize,
  /// Prefix for the prompt (e.g., ":")
  prefix:             String,
  /// Horizontal scroll offset (in characters)
  scroll_offset:      usize,
  /// Cursor animation state
  cursor_pos_smooth:  Option<f32>,
  cursor_anim_active: bool,
  /// Border glow animation (0.0 = start, 1.0 = done)
  glow_anim_t:        f32,
}

impl Prompt {
  /// Create a new prompt with the given prefix
  pub fn new(prefix: String) -> Self {
    Self {
      input: String::new(),
      cursor: 0,
      history: VecDeque::new(),
      history_pos: None,
      completions: Vec::new(),
      completion_index: None,
      show_completions: false,
      max_history: 100,
      prefix,
      scroll_offset: 0,
      cursor_pos_smooth: None,
      cursor_anim_active: false,
      glow_anim_t: 0.0, // Start glow animation
    }
  }

  /// Handle a key press and return the appropriate event
  pub fn handle_key(&mut self, key: KeyPress, registry: &CommandRegistry) -> PromptEvent {
    if !key.pressed {
      return PromptEvent::Update;
    }

    match key.code {
      Key::Enter => {
        // Execute command
        if !self.input.trim().is_empty() {
          self.add_to_history(self.input.clone());
        }
        PromptEvent::Validate
      },
      Key::Escape => {
        // Cancel prompt
        PromptEvent::Abort
      },
      Key::Backspace => {
        self.delete_char_backward();
        self.update_completions(registry);
        PromptEvent::Update
      },
      Key::Delete => {
        self.delete_char_forward();
        self.update_completions(registry);
        PromptEvent::Update
      },
      Key::Left => {
        self.move_cursor_left();
        PromptEvent::Update
      },
      Key::Right => {
        self.move_cursor_right();
        PromptEvent::Update
      },
      Key::Up => {
        if self.show_completions {
          self.completion_previous();
        } else {
          self.history_previous();
        }
        PromptEvent::Update
      },
      Key::Down => {
        if self.show_completions {
          self.completion_next();
        } else {
          self.history_next();
        }
        PromptEvent::Update
      },
      Key::Tab => {
        if self.show_completions {
          self.complete_current();
        } else {
          self.update_completions(registry);
          self.show_completions = !self.completions.is_empty();
        }
        PromptEvent::Update
      },
      Key::Home => {
        self.cursor = 0;
        PromptEvent::Update
      },
      Key::End => {
        self.cursor = self.input.len();
        PromptEvent::Update
      },
      Key::Char(c) => {
        self.insert_char(c);
        self.update_completions(registry);
        PromptEvent::Update
      },
      _ => PromptEvent::Update,
    }
  }

  /// Get the current input text
  pub fn input(&self) -> &str {
    &self.input
  }

  /// Set the input text
  pub fn set_input(&mut self, input: String) {
    self.input = input;
    self.cursor = self.input.len();
    self.show_completions = false;
    self.completion_index = None;
  }

  /// Clear the prompt
  pub fn clear(&mut self) {
    self.input.clear();
    self.cursor = 0;
    self.show_completions = false;
    self.completion_index = None;
    self.history_pos = None;
    self.scroll_offset = 0;
  }

  /// Update scroll offset to keep cursor visible
  /// visible_width is the number of characters that can fit in the prompt box
  fn update_scroll(&mut self, visible_width: usize) {
    let cursor_pos = self.cursor;

    // If cursor is before visible area, scroll left
    if cursor_pos < self.scroll_offset {
      self.scroll_offset = cursor_pos;
    }
    // If cursor is beyond visible area, scroll right
    else if cursor_pos >= self.scroll_offset + visible_width {
      self.scroll_offset = cursor_pos.saturating_sub(visible_width - 1);
    }
  }

  /// Render the prompt to the screen (internal)
  fn render_prompt(&mut self, renderer: &mut Renderer, area: Rect, cx: &Context) {
    let base_x = area.x as f32;
    let base_y = area.y as f32;

    // Get statusline background color from theme
    let theme = &cx.editor.theme;
    let statusline_style = theme.get("ui.statusline");
    let bg_color = statusline_style
      .bg
      .map(crate::ui::theme_color_to_renderer_color)
      .unwrap_or(Color::new(0.12, 0.12, 0.13, 1.0));

    // Draw background bar matching statusline
    const STATUS_BAR_HEIGHT: f32 = 28.0;
    renderer.draw_rect(
      0.0,
      base_y,
      renderer.width() as f32,
      STATUS_BAR_HEIGHT,
      bg_color,
    );

    // Update glow animation
    const GLOW_SPEED: f32 = 0.025; // Slower for sweeping effect
    if self.glow_anim_t < 1.0 {
      self.glow_anim_t = (self.glow_anim_t + GLOW_SPEED).min(1.0);
    }

    // Draw bordered box around prompt area
    const PROMPT_WIDTH_PERCENT: f32 = 0.25; // 25% of viewport width
    let prompt_box_width = renderer.width() as f32 * PROMPT_WIDTH_PERCENT;
    let border_color = Color::new(0.3, 0.3, 0.35, 1.0);
    const BORDER_THICKNESS: f32 = 1.0;

    // Top border
    renderer.draw_rect(
      0.0,
      base_y,
      prompt_box_width,
      BORDER_THICKNESS,
      border_color,
    );

    // Bottom border
    renderer.draw_rect(
      0.0,
      base_y + STATUS_BAR_HEIGHT - BORDER_THICKNESS,
      prompt_box_width,
      BORDER_THICKNESS,
      border_color,
    );

    // Left border
    renderer.draw_rect(
      0.0,
      base_y,
      BORDER_THICKNESS,
      STATUS_BAR_HEIGHT,
      border_color,
    );

    // Right border (vertical line separating prompt from statusline)
    renderer.draw_rect(
      prompt_box_width - BORDER_THICKNESS,
      base_y,
      BORDER_THICKNESS,
      STATUS_BAR_HEIGHT,
      border_color,
    );

    // Draw sweeping glow effect that travels from left to right
    if self.glow_anim_t < 1.0 {
      const GLOW_WIDTH: f32 = 4.0; // Width perpendicular to border
      const GLOW_SWEEP_WIDTH: f32 = 80.0; // Width of the traveling glow spot

      // Calculate the position of the glow center as it travels left to right
      let glow_center_x = self.glow_anim_t * prompt_box_width;

      // Draw glow segments on top and bottom borders
      let segments = 50; // Number of segments to draw for smooth gradient
      for i in 0..segments {
        let segment_x = (i as f32 / segments as f32) * prompt_box_width;
        let segment_width = prompt_box_width / segments as f32;

        // Calculate distance from glow center
        let dist = (segment_x - glow_center_x).abs();

        // Calculate intensity based on distance (Gaussian-like falloff)
        let intensity = if dist < GLOW_SWEEP_WIDTH {
          1.0 - (dist / GLOW_SWEEP_WIDTH).powf(2.0)
        } else {
          0.0
        };

        if intensity > 0.01 {
          let glow_color = Color::new(
            0.3 + intensity * 0.5,
            0.8 * intensity,
            0.7 * intensity,
            intensity * 0.6,
          );

          // Top border glow
          renderer.draw_rect(
            segment_x,
            base_y - GLOW_WIDTH,
            segment_width,
            GLOW_WIDTH,
            glow_color,
          );

          // Bottom border glow
          renderer.draw_rect(
            segment_x,
            base_y + STATUS_BAR_HEIGHT,
            segment_width,
            GLOW_WIDTH,
            glow_color,
          );
        }
      }

      // Draw glow on vertical borders when sweep passes through them
      // Left border glow (fades in at start)
      if self.glow_anim_t < 0.15 {
        let intensity = (self.glow_anim_t / 0.15).min(1.0);
        let glow_color = Color::new(
          0.3 + intensity * 0.5,
          0.8 * intensity,
          0.7 * intensity,
          intensity * 0.6,
        );
        renderer.draw_rect(
          -GLOW_WIDTH,
          base_y,
          GLOW_WIDTH,
          STATUS_BAR_HEIGHT,
          glow_color,
        );
      }

      // Right border glow (fades in at end)
      if self.glow_anim_t > 0.85 {
        let intensity = ((self.glow_anim_t - 0.85) / 0.15).min(1.0);
        let glow_color = Color::new(
          0.3 + intensity * 0.5,
          0.8 * intensity,
          0.7 * intensity,
          intensity * 0.6,
        );
        renderer.draw_rect(
          prompt_box_width,
          base_y,
          GLOW_WIDTH,
          STATUS_BAR_HEIGHT,
          glow_color,
        );
      }
    }

    // Calculate text baseline (vertically centered in status bar)
    let text_y = base_y + (STATUS_BAR_HEIGHT - UI_FONT_SIZE) * 0.5;

    // Calculate visible width (how many characters fit in the prompt box)
    const PADDING: f32 = 12.0;
    let usable_width = prompt_box_width - (PADDING * 2.0) - BORDER_THICKNESS;
    let visible_chars = (usable_width / UI_FONT_WIDTH) as usize;

    // Account for prefix in visible width
    let prefix_len = self.prefix.chars().count();
    let visible_input_chars = visible_chars.saturating_sub(prefix_len);

    // Update scroll to keep cursor visible
    self.update_scroll(visible_input_chars);

    // Build the visible text (prefix + visible portion of input)
    let input_chars: Vec<char> = self.input.chars().collect();
    let visible_end = (self.scroll_offset + visible_input_chars).min(input_chars.len());
    let visible_input: String = input_chars[self.scroll_offset..visible_end]
      .iter()
      .collect();
    let full_text = format!("{}{}", self.prefix, visible_input);

    // Calculate cursor position relative to visible area
    let visible_cursor_col = if self.cursor >= self.scroll_offset {
      prefix_len + (self.cursor - self.scroll_offset)
    } else {
      prefix_len
    };

    // Get cursor color from theme
    let theme = &cx.editor.theme;
    let cursor_style = theme.get("ui.cursor");
    let cursor_bg = cursor_style
      .bg
      .map(crate::ui::theme_color_to_renderer_color)
      .unwrap_or(Color::rgb(0.2, 0.8, 0.7));

    // Only draw cursor if it's within visible area
    if self.cursor >= self.scroll_offset && self.cursor < self.scroll_offset + visible_input_chars {
      let target_x = base_x + PADDING + visible_cursor_col as f32 * UI_FONT_WIDTH;

      // Cursor animation: lerp toward target position
      let cursor_lerp_factor = cx.editor.config().cursor_lerp_factor;
      let cursor_anim_enabled = cx.editor.config().cursor_anim_enabled;

      let anim_x = if cursor_anim_enabled {
        let mut sx = self.cursor_pos_smooth.unwrap_or(target_x);
        let dx = target_x - sx;
        sx += dx * cursor_lerp_factor;
        self.cursor_pos_smooth = Some(sx);
        // Mark animation active if still far from target
        self.cursor_anim_active = dx.abs() > 0.5;
        sx
      } else {
        self.cursor_anim_active = false;
        self.cursor_pos_smooth = Some(target_x);
        target_x
      };

      // Draw cursor with same height extension as main editor
      const CURSOR_HEIGHT_EXTENSION: f32 = 4.0;
      renderer.draw_rect(
        anim_x,
        text_y,
        UI_FONT_WIDTH,
        UI_FONT_SIZE + CURSOR_HEIGHT_EXTENSION,
        cursor_bg,
      );
    }

    // Get cursor foreground color from theme
    let cursor_fg = cursor_style
      .fg
      .map(crate::ui::theme_color_to_renderer_color)
      .unwrap_or(Color::rgb(0.1, 0.1, 0.15));

    // Render the visible text character by character
    for (i, ch) in full_text.chars().enumerate() {
      let x = base_x + PADDING + i as f32 * UI_FONT_WIDTH;
      let color = if i == visible_cursor_col {
        cursor_fg // Use theme cursor foreground color
      } else {
        Color::WHITE
      };
      renderer.draw_text(TextSection::simple(
        x,
        text_y,
        ch.to_string(),
        UI_FONT_SIZE,
        color,
      ));
    }

    // Render completions if visible (above the prompt)
    if self.show_completions && !self.completions.is_empty() {
      self.render_completions_internal(renderer, base_y);
    }
  }

  /// Execute the current command
  pub fn execute(&self, cx: &mut CommandContext) -> anyhow::Result<()> {
    let input = self.input.trim();
    if input.is_empty() {
      return Ok(());
    }

    // Parse command and arguments
    let parts: Vec<&str> = input.split_whitespace().collect();
    let command = parts[0];
    let args = &parts[1..];

    // Clone the command registry to avoid borrowing issues
    let registry = cx.editor.command_registry.clone();

    // Execute through command registry
    registry.execute(cx, command, args)
  }

  // Private methods

  fn insert_char(&mut self, c: char) {
    self.input.insert(self.cursor, c);
    self.cursor += 1;
    self.show_completions = false;
    self.completion_index = None;
  }

  fn delete_char_backward(&mut self) {
    if self.cursor > 0 {
      self.cursor -= 1;
      self.input.remove(self.cursor);
      self.show_completions = false;
      self.completion_index = None;
    }
  }

  fn delete_char_forward(&mut self) {
    if self.cursor < self.input.len() {
      self.input.remove(self.cursor);
      self.show_completions = false;
      self.completion_index = None;
    }
  }

  fn move_cursor_left(&mut self) {
    if self.cursor > 0 {
      self.cursor -= 1;
    }
  }

  fn move_cursor_right(&mut self) {
    if self.cursor < self.input.len() {
      self.cursor += 1;
    }
  }

  fn update_completions(&mut self, registry: &CommandRegistry) {
    let input = self.input.trim();
    if input.is_empty() {
      self.completions.clear();
      self.show_completions = false;
      self.completion_index = None;
      return;
    }

    // Get command name (first word)
    let command_prefix = input.split_whitespace().next().unwrap_or("");

    if !command_prefix.is_empty() {
      self.completions = registry
        .completions(command_prefix)
        .into_iter()
        .map(|s| s.to_string())
        .collect();
    } else {
      self.completions.clear();
    }

    self.completion_index = if self.completions.is_empty() {
      None
    } else {
      Some(0)
    };
  }

  fn completion_next(&mut self) {
    if let Some(index) = self.completion_index
      && !self.completions.is_empty()
    {
      self.completion_index = Some((index + 1) % self.completions.len());
    }
  }

  fn completion_previous(&mut self) {
    if let Some(index) = self.completion_index
      && !self.completions.is_empty()
    {
      self.completion_index = Some(if index == 0 {
        self.completions.len() - 1
      } else {
        index - 1
      });
    }
  }

  fn complete_current(&mut self) {
    if let Some(index) = self.completion_index
      && let Some(completion) = self.completions.get(index)
    {
      // Replace the current command with the completion
      let parts: Vec<&str> = self.input.split_whitespace().collect();
      if !parts.is_empty() {
        let mut new_input = completion.clone();
        if parts.len() > 1 {
          new_input.push(' ');
          new_input.push_str(&parts[1..].join(" "));
        }
        self.input = new_input;
        self.cursor = self.input.len();
        self.show_completions = false;
        self.completion_index = None;
        self.scroll_offset = 0; // Reset scroll after completion
      }
    }
  }

  fn add_to_history(&mut self, command: String) {
    // Don't add duplicate consecutive entries
    if self.history.back() == Some(&command) {
      return;
    }

    self.history.push_back(command);

    // Limit history size
    while self.history.len() > self.max_history {
      self.history.pop_front();
    }

    self.history_pos = None;
  }

  fn history_previous(&mut self) {
    if self.history.is_empty() {
      return;
    }

    let new_pos = match self.history_pos {
      None => self.history.len() - 1,
      Some(pos) => {
        if pos > 0 {
          pos - 1
        } else {
          return; // Already at oldest
        }
      },
    };

    self.history_pos = Some(new_pos);
    if let Some(cmd) = self.history.get(new_pos) {
      self.input = cmd.clone();
      self.cursor = self.input.len();
      self.scroll_offset = 0; // Reset scroll when changing history
    }
  }

  fn history_next(&mut self) {
    match self.history_pos {
      None => (), // Not in history mode
      Some(pos) => {
        if pos + 1 < self.history.len() {
          let new_pos = pos + 1;
          self.history_pos = Some(new_pos);
          if let Some(cmd) = self.history.get(new_pos) {
            self.input = cmd.clone();
            self.cursor = self.input.len();
            self.scroll_offset = 0; // Reset scroll when changing history
          }
        } else {
          // Return to current input
          self.history_pos = None;
          self.input.clear();
          self.cursor = 0;
          self.scroll_offset = 0;
        }
      },
    }
  }

  fn render_completions_internal(&self, renderer: &mut Renderer, prompt_y: f32) {
    if self.completions.is_empty() {
      return;
    }

    const STATUS_BAR_HEIGHT: f32 = 28.0;
    const COMPLETION_ITEM_HEIGHT: f32 = 20.0;

    // Draw completions above the prompt
    let completion_height = (self.completions.len() as f32 * COMPLETION_ITEM_HEIGHT).min(200.0);
    let completion_y = prompt_y - completion_height;

    // Draw background for completions
    let bg_color = Color::new(0.15, 0.15, 0.16, 0.95);
    renderer.draw_rect(12.0, completion_y, 300.0, completion_height, bg_color);

    // Draw completions
    for (i, completion) in self.completions.iter().take(10).enumerate() {
      let y = completion_y + i as f32 * COMPLETION_ITEM_HEIGHT;
      let is_selected = self.completion_index == Some(i);

      let color = if is_selected {
        Color::rgb(0.8, 0.8, 1.0) // Lighter blue for selected
      } else {
        Color::rgb(0.7, 0.7, 0.7) // Gray for normal
      };

      renderer.draw_text(TextSection::simple(
        17.0, // 12.0 + 5.0 padding
        y + 2.0,
        completion,
        UI_FONT_SIZE - 2.0, // Slightly smaller for completions
        color,
      ));
    }
  }
}

impl Default for Prompt {
  fn default() -> Self {
    Self::new(String::new())
  }
}

impl Component for Prompt {
  fn handle_event(&mut self, event: &Event, cx: &mut Context) -> EventResult {
    let Event::Key(key_binding) = event else {
      return EventResult::Ignored(None);
    };

    // Convert KeyBinding to KeyPress for handle_key
    let key_press = KeyPress {
      code:    key_binding.code,
      shift:   key_binding.shift,
      ctrl:    key_binding.ctrl,
      alt:     key_binding.alt,
      pressed: true,
    };

    let registry = cx.editor.command_registry.clone();
    let prompt_event = self.handle_key(key_press, &registry);

    match prompt_event {
      PromptEvent::Abort => {
        // Close the prompt and slide statusline back
        EventResult::Consumed(Some(Box::new(|compositor, cx| {
          // Switch back to Normal mode
          cx.editor.set_mode(crate::keymap::Mode::Normal);

          // Find statusline and slide back
          for layer in compositor.layers.iter_mut() {
            if let Some(statusline) = layer
              .as_any_mut()
              .downcast_mut::<crate::ui::components::statusline::StatusLine>(
            ) {
              statusline.slide_for_prompt(false);
              break;
            }
          }
          compositor.pop();
        })))
      },
      PromptEvent::Validate => {
        // Execute command before creating the closure
        let trimmed = self.input.trim().to_string();
        if !trimmed.is_empty() {
          let parts: Vec<&str> = trimmed.split_whitespace().collect();
          let command = parts[0].to_string();
          let args: Vec<String> = parts[1..].iter().map(|s| s.to_string()).collect();

          // Store command info for execution in the callback
          // We'll execute it in the callback to avoid lifetime issues
          EventResult::Consumed(Some(Box::new(move |_compositor, cx| {
            // Switch back to Normal mode first
            cx.editor.set_mode(crate::keymap::Mode::Normal);

            // Clone the registry before creating CommandContext
            let registry = cx.editor.command_registry.clone();

            // Create CommandContext and execute command inside the callback
            let mut cmd_cx = CommandContext {
              editor:               cx.editor,
              count:                None,
              register:             None,
              callback:             Vec::new(),
              on_next_key_callback: None,
              jobs:                 cx.jobs,
            };

            let args_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
            let result = registry.execute(&mut cmd_cx, &command, &args_refs);

            // Process any callbacks from the command first
            let callbacks = cmd_cx.callback;

            // Now we can safely use cx.editor again after cmd_cx is dropped
            if let Err(err) = result {
              cx.editor.set_error(err.to_string());
            }

            // Execute callbacks after cmd_cx is dropped
            for callback in callbacks {
              callback(_compositor, cx);
            }

            // Find statusline and slide back before closing
            for layer in _compositor.layers.iter_mut() {
              if let Some(statusline) = layer
                .as_any_mut()
                .downcast_mut::<crate::ui::components::statusline::StatusLine>(
              ) {
                statusline.slide_for_prompt(false);
                break;
              }
            }

            // Close the prompt after executing
            _compositor.pop();
          })))
        } else {
          // Just close if input is empty
          EventResult::Consumed(Some(Box::new(|compositor, cx| {
            // Switch back to Normal mode
            cx.editor.set_mode(crate::keymap::Mode::Normal);

            // Find statusline and slide back
            for layer in compositor.layers.iter_mut() {
              if let Some(statusline) = layer
                .as_any_mut()
                .downcast_mut::<crate::ui::components::statusline::StatusLine>(
              ) {
                statusline.slide_for_prompt(false);
                break;
              }
            }
            compositor.pop();
          })))
        }
      },
      PromptEvent::Update => EventResult::Consumed(None),
    }
  }

  fn render(&mut self, _area: Rect, surface: &mut Surface, cx: &mut Context) {
    // Render at the very bottom of the screen (where statusline normally is)
    // The prompt will cover the statusline since it's rendered on top
    const STATUS_BAR_HEIGHT: f32 = 28.0;
    let prompt_y = surface.height() as f32 - STATUS_BAR_HEIGHT;

    let prompt_area = Rect {
      x:      0,
      y:      prompt_y as u16,
      width:  surface.width() as u16,
      height: STATUS_BAR_HEIGHT as u16,
    };

    self.render_prompt(surface, prompt_area, cx);
  }

  fn cursor(&self, _area: Rect, _editor: &Editor) -> (Option<Position>, CursorKind) {
    // Calculate cursor position at the bottom of the screen
    // We don't actually need to return a cursor position since we draw the cursor
    // ourselves in render_prompt, but we return None to indicate no hardware
    // cursor needed
    (None, CursorKind::Block)
  }

  fn should_update(&self) -> bool {
    // Always update for input responsiveness
    true
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::core::command_registry::CommandRegistry;

  #[test]
  fn test_prompt_creation() {
    let prompt = Prompt::new(String::new());
    assert_eq!(prompt.input(), "");
    assert_eq!(prompt.cursor, 0);
  }

  #[test]
  fn test_prompt_input() {
    let mut prompt = Prompt::new(String::new());

    prompt.insert_char('q');
    assert_eq!(prompt.input(), "q");
    assert_eq!(prompt.cursor, 1);

    prompt.insert_char('u');
    prompt.insert_char('i');
    prompt.insert_char('t');
    assert_eq!(prompt.input(), "quit");
    assert_eq!(prompt.cursor, 4);
  }

  #[test]
  fn test_prompt_backspace() {
    let mut prompt = Prompt::new(String::new());
    prompt.set_input("quit".to_string());

    prompt.delete_char_backward();
    assert_eq!(prompt.input(), "qui");
    assert_eq!(prompt.cursor, 3);
  }

  #[test]
  fn test_prompt_completions() {
    let mut prompt = Prompt::new(String::new());
    let registry = CommandRegistry::new();

    prompt.set_input("q".to_string());
    prompt.update_completions(&registry);

    assert!(!prompt.completions.is_empty());
    assert!(prompt.completions.contains(&"quit".to_string()));
  }
}
