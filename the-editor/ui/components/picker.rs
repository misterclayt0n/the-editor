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
pub enum CachedPreview {
  /// Loaded document with syntax highlighting
  Document(Box<Document>),
  /// Directory with list of entries
  Directory(Vec<String>),
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
    lines:       Vec<String>,
    /// Syntax highlights: (highlight, byte_range)
    highlights:  Vec<(crate::core::syntax::Highlight, std::ops::Range<usize>)>,
    /// Offset of the first line in the document (for scrolled views)
    line_offset: usize,
    /// Byte offset of the first line in the document
    byte_offset: usize,
  },
  Directory {
    entries: Vec<String>,
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
  pub name:           Arc<str>,
  pub format:         ColumnFormatFn<T, D>,
  /// Whether this column should be used for nucleo matching/filtering
  pub filter:         bool,
  /// Whether this column is hidden (data-only, not displayed)
  pub hidden:         bool,
  /// Whether to truncate from the start (true) or end (false) when text is too
  /// long Useful for file paths where you want to see the filename at the end
  pub truncate_start: bool,
}

impl<T, D> Column<T, D> {
  /// Create a new column with the given name and format function
  pub fn new(name: impl Into<Arc<str>>, format: ColumnFormatFn<T, D>) -> Self {
    Self {
      name: name.into(),
      format,
      filter: true,
      hidden: false,
      truncate_start: false,
    }
  }

  /// Create a hidden column (not displayed, data-only)
  pub fn hidden(name: impl Into<Arc<str>>) -> Self {
    Self {
      name:           name.into(),
      format:         |_, _| unreachable!("hidden column should never be formatted"),
      filter:         false,
      hidden:         true,
      truncate_start: false,
    }
  }

  /// Disable filtering for this column (won't be passed to nucleo)
  pub fn without_filtering(mut self) -> Self {
    self.filter = false;
    self
  }

  /// Enable truncation from the start instead of the end
  /// Useful for file paths where you want to see the filename at the end
  pub fn with_truncate_start(mut self) -> Self {
    self.truncate_start = true;
    self
  }
}

/// Actions that can be performed on picker items
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PickerAction {
  /// Primary action (typically Enter key - open/select)
  Primary,
  /// Secondary action (typically Ctrl+s - horizontal split)
  Secondary,
  /// Tertiary action (typically Ctrl+v - vertical split)
  Tertiary,
}

/// Handler function for picker actions
/// Takes the selected item, editor data, and action type
/// Returns true if the picker should close, false to keep it open
pub type ActionHandler<T, D> = Arc<dyn Fn(&T, &D, PickerAction) -> bool + Send + Sync>;

/// Callback for dynamic queries that fetch items based on the query string
/// Takes the query string and an injector to add items asynchronously
/// This is useful for LSP workspace symbols, where we query the server as the
/// user types
pub type DynQueryCallback<T, D> = Arc<dyn Fn(String, Injector<T, D>) + Send + Sync>;

/// Preview handler for custom preview loading (can be async)
/// Takes a PathBuf and Context, returns an optional CachedPreview
pub type PreviewHandler =
  Arc<dyn Fn(&std::path::Path, &Context) -> Option<CachedPreview> + Send + Sync>;

/// A filter in the query
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueryFilter {
  /// Filter all columns with this pattern
  AllColumns(String),
  /// Filter a specific column with this pattern
  Column { name: String, pattern: String },
}

/// Parsed query with multiple filters
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedQuery {
  /// List of filters to apply
  pub filters: Vec<QueryFilter>,
}

impl ParsedQuery {
  /// Parse a query string into filters
  /// Syntax: `column:pattern` for column-specific, `pattern` for all columns
  /// Multiple filters separated by spaces
  pub fn parse(query: &str) -> Self {
    let mut filters = Vec::new();

    // Split query into tokens (space-separated, but respect quotes later if needed)
    for token in query.split_whitespace() {
      if let Some((column_name, pattern)) = token.split_once(':') {
        // Column-specific filter: "name:foo"
        if !column_name.is_empty() && !pattern.is_empty() {
          filters.push(QueryFilter::Column {
            name:    column_name.to_string(),
            pattern: pattern.to_string(),
          });
        }
      } else if !token.is_empty() {
        // All-columns filter: "foo"
        filters.push(QueryFilter::AllColumns(token.to_string()));
      }
    }

    ParsedQuery { filters }
  }

  /// Get the pattern for a specific column name
  /// Returns None if no filter applies to this column
  pub fn pattern_for_column(&self, column_name: &str) -> Option<String> {
    let mut patterns = Vec::new();

    for filter in &self.filters {
      match filter {
        QueryFilter::AllColumns(pattern) => {
          patterns.push(pattern.as_str());
        },
        QueryFilter::Column { name, pattern } if name == column_name => {
          patterns.push(pattern.as_str());
        },
        _ => {},
      }
    }

    if patterns.is_empty() {
      None
    } else {
      // Combine patterns with space (nucleo will OR them)
      Some(patterns.join(" "))
    }
  }

  /// Check if the query is empty
  pub fn is_empty(&self) -> bool {
    self.filters.is_empty()
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
  matcher:               Nucleo<T>,
  /// Columns for the picker table
  columns:               Arc<[Column<T, D>]>,
  /// Primary column index (default for filtering)
  primary_column:        usize,
  /// Editor data passed to column formatters
  editor_data:           Arc<D>,
  /// Current cursor position in results
  cursor:                u32,
  /// Search query
  query:                 String,
  /// Cursor position in query
  query_cursor:          usize,
  /// Version counter for invalidating background tasks
  version:               Arc<AtomicUsize>,
  /// Callback when item is selected (deprecated, use action_handler instead)
  on_select:             Box<dyn Fn(&T) + Send>,
  /// Action handler for picker actions (open, split, etc.)
  action_handler:        Option<ActionHandler<T, D>>,
  /// Callback when picker is closed
  on_close:              Option<Box<dyn FnOnce() + Send>>,
  /// Whether picker is visible
  visible:               bool,
  /// Number of visible rows
  completion_height:     u16,
  /// Entrance animation
  entrance_anim:         crate::core::animation::AnimationHandle<f32>,
  /// Preview panel fade animation
  preview_anim:          Option<crate::core::animation::AnimationHandle<f32>>,
  /// Hovered item index (for hover effects)
  hovered_item:          Option<u32>,
  /// Mouse position for hover effects
  hover_pos:             Option<(f32, f32)>,
  /// Cached layout info for mouse hit testing
  cached_layout:         Option<PickerLayout>,
  /// Previous cursor position for smooth animation
  prev_cursor:           u32,
  /// Selection animation
  selection_anim:        crate::core::animation::AnimationHandle<f32>,
  /// Input cursor animation
  query_cursor_anim:     Option<crate::core::animation::AnimationHandle<f32>>,
  /// Scroll offset for independent scrolling (VSCode-style)
  scroll_offset:         u32,
  /// Whether nucleo is still processing matches
  matcher_running:       bool,
  /// Height animation for smooth size transitions
  height_anim:           Option<crate::core::animation::AnimationHandle<f32>>,
  /// Preview callback to get file path from item, optionally with line range
  /// Returns (PathBuf, Option<(start_line, end_line)>) where lines are
  /// 0-indexed
  preview_fn: Option<Arc<dyn Fn(&T) -> Option<(PathBuf, Option<(usize, usize)>)> + Send + Sync>>,
  /// Custom preview handler for loading previews
  preview_handler:       Option<PreviewHandler>,
  /// Cache of loaded previews
  preview_cache:         HashMap<PathBuf, CachedPreview>,
  /// Reusable buffer for binary detection
  read_buffer:           Vec<u8>,
  /// Dynamic query callback for async item fetching
  dyn_query_callback:    Option<DynQueryCallback<T, D>>,
  /// Debounce timer for dynamic queries (milliseconds)
  dyn_query_debounce_ms: u64,
  /// Time when query was last changed (for debouncing)
  last_query_change:     Option<std::time::Instant>,
  /// Last query that was sent to dynamic callback
  last_dyn_query:        String,
  /// Register to store picker history (selected items)
  history_register:      Option<char>,
  /// Format function to convert items to strings for history register
  history_format:        Option<Arc<dyn Fn(&T, &D) -> String + Send + Sync>>,
  /// Pending items to add to history (flushed during render)
  pending_history:       Vec<String>,
  /// Whether preview animation has been initialized
  preview_initialized:   bool,
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
      action_handler: None,
      on_close: None,
      visible: true,
      completion_height: 0,
      entrance_anim: {
        let (duration, easing) = crate::core::animation::presets::POPUP;
        crate::core::animation::AnimationHandle::new(0.0, 1.0, duration, easing)
      },
      preview_anim: None,
      hovered_item: None,
      hover_pos: None,
      cached_layout: None,
      prev_cursor: 0,
      selection_anim: {
        let (duration, easing) = crate::core::animation::presets::FAST;
        crate::core::animation::AnimationHandle::new(1.0, 1.0, duration, easing)
      },
      query_cursor_anim: None,
      scroll_offset: 0,
      matcher_running: false,
      height_anim: None,
      preview_fn: None,
      preview_handler: None,
      preview_cache: HashMap::new(),
      read_buffer: Vec::new(),
      dyn_query_callback: None,
      dyn_query_debounce_ms: 300, // Default 300ms debounce
      last_query_change: None,
      last_dyn_query: String::new(),
      history_register: None,
      history_format: None,
      pending_history: Vec::new(),
      preview_initialized: false,
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
  /// The callback returns an optional tuple of (PathBuf, Option<(start_line,
  /// end_line)>) where line numbers are 0-indexed and the range will be
  /// highlighted in the preview
  pub fn with_preview<F>(mut self, preview_fn: F) -> Self
  where
    F: Fn(&T) -> Option<(PathBuf, Option<(usize, usize)>)> + Send + Sync + 'static,
  {
    self.preview_fn = Some(Arc::new(preview_fn));
    self
  }

  /// Set a custom preview handler for loading previews
  /// This allows customizing how previews are loaded, including async loading
  /// The handler receives the file path and context, and returns a
  /// CachedPreview
  pub fn with_preview_handler(mut self, handler: PreviewHandler) -> Self {
    self.preview_handler = Some(handler);
    self
  }

  /// Set the action handler for custom picker actions
  pub fn with_action_handler(mut self, handler: ActionHandler<T, D>) -> Self {
    self.action_handler = Some(handler);
    self
  }

  /// Set the dynamic query callback for async item fetching
  /// The callback will be called when the user types, with debouncing
  /// Useful for LSP workspace symbols or other dynamic data sources
  pub fn with_dynamic_query(mut self, callback: DynQueryCallback<T, D>) -> Self {
    self.dyn_query_callback = Some(callback);
    self
  }

  /// Set the debounce delay for dynamic queries (in milliseconds)
  /// Default is 300ms
  pub fn with_debounce(mut self, debounce_ms: u64) -> Self {
    self.dyn_query_debounce_ms = debounce_ms;
    self
  }

  /// Set the history register to store selected items
  /// Selected items will be pushed to this register, allowing access to picker
  /// history The format function converts items to strings for storage in the
  /// register
  pub fn with_history_register<F>(mut self, register: char, format: F) -> Self
  where
    F: Fn(&T, &D) -> String + Send + Sync + 'static,
  {
    self.history_register = Some(register);
    self.history_format = Some(Arc::new(format));
    self
  }

  /// Get the currently selected item
  pub fn selection(&self) -> Option<&T> {
    let snapshot = self.matcher.snapshot();
    snapshot.get_matched_item(self.cursor).map(|item| item.data)
  }

  /// Get preview for the currently selected item
  fn get_preview(&mut self, ctx: &Context) -> Option<(&CachedPreview, Option<(usize, usize)>)> {
    let preview_fn = self.preview_fn.as_ref()?;
    let selected = self.selection()?;
    let (path, line_range) = (preview_fn)(selected)?;

    // Check if already open in editor - if so, always use fresh data
    if let Some(doc) = ctx.editor.document_by_path(&path) {
      // Create a temporary document by extracting the necessary data
      let text_rope = doc.text().clone();

      // Create a new document with the same text
      let mut temp_doc = Document::from(
        text_rope,
        None, // encoding
        ctx.editor.config.clone(),
        ctx.editor.syn_loader.clone(),
      );

      // Set the same language as the original document to get proper syntax
      // highlighting
      if let Some(ref lang) = doc.language {
        let loader = ctx.editor.syn_loader.load();
        temp_doc.set_language(Some(lang.clone()), &loader);
      }

      // Cache it (it will be replaced on next call if document changes)
      self
        .preview_cache
        .insert(path.clone(), CachedPreview::Document(Box::new(temp_doc)));
      return Some((&self.preview_cache[&path], line_range));
    }

    // Check cache (for documents not open in editor)
    if self.preview_cache.contains_key(&path) {
      return Some((&self.preview_cache[&path], line_range));
    }

    // Use custom preview handler if provided
    if let Some(ref handler) = self.preview_handler {
      if let Some(preview) = handler(&path, ctx) {
        self.preview_cache.insert(path.clone(), preview);
        return Some((&self.preview_cache[&path], line_range));
      }
      // Handler returned None, use default loading
    }

    // Load file
    let preview = std::fs::metadata(&path)
      .and_then(|metadata| {
        if metadata.is_dir() {
          // Handle directory: list its contents
          let mut entries = std::fs::read_dir(&path)?
            .filter_map(|entry| entry.ok())
            .map(|entry| {
              let file_name = entry.file_name().to_string_lossy().to_string();
              let is_dir = entry
                .file_type()
                .ok()
                .map(|ft| ft.is_dir())
                .unwrap_or(false);
              if is_dir {
                format!("{}/", file_name)
              } else {
                file_name
              }
            })
            .collect::<Vec<_>>();

          // Sort: directories first, then files, both alphabetically
          entries.sort_by(|a, b| {
            let a_is_dir = a.ends_with('/');
            let b_is_dir = b.ends_with('/');
            match (a_is_dir, b_is_dir) {
              (true, false) => std::cmp::Ordering::Less,
              (false, true) => std::cmp::Ordering::Greater,
              _ => a.cmp(b),
            }
          });

          Ok(CachedPreview::Directory(entries))
        } else if metadata.is_file() {
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
          )
          .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;

          Ok(CachedPreview::Document(Box::new(doc)))
        } else {
          Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Not a regular file or directory",
          ))
        }
      })
      .unwrap_or(CachedPreview::NotFound);

    self.preview_cache.insert(path.clone(), preview);
    Some((&self.preview_cache[&path], line_range))
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
    // Restart selection animation
    let (duration, easing) = crate::core::animation::presets::FAST;
    self.selection_anim = crate::core::animation::AnimationHandle::new(0.0, 1.0, duration, easing);

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
    // Restart selection animation
    let (duration, easing) = crate::core::animation::presets::FAST;
    self.selection_anim = crate::core::animation::AnimationHandle::new(0.0, 1.0, duration, easing);

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
    // Restart selection animation
    let (duration, easing) = crate::core::animation::presets::FAST;
    self.selection_anim = crate::core::animation::AnimationHandle::new(0.0, 1.0, duration, easing);

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
    // Restart selection animation
    let (duration, easing) = crate::core::animation::presets::FAST;
    self.selection_anim = crate::core::animation::AnimationHandle::new(0.0, 1.0, duration, easing);

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
    // Restart selection animation
    let (duration, easing) = crate::core::animation::presets::FAST;
    self.selection_anim = crate::core::animation::AnimationHandle::new(0.0, 1.0, duration, easing);
    self.ensure_cursor_in_view();
  }

  fn to_end(&mut self) {
    let len = self.matcher.snapshot().matched_item_count();
    if len == 0 {
      return;
    }
    self.prev_cursor = self.cursor;
    self.cursor = len.saturating_sub(1);
    // Restart selection animation
    let (duration, easing) = crate::core::animation::presets::FAST;
    self.selection_anim = crate::core::animation::AnimationHandle::new(0.0, 1.0, duration, easing);
    self.ensure_cursor_in_view();
  }

  /// Update the search query
  fn update_query(&mut self) {
    use nucleo::pattern::{
      CaseMatching,
      Normalization,
    };

    // Parse the query into filters
    let parsed = ParsedQuery::parse(&self.query);

    // Update pattern for each filterable column
    let mut column_idx = 0;
    for column in self.columns.iter().filter(|c| c.filter) {
      // Get the pattern for this specific column
      let pattern = if parsed.is_empty() {
        // Empty query - match everything
        String::new()
      } else {
        // Get column-specific pattern, or empty if no filters apply
        parsed.pattern_for_column(&column.name).unwrap_or_default()
      };

      self.matcher.pattern.reparse(
        column_idx,
        &pattern,
        CaseMatching::Smart,
        Normalization::Smart,
        false,
      );

      column_idx += 1;
    }

    // Mark query as changed for debouncing
    self.last_query_change = Some(std::time::Instant::now());
  }

  /// Trigger the dynamic query callback if conditions are met
  fn trigger_dynamic_query(&mut self) {
    // Only trigger if we have a dynamic query callback
    let Some(ref callback) = self.dyn_query_callback else {
      return;
    };

    // Check if query has changed since last dynamic query
    if self.query == self.last_dyn_query {
      return;
    }

    // Check debounce timer
    if let Some(last_change) = self.last_query_change {
      let elapsed = last_change.elapsed().as_millis() as u64;
      if elapsed < self.dyn_query_debounce_ms {
        // Still within debounce period, don't trigger yet
        return;
      }
    }

    // Clear existing items and bump version so background injectors stop
    self.version.fetch_add(1, Ordering::Relaxed);
    self.matcher.restart(false);
    self.last_dyn_query = self.query.clone();

    // Call the dynamic query callback
    let query = self.query.clone();
    let injector = self.injector();
    callback(query, injector);
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
    self.query_cursor = self
      .query
      .chars()
      .take(char_idx)
      .map(|c| c.len_utf8())
      .sum();
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
    self.query_cursor = self
      .query
      .chars()
      .take(char_idx)
      .map(|c| c.len_utf8())
      .sum();
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

  /// Execute an action on the selected item
  /// Returns true if the picker should close
  fn execute_action(&mut self, action: PickerAction) -> bool {
    let Some(item) = self.selection() else {
      return false;
    };

    // Add to history if configured (format immediately to avoid borrow issues)
    let history_entry = if self.history_register.is_some() {
      self
        .history_format
        .as_ref()
        .map(|format_fn| format_fn(item, &self.editor_data))
    } else {
      None
    };

    let result = if let Some(ref handler) = self.action_handler {
      // Use action handler
      handler(item, &self.editor_data, action)
    } else {
      // Fall back to on_select for backward compatibility
      // Only execute for Primary action
      if action == PickerAction::Primary {
        (self.on_select)(item);
        true
      } else {
        false
      }
    };

    // Push history entry after action is executed
    if let Some(entry) = history_entry {
      self.pending_history.push(entry);
    }

    result
  }

  /// Select current item (deprecated, use execute_action instead)
  fn select(&mut self) {
    let should_close = self.execute_action(PickerAction::Primary);
    if should_close {
      self.close();
    }
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
                  // Restart selection animation
                  let (duration, easing) = crate::core::animation::presets::FAST;
                  self.selection_anim =
                    crate::core::animation::AnimationHandle::new(0.0, 1.0, duration, easing);
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
      (Key::Escape, ..) | (Key::Char('c'), true, ..) => {
        self.close();
        let callback = Box::new(
          |compositor: &mut crate::ui::compositor::Compositor, _ctx: &mut Context| {
            compositor.pop();
          },
        );
        EventResult::Consumed(Some(callback))
      },
      // Enter - primary action (open/select)
      (Key::Enter, false, false, _) => {
        let should_close = self.execute_action(PickerAction::Primary);
        if should_close {
          self.close();
          let callback = Box::new(
            |compositor: &mut crate::ui::compositor::Compositor, _ctx: &mut Context| {
              compositor.pop();
            },
          );
          EventResult::Consumed(Some(callback))
        } else {
          EventResult::Consumed(None)
        }
      },
      // Ctrl+s - secondary action (horizontal split)
      (Key::Char('s'), true, false, false) => {
        let should_close = self.execute_action(PickerAction::Secondary);
        if should_close {
          self.close();
          let callback = Box::new(
            |compositor: &mut crate::ui::compositor::Compositor, _ctx: &mut Context| {
              compositor.pop();
            },
          );
          EventResult::Consumed(Some(callback))
        } else {
          EventResult::Consumed(None)
        }
      },
      // Ctrl+v - tertiary action (vertical split)
      (Key::Char('v'), true, false, false) => {
        let should_close = self.execute_action(PickerAction::Tertiary);
        if should_close {
          self.close();
          let callback = Box::new(
            |compositor: &mut crate::ui::compositor::Compositor, _ctx: &mut Context| {
              compositor.pop();
            },
          );
          EventResult::Consumed(Some(callback))
        } else {
          EventResult::Consumed(None)
        }
      },

      // Query text editing (Emacs-style)
      // Ctrl+b / Left - backward char
      (Key::Char('b'), true, ..) | (Key::Left, false, false, false) => {
        self.move_cursor_left();
        EventResult::Consumed(None)
      },
      // Ctrl+f / Right - forward char
      (Key::Char('f'), true, ..) | (Key::Right, false, false, false) => {
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
      (Key::Char('a'), true, ..) => {
        self.query_cursor = 0;
        EventResult::Consumed(None)
      },
      // Ctrl+e - end of query line
      (Key::Char('e'), true, ..) => {
        self.query_cursor = self.query.len();
        EventResult::Consumed(None)
      },
      // Ctrl+h / Backspace - delete char backwards
      (Key::Char('h'), true, ..) | (Key::Backspace, false, false, _) => {
        self.delete_char_backwards();
        EventResult::Consumed(None)
      },
      // Delete - delete char forward
      (Key::Delete, false, false, false) => {
        self.delete_char_forward();
        EventResult::Consumed(None)
      },
      // Ctrl+w / Alt+Backspace / Ctrl+Backspace - delete word backward
      (Key::Char('w'), true, ..) | (Key::Backspace, _, true, _) | (Key::Backspace, true, ..) => {
        self.delete_word_backward();
        EventResult::Consumed(None)
      },
      // Alt+d / Alt+Delete / Ctrl+Delete - delete word forward
      (Key::Char('d'), _, true, _) | (Key::Delete, _, true, _) | (Key::Delete, true, ..) => {
        self.delete_word_forward();
        EventResult::Consumed(None)
      },
      // Ctrl+k - kill to end of query
      (Key::Char('k'), true, ..) => {
        self.kill_to_end();
        EventResult::Consumed(None)
      },

      // List navigation
      // Ctrl+p / Up / Shift+Tab - move up
      (Key::Char('p'), true, ..) | (Key::Up, false, false, false) | (Key::Tab, _, _, true) => {
        self.move_up();
        EventResult::Consumed(None)
      },
      // Ctrl+n / Down / Tab - move down
      (Key::Char('n'), true, ..) | (Key::Down, false, false, false) | (Key::Tab, _, _, false) => {
        self.move_down();
        EventResult::Consumed(None)
      },
      // Ctrl+u / PageUp - page up
      (Key::Char('u'), true, ..) | (Key::PageUp, ..) => {
        self.page_up();
        EventResult::Consumed(None)
      },
      // Ctrl+d / PageDown - page down (note: conflicts with delete forward, but Ctrl+d for page
      // down is more common in pickers) We handle delete forward with just Delete key above
      (Key::Char('d'), true, ..) | (Key::PageDown, ..) => {
        self.page_down();
        EventResult::Consumed(None)
      },
      // Home - to start
      (Key::Home, ..) => {
        self.to_start();
        EventResult::Consumed(None)
      },
      // End - to end
      (Key::End, ..) => {
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

    // Flush pending history to register
    if let Some(register) = self.history_register {
      if !self.pending_history.is_empty() {
        // Push items to register (most recent first, so reverse)
        for item in self.pending_history.drain(..).rev() {
          let _ = ctx.editor.registers.push(register, item);
        }
      }
    }

    // Update entrance animation
    self.entrance_anim.update(ctx.dt);

    // Update selection animation
    self.selection_anim.update(ctx.dt);

    // Determine if we should show preview panel based on actual available width
    // Need enough room for both panels + gap (minimum ~1200px for comfortable
    // split)
    let min_width_for_preview = 1200.0;
    let should_show_preview = area.width as f32 > min_width_for_preview;

    // Initialize or update preview animation
    let target_preview_value = if should_show_preview { 1.0 } else { 0.0 };
    if !self.preview_initialized {
      // On first render, start at target (no animation)
      let (duration, easing) = crate::core::animation::presets::FAST;
      self.preview_anim = Some(crate::core::animation::AnimationHandle::new(
        target_preview_value,
        target_preview_value,
        duration,
        easing,
      ));
      self.preview_initialized = true;
    } else {
      // Animate to new target if it changed
      match &mut self.preview_anim {
        Some(anim) => {
          if (*anim.target() - target_preview_value).abs() > 0.01 {
            // Target changed, retarget animation
            anim.retarget(target_preview_value);
          }
          anim.update(ctx.dt);
        },
        None => {
          // Create animation if it doesn't exist
          let (duration, easing) = crate::core::animation::presets::FAST;
          let current = if should_show_preview { 0.0 } else { 1.0 };
          self.preview_anim = Some(crate::core::animation::AnimationHandle::new(
            current,
            target_preview_value,
            duration,
            easing,
          ));
        },
      }
    }

    // Process pending updates from nucleo
    let status = self.matcher.tick(10);
    self.matcher_running = status.running || status.changed;

    // Check if we should trigger a dynamic query (debounce timer elapsed)
    self.trigger_dynamic_query();

    // Get preview before taking snapshot to avoid borrow checker issues
    // First ensure the preview is loaded into cache
    let _preview = self.get_preview(ctx);

    // Now extract preview data with mutable access for highlights
    let (preview_data, preview_line) = {
      let preview_fn = self.preview_fn.as_ref();
      let selected_result = preview_fn
        .and_then(|f| {
          let snapshot = self.matcher.snapshot();
          snapshot
            .get_matched_item(self.cursor)
            .map(|item| (f)(item.data))
        })
        .flatten();

      let (path, line_range) = if let Some((p, l)) = selected_result {
        (p, l)
      } else {
        (std::path::PathBuf::new(), None)
      };

      let data = if !path.as_os_str().is_empty() {
        self.preview_cache.get_mut(&path).map(|preview| {
          match preview {
            CachedPreview::Document(doc) => {
              // Extract the lines we need to render
              let text = doc.text();
              let total_lines = text.len_lines();
              let max_preview_lines = 200; // Maximum lines to load for preview

              // Determine which range of lines to load based on line_range
              let (preview_start, preview_end) =
                if let Some((target_start, target_end)) = line_range {
                  // Calculate context around the target range
                  let target_middle = target_start + (target_end.saturating_sub(target_start)) / 2;
                  let half_context = max_preview_lines / 2;

                  let start = target_middle.saturating_sub(half_context);
                  let end = (start + max_preview_lines).min(total_lines);
                  (start, end)
                } else {
                  // No target range, load from the beginning
                  (0, total_lines.min(max_preview_lines))
                };

              let lines: Vec<String> = (preview_start..preview_end)
                .map(|i| text.line(i).to_string())
                .collect();

              // Get syntax highlights for the loaded range
              let start_byte = text.line_to_byte(preview_start);
              let end_byte = if preview_end < total_lines {
                text.line_to_byte(preview_end)
              } else {
                text.len_bytes()
              };

              let highlights = doc
                .get_viewport_highlights(start_byte..end_byte, &ctx.editor.syn_loader.load())
                .unwrap_or_default();

              PreviewData::Document {
                lines,
                highlights,
                line_offset: preview_start,
                byte_offset: start_byte,
              }
            },
            CachedPreview::Directory(entries) => {
              PreviewData::Directory {
                entries: entries.clone(),
              }
            },
            CachedPreview::Binary => PreviewData::Placeholder("<Binary file>"),
            CachedPreview::LargeFile => PreviewData::Placeholder("<File too large to preview>"),
            CachedPreview::NotFound => PreviewData::Placeholder("<File not found>"),
          }
        })
      } else {
        None
      };

      (data, line_range)
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
    let preview_anim_value = self
      .preview_anim
      .as_ref()
      .map(|a| *a.current())
      .unwrap_or(0.0);
    let picker_width =
      picker_width_full + (picker_width_split - picker_width_full) * preview_anim_value;

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
    // Fixed height: always use max_rows (up to 30) regardless of item count
    // This prevents the picker from resizing based on results
    self.completion_height = max_rows.min(30) as u16;

    // Calculate actual height needed for the items
    let rows_height = if self.completion_height > 0 {
      self.completion_height as f32 * actual_item_height
        + (self.completion_height as f32 - 1.0).max(0.0) * item_gap
    } else {
      0.0
    };

    let height = header_height + rows_height + bottom_padding;

    // Smooth height animation for size changes
    let animated_height = match &mut self.height_anim {
      Some(anim) => {
        // Check if target changed
        if (*anim.target() - height).abs() > 0.5 {
          anim.retarget(height);
        }
        anim.update(ctx.dt);
        *anim.current()
      },
      None => {
        // Initialize animation
        let (duration, easing) = crate::core::animation::presets::FAST;
        self.height_anim = Some(crate::core::animation::AnimationHandle::new(
          height, height, duration, easing,
        ));
        height
      },
    };

    // Apply animation lerp with easing (entrance animation already applies easing)
    let ease = *self.entrance_anim.current();

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

    // Render picker content in overlay mode without masking the underlying
    // buffer so the blur can sample the live editor contents each frame.
    surface.with_overlay(|surface| {
      let picker_bg = Color::new(bg_color.r * 0.9, bg_color.g * 0.9, bg_color.b * 0.9, alpha);
      surface.draw_rounded_rect(
        x,
        y,
        picker_width_scaled,
        height_scaled,
        corner_radius,
        picker_bg,
      );

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
      let cursor_anim_enabled = ctx.editor.config().cursor_anim_enabled;

      let anim_x = if cursor_anim_enabled {
        match &mut self.query_cursor_anim {
          Some(anim) => {
            if (*anim.target() - target_x).abs() > 0.5 {
              anim.retarget(target_x);
            }
            anim.update(ctx.dt);
            *anim.current()
          },
          None => {
            let (duration, easing) = crate::core::animation::presets::CURSOR;
            self.query_cursor_anim = Some(crate::core::animation::AnimationHandle::new(
              target_x, target_x, duration, easing,
            ));
            target_x
          },
        }
      } else {
        self.query_cursor_anim = None;
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
            let selection_ease = *self.selection_anim.current();

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

            if !self.selection_anim.is_complete() {
              let pulse_ease = 1.0 - (1.0 - selection_ease) * (1.0 - selection_ease);
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
          // Check if primary column uses truncate_start
          let truncate_from_start = self
            .columns
            .iter()
            .find(|c| c.filter && !c.hidden)
            .map(|c| c.truncate_start)
            .unwrap_or(false);

          let truncated_text = if max_chars > 3 && display_text.chars().count() > max_chars {
            let truncate_to = max_chars.saturating_sub(3);
            if truncate_from_start {
              // Truncate from start: "...filename"
              let char_count = display_text.chars().count();
              let start_idx = char_count.saturating_sub(truncate_to);
              let truncated: String = display_text.chars().skip(start_idx).collect();
              format!("...{}", truncated)
            } else {
              // Truncate from end: "filename..."
              let truncated: String = display_text.chars().take(truncate_to).collect();
              format!("{}...", truncated)
            }
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
      if preview_anim_value > 0.0 {
        let preview_ease =
          preview_anim_value * preview_anim_value * (3.0 - 2.0 * preview_anim_value); // Smoothstep

        let preview_gap = 12.0; // Padding between picker and preview
        let preview_x = x + picker_width_scaled + preview_gap;
        let preview_width = (total_width - picker_width - preview_gap) * scale;

        // Apply alpha to preview
        let preview_alpha = alpha * preview_ease;
        let bg_color_preview = Color::new(
          bg_color.r * 0.9, // Match picker's darkening
          bg_color.g * 0.9,
          bg_color.b * 0.9,
          preview_alpha,
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
            PreviewData::Document {
              lines,
              highlights,
              line_offset,
              byte_offset,
            } => {
              // Render document content with clipping region
              let padding = 12.0;
              let line_height = UI_FONT_SIZE + 4.0;
              let content_x = preview_x + padding;
              let content_y = y + padding;
              let content_width = preview_width - (padding * 2.0);
              let content_height = height_scaled - (padding * 2.0);

              // Calculate how many lines we can show
              let max_lines = (content_height / line_height).floor() as usize;

              // If we have a preview line range, center the middle of the range in the view
              // Note: lines are in document coordinates, but lines vector is offset by
              // line_offset
              let start_line = if let Some((range_start, range_end)) = preview_line {
                // Adjust target range to be relative to our loaded lines
                let relative_start = range_start.saturating_sub(*line_offset);
                let relative_end = range_end.saturating_sub(*line_offset);

                // Calculate the middle of the range
                let range_height = relative_end.saturating_sub(relative_start);
                let middle_line = relative_start + range_height / 2;
                // Center the middle line, but ensure we don't go below 0
                let half_visible = max_lines / 2;
                middle_line
                  .saturating_sub(half_visible)
                  .min(lines.len().saturating_sub(max_lines))
              } else {
                0
              };

              let end_line = (start_line + max_lines).min(lines.len());
              let lines_to_show = end_line - start_line;

              // Calculate max characters per line based on available width
              let max_chars = (content_width / UI_FONT_WIDTH).floor() as usize;

              // Use overlay region for clipping
              // Capture byte_offset for use in closure
              let doc_byte_start = *byte_offset;
              surface.with_overlay(|surface| {
                let mut relative_byte_offset: usize =
                  lines.iter().take(start_line).map(|s| s.len()).sum();

                for (visible_idx, line_str) in lines
                  .iter()
                  .skip(start_line)
                  .enumerate()
                  .take(lines_to_show)
                {
                  let line_idx = start_line + visible_idx; // Index within lines vector
                  let doc_line_idx = line_idx + line_offset; // Actual line number in document

                  // Calculate text baseline position (following same pattern as hover/completion)
                  let text_y = content_y + UI_FONT_SIZE + visible_idx as f32 * line_height;

                  // Draw highlight background for lines in the target range (use doc coordinates)
                  let should_highlight = if let Some((range_start, range_end)) = preview_line {
                    doc_line_idx >= range_start && doc_line_idx <= range_end
                  } else {
                    false
                  };

                  if should_highlight {
                    let highlight_color = Color::new(
                      border_color_preview.r,
                      border_color_preview.g,
                      border_color_preview.b,
                      0.15 * preview_alpha,
                    );
                    // Draw highlight slightly above baseline, matching completion component
                    surface.draw_rect(
                      content_x,
                      text_y - 2.0,
                      content_width,
                      line_height,
                      highlight_color,
                    );
                  }
                  // Trim trailing whitespace
                  let trimmed = line_str.trim_end();
                  let line_byte_len = line_str.len();

                  // Calculate byte range for this line (relative to lines vector)
                  let line_start_byte = relative_byte_offset;
                  let line_end_byte = relative_byte_offset + line_byte_len;

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
                        style:   TextStyle {
                          size:  UI_FONT_SIZE,
                          color: text_color_preview,
                        },
                      });
                      break;
                    }

                    // Calculate byte position (relative to lines vector)
                    let relative_byte_pos = line_start_byte + current_byte_in_line;
                    // Convert to absolute document byte position for highlight lookup
                    let doc_byte_pos = doc_byte_start + relative_byte_pos;

                    // Find active highlight for this byte position
                    let mut active_color = text_color_preview;
                    for (highlight, range) in highlights.iter() {
                      if range.contains(&doc_byte_pos) {
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
                          style:   TextStyle {
                            size:  UI_FONT_SIZE,
                            color: active_color,
                          },
                        });
                      }
                    } else {
                      // First segment
                      segments.push(TextSegment {
                        content: ch.to_string(),
                        style:   TextStyle {
                          size:  UI_FONT_SIZE,
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
                      position: (content_x, text_y),
                      texts:    segments,
                    });
                  }

                  relative_byte_offset = line_end_byte;
                }
              });
            },
            PreviewData::Directory { entries } => {
              // Render directory listing
              let padding = 12.0;
              let line_height = UI_FONT_SIZE + 4.0;
              let content_x = preview_x + padding;
              let content_y = y + padding;
              let content_width = preview_width - (padding * 2.0);
              let content_height = height_scaled - (padding * 2.0);

              // Calculate how many entries we can show
              let max_entries = (content_height / line_height).floor() as usize;
              let entries_to_show = max_entries.min(entries.len());

              // Calculate max characters per line based on available width
              let max_chars = (content_width / UI_FONT_WIDTH).floor() as usize;

              // Render directory entries in overlay mode without masking
              surface.with_overlay(|surface| {
                for (idx, entry) in entries.iter().take(entries_to_show).enumerate() {
                  let text_y = content_y + UI_FONT_SIZE + idx as f32 * line_height;

                  let is_dir = entry.ends_with('/');

                  // Use different colors for directories vs files
                  let entry_color = if is_dir {
                    // Directories: light blue
                    let mut dir_color = Color::rgb(0.5, 0.7, 1.0);
                    dir_color.a *= preview_alpha;
                    dir_color
                  } else {
                    text_color_preview
                  };

                  // Truncate if entry name is too long
                  let max_display_chars = max_chars.saturating_sub(3);
                  let should_truncate = entry.chars().count() > max_display_chars;

                  let display_text = if should_truncate {
                    let truncated: String = entry.chars().take(max_display_chars).collect();
                    format!("{}...", truncated)
                  } else {
                    entry.clone()
                  };

                  surface.draw_text(TextSection {
                    position: (content_x, text_y),
                    texts:    vec![TextSegment {
                      content: display_text,
                      style:   TextStyle {
                        size:  UI_FONT_SIZE,
                        color: entry_color,
                      },
                    }],
                  });
                }

                // Show count if there are more entries
                if entries.len() > entries_to_show {
                  let remaining = entries.len() - entries_to_show;
                  let more_text = format!("... and {} more", remaining);
                  let text_y = content_y + UI_FONT_SIZE + entries_to_show as f32 * line_height;

                  surface.draw_text(TextSection {
                    position: (content_x, text_y),
                    texts:    vec![TextSegment {
                      content: more_text,
                      style:   TextStyle {
                        size:  UI_FONT_SIZE,
                        color: text_color_preview,
                      },
                    }],
                  });
                }
              });
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
                    size:  UI_FONT_SIZE,
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
                size:  UI_FONT_SIZE,
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
    !self.entrance_anim.is_complete()
      || self
        .preview_anim
        .as_ref()
        .map(|a| !a.is_complete())
        .unwrap_or(false)
      || !self.selection_anim.is_complete()
      || self
        .query_cursor_anim
        .as_ref()
        .map(|a| !a.is_complete())
        .unwrap_or(false)
      || self.matcher_running
      || self
        .height_anim
        .as_ref()
        .map(|a| !a.is_complete())
        .unwrap_or(false)
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
    let column = Column::<TestItem, TestData>::new("Name", |item, _data| item.name.clone())
      .without_filtering();

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
      Column::new("Value", |item: &TestItem, _data: &()| {
        item.value.to_string()
      }),
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
      Column::new("Value", |item: &TestItem, _data: &()| {
        item.value.to_string()
      }),
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
      Column::new("Value", |item: &TestItem, _data: &()| {
        item.value.to_string()
      })
      .without_filtering(),
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
      Column::new("Value", |item: &TestItem, _data: &()| {
        item.value.to_string()
      })
      .without_filtering(),
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

  #[test]
  fn test_action_handler_primary() {
    use std::sync::{
      Arc,
      Mutex,
    };

    let columns = vec![Column::new("Name", |item: &TestItem, _data: &()| {
      item.name.clone()
    })];

    let items = vec![TestItem {
      name:  "foo".to_string(),
      value: 1,
    }];

    let action_log = Arc::new(Mutex::new(Vec::new()));
    let action_log_clone = action_log.clone();

    let handler = Arc::new(move |item: &TestItem, _data: &(), action: PickerAction| {
      action_log_clone
        .lock()
        .unwrap()
        .push((item.name.clone(), action));
      true // Close picker
    });

    let mut picker = Picker::new(columns, 0, items, (), |_| {}).with_action_handler(handler);

    picker.matcher.tick(10);

    let should_close = picker.execute_action(PickerAction::Primary);
    assert!(should_close);

    let log = action_log.lock().unwrap();
    assert_eq!(log.len(), 1);
    assert_eq!(log[0], ("foo".to_string(), PickerAction::Primary));
  }

  #[test]
  fn test_action_handler_secondary() {
    use std::sync::{
      Arc,
      Mutex,
    };

    let columns = vec![Column::new("Name", |item: &TestItem, _data: &()| {
      item.name.clone()
    })];

    let items = vec![TestItem {
      name:  "bar".to_string(),
      value: 2,
    }];

    let action_log = Arc::new(Mutex::new(Vec::new()));
    let action_log_clone = action_log.clone();

    let handler = Arc::new(move |item: &TestItem, _data: &(), action: PickerAction| {
      action_log_clone
        .lock()
        .unwrap()
        .push((item.name.clone(), action));
      true
    });

    let mut picker = Picker::new(columns, 0, items, (), |_| {}).with_action_handler(handler);

    picker.matcher.tick(10);

    let should_close = picker.execute_action(PickerAction::Secondary);
    assert!(should_close);

    let log = action_log.lock().unwrap();
    assert_eq!(log.len(), 1);
    assert_eq!(log[0], ("bar".to_string(), PickerAction::Secondary));
  }

  #[test]
  fn test_action_handler_tertiary() {
    use std::sync::{
      Arc,
      Mutex,
    };

    let columns = vec![Column::new("Name", |item: &TestItem, _data: &()| {
      item.name.clone()
    })];

    let items = vec![TestItem {
      name:  "baz".to_string(),
      value: 3,
    }];

    let action_log = Arc::new(Mutex::new(Vec::new()));
    let action_log_clone = action_log.clone();

    let handler = Arc::new(move |item: &TestItem, _data: &(), action: PickerAction| {
      action_log_clone
        .lock()
        .unwrap()
        .push((item.name.clone(), action));
      false // Don't close picker
    });

    let mut picker = Picker::new(columns, 0, items, (), |_| {}).with_action_handler(handler);

    picker.matcher.tick(10);

    let should_close = picker.execute_action(PickerAction::Tertiary);
    assert!(!should_close); // Handler returns false

    let log = action_log.lock().unwrap();
    assert_eq!(log.len(), 1);
    assert_eq!(log[0], ("baz".to_string(), PickerAction::Tertiary));
  }

  #[test]
  fn test_fallback_to_on_select() {
    use std::sync::{
      Arc,
      Mutex,
    };

    let columns = vec![Column::new("Name", |item: &TestItem, _data: &()| {
      item.name.clone()
    })];

    let items = vec![TestItem {
      name:  "fallback".to_string(),
      value: 99,
    }];

    let selected = Arc::new(Mutex::new(None));
    let selected_clone = selected.clone();

    let mut picker = Picker::new(columns, 0, items, (), move |item: &TestItem| {
      *selected_clone.lock().unwrap() = Some(item.name.clone());
    });

    picker.matcher.tick(10);

    // No action handler set, should fall back to on_select for Primary
    let should_close = picker.execute_action(PickerAction::Primary);
    assert!(should_close);

    let result = selected.lock().unwrap();
    assert_eq!(*result, Some("fallback".to_string()));
  }

  #[test]
  fn test_action_handler_with_editor_data() {
    use std::sync::{
      Arc,
      Mutex,
    };

    struct CustomData {
      prefix: String,
    }

    let columns = vec![Column::new("Name", |item: &TestItem, data: &CustomData| {
      format!("{}{}", data.prefix, item.name)
    })];

    let items = vec![TestItem {
      name:  "test".to_string(),
      value: 42,
    }];

    let editor_data = CustomData {
      prefix: "data_".to_string(),
    };

    let action_log = Arc::new(Mutex::new(Vec::new()));
    let action_log_clone = action_log.clone();

    let handler = Arc::new(
      move |item: &TestItem, data: &CustomData, action: PickerAction| {
        action_log_clone
          .lock()
          .unwrap()
          .push((item.name.clone(), data.prefix.clone(), action));
        true
      },
    );

    let mut picker =
      Picker::new(columns, 0, items, editor_data, |_| {}).with_action_handler(handler);

    picker.matcher.tick(10);

    picker.execute_action(PickerAction::Primary);

    let log = action_log.lock().unwrap();
    assert_eq!(log.len(), 1);
    assert_eq!(
      log[0],
      (
        "test".to_string(),
        "data_".to_string(),
        PickerAction::Primary
      )
    );
  }

  #[test]
  fn test_query_parser_empty() {
    let parsed = ParsedQuery::parse("");
    assert!(parsed.is_empty());
    assert_eq!(parsed.filters.len(), 0);
  }

  #[test]
  fn test_query_parser_all_columns() {
    let parsed = ParsedQuery::parse("foo");
    assert!(!parsed.is_empty());
    assert_eq!(parsed.filters.len(), 1);
    assert_eq!(
      parsed.filters[0],
      QueryFilter::AllColumns("foo".to_string())
    );
  }

  #[test]
  fn test_query_parser_column_specific() {
    let parsed = ParsedQuery::parse("name:foo");
    assert_eq!(parsed.filters.len(), 1);
    assert_eq!(parsed.filters[0], QueryFilter::Column {
      name:    "name".to_string(),
      pattern: "foo".to_string(),
    });
  }

  #[test]
  fn test_query_parser_multiple_filters() {
    let parsed = ParsedQuery::parse("name:foo bar type:baz");
    assert_eq!(parsed.filters.len(), 3);
    assert_eq!(parsed.filters[0], QueryFilter::Column {
      name:    "name".to_string(),
      pattern: "foo".to_string(),
    });
    assert_eq!(
      parsed.filters[1],
      QueryFilter::AllColumns("bar".to_string())
    );
    assert_eq!(parsed.filters[2], QueryFilter::Column {
      name:    "type".to_string(),
      pattern: "baz".to_string(),
    });
  }

  #[test]
  fn test_query_parser_pattern_for_column() {
    let parsed = ParsedQuery::parse("name:foo bar name:baz");

    // name column should get both "foo" and "baz"
    let name_pattern = parsed.pattern_for_column("name");
    assert!(name_pattern.is_some());
    let pattern = name_pattern.unwrap();
    assert!(pattern.contains("foo"));
    assert!(pattern.contains("baz"));

    // non-existent column should get "bar" (all-columns filter)
    let other_pattern = parsed.pattern_for_column("other");
    assert_eq!(other_pattern, Some("bar".to_string()));

    // Column with no filters
    let parsed2 = ParsedQuery::parse("name:foo");
    assert_eq!(parsed2.pattern_for_column("other"), None);
  }

  #[test]
  fn test_query_parser_ignores_invalid() {
    // Empty column name
    let parsed = ParsedQuery::parse(":foo");
    assert!(parsed.is_empty());

    // Empty pattern
    let parsed = ParsedQuery::parse("name:");
    assert!(parsed.is_empty());

    // Just colon
    let parsed = ParsedQuery::parse(":");
    assert!(parsed.is_empty());
  }

  #[test]
  fn test_query_parser_multiple_colons() {
    // Only first colon is used as separator
    let parsed = ParsedQuery::parse("url:http://example.com");
    assert_eq!(parsed.filters.len(), 1);
    assert_eq!(parsed.filters[0], QueryFilter::Column {
      name:    "url".to_string(),
      pattern: "http://example.com".to_string(),
    });
  }

  #[test]
  fn test_picker_query_filtering_basic() {
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
      TestItem {
        name:  "foobar".to_string(),
        value: 3,
      },
    ];

    let mut picker = Picker::new(columns, 0, items, (), |_| {});
    picker.matcher.tick(10);

    // Initially all items match
    let snapshot = picker.matcher.snapshot();
    assert_eq!(snapshot.matched_item_count(), 3);

    // Filter for "foo"
    picker.query = "foo".to_string();
    picker.update_query();
    picker.matcher.tick(10);

    let snapshot = picker.matcher.snapshot();
    // Should match "foo" and "foobar"
    assert!(snapshot.matched_item_count() >= 1);
  }

  #[test]
  fn test_picker_query_filtering_column_specific() {
    let columns = vec![
      Column::new("Name", |item: &TestItem, _data: &()| item.name.clone()),
      Column::new("Value", |item: &TestItem, _data: &()| {
        item.value.to_string()
      }),
    ];

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

    let mut picker = Picker::new(columns, 0, items, (), |_| {});
    picker.matcher.tick(10);

    // Filter for "Name:foo" - should only search name column
    picker.query = "Name:foo".to_string();
    picker.update_query();
    picker.matcher.tick(10);

    let snapshot = picker.matcher.snapshot();
    // Should match at least the "foo" item
    assert!(snapshot.matched_item_count() >= 1);
  }

  #[test]
  fn test_dynamic_query_callback_called_after_debounce() {
    use std::sync::{
      Arc,
      Mutex,
    };

    let columns = vec![Column::new("Name", |item: &TestItem, _data: &()| {
      item.name.clone()
    })];

    let called_queries = Arc::new(Mutex::new(Vec::<String>::new()));
    let called_queries_clone = called_queries.clone();

    let callback = Arc::new(move |query: String, _injector: Injector<TestItem, ()>| {
      called_queries_clone.lock().unwrap().push(query);
    });

    let mut picker = Picker::new(columns, 0, Vec::new(), (), |_| {})
      .with_dynamic_query(callback)
      .with_debounce(100); // 100ms debounce

    // Set query and mark as changed
    picker.query = "test".to_string();
    picker.update_query();

    // Should not trigger immediately (still in debounce period)
    picker.trigger_dynamic_query();
    assert_eq!(called_queries.lock().unwrap().len(), 0);

    // Wait for debounce period
    std::thread::sleep(std::time::Duration::from_millis(150));

    // Now it should trigger
    picker.trigger_dynamic_query();
    let queries = called_queries.lock().unwrap();
    assert_eq!(queries.len(), 1);
    assert_eq!(queries[0], "test");
  }

  #[test]
  fn test_dynamic_query_callback_not_called_for_same_query() {
    use std::sync::{
      Arc,
      Mutex,
    };

    let columns = vec![Column::new("Name", |item: &TestItem, _data: &()| {
      item.name.clone()
    })];

    let called_count = Arc::new(Mutex::new(0));
    let called_count_clone = called_count.clone();

    let callback = Arc::new(move |_query: String, _injector: Injector<TestItem, ()>| {
      *called_count_clone.lock().unwrap() += 1;
    });

    let mut picker = Picker::new(columns, 0, Vec::new(), (), |_| {})
      .with_dynamic_query(callback)
      .with_debounce(50);

    // Set query and mark as changed
    picker.query = "test".to_string();
    picker.update_query();

    // Wait for debounce
    std::thread::sleep(std::time::Duration::from_millis(100));
    picker.trigger_dynamic_query();

    assert_eq!(*called_count.lock().unwrap(), 1);

    // Trigger again without changing query
    picker.trigger_dynamic_query();

    // Should still be 1 (not called again)
    assert_eq!(*called_count.lock().unwrap(), 1);
  }

  #[test]
  fn test_dynamic_query_callback_updates_version() {
    use std::sync::atomic::Ordering;

    let columns = vec![Column::new("Name", |item: &TestItem, _data: &()| {
      item.name.clone()
    })];

    let callback = Arc::new(move |_query: String, _injector: Injector<TestItem, ()>| {
      // Callback doesn't need to do anything for this test
    });

    let mut picker = Picker::new(columns, 0, Vec::new(), (), |_| {})
      .with_dynamic_query(callback)
      .with_debounce(50);

    let initial_version = picker.version.load(Ordering::Relaxed);

    // Set query and trigger
    picker.query = "test".to_string();
    picker.update_query();
    std::thread::sleep(std::time::Duration::from_millis(100));
    picker.trigger_dynamic_query();

    let new_version = picker.version.load(Ordering::Relaxed);
    assert!(
      new_version > initial_version,
      "Version should increment when dynamic query triggers"
    );
  }

  #[test]
  fn test_dynamic_query_injector_works() {
    use std::sync::{
      Arc,
      Mutex,
    };

    let columns = vec![Column::new("Name", |item: &TestItem, _data: &()| {
      item.name.clone()
    })];

    let injected_items = Arc::new(Mutex::new(Vec::<TestItem>::new()));
    let injected_items_clone = injected_items.clone();

    let callback = Arc::new(move |query: String, injector: Injector<TestItem, ()>| {
      // Simulate async query results
      let items = vec![
        TestItem {
          name:  format!("result_{}", query),
          value: 1,
        },
        TestItem {
          name:  format!("another_{}", query),
          value: 2,
        },
      ];

      for item in items.clone() {
        injected_items_clone.lock().unwrap().push(item.clone());
        let _ = injector.push(item);
      }
    });

    let mut picker = Picker::new(columns, 0, Vec::new(), (), |_| {})
      .with_dynamic_query(callback)
      .with_debounce(50);

    // Set query and trigger
    picker.query = "foo".to_string();
    picker.update_query();
    std::thread::sleep(std::time::Duration::from_millis(100));
    picker.trigger_dynamic_query();

    // Process injected items
    picker.matcher.tick(10);

    // Verify items were injected
    let items = injected_items.lock().unwrap();
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].name, "result_foo");
    assert_eq!(items[1].name, "another_foo");

    // Verify picker received the items
    let snapshot = picker.matcher.snapshot();
    assert_eq!(snapshot.item_count(), 2);
  }

  #[test]
  fn test_dynamic_query_without_callback_does_nothing() {
    let columns = vec![Column::new("Name", |item: &TestItem, _data: &()| {
      item.name.clone()
    })];

    let mut picker = Picker::new(columns, 0, Vec::new(), (), |_| {});

    // Set query without dynamic query callback
    picker.query = "test".to_string();
    picker.update_query();
    std::thread::sleep(std::time::Duration::from_millis(100));

    // Should not panic or error
    picker.trigger_dynamic_query();

    // Verify last_dyn_query remains empty (callback never called)
    assert_eq!(picker.last_dyn_query, "");
  }
}
