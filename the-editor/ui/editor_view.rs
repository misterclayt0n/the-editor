use ropey::RopeSlice;
use the_editor_renderer::{
  Color,
  TextSection,
};

use crate::{
  core::{
    commands::{
      self,
      MappableCommand,
      OnKeyCallback,
      OnKeyCallbackKind,
    },
    doc_formatter::{
      DocumentFormatter,
      GraphemeSource,
    },
    grapheme::Grapheme,
    graphics::{
      CursorKind,
      Rect,
    },
    position::{
      Position,
      char_idx_at_visual_offset,
      visual_offset_from_block,
    },
  },
  editor::Editor,
  keymap::{
    KeyBinding,
    KeymapResult,
    Keymaps,
    Mode,
  },
  ui::compositor::{
    Callback,
    Component,
    Context,
    Event,
    EventResult,
    Surface,
  },
};

// Constants from the old editor
const VIEW_PADDING_LEFT: f32 = 0.0;
const VIEW_PADDING_TOP: f32 = 0.0;
const VIEW_PADDING_BOTTOM: f32 = 0.0;
const STATUS_BAR_HEIGHT: f32 = 30.0;
const CURSOR_HEIGHT_EXTENSION: f32 = 4.0;
const LINE_SPACING: f32 = 4.0;

pub struct EditorView {
  pub keymaps: Keymaps,
  on_next_key: Option<(OnKeyCallback, OnKeyCallbackKind)>,
  // Track last command for macro replay
  last_insert: (MappableCommand, Vec<KeyBinding>),
}

impl EditorView {
  pub fn new(keymaps: Keymaps) -> Self {
    Self {
      keymaps,
      on_next_key: None,
      last_insert: (MappableCommand::NormalMode, Vec::new()),
    }
  }

  pub fn has_pending_on_next_key(&self) -> bool {
    self.on_next_key.is_some()
  }
}

impl Component for EditorView {
  fn handle_event(&mut self, event: &Event, cx: &mut Context) -> EventResult {
    match event {
      Event::Key(key) => {
        // Check if we're waiting for a key callback
        if let Some((callback, _)) = self.on_next_key.take() {
          // Execute the on_next_key callback
          let mut cmd_cx = commands::Context {
            register:             cx.editor.selected_register,
            count:                cx.editor.count,
            editor:               cx.editor,
            on_next_key_callback: None,
            callback:             Vec::new(),
            jobs:                 cx.jobs,
          };

          // Convert KeyBinding to KeyPress for the callback
          let key_press = the_editor_renderer::KeyPress {
            code:    key.code.clone(),
            pressed: true,
            shift:   key.shift,
            ctrl:    key.ctrl,
            alt:     key.alt,
          };
          callback(&mut cmd_cx, key_press);

          // Process any callbacks generated
          let callbacks = cmd_cx.callback;
          if !callbacks.is_empty() {
            return EventResult::Consumed(Some(Box::new(move |compositor, cx| {
              for callback in callbacks {
                callback(compositor, cx);
              }
            })));
          }

          return EventResult::Consumed(None);
        }

        // Handle insert mode specially - direct character insertion
        if cx.editor.mode() == Mode::Insert {
          // In insert mode, handle character input directly
          match &key.code {
            the_editor_renderer::Key::Char(ch) if !key.ctrl && !key.alt => {
              // Insert the character
              let mut cmd_cx = commands::Context {
                register:             cx.editor.selected_register,
                count:                cx.editor.count,
                editor:               cx.editor,
                on_next_key_callback: None,
                callback:             Vec::new(),
                jobs:                 cx.jobs,
              };

              commands::insert_char(&mut cmd_cx, *ch);
              return EventResult::Consumed(None);
            },
            // the_editor_renderer::Key::Enter => {
            //   // Insert newline
            //   let mut cmd_cx = commands::Context {
            //     register:             cx.editor.selected_register,
            //     count:                cx.editor.count,
            //     editor:               cx.editor,
            //     on_next_key_callback: None,
            //     callback:             Vec::new(),
            //     jobs:                 cx.jobs,
            //   };

            //   // Insert newline using insert_char
            //   commands::insert_char(&mut cmd_cx, '\n');
            //   return EventResult::Consumed(None);
            // },
            // the_editor_renderer::Key::Tab => {
            //   // Insert tab
            //   let mut cmd_cx = commands::Context {
            //     register:             cx.editor.selected_register,
            //     count:                cx.editor.count,
            //     editor:               cx.editor,
            //     on_next_key_callback: None,
            //     callback:             Vec::new(),
            //     jobs:                 cx.jobs,
            //   };

            //   commands::insert_tab(&mut cmd_cx);
            //   return EventResult::Consumed(None);
            // },
            // the_editor_renderer::Key::Backspace => {
            //   // Delete char before cursor
            //   let mut cmd_cx = commands::Context {
            //     register:             cx.editor.selected_register,
            //     count:                cx.editor.count,
            //     editor:               cx.editor,
            //     on_next_key_callback: None,
            //     callback:             Vec::new(),
            //     jobs:                 cx.jobs,
            //   };

            //   commands::delete_char_backward(&mut cmd_cx);
            //   return EventResult::Consumed(None);
            // },
            // the_editor_renderer::Key::Escape => {
            //   // Exit insert mode
            //   cx.editor.set_mode(Mode::Normal);
            //   return EventResult::Consumed(None);
            // },
            _ => {},
          }
        }

        // Convert to KeyPress for keymap lookup
        let key_press = the_editor_renderer::KeyPress {
          code:    key.code.clone(),
          pressed: true,
          shift:   key.shift,
          ctrl:    key.ctrl,
          alt:     key.alt,
        };

        // Process through keymap for non-insert modes
        use crate::keymap::{
          Command,
          KeymapResult,
        };
        match self.keymaps.get(cx.editor.mode(), &key_press) {
          KeymapResult::Matched(Command::Execute(cmd_fn)) => {
            // Save editor state before borrowing
            let register = cx.editor.selected_register;
            let count = cx.editor.count;

            // Create command context
            let mut cmd_cx = commands::Context {
              register,
              count,
              editor: cx.editor,
              on_next_key_callback: None,
              callback: Vec::new(),
              jobs: cx.jobs,
            };

            // Execute the command
            cmd_fn(&mut cmd_cx);

            // Handle on_next_key if set
            if let Some(on_next_key) = cmd_cx.on_next_key_callback {
              self.on_next_key = Some(on_next_key);
            }

            // Extract results (moving callback consumes cmd_cx)
            let new_register = cmd_cx.register;
            let new_count = cmd_cx.count;
            let callbacks = cmd_cx.callback;

            // Update editor state
            cx.editor.selected_register = new_register;
            cx.editor.count = new_count;

            // Process callbacks
            if !callbacks.is_empty() {
              EventResult::Consumed(Some(Box::new(move |compositor, cx| {
                for callback in callbacks {
                  callback(compositor, cx);
                }
              })))
            } else {
              EventResult::Consumed(None)
            }
          },
          KeymapResult::Pending(_) => EventResult::Consumed(None),
          KeymapResult::Cancelled(_) | KeymapResult::NotFound => {
            cx.editor.count = None;
            EventResult::Ignored(None)
          },
        }
      },
      _ => EventResult::Ignored(None),
    }
  }

  fn render(&mut self, _area: Rect, renderer: &mut Surface, cx: &mut Context) {
    let font_size = 22.0; // TODO: Get from config
    renderer.configure_font(&renderer.current_font_family().to_string(), font_size);
    let font_width = renderer.cell_width().max(1.0);

    let available_height = (renderer.height() as f32) - (VIEW_PADDING_TOP + VIEW_PADDING_BOTTOM);
    let available_height = available_height.max(font_size);
    let content_rows = ((available_height / (font_size + LINE_SPACING))
      .floor()
      .max(1.0)) as u16;

    let available_width = (renderer.width() as f32) - (VIEW_PADDING_LEFT * 2.0);
    let available_width = available_width.max(font_width);
    let area_width = (available_width / font_width).floor().max(1.0) as u16;
    let target_area = Rect::new(0, 0, area_width, content_rows.saturating_add(1));

    // Resize tree if needed
    if cx.editor.tree.resize(target_area) {
      let scrolloff = cx.editor.config().scrolloff;
      let view_ids: Vec<_> = cx.editor.tree.views().map(|(view, _)| view.id).collect();
      for view_id in view_ids {
        let view = cx.editor.tree.get_mut(view_id);
        let doc = cx.editor.documents.get_mut(&view.doc).unwrap();
        view.sync_changes(doc);
        view.ensure_cursor_in_view(doc, scrolloff);
      }
    }

    // Get theme colors
    let theme = &cx.editor.theme;
    let background_style = theme.get("ui.background");
    let normal_style = theme.get("ui.text");
    let selection_style = theme.get("ui.selection");
    let cursor_style = theme.get("ui.cursor");

    // Convert theme colors
    let background_color = background_style
      .bg
      .map(crate::ui::theme_color_to_renderer_color)
      .unwrap_or(Color::new(0.1, 0.1, 0.15, 1.0));
    renderer.set_background_color(background_color);

    let normal = normal_style
      .fg
      .map(crate::ui::theme_color_to_renderer_color)
      .unwrap_or(Color::rgb(0.85, 0.85, 0.9));

    let cursor_fg = Color::rgb(0.1, 0.1, 0.15);
    let cursor_bg = cursor_style
      .bg
      .map(crate::ui::theme_color_to_renderer_color)
      .unwrap_or(Color::rgb(0.2, 0.8, 0.7));

    let selection_bg = selection_style
      .bg
      .map(crate::ui::theme_color_to_renderer_color)
      .unwrap_or(Color::rgba(0.3, 0.5, 0.8, 0.3));

    let base_x = VIEW_PADDING_LEFT;
    let base_y = VIEW_PADDING_TOP;

    // Get current view and document
    let focus_view = cx.editor.tree.focus;
    let view = cx.editor.tree.get(focus_view);
    let doc_id = view.doc;
    let doc = &cx.editor.documents[&doc_id];
    let doc_text = doc.text();
    let selection = doc.selection(focus_view);

    let cursor_pos = selection.primary().cursor(doc_text.slice(..));

    // Get viewport information
    let viewport = view.inner_area(doc);
    let visible_lines = content_rows as usize;

    let text_fmt = doc.text_format(viewport.width, None);
    let annotations = view.text_annotations(doc, None);
    let view_offset = doc.view_offset(focus_view);

    let (top_char_idx, _) = char_idx_at_visual_offset(
      doc_text.slice(..),
      view_offset.anchor,
      view_offset.vertical_offset as isize,
      view_offset.horizontal_offset,
      &text_fmt,
      &annotations,
    );

    // Compute row offset
    let row_off = visual_offset_from_block(
      doc_text.slice(..),
      top_char_idx,
      top_char_idx,
      &text_fmt,
      &annotations,
    )
    .0
    .row;

    // Create document formatter
    let mut formatter = DocumentFormatter::new_at_prev_checkpoint(
      doc_text.slice(..),
      &text_fmt,
      &annotations,
      top_char_idx,
    );

    let viewport_cols = viewport.width as usize;

    // Helper: check if document position range overlaps any selection
    let is_selected = |start: usize, len: usize| -> bool {
      if len == 0 {
        return false;
      }
      let end = start + len;
      selection
        .ranges()
        .iter()
        .any(|r| r.from() < end && r.to() > start)
    };

    // Helper function to flush text run
    fn flush_text_run(
      renderer: &mut Surface,
      run_text: &mut String,
      run_start_x: f32,
      run_y: f32,
      font_size: f32,
      run_color: Color,
    ) {
      if !run_text.is_empty() {
        renderer.flush_text_batch();
        let text = std::mem::take(run_text);
        renderer.draw_text_batched(TextSection::simple(
          run_start_x,
          run_y,
          text,
          font_size,
          run_color,
        ));
        renderer.flush_text_batch();
      }
    }

    let mut current_row = usize::MAX;
    let mut run_text = String::new();
    let mut run_start_x = 0.0f32;
    let mut run_y = 0.0f32;
    let mut run_color = normal;

    // Render document graphemes
    while let Some(g) = formatter.next() {
      // Skip visual lines before the top row of the viewport
      if g.visual_pos.row < row_off {
        continue;
      }

      let rel_row = g.visual_pos.row - row_off;
      if rel_row >= visible_lines {
        break;
      }

      // Horizontal scrolling
      let abs_col = g.visual_pos.col;
      let width_cols = g.raw.width();
      let h_off = view_offset.horizontal_offset;

      // Skip if grapheme is left of viewport
      if abs_col + width_cols <= h_off {
        continue;
      }

      // Compute on-screen column
      let rel_col = abs_col.saturating_sub(h_off);
      if rel_col >= viewport_cols {
        continue;
      }

      // Handle partial width at left edge
      let left_clip = if abs_col < h_off { h_off - abs_col } else { 0 };
      let mut draw_cols = width_cols.saturating_sub(left_clip);
      let remaining_cols = viewport_cols.saturating_sub(rel_col);
      if draw_cols > remaining_cols {
        draw_cols = remaining_cols;
      }

      if rel_row != current_row {
        if !run_text.is_empty() {
          flush_text_run(
            renderer,
            &mut run_text,
            run_start_x,
            run_y,
            font_size,
            run_color,
          );
        }
        current_row = rel_row;
      }

      let x = base_x + (rel_col as f32) * font_width;
      let y = base_y + (rel_row as f32) * (font_size + LINE_SPACING);

      // Draw selection background
      let doc_len = g.doc_chars();
      if is_selected(g.char_idx, doc_len) {
        renderer.draw_rect(
          x,
          y,
          (draw_cols as f32) * font_width,
          font_size + LINE_SPACING,
          selection_bg,
        );
      }

      // Draw cursor
      let is_cursor_here = g.char_idx == cursor_pos;
      if is_cursor_here {
        let cursor_w = width_cols.max(1) as f32 * font_width;
        renderer.draw_rect(
          x,
          y,
          cursor_w.min((viewport_cols - rel_col) as f32 * font_width),
          font_size + CURSOR_HEIGHT_EXTENSION,
          cursor_bg,
        );
      }

      // Draw the grapheme
      match g.raw {
        Grapheme::Newline => {
          flush_text_run(
            renderer,
            &mut run_text,
            run_start_x,
            run_y,
            font_size,
            run_color,
          );
        },
        Grapheme::Tab { .. } => {
          flush_text_run(
            renderer,
            &mut run_text,
            run_start_x,
            run_y,
            font_size,
            run_color,
          );
          // Tabs are rendered as spacing, no text to draw
        },
        Grapheme::Other { ref g } => {
          if left_clip > 0 {
            flush_text_run(
              renderer,
              &mut run_text,
              run_start_x,
              run_y,
              font_size,
              run_color,
            );
            continue;
          }

          let fg = if is_cursor_here { cursor_fg } else { normal };

          // Split shaping run at cursor boundary
          if is_cursor_here && !run_text.is_empty() {
            flush_text_run(
              renderer,
              &mut run_text,
              run_start_x,
              run_y,
              font_size,
              run_color,
            );
            renderer.flush_text_batch();
          }

          if run_text.is_empty() {
            run_start_x = x;
            run_y = y;
            run_color = fg;
          } else {
            let color_changed = fg.r != run_color.r
              || fg.g != run_color.g
              || fg.b != run_color.b
              || fg.a != run_color.a;
            if color_changed {
              flush_text_run(
                renderer,
                &mut run_text,
                run_start_x,
                run_y,
                font_size,
                run_color,
              );
              run_start_x = x;
              run_y = y;
              run_color = fg;
            }
          }

          run_text.push_str(&g.to_string());
        },
      }
    }

    // Flush final text run
    flush_text_run(
      renderer,
      &mut run_text,
      run_start_x,
      run_y,
      font_size,
      run_color,
    );
  }

  fn cursor(&self, _area: Rect, _ctx: &Editor) -> (Option<Position>, CursorKind) {
    // TODO: Get cursor position from the current view
    (None, CursorKind::Hidden)
  }
}
