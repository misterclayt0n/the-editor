use std::time::Instant;

use the_editor_event::request_redraw;
use the_editor_renderer::{
  Color,
  TextSection,
};
use the_editor_stdx::rope::RopeSliceExt;
use the_terminal::{
  ffi::GhosttyCellExt,
  terminal::Cell,
};

use crate::{
  core::{
    animation::selection::{
      self as selection_anim,
    },
    commands::{
      self,
      MappableCommand,
      OnKeyCallback,
      OnKeyCallbackKind,
    },
    doc_formatter::DocumentFormatter,
    grapheme::Grapheme,
    graphics::{
      Color as ThemeColor,
      CursorKind,
      Rect,
    },
    layout::{
      Constraint as LayoutConstraint,
      Layout as UiLayout,
      center,
    },
    position::{
      Position,
      char_idx_at_visual_offset,
      visual_offset_from_block,
    },
    tree::Direction,
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

/// Wrapper around syntax::OverlayHighlighter that maintains position and style
struct OverlayHighlighter<'t> {
  inner: crate::core::syntax::OverlayHighlighter,
  pos:   usize,
  theme: &'t crate::core::theme::Theme,
  style: crate::core::graphics::Style,
}

pub(crate) fn theme_color_to_rgb(color: ThemeColor) -> Option<(u8, u8, u8)> {
  use ThemeColor::*;

  match color {
    Reset => None,
    Black => Some((0, 0, 0)),
    Red => Some((205, 0, 0)),
    Green => Some((0, 205, 0)),
    Yellow => Some((205, 205, 0)),
    Blue => Some((0, 0, 205)),
    Magenta => Some((205, 0, 205)),
    Cyan => Some((0, 205, 205)),
    Gray => Some((127, 127, 127)),
    LightRed => Some((255, 0, 0)),
    LightGreen => Some((0, 255, 0)),
    LightYellow => Some((255, 255, 0)),
    LightBlue => Some((92, 92, 255)),
    LightMagenta => Some((255, 0, 255)),
    LightCyan => Some((0, 255, 255)),
    LightGray => Some((229, 229, 229)),
    White => Some((255, 255, 255)),
    Rgb(r, g, b) => Some((r, g, b)),
    Indexed(i) => Some(ansi256_to_rgb(i)),
  }
}

fn ansi256_to_rgb(index: u8) -> (u8, u8, u8) {
  const ANSI_BASE: [(u8, u8, u8); 16] = [
    (0, 0, 0),
    (205, 0, 0),
    (0, 205, 0),
    (205, 205, 0),
    (0, 0, 238),
    (205, 0, 205),
    (0, 205, 205),
    (229, 229, 229),
    (127, 127, 127),
    (255, 0, 0),
    (0, 255, 0),
    (255, 255, 0),
    (92, 92, 255),
    (255, 0, 255),
    (0, 255, 255),
    (255, 255, 255),
  ];

  match index {
    0..=15 => ANSI_BASE[index as usize],
    16..=231 => {
      let idx = index - 16;
      let r = idx / 36;
      let g = (idx % 36) / 6;
      let b = idx % 6;
      (
        ansi_component_to_rgb(r),
        ansi_component_to_rgb(g),
        ansi_component_to_rgb(b),
      )
    },
    _ => {
      let level = 8 + (index as u16 - 232) * 10;
      let clamped = level.min(255) as u8;
      (clamped, clamped, clamped)
    },
  }
}

fn ansi_component_to_rgb(component: u8) -> u8 {
  if component == 0 {
    0
  } else {
    55 + component * 40
  }
}

impl<'t> OverlayHighlighter<'t> {
  fn new(
    overlays: Vec<crate::core::syntax::OverlayHighlights>,
    theme: &'t crate::core::theme::Theme,
  ) -> Self {
    let inner = crate::core::syntax::OverlayHighlighter::new(overlays);
    let mut highlighter = Self {
      inner,
      pos: 0,
      theme,
      style: crate::core::graphics::Style::default(),
    };
    highlighter.update_pos();
    highlighter
  }

  fn update_pos(&mut self) {
    self.pos = self.inner.next_event_offset();
  }

  fn advance(&mut self) {
    use crate::core::syntax::HighlightEvent;
    let (event, highlights) = self.inner.advance();
    let base = match event {
      HighlightEvent::Refresh => crate::core::graphics::Style::default(),
      HighlightEvent::Push => self.style,
    };

    self.style = highlights.fold(base, |acc, highlight| {
      acc.patch(self.theme.highlight(highlight))
    });
    self.update_pos();
  }

  fn style(&self) -> crate::core::graphics::Style {
    self.style
  }
}

pub struct EditorView {
  pub keymaps:               Keymaps,
  on_next_key:               Option<(OnKeyCallback, OnKeyCallbackKind)>,
  // Track last command for macro replay
  last_insert:               (MappableCommand, Vec<KeyBinding>),
  // Rendering optimizations
  dirty_region:              DirtyRegion,
  command_batcher:           CommandBatcher,
  last_cursor_pos:           Option<usize>,
  last_selection_hash:       u64,
  // Cursor animation
  cursor_animation:          Option<crate::core::animation::AnimationHandle<(f32, f32)>>,
  // Zoom animation state
  zoom_anim_active:          bool,
  selection_pulse_animating: bool,
  noop_effect_animating:     bool,
  // Gutter management
  pub gutter_manager:        GutterManager,
  // Completion popup
  pub(crate) completion:     Option<crate::ui::components::Completion>,
  // Signature help popup
  pub(crate) signature_help: Option<crate::ui::components::SignatureHelp>,
  // Cached font metrics for mouse handling (updated during render)
  cached_cell_width:         f32,
  cached_cell_height:        f32,
  cached_font_size:          f32, // Font size corresponding to cached metrics
  // Mouse drag state for selection
  mouse_pressed:             bool,
  mouse_drag_anchor:         Option<usize>, // Document char index where drag started
  // Multi-click detection (double/triple-click)
  last_click_time:           Option<std::time::Instant>,
  last_click_pos:            Option<(f32, f32)>,
  click_count:               u8,
  // Split separator interaction
  hovered_separator:         Option<SeparatorInfo>,
  dragging_separator:        Option<SeparatorDrag>,
  terminal_meta_pending:     bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct SeparatorInfo {
  /// Which view this separator is attached to
  view_id:    crate::core::ViewId,
  /// Is this a vertical (true) or horizontal (false) separator
  vertical:   bool,
  /// Position in pixels (x for vertical, y for horizontal)
  position:   f32,
  /// View area bounds for hit testing
  view_x:     u16,
  view_y:     u16,
  view_width: u16,
}

#[derive(Debug, Clone, Copy)]
struct SeparatorDrag {
  separator:         SeparatorInfo,
  start_mouse_px:    f32, // Mouse position when drag started (x or y depending on separator type)
  start_split_px:    f32, // Initial separator position in pixels
  accumulated_cells: i32, // Total cells we've already applied
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
      cursor_animation: None,
      zoom_anim_active: false,
      selection_pulse_animating: false,
      noop_effect_animating: false,
      gutter_manager: GutterManager::with_defaults(),
      completion: None,
      signature_help: None,
      cached_cell_width: 12.0,  // Default, will be updated during render
      cached_cell_height: 24.0, // Default, will be updated during render
      cached_font_size: 18.0,   // Default, will be updated during render
      mouse_pressed: false,
      mouse_drag_anchor: None,
      last_click_time: None,
      last_click_pos: None,
      click_count: 0,
      hovered_separator: None,
      dragging_separator: None,
      terminal_meta_pending: false,
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
    _trigger_offset: usize,
  ) -> Option<Rect> {
    use crate::ui::components::Completion;

    // Get the initial filter text (text typed since trigger)
    let (view, doc) = crate::current_ref!(editor);
    let text = doc.text();
    let cursor = doc.selection(view.id).primary().cursor(text.slice(..));

    let slice = text.slice(..);
    let word_prefix_len = slice
      .chars_at(cursor)
      .reversed()
      .take_while(|&ch| crate::core::chars::char_is_word(ch))
      .count();
    let start_offset = cursor.saturating_sub(word_prefix_len);

    // Calculate filter string from trigger offset to cursor
    let filter = text.slice(start_offset..cursor).to_string();

    let completion = Completion::new(items, start_offset, filter);

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

  /// Set signature help popup
  pub fn set_signature_help(
    &mut self,
    language: String,
    active_signature: usize,
    signatures: Vec<crate::handlers::signature_help::Signature>,
  ) {
    if let Some(sig_help) = &mut self.signature_help {
      // Update existing signature help
      sig_help.update(language, active_signature, signatures);
    } else {
      // Create new signature help component
      self.signature_help = Some(crate::ui::components::SignatureHelp::new(
        language,
        active_signature,
        signatures,
      ));
    }
  }

  /// Clear signature help popup
  pub fn clear_signature_help(&mut self) {
    self.signature_help = None;
  }

  /// Simple text wrapping function
  fn wrap_text(text: &str, max_width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current_line = String::new();
    let mut current_width = 0;

    for word in text.split_whitespace() {
      let word_len = word.len();

      if current_width + word_len + 1 > max_width && !current_line.is_empty() {
        // Start new line
        lines.push(current_line);
        current_line = word.to_string();
        current_width = word_len;
      } else {
        // Add to current line
        if !current_line.is_empty() {
          current_line.push(' ');
          current_width += 1;
        }
        current_line.push_str(word);
        current_width += word_len;
      }
    }

    if !current_line.is_empty() {
      lines.push(current_line);
    }

    lines
  }

  /// Render signature help popup

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
    // Redraw only when needed: dirty regions, cursor animation, zoom animation, or
    // popup animations
    self.dirty_region.needs_redraw()
      || self
        .cursor_animation
        .as_ref()
        .is_some_and(|anim| !anim.is_complete())
      || self.zoom_anim_active
      || self.selection_pulse_animating
      || self.noop_effect_animating
      || self.completion.as_ref().is_some_and(|c| c.is_animating())
      || self
        .signature_help
        .as_ref()
        .is_some_and(|s| s.is_animating())
      || self.dragging_separator.is_some()
  }

  fn handle_event(&mut self, event: &Event, cx: &mut Context) -> EventResult {
    match event {
      Event::Key(key) => {
        // Check if focused node is a terminal.
        let focus_id = cx.editor.tree.focus;
        if cx.editor.tree.get_terminal(focus_id).is_some() {
          use the_editor_renderer::Key;

          let mut prepend_escape = false;

          if self.terminal_meta_pending {
            self.terminal_meta_pending = false;
            if let Some(cmd_fn) = Self::terminal_toggle_command(key.code, true) {
              return self.execute_editor_command(cmd_fn, cx);
            } else {
              prepend_escape = true;
            }
          }

          if key.code == Key::Escape && !key.alt && !key.ctrl && !key.shift {
            self.terminal_meta_pending = true;
            return EventResult::Consumed(None);
          }

          if let Some(cmd_fn) = Self::terminal_toggle_command(key.code, key.alt) {
            return self.execute_editor_command(cmd_fn, cx);
          }

          if let Some(terminal) = cx.editor.tree.get_terminal_mut(focus_id) {
            let mut bytes = Vec::new();
            if prepend_escape {
              bytes.push(0x1B);
            }
            bytes.extend(Self::key_to_terminal_bytes(key));
            if !bytes.is_empty() {
              let result = {
                let session_ref = terminal.session.borrow();
                session_ref.send_input(bytes)
              };
              match result {
                Ok(()) => {
                  terminal.session.borrow().mark_needs_redraw();
                  request_redraw();
                },
                Err(e) => {
                  log::error!("Failed to send input to terminal: {}", e);
                },
              }
            }
          }
          return EventResult::Consumed(None);
        }

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

          // Check if callback set a new on_next_key
          if let Some(on_next_key) = cmd_cx.on_next_key_callback {
            self.on_next_key = Some(on_next_key);
          }

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

          // Let signature help handle the event if it's active
          if let Some(sig_help) = &mut self.signature_help {
            match sig_help.handle_event(event, cx) {
              EventResult::Consumed(callback) => {
                // Signature help handled the event (e.g., Escape)
                if callback.is_some() {
                  // Callback will close signature help
                  return EventResult::Consumed(callback);
                }
              },
              EventResult::Ignored(_) => {
                // Signature help didn't handle it, check if we should close on certain keys
                match &key.code {
                  // Close signature help on navigation or mode-changing keys
                  the_editor_renderer::Key::Escape
                  | the_editor_renderer::Key::Left
                  | the_editor_renderer::Key::Right
                  | the_editor_renderer::Key::Up
                  | the_editor_renderer::Key::Down
                  | the_editor_renderer::Key::PageUp
                  | the_editor_renderer::Key::PageDown
                  | the_editor_renderer::Key::Home
                  | the_editor_renderer::Key::End => {
                    self.signature_help = None;
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

            // Ensure cursor visibility and commit history for non-insert commands
            let mode_after = cx.editor.mode();
            let scrolloff = cx.editor.config().scrolloff;
            let (start_line, end_line) = {
              let (view, doc) = crate::current!(cx.editor);
              let text = doc.text();
              let text_slice = text.slice(..);
              let cursor_pos = doc.selection(view.id).primary().cursor(text_slice);
              let len_lines = text.len_lines();
              let len_chars = text.len_chars();
              let current_line = if len_chars == 0 {
                0
              } else if cursor_pos < len_chars {
                text.char_to_line(cursor_pos)
              } else {
                len_lines.saturating_sub(1)
              };

              view.ensure_cursor_in_view(doc, scrolloff);

              if mode_after != Mode::Insert {
                doc.append_changes_to_history(view);
              }

              let start = current_line.saturating_sub(1);
              let end = if len_lines == 0 {
                0
              } else {
                (current_line + 1).min(len_lines.saturating_sub(1))
              };
              (start, end)
            };

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
      Event::Mouse(mouse) => {
        // Note: We don't have access to the renderer here, so cursor icon changes
        // are handled during the next render() call
        self.handle_mouse_event(mouse, cx)
      },
      Event::Scroll(_) => EventResult::Ignored(None),
      _ => EventResult::Ignored(None),
    }
  }

  fn render(&mut self, area: Rect, renderer: &mut Surface, cx: &mut Context) {
    // Separator dimensions (used for clipping and rendering)
    const SEPARATOR_WIDTH_PX: f32 = 2.0;
    const SEPARATOR_HEIGHT_PX: f32 = 2.0;

    let font_size = cx
      .editor
      .font_size_override
      .unwrap_or(cx.editor.config().font_size);
    let font_family = renderer.current_font_family().to_string();
    renderer.configure_font(&font_family, font_size);
    let font_width = renderer.cell_width().max(1.0);

    // Cache font metrics for mouse handling
    self.cached_cell_width = font_width;
    self.cached_cell_height = renderer.cell_height();
    self.cached_font_size = font_size;

    // Calculate tree area from renderer dimensions
    let available_height = (renderer.height() as f32) - (VIEW_PADDING_TOP + VIEW_PADDING_BOTTOM);
    let available_height = available_height.max(font_size);
    let total_rows = ((available_height / self.cached_cell_height)
      .floor()
      .max(1.0)) as u16;

    // Don't subtract visual padding from viewport width - it's only for rendering
    // offset
    let available_width = renderer.width() as f32;
    let available_width = available_width.max(font_width);
    let area_width = (available_width / font_width).floor().max(1.0) as u16;

    // Reserve space at bottom for statusline (clip_bottom reserves 1 row)
    // The statusline is rendered as an overlay by the compositor, but we need to
    // prevent views from rendering underneath it
    let target_area = Rect::new(0, 0, area_width, total_rows).clip_bottom(1);

    // Resize tree if needed
    if cx.editor.tree.resize(target_area) {
      let scrolloff = cx.editor.config().scrolloff;
      let view_ids: Vec<_> = cx.editor.tree.views().map(|(view, _)| view.id).collect();
      for view_id in view_ids {
        // Calculate actual gutter width for this view (accounts for disabled gutters)
        let gutter_width = {
          let view = cx.editor.tree.get(view_id);
          let doc = &cx.editor.documents[&view.doc];
          (self.gutter_manager.total_width(view, doc) as u16).min(view.area.width)
        };

        let view = cx.editor.tree.get_mut(view_id);
        view.rendered_gutter_width = Some(gutter_width);
        let doc = cx.editor.documents.get_mut(&view.doc).unwrap();
        view.sync_changes(doc);
        view.ensure_cursor_in_view(doc, scrolloff);
      }
      // Viewport changed, mark everything dirty
      self.dirty_region.mark_all_dirty();
    }

    // Ensure cursor is kept within the viewport including scrolloff padding
    // (only for document views, not terminals)
    {
      let focus_view = cx.editor.tree.focus;

      // Only process if focused node is a view, not a terminal
      if cx.editor.tree.try_get(focus_view).is_some() {
        let scrolloff = cx.editor.config().scrolloff;

        // Calculate actual gutter width for focused view (accounts for disabled
        // gutters)
        let gutter_width = {
          let view = cx.editor.tree.get(focus_view);
          let doc = &cx.editor.documents[&view.doc];
          (self.gutter_manager.total_width(view, doc) as u16).min(view.area.width)
        };

        let view_id_doc;
        {
          // Limit the mutable borrow scope
          let view = cx.editor.tree.get_mut(focus_view);
          view.rendered_gutter_width = Some(gutter_width);
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
    }

    // Cursor animation config is now read directly from editor config when needed

    // Get theme colors
    let theme = &cx.editor.theme;
    let background_style = theme.get("ui.background");
    let normal_style = theme.get("ui.text");
    let selection_style = theme.get("ui.selection");
    let selection_glow_style = theme.get("ui.selection.glow");
    let cursor_style = theme.get("ui.cursor");

    // Convert theme colors
    let background_color = background_style
      .bg
      .map(crate::ui::theme_color_to_renderer_color)
      .unwrap_or(Color::new(0.1, 0.1, 0.15, 1.0));
    renderer.set_background_color(background_color);

    let normal_base = normal_style
      .fg
      .map(crate::ui::theme_color_to_renderer_color)
      .unwrap_or(Color::rgb(0.85, 0.85, 0.9));

    // Check if cursor style has reversed modifier
    use crate::core::graphics::Modifier;
    let cursor_reversed = cursor_style.add_modifier.contains(Modifier::REVERSED);

    // Cursor colors from theme
    let cursor_fg_from_theme = cursor_style
      .fg
      .map(crate::ui::theme_color_to_renderer_color);
    let cursor_bg_from_theme = cursor_style
      .bg
      .map(crate::ui::theme_color_to_renderer_color);

    // If no cursor colors are specified at all, default to reversed behavior
    // (adaptive cursor)
    let use_adaptive_cursor =
      cursor_reversed || (cursor_fg_from_theme.is_none() && cursor_bg_from_theme.is_none());

    let selection_bg_base = selection_style
      .bg
      .map(crate::ui::theme_color_to_renderer_color)
      .unwrap_or(Color::rgba(0.3, 0.5, 0.8, 0.3));

    let selection_glow_base = selection_glow_style
      .bg
      .map(crate::ui::theme_color_to_renderer_color);

    // Collect all views to render
    let focus_view_id = cx.editor.tree.focus;
    let views_to_render: Vec<_> = cx.editor.tree.traverse().map(|(id, _)| id).collect();
    let now = Instant::now();
    let mut pulses_active_any = false;

    // Update rendered_gutter_width for all views before rendering
    // This ensures cursor positioning logic uses the correct gutter width
    for &view_id in &views_to_render {
      let gutter_width = {
        let view = cx.editor.tree.get(view_id);
        let doc = &cx.editor.documents[&view.doc];
        (self.gutter_manager.total_width(view, doc) as u16).min(view.area.width)
      };
      let view = cx.editor.tree.get_mut(view_id);
      view.rendered_gutter_width = Some(gutter_width);
    }

    // Render each view
    for current_view_id in views_to_render {
      let is_focused = current_view_id == focus_view_id;

      // Clone colors for this view (will be modified by zoom animation)
      let mut normal = normal_base;
      let mut selection_base = selection_bg_base;

      // Get current view and document
      let focus_view = current_view_id;

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
      selection_base.a *= zoom_alpha;

      let mut selection_glow = selection_glow_base.unwrap_or(selection_bg_base);
      selection_glow.a *= zoom_alpha;

      // Get animated area for smooth split transitions
      let view_area = cx
        .editor
        .tree
        .get_animated_area(focus_view)
        .unwrap_or(view.area);

      // Calculate base coordinates from view's area (convert cell coords to pixels)
      let view_offset_x = view_area.x as f32 * font_width;
      let view_offset_y = view_area.y as f32 * (self.cached_cell_height);
      let mut base_y = view_offset_y + VIEW_PADDING_TOP + zoom_offset_y;

      // Calculate visible lines for THIS view based on its height
      // view.area.height is already in rows/cells
      let content_rows = view_area.height;

      // Calculate bottom edge for clipping (to prevent text from rendering into
      // separator)
      let has_horizontal_split_below =
        view_area.y + view_area.height < cx.editor.tree.area().height;
      let has_vertical_split_right = view_area.x + view_area.width < cx.editor.tree.area().width;
      let view_bottom_edge_px = view_offset_y
        + (view_area.height as f32 * (self.cached_cell_height))
        - if has_horizontal_split_below {
          SEPARATOR_HEIGHT_PX
        } else {
          0.0
        };

      let scissor_width = (view_area.width as f32 * font_width)
        - if has_vertical_split_right {
          SEPARATOR_WIDTH_PX
        } else {
          0.0
        };
      let scissor_height = view_bottom_edge_px - view_offset_y;
      renderer.push_scissor_rect(
        view_offset_x,
        view_offset_y,
        scissor_width.max(0.0),
        scissor_height.max(0.0),
      );

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

      let gutter_cols = {
        let doc = &cx.editor.documents[&doc_id];
        (self.gutter_manager.total_width(view, doc) as u16).min(view_area.width)
      };

      let (gutter_rect, mut content_rect) = if gutter_cols > 0 {
        let layout = UiLayout::horizontal().constraints(vec![
          LayoutConstraint::Length(gutter_cols),
          LayoutConstraint::Fill(1),
        ]);
        let mut chunks = layout.split(view_area).into_iter();
        (
          chunks.next().unwrap_or(Rect::new(
            view_area.x,
            view_area.y,
            gutter_cols,
            view_area.height,
          )),
          chunks.next().unwrap_or(Rect::new(
            view_area.x + gutter_cols,
            view_area.y,
            view_area.width.saturating_sub(gutter_cols),
            view_area.height,
          )),
        )
      } else {
        (
          Rect::new(view_area.x, view_area.y, 0, view_area.height),
          Rect::new(view_area.x, view_area.y, view_area.width, view_area.height),
        )
      };

      if content_rect.width == 0 {
        content_rect.width = 1;
      }

      let viewport = Rect::new(
        content_rect.x,
        content_rect.y,
        content_rect.width,
        content_rect.height,
      );

      let mut clear_pulse = false;

      // Wrap main rendering in scope to drop borrows before rendering completion
      {
        let doc = &cx.editor.documents[&doc_id];
        let doc_text = doc.text();
        let selection = doc.selection(focus_view);

        let mut selection_fill_color = selection_base;

        if let Some(pulse) = doc.selection_pulse(focus_view) {
          if let Some(sample) = pulse.sample(now) {
            let frame =
              selection_anim::evaluate_glow(pulse.kind(), selection_base, selection_glow, sample);
            selection_fill_color = frame.color;
            if frame.active {
              pulses_active_any = true;
            }
          } else {
            clear_pulse = true;
          }
        }

        let gutter_x = gutter_rect.x as f32 * font_width + VIEW_PADDING_LEFT;
        let mut base_x = content_rect.x as f32 * font_width + VIEW_PADDING_LEFT;

        // Apply screen shake if active
        let (shake_offset_x, shake_offset_y) = if let Some(shake) = doc.screen_shake(focus_view) {
          if let Some((x, y)) = shake.sample(now) {
            pulses_active_any = true;
            (x, y)
          } else {
            (0.0, 0.0)
          }
        } else {
          (0.0, 0.0)
        };

        base_x += shake_offset_x;
        base_y += shake_offset_y;

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

        // Get viewport information for scrolling calculations (already derived above)
        let visible_lines = viewport.height as usize;
        let cached_highlights = cached_highlights_opt;

        let text_fmt = doc.text_format(viewport.width, None);
        let annotations = view.text_annotations(doc, Some(theme));
        let view_offset = doc.view_offset(focus_view);

        let (top_char_idx, _) = char_idx_at_visual_offset(
          doc_text.slice(..),
          view_offset.anchor,
          view_offset.vertical_offset as isize,
          view_offset.horizontal_offset,
          &text_fmt,
          &annotations,
        );

        // Collect overlay highlights (e.g., jump labels) for the visible range
        let mut overlay_highlighter = {
          let start_char = top_char_idx;
          let end_char =
            (start_char + (visible_lines * viewport.width as usize)).min(doc_text.len_chars());
          let mut overlay_highlights =
            vec![annotations.collect_overlay_highlights(start_char..end_char)];

          // Add ACP message highlighting if this is an ACP buffer
          if doc.is_acp_buffer() && !doc.acp_message_spans.is_empty() {
            use crate::{
              acp::session::MessageRole,
              core::syntax::OverlayHighlights,
            };

            let start_byte = doc_text.char_to_byte(start_char);
            let end_byte = doc_text.char_to_byte(end_char);

            let mut acp_highlights = Vec::new();
            for (role, range) in &doc.acp_message_spans {
              // Only include spans that overlap with the visible range
              if range.end >= start_byte && range.start <= end_byte {
                let highlight_name = match role {
                  MessageRole::User => "acp.user",
                  MessageRole::Agent => "acp.agent",
                  MessageRole::Thinking => "acp.thinking",
                  MessageRole::Tool => "acp.tool",
                };

                if let Some(highlight) = theme.find_highlight(highlight_name) {
                  acp_highlights.push((highlight, range.clone()));
                }
              }
            }

            if !acp_highlights.is_empty() {
              overlay_highlights.push(OverlayHighlights::Heterogenous {
                highlights: acp_highlights,
              });
            }
          }

          OverlayHighlighter::new(overlay_highlights, theme)
        };

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

        // Create decoration manager for inline diagnostics
        let mut decoration_manager = crate::ui::text_decorations::DecorationManager::new();

        // Add diagnostic underlines decoration
        let underlines =
          crate::ui::text_decorations::diagnostic_underlines::DiagnosticUnderlines::new(
            doc,
            &cx.editor.theme,
            base_x,
            base_y,
            self.cached_cell_height,
            font_width,
            font_size,
            view_offset.horizontal_offset,
          );
        decoration_manager.add_decoration(underlines);

        // Add inline diagnostics decoration if enabled
        let inline_diagnostics_config = cx.editor.config().inline_diagnostics.clone();
        let eol_diagnostics = cx.editor.config().end_of_line_diagnostics;
        if !inline_diagnostics_config.disabled() {
          let inline_diag = crate::ui::text_decorations::diagnostics::InlineDiagnostics::new(
            doc,
            &cx.editor.theme,
            cursor_pos,
            inline_diagnostics_config,
            eol_diagnostics,
            base_x,
            base_y,
            self.cached_cell_height,
            font_width,
            viewport.width,
            view_offset.horizontal_offset,
          );
          decoration_manager.add_decoration(inline_diag);
        }

        // Add inlay hints decoration if available
        if let Some(hints) = doc.inlay_hints(focus_view) {
          let inlay_hints_decoration = crate::ui::text_decorations::inlay_hints::InlayHints::new(
            hints,
            &cx.editor.theme,
            cursor_pos,
            viewport.width,
            base_x,
            base_y,
            self.cached_cell_height,
          );
          decoration_manager.add_decoration(inlay_hints_decoration);
        }

        // Add fade decoration if fade mode is enabled
        if cx.editor.fade_mode.enabled {
          if let Some(relevant_ranges) = cx.editor.fade_mode.relevant_ranges.clone() {
            let fade_decoration =
              crate::ui::text_decorations::fade::FadeDecoration::new(relevant_ranges);
            decoration_manager.add_decoration(fade_decoration);
          }
        }

        // Prepare decorations for rendering
        decoration_manager.prepare_for_rendering(top_char_idx);

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

        // Calculate view's right edge in pixels for clipping (accounting for vertical
        // separator)
        let view_right_edge_px = (content_rect.x + content_rect.width) as f32 * font_width
          - if has_vertical_split_right {
            SEPARATOR_WIDTH_PX
          } else {
            0.0
          };

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
        let mut current_doc_line = usize::MAX;
        let mut last_doc_line_end_row = 0; // Track the last visual row of the previous doc_line
        let mut grapheme_count = 0;
        let mut line_batch = Vec::new(); // Batch characters on the same line
        let mut rendered_gutter_lines = std::collections::HashSet::new(); // Track which lines have gutters rendered
        let mut line_end_x = std::collections::HashMap::new(); // Track the rightmost x position for each doc line
        let mut current_line_max_x = base_x; // Track max x for current line

        // Indent guides tracking
        let mut last_line_indent_level = 0usize;
        let mut is_in_indent_area = true;
        let indent_guides_config = cx.editor.config().indent_guides.clone();
        let indent_width = doc.indent_width();
        let indent_guide_char = indent_guides_config.character.to_string();
        let indent_guide_style = cx
          .editor
          .theme
          .try_get("ui.virtual.indent-guide")
          .unwrap_or_else(|| cx.editor.theme.get("ui.virtual.whitespace"));
        let indent_guide_color = indent_guide_style
          .fg
          .map(crate::ui::theme_color_to_renderer_color)
          .unwrap_or(Color::rgb(0.3, 0.3, 0.35));

        // EOL diagnostics are now handled by the decoration system

        // Helper to draw indent guides for a line
        let draw_indent_guides = |last_indent: usize,
                                  rel_row: usize,
                                  batcher: &mut CommandBatcher,
                                  font_width: f32,
                                  font_size: f32,
                                  base_y: f32| {
          if !indent_guides_config.render || last_indent == 0 || indent_width == 0 {
            return;
          }

          let h_off = view_offset.horizontal_offset;
          let skip_levels = indent_guides_config.skip_levels as usize;

          // Calculate starting indent level accounting for horizontal scroll
          let starting_indent = (h_off / indent_width) + skip_levels;
          let end_indent = (last_indent / indent_width)
            .min((h_off + viewport_cols) / indent_width.max(1) + indent_width);

          if starting_indent >= end_indent {
            return;
          }

          let y = base_y + (rel_row as f32) * (self.cached_cell_height);

          // Draw guides at each indent level
          for i in starting_indent..end_indent {
            let guide_x = base_x + ((i * indent_width).saturating_sub(h_off) as f32) * font_width;

            // Only draw if visible in viewport
            if guide_x >= base_x && guide_x < base_x + (viewport_cols as f32) * font_width {
              batcher.add_command(RenderCommand::Text {
                section: TextSection::simple(
                  guide_x,
                  y,
                  indent_guide_char.clone(),
                  font_size,
                  indent_guide_color,
                ),
              });
            }
          }
        };

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

          // Calculate y position early (needed for gutter rendering)
          let y = base_y + (rel_row as f32) * (self.cached_cell_height);

          // Get doc_line early, before horizontal scrolling checks
          let doc_line = doc_text.char_to_line(g.char_idx.min(doc_text.len_chars()));

          // IMPORTANT: Render gutter BEFORE checking horizontal scrolling
          // This ensures gutters are always visible even when content is scrolled
          // horizontally
          if doc_line != current_doc_line {
            // Draw indent guides for the previous line before switching
            if current_doc_line != usize::MAX && last_doc_line_end_row < visible_lines {
              draw_indent_guides(
                last_line_indent_level,
                last_doc_line_end_row,
                &mut self.command_batcher,
                font_width,
                font_size,
                base_y,
              );
            }

            // Render end-of-line diagnostic for previous line before switching
            if current_doc_line != usize::MAX {
              // Render virtual lines for the previous line using the last visual row it ended
              // on
              decoration_manager.render_virtual_lines(
                renderer,
                (current_doc_line, last_doc_line_end_row as u16),
                viewport_cols,
              );
            }

            // Decorate the new line
            decoration_manager.decorate_line(renderer, (doc_line, rel_row as u16));
            current_doc_line = doc_line;
            last_doc_line_end_row = rel_row; // Initialize for the new doc_line
            current_line_max_x = base_x; // Reset for new line

            // Reset indent tracking for new line (but keep last_line_indent_level from
            // previous line) This allows guides to persist on
            // empty/less-indented lines within a block
            is_in_indent_area = true;
          } else {
            // Still on the same doc_line, update the last row we saw content on
            last_doc_line_end_row = rel_row;
          }

          // Render gutter for this line if we haven't already
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
              gutter_x,
              y,
              font_width,
              font_size,
              normal,
            );
          }

          // Track indent level for indent guides
          if is_in_indent_area {
            match &g.raw {
              Grapheme::Tab { .. } => {
                last_line_indent_level = g.visual_pos.col + 1;
              },
              Grapheme::Other { g: ch } => {
                if ch.chars().all(char::is_whitespace) {
                  last_line_indent_level = g.visual_pos.col + 1;
                } else {
                  last_line_indent_level = g.visual_pos.col;
                  is_in_indent_area = false;
                }
              },
              _ => {},
            }
          }

          // NOW check horizontal scrolling (after gutter is rendered)
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

          // Track the rightmost x position on this line
          current_line_max_x = current_line_max_x.max(x + (draw_cols as f32) * font_width);

          // Call decoration hook for this grapheme
          decoration_manager.decorate_grapheme(&g);

          // Add selection background command
          let doc_len = g.doc_chars();
          if is_selected(g.char_idx, doc_len) {
            self.command_batcher.add_command(RenderCommand::Selection {
              x,
              y,
              width: (draw_cols as f32) * font_width,
              height: self.cached_cell_height,
              color: selection_fill_color,
            });
          }

          // Check if this is the cursor position
          let is_cursor_here = g.char_idx == cursor_pos;

          // Advance overlay highlighter
          while g.char_idx >= overlay_highlighter.pos {
            overlay_highlighter.advance();
          }

          // Get text color - check for overlay/virtual text first, then syntax
          // highlighting
          use crate::core::doc_formatter::GraphemeSource;
          let syntax_fg = match g.source {
            GraphemeSource::VirtualText { highlight } => {
              // Use overlay highlight if present
              highlight.and_then(|h| cx.editor.theme.highlight(h).fg)
            },
            GraphemeSource::Document { .. } => {
              // Get syntax highlighting color for document text, then patch with overlay
              let mut active_style = if let Some(ref highlights) = cached_highlights {
                // Use cached highlights - find active highlights at this byte position
                let byte_pos = doc_text.char_to_byte(g.char_idx);
                let mut style = text_style;

                for (highlight, range) in highlights {
                  if range.contains(&byte_pos) {
                    let hl_style = cx.editor.theme.highlight(*highlight);
                    style = style.patch(hl_style);
                  }
                }

                style
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
                syntax_hl.style
              };

              // Patch with overlay highlights (e.g., jump labels)
              active_style = active_style.patch(overlay_highlighter.style());

              active_style.fg
            },
          };

          // Draw cursor if at this position (only for focused view)
          if is_cursor_here && is_focused {
            let cursor_w = width_cols.max(1) as f32 * font_width;
            // Cursor animation using the animation system
            let (anim_x, anim_y) = if cx.editor.config().cursor_anim_enabled {
              // Check if we need to start or retarget the animation
              if let Some(ref mut anim) = self.cursor_animation {
                // Check if target position changed
                if anim.target() != &(x, y) {
                  // Retarget to new position
                  anim.retarget((x, y));
                }
                // Update animation with time delta
                anim.update(cx.dt);
                *anim.current()
              } else {
                // No animation exists, create one using cursor preset
                let (duration, easing) = crate::core::animation::presets::CURSOR;
                let anim =
                  crate::core::animation::AnimationHandle::new((x, y), (x, y), duration, easing);
                let current = *anim.current();
                self.cursor_animation = Some(anim);
                current
              }
            } else {
              // Animation disabled, use exact position
              self.cursor_animation = None;
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

            // Use full cell height without centering (like terminal does)
            let cursor_y = anim_y;

            // Clip cursor to stay within view bounds (both horizontal and vertical)
            let max_cursor_width = (view_right_edge_px - anim_x).max(0.0);
            let clipped_cursor_w = cursor_w.min(max_cursor_width);

            let cursor_height = self.cached_cell_height;
            let max_cursor_height = (view_bottom_edge_px - cursor_y).max(0.0);
            let clipped_cursor_h = cursor_height.min(max_cursor_height);

            self.command_batcher.add_command(RenderCommand::Cursor {
              x:      anim_x,
              y:      cursor_y,
              width:  clipped_cursor_w,
              height: clipped_cursor_h,
              color:  cursor_bg_color,
            });
          }

          // Store char_idx before match (g gets shadowed inside Grapheme::Other)
          let grapheme_char_idx = g.char_idx;

          // Add text command
          match g.raw {
            Grapheme::Newline => {
              // Store the line end x position for this doc line
              line_end_x.insert(doc_line, current_line_max_x);
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

                // Apply fade effect if fade mode is enabled
                let fg = if cx.editor.fade_mode.enabled {
                  if let Some(ref relevant_ranges) = cx.editor.fade_mode.relevant_ranges {
                    if !relevant_ranges.contains(grapheme_char_idx) {
                      // This position is not relevant, apply fade
                      let fade_alpha = 0.3; // 30% opacity for faded text
                      let mut faded = fg;
                      faded.a *= fade_alpha;
                      faded
                    } else {
                      fg
                    }
                  } else {
                    fg
                  }
                } else {
                  fg
                };

                // Add to line batch for efficient rendering, but only if within view bounds
                // Check if text would be within view (not bleeding into separator bars)
                let text_end_x = x + (draw_cols as f32 * font_width);
                let text_bottom_y = y + font_size;
                if x < view_right_edge_px
                  && text_end_x <= view_right_edge_px
                  && text_bottom_y <= view_bottom_edge_px
                {
                  line_batch.push((x, y, g.to_string(), fg));
                }
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

        // Draw indent guides for the last line
        if current_doc_line != usize::MAX && last_doc_line_end_row < visible_lines {
          draw_indent_guides(
            last_line_indent_level,
            last_doc_line_end_row,
            &mut self.command_batcher,
            font_width,
            font_size,
            base_y,
          );
        }

        // Render virtual lines for the last line
        if current_doc_line != usize::MAX {
          decoration_manager.render_virtual_lines(
            renderer,
            (current_doc_line, last_doc_line_end_row as u16),
            viewport_cols,
          );
        }

        // If the document is empty or we didn't render any graphemes, at least render
        // the cursor (only for focused view)
        if grapheme_count == 0 && is_focused {
          // Render cursor at position 0 for empty document
          let x = base_x;
          // Use full cell height without centering (like terminal does)
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
            height: self.cached_cell_height,
            color: cursor_bg_color,
          });
        }
      } // End scope - drop doc borrow before rendering completion

      if clear_pulse {
        if let Some(doc) = cx.editor.documents.get_mut(&doc_id) {
          doc.clear_selection_pulse(focus_view);
        }
      }

      // Render all noop effects (explosions/lasers at multiple positions)
      if let Some(doc) = cx.editor.documents.get(&doc_id) {
        let effects: Vec<_> = doc.noop_effects(focus_view).to_vec();

        // Calculate screen shake offset for effects
        let (shake_offset_x, shake_offset_y) = if let Some(shake) = doc.screen_shake(focus_view) {
          shake.sample(now).unwrap_or((0.0, 0.0))
        } else {
          (0.0, 0.0)
        };

        if !effects.is_empty() {
          let view_offset = doc.view_offset(focus_view);

          for effect in &effects {
            if let Some(progress) = effect.progress(now) {
              // Effect stores visual row/col (stored as screen_x/screen_y)
              let visual_row = effect.screen_x as usize; // screen_x is actually row
              let visual_col = effect.screen_y as usize; // screen_y is actually col

              // Convert visual row/col to screen row/col
              let screen_row = visual_row as isize - view_offset.vertical_offset as isize;
              let screen_col = visual_col.saturating_sub(view_offset.horizontal_offset);

              if screen_row >= 0 && screen_row < viewport.height as isize {
                // Convert screen row/col to pixel coordinates
                let mut effect_base_x = content_rect.x as f32 * font_width + VIEW_PADDING_LEFT;
                let mut effect_base_y = (content_rect.y + screen_row as u16) as f32
                  * (self.cached_cell_height)
                  + VIEW_PADDING_TOP;
                effect_base_x += shake_offset_x;
                effect_base_y += shake_offset_y;
                let effect_x = effect_base_x + screen_col as f32 * font_width;
                let effect_y = effect_base_y;
                let effect_center_x = effect_x + font_width * 0.5;
                let effect_center_y = effect_y + font_size * 0.6;

                // Render noop effects (delete/insert)
                use crate::core::view::NoopEffectKind;
                match effect.kind {
                  NoopEffectKind::Delete => {
                    let num_sparks = 8;
                    let max_distance = font_width * 2.6;
                    let decay = (1.0 - progress).powf(0.6);

                    // Compact flash to emphasise the origin
                    if progress < 0.2 {
                      let flash_strength = (0.2 - progress) / 0.2;
                      let flash_size = font_width * (1.6 + flash_strength * 0.9);
                      self.command_batcher.add_command(RenderCommand::Rect {
                        x:      effect_center_x - flash_size / 2.0,
                        y:      effect_center_y - flash_size / 2.0,
                        width:  flash_size,
                        height: flash_size,
                        color:  Color::rgba(1.0, 0.85, 0.4, flash_strength * 0.7),
                      });
                    }

                    // Glowing ember at the center
                    let core_size = font_width * (0.28 + (1.0 - progress) * 0.35);
                    let core_alpha = (0.85 - progress).max(0.0);
                    self.command_batcher.add_command(RenderCommand::Rect {
                      x:      effect_center_x - core_size / 2.0,
                      y:      effect_center_y - core_size / 2.0,
                      width:  core_size,
                      height: core_size,
                      color:  Color::rgba(1.0, 0.72, 0.33, core_alpha),
                    });

                    // Soft halo expanding outward
                    let halo_alpha = (1.0 - progress).powf(1.5) * 0.4;
                    if halo_alpha > 0.0 {
                      let halo_size = core_size * (2.6 + progress * 1.4);
                      self.command_batcher.add_command(RenderCommand::Rect {
                        x:      effect_center_x - halo_size / 2.0,
                        y:      effect_center_y - halo_size / 2.0,
                        width:  halo_size,
                        height: halo_size,
                        color:  Color::rgba(1.0, 0.55, 0.2, halo_alpha),
                      });
                    }

                    // Sparks travelling outward with subtle trailing embers
                    for spark_idx in 0..num_sparks {
                      let angle = (spark_idx as f32 / num_sparks as f32) * std::f32::consts::TAU;
                      let speed_variation = 0.7 + (spark_idx as f32 * 0.41).sin() * 0.25;
                      let distance = progress.powf(0.85) * max_distance * speed_variation;
                      let spark_x = effect_center_x + angle.cos() * distance;
                      let spark_y = effect_center_y + angle.sin() * distance;

                      let spark_size = font_width * (0.18 + decay * 0.35);
                      let spark_color = Color::rgba(1.0, 0.55 + decay * 0.25, 0.25, decay * 0.9);

                      self.command_batcher.add_command(RenderCommand::Rect {
                        x:      spark_x - spark_size / 2.0,
                        y:      spark_y - spark_size / 2.0,
                        width:  spark_size,
                        height: spark_size,
                        color:  spark_color,
                      });

                      // trailing embers following each spark
                      for trail_step in 1..=3 {
                        let trail_progress = progress - trail_step as f32 * 0.05;
                        if trail_progress <= 0.0 {
                          break;
                        }

                        let trail_distance =
                          trail_progress.powf(0.85) * max_distance * speed_variation;
                        let trail_x = effect_center_x + angle.cos() * trail_distance;
                        let trail_y = effect_center_y + angle.sin() * trail_distance;
                        let trail_alpha = decay * (0.6 / trail_step as f32);

                        self.command_batcher.add_command(RenderCommand::Rect {
                          x:      trail_x - spark_size / 2.0,
                          y:      trail_y - spark_size / 2.0,
                          width:  spark_size,
                          height: spark_size,
                          color:  Color::rgba(0.9, 0.4, 0.15, trail_alpha),
                        });
                      }
                    }
                  },
                  NoopEffectKind::Insert => {
                    let launch_duration = 0.35;
                    let burst_phase =
                      ((progress - launch_duration) / (1.0 - launch_duration)).clamp(0.0, 1.0);
                    let launch_progress = (progress / launch_duration).clamp(0.0, 1.0);

                    // Rocket travels upward from below the line before bursting
                    let start_y = effect_center_y + font_size * 1.0;
                    let rocket_y =
                      start_y - (start_y - effect_center_y) * launch_progress.powf(0.75);
                    let rocket_x = effect_center_x;

                    if progress < launch_duration {
                      // Rocket head
                      let rocket_alpha = (1.0 - progress).powf(0.4);
                      let rocket_size = font_width * 0.18;
                      self.command_batcher.add_command(RenderCommand::Rect {
                        x:      rocket_x - rocket_size / 2.0,
                        y:      rocket_y - rocket_size / 2.0,
                        width:  rocket_size,
                        height: rocket_size,
                        color:  Color::rgba(0.9, 0.95, 1.0, rocket_alpha * 0.9),
                      });

                      // Rocket glow
                      let glow_size = font_width * (0.4 + launch_progress * 0.8);
                      self.command_batcher.add_command(RenderCommand::Rect {
                        x:      rocket_x - glow_size / 2.0,
                        y:      rocket_y - glow_size / 2.0,
                        width:  glow_size,
                        height: glow_size,
                        color:  Color::rgba(0.5, 0.85, 1.0, rocket_alpha * 0.6),
                      });

                      // Trailing sparks along the path
                      for step in 0..5 {
                        let t = step as f32 / 5.0;
                        let tail_alpha = (0.8 - t).max(0.0) * rocket_alpha;
                        if tail_alpha <= 0.0 {
                          continue;
                        }
                        let tail_y = start_y - (start_y - rocket_y) * t;
                        let offset = (step as f32 * 1.2).sin() * font_width * 0.08;
                        let tail_size = font_width * (0.18 - t * 0.08);
                        self.command_batcher.add_command(RenderCommand::Rect {
                          x:      rocket_x + offset - tail_size / 2.0,
                          y:      tail_y - tail_size / 2.0,
                          width:  tail_size,
                          height: tail_size,
                          color:  Color::rgba(0.4, 0.8, 1.0, tail_alpha * 0.7),
                        });
                      }
                    }

                    if burst_phase > 0.0 {
                      let burst_strength = (1.0 - burst_phase).powf(0.4);
                      let burst_radius = font_width * (0.4 + burst_phase.powf(0.65) * 2.6);

                      // Core burst halo
                      let halo_alpha = (1.0 - burst_phase).powf(1.2) * 0.5;
                      let halo_size = burst_radius * 0.9;
                      self.command_batcher.add_command(RenderCommand::Rect {
                        x:      effect_center_x - halo_size / 2.0,
                        y:      effect_center_y - halo_size / 2.0,
                        width:  halo_size,
                        height: halo_size,
                        color:  Color::rgba(0.6, 0.9, 1.0, halo_alpha),
                      });

                      // Firework petals
                      let petals = 14;
                      for i in 0..petals {
                        let angle = (i as f32 / petals as f32) * std::f32::consts::TAU
                          + burst_phase * std::f32::consts::PI;
                        let distance = burst_radius * (0.7 + (i as f32 * 1.37).sin() * 0.2);
                        let spark_x = effect_center_x + angle.cos() * distance;
                        let spark_y = effect_center_y + angle.sin() * distance;

                        let spark_size = font_width * (0.22 + burst_strength * 0.18);
                        let spark_alpha = (1.0 - burst_phase).powf(0.8) * 0.8;
                        self.command_batcher.add_command(RenderCommand::Rect {
                          x:      spark_x - spark_size / 2.0,
                          y:      spark_y - spark_size / 2.0,
                          width:  spark_size,
                          height: spark_size,
                          color:  Color::rgba(0.45, 0.95, 1.0, spark_alpha),
                        });

                        // Petal trails
                        for trail_step in 1..=3 {
                          let trail_factor = 1.0 - trail_step as f32 * 0.25;
                          if trail_factor <= 0.0 {
                            continue;
                          }
                          let trail_distance = distance * trail_factor;
                          let trail_angle = angle - trail_step as f32 * 0.12;
                          let trail_x = effect_center_x + trail_angle.cos() * trail_distance;
                          let trail_y = effect_center_y + trail_angle.sin() * trail_distance;
                          let trail_alpha = spark_alpha * (0.55 / trail_step as f32);

                          self.command_batcher.add_command(RenderCommand::Rect {
                            x:      trail_x - spark_size / 2.5,
                            y:      trail_y - spark_size / 2.5,
                            width:  spark_size / 1.6,
                            height: spark_size / 1.6,
                            color:  Color::rgba(0.35, 0.9, 1.0, trail_alpha.max(0.0)),
                          });
                        }
                      }

                      // Glitter that lingers near the burst
                      let glitter_points = 10;
                      for g in 0..glitter_points {
                        let theta = (g as f32 * 1.73 + burst_phase * 6.0).sin();
                        let radial = burst_radius * 0.6 * (g as f32 * 0.37).cos().abs();
                        let jitter = (g as f32 * 2.1).sin() * font_width * 0.1;
                        let glitter_x = effect_center_x + theta * radial + jitter;
                        let glitter_y = effect_center_y + theta.cos() * radial * 0.7;
                        let glitter_size = font_width * 0.12;
                        let glitter_alpha = (1.0 - burst_phase).powf(0.5) * 0.35;
                        if glitter_alpha > 0.0 {
                          self.command_batcher.add_command(RenderCommand::Rect {
                            x:      glitter_x - glitter_size / 2.0,
                            y:      glitter_y - glitter_size / 2.0,
                            width:  glitter_size,
                            height: glitter_size,
                            color:  Color::rgba(0.75, 0.95, 1.0, glitter_alpha),
                          });
                        }
                      }
                    }
                  },
                }

                // Mark that noop effects are active
                pulses_active_any = true;
              }
            }
          }
        }
      }

      // Clean up expired effects and screen shake (after doc borrow is dropped)
      if let Some(doc_mut) = cx.editor.documents.get_mut(&doc_id) {
        doc_mut.clear_expired_noop_effects(focus_view, now);

        if let Some(shake) = doc_mut.screen_shake(focus_view) {
          if shake.sample(now).is_none() {
            doc_mut.clear_screen_shake(focus_view);
          }
        }
      }

      // Execute draw commands while the view's clipping rect is active.
      self.command_batcher.execute(renderer);
      renderer.pop_scissor_rect();
    } // End view rendering loop

    self.selection_pulse_animating = pulses_active_any;
    self.noop_effect_animating = pulses_active_any;

    // Update cursor icon based on separator hover state
    match &self.hovered_separator {
      Some(sep) => {
        if sep.vertical {
          // Vertical separator - use horizontal resize cursor
          renderer.set_cursor_icon(the_editor_renderer::winit::window::CursorIcon::ColResize);
        } else {
          // Horizontal separator - use vertical resize cursor
          renderer.set_cursor_icon(the_editor_renderer::winit::window::CursorIcon::RowResize);
        }
      },
      None => {
        // Reset to default cursor
        renderer.reset_cursor_icon();
      },
    }

    // Render terminals
    self.render_terminals(renderer, cx, font_width, font_size);

    // Render split separators
    self.render_split_separators(renderer, cx, font_width, font_size);

    // Render completion and signature help popups on top (only for focused view)
    self.render_popups(area, renderer, cx);

    // Clear dirty regions after successful render
    self.dirty_region.clear();
  }

  fn cursor(&self, _area: Rect, _ctx: &Editor) -> (Option<Position>, CursorKind) {
    // TODO: Get cursor position from the current view
    (None, CursorKind::Hidden)
  }
}

impl EditorView {
  fn terminal_toggle_command(
    code: the_editor_renderer::Key,
    alt: bool,
  ) -> Option<fn(&mut commands::Context)> {
    use the_editor_renderer::Key;

    if !alt {
      return None;
    }

    match code {
      Key::Char('j') | Key::Char('J') => Some(commands::toggle_terminal_bottom),
      Key::Char('l') | Key::Char('L') => Some(commands::toggle_terminal_right),
      _ => None,
    }
  }

  fn execute_editor_command(
    &mut self,
    cmd_fn: fn(&mut commands::Context),
    cx: &mut Context,
  ) -> EventResult {
    cx.editor.clear_status();

    let mut cmd_cx = commands::Context {
      register:             cx.editor.selected_register,
      count:                cx.editor.count,
      editor:               cx.editor,
      on_next_key_callback: None,
      callback:             Vec::new(),
      jobs:                 cx.jobs,
    };

    cmd_fn(&mut cmd_cx);

    if let Some(on_next_key) = cmd_cx.on_next_key_callback {
      self.on_next_key = Some(on_next_key);
    }

    let new_register = cmd_cx.register;
    let new_count = cmd_cx.count;
    let callbacks = cmd_cx.callback;

    cx.editor.selected_register = new_register;
    cx.editor.count = new_count;

    self.dirty_region.mark_all_dirty();

    if callbacks.is_empty() {
      EventResult::Consumed(None)
    } else {
      EventResult::Consumed(Some(Box::new(move |compositor, cx| {
        for callback in callbacks {
          callback(compositor, cx);
        }
      })))
    }
  }

  /// Convert a KeyBinding to bytes for terminal PTY input
  fn key_to_terminal_bytes(key: &KeyBinding) -> Vec<u8> {
    use the_editor_renderer::Key;

    let key_code = key.code;
    let ctrl = key.ctrl;
    let alt = key.alt;
    let shift = key.shift;

    // Handle special keys with escape sequences
    match key_code {
      Key::Up => {
        if ctrl {
          b"\x1b[1;5A".to_vec()
        } else if alt {
          b"\x1b[1;3A".to_vec()
        } else {
          b"\x1b[A".to_vec()
        }
      },
      Key::Down => {
        if ctrl {
          b"\x1b[1;5B".to_vec()
        } else if alt {
          b"\x1b[1;3B".to_vec()
        } else {
          b"\x1b[B".to_vec()
        }
      },
      Key::Left => {
        if ctrl {
          b"\x1b[1;5D".to_vec()
        } else if alt {
          b"\x1b[1;3D".to_vec()
        } else {
          b"\x1b[D".to_vec()
        }
      },
      Key::Right => {
        if ctrl {
          b"\x1b[1;5C".to_vec()
        } else if alt {
          b"\x1b[1;3C".to_vec()
        } else {
          b"\x1b[C".to_vec()
        }
      },
      Key::Home => b"\x1b[H".to_vec(),
      Key::End => b"\x1b[F".to_vec(),
      Key::PageUp => b"\x1b[5~".to_vec(),
      Key::PageDown => b"\x1b[6~".to_vec(),
      Key::Tab => {
        if shift {
          b"\x1b[Z".to_vec() // Shift+Tab
        } else {
          b"\t".to_vec()
        }
      },
      Key::Backspace => b"\x7f".to_vec(),
      Key::Delete => b"\x1b[3~".to_vec(),
      Key::Enter => b"\r".to_vec(),
      Key::Escape => b"\x1b".to_vec(),
      Key::Char(c) => {
        let mut bytes = Vec::new();

        if ctrl {
          // Ctrl+key produces control character
          match c {
            'a'..='z' => bytes.push((c as u8) - b'a' + 1),
            'A'..='Z' => bytes.push((c as u8) - b'A' + 1),
            '[' => bytes.push(0x1B),
            _ => bytes.extend_from_slice(c.to_string().as_bytes()),
          }
        } else if alt {
          // Alt+key produces ESC key
          bytes.push(0x1B);
          bytes.extend_from_slice(c.to_string().as_bytes());
        } else {
          bytes.extend_from_slice(c.to_string().as_bytes());
        }

        bytes
      },
      _ => Vec::new(), // Other keys ignored for now
    }
  }

  /// Render all terminal nodes in the tree
  fn render_terminals(
    &mut self,
    renderer: &mut Surface,
    cx: &mut Context,
    font_width: f32,
    font_size: f32,
  ) {
    use the_editor_renderer::{
      Color,
      TextSection,
      TextSegment,
    };

    // Helper function for Powerline symbol detection
    let is_powerline_symbol = |ch: char| -> bool {
      matches!(ch, '\u{E0B0}'..='\u{E0D4}')
    };

    // Collect terminal IDs to avoid borrowing issues
    let terminal_ids: Vec<_> = cx.editor.tree.terminals().map(|(id, _)| id).collect();

    for term_id in terminal_ids {
      // Get terminal area (might be animated)
      let term_area = cx
        .editor
        .tree
        .get_animated_area(term_id)
        .or_else(|| cx.editor.tree.get_terminal_mut(term_id).map(|t| t.area));

      let Some(term_area) = term_area else {
        continue;
      };

      // Calculate pixel coordinates from cell coordinates
      // Use renderer's cell_height to match cosmic-text's metrics
      let term_x = term_area.x as f32 * font_width;
      let term_y = term_area.y as f32 * renderer.cell_height();

      // Get mutable reference to terminal
      let terminal = match cx.editor.tree.get_terminal_mut(term_id) {
        Some(t) => t,
        None => continue,
      };

      let background_rgb = cx
        .editor
        .theme
        .try_get("ui.background")
        .and_then(|style| style.bg)
        .and_then(theme_color_to_rgb);

      // Update session metadata (cell size, background color)
      {
        let mut session = terminal.session.borrow_mut();
        session.set_cell_pixel_size(font_width, renderer.cell_height());
        if let Some((r, g, b)) = background_rgb {
          session.set_background_color(r, g, b);
        }
      }

      // Calculate terminal dimensions in cells
      let new_cols = term_area.width;
      let new_rows = term_area.height;

      if new_cols == 0 || new_rows == 0 {
        // Keep dirty flags intact until the pane has a drawable area.
        continue;
      }

      // Resize terminal if dimensions changed
      let (current_rows, current_cols) = terminal.session.borrow().size();
      if new_cols != current_cols || new_rows != current_rows {
        if new_cols > 0 && new_rows > 0 {
          if let Err(e) = terminal.session.borrow_mut().resize(new_rows, new_cols) {
            log::error!("Failed to resize terminal {}: {}", terminal.id, e);
          }
        }
      }

      // Always repaint the full terminal area. The renderer clears the backing
      // surface each frame, so partial (dirty row) repaints would drop lines
      // and cause flicker after toggling panes.
      let session_borrow = terminal.session.borrow();
      session_borrow.clear_dirty_bits();

      // Lock terminal for rendering (separate lock, dirty bits already cleared)
      let term_guard = session_borrow.lock_terminal();
      let grid = term_guard.grid();
      let (grid_rows, grid_cols) = (grid.rows(), grid.cols());

      // Clamp rendering to avoid overflow
      let render_rows = grid_rows.min(new_rows);
      let render_cols = grid_cols.min(new_cols);

      // Use renderer's cell_height for consistent metrics with cosmic-text
      let line_height = renderer.cell_height();

      // Repaint every row to keep the surface populated.
      let is_full_render = true;

      let mut raw_row = vec![GhosttyCellExt::default(); render_cols as usize];

      // PASS 0: Render default background (ghostty's approach)
      // This provides a base layer so cells without explicit backgrounds (bg=None)
      // naturally show the default color underneath. Matches ghostty's rendering
      // pipeline.
      let default_bg = term_guard
        .get_default_background()
        .or_else(|| background_rgb.map(|(r, g, b)| the_terminal::terminal::Rgb { r, g, b }));

      if let Some(bg) = default_bg {
        let bg_color = Color::rgba(
          bg.r as f32 / 255.0,
          bg.g as f32 / 255.0,
          bg.b as f32 / 255.0,
          1.0,
        );

        // Render full terminal area with default background
        let term_width = render_cols as f32 * font_width;
        let term_height = render_rows as f32 * line_height;
        renderer.draw_rect(term_x, term_y, term_width, term_height, bg_color);
      }

      // PASS 1: Render cell backgrounds (selection + explicit backgrounds)
      // Only cells with explicit backgrounds (or selected/inverse) will render on top
      for row in 0..render_rows {
        let row_y = term_y + (row as f32 * line_height);
        let _ = term_guard.copy_row_ext(row, raw_row.as_mut_slice());

        for (col_idx, cell_ext) in raw_row.iter().enumerate() {
          let col = col_idx as u16;
          if col >= render_cols {
            break;
          }

          let cell: Cell = (*cell_ext).into();

          // Skip wide character continuation cells (width = 0)
          if cell.width == 0 {
            continue;
          }

          // Determine background color
          // Note: Colors are already swapped in wrapper.zig for inverse cells,
          // so we just use cell.bg directly (don't swap again!)
          let bg_to_render = if cell.selected {
            // Cell is selected - use foreground as selection background (ghostty default)
            Some(cell.fg)
          } else {
            // Normal: use explicit background if set (already swapped for inverse in Zig)
            cell.bg
          };

          // Render background if we have a color
          if let Some(bg) = bg_to_render {
            let cell_x = term_x + (col_idx as f32 * font_width);
            let bg_color = Color::rgba(
              bg.r as f32 / 255.0,
              bg.g as f32 / 255.0,
              bg.b as f32 / 255.0,
              1.0,
            );

            // Wide characters get proportionally wider backgrounds
            let bg_width = font_width * (cell.width.max(1) as f32);
            renderer.draw_rect(cell_x, row_y, bg_width, line_height, bg_color);
          }
        }
      }

      // SECOND PASS: Render text on top of backgrounds
      // Render rows as contiguous color runs to reduce draw calls
      for row in 0..render_rows {
        let row_y = term_y + (row as f32 * line_height);
        let mut run_text = String::with_capacity(render_cols as usize);
        let mut run_color: Option<(u8, u8, u8)> = None;
        let mut run_start_col = 0u16;

        let flush_run = |renderer: &mut Surface,
                         start_col: u16,
                         color: Option<(u8, u8, u8)>,
                         buffer: &mut String| {
          if buffer.is_empty() {
            return;
          }
          while buffer.ends_with(' ') {
            buffer.pop();
          }
          if buffer.is_empty() {
            return;
          }
          let Some((r, g, b)) = color else {
            buffer.clear();
            return;
          };

          let fg_color = Color::rgba(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0);
          let text = std::mem::take(buffer);
          let x = term_x + (start_col as f32 * font_width);
          // TextArea.top expects the top of the line - cosmic-text handles baseline internally
          let mut section = TextSection::new(x, row_y);
          section = section.add_text(
            TextSegment::new(text)
              .with_color(fg_color)
              .with_size(font_size),
          );
          renderer.draw_text(section);
          buffer.reserve(render_cols as usize);
        };

        let _ = term_guard.copy_row_ext(row, raw_row.as_mut_slice());

        for (col_idx, cell_ext) in raw_row.iter().enumerate() {
          let col = col_idx as u16;
          if col >= render_cols {
            break;
          }

          let cell: Cell = (*cell_ext).into();
          let mut ch = cell.character().unwrap_or(' ');
          if ch == '\0' {
            ch = ' ';
          }

          // Determine text color
          // Note: For inverse cells, wrapper.zig already swapped fg/bg,
          // so cell.fg now contains the correct text color. Don't swap again!
          let text_color = if cell.selected {
            // Selected: use background (original bg color) for text
            cell.bg.unwrap_or(cell.fg)
          } else {
            // Normal and inverse: use fg (already swapped for inverse in wrapper.zig)
            cell.fg
          };

          let rgb = (text_color.r, text_color.g, text_color.b);
          let cell_width = cell.width;
          let is_wide_continuation = cell_width == 0;
          if is_wide_continuation {
            continue;
          }

          // Check if this is a Powerline symbol that needs custom rendering
          if is_powerline_symbol(ch) {
            // Flush any pending text run before drawing the Powerline symbol
            flush_run(renderer, run_start_col, run_color, &mut run_text);

            // Draw the Powerline symbol using the renderer's built-in method
            let x = term_x + (col as f32 * font_width);
            let fg_color = Color::rgba(rgb.0 as f32 / 255.0, rgb.1 as f32 / 255.0, rgb.2 as f32 / 255.0, 1.0);
            renderer.draw_powerline_glyph(ch, x, row_y, font_width, line_height, fg_color);

            // Reset run for next text segment
            run_color = None;
            run_start_col = col + 1;
            continue;
          }

          if run_color.map(|current| current != rgb).unwrap_or(true) {
            flush_run(renderer, run_start_col, run_color, &mut run_text);
            run_color = Some(rgb);
            run_start_col = col;
          }

          run_text.push(ch);

          let glyph_width = usize::from(cell_width.max(1));
          if glyph_width > 1 {
            for _ in 1..glyph_width {
              run_text.push(' ');
            }
          }
        }

        flush_run(renderer, run_start_col, run_color, &mut run_text);
      }

      // Render cursor ONLY if visible AND viewport is at bottom (ghostty's approach)
      // - DECTCEM mode (CSI ?25h/l) controls cursor visibility
      // - Viewport position check prevents cursor rendering when scrolled back in
      //   history
      let cursor_visible = term_guard.is_cursor_visible();
      let viewport_at_bottom = term_guard.is_viewport_at_bottom();
      let (cursor_row, cursor_col) = term_guard.cursor_pos();

      if cursor_visible && viewport_at_bottom && cursor_row < grid_rows && cursor_col < grid_cols {
        let cursor_x = term_x + (cursor_col as f32 * font_width);

        // Add centering offset to match cosmic-text's vertical text positioning
        let glyph_height = font_size;
        let centering_offset = (line_height - glyph_height) / 2.0;
        let cursor_y = term_y + (cursor_row as f32 * line_height) + centering_offset;

        // Draw cursor as a semi-transparent rectangle
        renderer.draw_rect(
          cursor_x,
          cursor_y,
          font_width,
          glyph_height,
          Color::new(0.8, 0.8, 0.8, 0.5),
        );
      }

      // Drop the terminal guard to release the lock
      drop(term_guard);

      // Clear flags (dirty bits already cleared atomically above)
      if is_full_render {
        session_borrow.clear_full_render_flag();
      }
      session_borrow.clear_redraw_flag();
    }
  }

  /// Render split separator bars between views
  fn render_split_separators(
    &mut self,
    renderer: &mut Surface,
    cx: &Context,
    font_width: f32,
    font_size: f32,
  ) {
    // Get separator color from theme
    let theme = &cx.editor.theme;
    let separator_style = theme.get("ui.window");
    let separator_color = separator_style
      .bg
      .or(separator_style.fg)
      .map(crate::ui::theme_color_to_renderer_color)
      .unwrap_or(Color::rgba(0.3, 0.3, 0.4, 0.8));

    // Separator constants for this function scope
    const SEPARATOR_WIDTH_PX: f32 = 2.0;
    const SEPARATOR_HEIGHT_PX: f32 = 2.0;

    let tree = &cx.editor.tree;
    for (view, _) in tree.views() {
      let area = view.area;
      let has_vertical_neighbor = tree
        .find_split_in_direction(view.id, Direction::Right)
        .is_some();
      if has_vertical_neighbor {
        // Render vertical separator bar at the right edge
        // Center the thin separator in the gap
        let gap_center_x = (area.x + area.width) as f32 * font_width + (font_width / 2.0);
        let x = gap_center_x - (SEPARATOR_WIDTH_PX / 2.0);
        let y = area.y as f32 * (self.cached_cell_height);
        let width = SEPARATOR_WIDTH_PX;
        let height = area.height as f32 * (self.cached_cell_height);

        renderer.draw_rect(x, y, width, height, separator_color);
      }

      let has_horizontal_neighbor = tree
        .find_split_in_direction(view.id, Direction::Down)
        .is_some();
      if has_horizontal_neighbor {
        // Render horizontal separator bar at the bottom edge
        let x = area.x as f32 * font_width;
        let sep_y =
          (area.y + area.height) as f32 * (self.cached_cell_height) - SEPARATOR_HEIGHT_PX;
        let width = area.width as f32 * font_width;
        let height = SEPARATOR_HEIGHT_PX;

        renderer.draw_rect(x, sep_y, width, height, separator_color);
      }
    }
  }

  /// Render completion and signature help popups for the focused view
  fn render_popups(&mut self, area: Rect, renderer: &mut Surface, cx: &mut Context) {
    // Render completion popup on top if active
    if let Some(ref mut completion) = self.completion {
      // Position completion popup centered in viewport
      let popup_area = center(area, 60, 15);
      completion.render(popup_area, renderer, cx);
    }

    // Render signature help popup if active
    if let Some(ref mut sig_help) = self.signature_help {
      sig_help.render(area, renderer, cx);
    }
  }

  /// Handle mouse events (clicks, drags, etc.)
  fn handle_mouse_event(
    &mut self,
    mouse: &the_editor_renderer::MouseEvent,
    cx: &mut Context,
  ) -> EventResult {
    match mouse.button {
      Some(the_editor_renderer::MouseButton::Left) => {
        if mouse.pressed {
          // Check if clicking on a separator first
          if let Some(separator) = self.detect_separator_hover(mouse.position, cx) {
            // Start separator drag
            let start_coord = if separator.vertical {
              mouse.position.0
            } else {
              mouse.position.1
            };
            self.dragging_separator = Some(SeparatorDrag {
              separator,
              start_mouse_px: start_coord,
              start_split_px: separator.position,
              accumulated_cells: 0,
            });
            return EventResult::Consumed(None);
          }

          // Detect multi-click (double/triple-click)
          let now = std::time::Instant::now();
          let is_multi_click = if let (Some(last_time), Some(last_pos)) =
            (self.last_click_time, self.last_click_pos)
          {
            let time_delta = now.duration_since(last_time);
            let pos_delta = ((mouse.position.0 - last_pos.0).powi(2)
              + (mouse.position.1 - last_pos.1).powi(2))
            .sqrt();

            // Within 500ms and 5 pixels = multi-click
            time_delta.as_millis() < 500 && pos_delta < 5.0
          } else {
            false
          };

          if is_multi_click {
            self.click_count = (self.click_count + 1).min(3); // Cap at triple-click
          } else {
            self.click_count = 1;
          }

          self.last_click_time = Some(now);
          self.last_click_pos = Some(mouse.position);

          // First check which node (view or terminal) was clicked
          if let Some(node_id) = self.screen_coords_to_node(mouse.position, cx) {
            // Switch focus to the clicked node if different
            if cx.editor.tree.focus != node_id {
              cx.editor.focus(node_id);
            }

            // If it's a view, handle text selection
            if let Some((view_id, doc_pos)) = self.screen_coords_to_doc_pos(mouse.position, cx) {
              let scrolloff = cx.editor.config().scrolloff;

              // Mark drag as started (for potential drag after click)
              self.mouse_pressed = true;
              self.mouse_drag_anchor = Some(doc_pos);

              let view = cx.editor.tree.get(view_id);
              let doc_id = view.doc;
              let doc = cx.editor.documents.get_mut(&doc_id).unwrap();

              // Create selection based on click count
              let selection = match self.click_count {
                1 => {
                  // Single click - point selection
                  crate::core::selection::Selection::point(doc_pos)
                },
                2 => {
                  // Double-click - select word
                  let text = doc.text();
                  let range = crate::core::selection::Range::point(doc_pos);
                  let word_range = crate::core::textobject::textobject_word(
                    text.slice(..),
                    range,
                    crate::core::textobject::TextObject::Around,
                    1,
                    false, // short word (not WORD)
                  );
                  crate::core::selection::Selection::single(word_range.anchor, word_range.head)
                },
                3 => {
                  // Triple-click - select line
                  let text = doc.text();
                  let line = text.char_to_line(doc_pos.min(text.len_chars()));
                  let start = text.line_to_char(line);
                  let end = text.line_to_char((line + 1).min(text.len_lines()));
                  crate::core::selection::Selection::single(start, end)
                },
                _ => crate::core::selection::Selection::point(doc_pos),
              };

              doc.set_selection(view_id, selection);

              // Ensure cursor remains visible
              let view = cx.editor.tree.get_mut(view_id);
              view.ensure_cursor_in_view(doc, scrolloff);
            }
            // If it's a terminal, we've already switched focus above
            // TODO: Future enhancement - send mouse events to terminal

            return EventResult::Consumed(None);
          }
        } else {
          // Mouse button released - end drag
          self.mouse_pressed = false;
          self.mouse_drag_anchor = None;
          self.dragging_separator = None; // End separator drag
          return EventResult::Consumed(None);
        }
      },
      Some(the_editor_renderer::MouseButton::Middle) => {
        // Middle-click - paste from clipboard (only works on views, not terminals)
        if mouse.pressed {
          // First check which node was clicked
          if let Some(node_id) = self.screen_coords_to_node(mouse.position, cx) {
            // Switch focus to clicked node
            if cx.editor.tree.focus != node_id {
              cx.editor.focus(node_id);
            }

            // Only paste if it's a view (not a terminal)
            if let Some((view_id, doc_pos)) = self.screen_coords_to_doc_pos(mouse.position, cx) {
              // Move cursor to click position
              let scrolloff = cx.editor.config().scrolloff;
              let view = cx.editor.tree.get(view_id);
              let doc_id = view.doc;
              let doc = cx.editor.documents.get_mut(&doc_id).unwrap();

              let selection = crate::core::selection::Selection::point(doc_pos);
              doc.set_selection(view_id, selection);

              let view = cx.editor.tree.get_mut(view_id);
              view.ensure_cursor_in_view(doc, scrolloff);

              // Paste from clipboard ('+' register)
              let mut cmd_cx = commands::Context {
                register:             Some('+'),
                count:                cx.editor.count,
                editor:               cx.editor,
                on_next_key_callback: None,
                callback:             Vec::new(),
                jobs:                 cx.jobs,
              };

              commands::paste_after(&mut cmd_cx);
            }
            // If terminal was clicked, we've already switched focus

            return EventResult::Consumed(None);
          }
        }
      },
      None => {
        // Mouse motion without button

        // Check if we're dragging a separator
        if let Some(mut drag) = self.dragging_separator {
          // Apply separator drag
          let mouse_coord = if drag.separator.vertical {
            mouse.position.0
          } else {
            mouse.position.1
          };
          let delta_px = mouse_coord - drag.start_mouse_px;
          let (cell_width, cell_height) = self.get_current_cell_metrics(cx);

          // Calculate new separator position
          let font_metric = if drag.separator.vertical {
            cell_width
          } else {
            cell_height
          };

          let total_delta_cells = (delta_px / font_metric).round() as i32;

          // Only apply the incremental change (what we haven't applied yet)
          let incremental_delta = total_delta_cells - drag.accumulated_cells;

          if incremental_delta != 0 {
            // Perform the resize
            cx.editor.tree.resize_split(
              drag.separator.view_id,
              drag.separator.vertical,
              incremental_delta,
            );

            // Update accumulated cells
            drag.accumulated_cells = total_delta_cells;
            self.dragging_separator = Some(drag);
          }

          return EventResult::Consumed(None);
        }

        // Check if we're dragging text selection
        if self.mouse_pressed && self.click_count == 1 {
          // Only drag for single-click (not double/triple)
          if let Some(anchor) = self.mouse_drag_anchor {
            if let Some((view_id, doc_pos)) = self.screen_coords_to_doc_pos(mouse.position, cx) {
              let scrolloff = cx.editor.config().scrolloff;

              let view = cx.editor.tree.get(view_id);
              let doc_id = view.doc;
              let doc = cx.editor.documents.get_mut(&doc_id).unwrap();

              // Create range selection from anchor to current position
              let selection = crate::core::selection::Selection::single(anchor, doc_pos);
              doc.set_selection(view_id, selection);

              let view = cx.editor.tree.get_mut(view_id);
              view.ensure_cursor_in_view(doc, scrolloff);

              return EventResult::Consumed(None);
            }
          }
        }

        // Update separator hover state
        self.hovered_separator = self.detect_separator_hover(mouse.position, cx);
      },
      _ => {},
    }

    EventResult::Ignored(None)
  }

  /// Detect which node (view or terminal) was clicked
  /// Returns ViewId if click was within any node
  fn screen_coords_to_node(
    &self,
    mouse_pos: (f32, f32),
    cx: &Context,
  ) -> Option<crate::core::ViewId> {
    let (mouse_x, mouse_y) = mouse_pos;
    let (cell_width, cell_height) = self.get_current_cell_metrics(cx);

    // Convert pixel coordinates to cell coordinates
    let mouse_col = (mouse_x / cell_width) as u16;
    let mouse_row = (mouse_y / cell_height) as u16;

    // Check views first
    for (view, _) in cx.editor.tree.views() {
      if mouse_col >= view.area.x
        && mouse_col < view.area.x + view.area.width
        && mouse_row >= view.area.y
        && mouse_row < view.area.y + view.area.height
      {
        return Some(view.id);
      }
    }

    // Check terminals
    for (term_id, term_node) in cx.editor.tree.terminals() {
      if mouse_col >= term_node.area.x
        && mouse_col < term_node.area.x + term_node.area.width
        && mouse_row >= term_node.area.y
        && mouse_row < term_node.area.y + term_node.area.height
      {
        return Some(term_id);
      }
    }

    None
  }

  /// Convert screen pixel coordinates to document position
  /// Returns (ViewId, document_char_index) if click was within a view
  fn screen_coords_to_doc_pos(
    &self,
    mouse_pos: (f32, f32),
    cx: &Context,
  ) -> Option<(crate::core::ViewId, usize)> {
    let (mouse_x, mouse_y) = mouse_pos;
    let (cell_width, cell_height) = self.get_current_cell_metrics(cx);

    // Convert pixel coordinates to cell coordinates
    let mouse_col = (mouse_x / cell_width) as u16;
    let mouse_row = (mouse_y / cell_height) as u16;

    // Find which view was clicked
    for (view, _) in cx.editor.tree.views() {
      // Check if mouse is within view bounds
      if mouse_col < view.area.x
        || mouse_col >= view.area.x + view.area.width
        || mouse_row < view.area.y
        || mouse_row >= view.area.y + view.area.height
      {
        continue;
      }

      // Found the view! Now convert to document position
      let doc = &cx.editor.documents[&view.doc];
      let text = doc.text();

      // Calculate position relative to view's content area (excluding gutter)
      let gutter_width = view.rendered_gutter_width.unwrap_or(0);

      // Handle gutter click - select the entire line
      if mouse_col < view.area.x + gutter_width {
        let rel_row = mouse_row - view.area.y;
        let view_offset = doc.view_offset(view.id);

        // Calculate which document line was clicked
        let visual_row = rel_row as usize + view_offset.vertical_offset;
        let viewport = view.inner_area(doc);
        let text_fmt = doc.text_format(viewport.width, None);
        let annotations = view.text_annotations(doc, None);

        // Get the character position at the start of this visual row
        let (char_pos, _) = char_idx_at_visual_offset(
          text.slice(..),
          view_offset.anchor,
          visual_row as isize - view_offset.vertical_offset as isize,
          0,
          &text_fmt,
          &annotations,
        );

        // Clamp to valid document range
        let char_pos = char_pos.min(text.len_chars());

        // Return start of line for gutter clicks
        return Some((view.id, char_pos));
      }

      let rel_col = mouse_col - (view.area.x + gutter_width);
      let rel_row = mouse_row - view.area.y;

      // Get view offset (scroll position)
      let view_offset = doc.view_offset(view.id);

      // Calculate visual position accounting for scroll
      let visual_row = rel_row as isize + view_offset.vertical_offset as isize;
      let visual_col = rel_col as usize + view_offset.horizontal_offset;

      // Convert visual position to document character index
      let viewport = view.inner_area(doc);
      let text_fmt = doc.text_format(viewport.width, None);
      let annotations = view.text_annotations(doc, None);

      let (doc_pos, _) = char_idx_at_visual_offset(
        text.slice(..),
        view_offset.anchor,
        visual_row - view_offset.vertical_offset as isize,
        visual_col,
        &text_fmt,
        &annotations,
      );

      // Clamp to valid document range (char_idx_at_visual_offset usually handles
      // this, but we ensure it for edge cases like clicking way past EOF)
      let doc_pos = doc_pos.min(text.len_chars());

      return Some((view.id, doc_pos));
    }

    None
  }

  /// Get current cell metrics, recalculating if font size has changed
  /// Returns (cell_width, cell_height) accounting for any font size changes
  fn get_current_cell_metrics(&self, cx: &Context) -> (f32, f32) {
    let current_font_size = cx
      .editor
      .font_size_override
      .unwrap_or(cx.editor.config().font_size);

    // If font size has changed since last render, recalculate metrics
    if (current_font_size - self.cached_font_size).abs() > 0.1 {
      // Font size changed - estimate new metrics
      // Cell height scales proportionally with font size
      let scale = current_font_size / self.cached_font_size;
      let new_cell_height = self.cached_cell_height * scale;
      let new_cell_width = self.cached_cell_width * scale;
      (new_cell_width, new_cell_height)
    } else {
      // Use cached metrics
      (self.cached_cell_width, self.cached_cell_height)
    }
  }

  /// Detect if mouse is hovering over a split separator
  /// Returns separator info if hovering, None otherwise
  fn detect_separator_hover(&self, mouse_pos: (f32, f32), cx: &Context) -> Option<SeparatorInfo> {
    const SEPARATOR_WIDTH_PX: f32 = 2.0;
    const SEPARATOR_HEIGHT_PX: f32 = 2.0;
    const SEPARATOR_HOVER_THRESHOLD: f32 = 6.0; // Wider hit area for easier interaction

    let (mouse_x, mouse_y) = mouse_pos;
    let (font_width, cell_height) = self.get_current_cell_metrics(cx);

    // Check all views for nearby separators
    let tree = &cx.editor.tree;
    for (view, _) in tree.views() {
      let area = view.area;

      if tree
        .find_split_in_direction(view.id, Direction::Right)
        .is_some()
      {
        let gap_center_x = (area.x + area.width) as f32 * font_width + (font_width / 2.0);
        let sep_y = area.y as f32 * cell_height;
        let sep_height = area.height as f32 * cell_height;

        // Check if mouse is near this vertical separator
        if mouse_y >= sep_y
          && mouse_y <= sep_y + sep_height
          && (mouse_x - gap_center_x).abs() < SEPARATOR_HOVER_THRESHOLD
        {
          return Some(SeparatorInfo {
            view_id:    view.id,
            vertical:   true,
            position:   gap_center_x,
            view_x:     area.x,
            view_y:     area.y,
            view_width: area.width,
          });
        }
      }

      if tree
        .find_split_in_direction(view.id, Direction::Down)
        .is_some()
      {
        let sep_x = area.x as f32 * font_width;
        let sep_y = (area.y + area.height) as f32 * cell_height - SEPARATOR_HEIGHT_PX;
        let sep_width = area.width as f32 * font_width;

        // Check if mouse is near this horizontal separator
        let sep_center_y = sep_y + (SEPARATOR_HEIGHT_PX / 2.0);
        if mouse_x >= sep_x
          && mouse_x <= sep_x + sep_width
          && (mouse_y - sep_center_y).abs() < SEPARATOR_HOVER_THRESHOLD
        {
          return Some(SeparatorInfo {
            view_id:    view.id,
            vertical:   false,
            position:   sep_center_y,
            view_x:     area.x,
            view_y:     area.y,
            view_width: area.width,
          });
        }
      }
    }

    None
  }
}
