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
  opacities:         &'a std::collections::HashMap<usize, f32>,
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
    opacities: &'a std::collections::HashMap<usize, f32>,
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
      opacities,
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
    doc_line: usize,
  ) {
    if start_col >= end_col {
      return;
    }

    let mut color = self.severity_color(severity);

    // Apply animation opacity
    if let Some(&opacity) = self.opacities.get(&doc_line) {
      color.a *= opacity;
    }

    // Scale wave parameters based on font size (baseline: 16px font)
    let scale = self.font_size / 16.0;

    // Position wave at bottom of the text line
    let y_base = self.base_y + (row as f32 + 1.0) * self.line_height - (3.0 * scale);

    // Wave parameters scaled to font size
    let wave_amplitude = 1.5 * scale; // Height of wave peaks
    let wave_period = 6.0 * scale; // Pixels per full wave cycle
    let point_size = 1.0 * scale; // Size of each drawn point

    // Draw wave from start to end column
    let start_x = self.base_x + (start_col as f32) * self.font_width;
    let end_x = self.base_x + (end_col as f32) * self.font_width;

    let mut px = start_x;
    while px < end_x {
      let wave_y = y_base + (px / wave_period * std::f32::consts::TAU).sin() * wave_amplitude;
      surface.draw_rect(px, wave_y, point_size, point_size, color);
      px += point_size;
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
    // pos.0 is the document line, pos.1 is the visual row
    if let Some((start_col, end_col, severity)) = self.current_underline.take() {
      self.draw_underline(surface, start_col, end_col, pos.1, severity, pos.0);
    }

    // Don't consume any virtual lines
    crate::core::position::Position::new(0, 0)
  }
}
