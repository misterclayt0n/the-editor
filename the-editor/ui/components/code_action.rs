use the_editor_lsp_types::types as lsp;
use the_editor_renderer::{
  Color,
  TextSection,
  TextSegment,
  TextStyle,
};

use crate::{
  core::{
    graphics::{
      CursorKind,
      Rect,
    },
    position::Position,
    transaction::Transaction,
  },
  editor::Action,
  ui::{
    UI_FONT_SIZE,
    compositor::{
      Component,
      Context,
      Event,
      EventResult,
      Surface,
    },
  },
};

const MAX_VISIBLE_ITEMS: usize = 10;

pub struct CodeActionMenu {
  actions:       Vec<lsp::CodeActionOrCommand>,
  cursor:        usize,
  scroll_offset: usize,
  anim_progress: f32,
}

impl CodeActionMenu {
  pub const ID: &'static str = "code-action";

  pub fn new(actions: Vec<lsp::CodeActionOrCommand>) -> Self {
    Self {
      actions,
      cursor: 0,
      scroll_offset: 0,
      anim_progress: 0.0,
    }
  }

  fn move_cursor(&mut self, delta: isize) {
    let len = self.actions.len();
    if len == 0 {
      return;
    }

    let new_cursor = if delta < 0 {
      self.cursor.saturating_sub(delta.unsigned_abs())
    } else {
      self.cursor.saturating_add(delta as usize)
    };

    self.cursor = new_cursor.min(len - 1);

    // Adjust scroll offset
    if self.cursor < self.scroll_offset {
      self.scroll_offset = self.cursor;
    } else if self.cursor >= self.scroll_offset + MAX_VISIBLE_ITEMS {
      self.scroll_offset = self.cursor - MAX_VISIBLE_ITEMS + 1;
    }
  }

  fn get_action_title(action: &lsp::CodeActionOrCommand) -> String {
    match action {
      lsp::CodeActionOrCommand::Command(cmd) => cmd.title.clone(),
      lsp::CodeActionOrCommand::CodeAction(action) => action.title.clone(),
    }
  }

  fn apply_code_action(&self, cx: &mut Context) -> anyhow::Result<()> {
    let action = &self.actions[self.cursor];

    match action {
      lsp::CodeActionOrCommand::Command(command) => {
        // Execute the command
        log::info!("Executing command: {}", command.command);
        // TODO: Implement workspace command execution
        cx.editor.set_status(format!(
          "Command execution not yet implemented: {}",
          command.command
        ));
      },
      lsp::CodeActionOrCommand::CodeAction(action) => {
        // Apply the workspace edit if present
        if let Some(ref edit) = action.edit {
          self.apply_workspace_edit(cx, edit)?;
        }

        // Execute the command if present
        if let Some(ref command) = action.command {
          log::info!("Executing command: {}", command.command);
          // TODO: Implement workspace command execution
        }
      },
    }

    Ok(())
  }

  fn apply_workspace_edit(
    &self,
    cx: &mut Context,
    edit: &lsp::WorkspaceEdit,
  ) -> anyhow::Result<()> {
    use crate::lsp::util::lsp_range_to_range;

    // Apply document changes
    if let Some(ref changes) = edit.changes {
      for (uri, text_edits) in changes {
        let path = uri
          .to_file_path()
          .map_err(|_| anyhow::anyhow!("Invalid file path"))?;

        // Open or get the document
        let doc_id = cx.editor.open(&path, Action::Replace)?;
        let doc = cx
          .editor
          .documents
          .get_mut(&doc_id)
          .ok_or_else(|| anyhow::anyhow!("Failed to get document"))?;

        // Get the language server to determine offset encoding
        let language_server = doc
          .language_servers_with_feature(
            crate::core::syntax::config::LanguageServerFeature::CodeAction,
          )
          .next()
          .ok_or_else(|| anyhow::anyhow!("No language server"))?;

        let offset_encoding = language_server.offset_encoding();

        // Apply edits in reverse order to maintain offsets
        let mut edits: Vec<_> = text_edits.iter().collect();
        edits.sort_by_key(|edit| std::cmp::Reverse(edit.range.start));

        for edit in edits {
          let text = doc.text();
          if let Some(range) = lsp_range_to_range(text, edit.range, offset_encoding) {
            let transaction = Transaction::change(
              text,
              [(
                range.anchor,
                range.head,
                Some(edit.new_text.as_str().into()),
              )]
              .into_iter(),
            );

            doc.apply(&transaction, cx.editor.tree.focus);
          }
        }
      }
    }

    // TODO: Handle document_changes (which includes more than just text edits)

    cx.editor.set_status("Code action applied");
    Ok(())
  }
}

impl Component for CodeActionMenu {
  fn render(&mut self, area: Rect, surface: &mut Surface, cx: &mut Context) {
    // Animate entrance
    let anim_speed = 12.0;
    if self.anim_progress < 1.0 {
      self.anim_progress = (self.anim_progress + cx.dt * anim_speed).min(1.0);
    }

    let alpha = self.anim_progress;
    let scale = 0.95 + 0.05 * alpha;

    let line_height = UI_FONT_SIZE + 4.0;
    let visible_items = self.actions.len().min(MAX_VISIBLE_ITEMS);
    let menu_height = (visible_items as f32 * line_height) + 16.0;
    let menu_width = 400.0;

    // Get cursor position for positioning
    let (view, doc) = crate::current_ref!(cx.editor);
    let viewport = view.area;
    let text = doc.text().slice(..);
    let cursor = doc.selection(view.id).primary().cursor(text);

    // Simple position calculation (without decorations)
    let line = text.char_to_line(cursor);
    let line_start = text.line_to_char(line);
    let col = cursor - line_start;
    let row = line;

    let Position { row, col } = Position { row, col };

    // Calculate position (below cursor)
    let base_x = viewport.x as f32 + col as f32 * surface.cell_width();
    let base_y = viewport.y as f32 + (row as f32 + 1.0) * line_height;

    // Ensure menu fits in viewport
    let x = base_x.min(area.width as f32 - menu_width);
    let y = if base_y + menu_height > area.height as f32 {
      // Show above cursor if doesn't fit below
      (base_y - line_height - menu_height).max(0.0)
    } else {
      base_y
    };

    let anim_width = menu_width * scale;
    let anim_height = menu_height * scale;
    let anim_x = x + (menu_width - anim_width) / 2.0;
    let anim_y = y + (menu_height - anim_height) / 2.0;

    // Get theme colors
    let theme = &cx.editor.theme;
    let bg_style = theme.get("ui.popup");
    let text_style = theme.get("ui.text");
    let selected_style = theme.get("ui.menu.selected");

    let bg_color = bg_style
      .bg
      .map(crate::ui::theme_color_to_renderer_color)
      .unwrap_or(Color::new(0.1, 0.1, 0.1, 0.95));
    let text_color = text_style
      .fg
      .map(crate::ui::theme_color_to_renderer_color)
      .unwrap_or(Color::new(0.9, 0.9, 0.9, 1.0));
    let selected_bg = selected_style
      .bg
      .map(crate::ui::theme_color_to_renderer_color)
      .unwrap_or(Color::new(0.3, 0.3, 0.4, 1.0));

    let mut bg_color_anim = bg_color;
    bg_color_anim.a *= alpha;

    // Draw background
    surface.draw_rounded_rect(anim_x, anim_y, anim_width, anim_height, 6.0, bg_color_anim);

    // Render in overlay mode
    surface.with_overlay_region(anim_x, anim_y, anim_width, anim_height, |surface| {
      let start = self.scroll_offset;
      let end = (start + visible_items).min(self.actions.len());

      for (i, action) in self.actions[start..end].iter().enumerate() {
        let item_y = anim_y + 8.0 + (i as f32 * line_height);
        let is_selected = start + i == self.cursor;

        // Draw selection background
        if is_selected {
          let mut sel_bg = selected_bg;
          sel_bg.a *= alpha;
          surface.draw_rect(
            anim_x + 4.0,
            item_y - 2.0,
            anim_width - 8.0,
            line_height,
            sel_bg,
          );
        }

        // Draw action title
        let title = Self::get_action_title(action);
        let mut color = text_color;
        color.a *= alpha;

        surface.draw_text(TextSection {
          position: (anim_x + 8.0, item_y),
          texts:    vec![TextSegment {
            content: title,
            style:   TextStyle {
              size: UI_FONT_SIZE,
              color,
            },
          }],
        });
      }
    });
  }

  fn handle_event(&mut self, event: &Event, cx: &mut Context) -> EventResult {
    let Event::Key(key) = event else {
      return EventResult::Ignored(None);
    };

    use the_editor_renderer::Key;

    match (key.code, key.ctrl, key.alt, key.shift) {
      (Key::Escape, ..) => {
        EventResult::Consumed(Some(Box::new(|compositor, _| {
          compositor.remove(Self::ID);
        })))
      },
      (Key::Enter | Key::NumpadEnter, ..) => {
        if let Err(err) = self.apply_code_action(cx) {
          cx.editor
            .set_error(format!("Failed to apply code action: {}", err));
        }
        EventResult::Consumed(Some(Box::new(|compositor, _| {
          compositor.remove(Self::ID);
        })))
      },
      (Key::Up, ..) | (Key::Char('k'), false, ..) => {
        self.move_cursor(-1);
        EventResult::Consumed(None)
      },
      (Key::Down, ..) | (Key::Char('j'), false, ..) => {
        self.move_cursor(1);
        EventResult::Consumed(None)
      },
      _ => EventResult::Ignored(None),
    }
  }

  fn cursor(&self, _area: Rect, _ctx: &crate::editor::Editor) -> (Option<Position>, CursorKind) {
    (None, CursorKind::Hidden)
  }

  fn id(&self) -> Option<&'static str> {
    Some(Self::ID)
  }

  fn is_animating(&self) -> bool {
    self.anim_progress < 1.0
  }
}
