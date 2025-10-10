use the_editor_renderer::Color;

use crate::{
  core::{
    diagnostics::{
      Diagnostic,
      Severity,
    },
    doc_formatter::FormattedGrapheme,
    document::Document,
  },
  theme::Theme,
  ui::{
    compositor::Surface,
    text_decorations::Decoration,
  },
};

/// Decoration for rendering underlines beneath diagnostic ranges
pub struct DiagnosticUnderlines<'a> {
  diagnostics:       &'a [Diagnostic],
  diagnostic_idx:    usize,
  current_underline: Option<(usize, usize, Severity)>, // (start_col, end_col, severity)
  base_x:            f32,
  base_y:            f32,
  line_height:       f32,
  font_width:        f32,
  font_size:         f32,
  horizontal_offset: usize,
  hint_color:        Color,
  info_color:        Color,
  warning_color:     Color,
  error_color:       Color,
}

impl<'a> DiagnosticUnderlines<'a> {
  pub fn new(
    doc: &'a Document,
    theme: &Theme,
    base_x: f32,
    base_y: f32,
    line_height: f32,
    font_width: f32,
    font_size: f32,
    horizontal_offset: usize,
  ) -> Self {
    // Get colors from theme
    let hint_style = theme.get("diagnostic.hint");
    let info_style = theme.get("diagnostic.info");
    let warning_style = theme.get("diagnostic.warning");
    let error_style = theme.get("diagnostic.error");

    DiagnosticUnderlines {
      diagnostics: &doc.diagnostics,
      diagnostic_idx: 0,
      current_underline: None,
      base_x,
      base_y,
      line_height,
      font_width,
      font_size,
      horizontal_offset,
      hint_color: hint_style
        .underline_color
        .or(hint_style.fg)
        .map(crate::ui::theme_color_to_renderer_color)
        .unwrap_or(Color::rgb(0.5, 0.5, 0.5)),
      info_color: info_style
        .underline_color
        .or(info_style.fg)
        .map(crate::ui::theme_color_to_renderer_color)
        .unwrap_or(Color::rgb(0.3, 0.6, 1.0)),
      warning_color: warning_style
        .underline_color
        .or(warning_style.fg)
        .map(crate::ui::theme_color_to_renderer_color)
        .unwrap_or(Color::rgb(1.0, 0.8, 0.0)),
      error_color: error_style
        .underline_color
        .or(error_style.fg)
        .map(crate::ui::theme_color_to_renderer_color)
        .unwrap_or(Color::rgb(1.0, 0.3, 0.3)),
    }
  }

  fn severity_color(&self, severity: Severity) -> Color {
    match severity {
      Severity::Hint => self.hint_color,
      Severity::Info => self.info_color,
      Severity::Warning => self.warning_color,
      Severity::Error => self.error_color,
    }
  }

  fn draw_underline(
    &self,
    surface: &mut Surface,
    start_col: usize,
    end_col: usize,
    row: u16,
    severity: Severity,
  ) {
    if start_col >= end_col {
      return; // No underline to draw
    }

    let color = self.severity_color(severity);

    // Y position: place the underline character at the same baseline as the text
    // The text rendering system will position the "▁" character correctly relative
    // to baseline
    let base_y_for_line = self.base_y + (row as f32) * self.line_height;

    // Draw individual "▁" characters for each column, matching how text is rendered
    // This ensures perfect alignment since we're using the same font rendering
    // system
    for col in start_col..end_col {
      let x = self.base_x + (col as f32) * self.font_width;
      surface.draw_decoration_grapheme("▁", color, x, base_y_for_line);
    }
  }
}

impl Decoration for DiagnosticUnderlines<'_> {
  fn reset_pos(&mut self, pos: usize) -> usize {
    self.diagnostic_idx = self
      .diagnostics
      .partition_point(|diag| diag.range.start < pos);
    self.current_underline = None;

    // Return next diagnostic start position
    self
      .diagnostics
      .get(self.diagnostic_idx)
      .map(|diag| diag.range.start)
      .unwrap_or(usize::MAX)
  }

  fn skip_concealed_anchor(&mut self, conceal_end_char_idx: usize) -> usize {
    self.reset_pos(conceal_end_char_idx)
  }

  fn decorate_grapheme(&mut self, grapheme: &FormattedGrapheme) -> usize {
    let char_idx = grapheme.char_idx;

    // Check if we're entering a new diagnostic range
    while self.diagnostic_idx < self.diagnostics.len() {
      let diag = &self.diagnostics[self.diagnostic_idx];

      // Skip invalid/empty ranges to prevent infinite loops
      if diag.range.start >= diag.range.end {
        self.diagnostic_idx += 1;
        continue;
      }

      if char_idx < diag.range.start {
        break;
      }

      if char_idx >= diag.range.start && char_idx < diag.range.end {
        // We're inside a diagnostic range
        if let Some((_, end_col, current_severity)) = &mut self.current_underline {
          // Update the end column
          let vis_col = grapheme
            .visual_pos
            .col
            .saturating_sub(self.horizontal_offset);
          *end_col = vis_col + grapheme.raw.width();
          *current_severity = (*current_severity).max(diag.severity());
        } else {
          // Start a new underline
          let vis_col = grapheme
            .visual_pos
            .col
            .saturating_sub(self.horizontal_offset);
          self.current_underline = Some((vis_col, vis_col + grapheme.raw.width(), diag.severity()));
        }

        // Check if we're at the end of this diagnostic
        if char_idx + 1 >= diag.range.end {
          self.diagnostic_idx += 1;
          // Fall through to return next diagnostic start
        } else {
          // Still in the middle of this diagnostic, call us again for the next character
          return char_idx + 1;
        }

        break;
      } else if char_idx >= diag.range.end {
        // We've passed this diagnostic
        self.diagnostic_idx += 1;
      }
    }

    // Return next diagnostic start
    self
      .diagnostics
      .get(self.diagnostic_idx)
      .map(|diag| diag.range.start)
      .unwrap_or(usize::MAX)
  }

  fn render_virt_lines(
    &mut self,
    surface: &mut Surface,
    pos: (usize, u16),
    _virt_off: crate::core::position::Position,
  ) -> crate::core::position::Position {
    // Draw any pending underline at the end of the line
    if let Some((start_col, end_col, severity)) = self.current_underline.take() {
      self.draw_underline(surface, start_col, end_col, pos.1, severity);
    }

    // Don't consume any virtual lines
    crate::core::position::Position::new(0, 0)
  }
}
