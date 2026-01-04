use std::{
  collections::VecDeque,
  ops::RangeFrom,
  sync::Arc,
};

use the_editor_renderer::{
  Color,
  Key,
  KeyPress,
  TextSection,
  TextSegment,
  TextStyle,
};
use unicode_segmentation::{
  GraphemeCursor,
  UnicodeSegmentation,
};

use crate::{
  core::{
    command_line,
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
    components::button::Button,
    compositor::{
      Component,
      Context,
      Event,
      EventResult,
      Surface,
    },
  },
};

/// Bounds of the completion box for positioning the help box
struct CompletionBounds {
  x:      f32,
  y:      f32,
  width:  f32,
  height: f32,
}

/// A completion item with range information
/// The range specifies which part of the input should be replaced
#[derive(Debug, Clone, PartialEq)]
pub struct Completion {
  /// Range in the input to replace (from position onwards)
  pub range: RangeFrom<usize>,
  /// The completion text
  pub text:  String,
  /// Optional documentation/description
  pub doc:   Option<String>,
}

/// Function type for generating completions
/// Takes the editor and current input, returns list of completions
pub type CompletionFn = Arc<dyn Fn(&Editor, &str) -> Vec<Completion> + Send + Sync>;

/// Create a completion function that generates completions from a register's
/// history. This is used for prompts like search, shell commands, etc. to show
/// previous entries.
pub fn history_completion(register: char) -> CompletionFn {
  Arc::new(move |editor: &Editor, input: &str| -> Vec<Completion> {
    let Some(values) = editor.registers.read(register, editor) else {
      return Vec::new();
    };

    let input_lower = input.to_lowercase();
    let mut seen = std::collections::HashSet::new();

    // Filter, deduplicate, and map history entries to completions
    values
      .filter(|value| {
        // Match if input is empty or if value contains the input (case-insensitive)
        input.is_empty() || value.to_lowercase().contains(&input_lower)
      })
      .filter(|value| {
        // Deduplicate entries
        seen.insert(value.to_string())
      })
      .map(|value| {
        Completion {
          range: 0..,
          text:  value.to_string(),
          doc:   None,
        }
      })
      .collect()
  })
}

/// Function type for handling prompt events (validate, update, abort)
/// Takes context, current input, and event type
/// This allows custom behavior for different prompt types (rename, search,
/// etc.)
pub type CallbackFn = Box<dyn FnMut(&mut Context, &str, PromptEvent)>;

/// Events that can occur in the prompt component
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptEvent {
  /// Validate and execute the current input
  Validate,
  /// Update the input (continue editing)
  Update,
  /// Abort/cancel the prompt
  Abort,
}

/// Function type for generating documentation from input
pub type DocFn = Box<dyn Fn(&str) -> Option<String> + Send>;

/// A prompt component for handling command input
pub struct Prompt {
  /// Current input text
  input:                  String,
  /// Cursor position within the input
  cursor:                 usize,
  /// Command history (previous commands)
  history:                VecDeque<String>,
  /// Current position in history during navigation
  history_pos:            Option<usize>,
  /// Current completions based on input
  completions:            Vec<Completion>,
  /// Index of selected completion
  selection:              Option<usize>,
  /// Completion function to generate completions
  completion_fn:          Option<CompletionFn>,
  /// Callback function for handling events (validate, update, abort)
  /// If None, falls back to command execution
  callback_fn:            Option<CallbackFn>,
  /// Documentation function to generate help text from input
  doc_fn:                 Option<DocFn>,
  /// Maximum history size
  max_history:            usize,
  /// Prefix for the prompt (e.g., ":")
  prefix:                 String,
  /// Horizontal scroll offset (in characters)
  scroll_offset:          usize,
  /// Cursor animation state
  cursor_pos_smooth:      Option<f32>,
  cursor_anim_active:     bool,
  /// Border glow animation (0.0 = start, 1.0 = done)
  glow_anim_t:            f32,
  /// Completion scroll offset (for scrolling through many completions)
  completion_scroll:      usize,
  /// Selection animation (0.0 = start, 1.0 = done)
  selection_anim:         f32,
  /// Last selected index (to detect selection changes)
  last_selection:         Option<usize>,
  /// Completion list animation (0.0 = hidden, 1.0 = shown) - exponential decay
  completion_list_anim_t: f32,
  /// Prompt opening animation (0.0 = closed, 1.0 = open) - exponential decay
  prompt_open_t:          f32,
  /// Help box animation (0.0 = hidden, 1.0 = shown) - exponential decay
  help_box_anim_t:        f32,
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
      selection: None,
      completion_fn: None,
      callback_fn: None,
      doc_fn: None,
      max_history: 100,
      prefix,
      scroll_offset: 0,
      cursor_pos_smooth: None,
      cursor_anim_active: false,
      glow_anim_t: 0.0,
      completion_scroll: 0,
      selection_anim: 1.0,
      last_selection: None,
      completion_list_anim_t: 0.0,
      prompt_open_t: 0.0,
      help_box_anim_t: 0.0,
    }
  }

  /// Set the completion function for this prompt
  pub fn with_completion(mut self, completion_fn: CompletionFn) -> Self {
    self.completion_fn = Some(completion_fn);
    self
  }

  /// Set the completion function (builder pattern alternative)
  pub fn set_completion_fn(&mut self, completion_fn: CompletionFn) {
    self.completion_fn = Some(completion_fn);
  }

  /// Set a custom callback function for handling prompt events
  /// This allows the prompt to be used for custom interactions like rename,
  /// search, etc. If no callback is set, the prompt falls back to command
  /// execution
  pub fn with_callback(
    mut self,
    callback: impl FnMut(&mut Context, &str, PromptEvent) + 'static,
  ) -> Self {
    self.callback_fn = Some(Box::new(callback));
    self
  }

  /// Pre-fill the prompt with initial text (useful for rename, edit, etc.)
  pub fn with_prefill(mut self, text: String) -> Self {
    self.cursor = text.len();
    self.input = text;
    self
  }

  /// Set the documentation function for generating help text
  pub fn with_doc_fn(mut self, doc_fn: impl Fn(&str) -> Option<String> + Send + 'static) -> Self {
    self.doc_fn = Some(Box::new(doc_fn));
    self
  }

  /// Handle a key press and return the appropriate event
  /// Note: This is called from handle_event which has access to editor
  fn handle_key_internal(&mut self, key: KeyPress, editor: &Editor) -> PromptEvent {
    if !key.pressed {
      return PromptEvent::Update;
    }

    // Emacs-style keybindings (like Helix)
    match (key.code, key.ctrl, key.alt, key.shift) {
      // Enter - insert selected completion or execute
      (Key::Enter | Key::NumpadEnter, ..) => {
        // If we have a completion selected, insert it
        if self.selection.is_some() && !self.completions.is_empty() {
          // Apply the selected completion
          if let Some(index) = self.selection {
            self.apply_completion(index);

            // If we completed a directory path, recalculate for progressive navigation
            if self.should_recalculate_after_completion() {
              self.recalculate_completions(editor);
            }

            // Clear selection after applying
            self.selection = None;

            PromptEvent::Update
          } else {
            // Fallback to validation if something went wrong
            if !self.input.trim().is_empty() {
              self.add_to_history(self.input.clone());
            }
            PromptEvent::Validate
          }
        } else {
          // No completion selected - execute the command
          if !self.input.trim().is_empty() {
            self.add_to_history(self.input.clone());
          }
          PromptEvent::Validate
        }
      },
      // Escape / Ctrl+c - abort
      (Key::Escape, ..) | (Key::Char('c'), true, ..) => {
        self.exit_selection();
        PromptEvent::Abort
      },
      // Ctrl+b / Left - backward char
      (Key::Char('b'), true, ..) | (Key::Left, false, false, false) => {
        self.move_cursor_left();
        PromptEvent::Update
      },
      // Ctrl+f / Right - forward char
      (Key::Char('f'), true, ..) | (Key::Right, false, false, false) => {
        self.move_cursor_right();
        PromptEvent::Update
      },
      // Alt+b / Ctrl+Left - backward word
      (Key::Char('b'), _, true, _) | (Key::Left, true, false, _) => {
        self.move_word_backward();
        PromptEvent::Update
      },
      // Alt+f / Ctrl+Right - forward word
      (Key::Char('f'), _, true, _) | (Key::Right, true, false, _) => {
        self.move_word_forward();
        PromptEvent::Update
      },
      // Ctrl+a / Home - start of line
      (Key::Char('a'), true, ..) | (Key::Home, ..) => {
        self.cursor = 0;
        PromptEvent::Update
      },
      // Ctrl+e / End - end of line
      (Key::Char('e'), true, ..) | (Key::End, ..) => {
        self.cursor = self.input.len();
        PromptEvent::Update
      },
      // Ctrl+h / Backspace - delete char backward
      (Key::Char('h'), true, ..) | (Key::Backspace, false, false, _) => {
        self.delete_char_backward();
        self.recalculate_completions(editor);
        PromptEvent::Update
      },
      // Ctrl+d / Delete - delete char forward
      (Key::Char('d'), true, ..) | (Key::Delete, false, false, false) => {
        self.delete_char_forward();
        self.recalculate_completions(editor);
        PromptEvent::Update
      },
      // Ctrl+w / Alt+Backspace / Ctrl+Backspace - delete word backward
      (Key::Char('w'), true, ..) | (Key::Backspace, _, true, _) | (Key::Backspace, true, ..) => {
        self.delete_word_backward();
        self.recalculate_completions(editor);
        PromptEvent::Update
      },
      // Alt+d / Alt+Delete / Ctrl+Delete - delete word forward
      (Key::Char('d'), _, true, _) | (Key::Delete, _, true, _) | (Key::Delete, true, ..) => {
        self.delete_word_forward();
        self.recalculate_completions(editor);
        PromptEvent::Update
      },
      // Ctrl+k - kill to end of line
      (Key::Char('k'), true, ..) => {
        self.kill_to_end();
        self.recalculate_completions(editor);
        PromptEvent::Update
      },
      // Ctrl+u - kill to start of line
      (Key::Char('u'), true, ..) => {
        self.kill_to_start();
        self.recalculate_completions(editor);
        PromptEvent::Update
      },
      // // Up - history previous
      // (Key::Up, false, false, false) => {
      //   self.history_previous();
      //   PromptEvent::Update
      // },
      // // Down - history next
      // (Key::Down, false, false, false) => {
      //   self.history_next();
      //   PromptEvent::Update
      // },
      // Ctrl+n - next completion
      (Key::Char('n'), true, false, false) | (Key::Down, false, false, false) => {
        self.change_completion_selection_forward(editor);
        PromptEvent::Update
      },
      // Ctrl+p - previous completion
      (Key::Char('p'), true, false, false) | (Key::Up, false, false, false) => {
        self.change_completion_selection_backward(editor);
        PromptEvent::Update
      },
      // Ctrl+q - exit selection
      (Key::Char('q'), true, ..) => {
        self.exit_selection();
        PromptEvent::Update
      },
      // Tab - next completion and insert
      (Key::Tab, false, false, false) => {
        self.change_completion_selection_forward_and_insert(editor);
        PromptEvent::Update
      },
      // Shift+Tab - previous completion and insert
      (Key::Tab, false, false, true) => {
        self.change_completion_selection_backward_and_insert(editor);
        PromptEvent::Update
      },
      // Regular character input
      (Key::Char(c), false, false, _) => {
        self.insert_char(c);
        self.recalculate_completions(editor);
        PromptEvent::Update
      },
      _ => PromptEvent::Update,
    }
  }

  /// Get the current input text
  pub fn input(&self) -> &str {
    &self.input
  }

  /// Get the prompt prefix (e.g., "/" or ":")
  pub fn prefix(&self) -> &str {
    &self.prefix
  }

  /// Set the input text
  pub fn set_input(&mut self, input: String) {
    self.input = input;
    self.cursor = self.input.len();
    self.selection = None;
  }

  /// Clear the prompt
  pub fn clear(&mut self) {
    self.input.clear();
    self.cursor = 0;
    self.completions.clear();
    self.selection = None;
    self.history_pos = None;
    self.scroll_offset = 0;
    self.completion_scroll = 0;
    self.selection_anim = 1.0;
    self.last_selection = None;
    self.completion_list_anim_t = 0.0;
    self.prompt_open_t = 0.0;
    self.help_box_anim_t = 0.0;
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
  fn render_prompt(&mut self, surface: &mut Surface, area: Rect, cx: &Context) {
    let base_x = area.x as f32;
    let base_y = area.y as f32;

    // Save current font state before configuring for UI rendering
    let saved_font = surface.save_font_state();

    // Configure UI font to get accurate metrics
    let ui_font_family = surface.current_font_family().to_owned();
    surface.configure_font(&ui_font_family, UI_FONT_SIZE);
    let cell_width = surface.cell_width();

    // Get statusline background color from theme
    let theme = &cx.editor.theme;
    let statusline_style = theme.get("ui.statusline");
    let bg_color = statusline_style
      .bg
      .map(crate::ui::theme_color_to_renderer_color)
      .unwrap_or(Color::new(0.12, 0.12, 0.13, 1.0));

    // Draw background bar matching statusline
    const STATUS_BAR_HEIGHT: f32 = 28.0;
    let viewport_width = surface.width() as f32;
    surface.draw_rect(0.0, base_y, viewport_width, STATUS_BAR_HEIGHT, bg_color);

    // Exponential decay rates (like bufferline tabs)
    let fast_rate = 1.0 - 2.0_f32.powf(-45.0 * cx.dt); // Fast animations
    let medium_rate = 1.0 - 2.0_f32.powf(-30.0 * cx.dt); // Medium animations

    // Update prompt opening animation (exponential decay)
    self.prompt_open_t += (1.0 - self.prompt_open_t) * fast_rate;

    // Update glow animation (keep linear for sweeping effect)
    const GLOW_SPEED: f32 = 0.025; // Base speed for sweeping effect
    if self.glow_anim_t < 1.0 {
      let speed = GLOW_SPEED * 300.0; // Faster but still visible
      self.glow_anim_t = (self.glow_anim_t + speed * cx.dt).min(1.0);
    }

    // Update selection animation (exponential decay)
    self.selection_anim += (1.0 - self.selection_anim) * fast_rate;

    // Update completion list animation (exponential decay)
    let completion_target = if !self.completions.is_empty() {
      1.0
    } else {
      0.0
    };
    self.completion_list_anim_t += (completion_target - self.completion_list_anim_t) * medium_rate;

    // Update help box animation (exponential decay)
    let help_target = if self.selection.is_some() && self.doc_fn.is_some() {
      1.0
    } else {
      0.0
    };
    self.help_box_anim_t += (help_target - self.help_box_anim_t) * medium_rate;

    // Draw bordered box around prompt area with opening animation
    const PROMPT_WIDTH_PERCENT: f32 = 0.25; // 25% of viewport width
    let target_width = viewport_width * PROMPT_WIDTH_PERCENT;
    // Apply exponential decay animation to width
    let prompt_box_width = target_width * self.prompt_open_t;
    let prompt_alpha = self.prompt_open_t;
    let border_color = Color::new(0.3, 0.3, 0.35, prompt_alpha);
    const BORDER_THICKNESS: f32 = 1.0;

    // Calculate completion area for stencil mask (updated to match new rendering)
    const COMPLETION_ITEM_HEIGHT: f32 = 24.0;
    const ITEM_GAP: f32 = 2.0;
    const MAX_VISIBLE: usize = 10;
    const COMPLETION_PADDING: f32 = 12.0;
    // Completion box scales with prompt box (slightly inset)
    let completion_x = COMPLETION_PADDING;
    let completion_width = prompt_box_width - COMPLETION_PADDING * 2.0;
    let completion_height = if !self.completions.is_empty() {
      let offset = self.completion_scroll;
      let visible_count = (self.completions.len() - offset).min(MAX_VISIBLE);
      visible_count as f32 * (COMPLETION_ITEM_HEIGHT + ITEM_GAP) - ITEM_GAP + 5.0 // Include 5px gap
    } else {
      0.0
    };

    // Render prompt content in overlay mode with automatic masking
    // Only mask the prompt box area (not the full width) to avoid blanking the text
    // buffer
    let mask_width = prompt_box_width;
    let mask_height = STATUS_BAR_HEIGHT + completion_height;
    let mask_y = base_y - completion_height;

    surface.with_overlay_region(0.0, mask_y, mask_width, mask_height, |surface| {
      // Top border
      surface.draw_rect(
        0.0,
        base_y,
        prompt_box_width,
        BORDER_THICKNESS,
        border_color,
      );

      // Bottom border
      surface.draw_rect(
        0.0,
        base_y + STATUS_BAR_HEIGHT - BORDER_THICKNESS,
        prompt_box_width,
        BORDER_THICKNESS,
        border_color,
      );

      // Left border
      surface.draw_rect(
        0.0,
        base_y,
        BORDER_THICKNESS,
        STATUS_BAR_HEIGHT,
        border_color,
      );

      // Right border (vertical line separating prompt from statusline)
      surface.draw_rect(
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
            surface.draw_rect(
              segment_x,
              base_y - GLOW_WIDTH,
              segment_width,
              GLOW_WIDTH,
              glow_color,
            );

            // Bottom border glow
            surface.draw_rect(
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
          surface.draw_rect(
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
          surface.draw_rect(
            prompt_box_width,
            base_y,
            GLOW_WIDTH,
            STATUS_BAR_HEIGHT,
            glow_color,
          );
        }
      }

      // Calculate text baseline with fixed padding (like picker)
      let text_y = base_y + 8.0;

      // Calculate visible width (how many characters fit in the prompt box)
      const PADDING: f32 = 12.0;
      let usable_width = prompt_box_width - (PADDING * 2.0) - BORDER_THICKNESS;
      let visible_chars = (usable_width / cell_width) as usize;

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

      // Calculate cursor position relative to visible area (convert byte position to
      // grapheme index)
      let query_grapheme_count = if self.cursor >= self.scroll_offset {
        self.input[self.scroll_offset..self.cursor.min(self.input.len())]
          .graphemes(true)
          .count()
      } else {
        0
      };
      let visible_cursor_col = prefix_len + query_grapheme_count;

      // Get cursor color from theme
      let theme = &cx.editor.theme;
      let cursor_style = theme.get("ui.cursor");
      let cursor_bg = cursor_style
        .bg
        .map(crate::ui::theme_color_to_renderer_color)
        .unwrap_or(Color::rgb(0.2, 0.8, 0.7));

      // Only draw cursor if it's within visible area
      if self.cursor >= self.scroll_offset && self.cursor < self.scroll_offset + visible_input_chars
      {
        let target_x = base_x + PADDING + visible_cursor_col as f32 * cell_width;

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
        surface.draw_rect(
          anim_x,
          text_y,
          cell_width,
          UI_FONT_SIZE + CURSOR_HEIGHT_EXTENSION,
          cursor_bg,
        );
      }

      // Get cursor foreground color from theme
      let cursor_fg = cursor_style
        .fg
        .map(crate::ui::theme_color_to_renderer_color)
        .unwrap_or(Color::rgb(0.1, 0.1, 0.15));

      // Render the full text in one call to avoid positioning issues
      let text_x = base_x + PADDING;
      surface.draw_text(TextSection::simple(
        text_x,
        text_y,
        &full_text,
        UI_FONT_SIZE,
        Color::WHITE,
      ));

      // If cursor is within visible text, render the cursor grapheme again on top
      // with cursor color. This ensures the text layout is consistent while
      // still showing the cursor-colored grapheme
      let graphemes: Vec<&str> = full_text.graphemes(true).collect();
      if visible_cursor_col < graphemes.len() {
        let cursor_grapheme = graphemes[visible_cursor_col];
        let cursor_x = text_x + visible_cursor_col as f32 * cell_width;
        surface.draw_text(TextSection::simple(
          cursor_x,
          text_y,
          cursor_grapheme,
          UI_FONT_SIZE,
          cursor_fg,
        ));
      }

      // Render completions if visible (above the prompt)
      let completion_bounds = if !self.completions.is_empty() {
        self.render_completions_internal(
          surface,
          base_y,
          completion_x,
          completion_width,
          cell_width,
          cx,
        )
      } else {
        None
      };

      // Render help box to the right of completions
      if let Some(bounds) = completion_bounds {
        let screen_width = surface.width() as f32;
        self.render_help_box(surface, &bounds, screen_width, cx);
      }
    }); // End overlay region

    // Restore original font state
    surface.restore_font_state(saved_font);
  }

  /// Execute the current command
  pub fn execute(&self, cx: &mut CommandContext) -> anyhow::Result<()> {
    let input = self.input.trim();
    if input.is_empty() {
      return Ok(());
    }

    // Split command from arguments
    use crate::core::command_line;
    let (command, args_line, _) = command_line::split(input);

    // Clone the command registry to avoid borrowing issues
    let registry = cx.editor.command_registry.clone();

    // Execute through command registry with PromptEvent::Validate
    registry.execute(cx, command, args_line, PromptEvent::Validate)
  }

  // Private methods

  fn insert_char(&mut self, c: char) {
    self.input.insert(self.cursor, c);
    self.cursor += c.len_utf8();
  }

  fn delete_char_backward(&mut self) {
    if self.cursor > 0 {
      let prev_pos = Self::prev_grapheme_boundary(&self.input, self.cursor);
      self.input.drain(prev_pos..self.cursor).for_each(drop);
      self.cursor = prev_pos;
    }
  }

  fn delete_char_forward(&mut self) {
    if self.cursor < self.input.len() {
      let next_pos = Self::next_grapheme_boundary(&self.input, self.cursor);
      self.input.drain(self.cursor..next_pos).for_each(drop);
    }
  }

  fn move_cursor_left(&mut self) {
    self.cursor = Self::prev_grapheme_boundary(&self.input, self.cursor);
  }

  fn move_cursor_right(&mut self) {
    self.cursor = Self::next_grapheme_boundary(&self.input, self.cursor);
  }

  fn is_word_boundary(c: char) -> bool {
    c.is_whitespace() || c == '/' || c == '-' || c == '_'
  }

  fn move_word_backward(&mut self) {
    if self.cursor == 0 {
      return;
    }

    // Move back one grapheme first
    let mut pos = Self::prev_grapheme_boundary(&self.input, self.cursor);

    // Skip whitespace backward
    loop {
      if pos == 0 {
        break;
      }
      let prev_pos = Self::prev_grapheme_boundary(&self.input, pos);
      let grapheme = &self.input[prev_pos..pos];
      let ch = grapheme.chars().next().unwrap_or(' ');
      if !Self::is_word_boundary(ch) {
        break;
      }
      pos = prev_pos;
    }

    // Move to start of word backward
    loop {
      if pos == 0 {
        break;
      }
      let prev_pos = Self::prev_grapheme_boundary(&self.input, pos);
      let grapheme = &self.input[prev_pos..pos];
      let ch = grapheme.chars().next().unwrap_or(' ');
      if Self::is_word_boundary(ch) {
        break;
      }
      pos = prev_pos;
    }

    self.cursor = pos;
  }

  fn move_word_forward(&mut self) {
    if self.cursor >= self.input.len() {
      return;
    }

    let mut pos = self.cursor;

    // Skip current word
    loop {
      if pos >= self.input.len() {
        break;
      }
      let next_pos = Self::next_grapheme_boundary(&self.input, pos);
      let grapheme = &self.input[pos..next_pos];
      let ch = grapheme.chars().next().unwrap_or(' ');
      if Self::is_word_boundary(ch) {
        break;
      }
      pos = next_pos;
    }

    // Skip whitespace
    loop {
      if pos >= self.input.len() {
        break;
      }
      let next_pos = Self::next_grapheme_boundary(&self.input, pos);
      let grapheme = &self.input[pos..next_pos];
      let ch = grapheme.chars().next().unwrap_or(' ');
      if !Self::is_word_boundary(ch) {
        break;
      }
      pos = next_pos;
    }

    self.cursor = pos;
  }

  fn delete_word_backward(&mut self) {
    if self.cursor == 0 {
      return;
    }

    let old_cursor = self.cursor;
    self.move_word_backward();
    self.input.replace_range(self.cursor..old_cursor, "");
  }

  fn delete_word_forward(&mut self) {
    if self.cursor >= self.input.len() {
      return;
    }

    let old_cursor = self.cursor;
    self.move_word_forward();
    self.input.replace_range(old_cursor..self.cursor, "");
    self.cursor = old_cursor;
  }

  fn kill_to_end(&mut self) {
    self.input.truncate(self.cursor);
  }

  fn kill_to_start(&mut self) {
    self.input.replace_range(..self.cursor, "");
    self.cursor = 0;
  }

  fn prev_grapheme_boundary(text: &str, byte_idx: usize) -> usize {
    let mut gc = GraphemeCursor::new(byte_idx, text.len(), true);
    gc.prev_boundary(text, 0).unwrap_or(Some(0)).unwrap_or(0)
  }

  fn next_grapheme_boundary(text: &str, byte_idx: usize) -> usize {
    let mut gc = GraphemeCursor::new(byte_idx, text.len(), true);
    gc.next_boundary(text, 0)
      .unwrap_or(Some(text.len()))
      .unwrap_or(text.len())
  }

  /// Recalculate completions based on current input
  /// This is called whenever the input changes
  fn recalculate_completions(&mut self, editor: &Editor) {
    self.selection = None; // Clear selection on recalculate
    self.completion_scroll = 0; // Reset scroll
    // Don't reset completion_list_anim_t here - exponential decay handles it
    // smoothly

    if let Some(ref completion_fn) = self.completion_fn {
      self.completions = completion_fn(editor, &self.input);
    } else {
      self.completions.clear();
    }
  }

  /// Initialize completions (call when prompt is first shown)
  /// This allows completions to appear immediately without typing
  pub fn init_completions(&mut self, editor: &Editor) {
    if let Some(ref completion_fn) = self.completion_fn {
      self.completions = completion_fn(editor, &self.input);
      self.completion_list_anim_t = 0.0; // Start animation
    }
  }

  /// Move to next completion (Ctrl+N)
  fn change_completion_selection_forward(&mut self, _editor: &Editor) {
    if self.completions.is_empty() {
      return;
    }

    let index = match self.selection {
      Some(i) => (i + 1) % self.completions.len(),
      None => 0,
    };

    // Trigger animation if selection changed
    if self.selection != Some(index) {
      self.selection_anim = 0.0;
    }
    self.selection = Some(index);
    self.last_selection = Some(index);

    // Update scroll to keep selection visible (like VSCode)
    const MAX_VISIBLE: usize = 10;
    if index >= self.completion_scroll + MAX_VISIBLE {
      self.completion_scroll = index - MAX_VISIBLE + 1;
    } else if index < self.completion_scroll {
      self.completion_scroll = index;
    }

    // Don't apply completion here - only update selection
    // Completion is applied when Enter is pressed
  }

  /// Move to previous completion (Ctrl+P)
  fn change_completion_selection_backward(&mut self, _editor: &Editor) {
    if self.completions.is_empty() {
      return;
    }

    let index = match self.selection {
      Some(i) => {
        if i == 0 {
          self.completions.len() - 1
        } else {
          i - 1
        }
      },
      None => self.completions.len() - 1,
    };

    // Trigger animation if selection changed
    if self.selection != Some(index) {
      self.selection_anim = 0.0;
    }
    self.selection = Some(index);
    self.last_selection = Some(index);

    // Update scroll to keep selection visible (like VSCode)
    const MAX_VISIBLE: usize = 10;
    if index >= self.completion_scroll + MAX_VISIBLE {
      self.completion_scroll = index - MAX_VISIBLE + 1;
    } else if index < self.completion_scroll {
      self.completion_scroll = index;
    }

    // Don't apply completion here - only update selection
    // Completion is applied when Enter is pressed
  }

  /// Apply the completion at the given index to the input
  fn apply_completion(&mut self, index: usize) {
    if let Some(completion) = self.completions.get(index) {
      // Replace text from range.start onwards with completion text
      let range_start = completion.range.start;
      self.input.replace_range(range_start.., &completion.text);

      // Move cursor to end of input
      self.cursor = self.input.len();
      self.scroll_offset = 0; // Reset scroll
    }
  }

  /// Navigate to next completion and insert it (Tab key behavior)
  fn change_completion_selection_forward_and_insert(&mut self, editor: &Editor) {
    // First navigate to select the next completion
    self.change_completion_selection_forward(editor);

    // Then apply it immediately
    if let Some(idx) = self.selection {
      self.apply_completion(idx);

      // If this is a directory path, recalculate completions for progressive
      // navigation
      if self.should_recalculate_after_completion() {
        self.recalculate_completions(editor);
      }
    }
  }

  /// Navigate to previous completion and insert it (Shift+Tab key behavior)
  fn change_completion_selection_backward_and_insert(&mut self, editor: &Editor) {
    // First navigate to select the previous completion
    self.change_completion_selection_backward(editor);

    // Then apply it immediately
    if let Some(idx) = self.selection {
      self.apply_completion(idx);

      // If this is a directory path, recalculate completions for progressive
      // navigation
      if self.should_recalculate_after_completion() {
        self.recalculate_completions(editor);
      }
    }
  }

  /// Check if we should recalculate completions after applying a completion
  /// This is true for directory paths (ending with /) to enable progressive
  /// navigation
  fn should_recalculate_after_completion(&self) -> bool {
    self.input.ends_with(std::path::MAIN_SEPARATOR)
  }

  /// Exit completion selection mode
  fn exit_selection(&mut self) {
    self.selection = None;
    self.completions.clear();
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

  /// Render the help box above completions showing command documentation
  /// Returns the height of the help box (0 if not rendered)
  fn render_help_box(
    &self,
    surface: &mut Surface,
    completion_bounds: &CompletionBounds,
    screen_width: f32,
    cx: &Context,
  ) {
    // Get documentation for selected completion or current input
    let doc_text = self.doc_fn.as_ref().and_then(|doc_fn| {
      if let Some(selection_idx) = self.selection {
        // Use doc_fn with the selected completion's text
        self
          .completions
          .get(selection_idx)
          .and_then(|c| doc_fn(&c.text))
      } else {
        // No selection - try doc_fn with current input
        doc_fn(&self.input)
      }
    });

    let doc_text = match doc_text {
      Some(doc) if !doc.is_empty() => doc,
      _ => return,
    };

    const PADDING: f32 = 12.0;
    const LINE_HEIGHT: f32 = 20.0;
    const CORNER_RADIUS: f32 = 6.0;
    const GAP: f32 = 8.0; // Gap between completion box and help box

    // Get theme colors
    let theme = &cx.editor.theme;
    let bg_style = theme.get("ui.help");
    let bg_color = bg_style
      .bg
      .map(crate::ui::theme_color_to_renderer_color)
      .unwrap_or_else(|| {
        // Fallback to popup style
        theme
          .get("ui.popup")
          .bg
          .map(crate::ui::theme_color_to_renderer_color)
          .unwrap_or(Color::new(0.12, 0.12, 0.14, 0.95))
      });

    let text_color = bg_style
      .fg
      .map(crate::ui::theme_color_to_renderer_color)
      .unwrap_or_else(|| {
        theme
          .get("ui.text")
          .fg
          .map(crate::ui::theme_color_to_renderer_color)
          .unwrap_or(Color::new(0.8, 0.75, 0.6, 1.0))
      });

    let border_color = theme
      .get("ui.window")
      .fg
      .map(crate::ui::theme_color_to_renderer_color)
      .unwrap_or(Color::new(0.3, 0.3, 0.35, 0.8));

    // Position to the right of the completion box, anchored at the bottom (near
    // statusline)
    let base_box_x = completion_bounds.x + completion_bounds.width + GAP;
    let available_width = screen_width - base_box_x - 10.0; // 10px margin from right edge

    // Don't render if not enough space or animation not started
    if available_width < 150.0 || self.help_box_anim_t < 0.01 {
      return;
    }

    // Count lines and calculate dimensions
    let lines: Vec<&str> = doc_text.lines().collect();
    let line_count = lines.len();
    let content_height = line_count as f32 * LINE_HEIGHT;
    let box_height = content_height + PADDING * 2.0;
    let box_width = available_width;

    // Apply animation: slide from right and fade in
    let anim_t = self.help_box_anim_t;
    let slide_offset = (1.0 - anim_t) * 20.0; // Slide from right 20px
    let box_x = base_box_x + slide_offset;

    // Position at bottom (aligned with bottom of completions, just above
    // statusline)
    let completion_bottom = completion_bounds.y + completion_bounds.height;
    let box_y = completion_bottom - box_height;

    // Apply alpha based on animation
    let mut animated_bg = bg_color;
    animated_bg.a *= anim_t;
    let mut animated_border = border_color;
    animated_border.a *= anim_t;
    let mut animated_text = text_color;
    animated_text.a *= anim_t;

    // Use overlay region to mask underlying content
    surface.with_overlay_region(box_x, box_y, box_width, box_height, |surface| {
      // Draw background
      surface.draw_rounded_rect(
        box_x,
        box_y,
        box_width,
        box_height,
        CORNER_RADIUS,
        animated_bg,
      );

      // Draw border
      surface.draw_rounded_rect_stroke(
        box_x,
        box_y,
        box_width,
        box_height,
        CORNER_RADIUS,
        1.0,
        animated_border,
      );

      // Draw text lines
      let text_x = box_x + PADDING;
      let mut text_y = box_y + PADDING;

      for line in &lines {
        if text_y > box_y + box_height - PADDING {
          break; // Stop if we've exceeded the box height
        }
        surface.draw_text(TextSection {
          position: (text_x, text_y),
          texts:    vec![TextSegment {
            content: line.to_string(),
            style:   TextStyle {
              size:  crate::ui::UI_FONT_SIZE,
              color: animated_text,
            },
          }],
        });
        text_y += LINE_HEIGHT;
      }
    });
  }

  fn render_completions_internal(
    &mut self,
    surface: &mut Surface,
    prompt_y: f32,
    completion_x: f32,
    completion_width: f32,
    cell_width: f32,
    cx: &Context,
  ) -> Option<CompletionBounds> {
    // Returns the bounds of the completion box, or None if no completions
    if self.completions.is_empty() {
      return None;
    }

    const COMPLETION_ITEM_HEIGHT: f32 = 24.0; // Taller items like picker
    const MAX_VISIBLE: usize = 10;
    const ITEM_PADDING_X: f32 = 12.0;
    const ITEM_PADDING_Y: f32 = 6.0;
    const ITEM_GAP: f32 = 2.0;
    const CORNER_RADIUS: f32 = 6.0;

    // Get theme colors (like picker does)
    let theme = &cx.editor.theme;
    let bg_style = theme.get("ui.popup");
    let bg_color = bg_style
      .bg
      .map(crate::ui::theme_color_to_renderer_color)
      .unwrap_or(Color::new(0.15, 0.15, 0.16, 0.95));

    let text_style = theme.get("ui.text");
    let text_color = text_style
      .fg
      .map(crate::ui::theme_color_to_renderer_color)
      .unwrap_or(Color::rgb(0.8, 0.8, 0.8));

    let selected_style = theme.get("ui.picker.selected");
    let mut selected_fill = selected_style
      .bg
      .map(crate::ui::theme_color_to_renderer_color)
      .unwrap_or(Color::new(0.25, 0.45, 0.75, 0.6));
    selected_fill.a = 1.0;

    let mut selected_outline = selected_style
      .fg
      .map(crate::ui::theme_color_to_renderer_color)
      .unwrap_or(Color::new(0.3, 0.6, 0.9, 1.0));
    selected_outline.a = 1.0;

    let selected_fg_style = theme.get("ui.picker.selected");
    let selected_fg = selected_fg_style
      .fg
      .map(crate::ui::theme_color_to_renderer_color)
      .unwrap_or(Color::rgb(1.0, 1.0, 1.0));

    // Calculate visible range with scrolling
    let offset = self.completion_scroll;
    let visible_count = (self.completions.len() - offset).min(MAX_VISIBLE);
    let completion_height = visible_count as f32 * (COMPLETION_ITEM_HEIGHT + ITEM_GAP) - ITEM_GAP;

    // Apply animation: slide up from bottom and fade in
    let anim_t = self.completion_list_anim_t;
    let slide_offset = (1.0 - anim_t) * 15.0; // Slide up 15px
    let completion_y = prompt_y - completion_height - 5.0 + slide_offset; // 5px gap above prompt

    // Apply alpha to colors based on animation
    let mut animated_bg = bg_color;
    animated_bg.a *= anim_t;

    // Draw background with rounded corners
    surface.draw_rounded_rect(
      completion_x,
      completion_y,
      completion_width,
      completion_height,
      CORNER_RADIUS,
      animated_bg,
    );

    // Clip all completion content to the box bounds
    surface.push_scissor_rect(
      completion_x,
      completion_y,
      completion_width,
      completion_height,
    );

    // Draw completions with scrolling
    for (visual_i, actual_i) in (offset..offset + visible_count).enumerate() {
      if let Some(completion) = self.completions.get(actual_i) {
        let y = completion_y + visual_i as f32 * (COMPLETION_ITEM_HEIGHT + ITEM_GAP);
        let is_selected = self.selection == Some(actual_i);

        let item_x = completion_x + 4.0;
        let item_width = completion_width - 8.0;
        let item_radius = 4.0;

        // Alternating stripe background (like picker)
        let stripe_primary = Self::mix_colors(bg_color, selected_fill, 0.1);
        let stripe_secondary = Self::mix_colors(bg_color, selected_fill, 0.05);
        let stripe_color = if actual_i % 2 == 0 {
          stripe_primary
        } else {
          stripe_secondary
        };
        surface.draw_rounded_rect(
          item_x,
          y,
          item_width,
          COMPLETION_ITEM_HEIGHT,
          item_radius,
          stripe_color,
        );

        // Draw selection background with animation (like picker)
        if is_selected {
          let selection_t = self.selection_anim.clamp(0.0, 1.0);
          let selection_ease = selection_t * selection_t * (3.0 - 2.0 * selection_t); // Smoothstep

          let mut fill_color = selected_fill;
          fill_color.a = (fill_color.a * (0.82 + 0.18 * selection_ease)).clamp(0.0, 1.0);
          surface.draw_rounded_rect(
            item_x,
            y,
            item_width,
            COMPLETION_ITEM_HEIGHT,
            item_radius,
            fill_color,
          );

          // Draw outline with variable thickness (like picker)
          let bottom_thickness = (COMPLETION_ITEM_HEIGHT * 0.035).clamp(0.6, 1.2);
          let side_thickness = (bottom_thickness * 1.55).min(bottom_thickness + 1.6);
          let top_thickness = (bottom_thickness * 2.2).min(bottom_thickness + 2.4);
          surface.draw_rounded_rect_stroke_fade(
            item_x,
            y,
            item_width,
            COMPLETION_ITEM_HEIGHT,
            item_radius,
            top_thickness,
            side_thickness,
            bottom_thickness,
            selected_outline,
          );

          // Draw glow effect (like picker)
          let glow_strength = (0.85 + 0.15 * selection_ease).clamp(0.0, 1.0);
          Button::draw_hover_layers(
            surface,
            item_x,
            y,
            item_width,
            COMPLETION_ITEM_HEIGHT,
            item_radius,
            selected_outline,
            glow_strength,
          );

          // Pulse animation on selection change (like picker)
          if self.selection_anim < 1.0 {
            let pulse_ease = 1.0 - (1.0 - selection_t) * (1.0 - selection_t);
            let center_x = item_x + item_width * 0.5;
            let center_y = y + COMPLETION_ITEM_HEIGHT * 0.52;
            let pulse_radius =
              item_width.max(COMPLETION_ITEM_HEIGHT) * (0.42 + 0.35 * (1.0 - pulse_ease));
            let pulse_alpha = (1.0 - pulse_ease) * 0.18;
            let glow_color = Self::glow_from_base(selected_outline);
            surface.draw_rounded_rect_glow(
              item_x,
              y,
              item_width,
              COMPLETION_ITEM_HEIGHT,
              item_radius,
              center_x,
              center_y,
              pulse_radius,
              Color::new(glow_color.r, glow_color.g, glow_color.b, pulse_alpha),
            );
          }
        }

        // Draw text with truncation if too long
        let text_x = item_x + ITEM_PADDING_X;
        let text_y = y + ITEM_PADDING_Y;
        let item_color = if is_selected { selected_fg } else { text_color };

        // Calculate available width for text
        let available_width = item_width - (ITEM_PADDING_X * 2.0);
        let max_chars = (available_width / cell_width) as usize;

        // Truncate text if needed
        let display_text = if completion.text.chars().count() > max_chars && max_chars > 2 {
          let truncated: String = completion.text.chars().take(max_chars - 2).collect();
          format!("{}..", truncated)
        } else {
          completion.text.clone()
        };

        surface.draw_text(TextSection::simple(
          text_x,
          text_y,
          &display_text,
          UI_FONT_SIZE,
          item_color,
        ));
      }
    }

    // Pop the scissor rect for the entire completion box
    surface.pop_scissor_rect();

    Some(CompletionBounds {
      x:      completion_x,
      y:      completion_y,
      width:  completion_width,
      height: completion_height,
    })
  }

  // Helper functions (copied from picker)
  fn mix_colors(a: Color, b: Color, t: f32) -> Color {
    Color::new(
      a.r + (b.r - a.r) * t,
      a.g + (b.g - a.g) * t,
      a.b + (b.b - a.b) * t,
      a.a + (b.a - a.a) * t,
    )
  }

  fn glow_from_base(base: Color) -> Color {
    let brightness = (base.r + base.g + base.b) / 3.0;
    let boost = if brightness < 0.5 { 1.8 } else { 1.3 };
    Color::new(
      (base.r * boost).min(1.0),
      (base.g * boost).min(1.0),
      (base.b * boost).min(1.0),
      base.a,
    )
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
      super_:  false,
      pressed: true,
    };

    let prompt_event = self.handle_key_internal(key_press, cx.editor);

    // If we have a custom callback, use it instead of command execution
    if let Some(ref mut callback) = self.callback_fn {
      let input = self.input.clone();
      callback(cx, &input, prompt_event);

      // For Abort and Validate events, close the prompt
      if prompt_event == PromptEvent::Abort || prompt_event == PromptEvent::Validate {
        return EventResult::Consumed(Some(Box::new(|compositor, cx| {
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
        })));
      }

      return EventResult::Consumed(None);
    }

    // No custom callback - fall back to command execution
    // Special handling for numeric input - treat as :goto command
    let trimmed = self.input.trim();

    match prompt_event {
      PromptEvent::Abort => {
        // Execute command's Abort handler if we have input
        if !trimmed.is_empty() {
          let (command, args_line) = if trimmed.parse::<usize>().is_ok() {
            ("goto".to_string(), trimmed.to_string())
          } else {
            let (command, args_line, _) = command_line::split(trimmed);
            (command.to_string(), args_line.to_string())
          };

          let registry = cx.editor.command_registry.clone();
          let mut cmd_cx = CommandContext {
            editor:               cx.editor,
            count:                None,
            register:             None,
            callback:             Vec::new(),
            on_next_key_callback: None,
            jobs:                 cx.jobs,
          };
          let _ = registry.execute(&mut cmd_cx, &command, &args_line, PromptEvent::Abort);
        }

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
          use crate::core::command_line;

          // Check if it's numeric input - treat as :goto command
          let (command, args_line) = if trimmed.parse::<usize>().is_ok() {
            ("goto".to_string(), trimmed.clone())
          } else {
            let (command, args_line, _) = command_line::split(&trimmed);
            (command.to_string(), args_line.to_string())
          };

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

            let result = registry.execute(&mut cmd_cx, &command, &args_line, PromptEvent::Validate);

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
      PromptEvent::Update => {
        // Only execute preview-enabled commands on Update events
        // This prevents accidental execution of commands like :quit while typing
        if !trimmed.is_empty() {
          let (command, args_line) = if trimmed.parse::<usize>().is_ok() {
            // Numeric input is treated as goto command for line preview
            ("goto".to_string(), trimmed.to_string())
          } else {
            let (command, args_line, _) = command_line::split(trimmed);
            (command.to_string(), args_line.to_string())
          };

          // Whitelist of commands that support preview on Update events
          let preview_enabled_commands = ["theme", "goto"];

          if preview_enabled_commands.contains(&command.as_str()) {
            let registry = cx.editor.command_registry.clone();
            let mut cmd_cx = CommandContext {
              editor:               cx.editor,
              count:                None,
              register:             None,
              callback:             Vec::new(),
              on_next_key_callback: None,
              jobs:                 cx.jobs,
            };
            let _ = registry.execute(&mut cmd_cx, &command, &args_line, PromptEvent::Update);
          }
        }
        EventResult::Consumed(None)
      },
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
    // Keep updating while animations are active or for input responsiveness
    // For exponential decay, check if we're "close enough" to target (within 0.01)
    self.glow_anim_t < 1.0
      || self.selection_anim < 0.99
      || self.prompt_open_t < 0.99
      || (self.completion_list_anim_t > 0.01 && self.completion_list_anim_t < 0.99)
      || (!self.completions.is_empty() && self.completion_list_anim_t < 0.99)
      || (self.completions.is_empty() && self.completion_list_anim_t > 0.01)
      || self.help_box_anim_t > 0.01 && self.help_box_anim_t < 0.99
      || self.cursor_anim_active
      || !self.completions.is_empty() // Keep updating while completions are visible
  }
}

#[cfg(test)]
mod tests {
  use super::*;

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

    // Set up a simple completion function
    let completion_fn = Arc::new(|_editor: &Editor, input: &str| -> Vec<Completion> {
      if input.starts_with('q') {
        vec![
          Completion {
            range: 0..,
            text:  "quit".to_string(),
            doc:   None,
          },
          Completion {
            range: 0..,
            text:  "query".to_string(),
            doc:   None,
          },
        ]
      } else {
        vec![]
      }
    });

    prompt.set_completion_fn(completion_fn);
    prompt.set_input("q".to_string());

    // Note: recalculate_completions requires an Editor instance
    // For now, just test that completion function was set
    assert!(prompt.completion_fn.is_some());
  }
}
