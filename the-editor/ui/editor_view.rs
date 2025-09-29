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
    doc_formatter::DocumentFormatter,
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
  ui::{
    compositor::{
      Component,
      Context,
      Event,
      EventResult,
      Surface,
    },
    render_cache::DirtyRegion,
    render_commands::{
      CommandBatcher,
      RenderCommand,
    },
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
  pub keymaps:         Keymaps,
  on_next_key:         Option<(OnKeyCallback, OnKeyCallbackKind)>,
  // Track last command for macro replay
  last_insert:         (MappableCommand, Vec<KeyBinding>),
  // Rendering optimizations
  dirty_region:        DirtyRegion,
  command_batcher:     CommandBatcher,
  last_cursor_pos:     Option<usize>,
  last_selection_hash: u64,
  // Cursor animation state
  cursor_anim_enabled: bool,
  cursor_lerp_factor:  f32,
  cursor_pos_smooth:   Option<(f32, f32)>,
  cursor_anim_active:  bool,
}

impl EditorView {
  pub fn new(keymaps: Keymaps) -> Self {
    // Defaults; will be overridden from config on first render
    Self {
      keymaps,
      on_next_key: None,
      last_insert: (MappableCommand::NormalMode, Vec::new()),
      dirty_region: DirtyRegion::new(),
      command_batcher: CommandBatcher::new(),
      last_cursor_pos: None,
      last_selection_hash: 0,
      cursor_anim_enabled: true,
      cursor_lerp_factor: 0.25,
      cursor_pos_smooth: None,
      cursor_anim_active: false,
    }
  }

  pub fn has_pending_on_next_key(&self) -> bool {
    self.on_next_key.is_some()
  }
}

impl Component for EditorView {
  fn should_update(&self) -> bool {
    // Redraw only when needed: dirty regions or ongoing cursor animation
    self.dirty_region.needs_redraw() || (self.cursor_anim_enabled && self.cursor_anim_active)
  }

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
              // Mark current line as dirty before insertion
              let focus_view = cx.editor.tree.focus;
              let view = cx.editor.tree.get(focus_view);
              let doc = &cx.editor.documents[&view.doc];
              let cursor_pos = doc
                .selection(focus_view)
                .primary()
                .cursor(doc.text().slice(..));
              let current_line = if cursor_pos < doc.text().len_chars() {
                doc.text().char_to_line(cursor_pos)
              } else {
                doc.text().len_lines().saturating_sub(1)
              };
              self.dirty_region.mark_line_dirty(current_line);

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

              // Mark line as dirty after insertion (may be different if newline was inserted)
              let focus_view = cx.editor.tree.focus;
              let view = cx.editor.tree.get(focus_view);
              let doc = &cx.editor.documents[&view.doc];
              let new_cursor_pos = doc
                .selection(focus_view)
                .primary()
                .cursor(doc.text().slice(..));
              let new_line = if new_cursor_pos < doc.text().len_chars() {
                doc.text().char_to_line(new_cursor_pos)
              } else {
                doc.text().len_lines().saturating_sub(1)
              };
              if new_line != current_line {
                self.dirty_region.mark_line_dirty(new_line);
              }

              return EventResult::Consumed(None);
            },
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

            // Mark affected lines as dirty after command execution
            let focus_view = cx.editor.tree.focus;
            let view = cx.editor.tree.get(focus_view);
            let doc = &cx.editor.documents[&view.doc];
            let new_cursor_pos = doc
              .selection(focus_view)
              .primary()
              .cursor(doc.text().slice(..));
            let new_line = if new_cursor_pos < doc.text().len_chars() {
              doc.text().char_to_line(new_cursor_pos)
            } else {
              doc.text().len_lines().saturating_sub(1)
            };

            // Mark lines as dirty (conservative approach - mark a range around cursor)
            let start_line = new_line.saturating_sub(1);
            let end_line = (new_line + 1).min(doc.text().len_lines().saturating_sub(1));
            self.dirty_region.mark_range_dirty(start_line, end_line);

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
      // Viewport changed, mark everything dirty
      self.dirty_region.mark_all_dirty();
    }

    // Ensure cursor is kept within the viewport including scrolloff padding
    {
      let focus_view = cx.editor.tree.focus;
      let scrolloff = cx.editor.config().scrolloff;
      let view_id_doc;
      {
        // Limit the mutable borrow scope
        let view = cx.editor.tree.get_mut(focus_view);
        let doc = cx.editor.documents.get_mut(&view.doc).unwrap();
        view_id_doc = view.doc;
        if !view.is_cursor_in_view(doc, scrolloff) {
          view.ensure_cursor_in_view(doc, scrolloff);
          // Viewport changed, force a redraw of visible content
          self.dirty_region.mark_all_dirty();
        }
      }
      let _ = view_id_doc; // keep variable to ensure scope is closed
    }

    // Sync cursor animation config from editor config
    {
      let conf = cx.editor.config();
      self.cursor_anim_enabled = conf.cursor_anim_enabled;
      self.cursor_lerp_factor = conf.cursor_lerp_factor;
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

    // Check if cursor or selection changed
    let selection_hash = {
      use std::{
        collections::hash_map::DefaultHasher,
        hash::{
          Hash,
          Hasher,
        },
      };
      let mut hasher = DefaultHasher::new();
      for range in selection.ranges() {
        range.from().hash(&mut hasher);
        range.to().hash(&mut hasher);
      }
      hasher.finish()
    };

    let cursor_changed = self.last_cursor_pos != Some(cursor_pos);
    let selection_changed = self.last_selection_hash != selection_hash;

    if cursor_changed || selection_changed {
      // Only mark cursor-related areas as dirty, not entire viewport
      if let Some(old_cursor) = self.last_cursor_pos {
        // Mark old cursor line dirty
        let old_line = if old_cursor < doc_text.len_chars() {
          doc_text.char_to_line(old_cursor)
        } else {
          doc_text.len_lines().saturating_sub(1)
        };
        self.dirty_region.mark_line_dirty(old_line);
      }
      // Mark new cursor line dirty
      let new_line = if cursor_pos < doc_text.len_chars() {
        doc_text.char_to_line(cursor_pos)
      } else {
        doc_text.len_lines().saturating_sub(1)
      };
      self.dirty_region.mark_line_dirty(new_line);

      self.last_cursor_pos = Some(cursor_pos);
      self.last_selection_hash = selection_hash;
    }

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

    // Check document content
    let doc_len = doc_text.len_chars();

    // Update viewport bounds in dirty region tracker
    self
      .dirty_region
      .set_viewport(row_off, row_off + visible_lines);

    // For now, disable frame timing optimization as it's blocking renders
    // TODO: Fix frame timer logic
    // Always render when we have changes to show

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

    let mut current_row = usize::MAX;
    let mut grapheme_count = 0;
    let mut line_batch = Vec::new(); // Batch characters on the same line

    // Helper to flush a line batch
    let mut flush_line_batch = |batch: &mut Vec<(f32, f32, String, Color)>,
                                batcher: &mut CommandBatcher,
                                font_width: f32,
                                font_size: f32| {
      if batch.is_empty() {
        return;
      }

      // Group consecutive characters with same style
      let mut i = 0;
      while i < batch.len() {
        let (x, y, _, color) = batch[i].clone();
        let mut text = batch[i].2.clone();
        let mut j = i + 1;

        // Track the expected position for next character
        let mut expected_x = x + font_width;

        // Merge consecutive characters with same color at adjacent positions
        while j < batch.len() {
          let (next_x, _, _, next_color) = &batch[j];
          // Check if next character is adjacent and same color (compare RGBA components)
          if (next_x - expected_x).abs() < 1.0
            && next_color.r == color.r
            && next_color.g == color.g
            && next_color.b == color.b
            && next_color.a == color.a
          {
            text.push_str(&batch[j].2);
            expected_x = next_x + font_width;
            j += 1;
          } else {
            break;
          }
        }

        // Render the merged text
        batcher.add_command(RenderCommand::Text {
          section: TextSection::simple(x, y, text, font_size, color),
        });

        i = j;
      }
      batch.clear();
    };

    // Render document graphemes using command batcher
    while let Some(g) = formatter.next() {
      grapheme_count += 1;
      // Skip visual lines before the top row of the viewport
      if g.visual_pos.row < row_off {
        continue;
      }

      let rel_row = g.visual_pos.row - row_off;
      if rel_row >= visible_lines {
        break;
      }

      // For now, disable per-line dirty checking as it's causing rendering issues
      // We'll still benefit from batching and frame timing
      // TODO: Re-enable once we properly track all dirty regions

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

      // Track current row and flush batch on line change
      if rel_row != current_row {
        flush_line_batch(
          &mut line_batch,
          &mut self.command_batcher,
          font_width,
          font_size,
        );
        current_row = rel_row;
      }

      let x = base_x + (rel_col as f32) * font_width;
      let y = base_y + (rel_row as f32) * (font_size + LINE_SPACING);

      // Add selection background command
      let doc_len = g.doc_chars();
      if is_selected(g.char_idx, doc_len) {
        self.command_batcher.add_command(RenderCommand::Selection {
          x,
          y,
          width: (draw_cols as f32) * font_width,
          height: font_size + LINE_SPACING,
          color: selection_bg,
        });
      }

      // Check if this is the cursor position
      let is_cursor_here = g.char_idx == cursor_pos;
      if is_cursor_here {
        let cursor_w = width_cols.max(1) as f32 * font_width;
        // Cursor animation: lerp toward target (x, y)
        let (anim_x, anim_y) = if self.cursor_anim_enabled {
          let (mut sx, mut sy) = self.cursor_pos_smooth.unwrap_or((x, y));
          let dx = x - sx;
          let dy = y - sy;
          sx += dx * self.cursor_lerp_factor;
          sy += dy * self.cursor_lerp_factor;
          self.cursor_pos_smooth = Some((sx, sy));
          // Mark animation active if still far from target
          self.cursor_anim_active = (dx * dx + dy * dy).sqrt() > 0.5;
          (sx, sy)
        } else {
          self.cursor_anim_active = false;
          self.cursor_pos_smooth = Some((x, y));
          (x, y)
        };

        self.command_batcher.add_command(RenderCommand::Cursor {
          x:      anim_x,
          y:      anim_y,
          width:  cursor_w.min((viewport_cols - rel_col) as f32 * font_width),
          height: font_size + CURSOR_HEIGHT_EXTENSION,
          color:  cursor_bg,
        });
      }

      // Add text command
      match g.raw {
        Grapheme::Newline => {
          // End of line, no text to draw
        },
        Grapheme::Tab { .. } => {
          // Tabs are rendered as spacing, no text to draw
        },
        Grapheme::Other { ref g } => {
          if left_clip == 0 {
            let fg = if is_cursor_here { cursor_fg } else { normal };

            // Add to line batch for efficient rendering
            line_batch.push((x, y, g.to_string(), fg));
          }
        },
      }
    }

    // Flush any remaining batch
    flush_line_batch(
      &mut line_batch,
      &mut self.command_batcher,
      font_width,
      font_size,
    );

    // If the document is empty or we didn't render any graphemes, at least render
    // the cursor
    if grapheme_count == 0 {
      // Render cursor at position 0 for empty document
      let x = base_x;
      let y = base_y;
      self.command_batcher.add_command(RenderCommand::Cursor {
        x,
        y,
        width: font_width,
        height: font_size + CURSOR_HEIGHT_EXTENSION,
        color: cursor_bg,
      });
    }

    // Execute all batched commands
    self.command_batcher.execute(renderer);

    // Clear dirty regions after successful render
    self.dirty_region.clear();
  }

  fn cursor(&self, _area: Rect, _ctx: &Editor) -> (Option<Position>, CursorKind) {
    // TODO: Get cursor position from the current view
    (None, CursorKind::Hidden)
  }
}
