//! ACP Permission Popup component.
//!
//! Displays a simple yes/no prompt for the current pending permission request.
//! Styled identically to the code action menu.

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
    UI_FONT_WIDTH,
    compositor::{
      Component,
      Context,
      Event,
      EventResult,
      Surface,
    },
    popup_positioning::{
      calculate_cursor_position,
      position_popup_near_cursor,
    },
    theme_color_to_renderer_color,
  },
};

const HORIZONTAL_PADDING: f32 = 12.0;
const VERTICAL_PADDING: f32 = 10.0;
const MIN_MENU_WIDTH: f32 = 200.0;
const MAX_MENU_WIDTH: f32 = 400.0;

/// Popup component for approving/denying a single ACP permission request.
pub struct AcpPermissionPopup {
  cursor:    usize, // 0 = Yes, 1 = No
  animation: AnimationHandle<f32>,
}

impl AcpPermissionPopup {
  pub const ID: &'static str = "acp-permission";

  pub fn new() -> Self {
    let (duration, easing) = presets::POPUP;
    Self {
      cursor:    0,
      animation: AnimationHandle::new(0.0, 1.0, duration, easing),
    }
  }

  fn close_popup() -> EventResult {
    EventResult::Consumed(Some(Box::new(|compositor, _| {
      compositor.remove(Self::ID);
    })))
  }
}

impl Default for AcpPermissionPopup {
  fn default() -> Self {
    Self::new()
  }
}

impl Component for AcpPermissionPopup {
  fn handle_event(&mut self, event: &Event, cx: &mut Context) -> EventResult {
    // Auto-close if no permissions pending
    if cx.editor.acp_permissions.pending_count() == 0 {
      return Self::close_popup();
    }

    let Event::Key(key) = event else {
      return EventResult::Ignored(None);
    };

    match (key.code, key.ctrl, key.alt, key.shift) {
      // Close
      (Key::Escape, ..) => Self::close_popup(),

      // Navigate between Yes/No
      (Key::Up, ..)
      | (Key::Char('k'), false, false, false)
      | (Key::Left, ..)
      | (Key::Char('h'), false, false, false) => {
        self.cursor = 0;
        EventResult::Consumed(None)
      },
      (Key::Down, ..)
      | (Key::Char('j'), false, false, false)
      | (Key::Right, ..)
      | (Key::Char('l'), false, false, false) => {
        self.cursor = 1;
        EventResult::Consumed(None)
      },

      // Quick approve with 'y'
      (Key::Char('y'), false, false, false) => {
        cx.editor.acp_permissions.approve_next();
        if cx.editor.acp_permissions.pending_count() == 0 {
          Self::close_popup()
        } else {
          EventResult::Consumed(None)
        }
      },

      // Quick deny with 'n'
      (Key::Char('n'), false, false, false) => {
        cx.editor.acp_permissions.deny_next();
        if cx.editor.acp_permissions.pending_count() == 0 {
          Self::close_popup()
        } else {
          EventResult::Consumed(None)
        }
      },

      // Confirm selection with Enter
      (Key::Enter | Key::NumpadEnter, ..) => {
        if self.cursor == 0 {
          cx.editor.acp_permissions.approve_next();
        } else {
          cx.editor.acp_permissions.deny_next();
        }
        if cx.editor.acp_permissions.pending_count() == 0 {
          Self::close_popup()
        } else {
          self.cursor = 0; // Reset to Yes for next permission
          EventResult::Consumed(None)
        }
      },

      _ => EventResult::Ignored(None),
    }
  }

  fn render(&mut self, _area: Rect, surface: &mut Surface, cx: &mut Context) {
    let Some(permission) = cx.editor.acp_permissions.peek() else {
      return;
    };

    let font_state = surface.save_font_state();

    self.animation.update(cx.dt);
    let eased = *self.animation.current();
    let alpha = eased;
    let slide_offset = (1.0 - eased) * 8.0;
    let scale = 0.95 + eased * 0.05;

    let theme = &cx.editor.theme;
    let bg_color = theme
      .get("ui.popup")
      .bg
      .map(theme_color_to_renderer_color)
      .unwrap_or(Color::new(0.12, 0.12, 0.15, 1.0));
    let mut text_color = theme
      .get("ui.text")
      .fg
      .map(theme_color_to_renderer_color)
      .unwrap_or(Color::new(0.9, 0.9, 0.9, 1.0));
    let mut selected_fg = theme
      .get("ui.menu.selected")
      .fg
      .map(theme_color_to_renderer_color)
      .unwrap_or(Color::new(1.0, 1.0, 1.0, 1.0));
    let selected_bg = theme
      .get("ui.menu.selected")
      .bg
      .map(theme_color_to_renderer_color)
      .unwrap_or(Color::new(0.25, 0.3, 0.45, 1.0));

    text_color.a *= alpha;
    selected_fg.a *= alpha;

    let Some(cursor_pos) = calculate_cursor_position(cx, surface) else {
      surface.restore_font_state(font_state);
      return;
    };

    surface.configure_font(&font_state.family, UI_FONT_SIZE);
    let char_width = surface.cell_width().max(UI_FONT_WIDTH.max(1.0));
    let line_height = surface.cell_height().max(UI_FONT_SIZE + 4.0);

    // Build the label from the tool call title
    let label = permission.title();
    let label_width = label.chars().count() as f32 * char_width;

    // Menu has: label row, then Yes/No row
    let options = ["Yes (y)", "No (n)"];
    let options_width = options.iter().map(|s| s.len()).max().unwrap_or(6) as f32 * char_width;

    let menu_width = (label_width + HORIZONTAL_PADDING * 2.0)
      .max(options_width * 2.0 + HORIZONTAL_PADDING * 3.0)
      .clamp(MIN_MENU_WIDTH, MAX_MENU_WIDTH);
    let menu_height = (2.0 * line_height) + (VERTICAL_PADDING * 2.0);

    let viewport_width = surface.width() as f32;
    let viewport_height = surface.height() as f32;

    let popup_pos = position_popup_near_cursor(
      cursor_pos,
      menu_width,
      menu_height,
      viewport_width,
      viewport_height,
      slide_offset,
      scale,
      None,
    );

    let anim_width = menu_width * scale;
    let anim_height = menu_height * scale;
    let anim_x = popup_pos.x;
    let anim_y = popup_pos.y;

    surface.draw_rounded_rect(anim_x, anim_y, anim_width, anim_height, 6.0, bg_color);

    surface.with_overlay_region(anim_x, anim_y, anim_width, anim_height, |surface| {
      // Row 1: Permission label
      let y1 = anim_y + VERTICAL_PADDING;
      surface.draw_text(TextSection {
        position: (anim_x + HORIZONTAL_PADDING, y1),
        texts:    vec![TextSegment {
          content: label.to_string(),
          style:   TextStyle {
            size:  UI_FONT_SIZE,
            color: text_color,
          },
        }],
      });

      // Row 2: Yes / No options
      let y2 = y1 + line_height;
      let option_width = (anim_width - HORIZONTAL_PADDING * 2.0) / 2.0;

      for (i, opt) in options.iter().enumerate() {
        let opt_x = anim_x + HORIZONTAL_PADDING + (i as f32 * option_width);
        let is_selected = i == self.cursor;

        if is_selected {
          let mut sel_bg = selected_bg;
          sel_bg.a *= alpha;
          surface.draw_rect(
            opt_x - 4.0,
            y2 - 2.0,
            option_width - 4.0,
            line_height + 4.0,
            sel_bg,
          );
        }

        let fg = if is_selected { selected_fg } else { text_color };
        surface.draw_text(TextSection {
          position: (opt_x, y2),
          texts:    vec![TextSegment {
            content: (*opt).to_string(),
            style:   TextStyle {
              size:  UI_FONT_SIZE,
              color: fg,
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
    Some(Self::ID)
  }

  fn is_animating(&self) -> bool {
    !self.animation.is_complete()
  }
}
