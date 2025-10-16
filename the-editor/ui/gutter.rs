use the_editor_renderer::{
  Color,
  TextSection,
};

use crate::{
  core::{
    document::Document,
    position::Position,
    view::View,
  },
  editor::Editor,
  theme::Theme,
  ui::compositor::{
    Event,
    Surface,
  },
};

/// Result of gutter event handling
pub enum GutterEventResult {
  /// Event was handled by this gutter
  Handled,
  /// Event was ignored by this gutter
  Ignored,
}

/// Information about a line being rendered in the gutter
pub struct GutterLineInfo {
  /// Document line number (0-indexed)
  pub doc_line:          usize,
  /// Visual line number in viewport (0-indexed, relative to viewport top)
  pub visual_line:       usize,
  /// Whether this line is selected (cursor is on this line)
  pub selected:          bool,
  /// Whether this is the first visual line of a wrapped line
  pub first_visual_line: bool,
}

/// Trait for individual gutter components
pub trait Gutter: Send + Sync {
  /// Get the width of this gutter in columns
  fn width(&self, view: &View, doc: &Document) -> usize;

  /// Render a single line of the gutter
  ///
  /// Returns (text, color) to render, or None to render nothing
  fn render_line(
    &mut self,
    line_info: &GutterLineInfo,
    editor: &Editor,
    doc: &Document,
    view: &View,
    theme: &Theme,
  ) -> Option<(String, Color)>;

  /// Handle an event (e.g., mouse click on this gutter)
  ///
  /// Position is in gutter-local coordinates (column within this gutter's
  /// width)
  fn handle_event(
    &mut self,
    _event: &Event,
    _pos: Position,
    _editor: &mut Editor,
    _doc: &mut Document,
    _view: &View,
  ) -> GutterEventResult {
    GutterEventResult::Ignored
  }

  /// Whether this gutter is currently enabled
  fn is_enabled(&self) -> bool {
    true
  }

  /// Toggle this gutter on/off
  fn toggle(&mut self);

  /// Get a unique identifier for this gutter type
  fn id(&self) -> &'static str;

  /// Get a human-readable name
  fn name(&self) -> &'static str;
}

/// Manages a collection of gutters for an editor view
pub struct GutterManager {
  gutters: Vec<Box<dyn Gutter>>,
}

impl GutterManager {
  pub fn new() -> Self {
    Self {
      gutters: Vec::new(),
    }
  }

  /// Create the default gutter configuration
  pub fn with_defaults() -> Self {
    let mut manager = Self::new();
    manager.add_gutter(Box::new(AcpStateGutter::new()));      // ACP state (leftmost)
    manager.add_gutter(Box::new(DiagnosticGutter::new()));
    manager.add_gutter(Box::new(SpacerGutter::new()));
    manager.add_gutter(Box::new(LineNumberGutter::new()));
    manager.add_gutter(Box::new(SpacerGutter::new()));
    manager.add_gutter(Box::new(DiffGutter::new()));
    manager
  }

  /// Add a gutter to the manager
  pub fn add_gutter(&mut self, gutter: Box<dyn Gutter>) {
    self.gutters.push(gutter);
  }

  /// Remove a gutter by ID
  pub fn remove_gutter(&mut self, id: &str) -> Option<Box<dyn Gutter>> {
    let pos = self.gutters.iter().position(|g| g.id() == id)?;
    Some(self.gutters.remove(pos))
  }

  /// Toggle a gutter by ID
  pub fn toggle_gutter(&mut self, id: &str) -> bool {
    if let Some(gutter) = self.gutters.iter_mut().find(|g| g.id() == id) {
      gutter.toggle();
      true
    } else {
      false
    }
  }

  /// Get the total width of all enabled gutters
  pub fn total_width(&self, view: &View, doc: &Document) -> usize {
    self
      .gutters
      .iter()
      .filter(|g| g.is_enabled())
      .map(|g| g.width(view, doc))
      .sum()
  }

  /// Render all gutters for a specific line
  ///
  /// Returns the x offset after rendering (for positioning the next element)
  pub fn render_line(
    &mut self,
    line_info: &GutterLineInfo,
    editor: &Editor,
    doc: &Document,
    view: &View,
    theme: &Theme,
    surface: &mut Surface,
    base_x: f32,
    y: f32,
    font_width: f32,
    font_size: f32,
    _default_color: Color,
  ) -> f32 {
    let mut x_offset = base_x;

    for gutter in self.gutters.iter_mut().filter(|g| g.is_enabled()) {
      let width = gutter.width(view, doc);

      if let Some((text, color)) = gutter.render_line(line_info, editor, doc, view, theme) {
        // Right-align text within the gutter column
        let text_width = text.len();
        let padding = width.saturating_sub(text_width);
        let text_x = x_offset + (padding as f32 * font_width);

        surface.draw_text(TextSection::simple(text_x, y, text, font_size, color));
      }

      x_offset += (width as f32) * font_width;
    }

    x_offset
  }

  /// Handle an event, dispatching to the appropriate gutter
  pub fn handle_event(
    &mut self,
    event: &Event,
    pos: Position,
    editor: &mut Editor,
    doc: &mut Document,
    view: &View,
  ) -> GutterEventResult {
    let mut col_offset = 0;

    for gutter in self.gutters.iter_mut().filter(|g| g.is_enabled()) {
      let width = gutter.width(view, doc);

      // Check if the click is within this gutter's column range
      if pos.col >= col_offset && pos.col < col_offset + width {
        let local_pos = Position::new(pos.row, pos.col - col_offset);
        return gutter.handle_event(event, local_pos, editor, doc, view);
      }

      col_offset += width;
    }

    GutterEventResult::Ignored
  }

  /// Get a list of all gutters and their enabled state
  pub fn list_gutters(&self) -> Vec<(&'static str, &'static str, bool)> {
    self
      .gutters
      .iter()
      .map(|g| (g.id(), g.name(), g.is_enabled()))
      .collect()
  }
}

impl Default for GutterManager {
  fn default() -> Self {
    Self::with_defaults()
  }
}

// ========== Concrete Gutter Implementations ==========

/// Line numbers gutter
pub struct LineNumberGutter {
  enabled:   bool,
  min_width: usize,
}

impl LineNumberGutter {
  pub fn new() -> Self {
    Self {
      enabled:   true,
      min_width: 3,
    }
  }

  fn count_digits(n: usize) -> usize {
    (usize::checked_ilog10(n).unwrap_or(0) + 1) as usize
  }
}

impl Gutter for LineNumberGutter {
  fn width(&self, _view: &View, doc: &Document) -> usize {
    let text = doc.text();
    let last_line = text.len_lines().saturating_sub(1);
    let draw_last = text.line_to_byte(last_line) < text.len_bytes();
    let last_drawn = if draw_last { last_line + 1 } else { last_line };
    let digits = Self::count_digits(last_drawn);
    digits.max(self.min_width)
  }

  fn render_line(
    &mut self,
    line_info: &GutterLineInfo,
    _editor: &Editor,
    _doc: &Document,
    _view: &View,
    theme: &Theme,
  ) -> Option<(String, Color)> {
    if !line_info.first_visual_line {
      return None;
    }

    let display_num = line_info.doc_line + 1;

    // Get color from theme
    let color = if line_info.selected {
      theme
        .get("ui.linenr.selected")
        .fg
        .map(crate::ui::theme_color_to_renderer_color)
        .unwrap_or(Color::rgb(0.9, 0.9, 0.95))
    } else {
      theme
        .get("ui.linenr")
        .fg
        .map(crate::ui::theme_color_to_renderer_color)
        .unwrap_or(Color::rgb(0.5, 0.5, 0.6))
    };

    Some((
      format!("{:>width$}", display_num, width = self.width(_view, _doc)),
      color,
    ))
  }

  fn toggle(&mut self) {
    self.enabled = !self.enabled;
  }

  fn is_enabled(&self) -> bool {
    self.enabled
  }

  fn id(&self) -> &'static str {
    "line-numbers"
  }

  fn name(&self) -> &'static str {
    "Line Numbers"
  }
}

/// Diagnostics gutter (errors, warnings, hints)
pub struct DiagnosticGutter {
  enabled: bool,
}

impl DiagnosticGutter {
  pub fn new() -> Self {
    Self { enabled: true }
  }
}

impl Gutter for DiagnosticGutter {
  fn width(&self, _view: &View, _doc: &Document) -> usize {
    1
  }

  fn render_line(
    &mut self,
    line_info: &GutterLineInfo,
    _editor: &Editor,
    doc: &Document,
    _view: &View,
    theme: &Theme,
  ) -> Option<(String, Color)> {
    if !line_info.first_visual_line {
      return None;
    }

    // Check if there are diagnostics on this line
    let diagnostics = &doc.diagnostics;
    let first_diag_idx = diagnostics.partition_point(|d| d.line < line_info.doc_line);

    let diag = diagnostics
      .get(first_diag_idx..)
      .and_then(|diags| diags.iter().find(|d| d.line == line_info.doc_line))?;

    use crate::core::diagnostics::Severity;
    let (symbol, color) = match diag.severity {
      Some(Severity::Error) => {
        (
          "●",
          theme
            .get("error")
            .fg
            .map(crate::ui::theme_color_to_renderer_color)
            .unwrap_or(Color::rgb(0.9, 0.3, 0.3)),
        )
      },
      Some(Severity::Warning) | None => {
        (
          "●",
          theme
            .get("warning")
            .fg
            .map(crate::ui::theme_color_to_renderer_color)
            .unwrap_or(Color::rgb(0.9, 0.7, 0.3)),
        )
      },
      Some(Severity::Info) => {
        (
          "●",
          theme
            .get("info")
            .fg
            .map(crate::ui::theme_color_to_renderer_color)
            .unwrap_or(Color::rgb(0.3, 0.7, 0.9)),
        )
      },
      Some(Severity::Hint) => {
        (
          "●",
          theme
            .get("hint")
            .fg
            .map(crate::ui::theme_color_to_renderer_color)
            .unwrap_or(Color::rgb(0.5, 0.8, 0.5)),
        )
      },
    };

    Some((symbol.to_string(), color))
  }

  fn toggle(&mut self) {
    self.enabled = !self.enabled;
  }

  fn is_enabled(&self) -> bool {
    self.enabled
  }

  fn id(&self) -> &'static str {
    "diagnostics"
  }

  fn name(&self) -> &'static str {
    "Diagnostics"
  }
}

/// Diff gutter (shows git changes)
pub struct DiffGutter {
  enabled: bool,
}

impl DiffGutter {
  pub fn new() -> Self {
    Self { enabled: true }
  }
}

impl Gutter for DiffGutter {
  fn width(&self, _view: &View, _doc: &Document) -> usize {
    1
  }

  fn render_line(
    &mut self,
    line_info: &GutterLineInfo,
    _editor: &Editor,
    doc: &Document,
    _view: &View,
    theme: &Theme,
  ) -> Option<(String, Color)> {
    let diff_handle = doc.diff_handle()?;
    let hunks = diff_handle.load();

    if hunks.is_empty() {
      return None;
    }

    // Helper functions for hunk types
    let is_pure_insertion = |hunk: &the_editor_vcs::Hunk| hunk.before.start == hunk.before.end;
    let is_pure_removal = |hunk: &the_editor_vcs::Hunk| hunk.after.start == hunk.after.end;

    // Find the hunk for this line
    let line = line_info.doc_line as u32;
    let mut hunk_i = 0;
    let mut hunk = hunks.nth_hunk(hunk_i);

    // Advance to the right hunk
    while hunk.after.end < line || (!is_pure_removal(&hunk) && line == hunk.after.end) {
      hunk_i += 1;
      hunk = hunks.nth_hunk(hunk_i);
      if hunk == the_editor_vcs::Hunk::NONE {
        return None;
      }
    }

    // Check if line is outside this hunk
    if hunk.after.start > line {
      return None;
    }

    // Determine the symbol and color
    let (symbol, color) = if is_pure_insertion(&hunk) {
      (
        "▍",
        theme
          .get("diff.plus.gutter")
          .fg
          .or(theme.get("diff.plus").fg)
          .map(crate::ui::theme_color_to_renderer_color)
          .unwrap_or(Color::rgb(0.3, 0.8, 0.3)),
      )
    } else if is_pure_removal(&hunk) {
      if !line_info.first_visual_line {
        return None;
      }
      (
        "▔",
        theme
          .get("diff.minus.gutter")
          .fg
          .or(theme.get("diff.minus").fg)
          .map(crate::ui::theme_color_to_renderer_color)
          .unwrap_or(Color::rgb(0.9, 0.3, 0.3)),
      )
    } else {
      (
        "▍",
        theme
          .get("diff.delta.gutter")
          .fg
          .or(theme.get("diff.delta").fg)
          .map(crate::ui::theme_color_to_renderer_color)
          .unwrap_or(Color::rgb(0.7, 0.7, 0.3)),
      )
    };

    Some((symbol.to_string(), color))
  }

  fn toggle(&mut self) {
    self.enabled = !self.enabled;
  }

  fn is_enabled(&self) -> bool {
    self.enabled
  }

  fn id(&self) -> &'static str {
    "diff"
  }

  fn name(&self) -> &'static str {
    "Diff"
  }
}

/// Spacer gutter (just adds blank space)
pub struct SpacerGutter {
  enabled: bool,
}

impl SpacerGutter {
  pub fn new() -> Self {
    Self { enabled: true }
  }
}

impl Gutter for SpacerGutter {
  fn width(&self, _view: &View, _doc: &Document) -> usize {
    1
  }

  fn render_line(
    &mut self,
    _line_info: &GutterLineInfo,
    _editor: &Editor,
    _doc: &Document,
    _view: &View,
    _theme: &Theme,
  ) -> Option<(String, Color)> {
    None // Spacer renders nothing
  }

  fn toggle(&mut self) {
    self.enabled = !self.enabled;
  }

  fn is_enabled(&self) -> bool {
    self.enabled
  }

  fn id(&self) -> &'static str {
    "spacer"
  }

  fn name(&self) -> &'static str {
    "Spacer"
  }
}

// ACP State Gutter - shows emoji indicators for ACP session state
pub struct AcpStateGutter {
  enabled: bool,
}

impl AcpStateGutter {
  pub fn new() -> Self {
    Self { enabled: true }
  }
}

impl Gutter for AcpStateGutter {
  fn width(&self, _view: &View, _doc: &Document) -> usize {
    2  // Emoji width (some emojis are double-width)
  }

  fn render_line(
    &mut self,
    line_info: &GutterLineInfo,
    _editor: &Editor,
    doc: &Document,
    _view: &View,
    theme: &Theme,
  ) -> Option<(String, Color)> {
    // Only render on ACP buffers
    if !doc.is_acp_buffer {
      return None;
    }

    // Only render on first visual line of wrapped lines
    if !line_info.first_visual_line {
      return None;
    }

    // Get cached session state from document
    let state = doc.acp_gutter_state.as_ref()?;

    // Only show emoji on the current line where activity is happening
    if state.current_line != Some(line_info.doc_line) {
      return None;
    }

    // Map session state to emoji and theme key
    use crate::acp::session::SessionState;
    let (emoji, theme_key) = match state.state {
      SessionState::Thinking => ("⏳", "acp.gutter.thinking"),
      SessionState::Streaming => ("✍", "acp.gutter.streaming"),
      SessionState::ExecutingTool => ("🔧", "acp.gutter.tool"),
      SessionState::Idle => return None,  // Don't show anything when idle
    };

    // Get color from theme with fallback
    let color = theme
      .try_get(theme_key)
      .and_then(|s| s.fg)
      .map(crate::ui::theme_color_to_renderer_color)
      .unwrap_or(Color::rgb(0.8, 0.8, 0.8));

    Some((emoji.to_string(), color))
  }

  fn is_enabled(&self) -> bool {
    self.enabled
  }

  fn toggle(&mut self) {
    self.enabled = !self.enabled;
  }

  fn id(&self) -> &'static str {
    "acp-state"
  }

  fn name(&self) -> &'static str {
    "ACP State"
  }

  fn handle_event(
    &mut self,
    _event: &Event,
    _pos: Position,
    _editor: &mut Editor,
    _doc: &mut Document,
    _view: &View,
  ) -> GutterEventResult {
    GutterEventResult::Ignored
  }
}
