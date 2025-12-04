//! Confirmation Popup component.
//!
//! A reusable popup for confirmation dialogs (permissions, deletions, etc.).
//! Displays at top-right of screen with slide-in animation and configurable
//! buttons.

use the_editor_renderer::{
  Color,
  Key,
  TextSection,
  TextSegment,
  TextStyle,
};

use crate::{
  core::{
    animation::{
      AnimationHandle,
      presets,
    },
    graphics::{
      CursorKind,
      Rect,
    },
    position::Position,
  },
  ui::{
    UI_FONT_SIZE,
    compositor::{
      Callback,
      Component,
      Context,
      Event,
      EventResult,
      Surface,
    },
    theme_color_to_renderer_color,
  },
};

const PADDING: f32 = 16.0;
const MARGIN: f32 = 12.0;
const POPUP_WIDTH: f32 = 320.0;
const CORNER_RADIUS: f32 = 8.0;
const MAX_TITLE_LINES: usize = 3;

/// Callback type for confirmation result handler.
type ConfirmationCallback = Box<dyn FnOnce(&mut Context, ConfirmationResult) + Send>;

/// Button configuration for confirmation popup.
#[derive(Clone)]
pub struct ConfirmationButton {
  pub label:    &'static str,
  pub shortcut: &'static str,
}

impl ConfirmationButton {
  pub const fn new(label: &'static str, shortcut: &'static str) -> Self {
    Self { label, shortcut }
  }
}

/// Result of a confirmation action.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfirmationResult {
  /// User confirmed (selected first/primary button)
  Confirmed,
  /// User denied/cancelled (selected second button or escaped)
  Denied,
}

/// Configuration for the confirmation popup.
pub struct ConfirmationConfig {
  pub id:      &'static str,
  pub title:   String,
  pub buttons: [ConfirmationButton; 2],
}

impl ConfirmationConfig {
  pub fn new(id: &'static str, title: impl Into<String>) -> Self {
    Self {
      id,
      title: title.into(),
      buttons: [
        ConfirmationButton::new("Confirm", "y"),
        ConfirmationButton::new("Cancel", "n"),
      ],
    }
  }

  pub fn with_buttons(mut self, buttons: [ConfirmationButton; 2]) -> Self {
    self.buttons = buttons;
    self
  }
}

/// Popup component for confirmation dialogs.
/// Appears at top-right corner with a slide-in animation.
pub struct ConfirmationPopup {
  id:            &'static str,
  title:         String,
  buttons:       [ConfirmationButton; 2],
  cursor:        usize, // 0 = first button, 1 = second button
  animation:     AnimationHandle<f32>,
  closing:       bool,
  close_pending: bool,
  on_result:     Option<ConfirmationCallback>,
}

impl ConfirmationPopup {
  pub fn new<F>(config: ConfirmationConfig, on_result: F) -> Self
  where
    F: FnOnce(&mut Context, ConfirmationResult) + Send + 'static,
  {
    let (duration, easing) = presets::POPUP;
    Self {
      id:            config.id,
      title:         config.title,
      buttons:       config.buttons,
      cursor:        0,
      animation:     AnimationHandle::new(0.0, 1.0, duration, easing),
      closing:       false,
      close_pending: false,
      on_result:     Some(Box::new(on_result)),
    }
  }

  /// Start the closing animation and execute the callback immediately
  fn start_close(&mut self, result: ConfirmationResult, cx: &mut Context) {
    if !self.closing {
      self.closing = true;
      // Execute callback immediately so there's no delay
      if let Some(on_result) = self.on_result.take() {
        on_result(cx, result);
      }
      let (duration, easing) = presets::POPUP;
      self.animation = AnimationHandle::new(1.0, 0.0, duration, easing);
    }
  }

  fn close_callback(&self) -> Callback {
    let id = self.id;
    Box::new(move |compositor, _| {
      compositor.remove(id);
    })
  }

  /// Split text into lines that fit within max_chars
  fn wrap_text(text: &str, max_chars: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut remaining = text;

    while !remaining.is_empty() && lines.len() < MAX_TITLE_LINES {
      if remaining.len() <= max_chars {
        lines.push(remaining.to_string());
        break;
      }

      // Find a good break point (prefer path separators)
      let break_at = remaining[..max_chars]
        .rfind('/')
        .map(|i| i + 1) // Include the slash
        .or_else(|| remaining[..max_chars].rfind(' ').map(|i| i + 1))
        .unwrap_or(max_chars);

      lines.push(remaining[..break_at].to_string());
      remaining = &remaining[break_at..];
    }

    // If there's still remaining text on the last line, indicate truncation
    if !remaining.is_empty()
      && lines.len() == MAX_TITLE_LINES
      && let Some(last) = lines.last_mut()
      && last.len() > 3
    {
      last.truncate(last.len() - 3);
      last.push_str("...");
    }

    lines
  }
}

impl Component for ConfirmationPopup {
  fn handle_event(&mut self, event: &Event, cx: &mut Context) -> EventResult {
    // If closing animation is done, remove the component but let the event
    // propagate
    if self.close_pending {
      return EventResult::Ignored(Some(self.close_callback()));
    }

    // Don't handle events while closing, but let them propagate to other layers
    if self.closing {
      return EventResult::Ignored(None);
    }

    let Event::Key(key) = event else {
      return EventResult::Ignored(None);
    };

    match (key.code, key.ctrl, key.alt, key.shift) {
      // Close/Cancel
      (Key::Escape, ..) => {
        self.start_close(ConfirmationResult::Denied, cx);
        EventResult::Consumed(None)
      },

      // Navigate between buttons
      (Key::Left, ..) | (Key::Char('h'), false, false, false) | (Key::Tab, false, false, true) => {
        self.cursor = 0;
        EventResult::Consumed(None)
      },
      (Key::Right, ..)
      | (Key::Char('l'), false, false, false)
      | (Key::Tab, false, false, false) => {
        self.cursor = 1;
        EventResult::Consumed(None)
      },

      // Quick confirm with 'y' or first button shortcut
      (Key::Char('y'), false, false, false) => {
        self.start_close(ConfirmationResult::Confirmed, cx);
        EventResult::Consumed(None)
      },

      // Quick deny with 'n' or second button shortcut
      (Key::Char('n'), false, false, false) => {
        self.start_close(ConfirmationResult::Denied, cx);
        EventResult::Consumed(None)
      },

      // Confirm selection with Enter
      (Key::Enter | Key::NumpadEnter, ..) => {
        let result = if self.cursor == 0 {
          ConfirmationResult::Confirmed
        } else {
          ConfirmationResult::Denied
        };
        self.start_close(result, cx);
        EventResult::Consumed(None)
      },

      _ => EventResult::Ignored(None),
    }
  }

  fn render(&mut self, _area: Rect, surface: &mut Surface, cx: &mut Context) {
    // Update animation
    self.animation.update(cx.dt);
    let t = *self.animation.current();

    // Check if closing animation is complete
    if self.closing && self.animation.is_complete() {
      self.close_pending = true;
      return;
    }

    let font_state = surface.save_font_state();
    surface.configure_font(&font_state.family, UI_FONT_SIZE);

    // Use the animated value directly - easing is handled by AnimationHandle
    let eased = t;
    let alpha = eased;

    // Get theme colors
    let theme = &cx.editor.theme;
    let bg_color = {
      let mut c = theme
        .get("ui.popup")
        .bg
        .map(theme_color_to_renderer_color)
        .unwrap_or(Color::new(0.1, 0.1, 0.12, 1.0));
      c.a *= alpha;
      c
    };

    let text_color = {
      let mut c = theme
        .get("ui.text")
        .fg
        .map(theme_color_to_renderer_color)
        .unwrap_or(Color::new(0.9, 0.9, 0.9, 1.0));
      c.a *= alpha;
      c
    };

    let dim_color = {
      let mut c = text_color;
      c.a *= 0.6;
      c
    };

    let accent_color = {
      let mut c = theme
        .get("ui.selection")
        .bg
        .or_else(|| theme.get("special").fg)
        .map(theme_color_to_renderer_color)
        .unwrap_or(Color::new(0.3, 0.5, 0.8, 1.0));
      c.a *= alpha;
      c
    };

    // Calculate dimensions
    let viewport_width = surface.width() as f32;
    let line_height = UI_FONT_SIZE + 4.0;
    let char_width = UI_FONT_SIZE * 0.55;

    // Wrap title into multiple lines
    let max_chars = ((POPUP_WIDTH - PADDING * 2.0) / char_width) as usize;
    let title_lines = Self::wrap_text(&self.title, max_chars);
    let num_title_lines = title_lines.len();

    // Popup height: title lines + buttons + padding
    let popup_height = (num_title_lines as f32 * line_height) + line_height + 8.0 + PADDING * 2.5;

    // Position: top-right, slides in/out from the right
    let slide_offset = (1.0 - eased) * (POPUP_WIDTH + MARGIN);
    let popup_x = viewport_width - POPUP_WIDTH - MARGIN + slide_offset;
    let popup_y = MARGIN;

    // Use overlay region to render on top of other content
    surface.with_overlay_region(popup_x, popup_y, POPUP_WIDTH, popup_height, |surface| {
      // Draw background
      surface.draw_rounded_rect(
        popup_x,
        popup_y,
        POPUP_WIDTH,
        popup_height,
        CORNER_RADIUS,
        bg_color,
      );

      // Draw subtle border
      let mut border_color = accent_color;
      border_color.a *= 0.4;
      surface.draw_rounded_rect_stroke(
        popup_x,
        popup_y,
        POPUP_WIDTH,
        popup_height,
        CORNER_RADIUS,
        1.5,
        border_color,
      );

      // Draw accent bar on left
      surface.draw_rounded_rect(
        popup_x,
        popup_y,
        4.0,
        popup_height,
        CORNER_RADIUS,
        accent_color,
      );

      // Content area
      let content_x = popup_x + PADDING;
      let content_width = POPUP_WIDTH - PADDING * 2.0;

      // Title lines
      let mut y = popup_y + PADDING;
      for line in &title_lines {
        surface.draw_text(TextSection {
          position: (content_x, y),
          texts:    vec![TextSegment {
            content: line.clone(),
            style:   TextStyle {
              size:  UI_FONT_SIZE,
              color: text_color,
            },
          }],
        });
        y += line_height;
      }

      // Buttons row
      let buttons_y = y + PADDING * 0.3;
      let button_width = (content_width - PADDING) / 2.0;
      let button_height = line_height + 8.0;

      for (i, button) in self.buttons.iter().enumerate() {
        let btn_x = content_x + (i as f32 * (button_width + PADDING));
        let is_selected = i == self.cursor;

        // Button background
        let btn_bg = if is_selected {
          let mut c = accent_color;
          c.a = alpha * 0.3;
          c
        } else {
          let mut c = text_color;
          c.a = alpha * 0.08;
          c
        };

        surface.draw_rounded_rect(btn_x, buttons_y, button_width, button_height, 4.0, btn_bg);

        // Button border when selected
        if is_selected {
          surface.draw_rounded_rect_stroke(
            btn_x,
            buttons_y,
            button_width,
            button_height,
            4.0,
            1.0,
            accent_color,
          );
        }

        // Button text
        let btn_text_color = if is_selected { text_color } else { dim_color };
        let btn_text = format!("{} ({})", button.label, button.shortcut);
        let text_width = btn_text.len() as f32 * char_width;
        let text_x = btn_x + (button_width - text_width) / 2.0;
        let text_y = buttons_y + (button_height - UI_FONT_SIZE) / 2.0;

        surface.draw_text(TextSection {
          position: (text_x, text_y),
          texts:    vec![TextSegment {
            content: btn_text,
            style:   TextStyle {
              size:  UI_FONT_SIZE,
              color: btn_text_color,
            },
          }],
        });
      }
    });

    surface.restore_font_state(font_state);
  }

  fn cursor(&self, _area: Rect, _editor: &crate::editor::Editor) -> (Option<Position>, CursorKind) {
    (None, CursorKind::Hidden)
  }

  fn id(&self) -> Option<&'static str> {
    Some(self.id)
  }

  fn is_animating(&self) -> bool {
    !self.animation.is_complete()
  }
}
