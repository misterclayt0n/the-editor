//! ACP Permission Popup component.
//!
//! Displays pending permission requests from the ACP agent in a popup menu.
//! Users can approve or deny individual permissions or handle them in bulk.

use the_editor_renderer::{
  Color,
  Key,
  TextSection,
  TextSegment,
  TextStyle,
};

use crate::{
  acp::PermissionKind,
  core::{
    animation::{
      AnimationHandle,
      presets,
    },
    graphics::Rect,
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
    theme_color_to_renderer_color,
  },
};

const MAX_VISIBLE_ITEMS: usize = 10;
const HORIZONTAL_PADDING: f32 = 12.0;
const VERTICAL_PADDING: f32 = 10.0;
const MIN_POPUP_WIDTH: f32 = 300.0;
const MAX_POPUP_WIDTH: f32 = 500.0;
const ITEM_HEIGHT: f32 = 24.0;
const HEADER_HEIGHT: f32 = 28.0;
const FOOTER_HEIGHT: f32 = 24.0;

/// Popup component for managing ACP permission requests.
pub struct AcpPermissionPopup {
  cursor:        usize,
  scroll_offset: usize,
  animation:     AnimationHandle<f32>,
}

impl AcpPermissionPopup {
  pub const ID: &'static str = "acp-permission";

  pub fn new() -> Self {
    let (duration, easing) = presets::POPUP;
    Self {
      cursor:        0,
      scroll_offset: 0,
      animation:     AnimationHandle::new(0.0, 1.0, duration, easing),
    }
  }

  fn move_cursor(&mut self, delta: isize, count: usize) {
    if count == 0 {
      return;
    }

    let len = count as isize;
    let new_index = (self.cursor as isize + delta).clamp(0, len - 1);
    self.cursor = new_index as usize;
    self.ensure_cursor_visible(count);
  }

  fn ensure_cursor_visible(&mut self, count: usize) {
    if count == 0 {
      self.scroll_offset = 0;
      return;
    }

    if self.cursor < self.scroll_offset {
      self.scroll_offset = self.cursor;
    } else if self.cursor >= self.scroll_offset + MAX_VISIBLE_ITEMS {
      self.scroll_offset = self.cursor + 1 - MAX_VISIBLE_ITEMS;
    }
  }

  fn visible_range(&self, count: usize) -> (usize, usize) {
    if count == 0 {
      return (0, 0);
    }

    let start = self.scroll_offset.min(count.saturating_sub(1));
    let remaining = count - start;
    let visible = remaining.min(MAX_VISIBLE_ITEMS);
    (start, start + visible)
  }

  fn permission_icon(kind: &PermissionKind) -> &'static str {
    match kind {
      PermissionKind::ReadFile(_) => "R",
      PermissionKind::WriteFile(_) => "W",
      PermissionKind::CreateTerminal => "T",
      PermissionKind::Other(_) => "?",
    }
  }

  fn permission_label(kind: &PermissionKind) -> String {
    match kind {
      PermissionKind::ReadFile(path) => {
        format!(
          "Read: {}",
          path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| path.display().to_string())
        )
      },
      PermissionKind::WriteFile(path) => {
        format!(
          "Write: {}",
          path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| path.display().to_string())
        )
      },
      PermissionKind::CreateTerminal => "Create terminal".to_string(),
      PermissionKind::Other(desc) => desc.clone(),
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
  fn handle_event(&mut self, event: &Event, ctx: &mut Context) -> EventResult {
    let count = ctx.editor.acp_permissions.pending_count();

    // Auto-close if no permissions pending
    if count == 0 {
      return Self::close_popup();
    }

    match event {
      Event::Key(key) => {
        // Clamp cursor to valid range
        if self.cursor >= count {
          self.cursor = count.saturating_sub(1);
        }

        match (key.code, key.ctrl, key.alt, key.shift) {
          // Navigation
          (Key::Char('j') | Key::Down, false, false, false) => {
            self.move_cursor(1, count);
            EventResult::Consumed(None)
          },
          (Key::Char('k') | Key::Up, false, false, false) => {
            self.move_cursor(-1, count);
            EventResult::Consumed(None)
          },

          // Approve selected
          (Key::Char('y') | Key::Enter, false, false, false) => {
            if ctx.editor.acp_permissions.approve_at(self.cursor) {
              ctx.editor.set_status("Permission approved".to_string());
              // Clamp cursor after removal
              let new_count = ctx.editor.acp_permissions.pending_count();
              if self.cursor >= new_count && new_count > 0 {
                self.cursor = new_count - 1;
              }
            }
            // Auto-close if no more permissions
            if ctx.editor.acp_permissions.pending_count() == 0 {
              return Self::close_popup();
            }
            EventResult::Consumed(None)
          },

          // Deny selected
          (Key::Char('n'), false, false, false) => {
            if ctx.editor.acp_permissions.deny_at(self.cursor) {
              ctx.editor.set_status("Permission denied".to_string());
              // Clamp cursor after removal
              let new_count = ctx.editor.acp_permissions.pending_count();
              if self.cursor >= new_count && new_count > 0 {
                self.cursor = new_count - 1;
              }
            }
            // Auto-close if no more permissions
            if ctx.editor.acp_permissions.pending_count() == 0 {
              return Self::close_popup();
            }
            EventResult::Consumed(None)
          },

          // Approve all (Shift+Y)
          (Key::Char('Y'), false, false, true) | (Key::Char('y'), false, false, true) => {
            let approved_count = ctx.editor.acp_permissions.pending_count();
            ctx.editor.acp_permissions.approve_all();
            ctx
              .editor
              .set_status(format!("Approved {} permissions", approved_count));
            Self::close_popup()
          },

          // Deny all (Shift+N)
          (Key::Char('N'), false, false, true) | (Key::Char('n'), false, false, true) => {
            let denied_count = ctx.editor.acp_permissions.pending_count();
            ctx.editor.acp_permissions.deny_all();
            ctx
              .editor
              .set_status(format!("Denied {} permissions", denied_count));
            Self::close_popup()
          },

          // Close without action
          (Key::Escape | Key::Char('q'), false, false, false) => Self::close_popup(),

          _ => EventResult::Ignored(None),
        }
      },
      _ => EventResult::Ignored(None),
    }
  }

  fn render(&mut self, _area: Rect, surface: &mut Surface, ctx: &mut Context) {
    let permissions = ctx.editor.acp_permissions.pending_ref();
    let count = permissions.len();

    if count == 0 {
      return;
    }

    // Update animation
    self.animation.update(ctx.dt);
    let anim_t = *self.animation.current();
    let alpha = anim_t;
    let scale = 0.95 + (anim_t * 0.05);

    // Get theme colors
    let theme = &ctx.editor.theme;
    let bg_style = theme.get("ui.popup");
    let text_style = theme.get("ui.text");
    let selection_style = theme.get("ui.menu.selected");

    let mut bg_color = bg_style
      .bg
      .map(theme_color_to_renderer_color)
      .unwrap_or(Color::new(0.12, 0.12, 0.15, 1.0));
    let mut text_color = text_style
      .fg
      .map(theme_color_to_renderer_color)
      .unwrap_or(Color::new(0.9, 0.9, 0.9, 1.0));
    let mut selection_bg = selection_style
      .bg
      .map(theme_color_to_renderer_color)
      .unwrap_or(Color::new(0.2, 0.3, 0.5, 1.0));
    let mut dim_color = Color::new(0.6, 0.6, 0.6, 1.0);

    bg_color.a *= alpha;
    text_color.a *= alpha;
    selection_bg.a *= alpha;
    dim_color.a *= alpha;

    // Save font state
    let font_state = surface.save_font_state();
    surface.configure_font(&font_state.family, UI_FONT_SIZE);

    let cell_width = surface.cell_width().max(UI_FONT_WIDTH);
    let viewport_width = surface.width() as f32;
    let viewport_height = surface.height() as f32;

    // Calculate dimensions
    let visible_items = count.min(MAX_VISIBLE_ITEMS);
    let content_height =
      HEADER_HEIGHT + (visible_items as f32 * ITEM_HEIGHT) + FOOTER_HEIGHT + VERTICAL_PADDING * 2.0;

    // Calculate max label width
    let max_label_width = permissions
      .iter()
      .map(|p| Self::permission_label(&p.kind).chars().count())
      .max()
      .unwrap_or(20) as f32
      * cell_width;

    let content_width = (max_label_width + HORIZONTAL_PADDING * 4.0 + cell_width * 4.0)
      .clamp(MIN_POPUP_WIDTH, MAX_POPUP_WIDTH);

    // Position popup in center-top area
    let popup_width = content_width * scale;
    let popup_height = content_height * scale;
    let popup_x = (viewport_width - popup_width) / 2.0;
    let popup_y = viewport_height * 0.15; // 15% from top

    // Draw background
    let corner_radius = 8.0;
    surface.draw_rounded_rect(
      popup_x,
      popup_y,
      popup_width,
      popup_height,
      corner_radius,
      bg_color,
    );

    // Draw border
    let mut border_color = Color::new(0.3, 0.3, 0.35, 0.8);
    border_color.a *= alpha;
    surface.draw_rounded_rect_stroke(
      popup_x,
      popup_y,
      popup_width,
      popup_height,
      corner_radius,
      1.0,
      border_color,
    );

    // Draw content
    surface.with_overlay_region(popup_x, popup_y, popup_width, popup_height, |surface| {
      let content_x = popup_x + HORIZONTAL_PADDING;
      let mut y = popup_y + VERTICAL_PADDING;

      // Header
      let header_text = format!("ACP Permissions ({} pending)", count);
      surface.draw_text(TextSection {
        position: (content_x, y + UI_FONT_SIZE),
        texts:    vec![TextSegment {
          content: header_text,
          style:   TextStyle {
            size:  UI_FONT_SIZE,
            color: text_color,
          },
        }],
      });
      y += HEADER_HEIGHT;

      // Separator line
      let mut sep_color = Color::new(0.3, 0.3, 0.35, 0.5);
      sep_color.a *= alpha;
      surface.draw_rect(
        content_x,
        y - 4.0,
        popup_width - HORIZONTAL_PADDING * 2.0,
        1.0,
        sep_color,
      );

      // Permission items
      let (start, end) = self.visible_range(count);
      for (idx, permission) in permissions.iter().enumerate().skip(start).take(end - start) {
        let is_selected = idx == self.cursor;

        // Selection background
        if is_selected {
          surface.draw_rect(
            popup_x + 4.0,
            y,
            popup_width - 8.0,
            ITEM_HEIGHT,
            selection_bg,
          );
        }

        // Icon
        let icon = Self::permission_icon(&permission.kind);
        let icon_color = match &permission.kind {
          PermissionKind::ReadFile(_) => Color::new(0.4, 0.8, 0.4, alpha), // Green for read
          PermissionKind::WriteFile(_) => Color::new(0.9, 0.6, 0.3, alpha), // Orange for write
          PermissionKind::CreateTerminal => Color::new(0.6, 0.6, 0.9, alpha), // Blue for terminal
          PermissionKind::Other(_) => dim_color,
        };

        surface.draw_text(TextSection {
          position: (content_x, y + UI_FONT_SIZE + 2.0),
          texts:    vec![
            TextSegment {
              content: format!("[{}] ", icon),
              style:   TextStyle {
                size:  UI_FONT_SIZE,
                color: icon_color,
              },
            },
            TextSegment {
              content: Self::permission_label(&permission.kind),
              style:   TextStyle {
                size:  UI_FONT_SIZE,
                color: if is_selected { text_color } else { dim_color },
              },
            },
          ],
        });

        y += ITEM_HEIGHT;
      }

      // Separator before footer
      y += 4.0;
      surface.draw_rect(
        content_x,
        y,
        popup_width - HORIZONTAL_PADDING * 2.0,
        1.0,
        sep_color,
      );
      y += 8.0;

      // Footer with keybindings
      let footer_color = Color::new(0.5, 0.5, 0.5, alpha);
      surface.draw_text(TextSection {
        position: (content_x, y + UI_FONT_SIZE - 2.0),
        texts:    vec![TextSegment {
          content: "y:approve  n:deny  Y:all  N:none  esc:close".to_string(),
          style:   TextStyle {
            size:  UI_FONT_SIZE - 2.0,
            color: footer_color,
          },
        }],
      });
    });

    // Restore font state
    surface.restore_font_state(font_state);
  }

  fn required_size(&mut self, _viewport: (u16, u16)) -> Option<(u16, u16)> {
    None // Render in overlay mode
  }

  fn id(&self) -> Option<&'static str> {
    Some(Self::ID)
  }

  fn is_animating(&self) -> bool {
    !self.animation.is_complete()
  }
}
