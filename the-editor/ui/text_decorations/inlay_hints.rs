use the_editor_renderer::Color;

use crate::{
  core::{
    doc_formatter::FormattedGrapheme, document::DocumentInlayHints, position::Position,
    text_annotations::InlineAnnotation,
  },
  theme::Theme,
  ui::{UI_FONT_WIDTH, compositor::Surface, text_decorations::Decoration},
};

/// Inlay hints decoration that shows LSP inlay hints aligned to the right
/// Only displays hints when the cursor is positioned exactly at the hint's
/// anchor character
pub struct InlayHints<'a> {
  // All inlay hint annotations
  type_hints: &'a [InlineAnnotation],
  parameter_hints: &'a [InlineAnnotation],
  other_hints: &'a [InlineAnnotation],

  // Styling
  type_color: Color,
  parameter_color: Color,
  other_color: Color,

  // Cursor position for filtering
  cursor_pos: usize,

  // Rendering state
  base_x: f32,
  base_y: f32,
  line_height: f32,
  viewport_width: u16,

  // Track current position in hint arrays
  type_idx: usize,
  parameter_idx: usize,
  other_idx: usize,

  // Pending hints to render at end of line
  pending_hints: Vec<(String, u16, Color)>, // (text, col, color)
  current_doc_line: usize,
  last_col: u16, // Track the rightmost column on current line
}

impl<'a> InlayHints<'a> {
  pub fn new(
    hints: &'a DocumentInlayHints,
    theme: &Theme,
    cursor_pos: usize,
    viewport_width: u16,
    base_x: f32,
    base_y: f32,
    line_height: f32,
  ) -> Self {
    // Get colors from theme
    let type_style = theme
      .try_get("ui.virtual.inlay-hint.type")
      .or_else(|| theme.try_get("ui.virtual.inlay-hint"))
      .unwrap_or_else(|| theme.get("ui.text"));
    let parameter_style = theme
      .try_get("ui.virtual.inlay-hint.parameter")
      .or_else(|| theme.try_get("ui.virtual.inlay-hint"))
      .unwrap_or_else(|| theme.get("ui.text"));
    let other_style = theme
      .try_get("ui.virtual.inlay-hint")
      .unwrap_or_else(|| theme.get("ui.text"));

    let type_color = type_style
      .fg
      .map(crate::ui::theme_color_to_renderer_color)
      .unwrap_or(Color::rgba(0.6, 0.6, 0.6, 0.8));
    let parameter_color = parameter_style
      .fg
      .map(crate::ui::theme_color_to_renderer_color)
      .unwrap_or(Color::rgba(0.6, 0.6, 0.6, 0.8));
    let other_color = other_style
      .fg
      .map(crate::ui::theme_color_to_renderer_color)
      .unwrap_or(Color::rgba(0.6, 0.6, 0.6, 0.8));

    InlayHints {
      type_hints: &hints.type_inlay_hints,
      parameter_hints: &hints.parameter_inlay_hints,
      other_hints: &hints.other_inlay_hints,
      type_color,
      parameter_color,
      other_color,
      cursor_pos,
      base_x,
      base_y,
      line_height,
      viewport_width,
      type_idx: 0,
      parameter_idx: 0,
      other_idx: 0,
      pending_hints: Vec::new(),
      current_doc_line: 0,
      last_col: 0,
    }
  }

  /// Check all hint types for hints at the given character position
  /// If cursor is at this position, add to pending hints for rendering
  fn check_hints_at(&mut self, char_idx: usize, col: u16) {
    // Check type hints
    while self.type_idx < self.type_hints.len() {
      let hint = &self.type_hints[self.type_idx];
      if hint.char_idx < char_idx {
        self.type_idx += 1;
        continue;
      }
      if hint.char_idx > char_idx {
        break;
      }

      // Hint is at this position - always show it
      self
        .pending_hints
        .push((hint.text.to_string(), col, self.type_color));

      self.type_idx += 1;
    }

    // Check parameter hints
    while self.parameter_idx < self.parameter_hints.len() {
      let hint = &self.parameter_hints[self.parameter_idx];
      if hint.char_idx < char_idx {
        self.parameter_idx += 1;
        continue;
      }
      if hint.char_idx > char_idx {
        break;
      }

      self
        .pending_hints
        .push((hint.text.to_string(), col, self.parameter_color));

      self.parameter_idx += 1;
    }

    // Check other hints
    while self.other_idx < self.other_hints.len() {
      let hint = &self.other_hints[self.other_idx];
      if hint.char_idx < char_idx {
        self.other_idx += 1;
        continue;
      }
      if hint.char_idx > char_idx {
        break;
      }

      self
        .pending_hints
        .push((hint.text.to_string(), col, self.other_color));

      self.other_idx += 1;
    }
  }

  /// Render pending hints at the end of the current line
  fn render_pending_hints(&mut self, surface: &mut Surface, row: u16, line_end_col: u16) {
    if self.pending_hints.is_empty() {
      return;
    }

    // Calculate starting position - one column after line content
    let mut draw_col = line_end_col + 1;

    for (hint_text, _original_col, color) in &self.pending_hints {
      let hint_width = hint_text.chars().count() as u16;
      let available_width = self.viewport_width.saturating_sub(draw_col);

      if available_width > 0 {
        let x = self.base_x + (draw_col as f32) * UI_FONT_WIDTH;
        let y = self.base_y + (row as f32) * self.line_height;

        let chars_to_render = hint_width.min(available_width);
        let display_text: String = hint_text.chars().take(chars_to_render as usize).collect();

        surface.draw_decoration_grapheme(&display_text, *color, x, y);

        draw_col += chars_to_render + 1; // Add 1 for spacing between hints
      }

      if draw_col >= self.viewport_width {
        break;
      }
    }

    self.pending_hints.clear();
  }
}

impl Decoration for InlayHints<'_> {
  fn reset_pos(&mut self, pos: usize) -> usize {
    // Reset indices to start of arrays
    self.type_idx = 0;
    self.parameter_idx = 0;
    self.other_idx = 0;
    self.pending_hints.clear();
    self.current_doc_line = 0;
    self.last_col = 0;

    // Find first hint at or after pos
    let mut next_pos = usize::MAX;

    if let Some(hint) = self.type_hints.first() {
      if hint.char_idx >= pos {
        next_pos = next_pos.min(hint.char_idx);
      }
    }
    if let Some(hint) = self.parameter_hints.first() {
      if hint.char_idx >= pos {
        next_pos = next_pos.min(hint.char_idx);
      }
    }
    if let Some(hint) = self.other_hints.first() {
      if hint.char_idx >= pos {
        next_pos = next_pos.min(hint.char_idx);
      }
    }

    next_pos
  }

  fn skip_concealed_anchor(&mut self, conceal_end_char_idx: usize) -> usize {
    // Skip any hints that were concealed
    while self.type_idx < self.type_hints.len()
      && self.type_hints[self.type_idx].char_idx < conceal_end_char_idx
    {
      self.type_idx += 1;
    }
    while self.parameter_idx < self.parameter_hints.len()
      && self.parameter_hints[self.parameter_idx].char_idx < conceal_end_char_idx
    {
      self.parameter_idx += 1;
    }
    while self.other_idx < self.other_hints.len()
      && self.other_hints[self.other_idx].char_idx < conceal_end_char_idx
    {
      self.other_idx += 1;
    }

    self.reset_pos(conceal_end_char_idx)
  }

  fn decorate_grapheme(&mut self, grapheme: &FormattedGrapheme) -> usize {
    // Check if there are any hints at this character position
    self.check_hints_at(grapheme.char_idx, grapheme.visual_pos.col as u16);

    // Track the rightmost column position for this line
    let grapheme_width = grapheme.raw.width() as u16;
    let end_col = grapheme.visual_pos.col as u16 + grapheme_width;
    self.last_col = self.last_col.max(end_col);

    // Find next position where we should be called
    let mut next_pos = usize::MAX;

    if self.type_idx < self.type_hints.len() {
      next_pos = next_pos.min(self.type_hints[self.type_idx].char_idx);
    }
    if self.parameter_idx < self.parameter_hints.len() {
      next_pos = next_pos.min(self.parameter_hints[self.parameter_idx].char_idx);
    }
    if self.other_idx < self.other_hints.len() {
      next_pos = next_pos.min(self.other_hints[self.other_idx].char_idx);
    }

    next_pos
  }

  fn decorate_line(&mut self, surface: &mut Surface, pos: (usize, u16)) {
    let (doc_line, visual_line) = pos;

    // If switching to a new document line, render any pending hints from previous
    // line
    if doc_line != self.current_doc_line && !self.pending_hints.is_empty() {
      self.render_pending_hints(surface, visual_line.saturating_sub(1), self.last_col);
    }

    self.current_doc_line = doc_line;
    self.last_col = 0; // Reset for new line
  }

  fn render_virt_lines(
    &mut self,
    surface: &mut Surface,
    pos: (usize, u16),
    _virt_off: Position,
  ) -> Position {
    // Render any remaining pending hints at the end of this line
    let (_doc_line, visual_line) = pos;
    if !self.pending_hints.is_empty() {
      self.render_pending_hints(surface, visual_line, self.last_col);
    }

    // Inlay hints don't create virtual lines - they're rendered inline at line end
    Position::new(0, 0)
  }
}
