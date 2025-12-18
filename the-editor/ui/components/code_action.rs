use anyhow::{
  anyhow,
  bail,
};
use futures_executor::block_on;
use the_editor_lsp_types::types as lsp;
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
  lsp::LanguageServerId,
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

const MAX_VISIBLE_ITEMS: usize = 12;
const MAX_MENU_WIDTH_CH: u16 = 70;
const HORIZONTAL_PADDING: f32 = 12.0;
const VERTICAL_PADDING: f32 = 10.0;
const MIN_MENU_WIDTH: f32 = 220.0;

pub struct CodeActionEntry {
  pub action:             lsp::CodeActionOrCommand,
  pub language_server_id: LanguageServerId,
}

pub struct CodeActionMenu {
  entries:       Vec<CodeActionEntry>,
  cursor:        usize,
  scroll_offset: usize,
  animation:     AnimationHandle<f32>,
}

impl CodeActionMenu {
  pub const ID: &'static str = "code-action";

  pub fn new(entries: Vec<CodeActionEntry>) -> Self {
    let (duration, easing) = presets::POPUP;
    Self {
      entries,
      cursor: 0,
      scroll_offset: 0,
      animation: AnimationHandle::new(0.0, 1.0, duration, easing),
    }
  }

  fn action_title(entry: &CodeActionEntry) -> &str {
    match &entry.action {
      lsp::CodeActionOrCommand::Command(command) => &command.title,
      lsp::CodeActionOrCommand::CodeAction(action) => &action.title,
    }
  }

  fn action_kind(entry: &CodeActionEntry) -> Option<&str> {
    match &entry.action {
      lsp::CodeActionOrCommand::CodeAction(action) => {
        action.kind.as_ref().map(|kind| kind.as_str())
      },
      _ => None,
    }
  }

  fn is_preferred(entry: &CodeActionEntry) -> bool {
    matches!(
      entry.action,
      lsp::CodeActionOrCommand::CodeAction(lsp::CodeAction {
        is_preferred: Some(true),
        ..
      })
    )
  }

  fn move_cursor(&mut self, delta: isize) {
    if self.entries.is_empty() {
      return;
    }

    let len = self.entries.len() as isize;
    let new_index = (self.cursor as isize + delta).clamp(0, len - 1);
    self.cursor = new_index as usize;
    self.ensure_cursor_visible();
  }

  fn ensure_cursor_visible(&mut self) {
    if self.entries.is_empty() {
      self.scroll_offset = 0;
      return;
    }

    if self.cursor < self.scroll_offset {
      self.scroll_offset = self.cursor;
    } else if self.cursor >= self.scroll_offset + MAX_VISIBLE_ITEMS {
      self.scroll_offset = self.cursor + 1 - MAX_VISIBLE_ITEMS;
    }
  }

  fn visible_range(&self) -> (usize, usize) {
    if self.entries.is_empty() {
      return (0, 0);
    }

    let start = self.scroll_offset.min(self.entries.len().saturating_sub(1));
    let remaining = self.entries.len() - start;
    let visible = remaining.min(MAX_VISIBLE_ITEMS);
    (start, start + visible)
  }

  fn selection(&self) -> Option<&CodeActionEntry> {
    self.entries.get(self.cursor)
  }

  fn apply_selected(&mut self, cx: &mut Context) -> anyhow::Result<()> {
    let entry = self
      .selection()
      .ok_or_else(|| anyhow!("No code action selected"))?;

    let Some(language_server) = cx.editor.language_server_by_id(entry.language_server_id) else {
      cx.editor
        .set_error("Language server not found for code action");
      bail!("language server not found");
    };

    match &entry.action {
      lsp::CodeActionOrCommand::Command(command) => {
        cx.editor
          .execute_lsp_command(command.clone(), entry.language_server_id);
      },
      lsp::CodeActionOrCommand::CodeAction(action) => {
        let mut resolved = None;
        if action.edit.is_none() || action.command.is_none() {
          if let Some(future) = language_server.resolve_code_action(action) {
            match block_on(future) {
              Ok(action) => resolved = Some(action),
              Err(err) => {
                log::error!("Failed to resolve code action: {err}");
              },
            }
          }
        }

        let action = resolved.as_ref().unwrap_or(action);

        if let Some(edit) = &action.edit {
          if let Err(err) = cx
            .editor
            .apply_workspace_edit(language_server.offset_encoding(), edit)
          {
            cx.editor
              .set_error(format!("Failed to apply workspace edit: {}", err.kind));
            bail!("workspace edit failed");
          }
        }

        if let Some(command) = &action.command {
          cx.editor
            .execute_lsp_command(command.clone(), entry.language_server_id);
        }
      },
    }

    cx.editor.set_status("Code action applied");
    Ok(())
  }
}

fn truncate_to_width(text: &str, max_width: f32, char_width: f32) -> String {
  if max_width <= 0.0 {
    return String::new();
  }

  let char_width = char_width.max(1.0);
  let max_chars = (max_width / char_width).floor() as usize;
  if max_chars == 0 {
    return String::new();
  }

  let count = text.chars().count();
  if count <= max_chars {
    return text.to_string();
  }

  if max_chars == 1 {
    return "…".to_string();
  }

  let mut truncated = String::with_capacity(max_chars);
  let mut chars = text.chars();
  for _ in 0..(max_chars - 1) {
    if let Some(ch) = chars.next() {
      truncated.push(ch);
    } else {
      break;
    }
  }
  truncated.push('…');
  truncated
}

impl Component for CodeActionMenu {
  fn handle_event(&mut self, event: &Event, cx: &mut Context) -> EventResult {
    let Event::Key(key) = event else {
      return EventResult::Ignored(None);
    };

    match (key.code, key.ctrl, key.alt, key.shift) {
      (Key::Escape, ..) => {
        EventResult::Consumed(Some(Box::new(|compositor, _| {
          compositor.remove(Self::ID);
        })))
      },
      (Key::Up, ..) | (Key::Char('p'), true, ..) | (Key::Char('k'), false, false, false) => {
        self.move_cursor(-1);
        EventResult::Consumed(None)
      },
      (Key::Down, ..) | (Key::Char('n'), true, ..) | (Key::Char('j'), false, false, false) => {
        self.move_cursor(1);
        EventResult::Consumed(None)
      },
      (Key::PageUp, ..) | (Key::Char('u'), true, ..) => {
        let step = (MAX_VISIBLE_ITEMS / 2).max(1) as isize;
        self.move_cursor(-(step));
        EventResult::Consumed(None)
      },
      (Key::PageDown, ..) | (Key::Char('d'), true, ..) => {
        let step = (MAX_VISIBLE_ITEMS / 2).max(1) as isize;
        self.move_cursor(step);
        EventResult::Consumed(None)
      },
      (Key::Home, ..) => {
        self.cursor = 0;
        self.ensure_cursor_visible();
        EventResult::Consumed(None)
      },
      (Key::End, ..) => {
        if !self.entries.is_empty() {
          self.cursor = self.entries.len() - 1;
          self.ensure_cursor_visible();
        }
        EventResult::Consumed(None)
      },
      (Key::Enter | Key::NumpadEnter, ..) => {
        match self.apply_selected(cx) {
          Ok(()) => {
            EventResult::Consumed(Some(Box::new(|compositor, _| {
              compositor.remove(Self::ID);
            })))
          },
          Err(err) => {
            cx.editor
              .set_error(format!("Failed to apply code action: {err}"));
            EventResult::Consumed(None)
          },
        }
      },
      _ => EventResult::Ignored(None),
    }
  }

  fn render(&mut self, _area: Rect, surface: &mut Surface, cx: &mut Context) {
    if self.entries.is_empty() {
      return;
    }

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

    let Some(cursor) = calculate_cursor_position(cx, surface) else {
      surface.restore_font_state(font_state);
      return;
    };

    surface.configure_font(&font_state.family, UI_FONT_SIZE);
    let char_width = surface.cell_width().max(UI_FONT_WIDTH.max(1.0));
    let line_height = surface.cell_height().max(UI_FONT_SIZE + 4.0);

    let mut max_title_chars: f32 = 0.0;
    let mut max_kind_chars: f32 = 0.0;
    for entry in self.entries.iter().take(40) {
      max_title_chars = max_title_chars.max(Self::action_title(entry).chars().count() as f32);
      if let Some(kind) = Self::action_kind(entry) {
        max_kind_chars = max_kind_chars.max(kind.chars().count() as f32);
      }
    }

    let mut menu_width = HORIZONTAL_PADDING * 2.0 + (max_title_chars.max(20.0) * char_width);
    let mut kind_column_width = 0.0;
    if max_kind_chars > 0.0 {
      kind_column_width = (max_kind_chars + 4.0) * char_width;
      menu_width += kind_column_width;
    }

    let max_width = MAX_MENU_WIDTH_CH as f32 * char_width;
    menu_width = menu_width.clamp(MIN_MENU_WIDTH, max_width);
    if kind_column_width > 0.0 {
      kind_column_width = kind_column_width.min(menu_width * 0.45);
    }

    let mut available_title_width = menu_width - (HORIZONTAL_PADDING * 2.0);
    if kind_column_width > 0.0 {
      available_title_width -= kind_column_width + 8.0;
    }
    if available_title_width < char_width * 4.0 {
      available_title_width = char_width * 4.0;
    }

    let visible_items = self.entries.len().min(MAX_VISIBLE_ITEMS).max(1);
    let menu_height = (visible_items as f32 * line_height) + (VERTICAL_PADDING * 2.0);

    let viewport_width = surface.width() as f32;
    let viewport_height = surface.height() as f32;

    // min_y is the bufferline height (top boundary where popups cannot be placed)
    let min_y = cx.editor.viewport_pixel_offset.1;
    let popup_pos = position_popup_near_cursor(
      cursor,
      menu_width,
      menu_height,
      viewport_width,
      viewport_height,
      min_y,
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
      let (start, end) = self.visible_range();
      let kind_column_x = if kind_column_width > 0.0 {
        Some(anim_x + menu_width - HORIZONTAL_PADDING - kind_column_width)
      } else {
        None
      };

      for (i, entry) in self.entries[start..end].iter().enumerate() {
        let y = anim_y + VERTICAL_PADDING + (i as f32 * line_height);
        let is_selected = start + i == self.cursor;

        if is_selected {
          let mut sel_bg = selected_bg;
          sel_bg.a *= alpha;
          surface.draw_rect(
            anim_x + 4.0,
            y - 2.0,
            anim_width - 8.0,
            line_height + 4.0,
            sel_bg,
          );
        }

        let fg = if is_selected { selected_fg } else { text_color };
        let mut detail = fg;
        detail.a *= 0.8;
        detail.a = detail.a.min(fg.a);

        let title = truncate_to_width(Self::action_title(entry), available_title_width, char_width);
        let title = if Self::is_preferred(entry) {
          format!("★ {title}")
        } else {
          title
        };

        surface.draw_text(TextSection {
          position: (anim_x + HORIZONTAL_PADDING, y),
          texts:    vec![TextSegment {
            content: title,
            style:   TextStyle {
              size:  UI_FONT_SIZE,
              color: fg,
            },
          }],
        });

        if let (Some(kind), Some(kind_x)) = (Self::action_kind(entry), kind_column_x) {
          let kind_text = truncate_to_width(kind, kind_column_width, char_width);
          surface.draw_text(TextSection {
            position: (kind_x, y),
            texts:    vec![TextSegment {
              content: kind_text,
              style:   TextStyle {
                size:  UI_FONT_SIZE,
                color: detail,
              },
            }],
          });
        }
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
