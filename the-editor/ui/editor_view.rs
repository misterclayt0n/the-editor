use std::{
  collections::{
    HashMap,
    HashSet,
  },
  time::Instant,
};

use the_editor_event::request_redraw;
use the_editor_renderer::{
  Color,
  TextSection,
  TextSegment,
  TextStyle,
};
use the_editor_stdx::rope::RopeSliceExt;
use the_terminal::{
  ColorScheme as TerminalColorScheme,
  CursorShape as TerminalCursorShape,
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
    diagnostics::DiagnosticFilter,
    doc_formatter::DocumentFormatter,
    grapheme::{
      Grapheme,
      next_grapheme_boundary,
      prev_grapheme_boundary,
    },
    graphics::{
      Color as ThemeColor,
      CursorKind,
      Rect,
    },
    info::Info,
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
  editor::{
    Action,
    BufferLine,
    CompleteAction as EditorCompleteAction,
    Editor,
    FileTreePosition,
  },
  keymap::{
    KeyBinding,
    KeyTrie,
    Keymaps,
    Mode,
  },
  ui::{
    Explorer,
    components::bufferline,
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

const VIEW_PADDING_LEFT: f32 = 0.0; // No visual padding - only scrolloff
const VIEW_PADDING_TOP: f32 = 0.0;
const VIEW_PADDING_BOTTOM: f32 = 0.0; // No reservation - statusbar is now an overlay
const STATUSLINE_HEIGHT: f32 = 28.0; // Fixed statusline height (must match statusline.rs)
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

fn mix_hash(mut hash: u64, value: u64) -> u64 {
  const CONSTANT: u64 = 0x9E37_79B9_7F4A_7C15;
  const MULTIPLIER: u64 = 0xBF58_476D_1CE4_E5B9;
  hash = hash.wrapping_add(CONSTANT).rotate_left(5);
  hash ^ value.wrapping_mul(MULTIPLIER)
}

fn rgb_hash((r, g, b): (u8, u8, u8)) -> u64 {
  ((r as u64) << 16) | ((g as u64) << 8) | (b as u64)
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DragSelectMode {
  Character,
  Word,
  Line,
}

pub struct EditorView {
  pub keymaps:                   Keymaps,
  on_next_key:                   Option<(OnKeyCallback, OnKeyCallbackKind)>,
  // Track last command for macro replay
  last_insert:                   (MappableCommand, Vec<KeyBinding>),
  // Rendering optimizations
  dirty_region:                  DirtyRegion,
  command_batcher:               CommandBatcher,
  last_cursor_pos:               Option<usize>,
  last_selection_hash:           u64,
  // Cursor animation
  cursor_animation:              Option<crate::core::animation::AnimationHandle<(f32, f32)>>,
  // Zoom animation state
  zoom_anim_active:              bool,
  selection_pulse_animating:     bool,
  noop_effect_animating:         bool,
  // Gutter management
  pub gutter_manager:            GutterManager,
  // Completion popup
  pub(crate) completion:         Option<crate::ui::components::Completion>,
  // Signature help popup
  pub(crate) signature_help:     Option<crate::ui::components::SignatureHelp>,
  bufferline_visible:            bool,
  bufferline_alive_t:            f32, // Animation for show/hide (0.0 = hidden, 1.0 = visible)
  // Cached font metrics for mouse handling (updated during render)
  cached_cell_width:             f32,
  cached_cell_height:            f32,
  cached_font_size:              f32, // Font size corresponding to cached metrics
  // Mouse drag state for selection
  mouse_pressed:                 bool,
  mouse_drag_anchor_range:       Option<crate::core::selection::Range>,
  mouse_drag_mode:               DragSelectMode,
  // Multi-click detection (double/triple-click)
  last_click_time:               Option<std::time::Instant>,
  last_click_pos:                Option<(f32, f32)>,
  click_count:                   u8,
  // Terminal mouse selection state
  terminal_selection_active:     Option<the_terminal::TerminalId>,
  // Terminal scroll accumulator for smooth scrolling
  terminal_scroll_px:            f32,
  // Split separator interaction
  hovered_separator:             Option<SeparatorInfo>,
  dragging_separator:            Option<SeparatorDrag>,
  buffer_hover_index:            Option<usize>,
  buffer_tabs:                   Vec<bufferline::BufferTab>,
  bufferline_height:             f32,
  buffer_pressed_index:          Option<usize>,
  // RAD-style bufferline state
  buffer_close_hover_index:      Option<usize>,
  buffer_close_pressed_index:    Option<usize>,
  add_button_hovered:            bool,
  add_button_pressed:            bool,
  add_button_rect:               Option<crate::core::graphics::Rect>,
  tab_animation_states:
    std::collections::HashMap<bufferline::BufferKind, bufferline::TabAnimationState>,
  add_button_state:              bufferline::AddButtonState,
  bufferline_scroll_offset:      f32,
  bufferline_scroll_target:      f32,
  bufferline_max_scroll:         f32,
  bufferline_user_scrolled:      bool, // True when user manually scrolled, disables auto-scroll
  bufferline_last_active_idx:    Option<usize>, // Track active tab to detect changes
  // Tree explorer sidebar
  explorer:                      Option<Explorer>,
  // Explorer mouse interaction state
  explorer_px_width:             f32,
  explorer_position:             FileTreePosition,
  explorer_hovered_item:         Option<usize>,
  // Accumulator for fractional scroll amounts in explorer (for trackpad)
  explorer_scroll_accum:         f32,
  // Track last mouse position for scroll targeting
  last_mouse_pos:                Option<(f32, f32)>,
  // Indent guide animation state (per indent level -> current opacity)
  indent_guide_opacities:        std::collections::HashMap<usize, f32>,
  // Track if indent guide animation is in progress
  indent_guides_anim_active:     bool,
  // Diagnostic glow animation state (per doc line -> current opacity)
  diagnostic_glow_opacities:     std::collections::HashMap<usize, f32>,
  // Track if diagnostic glow animation is in progress
  diagnostic_glow_anim_active:   bool,
  // EOL diagnostic text animation state (per doc line -> current opacity)
  eol_diagnostic_opacities:      std::collections::HashMap<usize, f32>,
  // Track if EOL diagnostic animation is in progress
  eol_diagnostic_anim_active:    bool,
  // EOL diagnostic debounce: pending lines waiting to be animated (line -> first seen time)
  eol_diagnostic_pending:        std::collections::HashMap<usize, std::time::Instant>,
  // Underline animation state (per doc line -> current opacity)
  underline_opacities:           std::collections::HashMap<usize, f32>,
  // Track if underline animation is in progress
  underline_anim_active:         bool,
  // Inline diagnostic animation state (per doc line -> full animation state)
  inline_diagnostic_anim:
    std::collections::HashMap<usize, super::inline_diagnostic_animation::InlineDiagnosticAnimState>,
  // Track if inline diagnostic animation is in progress
  inline_diagnostic_anim_active: bool,
  // Terminal escape prefix state: true when waiting for command after Ctrl+\
  terminal_escape_pending:       bool,
  // Infobox animation state (0.0 → 1.0 appearance)
  infobox_animation:             Option<crate::core::animation::AnimationHandle<f32>>,
  // Track whether autoinfo was present last frame (to detect transitions)
  had_autoinfo:                  bool,
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
  fn key_binding_to_key_press(binding: &KeyBinding) -> the_editor_renderer::KeyPress {
    the_editor_renderer::KeyPress {
      code:    binding.code,
      pressed: true,
      shift:   binding.shift,
      ctrl:    binding.ctrl,
      alt:     binding.alt,
      super_:  false,
    }
  }

  pub fn new(keymaps: Keymaps) -> Self {
    // Defaults; will be overridden from config on first render
    Self {
      keymaps,
      on_next_key: None,
      last_insert: (MappableCommand::normal_mode, Vec::new()),
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
      bufferline_visible: false,
      bufferline_alive_t: 0.0,
      cached_cell_width: 12.0,  // Default, will be updated during render
      cached_cell_height: 24.0, // Default, will be updated during render
      cached_font_size: 18.0,   // Default, will be updated during render
      mouse_pressed: false,
      mouse_drag_anchor_range: None,
      mouse_drag_mode: DragSelectMode::Character,
      last_click_time: None,
      last_click_pos: None,
      click_count: 0,
      terminal_selection_active: None,
      terminal_scroll_px: 0.0,
      hovered_separator: None,
      dragging_separator: None,
      buffer_hover_index: None,
      buffer_tabs: Vec::new(),
      bufferline_height: 24.0,
      buffer_pressed_index: None,
      buffer_close_hover_index: None,
      buffer_close_pressed_index: None,
      add_button_hovered: false,
      add_button_pressed: false,
      add_button_rect: None,
      tab_animation_states: std::collections::HashMap::new(),
      add_button_state: bufferline::AddButtonState::default(),
      bufferline_scroll_offset: 0.0,
      bufferline_scroll_target: 0.0,
      bufferline_max_scroll: 0.0,
      bufferline_user_scrolled: false,
      bufferline_last_active_idx: None,
      explorer: None,
      explorer_px_width: 0.0,
      explorer_position: FileTreePosition::Left,
      explorer_hovered_item: None,
      explorer_scroll_accum: 0.0,
      last_mouse_pos: None,
      indent_guide_opacities: std::collections::HashMap::new(),
      indent_guides_anim_active: false,
      diagnostic_glow_opacities: std::collections::HashMap::new(),
      diagnostic_glow_anim_active: false,
      eol_diagnostic_opacities: std::collections::HashMap::new(),
      eol_diagnostic_anim_active: false,
      eol_diagnostic_pending: std::collections::HashMap::new(),
      underline_opacities: std::collections::HashMap::new(),
      underline_anim_active: false,
      inline_diagnostic_anim: std::collections::HashMap::new(),
      inline_diagnostic_anim_active: false,
      terminal_escape_pending: false,
      infobox_animation: None,
      had_autoinfo: false,
    }
  }

  /// Toggle the tree explorer sidebar
  pub fn toggle_explorer(&mut self, cx: &mut Context) {
    if let Some(ref mut explorer) = self.explorer {
      // Toggle based on target state, regardless of animation
      if explorer.is_opened() {
        explorer.close();
      } else {
        explorer.focus();
      }
    } else {
      // No explorer exists - create a new one
      match Explorer::new(cx) {
        Ok(explorer) => {
          self.explorer = Some(explorer);
        },
        Err(err) => {
          cx.editor
            .set_error(format!("Failed to open explorer: {}", err));
        },
      }
    }
  }

  /// Open the tree explorer sidebar
  pub fn open_explorer(&mut self, cx: &mut Context) {
    if self.explorer.is_none() {
      match Explorer::new(cx) {
        Ok(explorer) => {
          self.explorer = Some(explorer);
        },
        Err(err) => {
          cx.editor
            .set_error(format!("Failed to open explorer: {}", err));
        },
      }
    }
    // Focus the explorer if it exists
    if let Some(ref mut explorer) = self.explorer {
      explorer.focus();
    }
  }

  /// Close the tree explorer sidebar (with animation)
  pub fn close_explorer(&mut self) {
    if let Some(ref mut explorer) = self.explorer {
      if explorer.is_opened() {
        explorer.close();
      }
    }
  }

  /// Check if explorer is open and focused
  pub fn explorer_focused(&self) -> bool {
    self.explorer.as_ref().is_some_and(|e| e.is_focus())
  }

  /// Get the x offset for content when explorer is on the left.
  /// Returns 0.0 when explorer is on the right (no offset needed).
  fn content_x_offset(&self) -> f32 {
    match self.explorer_position {
      FileTreePosition::Left => self.explorer_px_width,
      FileTreePosition::Right => 0.0,
    }
  }

  /// Get the y adjustment for content to align with the bufferline.
  /// The bufferline uses UI_FONT_SIZE (fixed), while the layout reserves 1 cell
  /// row. This adjustment accounts for the difference between bufferline
  /// height and cell height.
  fn bufferline_y_adjustment(&self) -> f32 {
    if self.bufferline_alive_t > 0.01 {
      // Adjustment = actual bufferline height - 1 row of cell height
      // This aligns the content right after the bufferline
      self.bufferline_height - self.cached_cell_height
    } else {
      0.0
    }
  }

  /// Check if mouse is in the explorer area based on explorer position
  fn is_in_explorer_area(&self, mouse_x: f32, viewport_width: f32) -> bool {
    if self.explorer_px_width <= 0.0 {
      return false;
    }
    match self.explorer_position {
      FileTreePosition::Left => mouse_x < self.explorer_px_width,
      FileTreePosition::Right => mouse_x >= viewport_width - self.explorer_px_width,
    }
  }

  /// Get mutable reference to the explorer if it exists
  pub fn explorer_mut(&mut self) -> Option<&mut Explorer> {
    self.explorer.as_mut()
  }

  /// Get the current explorer position (from config)
  pub fn explorer_position(&self) -> FileTreePosition {
    self.explorer_position
  }

  /// Check if explorer is open (regardless of focus)
  pub fn explorer_is_open(&self) -> bool {
    self.explorer.as_ref().is_some_and(|e| e.is_opened())
  }

  pub fn set_keymaps(&mut self, map: &HashMap<Mode, KeyTrie>) {
    self.keymaps = Keymaps::new(map.clone());
  }

  pub fn has_pending_on_next_key(&self) -> bool {
    self
      .on_next_key
      .as_ref()
      .is_some_and(|(_, kind)| *kind == OnKeyCallbackKind::Pending)
  }

  /// Set the completion popup with the given items
  pub fn set_completion(
    &mut self,
    editor: &Editor,
    items: Vec<crate::handlers::completion::CompletionItem>,
    _trigger_offset: usize,
  ) -> Option<Rect> {
    use crate::ui::components::Completion;

    // Skip if focused view is not a document (e.g., terminal)
    let Some((view, doc)) = crate::try_current_ref!(editor) else {
      return None;
    };
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
    // Drop signature help to mirror Helix: popups never coexist to avoid overlap
    self.signature_help = None;

    // TODO: Calculate actual area based on cursor position
    Some(Rect::new(0, 0, 60, 15))
  }

  /// Clear the completion popup
  pub fn clear_completion(&mut self, _editor: &mut Editor) {
    self.completion = None;
  }

  fn close_completion_with_context(&mut self, cx: &mut Context) {
    self.completion = None;

    let Some(last_completion) = cx.editor.last_completion.take() else {
      return;
    };

    if let EditorCompleteAction::Applied { placeholder, .. } = last_completion {
      if placeholder {
        let callback: OnKeyCallback = Box::new(|cmd_cx, key_press| {
          if let the_editor_renderer::Key::Char(ch) = key_press.code {
            let (view, doc) = crate::current!(cmd_cx.editor);
            if let Some(snippet) = &doc.active_snippet {
              doc.apply(&snippet.delete_placeholder(doc.text()), view.id);
            }
            commands::insert_char(cmd_cx, ch);
          } else {
            // Re-dispatch non-char keys (like Esc, arrows) so they aren't swallowed
            let binding = crate::keymap::KeyBinding::new(key_press.code).with_modifiers(
              key_press.shift,
              key_press.ctrl,
              key_press.alt,
            );

            cmd_cx.callback.push(Box::new(move |compositor, cx| {
              compositor.handle_event(&crate::ui::compositor::Event::Key(binding), cx);
            }));
          }
        });
        self.on_next_key = Some((callback, OnKeyCallbackKind::Fallback));
      }
    }
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
    if self.dirty_region.needs_redraw() {
      crate::profile_message!("editor_view: dirty_region");
      log::trace!("editor_view.should_update: dirty_region");
      return true;
    }
    if self
      .cursor_animation
      .as_ref()
      .is_some_and(|anim| !anim.is_complete())
    {
      crate::profile_message!("editor_view: cursor_animation");
      log::trace!("editor_view.should_update: cursor_animation");
      return true;
    }
    if self.zoom_anim_active {
      crate::profile_message!("editor_view: zoom_anim");
      log::trace!("editor_view.should_update: zoom_anim");
      return true;
    }
    if self.selection_pulse_animating {
      crate::profile_message!("editor_view: selection_pulse");
      log::trace!("editor_view.should_update: selection_pulse");
      return true;
    }
    if self.noop_effect_animating {
      crate::profile_message!("editor_view: noop_effect");
      log::trace!("editor_view.should_update: noop_effect");
      return true;
    }
    if self.completion.as_ref().is_some_and(|c| c.is_animating()) {
      crate::profile_message!("editor_view: completion");
      log::trace!("editor_view.should_update: completion");
      return true;
    }
    if self
      .signature_help
      .as_ref()
      .is_some_and(|s| s.is_animating())
    {
      crate::profile_message!("editor_view: signature_help");
      log::trace!("editor_view.should_update: signature_help");
      return true;
    }
    if self.dragging_separator.is_some() {
      crate::profile_message!("editor_view: dragging_separator");
      log::trace!("editor_view.should_update: dragging_separator");
      return true;
    }
    if self.explorer.as_ref().is_some_and(|e| e.is_animating()) {
      crate::profile_message!("editor_view: explorer");
      log::trace!("editor_view.should_update: explorer");
      return true;
    }
    if self.indent_guides_anim_active {
      crate::profile_message!("editor_view: indent_guides");
      log::trace!("editor_view.should_update: indent_guides");
      return true;
    }
    if self.diagnostic_glow_anim_active {
      crate::profile_message!("editor_view: diagnostic_glow");
      log::trace!("editor_view.should_update: diagnostic_glow");
      return true;
    }
    if self.eol_diagnostic_anim_active {
      crate::profile_message!("editor_view: eol_diagnostic");
      log::trace!("editor_view.should_update: eol_diagnostic");
      return true;
    }
    if self.underline_anim_active {
      crate::profile_message!("editor_view: underline");
      log::trace!("editor_view.should_update: underline");
      return true;
    }
    if self.inline_diagnostic_anim_active {
      crate::profile_message!("editor_view: inline_diagnostic");
      log::trace!("editor_view.should_update: inline_diagnostic");
      return true;
    }
    if bufferline::needs_animation_update(&self.tab_animation_states, &self.add_button_state) {
      crate::profile_message!("editor_view: bufferline");
      log::trace!("editor_view.should_update: bufferline");
      return true;
    }
    if (self.bufferline_scroll_offset - self.bufferline_scroll_target).abs() > 0.5 {
      crate::profile_message!("editor_view: bufferline_scroll");
      log::trace!("editor_view.should_update: bufferline_scroll");
      return true;
    }
    if self.bufferline_alive_t > 0.01 && self.bufferline_alive_t < 0.99 {
      crate::profile_message!("editor_view: bufferline_alive");
      log::trace!("editor_view.should_update: bufferline_alive");
      return true;
    }
    false
  }

  fn handle_event(&mut self, event: &Event, cx: &mut Context) -> EventResult {
    if matches!(event, Event::Scroll(_)) {
      if let Some(result) = self.dispatch_signature_help_event(event, cx) {
        return match result {
          EventResult::Consumed(cb) | EventResult::Ignored(cb) => EventResult::Consumed(cb),
        };
      }
    } else if matches!(event, Event::Key(_)) {
      if let Some(result) = self.dispatch_signature_help_event(event, cx) {
        if matches!(result, EventResult::Consumed(_)) {
          return result;
        }
      }
    }

    // Handle explorer events if it's focused
    if let Some(ref mut explorer) = self.explorer {
      if explorer.is_focus() {
        let result = explorer.handle_event(event, cx);
        // Note: We intentionally keep the explorer instance alive when closed
        // so that the tree layout is preserved when reopening.
        if matches!(result, EventResult::Consumed(_)) {
          return result;
        }
      }
    }

    // Handle terminal input if focused view is a terminal
    if let Event::Key(key) = event {
      if let Some(result) = self.handle_terminal_key(key, cx) {
        return result;
      }
    }

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
            super_:  false,
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
                  // Completion wants to close, clear it and handle snippet placeholder state
                  self.close_completion_with_context(cx);
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
                    self.close_completion_with_context(cx);
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
              let Some(doc_id) = view.doc() else {
                return EventResult::Ignored(None);
              };
              let doc = &cx.editor.documents[&doc_id];
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
              let Some(doc_id) = view.doc() else {
                return EventResult::Consumed(None);
              };
              let doc = &cx.editor.documents[&doc_id];
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
          super_:  false,
        };

        // Process through keymap for non-insert modes
        use crate::keymap::KeymapResult;
        match self.keymaps.get(cx.editor.mode(), &key_press) {
          KeymapResult::Matched(command) => {
            cx.editor.autoinfo = None;
            self.execute_command_sequence(cx, std::iter::once(command))
          },
          KeymapResult::MatchedSequence(commands) => {
            cx.editor.autoinfo = None;
            self.execute_command_sequence(cx, commands)
          },
          KeymapResult::Pending(node) => {
            cx.editor.autoinfo = Some(node.infobox());
            EventResult::Consumed(None)
          },
          KeymapResult::Cancelled(_) | KeymapResult::NotFound => {
            cx.editor.autoinfo = None;
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
      Event::Scroll(delta) => {
        use the_editor_renderer::ScrollDelta;

        // Handle scroll for terminal views
        // First try mouse position, then fall back to focused view
        let terminal_id = if let Some((mouse_x, mouse_y)) = self.last_mouse_pos {
          // Check if mouse is over a terminal view
          self
            .screen_coords_to_node((mouse_x, mouse_y), cx)
            .and_then(|node_id| {
              let view = cx.editor.tree.get(node_id);
              view.terminal()
            })
        } else {
          // No mouse position - check if focused view is a terminal
          let focused = cx.editor.tree.focus;
          let view = cx.editor.tree.get(focused);
          view.terminal()
        };

        if let Some(terminal_id) = terminal_id {
          if let Some(term) = cx.editor.terminal(terminal_id) {
            let line_height = self.cached_cell_height.max(1.0);

            // Convert scroll delta to pixels (like Zed's pixel_delta)
            let delta_px = match delta {
              ScrollDelta::Lines { y, .. } => *y * line_height,
              ScrollDelta::Pixels { y, .. } => *y,
            };

            // Accumulate and calculate line difference (Zed's approach)
            let old_lines = (self.terminal_scroll_px / line_height) as i32;
            self.terminal_scroll_px += delta_px;
            let new_lines = (self.terminal_scroll_px / line_height) as i32;
            let scroll_lines = new_lines - old_lines;

            // Reset accumulator modulo terminal height to prevent overflow
            let (_, rows) = term.dimensions();
            let terminal_height = (rows as f32 * line_height).max(line_height);
            self.terminal_scroll_px %= terminal_height;

            if scroll_lines != 0 {
              if term.mouse_mode() || term.alt_screen_mode() {
                // Get terminal cell position for scroll event
                let (col, row) = if let Some((mouse_x, mouse_y)) = self.last_mouse_pos {
                  if let Some(node_id) = self.screen_coords_to_node((mouse_x, mouse_y), cx) {
                    let view = cx.editor.tree.get(node_id);
                    let view_area = view.area;
                    let rel_x = mouse_x
                      - self.content_x_offset()
                      - view_area.x as f32 * self.cached_cell_width;
                    let rel_y = mouse_y
                      - view_area.y as f32 * self.cached_cell_height
                      - self.bufferline_y_adjustment();
                    (
                      (rel_x / self.cached_cell_width).floor().max(0.0) as u16,
                      (rel_y / self.cached_cell_height).floor().max(0.0) as u16,
                    )
                  } else {
                    (1, 1)
                  }
                } else {
                  (1, 1) // Default to top-left if no mouse position
                };

                // Send scroll events to terminal (SGR mouse mode)
                // Button 64 = scroll up, 65 = scroll down
                let button = if scroll_lines > 0 { 64 } else { 65 };
                for _ in 0..scroll_lines.abs() {
                  let report = format!("\x1b[<{};{};{}M", button, col + 1, row + 1);
                  term.write(report.as_bytes());
                }
              } else {
                // Normal scroll - scroll terminal history
                term.scroll(scroll_lines);
              }
              request_redraw();
            }
          }
          return EventResult::Consumed(None);
        }

        // Handle scroll in bufferline area
        if let Some((_, mouse_y)) = self.last_mouse_pos {
          if mouse_y < self.bufferline_height && self.bufferline_max_scroll > 0.0 {
            use the_editor_renderer::ScrollDelta;

            // Convert scroll delta to pixels for horizontal scroll
            let scroll_px = match delta {
              ScrollDelta::Lines { x, y, .. } => {
                // For mouse wheel, prefer horizontal scroll (x), fallback to vertical (y)
                let scroll_amount = if x.abs() > 0.001 { *x } else { *y };
                scroll_amount * 40.0 // 40 pixels per scroll line
              },
              ScrollDelta::Pixels { x, y, .. } => {
                // For trackpad, prefer horizontal scroll, fallback to vertical
                if x.abs() > 0.001 { *x } else { *y }
              },
            };

            // Update scroll target (animated toward in render)
            self.bufferline_scroll_target =
              (self.bufferline_scroll_target + scroll_px).clamp(0.0, self.bufferline_max_scroll);
            // Mark that user manually scrolled - disables auto-scroll until active tab
            // changes
            self.bufferline_user_scrolled = true;
            self.dirty_region.mark_all_dirty();
            request_redraw();
            return EventResult::Consumed(None);
          }
        }

        // Handle scroll in explorer area if mouse is over it
        if let Some((mouse_x, _)) = self.last_mouse_pos {
          // Calculate viewport width from editor tree area and font
          let viewport_px_width =
            cx.editor.tree.area().width as f32 * self.cached_cell_width + self.explorer_px_width;
          let in_explorer = self.is_in_explorer_area(mouse_x, viewport_px_width);
          if in_explorer {
            if let Some(ref mut explorer) = self.explorer {
              if explorer.is_opened() {
                use the_editor_renderer::ScrollDelta;

                // Convert scroll delta to lines, accumulating fractional amounts
                let delta_lines = match delta {
                  ScrollDelta::Lines { y, .. } => {
                    // Discrete scroll (mouse wheel) - use directly
                    // Reset accumulator on discrete scrolls
                    self.explorer_scroll_accum = 0.0;
                    -*y
                  },
                  ScrollDelta::Pixels { y, .. } => {
                    // Continuous scroll (trackpad) - accumulate fractional amounts
                    // Use cached cell height for accurate line calculation
                    let line_height = self.cached_cell_height.max(1.0);
                    -*y / line_height
                  },
                };

                // Accumulate scroll amount
                self.explorer_scroll_accum += delta_lines;

                // Extract integer lines to scroll
                let lines_to_scroll = self.explorer_scroll_accum.trunc() as i32;

                // Keep fractional remainder for next event
                self.explorer_scroll_accum -= lines_to_scroll as f32;

                if lines_to_scroll != 0 {
                  explorer.scroll(lines_to_scroll);
                  request_redraw();
                }
                return EventResult::Consumed(None);
              }
            }
          }
        }

        EventResult::Ignored(None)
      },
      _ => EventResult::Ignored(None),
    }
  }

  fn render(&mut self, area: Rect, renderer: &mut Surface, cx: &mut Context) {
    // Separator dimensions (used for clipping and rendering)
    const SEPARATOR_WIDTH_PX: f32 = 2.0;
    const SEPARATOR_HEIGHT_PX: f32 = 2.0;

    // Get font size with fallback chain based on config
    let per_buffer_enabled = cx.editor.config().per_buffer_font_size;
    let font_size = if per_buffer_enabled {
      // Per-buffer mode: document override -> editor override -> config default
      let doc_override = cx
        .editor
        .tree
        .try_get(cx.editor.tree.focus)
        .and_then(|view| view.doc())
        .and_then(|doc_id| cx.editor.documents.get(&doc_id))
        .and_then(|doc| doc.font_size_override);
      let size = doc_override
        .or(cx.editor.font_size_override)
        .unwrap_or(cx.editor.config().font_size);
      log::trace!(
        "Per-buffer font size: doc_override={:?}, final={}",
        doc_override,
        size
      );
      size
    } else {
      // Global mode: editor override -> config default
      cx.editor
        .font_size_override
        .unwrap_or(cx.editor.config().font_size)
    };
    let font_family = renderer.current_font_family().to_string();
    renderer.configure_font(&font_family, font_size);
    let font_width = renderer.cell_width().max(1.0);

    let bufferline_mode = {
      let config = cx.editor.config();
      config.bufferline.clone()
    };
    let use_bufferline = match bufferline_mode {
      BufferLine::Always => true,
      BufferLine::Multiple => {
        // Count both documents and visible terminals
        let total_tabs = cx.editor.documents.len() + cx.editor.visible_terminals().count();
        total_tabs > 1
      },
      BufferLine::Never => false,
    };
    if self.bufferline_visible != use_bufferline {
      self.bufferline_visible = use_bufferline;
      self.dirty_region.mark_all_dirty();
    }

    // Animate bufferline visibility using exponential decay (slower rate for
    // deliberate animation)
    {
      let alive_rate = 1.0 - 2.0_f32.powf(-30.0 * cx.dt);
      let target = if self.bufferline_visible { 1.0 } else { 0.0 };
      self.bufferline_alive_t += (target - self.bufferline_alive_t) * alive_rate;
      // Snap when close enough
      if (self.bufferline_alive_t - target).abs() < 0.01 {
        self.bufferline_alive_t = target;
      }
    }

    // Cache font metrics for mouse handling
    self.cached_cell_width = font_width;
    self.cached_cell_height = renderer.cell_height();
    self.cached_font_size = font_size;

    // Calculate tree area from renderer dimensions
    // Subtract statusline height in pixels (not rows) to avoid variable gap at
    // bottom
    let available_height =
      (renderer.height() as f32) - (VIEW_PADDING_TOP + VIEW_PADDING_BOTTOM) - STATUSLINE_HEIGHT;
    let available_height = available_height.max(font_size);
    let total_rows = ((available_height / self.cached_cell_height)
      .floor()
      .max(1.0)) as u16;

    // Don't subtract visual padding from viewport width - it's only for rendering
    // offset
    let viewport_px_width = renderer.width() as f32;
    let available_width = viewport_px_width.max(font_width);
    let area_width = (available_width / font_width).floor().max(1.0) as u16;

    // No clip_bottom needed - statusline height is already subtracted in pixels
    // above
    let mut target_area = Rect::new(0, 0, area_width, total_rows);
    // Reserve 1 row for bufferline when visible. The actual pixel adjustment
    // (bufferline_y_adjustment) handles the mismatch between bufferline height
    // and cell height to prevent overlap or gaps.
    if self.bufferline_alive_t > 0.01 {
      target_area = target_area.clip_top(1);
    }

    // Calculate explorer pixel width using UI font (independent of buffer font)
    // We need to temporarily configure the font to get UI metrics
    let ui_font_family = renderer.current_font_family().to_string();
    renderer.configure_font(&ui_font_family, crate::ui::UI_FONT_SIZE);
    let ui_cell_width = renderer.cell_width();
    // Restore buffer font configuration
    renderer.configure_font(&font_family, font_size);

    // Update explorer position from config
    self.explorer_position = cx.editor.config().file_tree.position;

    // Update and calculate explorer pixel width (using UI font metrics, not buffer
    // font)
    // Note: We keep the explorer instance alive even when closed to preserve tree
    // layout.
    self.explorer_px_width = if let Some(ref mut explorer) = self.explorer {
      // Update animation
      explorer.update_closing(cx.dt);
      // Width is base_width * animation_progress (0.0 when closed, 1.0 when open)
      let explorer_width_cells = explorer.column_width();
      let base_width = explorer_width_cells as f32 * ui_cell_width;
      base_width * explorer.closing_progress()
    } else {
      0.0
    };
    let explorer_px_width = self.explorer_px_width;
    let explorer_position = self.explorer_position;

    // Calculate explorer width in buffer font cells for target_area width
    // adjustment Note: We do NOT offset target_area.x - instead we add
    // explorer_px_width directly to all rendering coordinates. This ensures
    // popup positioning uses the same offset.
    let explorer_width_buffer_cells = if explorer_px_width > 0.0 {
      (explorer_px_width / font_width).ceil() as u16
    } else {
      0
    };

    // Reduce editor width to make room for the explorer, but keep x=0
    // The explorer offset is applied during rendering via explorer_px_width
    if explorer_width_buffer_cells > 0 {
      target_area = Rect::new(
        target_area.x, // Keep x at 0 - offset applied during rendering
        target_area.y,
        target_area
          .width
          .saturating_sub(explorer_width_buffer_cells),
        target_area.height,
      );
    }

    // Resize tree if needed
    if cx.editor.tree.resize(target_area) {
      let scrolloff = cx.editor.config().scrolloff;
      let view_ids: Vec<_> = cx.editor.tree.views().map(|(view, _)| view.id).collect();
      for view_id in view_ids {
        let view = cx.editor.tree.get(view_id);
        // Skip terminal views - they don't have document selections
        let Some(doc_id) = view.doc() else { continue };

        // Calculate actual gutter width for this view (accounts for disabled gutters)
        let gutter_width = {
          let doc = &cx.editor.documents[&doc_id];
          (self.gutter_manager.total_width(view, doc) as u16).min(view.area.width)
        };

        let view = cx.editor.tree.get_mut(view_id);
        view.rendered_gutter_width = Some(gutter_width);
        let doc = cx.editor.documents.get_mut(&doc_id).unwrap();
        view.sync_changes(doc);
        view.ensure_cursor_in_view(doc, scrolloff);
      }
      // Viewport changed, mark everything dirty
      self.dirty_region.mark_all_dirty();
    }

    // Ensure cursor is kept within the viewport including scrolloff padding
    // Skip for terminal views since they don't have document selections
    {
      let focus_view = cx.editor.tree.focus;

      if let Some(view) = cx.editor.tree.try_get(focus_view)
        && let Some(doc_id) = view.doc()
      {
        let scrolloff = cx.editor.config().scrolloff;

        // Calculate actual gutter width for focused view (accounts for disabled
        // gutters)
        let gutter_width = {
          let view = cx.editor.tree.get(focus_view);
          let doc = &cx.editor.documents[&doc_id];
          (self.gutter_manager.total_width(view, doc) as u16).min(view.area.width)
        };

        {
          // Limit the mutable borrow scope
          let view = cx.editor.tree.get_mut(focus_view);
          view.rendered_gutter_width = Some(gutter_width);
          let doc = cx.editor.documents.get_mut(&doc_id).unwrap();
          if !view.is_cursor_in_view(doc, scrolloff) {
            view.ensure_cursor_in_view(doc, scrolloff);
            // Viewport changed, force a redraw of visible content
            self.dirty_region.mark_all_dirty();
          }
        }
      }
    }

    // Cursor animation config is now read directly from editor config when needed

    // Resize any terminal views to match their area
    // This must happen before we borrow the theme immutably
    {
      // Get cell dimensions for PTY sizing
      let cell_width = self.cached_cell_width.max(1.0) as u16;
      let cell_height = self.cached_cell_height.max(1.0) as u16;

      let terminal_resizes: Vec<_> = cx
        .editor
        .tree
        .traverse()
        .filter_map(|(view_id, _)| {
          let view = cx.editor.tree.get(view_id);
          view
            .terminal()
            .map(|tid| (tid, view.area.width, view.area.height))
        })
        .collect();

      for (terminal_id, cols, rows) in terminal_resizes {
        // Guard against zero dimensions that cause alacritty grid underflow
        if cols == 0 || rows == 0 {
          continue;
        }
        if let Some(term) = cx.editor.terminal_mut(terminal_id) {
          term.resize(cols, rows, cell_width, cell_height);
        }
      }
    }

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

    // Calculate content x offset (only applies when explorer is on the left)
    let content_x_offset = match explorer_position {
      FileTreePosition::Left => explorer_px_width,
      FileTreePosition::Right => 0.0,
    };

    // Animate bufferline scroll offset toward target using exponential decay
    // Use slower rate (-30) for smooth scrolling feel
    {
      let scroll_rate = 1.0 - 2.0_f32.powf(-30.0 * cx.dt);
      self.bufferline_scroll_offset +=
        (self.bufferline_scroll_target - self.bufferline_scroll_offset) * scroll_rate;
      // Snap when close enough
      if (self.bufferline_scroll_offset - self.bufferline_scroll_target).abs() < 0.5 {
        self.bufferline_scroll_offset = self.bufferline_scroll_target;
      }
    }

    // Render bufferline when visible or animating
    if self.bufferline_alive_t > 0.01 {
      // Offset bufferline by explorer width so it doesn't overlap (only for left-side
      // explorer)
      let result = bufferline::render(
        cx.editor,
        content_x_offset,
        0.0,
        viewport_px_width - explorer_px_width,
        renderer,
        self.buffer_hover_index,
        self.buffer_pressed_index,
        self.buffer_close_hover_index,
        self.buffer_close_pressed_index,
        self.add_button_hovered,
        self.add_button_pressed,
        &mut self.buffer_tabs,
        &mut self.tab_animation_states,
        &mut self.add_button_state,
        self.bufferline_scroll_offset,
        self.last_mouse_pos,
        cx.dt,
        self.bufferline_alive_t, // height scale for slide animation (0.0 = collapsed, 1.0 = full)
      );
      self.bufferline_height = result.height;
      self.add_button_rect = result.add_button_rect;
      self.bufferline_max_scroll = result.max_scroll;

      // Check if active tab changed - if so, reset user scroll flag and allow
      // auto-scroll
      if result.active_tab_index != self.bufferline_last_active_idx {
        self.bufferline_user_scrolled = false;
        self.bufferline_last_active_idx = result.active_tab_index;
      }

      // Auto-scroll to ensure active tab is visible (only when user hasn't manually
      // scrolled)
      if !self.bufferline_user_scrolled {
        if let Some(active_idx) = result.active_tab_index {
          if let Some(tab) = self.buffer_tabs.get(active_idx) {
            let available_width = viewport_px_width - explorer_px_width - result.height - 12.0;
            let visible_start = self.bufferline_scroll_offset;
            let visible_end = visible_start + available_width;

            // Tab positions are relative to the scroll offset, so we need the unscrolled
            // positions
            let tab_start = tab.start_x + self.bufferline_scroll_offset - content_x_offset - 4.0;
            let tab_end = tab.end_x + self.bufferline_scroll_offset - content_x_offset - 4.0;

            if tab_start < visible_start {
              // Tab is to the left of visible area - scroll left
              self.bufferline_scroll_target = tab_start.max(0.0);
            } else if tab_end > visible_end {
              // Tab is to the right of visible area - scroll right
              self.bufferline_scroll_target =
                (tab_end - available_width).clamp(0.0, result.max_scroll);
            }
          }
        }
      }
    } else {
      self.buffer_tabs.clear();
      self.buffer_hover_index = None;
      self.buffer_pressed_index = None;
      self.buffer_close_hover_index = None;
      self.add_button_hovered = false;
      self.add_button_pressed = false;
      self.add_button_rect = None;
      self.bufferline_height = 0.0;
    }

    // Update viewport pixel offsets for popup positioning
    // These offsets account for explorer width (x) and bufferline height (y)
    // Only offset x when explorer is on the left
    cx.editor.viewport_pixel_offset = (content_x_offset, self.bufferline_height);

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
      let view = cx.editor.tree.get(view_id);
      let Some(doc_id) = view.doc() else { continue };
      let gutter_width = {
        let doc = &cx.editor.documents[&doc_id];
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

      // Update zoom animation using exponential decay
      // Formula: rate = 1 - 2^(-speed * dt)
      // Speed 10 gives ~200-300ms fade, speed 8 gives ~350ms
      // Clamp dt to prevent animation skipping on slow frames (large files)
      {
        let view = cx.editor.tree.get_mut(focus_view);
        if view.zoom_anim < 1.0 {
          let anim_dt = cx.dt.min(0.032); // Cap at ~30fps worth of progress
          let rate = 1.0 - 2.0_f32.powf(-10.0 * anim_dt);
          view.zoom_anim += (1.0 - view.zoom_anim) * rate;

          // Snap to 1.0 when very close to avoid endless tiny updates
          if view.zoom_anim > 0.99 {
            view.zoom_anim = 1.0;
          }
          self.zoom_anim_active = view.zoom_anim < 1.0;
        } else {
          self.zoom_anim_active = false;
        }
      }

      let view = cx.editor.tree.get(focus_view);
      let zoom_alpha = view.zoom_anim; // Raddebugger-style: just fade, no squish

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
      // Add content_x_offset to X offset - this is the key to consistent popup
      // positioning (only applies when explorer is on the left)
      let view_offset_x = content_x_offset + view_area.x as f32 * font_width;
      // Add bufferline_y_adjustment to align content with actual bufferline height
      let view_offset_y =
        view_area.y as f32 * self.cached_cell_height + self.bufferline_y_adjustment();
      let mut base_y = view_offset_y + VIEW_PADDING_TOP;

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

      // Check if this view is a terminal - if so, render it and continue
      if let Some(terminal_id) = view.terminal() {
        self.render_terminal_view(
          terminal_id,
          view_offset_x,
          base_y,
          is_focused,
          font_width,
          renderer,
          cx.editor,
        );
        renderer.pop_scissor_rect();
        continue;
      }

      let doc_id = view.doc_id();

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

        // Add content_x_offset to gutter and content X positions (only for left-side
        // explorer)
        let gutter_x = content_x_offset + gutter_rect.x as f32 * font_width + VIEW_PADDING_LEFT;
        let mut base_x = content_x_offset + content_rect.x as f32 * font_width + VIEW_PADDING_LEFT;

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

        let primary_index = selection.primary_index();
        let cursor_pos = selection.primary().cursor(doc_text.slice(..));
        let cursor_line = doc_text.char_to_line(cursor_pos);
        let primary_range = selection.primary();
        let has_selection = !primary_range.is_empty(); // Check if there's an actual selection (not just cursor)
        let secondary_cursor_positions: HashSet<usize> = selection
          .ranges()
          .iter()
          .enumerate()
          .filter_map(|(idx, range)| {
            if idx == primary_index {
              None
            } else {
              Some(range.cursor(doc_text.slice(..)))
            }
          })
          .collect();

        let editor_mode = cx.editor.mode();
        // Get cursor shape from config based on current mode (needed for selection
        // exclusion)
        let cursor_kind = cx.editor.config().cursor_shape.from_mode(editor_mode);
        let primary_cursor_is_block = cursor_kind == CursorKind::Block;
        let selection_highlight_ranges: Vec<(usize, usize)> = {
          let text_slice = doc_text.slice(..);
          let mut highlight_ranges = Vec::new();

          for (idx, range) in selection.ranges().iter().enumerate() {
            if range.anchor == range.head {
              continue;
            }

            let range = range.min_width_1(text_slice);
            let is_primary = idx == primary_index;

            if range.head > range.anchor {
              let cursor_start = prev_grapheme_boundary(text_slice, range.head);
              let selection_end =
                if is_primary && !primary_cursor_is_block && editor_mode != Mode::Insert {
                  range.head
                } else {
                  cursor_start
                };

              if range.anchor < selection_end {
                highlight_ranges.push((range.anchor, selection_end));
              }
            } else if range.head < range.anchor {
              let cursor_end = next_grapheme_boundary(text_slice, range.head);
              let selection_start = if is_primary
                && !primary_cursor_is_block
                && !(editor_mode == Mode::Insert && cursor_end == range.anchor)
              {
                range.head
              } else {
                cursor_end
              };

              if selection_start < range.anchor {
                highlight_ranges.push((selection_start, range.anchor));
              }
            }
          }

          highlight_ranges
        };

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
          let overlay_highlights =
            vec![annotations.collect_overlay_highlights(start_char..end_char)];

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

        // Update underline animation state (fast, no debounce)
        {
          let underline_lines: std::collections::HashSet<usize> = doc
            .diagnostics()
            .iter()
            .map(|d| doc_text.char_to_line(d.range.start.min(doc_text.len_chars())))
            .collect();

          // Add new diagnostic lines with opacity 0
          for &line in &underline_lines {
            if !self.underline_opacities.contains_key(&line) {
              self.underline_opacities.insert(line, 0.0);
            }
          }

          // Animate existing opacities (fast rate)
          let anim_rate = 1.0 - 2.0_f32.powf(-60.0 * cx.dt);
          let mut animating = false;

          for (&line, current) in self.underline_opacities.iter_mut() {
            let target = if underline_lines.contains(&line) {
              1.0
            } else {
              0.0
            };
            let delta = target - *current;
            if delta.abs() > 0.01 {
              animating = true;
              *current += anim_rate * delta;
            } else {
              *current = target;
            }
          }

          // Clean up lines that faded out
          self
            .underline_opacities
            .retain(|line, opacity| underline_lines.contains(line) || *opacity > 0.01);

          self.underline_anim_active = animating;
        }

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
            &self.underline_opacities,
          );
        decoration_manager.add_decoration(underlines);

        // Update EOL diagnostic text animation state with debouncing
        {
          use std::time::{
            Duration,
            Instant,
          };
          const EOL_DEBOUNCE: Duration = Duration::from_millis(350);

          let eol_diagnostic_lines: std::collections::HashSet<usize> = doc
            .diagnostics()
            .iter()
            .map(|d| doc_text.char_to_line(d.range.start.min(doc_text.len_chars())))
            .collect();

          let now = Instant::now();

          // Add new diagnostic lines to pending (if not already tracked)
          for &line in &eol_diagnostic_lines {
            if !self.eol_diagnostic_opacities.contains_key(&line)
              && !self.eol_diagnostic_pending.contains_key(&line)
            {
              self.eol_diagnostic_pending.insert(line, now);
            }
          }

          // Remove pending lines that disappeared before debounce completed
          self
            .eol_diagnostic_pending
            .retain(|line, _| eol_diagnostic_lines.contains(line));

          // Move pending lines to opacities once debounce period passes
          let ready_lines: Vec<usize> = self
            .eol_diagnostic_pending
            .iter()
            .filter(|(_, first_seen)| now.duration_since(**first_seen) >= EOL_DEBOUNCE)
            .map(|(line, _)| *line)
            .collect();

          for line in ready_lines {
            self.eol_diagnostic_pending.remove(&line);
            self.eol_diagnostic_opacities.insert(line, 0.0);
          }

          // Animate existing opacities
          let eol_anim_rate = 1.0 - 2.0_f32.powf(-40.0 * cx.dt);
          let mut eol_animating = false;

          for (&line, current) in self.eol_diagnostic_opacities.iter_mut() {
            let target = if eol_diagnostic_lines.contains(&line) {
              1.0
            } else {
              0.0
            };
            let delta = target - *current;
            if delta.abs() > 0.01 {
              eol_animating = true;
              *current += eol_anim_rate * delta;
            } else {
              *current = target;
            }
          }

          // Clean up lines that faded out completely
          self
            .eol_diagnostic_opacities
            .retain(|line, opacity| eol_diagnostic_lines.contains(line) || *opacity > 0.01);

          // Keep animating if there are pending lines waiting for debounce
          self.eol_diagnostic_anim_active =
            eol_animating || !self.eol_diagnostic_pending.is_empty();
        }

        // Prepare inline diagnostics config (needed for both animation and rendering)
        let mut inline_diagnostics_config = cx.editor.config().inline_diagnostics.clone();
        if !view.inline_diagnostics_enabled {
          inline_diagnostics_config.cursor_line = DiagnosticFilter::Disable;
          inline_diagnostics_config.other_lines = DiagnosticFilter::Disable;
        }
        let eol_diagnostics = cx.editor.config().end_of_line_diagnostics;
        let inline_decoration_enabled = !inline_diagnostics_config.disabled();
        let eol_decoration_enabled = !matches!(eol_diagnostics, DiagnosticFilter::Disable);

        // Get enable_cursor_line state for config preparation
        let enable_cursor_line = {
          let view = cx.editor.tree.get(focus_view);
          view
            .diagnostics_handler
            .show_cursorline_diagnostics(doc, focus_view)
        };
        let prepared_config = inline_diagnostics_config.prepare(viewport.width, enable_cursor_line);

        // Update inline diagnostic animation state
        {
          use super::inline_diagnostic_animation::{
            InlineDiagnosticAnimTarget,
            update_animation,
          };
          use crate::core::diagnostics::Severity;

          // Compute which lines are DISPLAYING inline diagnostics (not just have them)
          // This considers cursor position and severity filters
          let displaying_inline_diags: std::collections::HashSet<usize> = doc
            .diagnostics()
            .iter()
            .filter_map(|diag| {
              let line = doc_text.char_to_line(diag.range.start.min(doc_text.len_chars()));
              let severity = diag.severity.unwrap_or(Severity::Hint);

              // Check filter based on cursor line vs other lines
              let filter = if line == cursor_line {
                prepared_config.cursor_line
              } else {
                prepared_config.other_lines
              };

              // Only include if filter allows this severity
              match filter {
                DiagnosticFilter::Enable(threshold) if threshold <= severity => Some(line),
                _ => None,
              }
            })
            .collect();

          // Add new displaying lines to animation state
          for &line in &displaying_inline_diags {
            if !self.inline_diagnostic_anim.contains_key(&line) {
              self.inline_diagnostic_anim.insert(line, Default::default());
            }
          }

          // Update existing animations
          let mut inline_animating = false;

          for (&line, state) in self.inline_diagnostic_anim.iter_mut() {
            let target = if displaying_inline_diags.contains(&line) {
              InlineDiagnosticAnimTarget::visible()
            } else {
              InlineDiagnosticAnimTarget::hidden()
            };

            if update_animation(state, target, cx.dt) {
              inline_animating = true;
            }
          }

          // Clean up lines that have fully faded out
          self
            .inline_diagnostic_anim
            .retain(|line, state| displaying_inline_diags.contains(line) || state.opacity > 0.01);

          self.inline_diagnostic_anim_active = inline_animating;
        }

        if inline_decoration_enabled || eol_decoration_enabled {
          let eol_cursor_line_only = cx.editor.config().end_of_line_diagnostics_cursor_line_only;
          let inline_diag = crate::ui::text_decorations::diagnostics::InlineDiagnostics::new(
            doc,
            &cx.editor.theme,
            cursor_pos,
            cursor_line,
            prepared_config,
            eol_diagnostics,
            eol_cursor_line_only,
            &self.eol_diagnostic_opacities,
            &self.inline_diagnostic_anim,
            base_x,
            base_y,
            self.cached_cell_height,
            font_width,
            self.cached_font_size,
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
        // separator and explorer offset)
        let view_right_edge_px = explorer_px_width
          + (content_rect.x + content_rect.width) as f32 * font_width
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
          selection_highlight_ranges
            .iter()
            .any(|&(from, to)| from < end && to > start)
        };

        let mut current_row = usize::MAX;
        let mut current_doc_line = usize::MAX;
        let mut last_doc_line_end_row = 0; // Track the last visual row of the previous doc_line
        let mut grapheme_count = 0;
        let mut line_batch = Vec::new(); // Batch characters on the same line
        let mut rendered_gutter_lines = HashSet::new(); // Track which lines have gutters rendered
        let mut line_end_cols: HashMap<usize, usize> = HashMap::new(); // Track the rightmost column for each doc line
        let mut current_line_max_col = 0usize; // Track max absolute column for current line

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

        // Calculate cursor indent level based on LINE indentation (O(1) - single line)
        let cursor_line = doc_text.char_to_line(cursor_pos.min(doc_text.len_chars()));
        let cursor_line_text = doc_text.line(cursor_line);
        let line_indent_chars = cursor_line_text
          .chars()
          .take_while(|c| c.is_whitespace() && *c != '\n')
          .count();
        let cursor_indent_level = if indent_width > 0 {
          line_indent_chars / indent_width
        } else {
          0
        };

        // Update indent guide animation state with exponential decay
        let anim_rate = 1.0 - 2.0_f32.powf(-60.0 * cx.dt);
        const BASE_OPACITY: f32 = 0.2;
        const MAX_LEVELS: usize = 20; // Reasonable max indent levels to track

        // Track if any animation is still in progress
        let mut any_animating = false;

        // Update opacities for all levels
        for level in 0..MAX_LEVELS {
          let target = if level <= cursor_indent_level {
            // Active scope: opacity decreases with depth from cursor (like raddebugger)
            let depth_from_cursor = cursor_indent_level.saturating_sub(level);
            (1.0 - depth_from_cursor as f32 / 6.0).max(BASE_OPACITY)
          } else {
            BASE_OPACITY
          };

          let current = self
            .indent_guide_opacities
            .entry(level)
            .or_insert(BASE_OPACITY);
          let delta = target - *current;

          // Check if still animating before updating
          if delta.abs() > 0.01 {
            any_animating = true;
            *current += anim_rate * delta;
          } else {
            // Snap when close
            *current = target;
          }
        }

        self.indent_guides_anim_active = any_animating;

        // Update diagnostic glow animation state
        // Collect lines with diagnostics
        let diagnostic_lines: std::collections::HashSet<usize> = doc
          .diagnostics()
          .iter()
          .map(|d| doc_text.char_to_line(d.range.start.min(doc_text.len_chars())))
          .collect();

        // Animate glow opacities with exponential decay (faster rate for glow)
        let glow_anim_rate = 1.0 - 2.0_f32.powf(-30.0 * cx.dt);
        let mut glow_animating = false;

        // Add new diagnostic lines (O(diagnostic_count) instead of O(10000))
        for &line in &diagnostic_lines {
          if !self.diagnostic_glow_opacities.contains_key(&line) {
            self.diagnostic_glow_opacities.insert(line, 0.0);
            glow_animating = true;
          }
        }

        // Animate existing entries toward their targets
        for (line, current) in self.diagnostic_glow_opacities.iter_mut() {
          let target = if diagnostic_lines.contains(line) {
            1.0
          } else {
            0.0
          };
          let delta = target - *current;
          if delta.abs() > 0.01 {
            glow_animating = true;
            *current += glow_anim_rate * delta;
          } else {
            *current = target;
          }
        }

        // Clean up lines that are no longer needed (opacity ~0 and no diagnostic)
        self
          .diagnostic_glow_opacities
          .retain(|line, opacity| diagnostic_lines.contains(line) || *opacity > 0.01);

        self.diagnostic_glow_anim_active = glow_animating;

        // Clone for use in rendering
        let glow_opacities = self.diagnostic_glow_opacities.clone();

        // Clone opacities for use in closure
        let guide_opacities = self.indent_guide_opacities.clone();

        // Helper to draw indent guides for a line
        let draw_indent_guides = |last_indent: usize,
                                  rel_row: usize,
                                  _doc_line: usize,
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

          let y = base_y + (rel_row as f32) * self.cached_cell_height;

          // Draw guides at each indent level
          for i in starting_indent..end_indent {
            let guide_x = base_x + ((i * indent_width).saturating_sub(h_off) as f32) * font_width;

            // Only draw if visible in viewport
            if guide_x >= base_x && guide_x < base_x + (viewport_cols as f32) * font_width {
              // Simple "in scope" check: guide level <= cursor's indent level
              // This avoids expensive O(n) scope boundary scanning
              let in_scope = i <= cursor_indent_level;

              // Get animated opacity only if in scope, otherwise use base
              let opacity = if in_scope {
                guide_opacities.get(&i).copied().unwrap_or(BASE_OPACITY)
              } else {
                BASE_OPACITY
              };
              let mut color = indent_guide_color;
              color.a *= opacity;

              batcher.add_command(RenderCommand::Text {
                section: TextSection::simple(
                  guide_x,
                  y,
                  indent_guide_char.clone(),
                  font_size,
                  color,
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
          let y = base_y + (rel_row as f32) * self.cached_cell_height;

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
                current_doc_line,
                &mut self.command_batcher,
                font_width,
                font_size,
                base_y,
              );
            }

            // Render end-of-line diagnostic for previous line before switching
            if current_doc_line != usize::MAX {
              let prev_line_end_col = line_end_cols
                .remove(&current_doc_line)
                .unwrap_or(current_line_max_col);
              // Render virtual lines for the previous line using the last visual row it ended
              // on
              decoration_manager.render_virtual_lines(
                renderer,
                (current_doc_line, last_doc_line_end_row as u16),
                prev_line_end_col,
              );
            }

            // Decorate the new line
            decoration_manager.decorate_line(renderer, (doc_line, rel_row as u16));
            current_doc_line = doc_line;
            last_doc_line_end_row = rel_row; // Initialize for the new doc_line
            current_line_max_col = 0; // Reset for new line

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

            // Render diagnostic glow if this line has one
            if let Some(&glow_opacity) = glow_opacities.get(&doc_line) {
              if glow_opacity > 0.01 {
                // Find the highest severity diagnostic on this line
                use crate::core::diagnostics::Severity;
                let diagnostics = &doc.diagnostics;
                let first_diag_idx = diagnostics.partition_point(|d| d.line < doc_line);
                let severity = diagnostics
                  .get(first_diag_idx..)
                  .and_then(|diags| diags.iter().find(|d| d.line == doc_line))
                  .and_then(|d| d.severity);

                // Get color based on severity
                let glow_color = match severity {
                  Some(Severity::Error) => {
                    cx.editor
                      .theme
                      .get("error")
                      .fg
                      .map(crate::ui::theme_color_to_renderer_color)
                      .unwrap_or(Color::rgb(0.9, 0.3, 0.3))
                  },
                  Some(Severity::Warning) | None => {
                    cx.editor
                      .theme
                      .get("warning")
                      .fg
                      .map(crate::ui::theme_color_to_renderer_color)
                      .unwrap_or(Color::rgb(0.9, 0.7, 0.3))
                  },
                  Some(Severity::Info) => {
                    cx.editor
                      .theme
                      .get("info")
                      .fg
                      .map(crate::ui::theme_color_to_renderer_color)
                      .unwrap_or(Color::rgb(0.3, 0.7, 0.9))
                  },
                  Some(Severity::Hint) => {
                    cx.editor
                      .theme
                      .get("hint")
                      .fg
                      .map(crate::ui::theme_color_to_renderer_color)
                      .unwrap_or(Color::rgb(0.5, 0.8, 0.5))
                  },
                };

                // Draw gradient glow rectangle in gutter area only (stops at line content)
                // Width: spans from gutter to content area start
                // Opacity: 30% of color, further scaled by glow_opacity
                // Fades from left (full color) to right (transparent)
                let glow_width = (base_x - gutter_x) * glow_opacity;
                let mut final_color = glow_color;
                final_color.a = 0.3 * glow_opacity;

                self
                  .command_batcher
                  .add_command(RenderCommand::GradientRect {
                    x: gutter_x,
                    y,
                    width: glow_width,
                    height: self.cached_cell_height,
                    color: final_color,
                  });
              }
            }
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

          // Track the rightmost absolute column on this line
          let grapheme_width = width_cols;
          current_line_max_col = current_line_max_col.max(abs_col.saturating_add(grapheme_width));

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

          // Call decoration hook for this grapheme
          decoration_manager.decorate_grapheme(&g);

          // Check if this is the cursor position
          let is_primary_cursor_here = g.char_idx == cursor_pos;
          let is_secondary_cursor_here = secondary_cursor_positions.contains(&g.char_idx);
          let is_cursor_here = is_primary_cursor_here || is_secondary_cursor_here;
          let cursor_kind_for_position = cursor_kind;
          let cursor_is_block_here = cursor_kind_for_position == CursorKind::Block;

          // Add selection background command
          // For non-block cursors, exclude cursor position from selection ONLY when
          // there's no actual selection (i.e., when it's just a cursor at a
          // single position, not a range selection) When there's an actual
          // selection, the background should include the cursor position
          let doc_len = g.doc_chars();
          let should_draw_selection = if is_selected(g.char_idx, doc_len) {
            // Only exclude cursor position for non-block cursors when there's no actual
            // selection
            !(is_cursor_here && is_focused && !cursor_is_block_here && !has_selection)
          } else {
            false
          };

          if should_draw_selection {
            self.command_batcher.add_command(RenderCommand::Selection {
              x,
              y,
              width: (draw_cols as f32) * font_width,
              height: self.cached_cell_height,
              color: selection_fill_color,
            });
          }

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
            let (anim_x, anim_y) = if is_primary_cursor_here {
              if cx.editor.config().cursor_anim_enabled {
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
              }
            } else {
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
                // normal already has zoom_alpha applied
                normal
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

            // Use full cell height without centering for better legibility
            let cursor_y = anim_y;

            // Clip cursor to stay within view bounds (both horizontal and vertical)
            let max_cursor_width = (view_right_edge_px - anim_x).max(0.0);
            let clipped_cursor_w = cursor_w.min(max_cursor_width);

            let cursor_height = self.cached_cell_height;
            let max_cursor_height = (view_bottom_edge_px - cursor_y).max(0.0);
            let clipped_cursor_h = cursor_height.min(max_cursor_height);

            // Determine cursor color based on type
            let cursor_color = if cursor_is_block_here {
              // Block cursor: use background color
              cursor_bg_color
            } else {
              // Bar/underline cursor: use foreground color (the line color)
              if let Some(mut cursor_fg) = cursor_fg_from_theme {
                cursor_fg.a *= zoom_alpha;
                cursor_fg
              } else if let Some(color) = syntax_fg {
                // Fallback to syntax color if no explicit cursor fg
                let mut color = crate::ui::theme_color_to_renderer_color(color);
                color.a *= zoom_alpha;
                color
              } else {
                // normal already has zoom_alpha applied
                normal
              }
            };

            self.command_batcher.add_command(RenderCommand::Cursor {
              x:       anim_x,
              y:       cursor_y,
              width:   clipped_cursor_w,
              height:  clipped_cursor_h,
              color:   cursor_color,
              kind:    cursor_kind_for_position,
              primary: is_primary_cursor_here,
            });
          } else if is_cursor_here && !is_focused {
            // Draw hollow cursor for unfocused views to indicate cursor position
            let cursor_w = width_cols.max(1) as f32 * font_width;

            // Clip cursor to stay within view bounds
            let max_cursor_width = (view_right_edge_px - x).max(0.0);
            let clipped_cursor_w = cursor_w.min(max_cursor_width);

            let cursor_height = self.cached_cell_height;
            let max_cursor_height = (view_bottom_edge_px - y).max(0.0);
            let clipped_cursor_h = cursor_height.min(max_cursor_height);

            // Use cursor background color with reduced opacity for hollow cursor
            let hollow_cursor_color = if let Some(mut bg) = cursor_bg_from_theme {
              bg.a *= zoom_alpha * 0.7; // Slightly dimmed for unfocused
              bg
            } else {
              let mut color = Color::rgb(0.2, 0.8, 0.7);
              color.a *= zoom_alpha * 0.7;
              color
            };

            self.command_batcher.add_command(RenderCommand::Cursor {
              x,
              y,
              width: clipped_cursor_w,
              height: clipped_cursor_h,
              color: hollow_cursor_color,
              kind: CursorKind::Hollow,
              primary: is_primary_cursor_here,
            });
          }

          // Store char_idx before match (g gets shadowed inside Grapheme::Other)
          let grapheme_char_idx = g.char_idx;

          // Add text command
          match g.raw {
            Grapheme::Newline => {
              // Store the tracked line end column for this doc line
              line_end_cols.insert(doc_line, current_line_max_col);
              // End of line, no text to draw
            },
            Grapheme::Tab { .. } => {
              // Tabs are rendered as spacing, no text to draw
            },
            Grapheme::Other { ref g } => {
              if left_clip == 0 {
                // Determine foreground color
                // Only invert text color for cursor when view is focused
                let fg = if is_cursor_here && is_focused {
                  if cursor_is_block_here {
                    // Block cursor: use background color as fg (adaptive) or cursor fg from theme
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
                  } else {
                    // Bar/underline cursor: always use cursor foreground color
                    if let Some(mut cursor_fg) = cursor_fg_from_theme {
                      cursor_fg.a *= zoom_alpha;
                      cursor_fg
                    } else if let Some(color) = syntax_fg {
                      // Fallback to syntax color if no explicit cursor fg
                      let mut color = crate::ui::theme_color_to_renderer_color(color);
                      color.a *= zoom_alpha;
                      color
                    } else {
                      normal
                    }
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
            current_doc_line,
            &mut self.command_batcher,
            font_width,
            font_size,
            base_y,
          );
        }

        // Render virtual lines for the last line
        if current_doc_line != usize::MAX {
          let last_line_end_col = line_end_cols
            .remove(&current_doc_line)
            .unwrap_or(current_line_max_col);
          decoration_manager.render_virtual_lines(
            renderer,
            (current_doc_line, last_doc_line_end_row as u16),
            last_line_end_col,
          );
        }

        // If the document is empty or we didn't render any graphemes, at least render
        // the cursor (only for focused view)
        if grapheme_count == 0 && is_focused {
          // Render cursor at position 0 for empty document
          let x = base_x;
          // Use full cell height without centering for better legibility
          let y = base_y;

          // Get cursor shape from config based on current mode
          let cursor_kind = cx.editor.config().cursor_shape.from_mode(cx.editor.mode());
          let primary_cursor_is_block = cursor_kind == CursorKind::Block;

          // Determine cursor colors
          let block_cursor_color = if use_adaptive_cursor {
            // For empty document with adaptive cursor, use normal text color
            // normal already has zoom_alpha applied
            normal
          } else if let Some(mut bg) = cursor_bg_from_theme {
            bg.a *= zoom_alpha;
            bg
          } else {
            // Should not reach here, but default to cyan
            let mut color = Color::rgb(0.2, 0.8, 0.7);
            color.a *= zoom_alpha;
            color
          };

          let primary_cursor_color = if primary_cursor_is_block {
            block_cursor_color
          } else if let Some(mut cursor_fg) = cursor_fg_from_theme {
            cursor_fg.a *= zoom_alpha;
            cursor_fg
          } else {
            // normal already has zoom_alpha applied
            normal
          };

          self.command_batcher.add_command(RenderCommand::Cursor {
            x,
            y,
            width: font_width,
            height: self.cached_cell_height,
            color: primary_cursor_color,
            kind: cursor_kind,
            primary: true,
          });

          for _ in secondary_cursor_positions.iter() {
            self.command_batcher.add_command(RenderCommand::Cursor {
              x,
              y,
              width: font_width,
              height: self.cached_cell_height,
              color: primary_cursor_color,
              kind: cursor_kind,
              primary: false,
            });
          }
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
                let mut effect_base_x =
                  content_x_offset + content_rect.x as f32 * font_width + VIEW_PADDING_LEFT;
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

    // Render split separators
    self.render_split_separators(renderer, cx, font_width, font_size);

    // Render explorer sidebar if open
    if let Some(ref mut explorer) = self.explorer {
      if explorer.is_opened() && explorer_px_width > 0.0 {
        // Viewport height minus statusline
        const STATUS_BAR_HEIGHT: f32 = 28.0;
        let explorer_px_height = renderer.height() as f32 - STATUS_BAR_HEIGHT;

        // Calculate explorer x position based on config
        let explorer_x = match explorer_position {
          FileTreePosition::Left => 0.0,
          FileTreePosition::Right => viewport_px_width - explorer_px_width,
        };

        explorer.render(
          explorer_x,
          0.0, // y position
          explorer_px_width,
          explorer_px_height,
          renderer,
          cx,
        );
      }
    }

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
  /// Render a terminal view.
  ///
  /// Note: Terminal resizing should be done before calling this method,
  /// as this method only needs immutable access to the editor.
  #[allow(clippy::too_many_arguments)]
  fn render_terminal_view(
    &self,
    terminal_id: the_terminal::TerminalId,
    base_x: f32,
    base_y: f32,
    is_focused: bool,
    font_width: f32,
    renderer: &mut Surface,
    editor: &Editor,
  ) {
    let terminal = match editor.terminal(terminal_id) {
      Some(t) => t,
      None => return,
    };

    // Get theme colors for terminal
    let theme = &editor.theme;
    let bg_style = theme.get("ui.background");
    let fg_style = theme.get("ui.text");

    // Build terminal color scheme from theme
    let colors = self.build_terminal_color_scheme_from_theme(theme);

    // Get terminal cells and cursor info
    let (cols, rows) = terminal.dimensions();
    let cells = terminal.render_cells(&colors);
    let cursor_info = terminal.cursor_info();

    // Draw background
    let bg_color = bg_style
      .bg
      .map(crate::ui::theme_color_to_renderer_color)
      .unwrap_or(Color::new(0.1, 0.1, 0.15, 1.0));

    let cell_height = self.cached_cell_height;
    let total_width = cols as f32 * font_width;
    let total_height = rows as f32 * cell_height;

    renderer.draw_rect(base_x, base_y, total_width, total_height, bg_color);

    // Debug: Log terminal metrics once
    static DEBUG_LOGGED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if !DEBUG_LOGGED.swap(true, std::sync::atomic::Ordering::Relaxed) {
      log::info!(
        "TERMINAL DEBUG: font_size={}, cell_width={}, cell_height={}, cols={}, rows={}, \
         line_top_offset={}",
        self.cached_font_size,
        font_width,
        cell_height,
        cols,
        rows,
        renderer.line_top_offset()
      );
    }

    // Get selection range for highlighting
    // Skip selection rendering when TUI app (vim, helix, etc.) controls the display
    let selection_range = if terminal.alt_screen_mode() {
      None
    } else {
      terminal.selection_range()
    };
    let selection_bg = theme
      .get("ui.selection")
      .bg
      .map(crate::ui::theme_color_to_renderer_color)
      .unwrap_or(Color::new(0.3, 0.4, 0.6, 0.5));

    // Get search match range for highlighting
    let search_match_range = if terminal.alt_screen_mode() {
      None
    } else {
      terminal.vi_search_match_range()
    };
    // Use a distinct color for search match (yellow/orange tint)
    let search_match_bg = theme
      .get("ui.selection.primary")
      .bg
      .map(crate::ui::theme_color_to_renderer_color)
      .unwrap_or(Color::new(0.6, 0.5, 0.2, 0.7));

    // Draw cells
    for cell in cells {
      // Use floor() for pixel-perfect grid alignment (matches Zed's approach)
      let x = (base_x + cell.col as f32 * font_width).floor();
      let y = (base_y + cell.row as f32 * cell_height).floor();

      // Draw background if different from default
      let cell_bg = Color::rgb(
        cell.bg.0 as f32 / 255.0,
        cell.bg.1 as f32 / 255.0,
        cell.bg.2 as f32 / 255.0,
      );

      // Calculate cell width (double for wide characters like CJK)
      let char_width = if cell.is_wide {
        font_width * 2.0
      } else {
        font_width
      };

      // Check if cell is in search match (single-line only for simplicity)
      // Note: end_col is exclusive
      let in_search_match =
        search_match_range.map_or(false, |((start_col, start_row), (end_col, end_row))| {
          let cell_row = cell.row as i32;
          let cell_col = cell.col;

          if cell_row < start_row || cell_row > end_row {
            false
          } else if cell_row == start_row && cell_row == end_row {
            // Single line match (most common case)
            cell_col >= start_col && cell_col < end_col
          } else if cell_row == start_row {
            cell_col >= start_col
          } else if cell_row == end_row {
            cell_col < end_col
          } else {
            true
          }
        });

      // Check if cell is in selection
      // Note: end_col is exclusive (one past the last selected column)
      let in_selection =
        selection_range.map_or(false, |((start_col, start_row), (end_col, end_row))| {
          let cell_row = cell.row as i32;
          let cell_col = cell.col;

          if cell_row < start_row || cell_row > end_row {
            false
          } else if cell_row == start_row && cell_row == end_row {
            // Single line selection (end_col is exclusive)
            cell_col >= start_col && cell_col < end_col
          } else if cell_row == start_row {
            // First line of multi-line selection
            cell_col >= start_col
          } else if cell_row == end_row {
            // Last line of multi-line selection (end_col is exclusive)
            cell_col < end_col
          } else {
            // Middle lines are fully selected
            true
          }
        });

      // Draw background: search match takes priority over selection
      // Use ceil() on both width and height to ensure full cell coverage and avoid
      // gaps
      if in_search_match {
        renderer.draw_rect(x, y, char_width.ceil(), cell_height.ceil(), search_match_bg);
      } else if in_selection {
        // Draw selection highlight
        renderer.draw_rect(x, y, char_width.ceil(), cell_height.ceil(), selection_bg);
      } else if cell.bg != colors.background {
        renderer.draw_rect(x, y, char_width.ceil(), cell_height.ceil(), cell_bg);
      }

      // Draw character if not empty/space
      if cell.c != ' ' && cell.c != '\0' {
        let cell_fg = Color::rgb(
          cell.fg.0 as f32 / 255.0,
          cell.fg.1 as f32 / 255.0,
          cell.fg.2 as f32 / 255.0,
        );

        let section = TextSection::simple(x, y, cell.c.to_string(), self.cached_font_size, cell_fg);
        renderer.draw_text_immediate(section);
      }
    }

    // Draw cursor if focused and not scrolled into history
    // When display_offset > 0, the cursor is scrolled off the bottom of the
    // viewport
    let display_offset = terminal.display_offset();
    if is_focused && cursor_info.visible && display_offset == 0 {
      // Use floor() for pixel-perfect cursor alignment
      let cursor_x = (base_x + cursor_info.col as f32 * font_width).floor();
      let cursor_y = (base_y + cursor_info.row as f32 * cell_height).floor();

      let cursor_color = fg_style
        .fg
        .map(crate::ui::theme_color_to_renderer_color)
        .unwrap_or(Color::rgb(0.85, 0.85, 0.9));

      match cursor_info.shape {
        TerminalCursorShape::Block => {
          // Draw block cursor with inverted colors
          renderer.draw_rect(cursor_x, cursor_y, font_width, cell_height, cursor_color);
        },
        TerminalCursorShape::Underline => {
          // Draw underline cursor
          let underline_height = 2.0;
          renderer.draw_rect(
            cursor_x,
            cursor_y + cell_height - underline_height,
            font_width,
            underline_height,
            cursor_color,
          );
        },
        TerminalCursorShape::Beam => {
          // Draw beam/bar cursor
          let beam_width = 2.0;
          renderer.draw_rect(cursor_x, cursor_y, beam_width, cell_height, cursor_color);
        },
      }
    }

    // Draw vi mode cursor if active (hollow block cursor)
    if is_focused && terminal.vi_mode() {
      if let Some((vi_col, vi_row)) = terminal.vi_cursor_position() {
        // Only draw if the vi cursor is in the visible viewport
        if vi_row >= 0 && vi_row < rows as i32 {
          let vi_cursor_x = (base_x + vi_col as f32 * font_width).floor();
          let vi_cursor_y = (base_y + vi_row as f32 * cell_height).floor();

          // Use a distinct color for vi mode cursor (yellowish)
          let vi_cursor_color = Color::new(1.0, 0.9, 0.4, 1.0);
          let border_width = 2.0;

          // Draw hollow block cursor (4 sides)
          // Top
          renderer.draw_rect(
            vi_cursor_x,
            vi_cursor_y,
            font_width,
            border_width,
            vi_cursor_color,
          );
          // Bottom
          renderer.draw_rect(
            vi_cursor_x,
            vi_cursor_y + cell_height - border_width,
            font_width,
            border_width,
            vi_cursor_color,
          );
          // Left
          renderer.draw_rect(
            vi_cursor_x,
            vi_cursor_y,
            border_width,
            cell_height,
            vi_cursor_color,
          );
          // Right
          renderer.draw_rect(
            vi_cursor_x + font_width - border_width,
            vi_cursor_y,
            border_width,
            cell_height,
            vi_cursor_color,
          );
        }
      }
    }

    // Flush text batch
    renderer.flush_text_batch();
  }

  /// Handle key input for terminal views.
  ///
  /// Returns `Some(EventResult)` if the key was handled by terminal,
  /// `None` if normal editor handling should proceed.
  fn handle_terminal_key(&mut self, key: &KeyBinding, cx: &mut Context) -> Option<EventResult> {
    use the_editor_renderer::Key;

    // Check if focused view is a terminal
    let focus = cx.editor.tree.focus;
    let view = cx.editor.tree.get(focus);
    let terminal_id = view.terminal()?;

    // Handle terminal escape prefix (Ctrl+\)
    if self.terminal_escape_pending {
      self.terminal_escape_pending = false;

      match &key.code {
        // Ctrl+\ Ctrl+\ - send literal Ctrl+\ to terminal
        Key::Char('\\') if key.ctrl => {
          if let Some(term) = cx.editor.terminal(terminal_id) {
            term.write(&[0x1C]); // Ctrl+\ = 0x1c
          }
          return Some(EventResult::Consumed(None));
        },
        // Ctrl+\ q - close terminal
        Key::Char('q') => {
          cx.editor.close_terminal(terminal_id);
          // Focus next view or close the view
          return Some(EventResult::Consumed(None));
        },
        // Ctrl+\ n - focus next view
        Key::Char('n') => {
          cx.editor.focus_next();
          return Some(EventResult::Consumed(None));
        },
        // Ctrl+\ p - focus previous view
        Key::Char('p') => {
          cx.editor.focus_prev();
          return Some(EventResult::Consumed(None));
        },
        // Ctrl+\ Escape - reserved
        Key::Escape => {
          return Some(EventResult::Consumed(None));
        },
        // Unknown escape command - ignore
        _ => {
          cx.editor.set_status("Unknown terminal escape command");
          return Some(EventResult::Consumed(None));
        },
      }
    }

    // Check for escape prefix (Ctrl+\)
    if key.ctrl && matches!(&key.code, Key::Char('\\')) {
      self.terminal_escape_pending = true;
      cx.editor.set_status("Terminal: Ctrl+\\ ...");
      return Some(EventResult::Consumed(None));
    }

    // Let window control keys pass through to keymap processing
    // Ctrl+W is the window prefix key
    if key.ctrl && !key.alt && !key.shift && matches!(&key.code, Key::Char('w')) {
      return None;
    }

    // If keymap is in pending state (e.g., after Ctrl+W), pass keys through
    if !self.keymaps.pending().is_empty() {
      return None;
    }

    // Ctrl+Shift+Space - toggle vi mode
    if key.ctrl && key.shift && !key.alt && matches!(&key.code, Key::Char(' ')) {
      if let Some(term) = cx.editor.terminal_mut(terminal_id) {
        term.toggle_vi_mode();
        if term.vi_mode() {
          cx.editor.set_status("-- VI --");
        } else {
          cx.editor.set_status("");
        }
      }
      the_editor_event::request_redraw();
      return Some(EventResult::Consumed(None));
    }

    // Handle vi mode if active
    if let Some(term) = cx.editor.terminal(terminal_id) {
      if term.vi_mode() {
        return self.handle_vi_mode_key(key, terminal_id, cx);
      }
    }

    // Handle terminal clipboard shortcuts
    // Ctrl+Shift+C - copy selection to clipboard
    if key.ctrl && key.shift && !key.alt && matches!(&key.code, Key::Char('c') | Key::Char('C')) {
      if let Some(term) = cx.editor.terminal(terminal_id) {
        if let Some(text) = term.selection_text() {
          if !text.is_empty() {
            let _ = cx.editor.registers.write('+', vec![text.clone()]);
            cx.editor
              .set_status(format!("Copied {} chars to clipboard", text.len()));
          }
        }
      }
      return Some(EventResult::Consumed(None));
    }

    // Ctrl+Shift+V - paste from clipboard
    if key.ctrl && key.shift && !key.alt && matches!(&key.code, Key::Char('v') | Key::Char('V')) {
      if let Some(term) = cx.editor.terminal(terminal_id) {
        // Get text from clipboard ('+' register)
        if let Some(values) = cx.editor.registers.read('+', cx.editor) {
          let text: String = values.collect::<Vec<_>>().join("\n");
          if !text.is_empty() {
            // Use bracketed paste if terminal supports it
            let paste_text = format!("\x1b[200~{}\x1b[201~", text);
            term.write(paste_text.as_bytes());
          }
        }
      }
      return Some(EventResult::Consumed(None));
    }

    // Alt+X - open command mode (: prompt)
    if key.alt && !key.ctrl && !key.shift && matches!(&key.code, Key::Char('x')) {
      let mut cmd_cx = commands::Context {
        register:             cx.editor.selected_register,
        count:                cx.editor.count,
        editor:               cx.editor,
        on_next_key_callback: None,
        callback:             Vec::new(),
        jobs:                 cx.jobs,
      };
      commands::command_mode(&mut cmd_cx);

      // Process callbacks (command_mode pushes a callback to show the prompt)
      let callbacks = cmd_cx.callback;
      if callbacks.is_empty() {
        return Some(EventResult::Consumed(None));
      } else {
        return Some(EventResult::Consumed(Some(Box::new(
          move |compositor, cx| {
            for callback in callbacks {
              callback(compositor, cx);
            }
          },
        ))));
      }
    }

    // Alt+0-9 - quick slots
    if key.alt && !key.ctrl && !key.shift {
      if let Key::Char(c) = &key.code {
        if let Some(slot) = c.to_digit(10) {
          cx.editor.slot_toggle(slot as u8);
          return Some(EventResult::Consumed(None));
        }
      }
    }

    // Convert key to terminal bytes and send
    let bytes = self.key_to_terminal_bytes(key);
    if !bytes.is_empty() {
      if let Some(term) = cx.editor.terminal(terminal_id) {
        // Scroll to bottom when typing (standard terminal behavior)
        term.scroll_to_bottom();
        // Clear any active selection when typing
        term.clear_selection();
        term.write(&bytes);
      }
      the_editor_event::request_redraw();
      return Some(EventResult::Consumed(None));
    }

    Some(EventResult::Consumed(None))
  }

  /// Convert a key event to terminal escape sequence bytes.
  fn key_to_terminal_bytes(&self, key: &KeyBinding) -> Vec<u8> {
    use the_editor_renderer::Key;

    match &key.code {
      // Control characters
      Key::Char(c) if key.ctrl => {
        let c = c.to_ascii_lowercase();
        if c >= 'a' && c <= 'z' {
          vec![(c as u8) - b'a' + 1]
        } else {
          vec![]
        }
      },
      // Alt (Meta) - send ESC prefix
      Key::Char(c) if key.alt => {
        let mut bytes = vec![0x1B]; // ESC
        bytes.extend(c.to_string().as_bytes());
        bytes
      },
      // Regular characters
      Key::Char(c) => c.to_string().into_bytes(),
      // Alt+Enter - send ESC + CR (for nushell multiline, etc.)
      Key::Enter if key.alt => vec![0x1B, b'\r'],
      // Enter
      Key::Enter | Key::NumpadEnter => vec![b'\r'],
      // Tab
      // Shift+Tab - send CSI Z (backtab/reverse tab)
      Key::Tab if key.shift => vec![0x1B, b'[', b'Z'],
      // Ctrl+Tab - some terminals use this, send CSI with modifier
      Key::Tab if key.ctrl => vec![0x1B, b'[', b'2', b'7', b';', b'5', b'~'],
      // Regular Tab
      Key::Tab => vec![b'\t'],
      // Alt+Backspace - send ESC + DEL (word delete in most shells)
      Key::Backspace if key.alt => vec![0x1B, 0x7F],
      // Ctrl+Backspace - send DEL or could use CSI sequence
      Key::Backspace if key.ctrl => vec![0x1B, 0x7F], // Same as Alt+Backspace for compatibility
      // Backspace
      Key::Backspace => vec![0x7F], // DEL
      // Escape
      Key::Escape => vec![0x1B],
      // Arrow keys with modifiers (CSI 1 ; modifier code)
      // Modifier codes: 2=Shift, 3=Alt, 5=Ctrl, 7=Ctrl+Alt
      Key::Up => Self::csi_with_modifiers(b'A', key),
      Key::Down => Self::csi_with_modifiers(b'B', key),
      Key::Right => Self::csi_with_modifiers(b'C', key),
      Key::Left => Self::csi_with_modifiers(b'D', key),
      // Home/End with modifiers
      Key::Home => Self::csi_with_modifiers(b'H', key),
      Key::End => Self::csi_with_modifiers(b'F', key),
      // Page Up/Down with modifiers (CSI number ; modifier ~)
      Key::PageUp => Self::csi_tilde_with_modifiers(5, key),
      Key::PageDown => Self::csi_tilde_with_modifiers(6, key),
      // Insert/Delete with modifiers
      Key::Insert => Self::csi_tilde_with_modifiers(2, key),
      Key::Delete => Self::csi_tilde_with_modifiers(3, key),
      // Function keys
      Key::F1 => b"\x1bOP".to_vec(),
      Key::F2 => b"\x1bOQ".to_vec(),
      Key::F3 => b"\x1bOR".to_vec(),
      Key::F4 => b"\x1bOS".to_vec(),
      Key::F5 => b"\x1b[15~".to_vec(),
      Key::F6 => b"\x1b[17~".to_vec(),
      Key::F7 => b"\x1b[18~".to_vec(),
      Key::F8 => b"\x1b[19~".to_vec(),
      Key::F9 => b"\x1b[20~".to_vec(),
      Key::F10 => b"\x1b[21~".to_vec(),
      Key::F11 => b"\x1b[23~".to_vec(),
      Key::F12 => b"\x1b[24~".to_vec(),
      // Unknown
      Key::Other => vec![],
    }
  }

  /// Generate CSI sequence with modifiers for arrow/home/end keys.
  /// Format: ESC [ 1 ; modifier code (e.g., \x1b[1;5D for Ctrl+Left)
  fn csi_with_modifiers(key_char: u8, key: &KeyBinding) -> Vec<u8> {
    let modifier = Self::modifier_code(key);
    if modifier > 1 {
      format!("\x1b[1;{}{}", modifier, key_char as char).into_bytes()
    } else {
      vec![0x1B, b'[', key_char]
    }
  }

  /// Generate CSI sequence with modifiers for keys using ~ format.
  /// Format: ESC [ number ; modifier ~ (e.g., \x1b[3;5~ for Ctrl+Delete)
  fn csi_tilde_with_modifiers(number: u8, key: &KeyBinding) -> Vec<u8> {
    let modifier = Self::modifier_code(key);
    if modifier > 1 {
      format!("\x1b[{};{}~", number, modifier).into_bytes()
    } else {
      format!("\x1b[{}~", number).into_bytes()
    }
  }

  /// Calculate the modifier code for CSI sequences.
  /// 1 = none, 2 = Shift, 3 = Alt, 4 = Shift+Alt, 5 = Ctrl, 6 = Ctrl+Shift, 7 =
  /// Ctrl+Alt, 8 = all
  fn modifier_code(key: &KeyBinding) -> u8 {
    let mut code = 1u8;
    if key.shift {
      code += 1;
    }
    if key.alt {
      code += 2;
    }
    if key.ctrl {
      code += 4;
    }
    code
  }

  /// Handle key input for terminal vi mode.
  fn handle_vi_mode_key(
    &mut self,
    key: &KeyBinding,
    terminal_id: the_terminal::TerminalId,
    cx: &mut Context,
  ) -> Option<EventResult> {
    use the_editor_renderer::Key;
    use the_terminal::ViMotion;

    let term = cx.editor.terminal_mut(terminal_id)?;

    // Clear pending 'g' state for all keys except 'g' itself
    if !matches!(&key.code, Key::Char('g') if !key.ctrl && !key.alt) {
      term.vi_clear_pending_g();
    }

    match &key.code {
      // Escape - if in visual mode, go back to normal vi mode; otherwise do nothing
      Key::Escape => {
        if term.vi_selection_active() {
          term.vi_clear_selection();
          cx.editor.set_status("-- VI --");
        }
        // Stay in vi mode (don't exit)
      },
      // Exit vi mode - Ctrl+C, i, or a
      Key::Char('c') if key.ctrl => {
        term.exit_vi_mode();
        cx.editor.set_status("");
      },
      Key::Char('i') | Key::Char('a') if !key.ctrl && !key.alt => {
        term.exit_vi_mode();
        cx.editor.set_status("");
      },
      // Toggle visual (character) selection - v
      Key::Char('v') if !key.ctrl && !key.alt => {
        term.vi_toggle_selection();
        if term.vi_selection_active() {
          cx.editor.set_status("-- VISUAL --");
        } else {
          cx.editor.set_status("-- VI --");
        }
      },
      // Toggle visual line selection - V
      Key::Char('V') if !key.ctrl && !key.alt => {
        term.vi_toggle_line_selection();
        if term.vi_selection_active() {
          cx.editor.set_status("-- VISUAL LINE --");
        } else {
          cx.editor.set_status("-- VI --");
        }
      },
      // Yank selection
      Key::Char('y') if !key.ctrl && !key.alt => {
        // Get selection text and exit vi mode before borrowing other editor fields
        let text = term.selection_text();
        term.exit_vi_mode();
        // Now we can safely borrow registers (term borrow ends here)
        if let Some(text) = text {
          if !text.is_empty() {
            let len = text.len();
            let _ = cx.editor.registers.write('+', vec![text]);
            cx.editor.set_status(format!("Yanked {} chars", len));
          }
        }
        the_editor_event::request_redraw();
        return Some(EventResult::Consumed(None));
      },
      // Basic motions
      Key::Char('h') if !key.ctrl && !key.alt => term.vi_motion(ViMotion::Left),
      Key::Char('j') if !key.ctrl && !key.alt => term.vi_motion(ViMotion::Down),
      Key::Char('k') if !key.ctrl && !key.alt => term.vi_motion(ViMotion::Up),
      Key::Char('l') if !key.ctrl && !key.alt => term.vi_motion(ViMotion::Right),
      Key::Left => term.vi_motion(ViMotion::Left),
      Key::Down => term.vi_motion(ViMotion::Down),
      Key::Up => term.vi_motion(ViMotion::Up),
      Key::Right => term.vi_motion(ViMotion::Right),
      // Line motions
      Key::Char('0') if !key.ctrl && !key.alt => term.vi_motion(ViMotion::First),
      Key::Char('$') if !key.ctrl && !key.alt => term.vi_motion(ViMotion::Last),
      Key::Char('^') if !key.ctrl && !key.alt => term.vi_motion(ViMotion::FirstOccupied),
      // Word motions (semantic)
      Key::Char('w') if !key.ctrl && !key.alt => term.vi_motion(ViMotion::SemanticRight),
      Key::Char('b') if !key.ctrl && !key.alt => term.vi_motion(ViMotion::SemanticLeft),
      Key::Char('e') if !key.ctrl && !key.alt => term.vi_motion(ViMotion::SemanticRightEnd),
      // Word motions (whitespace)
      Key::Char('W') if !key.ctrl && !key.alt => term.vi_motion(ViMotion::WordRight),
      Key::Char('B') if !key.ctrl && !key.alt => term.vi_motion(ViMotion::WordLeft),
      Key::Char('E') if !key.ctrl && !key.alt => term.vi_motion(ViMotion::WordRightEnd),
      // Screen motions
      Key::Char('H') if !key.ctrl && !key.alt => term.vi_motion(ViMotion::High),
      Key::Char('M') if !key.ctrl && !key.alt => term.vi_motion(ViMotion::Middle),
      Key::Char('L') if !key.ctrl && !key.alt => term.vi_motion(ViMotion::Low),
      // Paragraph motions
      Key::Char('{') if !key.ctrl && !key.alt => term.vi_motion(ViMotion::ParagraphUp),
      Key::Char('}') if !key.ctrl && !key.alt => term.vi_motion(ViMotion::ParagraphDown),
      // Bracket matching
      Key::Char('%') if !key.ctrl && !key.alt => term.vi_motion(ViMotion::Bracket),
      // Scroll (Ctrl+B/F/U/D)
      Key::Char('b') if key.ctrl => {
        let rows = term.dimensions().1 as i32;
        // Positive = scroll up (back toward history)
        term.vi_scroll(rows);
      },
      Key::Char('f') if key.ctrl => {
        let rows = term.dimensions().1 as i32;
        // Negative = scroll down (forward toward current)
        term.vi_scroll(-rows);
      },
      Key::Char('u') if key.ctrl => {
        let rows = term.dimensions().1 as i32;
        // Positive = scroll up (toward history)
        term.vi_scroll(rows / 2);
      },
      Key::Char('d') if key.ctrl => {
        let rows = term.dimensions().1 as i32;
        // Negative = scroll down (toward current)
        term.vi_scroll(-rows / 2);
      },
      // Go to top (gg) - requires two g presses
      Key::Char('g') if !key.ctrl && !key.alt => {
        if term.vi_pending_g() {
          // Second g - execute gg motion (go to absolute top of history)
          term.vi_clear_pending_g();
          term.vi_goto_top();
        } else {
          // First g - set pending state
          term.vi_set_pending_g();
        }
      },
      // Go to bottom (G)
      Key::Char('G') if !key.ctrl && !key.alt => {
        term.vi_clear_pending_g();
        term.vi_goto_bottom();
      },
      // Search forward - / opens search prompt
      Key::Char('/') if !key.ctrl && !key.alt => {
        // Open search prompt via callback
        let term_id = terminal_id;
        let callback: crate::ui::compositor::Callback = Box::new(move |compositor, cx| {
          // Set custom mode string to SEARCH
          cx.editor.set_custom_mode_str("SEARCH".to_string());

          // Trigger statusline slide animation like buffer search
          for layer in compositor.layers.iter_mut() {
            if let Some(statusline) = layer
              .as_any_mut()
              .downcast_mut::<crate::ui::components::statusline::StatusLine>(
            ) {
              statusline.slide_for_prompt(true);
              break;
            }
          }

          let prompt = crate::ui::components::Prompt::new(String::new()).with_callback(
            move |cx, input, event| {
              use crate::ui::components::prompt::PromptEvent;
              log::debug!(
                "Terminal search prompt callback: event={:?}, input='{}'",
                event,
                input
              );
              match event {
                PromptEvent::Validate => {
                  // Clear custom mode string
                  cx.editor.clear_custom_mode_str();
                  if !input.is_empty() {
                    log::debug!("Terminal search: looking up terminal {}", term_id.0);
                    if let Some(term) = cx.editor.terminal_mut(term_id) {
                      log::debug!("Terminal search: got terminal, vi_mode={}", term.vi_mode());
                      match term.vi_set_search(input) {
                        Ok(()) => {
                          log::debug!("Terminal search: pattern set, calling search_next");
                          // Search for first match
                          if term.vi_search_next() {
                            cx.editor.set_status(format!("/{}", input));
                          } else {
                            cx.editor
                              .set_status(format!("Pattern not found: {}", input));
                          }
                        },
                        Err(e) => cx.editor.set_status(e),
                      }
                    } else {
                      log::debug!("Terminal search: terminal not found!");
                    }
                  }
                },
                PromptEvent::Abort => {
                  cx.editor.clear_custom_mode_str();
                  cx.editor.set_status("-- VI --");
                },
                PromptEvent::Update => {},
              }
            },
          );
          compositor.push(Box::new(prompt));
        });
        return Some(EventResult::Consumed(Some(callback)));
      },
      // Search next - n
      Key::Char('n') if !key.ctrl && !key.alt => {
        if term.vi_search_active() {
          if !term.vi_search_next() {
            cx.editor.set_status("No more matches");
          }
        } else {
          cx.editor.set_status("No search pattern");
        }
      },
      // Search previous - N
      Key::Char('N') if !key.ctrl && !key.alt => {
        if term.vi_search_active() {
          if !term.vi_search_prev() {
            cx.editor.set_status("No more matches");
          }
        } else {
          cx.editor.set_status("No search pattern");
        }
      },
      // Other keys - ignore in vi mode
      _ => {},
    }

    the_editor_event::request_redraw();
    Some(EventResult::Consumed(None))
  }

  /// Build a terminal color scheme from the given theme.
  fn build_terminal_color_scheme_from_theme(
    &self,
    theme: &crate::core::theme::Theme,
  ) -> TerminalColorScheme {
    // Debug: Log theme name to verify it's changing
    static LAST_THEME: std::sync::Mutex<String> = std::sync::Mutex::new(String::new());
    {
      let mut last = LAST_THEME.lock().unwrap();
      if *last != theme.name() {
        log::info!("Terminal color scheme: theme changed to '{}'", theme.name());
        *last = theme.name().to_string();
      }
    }

    // Helper to extract RGB from theme style
    let get_color = |key: &str, is_fg: bool| -> (u8, u8, u8) {
      let style = theme.get(key);
      let color = if is_fg { style.fg } else { style.bg };
      color.and_then(theme_color_to_rgb).unwrap_or(if is_fg {
        (204, 204, 204)
      } else {
        (30, 30, 30)
      })
    };

    // Try to get terminal-specific colors from theme, fallback to UI colors
    let foreground = theme
      .get("terminal.foreground")
      .fg
      .and_then(theme_color_to_rgb)
      .unwrap_or_else(|| get_color("ui.text", true));
    let background = theme
      .get("terminal.background")
      .fg
      .and_then(theme_color_to_rgb)
      .unwrap_or_else(|| get_color("ui.background", false));
    let cursor = theme
      .get("terminal.cursor")
      .fg
      .and_then(theme_color_to_rgb)
      .unwrap_or_else(|| get_color("ui.cursor", true));

    // ANSI colors - try theme-specific colors, fallback to reasonable defaults
    TerminalColorScheme {
      foreground,
      background,
      cursor,
      black: theme
        .get("terminal.black")
        .fg
        .and_then(theme_color_to_rgb)
        .unwrap_or((0, 0, 0)),
      red: theme
        .get("terminal.red")
        .fg
        .and_then(theme_color_to_rgb)
        .unwrap_or((204, 0, 0)),
      green: theme
        .get("terminal.green")
        .fg
        .and_then(theme_color_to_rgb)
        .unwrap_or((0, 204, 0)),
      yellow: theme
        .get("terminal.yellow")
        .fg
        .and_then(theme_color_to_rgb)
        .unwrap_or((204, 204, 0)),
      blue: theme
        .get("terminal.blue")
        .fg
        .and_then(theme_color_to_rgb)
        .unwrap_or((0, 0, 204)),
      magenta: theme
        .get("terminal.magenta")
        .fg
        .and_then(theme_color_to_rgb)
        .unwrap_or((204, 0, 204)),
      cyan: theme
        .get("terminal.cyan")
        .fg
        .and_then(theme_color_to_rgb)
        .unwrap_or((0, 204, 204)),
      white: theme
        .get("terminal.white")
        .fg
        .and_then(theme_color_to_rgb)
        .unwrap_or((204, 204, 204)),
      bright_black: theme
        .get("terminal.bright_black")
        .fg
        .and_then(theme_color_to_rgb)
        .unwrap_or((128, 128, 128)),
      bright_red: theme
        .get("terminal.bright_red")
        .fg
        .and_then(theme_color_to_rgb)
        .unwrap_or((255, 0, 0)),
      bright_green: theme
        .get("terminal.bright_green")
        .fg
        .and_then(theme_color_to_rgb)
        .unwrap_or((0, 255, 0)),
      bright_yellow: theme
        .get("terminal.bright_yellow")
        .fg
        .and_then(theme_color_to_rgb)
        .unwrap_or((255, 255, 0)),
      bright_blue: theme
        .get("terminal.bright_blue")
        .fg
        .and_then(theme_color_to_rgb)
        .unwrap_or((0, 0, 255)),
      bright_magenta: theme
        .get("terminal.bright_magenta")
        .fg
        .and_then(theme_color_to_rgb)
        .unwrap_or((255, 0, 255)),
      bright_cyan: theme
        .get("terminal.bright_cyan")
        .fg
        .and_then(theme_color_to_rgb)
        .unwrap_or((0, 255, 255)),
      bright_white: theme
        .get("terminal.bright_white")
        .fg
        .and_then(theme_color_to_rgb)
        .unwrap_or((255, 255, 255)),
    }
  }

  fn execute_command_sequence<I>(&mut self, cx: &mut Context, commands: I) -> EventResult
  where
    I: IntoIterator<Item = MappableCommand>,
  {
    let mut pending_callbacks: Vec<commands::Callback> = Vec::new();

    for command in commands {
      self.run_command(cx, command, &mut pending_callbacks);
    }

    if pending_callbacks.is_empty() {
      EventResult::Consumed(None)
    } else {
      EventResult::Consumed(Some(Box::new(move |compositor, cx| {
        for callback in pending_callbacks {
          callback(compositor, cx);
        }
      })))
    }
  }

  fn run_command(
    &mut self,
    cx: &mut Context,
    command: MappableCommand,
    pending_callbacks: &mut Vec<commands::Callback>,
  ) {
    let register = cx.editor.selected_register;
    let count = cx.editor.count;

    let mut cmd_cx = commands::Context {
      register,
      count,
      editor: cx.editor,
      on_next_key_callback: None,
      callback: Vec::new(),
      jobs: cx.jobs,
    };

    command.execute(&mut cmd_cx);

    let on_next_key = cmd_cx.on_next_key_callback;

    let commands::Context {
      register: new_register,
      count: new_count,
      callback: callbacks,
      ..
    } = cmd_cx;

    if let Some(on_next_key) = on_next_key {
      self.on_next_key = Some(on_next_key);
    }

    cx.editor.selected_register = new_register;
    cx.editor.count = new_count;

    self.update_post_command(cx);

    pending_callbacks.extend(callbacks);
  }

  fn update_post_command(&mut self, cx: &mut Context) {
    // Capture immutable values before mutable borrow
    let mode_after = cx.editor.mode();
    let scrolloff = cx.editor.config().scrolloff;

    // Skip document-specific updates if focused on a terminal view
    let Some((view, doc)) = crate::try_current!(cx.editor) else {
      return;
    };

    let (start_line, end_line) = {
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

  fn render_split_separators(
    &mut self,
    renderer: &mut Surface,
    cx: &Context,
    font_width: f32,
    _font_size: f32,
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
      // Skip separators for closing views - they're animating out
      if tree.is_closing(view.id) {
        continue;
      }

      // Use animated area for separator positioning
      let area = tree.get_animated_area(view.id).unwrap_or(view.area);

      // Check for right neighbor and draw separator at right edge
      let right_neighbor = tree.find_split_in_direction(view.id, Direction::Right);
      if let Some(right_id) = right_neighbor {
        // Only draw if the right neighbor is not closing
        if !tree.is_closing(right_id) {
          let gap_center_x = self.content_x_offset()
            + (area.x + area.width) as f32 * font_width
            + (font_width / 2.0);
          let x = gap_center_x - (SEPARATOR_WIDTH_PX / 2.0);
          let y = area.y as f32 * self.cached_cell_height + self.bufferline_y_adjustment();
          let width = SEPARATOR_WIDTH_PX;
          let height = area.height as f32 * self.cached_cell_height;
          renderer.draw_rect(x, y, width, height, separator_color);
        }
      }

      // Check for left neighbor closing - draw separator at our left edge
      let left_neighbor = tree.find_split_in_direction(view.id, Direction::Left);
      if let Some(left_id) = left_neighbor {
        if tree.is_closing(left_id) {
          // Left neighbor is closing, draw separator at our animated left edge
          let gap_center_x =
            self.content_x_offset() + area.x as f32 * font_width - (font_width / 2.0);
          let x = gap_center_x - (SEPARATOR_WIDTH_PX / 2.0);
          let y = area.y as f32 * self.cached_cell_height + self.bufferline_y_adjustment();
          let width = SEPARATOR_WIDTH_PX;
          let height = area.height as f32 * self.cached_cell_height;
          renderer.draw_rect(x, y, width, height, separator_color);
        }
      }

      // Check for bottom neighbor and draw separator at bottom edge
      let down_neighbor = tree.find_split_in_direction(view.id, Direction::Down);
      if let Some(down_id) = down_neighbor {
        if !tree.is_closing(down_id) {
          let x = self.content_x_offset() + area.x as f32 * font_width;
          let sep_y = (area.y + area.height) as f32 * self.cached_cell_height
            + self.bufferline_y_adjustment()
            - SEPARATOR_HEIGHT_PX;
          let width = area.width as f32 * font_width;
          let height = SEPARATOR_HEIGHT_PX;
          renderer.draw_rect(x, sep_y, width, height, separator_color);
        }
      }

      // Check for top neighbor closing - draw separator at our top edge
      let up_neighbor = tree.find_split_in_direction(view.id, Direction::Up);
      if let Some(up_id) = up_neighbor {
        if tree.is_closing(up_id) {
          // Top neighbor is closing, draw separator at our animated top edge
          let x = self.content_x_offset() + area.x as f32 * font_width;
          let sep_y = area.y as f32 * self.cached_cell_height + self.bufferline_y_adjustment();
          let width = area.width as f32 * font_width;
          let height = SEPARATOR_HEIGHT_PX;
          renderer.draw_rect(x, sep_y, width, height, separator_color);
        }
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

    // Render keymap infobox if pending keys
    let has_autoinfo = cx.editor.autoinfo.is_some();

    // Detect transition: autoinfo just appeared
    if has_autoinfo && !self.had_autoinfo {
      // Start animation (exponential decay RAD style)
      let (duration, easing) = crate::core::animation::presets::MEDIUM;
      self.infobox_animation = Some(crate::core::animation::AnimationHandle::new(
        0.0, 1.0, duration, easing,
      ));
    } else if !has_autoinfo {
      // Clear animation when autoinfo disappears
      self.infobox_animation = None;
    }
    self.had_autoinfo = has_autoinfo;

    // Update and render infobox with animation
    if let Some(ref info) = cx.editor.autoinfo {
      // Update animation
      let anim_t = if let Some(ref mut anim) = self.infobox_animation {
        anim.update(cx.dt);
        *anim.current()
      } else {
        1.0 // Fallback to fully visible
      };
      self.render_infobox(info, renderer, cx, anim_t);
    }
  }

  /// Render keymap infobox (which-key style popup) at bottom-right
  fn render_infobox(&self, info: &Info, renderer: &mut Surface, cx: &Context, anim_t: f32) {
    use crate::ui::{
      UI_FONT_SIZE,
      theme_color_to_renderer_color,
    };

    let theme = &cx.editor.theme;

    // Animation effects (RAD Debugger style exponential decay)
    let alpha = anim_t;
    let scale = 0.92 + (anim_t * 0.08); // 92% → 100%
    let slide_offset = (1.0 - anim_t) * 16.0; // Slide up from bottom

    // Get colors from theme with animation alpha applied
    let mut bg_color = theme
      .get("ui.popup")
      .bg
      .map(theme_color_to_renderer_color)
      .unwrap_or(Color::new(0.12, 0.12, 0.15, 0.95));
    bg_color.a *= alpha;

    let mut border_color = theme
      .get("ui.popup")
      .fg
      .map(theme_color_to_renderer_color)
      .unwrap_or(Color::new(0.3, 0.3, 0.35, 0.8));
    border_color.a *= alpha;

    let mut title_color = theme
      .get("ui.text.focus")
      .fg
      .map(theme_color_to_renderer_color)
      .unwrap_or(Color::new(0.9, 0.85, 0.6, 1.0));
    title_color.a *= alpha;

    let mut text_color = theme
      .get("ui.text")
      .fg
      .map(theme_color_to_renderer_color)
      .unwrap_or(Color::new(0.7, 0.7, 0.7, 1.0));
    text_color.a *= alpha;

    // Use UI font for consistent sizing
    let font_family = renderer.current_font_family().to_string();
    renderer.configure_font(&font_family, UI_FONT_SIZE);
    let cell_width = renderer.cell_width();
    let cell_height = renderer.cell_height();

    // Calculate dimensions
    let padding_x = 12.0;
    let padding_y = 8.0;
    let line_height = cell_height * 1.3;
    let corner_radius = 6.0;
    let border_thickness = 1.0;

    // Position constraints
    let statusline_height = 28.0;
    let margin = 8.0;
    let viewport_width = renderer.width() as f32;
    let viewport_height = renderer.height() as f32;

    // Calculate available height (viewport minus statusline, margins, and some top
    // padding)
    let min_top_margin = 40.0; // Keep some space at top
    let available_height = viewport_height - statusline_height - margin - min_top_margin;

    // Calculate content size with max height constraint
    let title_width = info.title.len() as f32 * cell_width;
    let content_width = info.width as f32 * cell_width;
    let box_width = content_width.max(title_width) + padding_x * 2.0;

    // Calculate how many lines we can show
    let header_height = line_height * 1.5 + padding_y * 2.0; // title + separator + padding
    let max_content_height = available_height - header_height;
    let max_visible_lines = (max_content_height / line_height).floor() as usize;

    let lines: Vec<&str> = info.text.lines().collect();
    let total_lines = lines.len();
    let (visible_lines, is_truncated) = if total_lines > max_visible_lines && max_visible_lines > 1
    {
      // Reserve one line for "..." indicator
      (&lines[..max_visible_lines.saturating_sub(1)], true)
    } else {
      (&lines[..], false)
    };

    let displayed_lines = visible_lines.len() + if is_truncated { 1 } else { 0 };
    let content_height = displayed_lines as f32 * line_height;
    let box_height = header_height + content_height;

    // Calculate base position at bottom-right (above statusline)
    let base_box_x = viewport_width - box_width - margin;
    let base_box_y =
      (viewport_height - box_height - statusline_height - margin).max(min_top_margin);

    // Apply animation transforms (scale around center, slide up)
    let scaled_width = box_width * scale;
    let scaled_height = box_height * scale;
    let box_x = base_box_x + (box_width - scaled_width) / 2.0;
    let box_y = base_box_y + (box_height - scaled_height) / 2.0 + slide_offset;
    let box_width = scaled_width;
    let box_height = scaled_height;

    // Use overlay region to mask underlying content
    renderer.with_overlay_region(box_x, box_y, box_width, box_height, |renderer| {
      // Draw background with rounded corners
      renderer.draw_rounded_rect(box_x, box_y, box_width, box_height, corner_radius, bg_color);

      // Draw border
      renderer.draw_rounded_rect_stroke(
        box_x,
        box_y,
        box_width,
        box_height,
        corner_radius,
        border_thickness,
        border_color,
      );

      // Draw title
      let title_x = box_x + padding_x;
      let title_y = box_y + padding_y;
      renderer.draw_text(TextSection {
        position: (title_x, title_y),
        texts:    vec![TextSegment {
          content: info.title.to_string(),
          style:   TextStyle {
            size:  UI_FONT_SIZE,
            color: title_color,
          },
        }],
      });

      // Draw separator line below title
      let sep_y = title_y + line_height;
      let sep_color = Color::new(border_color.r, border_color.g, border_color.b, 0.4 * alpha);
      renderer.draw_rect(
        box_x + padding_x,
        sep_y,
        box_width - padding_x * 2.0,
        1.0,
        sep_color,
      );

      // Draw content (pre-formatted text with newlines)
      let content_y = sep_y + line_height * 0.5;
      for (i, line) in visible_lines.iter().enumerate() {
        let y = content_y + (i as f32 * line_height);
        renderer.draw_text(TextSection {
          position: (title_x, y),
          texts:    vec![TextSegment {
            content: line.to_string(),
            style:   TextStyle {
              size:  UI_FONT_SIZE,
              color: text_color,
            },
          }],
        });
      }

      // Draw truncation indicator if needed
      if is_truncated {
        let truncate_y = content_y + (visible_lines.len() as f32 * line_height);
        let more_count = total_lines - visible_lines.len();
        let truncate_text = format!("... +{} more", more_count);
        renderer.draw_text(TextSection {
          position: (title_x, truncate_y),
          texts:    vec![TextSegment {
            content: truncate_text,
            style:   TextStyle {
              size:  UI_FONT_SIZE,
              color: Color::new(text_color.r, text_color.g, text_color.b, 0.5 * alpha),
            },
          }],
        });
      }
    });

    // Restore font configuration
    renderer.configure_font(&font_family, self.cached_font_size);
  }

  fn dispatch_signature_help_event(
    &mut self,
    event: &Event,
    cx: &mut Context,
  ) -> Option<EventResult> {
    let sig_help = self.signature_help.as_mut()?;
    Some(Component::handle_event(sig_help, event, cx))
  }

  /// Handle mouse events (clicks, drags, etc.)
  fn handle_mouse_event(
    &mut self,
    mouse: &the_editor_renderer::MouseEvent,
    cx: &mut Context,
  ) -> EventResult {
    // Track mouse position for scroll targeting
    self.last_mouse_pos = Some(mouse.position);

    // Calculate viewport width for explorer area detection
    let viewport_px_width =
      cx.editor.tree.area().width as f32 * self.cached_cell_width + self.explorer_px_width;

    // Handle explorer mouse interaction
    if self.explorer_px_width > 0.0 {
      let in_explorer_area = self.is_in_explorer_area(mouse.position.0, viewport_px_width);

      if let Some(ref mut explorer) = self.explorer {
        if explorer.is_opened() {
          if in_explorer_area {
            // Calculate visual row from mouse Y position
            // Header height matches bufferline: (base_cell_height + 10.0).max(UI_FONT_SIZE
            // + 16.0)
            let base_cell_height = self.cached_cell_height.max(crate::ui::UI_FONT_SIZE + 4.0);
            let header_height = (base_cell_height + 10.0).max(crate::ui::UI_FONT_SIZE + 16.0) + 1.0; // +1 for separator
            // Item height matches tree.rs: line_height (UI_FONT_SIZE) + item_padding_y
            // (4.0) * 2 Plus item_gap (2.0) between items
            let item_padding_y = 4.0;
            let item_gap = 2.0;
            let item_height = crate::ui::UI_FONT_SIZE + item_padding_y * 2.0 + item_gap;

            if mouse.position.1 > header_height {
              let relative_y = mouse.position.1 - header_height;
              let visual_row = (relative_y / item_height).floor() as usize;

              // Update hover state for hover glow animation
              let new_hover = if visual_row < explorer.visible_item_count() {
                Some(visual_row)
              } else {
                None
              };
              if self.explorer_hovered_item != new_hover {
                self.explorer_hovered_item = new_hover;
                explorer.set_hovered_row(new_hover);
                request_redraw();
              }

              // Always update mouse position for glow effect
              explorer.set_mouse_pos(Some((mouse.position.0, mouse.position.1)));

              match mouse.button {
                Some(the_editor_renderer::MouseButton::Left) if mouse.pressed => {
                  self.last_click_time = Some(std::time::Instant::now());
                  self.last_click_pos = Some(mouse.position);

                  // Focus explorer if not focused
                  if !explorer.is_focus() {
                    explorer.focus();
                  }

                  // Single click activates (opens file/toggles folder)
                  explorer.handle_mouse_click(visual_row, true, cx);
                  request_redraw();
                  return EventResult::Consumed(None);
                },
                _ => {},
              }
            } else {
              // Mouse is over header, clear hover
              if self.explorer_hovered_item.is_some() {
                self.explorer_hovered_item = None;
                explorer.set_hovered_row(None);
                explorer.set_mouse_pos(None);
                request_redraw();
              }
            }

            // Consume all mouse events in explorer area (don't pass through)
            return EventResult::Consumed(None);
          } else {
            // Mouse moved outside explorer area - clear hover
            if self.explorer_hovered_item.is_some() {
              self.explorer_hovered_item = None;
              explorer.set_hovered_row(None);
              explorer.set_mouse_pos(None);
              request_redraw();
            }

            // Clicked outside explorer area - unfocus explorer if it was focused
            if explorer.is_focus() {
              if let Some(the_editor_renderer::MouseButton::Left) = mouse.button {
                if mouse.pressed {
                  explorer.unfocus();
                  request_redraw();
                  // Don't return - let the click pass through to the editor
                }
              }
            }
          }
        }
      }
    }

    if self.bufferline_visible {
      let buffer_height = if self.bufferline_height > 0.0 {
        self.bufferline_height
      } else {
        self.cached_cell_height
      };

      // Check if mouse is within bufferline area, accounting for explorer position
      let within_bufferline_y = mouse.position.1 >= 0.0 && mouse.position.1 <= buffer_height;
      let within_bufferline_x = if self.explorer_px_width > 0.0 {
        match self.explorer_position {
          crate::editor::FileTreePosition::Left => mouse.position.0 >= self.explorer_px_width,
          crate::editor::FileTreePosition::Right => {
            mouse.position.0 < viewport_px_width - self.explorer_px_width
          },
        }
      } else {
        true // No explorer, full width is bufferline
      };
      let within_bufferline = within_bufferline_y && within_bufferline_x;

      if within_bufferline {
        let (mx, my) = mouse.position;

        // Check if hovering over add button
        let in_add_button = self.add_button_rect.map_or(false, |rect| {
          let rx = rect.x as f32;
          let ry = rect.y as f32;
          let rw = rect.width as f32;
          let rh = rect.height as f32;
          mx >= rx && mx < rx + rw && my >= ry && my < ry + rh
        });

        // Find which tab is hit
        let hit_index = self
          .buffer_tabs
          .iter()
          .position(|tab| mx >= tab.start_x && mx < tab.end_x);

        // Check if hovering over close button within a tab
        let close_hit_index = hit_index.and_then(|idx| {
          let tab = &self.buffer_tabs[idx];
          if mx >= tab.close_start_x && mx < tab.close_end_x {
            Some(idx)
          } else {
            None
          }
        });

        // Update hover states
        let hover_changed = self.buffer_hover_index != hit_index
          || self.buffer_close_hover_index != close_hit_index
          || self.add_button_hovered != in_add_button;

        if hover_changed {
          self.buffer_hover_index = hit_index;
          self.buffer_close_hover_index = close_hit_index;
          self.add_button_hovered = in_add_button;
          self.dirty_region.mark_all_dirty();
          request_redraw();
        }

        match mouse.button {
          // Left click press
          Some(the_editor_renderer::MouseButton::Left) if mouse.pressed => {
            // Click on add button
            if in_add_button {
              self.add_button_pressed = true;
              self.dirty_region.mark_all_dirty();
              request_redraw();
              return EventResult::Consumed(None);
            }

            // Click on close button
            if let Some(idx) = close_hit_index {
              self.buffer_close_pressed_index = Some(idx);
              self.dirty_region.mark_all_dirty();
              request_redraw();
              return EventResult::Consumed(None);
            }

            // Click on tab (switch to buffer)
            if self.buffer_pressed_index != hit_index {
              self.buffer_pressed_index = hit_index;
              self.dirty_region.mark_all_dirty();
            }
            if let Some(idx) = hit_index {
              match self.buffer_tabs[idx].kind {
                bufferline::BufferKind::Document(doc_id) => {
                  // First, try to find an existing view showing this document
                  let target_view = cx
                    .editor
                    .tree
                    .views()
                    .find_map(|(view, _)| (view.doc() == Some(doc_id)).then_some(view.id));

                  if let Some(view_id) = target_view {
                    cx.editor.focus(view_id);
                  } else if let Some(view_id) = cx.editor.focused_view_id() {
                    // Check if current view is a terminal
                    let current_terminal = cx.editor.tree.get(view_id).terminal();
                    if current_terminal.is_some() {
                      // Current view is a terminal - replace it with the document
                      // Note: We don't hide the terminal, just switch this view to show the
                      // document The terminal stays "visible" in the
                      // bufferline for easy switching back
                      let view = cx.editor.tree.get_mut(view_id);
                      view.set_doc(doc_id);
                      // Initialize the document for this view
                      if let Some(doc) = cx.editor.documents.get_mut(&doc_id) {
                        doc.ensure_view_init(view_id);
                      }
                    } else {
                      // Current view is a document, use normal switch
                      let current_doc = cx.editor.tree.get(view_id).doc();
                      if current_doc != Some(doc_id) {
                        cx.editor.switch(doc_id, Action::Replace, false);
                      }
                    }
                  } else if let Some(new_view_id) = cx.editor.open_view_for_document(doc_id) {
                    cx.editor.focus(new_view_id);
                  } else {
                    cx.editor
                      .set_error("No document view available to show this buffer");
                  }
                },
                bufferline::BufferKind::Terminal(terminal_id) => {
                  // Find if there's already a view showing this terminal
                  let target_view = cx.editor.tree.views().find_map(|(view, _)| {
                    (view.terminal() == Some(terminal_id)).then_some(view.id)
                  });

                  if let Some(view_id) = target_view {
                    cx.editor.focus(view_id);
                  } else {
                    cx.editor.show_terminal(terminal_id, Action::Replace);
                  }
                },
              }
            }
            request_redraw();
            self.dirty_region.mark_all_dirty();
            return EventResult::Consumed(None);
          },

          // Left click release
          Some(the_editor_renderer::MouseButton::Left) => {
            // Release on add button - create scratch buffer
            if self.add_button_pressed && in_add_button {
              self.add_button_pressed = false;
              cx.editor.create_scratch_buffer(Action::Replace);
              self.dirty_region.mark_all_dirty();
              request_redraw();
              return EventResult::Consumed(None);
            }
            self.add_button_pressed = false;

            // Release on close button - close the document or terminal
            if let Some(idx) = close_hit_index {
              if self.buffer_close_pressed_index == Some(idx) {
                if let Some(tab) = self.buffer_tabs.get(idx) {
                  match tab.kind {
                    bufferline::BufferKind::Document(doc_id) => {
                      // Close the document
                      let _ = cx.editor.close_document(doc_id, false);
                    },
                    bufferline::BufferKind::Terminal(terminal_id) => {
                      cx.editor.destroy_terminal(terminal_id);
                    },
                  }
                }
              }
            }

            // Clear pressed states
            if self.buffer_pressed_index.take().is_some() {
              self.dirty_region.mark_all_dirty();
            }
            if self.buffer_close_pressed_index.take().is_some() {
              self.dirty_region.mark_all_dirty();
            }
            request_redraw();
            return EventResult::Consumed(None);
          },

          // Middle click - close tab (raddebugger behavior)
          Some(the_editor_renderer::MouseButton::Middle) if mouse.pressed => {
            if let Some(idx) = hit_index {
              if let Some(tab) = self.buffer_tabs.get(idx) {
                match tab.kind {
                  bufferline::BufferKind::Document(doc_id) => {
                    let _ = cx.editor.close_document(doc_id, false);
                    self.dirty_region.mark_all_dirty();
                    request_redraw();
                  },
                  bufferline::BufferKind::Terminal(terminal_id) => {
                    cx.editor.destroy_terminal(terminal_id);
                    self.dirty_region.mark_all_dirty();
                    request_redraw();
                  },
                }
              }
            }
            return EventResult::Consumed(None);
          },

          _ => {
            return EventResult::Consumed(None);
          },
        }
      } else {
        // Mouse left bufferline area - clear all hover states
        let mut changed = false;
        if self.buffer_hover_index.take().is_some() {
          changed = true;
        }
        if self.buffer_pressed_index.take().is_some() {
          changed = true;
        }
        if self.buffer_close_hover_index.take().is_some() {
          changed = true;
        }
        if self.buffer_close_pressed_index.take().is_some() {
          changed = true;
        }
        if self.add_button_hovered {
          self.add_button_hovered = false;
          changed = true;
        }
        if self.add_button_pressed {
          self.add_button_pressed = false;
          changed = true;
        }
        if changed {
          self.dirty_region.mark_all_dirty();
          request_redraw();
        }
      }
    }

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

          // First check which view was clicked
          if let Some(node_id) = self.screen_coords_to_node(mouse.position, cx) {
            // Switch focus to the clicked node if different
            if cx.editor.tree.focus != node_id {
              cx.editor.focus(node_id);
            }

            // Check if this is a terminal view
            let view = cx.editor.tree.get(node_id);
            if let Some(terminal_id) = view.terminal() {
              let view_area = view.area;

              // Calculate terminal cell coordinates
              let rel_x = mouse.position.0
                - self.content_x_offset()
                - view_area.x as f32 * self.cached_cell_width;
              let rel_y = mouse.position.1
                - view_area.y as f32 * self.cached_cell_height
                - self.bufferline_y_adjustment()
                - VIEW_PADDING_TOP;
              let col = (rel_x / self.cached_cell_width).floor().max(0.0) as u16;
              let row = (rel_y / self.cached_cell_height).floor().max(0.0) as i32;

              if let Some(term) = cx.editor.terminal(terminal_id) {
                if term.mouse_mode() || term.alt_screen_mode() {
                  // Terminal or TUI app wants mouse events - send to PTY
                  // SGR mouse format: ESC [ < Cb ; Cx ; Cy M (press) / m (release)
                  let cb = 0; // Left button = 0
                  let report = format!("\x1b[<{};{};{}M", cb, col + 1, row + 1);
                  term.write(report.as_bytes());
                  // Track so we can send release
                  self.terminal_selection_active = Some(terminal_id);
                } else {
                  // Terminal not in mouse mode - start selection
                  use the_terminal::SelectionType;
                  let selection_type = match self.click_count {
                    2 => SelectionType::Semantic, // Word selection
                    3 => SelectionType::Lines,    // Line selection
                    _ => SelectionType::Simple,   // Character selection
                  };
                  term.start_selection(col, row, selection_type);
                  self.terminal_selection_active = Some(terminal_id);
                }
                request_redraw();
              }
              return EventResult::Consumed(None);
            }

            if let Some((view_id, doc_pos)) = self.screen_coords_to_doc_pos(mouse.position, cx) {
              let scrolloff = cx.editor.config().scrolloff;

              // Mark drag as started (for potential drag after click)
              self.mouse_pressed = true;
              self.mouse_drag_anchor_range = None;

              let view = cx.editor.tree.get(view_id);
              let doc_id = view.doc_id();
              let doc = cx.editor.documents.get_mut(&doc_id).unwrap();

              let drag_mode = match self.click_count {
                2 => DragSelectMode::Word,
                3 => DragSelectMode::Line,
                _ => DragSelectMode::Character,
              };

              // Create selection based on click count
              let selection = match drag_mode {
                DragSelectMode::Character => crate::core::selection::Selection::point(doc_pos),
                DragSelectMode::Word => {
                  let text = doc.text();
                  let range = crate::core::selection::Range::point(doc_pos);
                  let word_range = crate::core::textobject::textobject_word(
                    text.slice(..),
                    range,
                    crate::core::textobject::TextObject::Inside,
                    1,
                    false,
                  );
                  crate::core::selection::Selection::single(word_range.anchor, word_range.head)
                },
                DragSelectMode::Line => {
                  let text = doc.text();
                  let line = text.char_to_line(doc_pos.min(text.len_chars()));
                  let start = text.line_to_char(line);
                  let end = text.line_to_char((line + 1).min(text.len_lines()));
                  crate::core::selection::Selection::single(start, end)
                },
              };

              let initial_range = selection.primary();

              doc.set_selection(view_id, selection);
              self.mouse_drag_anchor_range = Some(initial_range);
              self.mouse_drag_mode = drag_mode;

              // Ensure cursor remains visible
              let view = cx.editor.tree.get_mut(view_id);
              view.ensure_cursor_in_view(doc, scrolloff);
            } else {
              self.mouse_drag_anchor_range = None;
              self.mouse_drag_mode = DragSelectMode::Character;
            }

            return EventResult::Consumed(None);
          }
        } else {
          // Mouse button released - end drag
          self.mouse_pressed = false;
          self.mouse_drag_anchor_range = None;
          self.mouse_drag_mode = DragSelectMode::Character;
          self.dragging_separator = None; // End separator drag

          // Handle terminal mouse release
          if let Some(terminal_id) = self.terminal_selection_active.take() {
            if let Some(term) = cx.editor.terminal(terminal_id) {
              if term.mouse_mode() || term.alt_screen_mode() {
                // Send mouse release to PTY
                if let Some(node_id) = self.screen_coords_to_node(mouse.position, cx) {
                  let view = cx.editor.tree.get(node_id);
                  if view.terminal() == Some(terminal_id) {
                    let view_area = view.area;
                    let rel_x = mouse.position.0
                      - self.content_x_offset()
                      - view_area.x as f32 * self.cached_cell_width;
                    let rel_y = mouse.position.1
                      - view_area.y as f32 * self.cached_cell_height
                      - self.bufferline_y_adjustment()
                      - VIEW_PADDING_TOP;
                    let col = (rel_x / self.cached_cell_width).floor().max(0.0) as u16;
                    let row = (rel_y / self.cached_cell_height).floor().max(0.0) as u16;
                    let report = format!("\x1b[<0;{};{}m", col + 1, row + 1);
                    term.write(report.as_bytes());
                  }
                }
              } else {
                // Copy selected text to clipboard
                if let Some(text) = term.selection_text() {
                  if !text.is_empty() {
                    let _ = cx.editor.registers.write('+', vec![text]);
                  }
                }
              }
              request_redraw();
            }
          }

          return EventResult::Consumed(None);
        }
      },
      Some(the_editor_renderer::MouseButton::Middle) => {
        // Middle-click - paste from clipboard
        if mouse.pressed {
          // First check which node was clicked
          if let Some(node_id) = self.screen_coords_to_node(mouse.position, cx) {
            // Switch focus to clicked node
            if cx.editor.tree.focus != node_id {
              cx.editor.focus(node_id);
            }

            // Only paste if it's a view
            if let Some((view_id, doc_pos)) = self.screen_coords_to_doc_pos(mouse.position, cx) {
              // Move cursor to click position
              let scrolloff = cx.editor.config().scrolloff;
              let view = cx.editor.tree.get(view_id);
              let doc_id = view.doc_id();
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

        // Check if we're dragging terminal selection
        if let Some(terminal_id) = self.terminal_selection_active {
          if let Some(term) = cx.editor.terminal(terminal_id) {
            // Find the terminal view to get coordinates
            for (view, _is_focused) in cx.editor.tree.views() {
              if view.terminal() == Some(terminal_id) {
                let view_area = view.area;
                let rel_x = mouse.position.0
                  - self.content_x_offset()
                  - view_area.x as f32 * self.cached_cell_width;
                let rel_y = mouse.position.1
                  - view_area.y as f32 * self.cached_cell_height
                  - self.bufferline_y_adjustment()
                  - VIEW_PADDING_TOP;
                let col = (rel_x / self.cached_cell_width).floor().max(0.0) as u16;
                let row_unclamped = (rel_y / self.cached_cell_height).floor() as i32;
                let (_cols, rows) = term.dimensions();

                if term.mouse_mode() || term.alt_screen_mode() {
                  // Send mouse drag to PTY (button 32 = left button + motion flag)
                  let row = row_unclamped.max(0);
                  let report = format!("\x1b[<32;{};{}M", col + 1, row + 1);
                  term.write(report.as_bytes());
                } else {
                  // Auto-scroll when mouse is at edge of viewport during selection
                  if row_unclamped < 0 {
                    // Mouse above viewport - scroll up (show history)
                    term.scroll(-row_unclamped); // Positive delta scrolls up into history
                  } else if row_unclamped >= rows as i32 {
                    // Mouse below viewport - scroll down (towards current)
                    let overflow = row_unclamped - rows as i32 + 1;
                    term.scroll(-overflow); // Negative delta scrolls down
                  }

                  // Update selection with clamped row
                  let row = row_unclamped.clamp(0, rows as i32 - 1);
                  term.update_selection(col, row);
                }
                request_redraw();
                break;
              }
            }
          }
          return EventResult::Consumed(None);
        }

        // Check if we're dragging text selection
        if self.mouse_pressed {
          if let Some(anchor_range) = self.mouse_drag_anchor_range {
            if let Some((view_id, doc_pos)) = self.screen_coords_to_doc_pos(mouse.position, cx) {
              let scrolloff = cx.editor.config().scrolloff;

              let view = cx.editor.tree.get(view_id);
              let doc_id = view.doc_id();
              let doc = cx.editor.documents.get_mut(&doc_id).unwrap();

              let text = doc.text();
              let slice = text.slice(..);

              let selection = match self.mouse_drag_mode {
                DragSelectMode::Character => {
                  crate::core::selection::Selection::single(anchor_range.anchor, doc_pos)
                },
                DragSelectMode::Word => {
                  let base_start = anchor_range.from();
                  let base_end = anchor_range.to();

                  let target_range = crate::core::textobject::textobject_word(
                    slice,
                    crate::core::selection::Range::point(doc_pos),
                    crate::core::textobject::TextObject::Inside,
                    1,
                    false,
                  );

                  let mut start = base_start.min(target_range.from());
                  let mut end = base_end.max(target_range.to());

                  if target_range.is_empty() {
                    if doc_pos < base_start {
                      start = doc_pos;
                      end = base_end;
                    } else if doc_pos > base_end {
                      start = base_start;
                      end = doc_pos;
                    } else {
                      start = base_start;
                      end = base_end;
                    }
                  }

                  let (anchor, head) = if doc_pos < base_start {
                    (end, start)
                  } else if doc_pos > base_end {
                    (start, end)
                  } else {
                    (anchor_range.anchor, anchor_range.head)
                  };

                  crate::core::selection::Selection::single(anchor, head)
                },
                DragSelectMode::Line => {
                  let total_chars = text.len_chars();
                  if total_chars == 0 {
                    crate::core::selection::Selection::single(
                      anchor_range.anchor,
                      anchor_range.head,
                    )
                  } else {
                    let total_lines = text.len_lines();
                    let clamp_line = |pos: usize| -> usize {
                      if pos >= total_chars {
                        total_lines.saturating_sub(1)
                      } else {
                        text.char_to_line(pos)
                      }
                    };

                    let base_start_line = clamp_line(anchor_range.from());
                    let base_end_char = anchor_range.to().saturating_sub(1);
                    let base_end_line = clamp_line(base_end_char);
                    let doc_line = clamp_line(doc_pos);

                    let start_line = base_start_line.min(doc_line);
                    let end_line = base_end_line.max(doc_line);

                    let start_char = text.line_to_char(start_line);
                    let end_char = text.line_to_char((end_line + 1).min(total_lines));

                    let (anchor, head) = if doc_line < base_start_line {
                      (end_char, start_char)
                    } else if doc_line > base_end_line {
                      (start_char, end_char)
                    } else {
                      (anchor_range.anchor, anchor_range.head)
                    };

                    crate::core::selection::Selection::single(anchor, head)
                  }
                },
              };

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

  /// Detect which view was clicked
  /// Returns ViewId if click was within a view
  fn screen_coords_to_node(
    &self,
    mouse_pos: (f32, f32),
    cx: &Context,
  ) -> Option<crate::core::ViewId> {
    let (mouse_x, mouse_y) = mouse_pos;
    let (cell_width, cell_height) = self.get_current_cell_metrics(cx);

    // Subtract explorer offset from mouse X to get position relative to editor area
    // (only subtract when explorer is on the left)
    let adjusted_mouse_x = mouse_x - self.content_x_offset();
    if adjusted_mouse_x < 0.0 {
      return None; // Click is in explorer area (left side)
    }

    // Convert pixel coordinates to cell coordinates
    // Account for bufferline y adjustment to align with view.area coordinates
    let mouse_col = (adjusted_mouse_x / cell_width) as u16;
    let adjusted_mouse_y = mouse_y - self.bufferline_y_adjustment();
    let mouse_row = (adjusted_mouse_y / cell_height) as u16;

    // Check views
    for (view, _) in cx.editor.tree.views() {
      if mouse_col >= view.area.x
        && mouse_col < view.area.x + view.area.width
        && mouse_row >= view.area.y
        && mouse_row < view.area.y + view.area.height
      {
        return Some(view.id);
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

    // Subtract explorer offset from mouse X to get position relative to editor area
    // The explorer renders at pixel coordinates, but tree/view areas start at x=0
    // (only subtract when explorer is on the left)
    let adjusted_mouse_x = mouse_x - self.content_x_offset();
    if adjusted_mouse_x < 0.0 {
      return None; // Click is in explorer area (left side)
    }

    // Convert pixel coordinates to cell coordinates
    // Account for bufferline y adjustment to align with view.area coordinates
    let mouse_col = (adjusted_mouse_x / cell_width) as u16;
    let adjusted_mouse_y = mouse_y - self.bufferline_y_adjustment();
    let mouse_row = (adjusted_mouse_y / cell_height) as u16;

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

      // Skip terminal views - they don't have document positions
      let Some(doc_id) = view.doc() else {
        return None;
      };

      // Found the view! Now convert to document position
      let doc = &cx.editor.documents[&doc_id];
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
        let gap_center_x =
          self.content_x_offset() + (area.x + area.width) as f32 * font_width + (font_width / 2.0);
        let sep_y = area.y as f32 * cell_height + self.bufferline_y_adjustment();
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
        let sep_x = self.content_x_offset() + area.x as f32 * font_width;
        let sep_y = (area.y + area.height) as f32 * cell_height + self.bufferline_y_adjustment()
          - SEPARATOR_HEIGHT_PX;
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
