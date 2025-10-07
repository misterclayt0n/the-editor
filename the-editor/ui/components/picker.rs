use std::{
  collections::HashMap,
  path::PathBuf,
  sync::{
    Arc,
    atomic::{
      AtomicUsize,
      Ordering,
    },
  },
};

use nucleo::{
  Config,
  Nucleo,
};
use the_editor_renderer::{
  Color,
  Key,
  TextSection,
  TextSegment,
  TextStyle,
};

use super::button::Button;
use crate::core::document::Document;

/// Minimum area width to show preview panel (needs enough room for both panels)
const MIN_AREA_WIDTH_FOR_PREVIEW: u16 = 120;

/// Maximum file size to preview (10MB)
const MAX_FILE_SIZE_FOR_PREVIEW: u64 = 10 * 1024 * 1024;

/// Cached preview types
enum CachedPreview {
  /// Loaded document with syntax highlighting
  Document(Box<Document>),
  /// Binary file (not text)
  Binary,
  /// File too large to preview
  LargeFile,
  /// File not found
  NotFound,
}

/// Preview data prepared for rendering (to avoid borrow issues)
enum PreviewData {
  Document {
    lines: Vec<String>,
    /// Syntax highlights: (highlight, byte_range)
    highlights: Vec<(crate::core::syntax::Highlight, std::ops::Range<usize>)>,
  },
  Placeholder(&'static str),
}

use crate::{
  core::{
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
    },
  },
};

/// Format function for a picker column
pub type ColumnFormatFn<T, D> = for<'a> fn(&'a T, &'a D) -> String;

/// A column in the picker table
pub struct Column<T, D> {
  pub name:   Arc<str>,
  pub format: ColumnFormatFn<T, D>,
  /// Whether this column should be used for nucleo matching/filtering
  pub filter: bool,
  /// Whether this column is hidden (data-only, not displayed)
  pub hidden: bool,
}

impl<T, D> Column<T, D> {
  /// Create a new column with the given name and format function
  pub fn new(name: impl Into<Arc<str>>, format: ColumnFormatFn<T, D>) -> Self {
    Self {
      name:   name.into(),
      format,
      filter: true,
      hidden: false,
    }
  }

  /// Create a hidden column (not displayed, data-only)
  pub fn hidden(name: impl Into<Arc<str>>) -> Self {
    Self {
      name:   name.into(),
      format: |_, _| unreachable!("hidden column should never be formatted"),
      filter: false,
      hidden: true,
    }
  }

  /// Disable filtering for this column (won't be passed to nucleo)
  pub fn without_filtering(mut self) -> Self {
    self.filter = false;
    self
  }
}

/// Helper function to inject an item into nucleo with all columns
fn inject_nucleo_item<T, D>(
  injector: &nucleo::Injector<T>,
  columns: &[Column<T, D>],
  item: T,
  editor_data: &D,
) {
  injector.push(item, |item, dst| {
    for (column, text) in columns.iter().filter(|col| col.filter).zip(dst) {
      *text = (column.format)(item, editor_data).into();
    }
  });
}

/// Injector for adding items to the picker asynchronously
#[derive(Clone)]
pub struct Injector<T, D> {
  dst:            nucleo::Injector<T>,
  columns:        Arc<[Column<T, D>]>,
  editor_data:    Arc<D>,
  version:        usize,
  picker_version: Arc<AtomicUsize>,
}

impl<T, D> Injector<T, D> {
  pub fn push(&self, item: T) -> Result<(), ()> {
    // Check if picker has been closed/reset
    if self.version != self.picker_version.load(Ordering::Relaxed) {
      return Err(());
    }
    inject_nucleo_item(&self.dst, &self.columns, item, &self.editor_data);
    Ok(())
  }
}

/// Generic picker component for fuzzy finding
pub struct Picker<T: 'static + Send + Sync, D: 'static> {
  /// Nucleo matcher for fuzzy finding
  matcher:                  Nucleo<T>,
  /// Columns for the picker table
  columns:                  Arc<[Column<T, D>]>,
  /// Primary column index (default for filtering)
  primary_column:           usize,
  /// Editor data passed to column formatters
  editor_data:              Arc<D>,
  /// Current cursor position in results
  cursor:                   u32,
  /// Search query
  query:                    String,
  /// Cursor position in query
  query_cursor:             usize,
  /// Version counter for invalidating background tasks
  version:                  Arc<AtomicUsize>,
  /// Callback when item is selected
  on_select:                Box<dyn Fn(&T) + Send>,
  /// Callback when picker is closed
  on_close:                 Option<Box<dyn FnOnce() + Send>>,
  /// Whether picker is visible
  visible:                  bool,
  /// Number of visible rows
  completion_height:        u16,
  /// Animation lerp factor (0.0 = just opened, 1.0 = fully visible)
  anim_lerp:                f32,
  /// Preview panel animation lerp (0.0 = hidden, 1.0 = fully visible)
  preview_anim:             f32,
  /// Whether we've done initial preview setup (to skip animation on first
  /// render)
  preview_initialized:      bool,
  /// Hovered item index (for hover effects)
  hovered_item:             Option<u32>,
  /// Mouse position for hover effects
  hover_pos:                Option<(f32, f32)>,
  /// Cached layout info for mouse hit testing
  cached_layout:            Option<PickerLayout>,
  /// Previous cursor position for smooth animation
  prev_cursor:              u32,
  /// Selection animation lerp (0.0 = at prev_cursor, 1.0 = at cursor)
  selection_anim:           f32,
  /// Input cursor animation state
  query_cursor_pos_smooth:  Option<f32>,
  query_cursor_anim_active: bool,
  /// Scroll offset for independent scrolling (VSCode-style)
  scroll_offset:            u32,
  /// Whether nucleo is still processing matches
  matcher_running:          bool,
  /// Animated height for smooth size transitions
  height_smooth:            Option<f32>,
  height_anim_active:       bool,
  /// Preview callback to get file path from item
  preview_fn:               Option<Arc<dyn Fn(&T) -> Option<PathBuf> + Send + Sync>>,
  /// Cache of loaded previews
  preview_cache:            HashMap<PathBuf, CachedPreview>,
  /// Reusable buffer for binary detection
  read_buffer:              Vec<u8>,
}

#[derive(Clone)]
struct PickerLayout {
  x:             f32,
  y:             f32,
  picker_width:  f32,
  height:        f32,
  results_y:     f32,
  item_height:   f32,
  item_gap:      f32,
  offset:        u32,
  visible_count: u32,
}

impl<T: 'static + Send + Sync, D: 'static> Picker<T, D> {
  /// Create a new picker with columns
  pub fn new<C, O>(
    columns: C,
    primary_column: usize,
    options: O,
    editor_data: D,
    on_select: impl Fn(&T) + Send + 'static,
  ) -> Self
  where
    C: IntoIterator<Item = Column<T, D>>,
    O: IntoIterator<Item = T>,
  {
    let columns: Arc<[_]> = columns.into_iter().collect();
    let matcher_columns = columns.iter().filter(|col| col.filter).count() as u32;
    assert!(
      matcher_columns > 0,
      "Picker must have at least one filterable column"
    );

    let matcher = Nucleo::new(
      Config::DEFAULT,
      Arc::new(|| {}), // No-op redraw callback
      None,
      matcher_columns,
    );

    let editor_data = Arc::new(editor_data);
    let injector = matcher.injector();

    // Inject initial items
    for item in options {
      inject_nucleo_item(&injector, &columns, item, &editor_data);
    }

    Self {
      matcher,
      columns,
      primary_column,
      editor_data,
      cursor: 0,
      query: String::new(),
      query_cursor: 0,
      version: Arc::new(AtomicUsize::new(0)),
      on_select: Box::new(on_select),
      on_close: None,
      visible: true,
      completion_height: 0,
      anim_lerp: 0.0,
      preview_anim: 0.0,
      preview_initialized: false,
      hovered_item: None,
      hover_pos: None,
      cached_layout: None,
      prev_cursor: 0,
      selection_anim: 1.0,
      query_cursor_pos_smooth: None,
      query_cursor_anim_active: false,
      scroll_offset: 0,
      matcher_running: false,
      height_smooth: None,
      height_anim_active: false,
      preview_fn: None,
      preview_cache: HashMap::new(),
      read_buffer: Vec::new(),
    }
  }

  /// Get an injector for adding items asynchronously
  pub fn injector(&self) -> Injector<T, D> {
    Injector {
      dst:            self.matcher.injector(),
      columns:        self.columns.clone(),
      editor_data:    self.editor_data.clone(),
      version:        self.version.load(Ordering::Relaxed),
      picker_version: self.version.clone(),
    }
  }

  /// Set the close callback
  pub fn on_close<F>(mut self, callback: F) -> Self
  where
    F: FnOnce() + Send + 'static,
  {
    self.on_close = Some(Box::new(callback));
    self
  }

  /// Set the preview callback to enable file preview
  pub fn with_preview<F>(mut self, preview_fn: F) -> Self
  where
    F: Fn(&T) -> Option<PathBuf> + Send + Sync + 'static,
  {
    self.preview_fn = Some(Arc::new(preview_fn));
    self
  }

  /// Get the currently selected item
  pub fn selection(&self) -> Option<&T> {
    let snapshot = self.matcher.snapshot();
    snapshot.get_matched_item(self.cursor).map(|item| item.data)
  }

  /// Get preview for the currently selected item
  fn get_preview(&mut self, ctx: &Context) -> Option<&CachedPreview> {
    let preview_fn = self.preview_fn.as_ref()?;
    let selected = self.selection()?;
    let path = (preview_fn)(selected)?;

    // Check if already open in editor
    if ctx.editor.document_by_path(&path).is_some() {
      // For now, we'll still load it into cache rather than reference the editor doc
      // This is simpler and avoids lifetime issues
    }

    // Check cache
    if self.preview_cache.contains_key(&path) {
      return Some(&self.preview_cache[&path]);
    }

    // Load file
    let preview = std::fs::metadata(&path)
      .and_then(|metadata| {
        if metadata.is_file() {
          if metadata.len() > MAX_FILE_SIZE_FOR_PREVIEW {
            return Ok(CachedPreview::LargeFile);
          }

          // Check if binary by reading first 1KB
          let file = std::fs::File::open(&path)?;
          use std::io::Read;
          let n = file.take(1024).read_to_end(&mut self.read_buffer)?;

          // Simple binary detection: check for null bytes
          let is_binary = self.read_buffer[..n].contains(&0);
          self.read_buffer.clear();

          if is_binary {
            return Ok(CachedPreview::Binary);
          }

          // Load document
          let doc = Document::open(
            &path,
            None,
            true, // detect language
            ctx.editor.config.clone(),
            ctx.editor.syn_loader.clone(),
          ).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;

          Ok(CachedPreview::Document(Box::new(doc)))
        } else {
          Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Not a regular file",
          ))
        }
      })
      .unwrap_or(CachedPreview::NotFound);

    self.preview_cache.insert(path.clone(), preview);
    Some(&self.preview_cache[&path])
  }

  fn mix_rgb(base: Color, accent: Color, t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    Color::new(
      base.r * (1.0 - t) + accent.r * t,
      base.g * (1.0 - t) + accent.g * t,
      base.b * (1.0 - t) + accent.b * t,
      base.a,
    )
  }

  fn adjust_lightness(color: Color, amount: f32) -> Color {
    let t = amount.abs().clamp(0.0, 1.0);
    let target = if amount >= 0.0 {
      Color::WHITE
    } else {
      Color::BLACK
    };
    let mut mixed = Self::mix_rgb(color, target, t);
    mixed.a = color.a;
    mixed
  }

  fn luminance(color: Color) -> f32 {
    0.2126 * color.r + 0.7152 * color.g + 0.0722 * color.b
  }

  fn glow_rgb_from_base(base: Color) -> Color {
    let lum = Self::luminance(base);
    let t = if lum < 0.35 {
      0.75
    } else if lum < 0.65 {
      0.55
    } else {
      0.35
    };
    let mut glow = Self::mix_rgb(base, Color::WHITE, t);
    glow.a = 1.0;
    glow
  }

  /// Move cursor up
  fn move_up(&mut self) {
    let len = self.matcher.snapshot().matched_item_count();
    if len == 0 {
      return;
    }
    self.prev_cursor = self.cursor;
    self.cursor = if self.cursor == 0 {
      len.saturating_sub(1)
    } else {
      self.cursor.saturating_sub(1)
    };
    self.selection_anim = 0.0; // Start animation

    // Auto-scroll to keep cursor in view
    self.ensure_cursor_in_view();
  }

  /// Move cursor down
  fn move_down(&mut self) {
    let len = self.matcher.snapshot().matched_item_count();
    if len == 0 {
      return;
    }
    self.prev_cursor = self.cursor;
    self.cursor = (self.cursor + 1) % len;
    self.selection_anim = 0.0; // Start animation

    // Auto-scroll to keep cursor in view
    self.ensure_cursor_in_view();
  }

  /// Ensure cursor is visible by adjusting scroll_offset if needed
  fn ensure_cursor_in_view(&mut self) {
    let visible_count = self.completion_height as u32;

    // Skip if no items are visible (e.g., during tests or before first render)
    if visible_count == 0 {
      return;
    }

    // If cursor is above visible area, scroll up
    if self.cursor < self.scroll_offset {
      self.scroll_offset = self.cursor;
    }
    // If cursor is below visible area, scroll down
    else if self.cursor >= self.scroll_offset + visible_count {
      self.scroll_offset = self.cursor.saturating_sub(visible_count - 1);
    }
  }

  /// Page up
  fn page_up(&mut self) {
    let len = self.matcher.snapshot().matched_item_count();
    if len == 0 {
      return;
    }
    let page_size = self.completion_height.max(1) as u32;
    self.prev_cursor = self.cursor;
    self.cursor = self
      .cursor
      .saturating_sub(page_size)
      .min(len.saturating_sub(1));
    self.selection_anim = 0.0; // Start animation

    // Auto-scroll to keep cursor in view
    self.ensure_cursor_in_view();
  }

  /// Page down
  fn page_down(&mut self) {
    let len = self.matcher.snapshot().matched_item_count();
    if len == 0 {
      return;
    }
    let page_size = self.completion_height.max(1) as u32;
    self.prev_cursor = self.cursor;
    self.cursor = (self.cursor + page_size).min(len.saturating_sub(1));
    self.selection_anim = 0.0; // Start animation

    // Auto-scroll to keep cursor in view
    self.ensure_cursor_in_view();
  }

  fn to_start(&mut self) {
    let len = self.matcher.snapshot().matched_item_count();
    if len == 0 {
      return;
    }
    self.prev_cursor = self.cursor;
    self.cursor = 0;
    self.selection_anim = 0.0; // Start animation
    self.ensure_cursor_in_view();
  }

  fn to_end(&mut self) {
    let len = self.matcher.snapshot().matched_item_count();
    if len == 0 {
      return;
    }
    self.prev_cursor = self.cursor;
    self.cursor = len.saturating_sub(1);
    self.selection_anim = 0.0; // Start animation
    self.ensure_cursor_in_view();
  }

  /// Update the search query
  fn update_query(&mut self) {
    use nucleo::pattern::{
      CaseMatching,
      Normalization,
    };

    self.matcher.pattern.reparse(
      0,
      &self.query,
      CaseMatching::Smart,
      Normalization::Smart,
      false,
    );
  }

  /// Handle text input
  fn insert_char(&mut self, c: char) {
    self.query.insert(self.query_cursor, c);
    self.query_cursor += c.len_utf8();
    self.update_query();
  }

  /// Delete character before cursor
  fn delete_char_backwards(&mut self) {
    if self.query_cursor > 0 {
      let mut cursor = self.query_cursor;
      while cursor > 0 {
        cursor -= 1;
        if self.query.is_char_boundary(cursor) {
          break;
        }
      }
      self.query.remove(cursor);
      self.query_cursor = cursor;
      self.update_query();
    }
  }

  fn delete_char_forward(&mut self) {
    if self.query_cursor < self.query.len() {
      self.query.remove(self.query_cursor);
      self.update_query();
    }
  }

  fn move_cursor_left(&mut self) {
    if self.query_cursor > 0 {
      let mut cursor = self.query_cursor;
      while cursor > 0 {
        cursor -= 1;
        if self.query.is_char_boundary(cursor) {
          break;
        }
      }
      self.query_cursor = cursor;
    }
  }

  fn move_cursor_right(&mut self) {
    if self.query_cursor < self.query.len() {
      let mut cursor = self.query_cursor + 1;
      while cursor < self.query.len() && !self.query.is_char_boundary(cursor) {
        cursor += 1;
      }
      self.query_cursor = cursor;
    }
  }

  fn is_word_boundary(c: char) -> bool {
    c.is_whitespace() || c == '/' || c == '-' || c == '_'
  }

  fn move_word_backward(&mut self) {
    if self.query_cursor == 0 {
      return;
    }

    let chars: Vec<char> = self.query.chars().collect();

    // Convert byte position to char position
    let char_pos = self.query[..self.query_cursor].chars().count();
    if char_pos == 0 {
      return;
    }

    let mut char_idx = char_pos.saturating_sub(1);

    // Skip whitespace
    while char_idx > 0 && Self::is_word_boundary(chars[char_idx]) {
      char_idx -= 1;
    }

    // Move to start of word
    while char_idx > 0 && !Self::is_word_boundary(chars[char_idx - 1]) {
      char_idx -= 1;
    }

    // Convert char position back to byte position
    self.query_cursor = self.query.chars().take(char_idx).map(|c| c.len_utf8()).sum();
  }

  fn move_word_forward(&mut self) {
    let chars: Vec<char> = self.query.chars().collect();
    if chars.is_empty() {
      return;
    }

    // Convert byte position to char position
    let char_pos = self.query[..self.query_cursor].chars().count();
    if char_pos >= chars.len() {
      return;
    }

    let mut char_idx = char_pos;

    // Skip current word
    while char_idx < chars.len() && !Self::is_word_boundary(chars[char_idx]) {
      char_idx += 1;
    }

    // Skip whitespace
    while char_idx < chars.len() && Self::is_word_boundary(chars[char_idx]) {
      char_idx += 1;
    }

    // Convert char position back to byte position
    self.query_cursor = self.query.chars().take(char_idx).map(|c| c.len_utf8()).sum();
  }

  fn delete_word_backward(&mut self) {
    if self.query_cursor == 0 {
      return;
    }

    let old_cursor = self.query_cursor;
    self.move_word_backward();
    self.query.replace_range(self.query_cursor..old_cursor, "");
    self.update_query();
  }

  fn delete_word_forward(&mut self) {
    if self.query_cursor >= self.query.len() {
      return;
    }

    let old_cursor = self.query_cursor;
    self.move_word_forward();
    self.query.replace_range(old_cursor..self.query_cursor, "");
    self.query_cursor = old_cursor;
    self.update_query();
  }

  fn kill_to_end(&mut self) {
    self.query.truncate(self.query_cursor);
    self.update_query();
  }

  fn kill_to_start(&mut self) {
    self.query.replace_range(..self.query_cursor, "");
    self.query_cursor = 0;
    self.update_query();
  }

  /// Close the picker
  fn close(&mut self) {
    self.visible = false;
    self.version.fetch_add(1, Ordering::Relaxed);
    if let Some(callback) = self.on_close.take() {
      callback();
    }
  }

  /// Select current item
  fn select(&mut self) {
    if let Some(item) = self.selection() {
      (self.on_select)(item);
    }
    self.close();
  }
}

impl<T: 'static + Send + Sync, D: 'static> Component for Picker<T, D> {
  fn handle_event(&mut self, event: &Event, _ctx: &mut Context) -> EventResult {
    if !self.visible {
      return EventResult::Ignored(None);
    }

    // Handle scroll events (VSCode-style: scroll view without changing selection)
    if let Event::Scroll(delta) = event {
      use the_editor_renderer::ScrollDelta;

      let scroll_lines = match delta {
        ScrollDelta::Lines { y, .. } => *y,
        ScrollDelta::Pixels { y, .. } => {
          // Approximate: 20 pixels per line
          *y / 20.0
        },
      };

      let snapshot = self.matcher.snapshot();
      let len = snapshot.matched_item_count();

      // Negative scroll = scroll down
      // Positive scroll = scroll up
      if scroll_lines < 0.0 {
        // Scroll down
        let amount = (scroll_lines.abs() as u32).min(3);
        self.scroll_offset = self
          .scroll_offset
          .saturating_add(amount)
          .min(len.saturating_sub(1));
      } else if scroll_lines > 0.0 {
        // Scroll up
        let amount = (scroll_lines as u32).min(3);
        self.scroll_offset = self.scroll_offset.saturating_sub(amount);
      }

      return EventResult::Consumed(None);
    }

    // Handle mouse events for hover and click
    if let Event::Mouse(mouse) = event
      && let Some(layout) = &self.cached_layout
    {
      let (mx, my) = mouse.position;

      // Check if mouse is within picker area
      let in_picker = mx >= layout.x
        && mx <= layout.x + layout.picker_width
        && my >= layout.y
        && my <= layout.y + layout.height;

      if in_picker {
        // Store hover position for glow effects
        self.hover_pos = Some((mx, my));

        // Check if mouse is over a file item
        if my >= layout.results_y {
          let relative_y = my - layout.results_y;
          let item_idx_in_view =
            (relative_y / (layout.item_height + layout.item_gap)).floor() as u32;

          if item_idx_in_view < layout.visible_count {
            let global_item_idx = layout.offset + item_idx_in_view;

            // Update hovered item
            self.hovered_item = Some(global_item_idx);

            // Handle click
            if let Some(button) = mouse.button {
              use the_editor_renderer::MouseButton;
              if button == MouseButton::Left && !mouse.pressed {
                // Click on item - select and close (with animation)
                if self.cursor != global_item_idx {
                  self.prev_cursor = self.cursor;
                  self.cursor = global_item_idx;
                  self.selection_anim = 0.0; // Start animation
                }
                self.select();
                let callback = Box::new(
                  |compositor: &mut crate::ui::compositor::Compositor, _ctx: &mut Context| {
                    compositor.pop();
                  },
                );
                return EventResult::Consumed(Some(callback));
              }
            }
          } else {
            self.hovered_item = None;
          }
        } else {
          self.hovered_item = None;
        }

        return EventResult::Consumed(None);
      } else {
        self.hovered_item = None;
        self.hover_pos = None;
      }
    }

    let Event::Key(key) = event else {
      return EventResult::Ignored(None);
    };

    // Emacs-style keybindings (like Helix)
    match (key.code, key.ctrl, key.alt, key.shift) {
      // Escape / Ctrl+c - close
      (Key::Escape, _, _, _) | (Key::Char('c'), true, _, _) => {
        self.close();
        let callback = Box::new(
          |compositor: &mut crate::ui::compositor::Compositor, _ctx: &mut Context| {
            compositor.pop();
          },
        );
        EventResult::Consumed(Some(callback))
      },
      // Enter - select
      (Key::Enter, _, _, _) => {
        self.select();
        let callback = Box::new(
          |compositor: &mut crate::ui::compositor::Compositor, _ctx: &mut Context| {
            compositor.pop();
          },
        );
        EventResult::Consumed(Some(callback))
      },

      // Query text editing (Emacs-style)
      // Ctrl+b / Left - backward char
      (Key::Char('b'), true, _, _) | (Key::Left, false, false, false) => {
        self.move_cursor_left();
        EventResult::Consumed(None)
      },
      // Ctrl+f / Right - forward char
      (Key::Char('f'), true, _, _) | (Key::Right, false, false, false) => {
        self.move_cursor_right();
        EventResult::Consumed(None)
      },
      // Alt+b / Ctrl+Left - backward word
      (Key::Char('b'), _, true, _) | (Key::Left, true, false, _) => {
        self.move_word_backward();
        EventResult::Consumed(None)
      },
      // Alt+f / Ctrl+Right - forward word
      (Key::Char('f'), _, true, _) | (Key::Right, true, false, _) => {
        self.move_word_forward();
        EventResult::Consumed(None)
      },
      // Ctrl+a - start of query line
      (Key::Char('a'), true, _, _) => {
        self.query_cursor = 0;
        EventResult::Consumed(None)
      },
      // Ctrl+e - end of query line
      (Key::Char('e'), true, _, _) => {
        self.query_cursor = self.query.len();
        EventResult::Consumed(None)
      },
      // Ctrl+h / Backspace - delete char backwards
      (Key::Char('h'), true, _, _) | (Key::Backspace, false, false, _) => {
        self.delete_char_backwards();
        EventResult::Consumed(None)
      },
      // Delete - delete char forward
      (Key::Delete, false, false, false) => {
        self.delete_char_forward();
        EventResult::Consumed(None)
      },
      // Ctrl+w / Alt+Backspace / Ctrl+Backspace - delete word backward
      (Key::Char('w'), true, _, _) | (Key::Backspace, _, true, _) | (Key::Backspace, true, _, _) => {
        self.delete_word_backward();
        EventResult::Consumed(None)
      },
      // Alt+d / Alt+Delete / Ctrl+Delete - delete word forward
      (Key::Char('d'), _, true, _) | (Key::Delete, _, true, _) | (Key::Delete, true, _, _) => {
        self.delete_word_forward();
        EventResult::Consumed(None)
      },
      // Ctrl+k - kill to end of query
      (Key::Char('k'), true, _, _) => {
        self.kill_to_end();
        EventResult::Consumed(None)
      },

      // List navigation
      // Ctrl+p / Up / Shift+Tab - move up
      (Key::Char('p'), true, _, _) | (Key::Up, false, false, false) | (Key::Tab, _, _, true) => {
        self.move_up();
        EventResult::Consumed(None)
      },
      // Ctrl+n / Down / Tab - move down
      (Key::Char('n'), true, _, _) | (Key::Down, false, false, false) | (Key::Tab, _, _, false) => {
        self.move_down();
        EventResult::Consumed(None)
      },
      // Ctrl+u / PageUp - page up
      (Key::Char('u'), true, _, _) | (Key::PageUp, _, _, _) => {
        self.page_up();
        EventResult::Consumed(None)
      },
      // Ctrl+d / PageDown - page down (note: conflicts with delete forward, but Ctrl+d for page down is more common in pickers)
      // We handle delete forward with just Delete key above
      (Key::Char('d'), true, _, _) | (Key::PageDown, _, _, _) => {
        self.page_down();
        EventResult::Consumed(None)
      },
      // Home - to start
      (Key::Home, _, _, _) => {
        self.to_start();
        EventResult::Consumed(None)
      },
      // End - to end
      (Key::End, _, _, _) => {
        self.to_end();
        EventResult::Consumed(None)
      },

      // Regular character input
      (Key::Char(c), false, false, _) => {
        self.insert_char(c);
        EventResult::Consumed(None)
      },
      _ => EventResult::Ignored(None),
    }
  }

  fn render(
    &mut self,
    area: Rect,
    surface: &mut crate::ui::compositor::Surface,
    ctx: &mut Context,
  ) {
    if !self.visible {
      return;
    }

    // Animate lerp factor for smooth entrance
    let anim_speed = 12.0; // Speed of animation
    if self.anim_lerp < 1.0 {
      self.anim_lerp = (self.anim_lerp + ctx.dt * anim_speed).min(1.0);
    }

    // Animate selection changes
    let selection_anim_speed = 15.0;
    if self.selection_anim < 1.0 {
      self.selection_anim = (self.selection_anim + ctx.dt * selection_anim_speed).min(1.0);
    }

    // Determine if we should show preview panel based on actual available width
    // Need enough room for both panels + gap (minimum ~1200px for comfortable
    // split)
    let min_width_for_preview = 1200.0;
    let should_show_preview = area.width as f32 > min_width_for_preview;

    // Initialize preview state on first render (no animation)
    if !self.preview_initialized {
      self.preview_anim = if should_show_preview { 1.0 } else { 0.0 };
      self.preview_initialized = true;
    } else {
      // Animate preview panel appearance/disappearance on resize
      let preview_anim_speed = 8.0;
      if should_show_preview {
        // Fade in
        self.preview_anim = (self.preview_anim + ctx.dt * preview_anim_speed).min(1.0);
      } else {
        // Fade out
        self.preview_anim = (self.preview_anim - ctx.dt * preview_anim_speed).max(0.0);
      }
    }

    // Process pending updates from nucleo
    let status = self.matcher.tick(10);
    self.matcher_running = status.running || status.changed;

    // Get preview before taking snapshot to avoid borrow checker issues
    // First ensure the preview is loaded into cache
    let _preview = self.get_preview(ctx);

    // Now extract preview data with mutable access for highlights
    let preview_data = {
      let preview_fn = self.preview_fn.as_ref();
      let selected = preview_fn.and_then(|f| {
        let snapshot = self.matcher.snapshot();
        snapshot.get_matched_item(self.cursor).map(|item| (f)(item.data))
      }).flatten();

      selected.and_then(|path| {
        self.preview_cache.get_mut(&path).map(|preview| {
          match preview {
            CachedPreview::Document(doc) => {
              // Extract the lines we need to render
              let text = doc.text();
              let total_lines = text.len_lines();
              let max_preview_lines = 100; // Limit to first 100 lines
              let lines: Vec<String> = (0..total_lines.min(max_preview_lines))
                .map(|i| text.line(i).to_string())
                .collect();

              // Get syntax highlights for the visible range
              let end_line = total_lines.min(max_preview_lines);
              let start_byte = 0;
              let end_byte = if end_line < total_lines {
                text.line_to_byte(end_line)
              } else {
                text.len_bytes()
              };

              let highlights = doc.get_viewport_highlights(start_byte..end_byte, &ctx.editor.syn_loader.load())
                .unwrap_or_default();

              PreviewData::Document { lines, highlights }
            },
            CachedPreview::Binary => PreviewData::Placeholder("<Binary file>"),
            CachedPreview::LargeFile => PreviewData::Placeholder("<File too large to preview>"),
            CachedPreview::NotFound => PreviewData::Placeholder("<File not found>"),
          }
        })
      })
    };

    let snapshot = self.matcher.snapshot();

    // Ensure cursor is in bounds
    let len = snapshot.matched_item_count();
    if len > 0 && self.cursor >= len {
      self.cursor = len.saturating_sub(1);
    }

    // Get theme colors
    let theme = &ctx.editor.theme;
    let bg_style = theme.get("ui.popup");
    let border_style = theme.get("ui.window");
    let text_style = theme.get("ui.text");
    let count_style = theme.get("ui.text.inactive");
    let sep_style = theme.get("ui.background.separator");

    // Convert to renderer colors
    let bg_color = bg_style
      .bg
      .map(crate::ui::theme_color_to_renderer_color)
      .unwrap_or(Color::new(0.1, 0.1, 0.1, 0.95));
    // Use a more vibrant accent color for borders (try ui.selection, fallback to
    // bright blue)
    let accent_style = theme.get("ui.selection");
    let border_color = accent_style
      .bg
      .or(border_style.bg)
      .map(crate::ui::theme_color_to_renderer_color)
      .unwrap_or(Color::new(0.3, 0.6, 0.9, 1.0)); // Bright blue fallback
    let mut border_color_rgb = border_color;
    border_color_rgb.a = 1.0;
    let text_color = text_style
      .fg
      .map(crate::ui::theme_color_to_renderer_color)
      .unwrap_or(Color::new(0.9, 0.9, 0.9, 1.0));

    // Use a specific theme key for picker selected text for better contrast
    let picker_selected_text_style = theme.try_get_exact("ui.picker.selected.text");
    let selected_fg = picker_selected_text_style
      .and_then(|s| s.fg)
      .map(crate::ui::theme_color_to_renderer_color)
      .unwrap_or(Color::new(1.0, 1.0, 1.0, 1.0)); // Bright white for contrast
    let button_style = theme.get("ui.button");
    let mut button_base_color = button_style
      .fg
      .map(crate::ui::theme_color_to_renderer_color)
      .unwrap_or(Color::new(0.58, 0.6, 0.82, 1.0));
    button_base_color.a = 1.0;
    let mut button_fill_color = button_style
      .bg
      .map(crate::ui::theme_color_to_renderer_color)
      .unwrap_or_else(|| {
        let mut base = bg_color;
        base.a = 1.0;
        base
      });
    button_fill_color.a = 1.0;
    let button_highlight_color = theme
      .try_get_exact("ui.button.highlight")
      .and_then(|style| style.fg)
      .map(crate::ui::theme_color_to_renderer_color)
      .unwrap_or_else(|| Self::glow_rgb_from_base(button_base_color));

    let picker_selected_style = theme.get("ui.picker.selected");
    let mut picker_selected_fill = picker_selected_style
      .bg
      .map(crate::ui::theme_color_to_renderer_color)
      .unwrap_or_else(|| Self::mix_rgb(button_fill_color, bg_color, 0.4));
    picker_selected_fill.a = 1.0;
    let mut picker_selected_outline = picker_selected_style
      .fg
      .map(crate::ui::theme_color_to_renderer_color)
      .unwrap_or(button_highlight_color);
    picker_selected_outline.a = 1.0;
    let query_color = text_color;
    let count_color = count_style
      .fg
      .map(crate::ui::theme_color_to_renderer_color)
      .unwrap_or(Color::new(0.6, 0.6, 0.6, 1.0));
    let sep_color = sep_style
      .fg
      .map(crate::ui::theme_color_to_renderer_color)
      .unwrap_or(Color::new(0.3, 0.3, 0.3, 1.0));

    // Calculate dimensions - much taller picker (90% of screen height, 80% width)
    let total_width = (area.width as f32 * 0.8).max(600.0);
    let max_height = (area.height as f32 * 0.9).max(400.0);

    // Lerp picker width based on preview animation
    // When preview is hidden (0.0), picker takes full width
    // When preview is visible (1.0), picker takes half width
    let picker_width_full = total_width;
    let picker_width_split = total_width * 0.5;
    let picker_width =
      picker_width_full + (picker_width_split - picker_width_full) * self.preview_anim;

    let line_height = UI_FONT_SIZE + 4.0;

    // Calculate number of result rows to show - account for item padding and gaps
    // These must match the values used in rendering!
    let item_padding_y = 8.0;
    let item_gap = 3.0;
    let actual_item_height = line_height + item_padding_y * 2.0;

    // Header height must account for prompt box padding
    let prompt_internal_padding = 8.0;
    let header_height = line_height + 8.0 + prompt_internal_padding * 2.0;
    let bottom_padding = 16.0;
    let available_height = max_height - header_height - bottom_padding;

    // Calculate how many items can fit with gaps
    let max_rows = ((available_height + item_gap) / (actual_item_height + item_gap)).floor() as u32;
    // Allow more rows (up to 30 instead of 15)
    self.completion_height = max_rows.min(len).min(30) as u16;

    // Calculate actual height needed for the items
    let rows_height = if self.completion_height > 0 {
      self.completion_height as f32 * actual_item_height
        + (self.completion_height as f32 - 1.0).max(0.0) * item_gap
    } else {
      0.0
    };

    let height = header_height + rows_height + bottom_padding;

    // Smooth height animation for size changes
    let height_lerp_speed = 30.0;
    let animated_height = if let Some(current_smooth) = self.height_smooth {
      let target = height;
      let diff = target - current_smooth;

      if diff.abs() > 0.5 {
        let lerp_factor = 1.0 - (-ctx.dt * height_lerp_speed).exp();
        let new_smooth = current_smooth + diff * lerp_factor;
        self.height_smooth = Some(new_smooth);
        self.height_anim_active = true;
        new_smooth
      } else {
        self.height_smooth = Some(target);
        self.height_anim_active = false;
        target
      }
    } else {
      self.height_smooth = Some(height);
      self.height_anim_active = false;
      height
    };

    // Apply animation lerp with easing
    let t = self.anim_lerp;
    let ease = t * t * (3.0 - 2.0 * t); // Smoothstep easing

    // Center the picker (use total_width for centering calculation)
    let target_x = area.x as f32 + (area.width as f32 - total_width) / 2.0;
    let target_y = area.y as f32 + (area.height as f32 - animated_height) / 2.0;

    // Apply scale animation (start at 95% scale) for visual effect
    let scale = 0.95 + 0.05 * ease;
    let x = target_x + (picker_width * (1.0 - scale)) / 2.0;
    let y = target_y + (animated_height * (1.0 - scale)) / 2.0;
    let picker_width_scaled = picker_width * scale;
    let height_scaled = animated_height * scale;

    // Apply alpha animation to foreground elements
    let alpha = ease;

    // Button-style rounded corners
    let corner_radius = (height_scaled * 0.02).min(8.0);
    let border_thickness = 1.0;

    // Draw opaque picker background first (before enabling overlay text mode)
    let picker_bg = Color::new(bg_color.r, bg_color.g, bg_color.b, alpha);
    surface.draw_rounded_rect(
      x,
      y,
      picker_width_scaled,
      height_scaled,
      corner_radius,
      picker_bg,
    );

    // Calculate mask width for overlay region (covers picker and preview if visible)
    let mask_width = if self.preview_anim > 0.0 {
      // Mask covers both picker and preview panels
      total_width * scale
    } else {
      // Mask covers only picker panel
      picker_width_scaled
    };

    // Render picker content in overlay mode with automatic masking
    surface.with_overlay_region(x, y, mask_width, height_scaled, |surface| {

    // Apply alpha to border color
    let border_color_anim = Color::new(
      border_color.r,
      border_color.g,
      border_color.b,
      border_color.a * alpha * 0.95,
    );

    // Draw rounded border outline (button-style)
    surface.draw_rounded_rect_stroke(
      x,
      y,
      picker_width_scaled,
      height_scaled,
      corner_radius,
      border_thickness,
      border_color_anim,
    );

    // Apply alpha to text colors
    let query_color_anim = Color::new(
      query_color.r,
      query_color.g,
      query_color.b,
      query_color.a * alpha,
    );
    let count_color_anim = Color::new(
      count_color.r,
      count_color.g,
      count_color.b,
      count_color.a * alpha,
    );
    let text_color_anim = Color::new(
      text_color.r,
      text_color.g,
      text_color.b,
      text_color.a * alpha,
    );
    let selected_fg_anim = Color::new(
      selected_fg.r,
      selected_fg.g,
      selected_fg.b,
      selected_fg.a * alpha,
    );
    let sep_color_anim = Color::new(sep_color.r, sep_color.g, sep_color.b, sep_color.a * alpha);

    // Calculate count_width early for layout
    let count_text = format!(
      "{}/{}",
      snapshot.matched_item_count(),
      snapshot.item_count()
    );
    let count_width = count_text.len() as f32 * UI_FONT_WIDTH;

    // Draw input prompt box with colored border and matching background
    // The box should span the full width to include both input and count
    let prompt_box_padding = 8.0; // Increased internal padding
    let prompt_box_x = x + 8.0 - prompt_box_padding;
    let prompt_box_y = y + 8.0 - prompt_box_padding;
    let prompt_box_width = picker_width_scaled - 16.0 + prompt_box_padding * 2.0; // Full width
    let prompt_box_height = line_height + prompt_box_padding * 2.0; // Account for vertical padding
    let prompt_box_radius = 6.0; // Slightly larger radius

    // Draw input box background (same as border color but more transparent)
    let input_box_bg = Color::new(
      border_color.r,
      border_color.g,
      border_color.b,
      0.15 * alpha, // Subtle background tint
    );
    surface.draw_rounded_rect(
      prompt_box_x,
      prompt_box_y,
      prompt_box_width,
      prompt_box_height,
      prompt_box_radius,
      input_box_bg,
    );

    // Draw input box border
    let input_box_border = Color::new(
      border_color.r,
      border_color.g,
      border_color.b,
      border_color.a * alpha * 0.6,
    );
    surface.draw_rounded_rect_stroke(
      prompt_box_x,
      prompt_box_y,
      prompt_box_width,
      prompt_box_height,
      prompt_box_radius,
      1.0,
      input_box_border,
    );

    // Draw search query with prompt and cursor (similar to command prompt)
    let prompt_prefix = "› ";
    let full_text = format!("{}{}", prompt_prefix, self.query);
    let prompt_x = x + 8.0;
    let prompt_y = y + 8.0;
    let prefix_len = prompt_prefix.chars().count();

    // Get cursor colors from theme
    let cursor_style = theme.get("ui.cursor");
    let cursor_bg = cursor_style
      .bg
      .map(crate::ui::theme_color_to_renderer_color)
      .unwrap_or(Color::new(1.0, 1.0, 1.0, 1.0));
    let cursor_fg = cursor_style
      .fg
      .map(crate::ui::theme_color_to_renderer_color)
      .unwrap_or(Color::new(0.0, 0.0, 0.0, 1.0));

    // Calculate visible cursor column
    let visible_cursor_col = prefix_len + self.query_cursor;

    // Cursor animation
    let target_x = prompt_x + visible_cursor_col as f32 * UI_FONT_WIDTH;
    let cursor_lerp_factor = ctx.editor.config().cursor_lerp_factor;
    let cursor_anim_enabled = ctx.editor.config().cursor_anim_enabled;

    let anim_x = if cursor_anim_enabled {
      let mut sx = self.query_cursor_pos_smooth.unwrap_or(target_x);
      let dx = target_x - sx;
      sx += dx * cursor_lerp_factor;
      self.query_cursor_pos_smooth = Some(sx);
      self.query_cursor_anim_active = dx.abs() > 0.5;
      sx
    } else {
      self.query_cursor_anim_active = false;
      self.query_cursor_pos_smooth = Some(target_x);
      target_x
    };

    // Draw cursor background
    const CURSOR_HEIGHT_EXTENSION: f32 = 4.0;
    let cursor_bg_anim = Color::new(cursor_bg.r, cursor_bg.g, cursor_bg.b, cursor_bg.a * alpha);
    surface.draw_rect(
      anim_x,
      prompt_y,
      UI_FONT_WIDTH,
      UI_FONT_SIZE + CURSOR_HEIGHT_EXTENSION,
      cursor_bg_anim,
    );

    // Render text character by character (like prompt does)
    for (i, ch) in full_text.chars().enumerate() {
      let char_x = prompt_x + i as f32 * UI_FONT_WIDTH;
      let color = if i == visible_cursor_col {
        // Use cursor foreground color for character under cursor
        Color::new(cursor_fg.r, cursor_fg.g, cursor_fg.b, cursor_fg.a * alpha)
      } else {
        query_color_anim
      };

      surface.draw_text(TextSection {
        position: (char_x, prompt_y),
        texts:    vec![TextSegment {
          content: ch.to_string(),
          style:   TextStyle {
            color,
            ..Default::default()
          },
        }],
      });
    }

    // Draw match count (count_text and count_width already calculated above)
    surface.draw_text(TextSection {
      position: (x + picker_width_scaled - count_width - 16.0, y + 8.0),
      texts:    vec![TextSegment {
        content: count_text,
        style:   TextStyle {
          color: count_color_anim,
          ..Default::default()
        },
      }],
    });

    // Draw separator
    let sep_y = y + header_height * scale;
    surface.draw_rect(
      x + 4.0,
      sep_y,
      picker_width_scaled - 8.0,
      1.0,
      sep_color_anim,
    );

    // Draw results
    let results_y = sep_y + 8.0;
    // Use scroll_offset for rendering (VSCode-style independent scrolling)
    let offset = self.scroll_offset;
    let end = (offset + self.completion_height as u32).min(len);

    // Increased padding for button-like items (made fatter)
    let item_padding_x = 12.0;
    let item_padding_y = 8.0;
    let item_height = line_height * scale + item_padding_y * 2.0;
    let item_gap = 3.0; // Small gap between items

    // Cache layout info for mouse hit testing
    self.cached_layout = Some(PickerLayout {
      x,
      y,
      picker_width: picker_width_scaled,
      height: height_scaled,
      results_y,
      item_height,
      item_gap,
      offset,
      visible_count: (end - offset).min(self.completion_height as u32),
    });

    for (i, item_idx) in (offset..end).enumerate() {
      if let Some(item) = snapshot.get_matched_item(item_idx) {
        let item_y = results_y + i as f32 * (item_height + item_gap);
        let is_selected = item_idx == self.cursor;
        let is_hovered = self.hovered_item == Some(item_idx);

        // Button-like item with rounded corners
        let item_x = x + 8.0;
        let item_width = picker_width_scaled - 16.0;
        let item_radius = 4.0;

        let selection_gap = (item_height * 0.12).clamp(1.5, item_height * 0.35);
        let selection_height = (item_height - selection_gap).max(item_height * 0.55);
        let selection_y = item_y;

        // Alternating stripe background to hint at row separation
        let stripe_primary = Self::mix_rgb(picker_bg, button_fill_color, 0.28);
        let stripe_secondary = Self::mix_rgb(picker_bg, button_fill_color, 0.18);
        let mut stripe_color = if item_idx % 2 == 0 {
          Self::adjust_lightness(stripe_primary, 0.02)
        } else {
          Self::adjust_lightness(stripe_secondary, -0.015)
        };
        stripe_color.a = (picker_bg.a * 0.92).clamp(0.0, 1.0);
        surface.draw_rounded_rect(
          item_x,
          item_y,
          item_width,
          item_height,
          item_radius,
          stripe_color,
        );

        if is_selected {
          let selection_t = self.selection_anim.clamp(0.0, 1.0);
          let selection_ease = selection_t * selection_t * (3.0 - 2.0 * selection_t);

          let mut fill_color = picker_selected_fill;
          fill_color.a = (fill_color.a * (0.82 + 0.18 * selection_ease)).clamp(0.0, 1.0);
          surface.draw_rounded_rect(
            item_x,
            selection_y,
            item_width,
            selection_height,
            item_radius,
            fill_color,
          );

          let mut outline_color = picker_selected_outline;
          outline_color.a = (outline_color.a * alpha).clamp(0.0, 1.0);

          let bottom_thickness = (selection_height * 0.035).clamp(0.6, 1.2);
          let side_thickness = (bottom_thickness * 1.55).min(bottom_thickness + 1.6);
          let top_thickness = (bottom_thickness * 2.2).min(bottom_thickness + 2.4);
          surface.draw_rounded_rect_stroke_fade(
            item_x,
            selection_y,
            item_width,
            selection_height,
            item_radius,
            top_thickness,
            side_thickness,
            bottom_thickness,
            outline_color,
          );

          let top_center_x = item_x + item_width * 0.5;
          let glow_color = Self::glow_rgb_from_base(picker_selected_outline);
          let glow_strength = (alpha * (0.85 + 0.15 * selection_ease)).clamp(0.0, 1.0);
          Button::draw_hover_layers(
            surface,
            item_x,
            selection_y,
            item_width,
            selection_height,
            item_radius,
            picker_selected_outline,
            glow_strength,
          );

          if self.selection_anim < 1.0 {
            let pulse_ease = 1.0 - (1.0 - selection_t) * (1.0 - selection_t);
            let center_y = selection_y + selection_height * 0.52;
            let pulse_radius =
              item_width.max(selection_height) * (0.42 + 0.35 * (1.0 - pulse_ease));
            let pulse_alpha = (1.0 - pulse_ease) * 0.18 * alpha;
            surface.draw_rounded_rect_glow(
              item_x,
              selection_y,
              item_width,
              selection_height,
              item_radius,
              top_center_x,
              center_y,
              pulse_radius,
              Color::new(glow_color.r, glow_color.g, glow_color.b, pulse_alpha),
            );
          }
        } else if is_hovered {
          let mut hover_bg = Self::mix_rgb(stripe_color, picker_selected_outline, 0.15);
          hover_bg.a = (hover_bg.a + 0.05 * alpha).clamp(0.0, 1.0);
          surface.draw_rounded_rect(
            item_x,
            selection_y,
            item_width,
            selection_height,
            item_radius,
            hover_bg,
          );

          if let Some((hover_x, hover_y)) = self.hover_pos {
            let clamped_x = hover_x.clamp(item_x, item_x + item_width);
            let clamped_y = hover_y.clamp(selection_y, selection_y + selection_height);
            let glow_color = Color::new(
              picker_selected_outline.r,
              picker_selected_outline.g,
              picker_selected_outline.b,
              0.1 * alpha,
            );
            surface.draw_rounded_rect_glow(
              item_x,
              selection_y,
              item_width,
              selection_height,
              item_radius,
              clamped_x,
              clamped_y,
              item_width.max(selection_height) * 0.9,
              glow_color,
            );
          }
        }

        // Format item text from all visible columns
        let mut display_text = String::new();
        for (i, column) in self.columns.iter().filter(|c| !c.hidden).enumerate() {
          if i > 0 {
            display_text.push_str("  ");
          }
          display_text.push_str(&(column.format)(item.data, &self.editor_data));
        }

        // Skip rendering text if empty (should not happen, but safety check)
        if display_text.is_empty() {
          continue;
        }

        let prefix = "  ";

        // Position text with padding
        let text_x = item_x + item_padding_x;
        let text_y = item_y + item_padding_y;

        // Calculate available width for text (excluding padding)
        let available_width = item_width - (item_padding_x * 2.0);
        let prefix_width = prefix.len() as f32 * UI_FONT_WIDTH;
        let text_available_width = available_width - prefix_width;
        let max_chars = (text_available_width / UI_FONT_WIDTH).floor() as usize;

        // Truncate text if it's too long
        let truncated_text = if display_text.chars().count() > max_chars.saturating_sub(3) && max_chars > 3 {
          let truncated: String = display_text.chars().take(max_chars.saturating_sub(3)).collect();
          format!("{}...", truncated)
        } else {
          display_text
        };

        // Draw text in single color
        let item_color = if is_selected {
          selected_fg_anim
        } else {
          text_color_anim
        };

        surface.draw_text(TextSection {
          position: (text_x, text_y),
          texts:    vec![
            TextSegment {
              content: prefix.to_string(),
              style:   TextStyle {
                color: item_color,
                ..Default::default()
              },
            },
            TextSegment {
              content: truncated_text,
              style:   TextStyle {
                color: item_color,
                ..Default::default()
              },
            },
          ],
        });
      }
    }

    // Draw preview panel with animation
    if self.preview_anim > 0.0 {
      let preview_ease = self.preview_anim * self.preview_anim * (3.0 - 2.0 * self.preview_anim); // Smoothstep

      let preview_gap = 12.0; // Padding between picker and preview
      let preview_x = x + picker_width_scaled + preview_gap;
      let preview_width = (total_width - picker_width - preview_gap) * scale;

      // Apply alpha to preview
      let preview_alpha = alpha * preview_ease;
      let bg_color_preview = Color::new(
        bg_color.r,
        bg_color.g,
        bg_color.b,
        bg_color.a * preview_alpha,
      );
      let border_color_preview = Color::new(
        border_color.r,
        border_color.g,
        border_color.b,
        border_color.a * preview_alpha,
      );
      let text_color_preview = Color::new(
        text_color.r,
        text_color.g,
        text_color.b,
        text_color.a * preview_alpha,
      );

      // Draw preview background with rounded corners
      surface.draw_rounded_rect(
        preview_x,
        y,
        preview_width,
        height_scaled,
        corner_radius,
        bg_color_preview,
      );

      // Draw preview border (button-style outline)
      surface.draw_rounded_rect_stroke(
        preview_x,
        y,
        preview_width,
        height_scaled,
        corner_radius,
        border_thickness,
        border_color_preview,
      );

      // Render preview content with clipping
      if let Some(preview) = &preview_data {
        match preview {
          PreviewData::Document { lines, highlights } => {
            // Render document content with clipping region
            let padding = 12.0;
            let line_height = UI_FONT_SIZE + 4.0;
            let content_x = preview_x + padding;
            let content_y = y + padding;
            let content_width = preview_width - (padding * 2.0);
            let content_height = height_scaled - (padding * 2.0);

            // Calculate how many lines we can show
            let max_lines = (content_height / line_height).floor() as usize;
            let lines_to_show = max_lines.min(lines.len());

            // Calculate max characters per line based on available width
            let max_chars = (content_width / UI_FONT_WIDTH).floor() as usize;

            // Use overlay region for clipping
            surface.with_overlay_region(
              preview_x + padding,
              y + padding,
              content_width,
              content_height,
              |surface| {
                let mut current_y = content_y + UI_FONT_SIZE; // Start with font size offset for baseline
                let mut byte_offset = 0;

                for (_line_idx, line_str) in lines.iter().enumerate().take(lines_to_show) {
                  // Trim trailing whitespace
                  let trimmed = line_str.trim_end();
                  let line_byte_len = line_str.len();

                  // Calculate byte range for this line
                  let line_start_byte = byte_offset;
                  let line_end_byte = byte_offset + line_byte_len;

                  // Build text segments with syntax highlighting
                  let mut segments = Vec::new();
                  let mut current_char_idx = 0;
                  let mut current_byte_in_line = 0;

                  // Truncate if line is too long
                  let max_display_chars = max_chars.saturating_sub(3);
                  let should_truncate = trimmed.chars().count() > max_display_chars;

                  for ch in trimmed.chars() {
                    if should_truncate && current_char_idx >= max_display_chars {
                      // Add ellipsis and stop
                      segments.push(TextSegment {
                        content: "...".to_string(),
                        style: TextStyle {
                          size: UI_FONT_SIZE,
                          color: text_color_preview,
                        },
                      });
                      break;
                    }

                    let char_byte_pos = line_start_byte + current_byte_in_line;

                    // Find active highlight for this byte position
                    let mut active_color = text_color_preview;
                    for (highlight, range) in highlights.iter() {
                      if range.contains(&char_byte_pos) {
                        // Apply theme color for this highlight
                        let hl_style = theme.highlight(*highlight);
                        if let Some(fg) = hl_style.fg {
                          active_color = crate::ui::theme_color_to_renderer_color(fg);
                          active_color.a *= preview_alpha;
                        }
                        break;
                      }
                    }

                    // Check if we can merge with previous segment (same color)
                    if let Some(last_seg) = segments.last_mut() {
                      // Compare colors (approximately)
                      let colors_match = (last_seg.style.color.r - active_color.r).abs() < 0.001
                        && (last_seg.style.color.g - active_color.g).abs() < 0.001
                        && (last_seg.style.color.b - active_color.b).abs() < 0.001;

                      if colors_match {
                        // Merge with previous segment
                        last_seg.content.push(ch);
                      } else {
                        // Start new segment
                        segments.push(TextSegment {
                          content: ch.to_string(),
                          style: TextStyle {
                            size: UI_FONT_SIZE,
                            color: active_color,
                          },
                        });
                      }
                    } else {
                      // First segment
                      segments.push(TextSegment {
                        content: ch.to_string(),
                        style: TextStyle {
                          size: UI_FONT_SIZE,
                          color: active_color,
                        },
                      });
                    }

                    current_char_idx += 1;
                    current_byte_in_line += ch.len_utf8();
                  }

                  // Render the line with all segments
                  if !segments.is_empty() {
                    surface.draw_text(TextSection {
                      position: (content_x, current_y),
                      texts: segments,
                    });
                  }

                  current_y += line_height;
                  byte_offset = line_end_byte;
                }
              },
            );
          },
          PreviewData::Placeholder(placeholder) => {
            // Show placeholder text centered
            let text_width = placeholder.len() as f32 * surface.cell_width();
            let text_x = preview_x + (preview_width - text_width) / 2.0;
            let text_y = y + height_scaled / 2.0;

            surface.draw_text(TextSection {
              position: (text_x, text_y),
              texts:    vec![TextSegment {
                content: placeholder.to_string(),
                style:   TextStyle {
                  size: UI_FONT_SIZE,
                  color: text_color_preview,
                },
              }],
            });
          },
        }
      } else {
        // No preview available - show placeholder
        let placeholder = "No preview";
        let text_width = placeholder.len() as f32 * surface.cell_width();
        let text_x = preview_x + (preview_width - text_width) / 2.0;
        let text_y = y + height_scaled / 2.0;

        surface.draw_text(TextSection {
          position: (text_x, text_y),
          texts:    vec![TextSegment {
            content: placeholder.to_string(),
            style:   TextStyle {
              size: UI_FONT_SIZE,
              color: text_color_preview,
            },
          }],
        });
      }
    }
    }); // End overlay region
  }

  fn cursor(&self, _area: Rect, _editor: &crate::editor::Editor) -> (Option<Position>, CursorKind) {
    (None, CursorKind::Hidden)
  }

  fn should_update(&self) -> bool {
    // Request redraws while any animation is active or matcher is processing
    self.anim_lerp < 1.0
      || self.preview_anim > 0.0 && self.preview_anim < 1.0
      || self.selection_anim < 1.0
      || self.query_cursor_anim_active
      || self.matcher_running
      || self.height_anim_active
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[derive(Debug, Clone, PartialEq)]
  struct TestItem {
    name:  String,
    value: u32,
  }

  struct TestData {
    prefix: String,
  }

  #[test]
  fn test_column_new_creates_filterable_visible_column() {
    let column = Column::<TestItem, TestData>::new("Name", |item, _data| item.name.clone());

    assert_eq!(column.name.as_ref(), "Name");
    assert!(column.filter, "New column should be filterable by default");
    assert!(!column.hidden, "New column should be visible by default");
  }

  #[test]
  fn test_column_hidden_creates_hidden_column() {
    let column = Column::<TestItem, TestData>::hidden("Hidden");

    assert_eq!(column.name.as_ref(), "Hidden");
    assert!(!column.filter, "Hidden column should not be filterable");
    assert!(column.hidden, "Hidden column should be hidden");
  }

  #[test]
  fn test_column_without_filtering_disables_filtering() {
    let column =
      Column::<TestItem, TestData>::new("Name", |item, _data| item.name.clone()).without_filtering();

    assert_eq!(column.name.as_ref(), "Name");
    assert!(!column.filter, "Column should not be filterable");
    assert!(!column.hidden, "Column should still be visible");
  }

  #[test]
  fn test_column_format_with_editor_data() {
    let column = Column::<TestItem, TestData>::new("Name", |item, data| {
      format!("{}{}", data.prefix, item.name)
    });

    let item = TestItem {
      name:  "test".to_string(),
      value: 42,
    };
    let data = TestData {
      prefix: "prefix_".to_string(),
    };

    let result = (column.format)(&item, &data);
    assert_eq!(result, "prefix_test");
  }

  #[test]
  fn test_picker_new_with_single_column() {
    let columns = vec![Column::new("Name", |item: &TestItem, _data: &()| {
      item.name.clone()
    })];

    let items = vec![
      TestItem {
        name:  "foo".to_string(),
        value: 1,
      },
      TestItem {
        name:  "bar".to_string(),
        value: 2,
      },
    ];

    let mut picker = Picker::new(columns, 0, items, (), |_item| {});

    // Process injected items
    picker.matcher.tick(10);

    let snapshot = picker.matcher.snapshot();
    assert_eq!(snapshot.item_count(), 2);
  }

  #[test]
  fn test_picker_new_with_multiple_columns() {
    let columns = vec![
      Column::new("Name", |item: &TestItem, _data: &()| item.name.clone()),
      Column::new("Value", |item: &TestItem, _data: &()| item.value.to_string()),
    ];

    let items = vec![TestItem {
      name:  "test".to_string(),
      value: 42,
    }];

    let picker = Picker::new(columns, 0, items, (), |_item| {});

    assert_eq!(picker.columns.len(), 2);
    assert_eq!(picker.primary_column, 0);
  }

  #[test]
  fn test_picker_with_hidden_column() {
    let columns = vec![
      Column::new("Name", |item: &TestItem, _data: &()| item.name.clone()),
      Column::hidden("Hidden"),
      Column::new("Value", |item: &TestItem, _data: &()| item.value.to_string()),
    ];

    let items = vec![TestItem {
      name:  "test".to_string(),
      value: 42,
    }];

    let picker = Picker::new(columns, 0, items, (), |_item| {});

    let visible_columns: Vec<_> = picker.columns.iter().filter(|c| !c.hidden).collect();
    assert_eq!(visible_columns.len(), 2);
  }

  #[test]
  fn test_picker_with_non_filterable_column() {
    let columns = vec![
      Column::new("Name", |item: &TestItem, _data: &()| item.name.clone()),
      Column::new("Value", |item: &TestItem, _data: &()| item.value.to_string()).without_filtering(),
    ];

    let items = vec![TestItem {
      name:  "test".to_string(),
      value: 42,
    }];

    let picker = Picker::new(columns, 0, items, (), |_item| {});

    let filterable_columns: Vec<_> = picker.columns.iter().filter(|c| c.filter).collect();
    assert_eq!(filterable_columns.len(), 1);
  }

  #[test]
  #[should_panic(expected = "Picker must have at least one filterable column")]
  fn test_picker_panics_with_no_filterable_columns() {
    let columns = vec![
      Column::new("Name", |item: &TestItem, _data: &()| item.name.clone()).without_filtering(),
      Column::new("Value", |item: &TestItem, _data: &()| item.value.to_string()).without_filtering(),
    ];

    let _picker = Picker::new(columns, 0, Vec::<TestItem>::new(), (), |_item| {});
  }

  #[test]
  fn test_injector_push() {
    let columns = vec![Column::new("Name", |item: &TestItem, _data: &()| {
      item.name.clone()
    })];

    let picker = Picker::new(columns, 0, Vec::new(), (), |_item| {});
    let injector = picker.injector();

    let item = TestItem {
      name:  "injected".to_string(),
      value: 100,
    };

    assert!(injector.push(item).is_ok());
  }

  #[test]
  fn test_injector_push_after_version_change() {
    let columns = vec![Column::new("Name", |item: &TestItem, _data: &()| {
      item.name.clone()
    })];

    let mut picker = Picker::new(columns, 0, Vec::new(), (), |_item| {});
    let injector = picker.injector();

    // Close picker (increments version)
    picker.close();

    let item = TestItem {
      name:  "injected".to_string(),
      value: 100,
    };

    // Should fail because picker version changed
    assert!(injector.push(item).is_err());
  }

  #[test]
  fn test_picker_selection() {
    let columns = vec![Column::new("Name", |item: &TestItem, _data: &()| {
      item.name.clone()
    })];

    let items = vec![
      TestItem {
        name:  "foo".to_string(),
        value: 1,
      },
      TestItem {
        name:  "bar".to_string(),
        value: 2,
      },
    ];

    let mut picker = Picker::new(columns, 0, items, (), |_item| {});

    // Process injected items
    picker.matcher.tick(10);

    // Initially cursor is at 0
    let selection = picker.selection();
    assert!(selection.is_some());
    assert_eq!(selection.unwrap().name, "foo");
  }

  #[test]
  fn test_picker_move_down() {
    let columns = vec![Column::new("Name", |item: &TestItem, _data: &()| {
      item.name.clone()
    })];

    let items = vec![
      TestItem {
        name:  "foo".to_string(),
        value: 1,
      },
      TestItem {
        name:  "bar".to_string(),
        value: 2,
      },
    ];

    let mut picker = Picker::new(columns, 0, items, (), |_item| {});

    // Process injected items
    picker.matcher.tick(10);

    assert_eq!(picker.cursor, 0);
    picker.move_down();
    assert_eq!(picker.cursor, 1);
    picker.move_down(); // Should wrap to 0
    assert_eq!(picker.cursor, 0);
  }

  #[test]
  fn test_picker_move_up() {
    let columns = vec![Column::new("Name", |item: &TestItem, _data: &()| {
      item.name.clone()
    })];

    let items = vec![
      TestItem {
        name:  "foo".to_string(),
        value: 1,
      },
      TestItem {
        name:  "bar".to_string(),
        value: 2,
      },
    ];

    let mut picker = Picker::new(columns, 0, items, (), |_item| {});

    // Process injected items
    picker.matcher.tick(10);

    assert_eq!(picker.cursor, 0);
    picker.move_up(); // Should wrap to last item
    assert_eq!(picker.cursor, 1);
    picker.move_up();
    assert_eq!(picker.cursor, 0);
  }
}
