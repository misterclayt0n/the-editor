use the_editor_renderer::{
  Color,
  TextSection,
};
use the_editor_stdx::rope::RopeSliceExt;

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
    gutter::GutterManager,
    render_cache::DirtyRegion,
    render_commands::{
      CommandBatcher,
      RenderCommand,
    },
  },
};

// Constants from the old editor
const VIEW_PADDING_LEFT: f32 = 0.0; // No visual padding - only scrolloff
const VIEW_PADDING_TOP: f32 = 0.0;
const VIEW_PADDING_BOTTOM: f32 = 0.0; // No reservation - statusbar is now an overlay
const CURSOR_HEIGHT_EXTENSION: f32 = 4.0;
const LINE_SPACING: f32 = 4.0;

pub struct EditorView {
  pub keymaps:          Keymaps,
  on_next_key:          Option<(OnKeyCallback, OnKeyCallbackKind)>,
  // Track last command for macro replay
  last_insert:          (MappableCommand, Vec<KeyBinding>),
  // Rendering optimizations
  dirty_region:         DirtyRegion,
  command_batcher:      CommandBatcher,
  last_cursor_pos:      Option<usize>,
  last_selection_hash:  u64,
  // Cursor animation state
  cursor_anim_enabled:  bool,
  cursor_lerp_factor:   f32,
  cursor_pos_smooth:    Option<(f32, f32)>,
  cursor_anim_active:   bool,
  // Zoom animation state
  zoom_anim_active:     bool,
  // Gutter management
  pub gutter_manager:   GutterManager,
  // Completion popup
  pub(crate) completion: Option<crate::ui::components::Completion>,
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
      zoom_anim_active: false,
      gutter_manager: GutterManager::with_defaults(),
      completion: None,
    }
  }

  pub fn has_pending_on_next_key(&self) -> bool {
    self.on_next_key.is_some()
  }

  /// Set the completion popup with the given items
  pub fn set_completion(
    &mut self,
    editor: &Editor,
    items: Vec<crate::handlers::completion::CompletionItem>,
    trigger_offset: usize,
  ) -> Option<Rect> {
    use crate::ui::components::Completion;

    // Get the initial filter text (text typed since trigger)
    let (view, doc) = crate::current_ref!(editor);
    let text = doc.text();
    let cursor = doc.selection(view.id).primary().cursor(text.slice(..));

    // Calculate filter string from trigger offset to cursor
    let filter = if cursor >= trigger_offset {
      text.slice(trigger_offset..cursor).to_string()
    } else {
      String::new()
    };

    let completion = Completion::new(items, trigger_offset, filter);

    if completion.is_empty() {
      // Skip if we got no completion results
      return None;
    }

    // Store the completion
    self.completion = Some(completion);

    // TODO: Calculate actual area based on cursor position
    Some(Rect::new(0, 0, 60, 15))
  }

  /// Clear the completion popup
  pub fn clear_completion(&mut self, _editor: &mut Editor) {
    self.completion = None;
  }

  /// Mark all visible lines as dirty to force a full redraw
  pub fn mark_all_dirty(&mut self) {
    self.dirty_region.mark_all_dirty();
  }
}

/// Wrapper for syntax highlighting that tracks position and styles
struct SyntaxHighlighter<'h, 'r, 't> {
  inner:      Option<crate::core::syntax::Highlighter<'h>>,
  text:       ropey::RopeSlice<'r>,
  pos:        usize, // Character index of next highlight event
  theme:      &'t crate::core::theme::Theme,
  text_style: crate::core::graphics::Style,
  style:      crate::core::graphics::Style, // Current accumulated style
}

impl<'h, 'r, 't> SyntaxHighlighter<'h, 'r, 't> {
  fn new(
    inner: Option<crate::core::syntax::Highlighter<'h>>,
    text: ropey::RopeSlice<'r>,
    theme: &'t crate::core::theme::Theme,
    text_style: crate::core::graphics::Style,
  ) -> Self {
    let mut highlighter = Self {
      inner,
      text,
      pos: 0,
      theme,
      style: text_style,
      text_style,
    };
    highlighter.update_pos();
    highlighter
  }

  fn update_pos(&mut self) {
    self.pos = self
      .inner
      .as_ref()
      .and_then(|highlighter| {
        let next_byte_idx = highlighter.next_event_offset();
        (next_byte_idx != u32::MAX).then(|| {
          // Move byte index to nearest character boundary and convert to char index
          self
            .text
            .byte_to_char(self.text.ceil_char_boundary(next_byte_idx as usize))
        })
      })
      .unwrap_or(usize::MAX);
  }

  fn advance(&mut self) {
    let Some(highlighter) = self.inner.as_mut() else {
      return;
    };

    use crate::core::syntax::HighlightEvent;
    let (event, highlights) = highlighter.advance();
    let base = match event {
      HighlightEvent::Refresh => self.text_style,
      HighlightEvent::Push => self.style,
    };

    self.style = highlights.fold(base, |acc, highlight| {
      let highlight_style = self.theme.highlight(highlight);
      acc.patch(highlight_style)
    });

    self.update_pos();
  }
}

impl Component for EditorView {
  fn should_update(&self) -> bool {
    // Redraw only when needed: dirty regions, cursor animation, or zoom animation
    self.dirty_region.needs_redraw()
      || (self.cursor_anim_enabled && self.cursor_anim_active)
      || self.zoom_anim_active
  }

  fn handle_event(&mut self, event: &Event, cx: &mut Context) -> EventResult {
    match event {
      Event::Key(key) => {
        // Clear status on any key press
        cx.editor.clear_status();

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
            code:    key.code,
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
          // Let completion handle the event first if it's active
          if let Some(completion) = &mut self.completion {
            // Give completion first chance to handle the event
            match completion.handle_event(event, cx) {
              EventResult::Consumed(callback) => {
                // Completion handled the event
                let should_close = callback.is_some();
                if should_close {
                  // Completion wants to close, clear it
                  self.completion = None;
                  cx.editor.last_completion = None;
                }
                return EventResult::Consumed(callback);
              },
              EventResult::Ignored(_) => {
                // Completion didn't handle it, continue with normal insert mode handling
                // But first, check if we should close completion on certain keys
                match &key.code {
                  // Close completion on navigation or mode-changing keys
                  the_editor_renderer::Key::Escape
                  | the_editor_renderer::Key::Left
                  | the_editor_renderer::Key::Right
                  | the_editor_renderer::Key::Up
                  | the_editor_renderer::Key::Down
                  | the_editor_renderer::Key::PageUp
                  | the_editor_renderer::Key::PageDown
                  | the_editor_renderer::Key::Home
                  | the_editor_renderer::Key::End => {
                    self.completion = None;
                    cx.editor.last_completion = None;
                  },
                  _ => {},
                }
              },
            }
          }

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

              // Extract callbacks before re-borrowing cx.editor
              let callbacks = cmd_cx.callback;

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

              // Process any callbacks generated (e.g., from PostInsertChar hooks)
              if !callbacks.is_empty() {
                return EventResult::Consumed(Some(Box::new(move |compositor, cx| {
                  for callback in callbacks {
                    callback(compositor, cx);
                  }
                })));
              }

              return EventResult::Consumed(None);
            },
            _ => {},
          }
        }

        // Convert to KeyPress for keymap lookup
        let key_press = the_editor_renderer::KeyPress {
          code:    key.code,
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
    let font_size = cx
      .editor
      .font_size_override
      .unwrap_or(cx.editor.config().font_size);
    let font_family = renderer.current_font_family().to_string();
    renderer.configure_font(&font_family, font_size);
    let font_width = renderer.cell_width().max(1.0);

    let available_height = (renderer.height() as f32) - (VIEW_PADDING_TOP + VIEW_PADDING_BOTTOM);
    let available_height = available_height.max(font_size);
    let content_rows = ((available_height / (font_size + LINE_SPACING))
      .floor()
      .max(1.0)) as u16;

    // Don't subtract visual padding from viewport width - it's only for rendering offset
    let available_width = renderer.width() as f32;
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

    let mut normal = normal_style
      .fg
      .map(crate::ui::theme_color_to_renderer_color)
      .unwrap_or(Color::rgb(0.85, 0.85, 0.9));

    // Check if cursor style has reversed modifier
    use crate::core::graphics::Modifier;
    let cursor_reversed = cursor_style.add_modifier.contains(Modifier::REVERSED);

    // Cursor colors from theme
    let cursor_fg_from_theme = cursor_style.fg.map(crate::ui::theme_color_to_renderer_color);
    let cursor_bg_from_theme = cursor_style.bg.map(crate::ui::theme_color_to_renderer_color);

    // If no cursor colors are specified at all, default to reversed behavior (adaptive cursor)
    let use_adaptive_cursor = cursor_reversed || (cursor_fg_from_theme.is_none() && cursor_bg_from_theme.is_none());

    let mut selection_bg = selection_style
      .bg
      .map(crate::ui::theme_color_to_renderer_color)
      .unwrap_or(Color::rgba(0.3, 0.5, 0.8, 0.3));

    // Get current view and document
    let focus_view = cx.editor.tree.focus;

    // Update zoom animation
    let zoom_anim_speed = 20.0; // Fast animation (completes in ~0.05s)
    {
      let view = cx.editor.tree.get_mut(focus_view);
      if view.zoom_anim < 1.0 {
        view.zoom_anim = (view.zoom_anim + cx.dt * zoom_anim_speed).min(1.0);
        self.zoom_anim_active = true;
      } else {
        self.zoom_anim_active = false;
      }
    }

    let view = cx.editor.tree.get(focus_view);
    let zoom_t = view.zoom_anim;
    let zoom_ease = zoom_t * zoom_t * (3.0 - 2.0 * zoom_t); // Smoothstep easing

    // Apply slide-up + fade effect with more pronounced motion
    // Alpha fades in quickly, but slide is more dramatic
    let zoom_alpha = zoom_ease.powf(0.7); // Faster fade-in
    let zoom_offset_y = (1.0 - zoom_ease) * 50.0; // Start 50px below, slide to normal position

    // Apply zoom alpha to all colors for fade-in effect
    normal.a *= zoom_alpha;
    selection_bg.a *= zoom_alpha;

    let base_y = VIEW_PADDING_TOP + zoom_offset_y;

    let doc_id = view.doc;

    // Get cached highlights early while we can still mutably borrow doc
    // (We'll compute the exact range later, but pre-cache a larger range)
    let cached_highlights_opt = {
      let doc = cx.editor.documents.get_mut(&doc_id).unwrap();
      let text = doc.text();
      let view_offset = doc.view_offset(focus_view);
      let row = text.char_to_line(view_offset.anchor.min(text.len_chars()));
      let visible_lines = content_rows as usize;

      // Calculate byte range for visible viewport with some margin
      let start_line = row.saturating_sub(10);
      let end_line = (row + visible_lines + 10).min(text.len_lines());
      let start_byte = text.line_to_byte(start_line);
      let end_byte = if end_line < text.len_lines() {
        text.line_to_byte(end_line)
      } else {
        text.len_bytes()
      };

      let loader = cx.editor.syn_loader.load();
      doc.get_viewport_highlights(start_byte..end_byte, &loader)
    };

    // Wrap main rendering in scope to drop borrows before rendering completion
    {
      let doc = &cx.editor.documents[&doc_id];
      let doc_text = doc.text();
      let selection = doc.selection(focus_view);

    // Calculate gutter offset using GutterManager
    let gutter_width = self.gutter_manager.total_width(view, doc);
    let gutter_offset = gutter_width as f32 * font_width;
    let base_x = VIEW_PADDING_LEFT + gutter_offset;

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
    let cached_highlights = cached_highlights_opt;

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

    // Update viewport bounds in dirty region tracker
    self
      .dirty_region
      .set_viewport(row_off, row_off + visible_lines);

    // For now, disable frame timing optimization as it's blocking renders
    // TODO: Fix frame timer logic
    // Always render when we have changes to show

    // Create document formatter
    let formatter = DocumentFormatter::new_at_prev_checkpoint(
      doc_text.slice(..),
      &text_fmt,
      &annotations,
      top_char_idx,
    );

    // Create syntax highlighter - use cached highlights if available, otherwise
    // create live highlighter
    let syn_loader = cx.editor.syn_loader.load();
    let syntax_highlighter = if cached_highlights.is_none() {
      // No cached highlights, create live highlighter as fallback
      doc.syntax().map(|syntax| {
        let text = doc_text.slice(..);
        let row = text.char_to_line(top_char_idx.min(text.len_chars()));

        // Calculate byte range for visible viewport
        let start_line = row;
        let end_line = (row + visible_lines).min(text.len_lines());
        let start_byte = text.line_to_byte(start_line);
        let end_byte = if end_line < text.len_lines() {
          text.line_to_byte(end_line)
        } else {
          text.len_bytes()
        };

        let range = start_byte as u32..end_byte as u32;
        syntax.highlighter(text, &syn_loader, range)
      })
    } else {
      // We have cached highlights, don't create a live highlighter
      None
    };

    let text_style = normal_style;

    let mut syntax_hl = SyntaxHighlighter::new(
      syntax_highlighter,
      doc_text.slice(..),
      &cx.editor.theme,
      text_style,
    );

    // Debug per document (track by document ID)
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
    let mut rendered_gutter_lines = std::collections::HashSet::new(); // Track which lines have gutters rendered

    // Helper to flush a line batch
    let flush_line_batch = |batch: &mut Vec<(f32, f32, String, Color)>,
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
    for g in formatter {
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
      let left_clip = h_off.saturating_sub(abs_col);
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

      // Render gutter for this line if we haven't already
      let doc_line = doc_text.char_to_line(g.char_idx.min(doc_text.len_chars()));
      if rendered_gutter_lines.insert(doc_line) {
        // This is the first time we're rendering this doc line, so render its gutter
        let cursor_line = doc_text.char_to_line(cursor_pos.min(doc_text.len_chars()));
        let selected = doc_line == cursor_line;

        let line_info = crate::ui::gutter::GutterLineInfo {
          doc_line,
          visual_line: rel_row,
          selected,
          first_visual_line: g.visual_pos.col == 0, // First visual line if at column 0
        };

        self.gutter_manager.render_line(
          &line_info,
          cx.editor,
          doc,
          view,
          &cx.editor.theme,
          renderer,
          VIEW_PADDING_LEFT,
          y,
          font_width,
          font_size,
          normal,
        );
      }

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

      // Get syntax highlighting color first (needed for cursor rendering)
      let syntax_fg = if let Some(ref highlights) = cached_highlights {
        // Use cached highlights - find active highlights at this byte position
        let byte_pos = doc_text.char_to_byte(g.char_idx);
        let mut active_style = text_style;

        for (highlight, range) in highlights {
          if range.contains(&byte_pos) {
            let hl_style = cx.editor.theme.highlight(*highlight);
            active_style = active_style.patch(hl_style);
          }
        }

        active_style.fg
      } else {
        // Use live highlighter
        let mut advance_count = 0;
        while g.char_idx >= syntax_hl.pos {
          syntax_hl.advance();
          advance_count += 1;
          if advance_count > 100 {
            eprintln!(
              "WARNING: Too many advances at char_idx {}, breaking",
              g.char_idx
            );
            break;
          }
        }
        syntax_hl.style.fg
      };

      // Draw cursor if at this position
      if is_cursor_here {
        let cursor_w = width_cols.max(1) as f32 * font_width;
        // Cursor animation: lerp toward target (x, y)
        let (anim_x, anim_y) = if self.cursor_anim_enabled {
          let (mut sx, mut sy) = self.cursor_pos_smooth.unwrap_or((x, y));
          let dx = x - sx;
          let dy = y - sy;
          // Time-based lerp: much faster for snappier cursor while typing
          let lerp_t = 1.0 - (1.0 - self.cursor_lerp_factor).powf(cx.dt * 800.0);
          sx += dx * lerp_t;
          sy += dy * lerp_t;
          self.cursor_pos_smooth = Some((sx, sy));
          // Mark animation active if still far from target
          self.cursor_anim_active = (dx * dx + dy * dy).sqrt() > 0.5;
          (sx, sy)
        } else {
          self.cursor_anim_active = false;
          self.cursor_pos_smooth = Some((x, y));
          (x, y)
        };

        // Determine cursor background color
        let cursor_bg_color = if use_adaptive_cursor {
          // Adaptive/reversed: use character's syntax color as bg
          if let Some(color) = syntax_fg {
            let mut color = crate::ui::theme_color_to_renderer_color(color);
            color.a *= zoom_alpha;
            color
          } else {
            let mut color = normal;
            color.a *= zoom_alpha;
            color
          }
        } else if let Some(mut bg) = cursor_bg_from_theme {
          // Explicit bg from theme
          bg.a *= zoom_alpha;
          bg
        } else {
          // Should not reach here, but default to cyan
          let mut color = Color::rgb(0.2, 0.8, 0.7);
          color.a *= zoom_alpha;
          color
        };

        self.command_batcher.add_command(RenderCommand::Cursor {
          x:      anim_x,
          y:      anim_y,
          width:  cursor_w.min((viewport_cols - rel_col) as f32 * font_width),
          height: font_size + CURSOR_HEIGHT_EXTENSION,
          color:  cursor_bg_color,
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
            // Determine foreground color
            let fg = if is_cursor_here {
              if use_adaptive_cursor {
                // Adaptive/reversed: use background color as fg
                let mut bg = background_color;
                bg.a *= zoom_alpha;
                bg
              } else if let Some(mut cursor_fg) = cursor_fg_from_theme {
                // Explicit fg color in theme
                cursor_fg.a *= zoom_alpha;
                cursor_fg
              } else if let Some(color) = syntax_fg {
                // No explicit fg: use character's syntax color (inverted cursor effect)
                let mut color = crate::ui::theme_color_to_renderer_color(color);
                color.a *= zoom_alpha;
                color
              } else {
                normal
              }
            } else if let Some(color) = syntax_fg {
              let mut color = crate::ui::theme_color_to_renderer_color(color);
              color.a *= zoom_alpha; // Apply zoom fade
              color
            } else {
              normal
            };

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

      // Use default cursor bg for empty document
      let cursor_bg_color = if use_adaptive_cursor {
        // For empty document with adaptive cursor, use normal text color
        let mut color = normal;
        color.a *= zoom_alpha;
        color
      } else if let Some(mut bg) = cursor_bg_from_theme {
        bg.a *= zoom_alpha;
        bg
      } else {
        // Should not reach here, but default to cyan
        let mut color = Color::rgb(0.2, 0.8, 0.7);
        color.a *= zoom_alpha;
        color
      };

      self.command_batcher.add_command(RenderCommand::Cursor {
        x,
        y,
        width: font_width,
        height: font_size + CURSOR_HEIGHT_EXTENSION,
        color: cursor_bg_color,
      });
    }
    } // End scope - drop doc borrow before rendering completion

    // Execute all batched commands
    self.command_batcher.execute(renderer);

    // Render completion popup on top if active
    if let Some(ref mut completion) = self.completion {
      // Position completion popup
      // TODO: Calculate proper position based on cursor location
      let popup_area = Rect::new(0, 0, 60, 15);
      completion.render(popup_area, renderer, cx);
    }

    // Clear dirty regions after successful render
    self.dirty_region.clear();
  }

  fn cursor(&self, _area: Rect, _ctx: &Editor) -> (Option<Position>, CursorKind) {
    // TODO: Get cursor position from the current view
    (None, CursorKind::Hidden)
  }
}
