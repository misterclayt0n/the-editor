use the_editor_renderer::Color;

use crate::{
  core::{
    diagnostics::{Diagnostic, InlineDiagnosticAccumulator, InlineDiagnosticsConfig, Severity},
    doc_formatter::{DocumentFormatter, FormattedGrapheme},
    document::Document,
    position::Position,
    text_annotations::TextAnnotations,
  },
  theme::Theme,
  ui::{
    compositor::Surface,
    text_decorations::{Decoration, DecorationRenderer},
  },
};

/// Box-drawing characters for diagnostic rendering
const BL_CORNER: &str = "┘";
const TR_CORNER: &str = "┌";
const BR_CORNER: &str = "└";
const STACK: &str = "├";
const MULTI: &str = "┴";
const HOR_BAR: &str = "─";
const VER_BAR: &str = "│";

/// Styles for different diagnostic severities
#[derive(Debug)]
struct Styles {
  hint: Color,
  info: Color,
  warning: Color,
  error: Color,
}

impl Styles {
  fn new(theme: &Theme) -> Styles {
    let hint_style = theme.get("hint");
    let info_style = theme.get("info");
    let warning_style = theme.get("warning");
    let error_style = theme.get("error");

    Styles {
      hint: hint_style
        .fg
        .map(crate::ui::theme_color_to_renderer_color)
        .unwrap_or(Color::rgb(0.5, 0.5, 0.5)),
      info: info_style
        .fg
        .map(crate::ui::theme_color_to_renderer_color)
        .unwrap_or(Color::rgb(0.3, 0.6, 1.0)),
      warning: warning_style
        .fg
        .map(crate::ui::theme_color_to_renderer_color)
        .unwrap_or(Color::rgb(1.0, 0.8, 0.0)),
      error: error_style
        .fg
        .map(crate::ui::theme_color_to_renderer_color)
        .unwrap_or(Color::rgb(1.0, 0.3, 0.3)),
    }
  }

  fn severity_style(&self, severity: Severity) -> Color {
    match severity {
      Severity::Hint => self.hint,
      Severity::Info => self.info,
      Severity::Warning => self.warning,
      Severity::Error => self.error,
    }
  }
}

/// Inline diagnostics decoration for rendering diagnostic messages
pub struct InlineDiagnostics<'a> {
  state: InlineDiagnosticAccumulator<'a>,
  eol_diagnostics: crate::core::diagnostics::DiagnosticFilter,
  eol_cursor_line_only: bool,
  eol_opacities: &'a std::collections::HashMap<usize, f32>,
  inline_anim_states: &'a std::collections::HashMap<
    usize,
    crate::ui::inline_diagnostic_animation::InlineDiagnosticAnimState,
  >,
  cursor_line: usize,
  styles: Styles,
  base_x: f32,
  base_y: f32,
  line_height: f32,
  font_width: f32,
  font_size: f32,
  viewport_width: u16,
  horizontal_offset: usize,
}

impl<'a> InlineDiagnostics<'a> {
  pub fn new(
    doc: &'a Document,
    theme: &Theme,
    cursor: usize,
    cursor_line: usize,
    config: InlineDiagnosticsConfig,
    eol_diagnostics: crate::core::diagnostics::DiagnosticFilter,
    eol_cursor_line_only: bool,
    eol_opacities: &'a std::collections::HashMap<usize, f32>,
    inline_anim_states: &'a std::collections::HashMap<
      usize,
      crate::ui::inline_diagnostic_animation::InlineDiagnosticAnimState,
    >,
    base_x: f32,
    base_y: f32,
    line_height: f32,
    font_width: f32,
    font_size: f32,
    viewport_width: u16,
    horizontal_offset: usize,
  ) -> Self {
    InlineDiagnostics {
      state: InlineDiagnosticAccumulator::new(cursor, doc, config),
      eol_diagnostics,
      eol_cursor_line_only,
      eol_opacities,
      inline_anim_states,
      cursor_line,
      styles: Styles::new(theme),
      base_x,
      base_y,
      line_height,
      font_width,
      font_size,
      viewport_width,
      horizontal_offset,
    }
  }

  /// Get animation state for a document line
  /// Returns None if line has no animation state (still in debounce or not
  /// tracked)
  fn get_anim_state(
    &self,
    doc_line: usize,
  ) -> Option<crate::ui::inline_diagnostic_animation::InlineDiagnosticAnimState> {
    self.inline_anim_states.get(&doc_line).copied()
  }

  /// Draw a single decoration grapheme at the specified column and row (no
  /// animation)
  fn draw_decoration(
    &self,
    surface: &mut Surface,
    g: &'static str,
    severity: Severity,
    col: u16,
    row: u16,
  ) {
    let x = self.base_x + (col as f32) * self.font_width;
    let y = self.base_y + (row as f32) * self.line_height;
    let color = self.styles.severity_style(severity);
    surface.draw_decoration_grapheme(g, color, x, y);
  }

  /// Draw a single decoration grapheme with animation applied
  fn draw_decoration_with_anim(
    &self,
    surface: &mut Surface,
    g: &'static str,
    severity: Severity,
    col: u16,
    row: u16,
    anim: &crate::ui::inline_diagnostic_animation::InlineDiagnosticAnimState,
  ) {
    let x = self.base_x + (col as f32) * self.font_width;
    // Apply vertical slide offset
    let y = self.base_y + ((row as f32) - anim.slide_offset) * self.line_height;
    let mut color = self.styles.severity_style(severity);
    color.a *= anim.opacity;
    surface.draw_decoration_grapheme(g, color, x, y);
  }

  /// Draw end-of-line diagnostic (message at line end, potentially multi-line)
  fn draw_eol_diagnostic(
    &self,
    surface: &mut Surface,
    diag: &Diagnostic,
    doc_line: usize,
    row: u16,
    line_end_col: usize,
  ) -> u16 {
    if self.viewport_width == 0 {
      return 0;
    }

    // Get animation opacity - if not in map, don't render (still in debounce
    // period)
    let Some(opacity) = self.eol_opacities.get(&doc_line).copied() else {
      return 0;
    };

    let viewport_width = self.viewport_width as usize;
    let start_col_in_view = line_end_col.saturating_sub(self.horizontal_offset);
    if start_col_in_view >= viewport_width {
      return 0;
    }

    let mut draw_col = start_col_in_view.saturating_add(1);
    if draw_col >= viewport_width {
      return 0;
    }

    // Apply opacity to color
    let mut color = self.styles.severity_style(diag.severity());
    color.a *= opacity;

    let mut end_col = start_col_in_view as u16;

    for line in diag.message.lines() {
      if draw_col >= viewport_width {
        break;
      }

      let available_width = viewport_width - draw_col;
      if available_width == 0 {
        break;
      }

      let x = self.base_x + (draw_col as f32) * self.font_width;
      let y = self.base_y + (row as f32) * self.line_height;

      let chars_drawn = surface.draw_truncated_text_with_font_size(
        line,
        x,
        y,
        available_width,
        color,
        self.font_size,
      );
      if chars_drawn == 0 {
        break;
      }

      end_col = (draw_col + chars_drawn) as u16;
      draw_col = draw_col.saturating_add(chars_drawn + 2);
    }

    end_col.saturating_sub(start_col_in_view as u16)
  }

  /// Draw a full diagnostic message with box-drawing in virtual lines (with
  /// animation)
  fn draw_diagnostic(
    &self,
    surface: &mut Surface,
    diag: &Diagnostic,
    col: u16,
    current_row: &mut u16,
    next_severity: Option<Severity>,
    anim: &crate::ui::inline_diagnostic_animation::InlineDiagnosticAnimState,
  ) {
    let severity = diag.severity();
    let (sym, sym_severity) = if let Some(next_severity) = next_severity {
      (STACK, next_severity.max(severity))
    } else {
      (BR_CORNER, severity)
    };

    // Draw corner and horizontal bar with animation
    self.draw_decoration_with_anim(surface, sym, sym_severity, col, *current_row, anim);

    // Draw horizontal bars
    for i in 0..self.state.config.prefix_len {
      self.draw_decoration_with_anim(surface, HOR_BAR, severity, col + i + 1, *current_row, anim);
    }

    // Draw diagnostic message text
    let text_col = col + self.state.config.prefix_len + 1;
    let text_fmt = self.state.config.text_fmt(text_col, self.viewport_width);
    let annotations = TextAnnotations::default();

    let formatter = DocumentFormatter::new_at_prev_checkpoint(
      diag.message.as_str().trim().into(),
      &text_fmt,
      &annotations,
      0,
    );

    let mut color = self.styles.severity_style(severity);
    color.a *= anim.opacity;
    let mut last_row = 0;

    for grapheme in formatter {
      last_row = grapheme.visual_pos.row;
      let x = self.base_x + ((text_col + grapheme.visual_pos.col as u16) as f32) * self.font_width;
      // Apply slide offset to Y coordinate
      let y = self.base_y
        + ((*current_row + grapheme.visual_pos.row as u16) as f32 - anim.slide_offset)
          * self.line_height;

      // Convert grapheme to string for rendering
      let grapheme_str = match &grapheme.raw {
        crate::core::grapheme::Grapheme::Newline => continue,
        crate::core::grapheme::Grapheme::Tab { .. } => " ",
        crate::core::grapheme::Grapheme::Other { g } => g.as_ref(),
      };

      surface.draw_decoration_grapheme(grapheme_str, color, x, y);
    }

    *current_row += 1;

    // Draw vertical bars for additional lines if there's a next diagnostic
    let extra_lines = last_row;
    if let Some(next_severity) = next_severity {
      for _ in 0..extra_lines {
        self.draw_decoration_with_anim(surface, VER_BAR, next_severity, col, *current_row, anim);
        *current_row += 1;
      }
    } else {
      *current_row += extra_lines as u16;
    }
  }

  /// Draw multiple diagnostics stacked together (with animation)
  fn draw_multi_diagnostics(
    &self,
    surface: &mut Surface,
    stack: &mut Vec<(&Diagnostic, u16)>,
    row: &mut u16,
    anim: &crate::ui::inline_diagnostic_animation::InlineDiagnosticAnimState,
  ) {
    let Some(&(last_diag, last_anchor)) = stack.last() else {
      return;
    };

    let start = self.state.config.max_diagnostic_start(self.viewport_width);

    if last_anchor <= start {
      return;
    }

    let mut severity = last_diag.severity();
    let mut last_anchor = last_anchor;

    self.draw_decoration_with_anim(surface, BL_CORNER, severity, last_anchor, *row, anim);

    let mut stacked_diagnostics = 1;
    for &(diag, anchor) in stack.iter().rev().skip(1) {
      use std::cmp::Ordering;
      let sym = match anchor.cmp(&start) {
        Ordering::Less => break,
        Ordering::Equal => STACK,
        Ordering::Greater => MULTI,
      };

      stacked_diagnostics += 1;
      severity = severity.max(diag.severity());
      let old_severity = severity;

      if anchor == last_anchor && severity == old_severity {
        continue;
      }

      // Draw horizontal bars
      for col in (anchor + 1)..last_anchor {
        self.draw_decoration_with_anim(surface, HOR_BAR, old_severity, col, *row, anim);
      }

      self.draw_decoration_with_anim(surface, sym, severity, anchor, *row, anim);
      last_anchor = anchor;
    }

    // Draw the connecting line to start position
    if last_anchor != start {
      for col in (start + 1)..last_anchor {
        self.draw_decoration_with_anim(surface, HOR_BAR, severity, col, *row, anim);
      }
      self.draw_decoration_with_anim(surface, TR_CORNER, severity, start, *row, anim);
    }

    *row += 1;

    // Draw all stacked diagnostics
    let stacked_diagnostics = &stack[stack.len() - stacked_diagnostics..];

    for (i, (diag, _)) in stacked_diagnostics.iter().rev().enumerate() {
      let next_severity = stacked_diagnostics[..stacked_diagnostics.len() - i - 1]
        .iter()
        .map(|(diag, _)| diag.severity())
        .max();
      self.draw_diagnostic(surface, diag, start, row, next_severity, anim);
    }

    stack.truncate(stack.len() - stacked_diagnostics.len());
  }

  /// Draw all diagnostics in the stack (with animation)
  fn draw_diagnostics(
    &self,
    surface: &mut Surface,
    stack: &mut Vec<(&Diagnostic, u16)>,
    first_row: u16,
    current_row: &mut u16,
    anim: &crate::ui::inline_diagnostic_animation::InlineDiagnosticAnimState,
  ) {
    let mut stack_iter = stack.drain(..).rev().peekable();
    let mut last_anchor = self.viewport_width;

    while let Some((diag, anchor)) = stack_iter.next() {
      if anchor != last_anchor {
        // Draw vertical bars from first row to current row
        for row in first_row..*current_row {
          self.draw_decoration_with_anim(surface, VER_BAR, diag.severity(), anchor, row, anim);
        }
      }

      let next_severity = stack_iter
        .peek()
        .and_then(|&(diag, next_anchor)| (next_anchor == anchor).then_some(diag.severity()));

      self.draw_diagnostic(surface, diag, anchor, current_row, next_severity, anim);
      last_anchor = anchor;
    }
  }
}

impl Decoration for InlineDiagnostics<'_> {
  fn reset_pos(&mut self, pos: usize) -> usize {
    self.state.reset_pos(pos)
  }

  fn skip_concealed_anchor(&mut self, conceal_end_char_idx: usize) -> usize {
    self.state.skip_concealed(conceal_end_char_idx)
  }

  fn decorate_grapheme(&mut self, grapheme: &FormattedGrapheme) -> usize {
    self
      .state
      .proccess_anchor(grapheme, self.viewport_width, self.horizontal_offset)
  }

  fn render_virt_lines(
    &mut self,
    surface: &mut Surface,
    pos: (usize, u16),
    virt_off: Position,
  ) -> Position {
    use crate::core::diagnostics::DiagnosticFilter;

    let doc_line = pos.0;
    let mut col_off = 0;
    let filter = self.state.filter();

    // Phase 1: Render EOL diagnostic (highest severity NOT shown inline)
    let eol_diagnostic = match self.eol_diagnostics {
      DiagnosticFilter::Enable(eol_filter) => {
        let eol_diagnostics = self
          .state
          .stack
          .iter()
          .filter(|(diag, _)| eol_filter <= diag.severity());
        match filter {
          DiagnosticFilter::Enable(inline_filter) => eol_diagnostics
            .filter(|(diag, _)| inline_filter > diag.severity())
            .max_by_key(|(diagnostic, _)| diagnostic.severity()),
          DiagnosticFilter::Disable => {
            eol_diagnostics.max_by_key(|(diagnostic, _)| diagnostic.severity())
          },
        }
      },
      DiagnosticFilter::Disable => None,
    };

    // Only render EOL diagnostic if not in cursor-line-only mode, or if we're on
    // the cursor line
    let show_eol = !self.eol_cursor_line_only || doc_line == self.cursor_line;
    if show_eol {
      if let Some((eol_diagnostic, _)) = eol_diagnostic {
        col_off = self.draw_eol_diagnostic(surface, eol_diagnostic, doc_line, pos.1, virt_off.col);
      }
    }

    // Phase 2: Compute and render inline diagnostics
    self.state.compute_line_diagnostics();

    if self.state.stack.is_empty() {
      return Position::new(0, col_off as usize);
    }

    // Get animation state for this line - if None, we're still in debounce period
    let Some(anim) = self.get_anim_state(doc_line) else {
      self.state.stack.clear();
      return Position::new(0, col_off as usize);
    };

    // Skip rendering if opacity is too low (faded out)
    if anim.opacity < 0.01 {
      self.state.stack.clear();
      return Position::new(0, col_off as usize);
    }

    // Check if there's enough space to render inline diagnostics
    // Like Helix, we need a minimum viewport width to render the arrows and
    // messages Require at least 60 columns for inline diagnostics to look
    // reasonable
    const MIN_VIEWPORT_WIDTH_FOR_INLINE: u16 = 60;
    if self.viewport_width < MIN_VIEWPORT_WIDTH_FOR_INLINE {
      // Not enough space - clear the stack and only show the underlines
      self.state.stack.clear();
      return Position::new(0, col_off as usize);
    }

    // Check if we need multi-diagnostic rendering
    let has_multi = self.state.has_multi(self.viewport_width);

    // We'll render in virtual lines below the current line
    // pos.1 is the last visual row where text was rendered
    // virt_off.row indicates which virtual line slot we should use (starts at 1)
    let mut current_row = pos.1 + virt_off.row as u16;
    let first_row = current_row;

    // Clone the stack for rendering
    let mut stack = self.state.stack.clone();

    // Render the diagnostics with animation (opacity + slide down)
    if has_multi {
      self.draw_multi_diagnostics(surface, &mut stack, &mut current_row, &anim);
    }

    self.draw_diagnostics(surface, &mut stack, first_row, &mut current_row, &anim);

    let total_height = (current_row - first_row) as usize;

    // Clear the original stack since we processed it
    self.state.stack.clear();

    Position::new(total_height, col_off as usize)
  }
}
