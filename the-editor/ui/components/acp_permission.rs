//! ACP Permission Popup component.
//!
//! Displays a permission request notification at the top-right of the screen.
//! Slides in from the right with a smooth animation.

use the_editor_renderer::{Color, Key, TextSection, TextSegment, TextStyle};

use crate::{
  core::{
    animation::{AnimationHandle, presets},
    graphics::{CursorKind, Rect},
    position::Position,
  },
  ui::{
    UI_FONT_SIZE,
    compositor::{Callback, Component, Context, Event, EventResult, Surface},
    theme_color_to_renderer_color,
  },
};

const PADDING: f32 = 16.0;
const MARGIN: f32 = 12.0;
const POPUP_WIDTH: f32 = 320.0;
const CORNER_RADIUS: f32 = 8.0;
const MAX_TITLE_LINES: usize = 3;

/// Popup component for approving/denying ACP permission requests.
/// Appears at top-right corner with a slide-in animation.
pub struct AcpPermissionPopup {
  cursor: usize, // 0 = Allow, 1 = Deny
  animation: AnimationHandle<f32>,
  closing: bool,
  close_pending: bool,
}

impl AcpPermissionPopup {
  pub const ID: &'static str = "acp-permission";

  pub fn new() -> Self {
    let (duration, easing) = presets::POPUP;
    Self {
      cursor: 0,
      animation: AnimationHandle::new(0.0, 1.0, duration, easing),
      closing: false,
      close_pending: false,
    }
  }

  /// Start the closing animation
  fn start_close(&mut self) {
    if !self.closing {
      self.closing = true;
      let (duration, easing) = presets::POPUP;
      self.animation = AnimationHandle::new(1.0, 0.0, duration, easing);
    }
  }

  fn close_callback() -> Callback {
    Box::new(|compositor, _| {
      compositor.remove(Self::ID);
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
    if !remaining.is_empty() && lines.len() == MAX_TITLE_LINES {
      if let Some(last) = lines.last_mut() {
        if last.len() > 3 {
          last.truncate(last.len() - 3);
          last.push_str("...");
        }
      }
    }

    lines
  }
}

impl Default for AcpPermissionPopup {
  fn default() -> Self {
    Self::new()
  }
}

impl Component for AcpPermissionPopup {
  fn handle_event(&mut self, event: &Event, cx: &mut Context) -> EventResult {
    // If closing animation is done, remove the component but let the event
    // propagate
    if self.close_pending {
      return EventResult::Ignored(Some(Self::close_callback()));
    }

    // Don't handle events while closing, but let them propagate to other layers
    if self.closing {
      return EventResult::Ignored(None);
    }

    // Auto-close if no permissions pending
    if cx.editor.acp_permissions.pending_count() == 0 {
      self.start_close();
      return EventResult::Consumed(None);
    }

    let Event::Key(key) = event else {
      return EventResult::Ignored(None);
    };

    match (key.code, key.ctrl, key.alt, key.shift) {
      // Close
      (Key::Escape, ..) => {
        self.start_close();
        EventResult::Consumed(None)
      },

      // Navigate between Allow/Deny
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

      // Quick approve with 'y'
      (Key::Char('y'), false, false, false) => {
        cx.editor.acp_permissions.approve_next();
        if cx.editor.acp_permissions.pending_count() == 0 {
          self.start_close();
        }
        EventResult::Consumed(None)
      },

      // Quick deny with 'n'
      (Key::Char('n'), false, false, false) => {
        cx.editor.acp_permissions.deny_next();
        if cx.editor.acp_permissions.pending_count() == 0 {
          self.start_close();
        }
        EventResult::Consumed(None)
      },

      // Confirm selection with Enter
      (Key::Enter | Key::NumpadEnter, ..) => {
        if self.cursor == 0 {
          cx.editor.acp_permissions.approve_next();
        } else {
          cx.editor.acp_permissions.deny_next();
        }
        if cx.editor.acp_permissions.pending_count() == 0 {
          self.start_close();
        } else {
          self.cursor = 0; // Reset to Allow for next permission
        }
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

    let Some(permission) = cx.editor.acp_permissions.peek() else {
      if !self.closing {
        self.start_close();
      }
      return;
    };

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
    let title = permission.title();
    let max_chars = ((POPUP_WIDTH - PADDING * 2.0) / char_width) as usize;
    let title_lines = Self::wrap_text(title, max_chars);
    let num_title_lines = title_lines.len();

    // Popup height: title lines + buttons + padding
    let popup_height = (num_title_lines as f32 * line_height) + line_height + 8.0 + PADDING * 2.5;

    // Position: top-right, slides in/out from the right
    let slide_offset = (1.0 - eased) * (POPUP_WIDTH + MARGIN);
    let popup_x = viewport_width - POPUP_WIDTH - MARGIN + slide_offset;
    let popup_y = MARGIN;

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
        texts: vec![TextSegment {
          content: line.clone(),
          style: TextStyle {
            size: UI_FONT_SIZE,
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

    let buttons = [("Allow", "y"), ("Deny", "n")];
    for (i, (label, key)) in buttons.iter().enumerate() {
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
      let btn_text = format!("{} ({})", label, key);
      let text_width = btn_text.len() as f32 * char_width;
      let text_x = btn_x + (button_width - text_width) / 2.0;
      let text_y = buttons_y + (button_height - UI_FONT_SIZE) / 2.0;

      surface.draw_text(TextSection {
        position: (text_x, text_y),
        texts: vec![TextSegment {
          content: btn_text,
          style: TextStyle {
            size: UI_FONT_SIZE,
            color: btn_text_color,
          },
        }],
      });
    }

    surface.restore_font_state(font_state);
  }

  fn cursor(&self, _area: Rect, _editor: &crate::editor::Editor) -> (Option<Position>, CursorKind) {
    (None, CursorKind::Hidden)
  }

  fn id(&self) -> Option<&'static str> {
    Some(Self::ID)
  }

  fn is_animating(&self) -> bool {
    !self.animation.is_complete()
  }
}
