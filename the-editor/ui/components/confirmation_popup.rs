//! Confirmation Popup component.
//!
//! A reusable popup for confirmation dialogs (permissions, deletions, etc.).
//! Displays at top-right of screen with slide-in animation and configurable
//! buttons.

use the_editor_renderer::{Color, Key, MouseButton, TextSection, TextSegment, TextStyle};

use crate::{
  core::{
    animation::{AnimationHandle, presets},
    graphics::{CursorKind, Rect},
    position::Position,
  },
  ui::{
    UI_FONT_SIZE,
    components::button::Button,
    compositor::{Callback, Component, Context, Event, EventResult, Surface},
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
  pub label: &'static str,
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
  pub id: &'static str,
  pub title: String,
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

/// State for an individual button in the popup.
#[derive(Default)]
struct ButtonState {
  hovered: bool,
  pressed: bool,
  anim_t: f32,
  /// Cursor position relative to button top-left (when hovered)
  cursor_px: Option<(f32, f32)>,
}

impl ButtonState {
  /// Update click animation state, returns eased progress
  fn update_anim(&mut self, dt: f32) -> f32 {
    let target = if self.pressed { 1.0 } else { 0.0 };
    let anim_speed = 12.0;

    if (self.anim_t - target).abs() < 0.01 {
      self.anim_t = target;
    } else if self.anim_t < target {
      self.anim_t = (self.anim_t + dt * anim_speed).min(target);
    } else {
      self.anim_t = (self.anim_t - dt * anim_speed).max(target);
    }

    // Smoothstep easing
    let t = self.anim_t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
  }

  fn is_animating(&self) -> bool {
    let target = if self.pressed { 1.0 } else { 0.0 };
    (self.anim_t - target).abs() > 0.01
  }
}

/// Popup component for confirmation dialogs.
/// Appears at top-right corner with a slide-in animation.
pub struct ConfirmationPopup {
  id: &'static str,
  title: String,
  buttons: [ConfirmationButton; 2],
  button_states: [ButtonState; 2],
  cursor: usize, // 0 = first button, 1 = second button (keyboard selection)
  animation: AnimationHandle<f32>,
  closing: bool,
  close_pending: bool,
  on_result: Option<ConfirmationCallback>,
  /// Cached button rects for mouse hit testing (in pixels)
  button_rects: [(f32, f32, f32, f32); 2],
}

impl ConfirmationPopup {
  pub fn new<F>(config: ConfirmationConfig, on_result: F) -> Self
  where
    F: FnOnce(&mut Context, ConfirmationResult) + Send + 'static,
  {
    let (duration, easing) = presets::POPUP;
    Self {
      id: config.id,
      title: config.title,
      buttons: config.buttons,
      button_states: Default::default(),
      cursor: 0,
      animation: AnimationHandle::new(0.0, 1.0, duration, easing),
      closing: false,
      close_pending: false,
      on_result: Some(Box::new(on_result)),
      button_rects: [(0.0, 0.0, 0.0, 0.0); 2],
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

  /// Check if a point is inside a button rect
  fn point_in_button(&self, idx: usize, mx: f32, my: f32) -> bool {
    let (x, y, w, h) = self.button_rects[idx];
    mx >= x && mx <= x + w && my >= y && my <= y + h
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

    match event {
      Event::Mouse(mouse) => {
        let (mx, my) = mouse.position;

        // Update hover state for both buttons
        for (i, state) in self.button_states.iter_mut().enumerate() {
          let (bx, by, bw, bh) = self.button_rects[i];
          let inside = mx >= bx && mx <= bx + bw && my >= by && my <= by + bh;

          if inside {
            state.hovered = true;
            state.cursor_px = Some((mx - bx, my - by));
            // Update keyboard cursor to match hovered button
            self.cursor = i;
          } else {
            state.hovered = false;
            state.cursor_px = None;
          }
        }

        // Handle clicks
        if let Some(MouseButton::Left) = mouse.button {
          for i in 0..2 {
            let inside = self.point_in_button(i, mx, my);
            if inside && mouse.pressed {
              self.button_states[i].pressed = true;
              return EventResult::Consumed(None);
            } else if self.button_states[i].pressed && !mouse.pressed {
              self.button_states[i].pressed = false;
              if inside {
                // Click completed inside button - trigger action
                let result = if i == 0 {
                  ConfirmationResult::Confirmed
                } else {
                  ConfirmationResult::Denied
                };
                self.start_close(result, cx);
              }
              return EventResult::Consumed(None);
            }
          }
        }

        // Consume mouse events over the popup to prevent pass-through
        EventResult::Consumed(None)
      },

      Event::Key(key) => {
        match (key.code, key.ctrl, key.alt, key.shift) {
          // Close/Cancel
          (Key::Escape, ..) => {
            self.start_close(ConfirmationResult::Denied, cx);
            EventResult::Consumed(None)
          },

          // Navigate between buttons
          (Key::Left, ..)
          | (Key::Char('h'), false, false, false)
          | (Key::Tab, false, false, true) => {
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

    // Calculate button positions for hit testing
    let content_x = popup_x + PADDING;
    let content_width = POPUP_WIDTH - PADDING * 2.0;
    let buttons_y = popup_y + PADDING + (num_title_lines as f32 * line_height) + PADDING * 0.3;
    let button_width = (content_width - PADDING) / 2.0;
    let button_height = line_height + 8.0;

    // Store button rects for mouse hit testing
    for i in 0..2 {
      let btn_x = content_x + (i as f32 * (button_width + PADDING));
      self.button_rects[i] = (btn_x, buttons_y, button_width, button_height);
    }

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

      // Buttons
      for (i, button) in self.buttons.iter().enumerate() {
        let (btn_x, btn_y, btn_w, btn_h) = self.button_rects[i];
        let state = &mut self.button_states[i];
        let is_selected = i == self.cursor;
        let btn_radius = 4.0;

        // Update button animation
        let click_t = state.update_anim(cx.dt);

        // Determine button colors based on state
        let (btn_bg, btn_outline, btn_text_color) = if state.pressed {
          // Pressed state - darker/more saturated
          let pressed_bg = {
            let mut c = accent_color;
            c.a = alpha * 0.5;
            c
          };
          (pressed_bg, accent_color, text_color)
        } else if state.hovered || is_selected {
          // Hovered or keyboard-selected
          let hover_bg = {
            let mut c = accent_color;
            c.a = alpha * 0.3;
            c
          };
          (hover_bg, accent_color, text_color)
        } else {
          // Normal state
          let normal_bg = {
            let mut c = text_color;
            c.a = alpha * 0.08;
            c
          };
          (normal_bg, dim_color, dim_color)
        };

        // Draw button background
        surface.draw_rounded_rect(btn_x, btn_y, btn_w, btn_h, btn_radius, btn_bg);

        // Draw button outline
        surface.draw_rounded_rect_stroke(btn_x, btn_y, btn_w, btn_h, btn_radius, 1.0, btn_outline);

        // Draw hover glow effect using Button's helper
        if state.hovered {
          let glow_strength = (1.0 - click_t * 0.7).max(0.0);
          Button::draw_hover_layers(
            surface,
            btn_x,
            btn_y,
            btn_w,
            btn_h,
            btn_radius,
            accent_color,
            glow_strength * alpha,
          );
        }

        // Draw press glow (bottom glow on click)
        if click_t > 0.0 {
          let bottom_center_y = btn_y + btn_h + 1.5;
          let bottom_glow_strength = click_t * 0.12 * alpha;
          let bottom_glow = Color::new(
            accent_color.r,
            accent_color.g,
            accent_color.b,
            bottom_glow_strength,
          );
          let bottom_radius = (btn_w * 0.45).max(btn_h * 0.42);
          surface.draw_rounded_rect_glow(
            btn_x,
            btn_y,
            btn_w,
            btn_h,
            btn_radius,
            btn_x + btn_w * 0.5,
            bottom_center_y,
            bottom_radius,
            bottom_glow,
          );
        }

        // Button text
        let btn_text = format!("{} ({})", button.label, button.shortcut);
        let text_width = btn_text.len() as f32 * char_width;
        let text_x = btn_x + (btn_w - text_width) / 2.0;
        let text_y = btn_y + (btn_h - UI_FONT_SIZE) / 2.0;

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
    });

    surface.restore_font_state(font_state);
  }

  fn cursor(&self, _area: Rect, _editor: &crate::editor::Editor) -> (Option<Position>, CursorKind) {
    (None, CursorKind::Hidden)
  }

  fn id(&self) -> Option<&'static str> {
    Some(self.id)
  }

  fn should_update(&self) -> bool {
    // Keep updating while any animation is active
    !self.animation.is_complete()
      || self.button_states[0].is_animating()
      || self.button_states[1].is_animating()
      || self.button_states[0].hovered
      || self.button_states[1].hovered
  }

  fn is_animating(&self) -> bool {
    !self.animation.is_complete()
      || self.button_states[0].is_animating()
      || self.button_states[1].is_animating()
  }
}
