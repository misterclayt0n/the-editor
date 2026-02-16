use std::{
  cell::RefCell,
  rc::Rc,
};

use ropey::Rope;
use the_core::grapheme::Grapheme;

use crate::{
  Tendril,
  diagnostics::DiagnosticSeverity,
  position::Position,
  render::{
    doc_formatter::DocumentFormatter,
    text_annotations::{
      LineAnnotation,
      TextAnnotations,
    },
    text_format::TextFormat,
  },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InlineDiagnosticFilter {
  Disable,
  Enable(DiagnosticSeverity),
}

impl InlineDiagnosticFilter {
  pub fn allows(self, severity: DiagnosticSeverity) -> bool {
    match self {
      Self::Disable => false,
      Self::Enable(min) => diagnostic_severity_rank(severity) >= diagnostic_severity_rank(min),
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InlineDiagnosticsConfig {
  pub cursor_line:          InlineDiagnosticFilter,
  pub other_lines:          InlineDiagnosticFilter,
  pub min_diagnostic_width: u16,
  pub prefix_len:           u16,
  pub max_wrap:             u16,
  pub max_diagnostics:      usize,
}

impl Default for InlineDiagnosticsConfig {
  fn default() -> Self {
    Self {
      cursor_line:          InlineDiagnosticFilter::Enable(DiagnosticSeverity::Warning),
      other_lines:          InlineDiagnosticFilter::Disable,
      min_diagnostic_width: 40,
      prefix_len:           1,
      max_wrap:             20,
      max_diagnostics:      10,
    }
  }
}

impl InlineDiagnosticsConfig {
  pub fn disabled(&self) -> bool {
    matches!(
      self,
      Self {
        cursor_line: InlineDiagnosticFilter::Disable,
        other_lines: InlineDiagnosticFilter::Disable,
        ..
      }
    )
  }

  pub fn prepare(&self, width: u16, enable_cursor_line: bool) -> Self {
    let mut config = self.clone();
    if width < self.min_diagnostic_width.saturating_add(self.prefix_len) {
      config.cursor_line = InlineDiagnosticFilter::Disable;
      config.other_lines = InlineDiagnosticFilter::Disable;
    } else if !enable_cursor_line {
      config.cursor_line = self.other_lines;
    }
    config
  }

  pub fn max_diagnostic_start(&self, width: u16) -> u16 {
    width
      .saturating_sub(self.min_diagnostic_width)
      .saturating_sub(self.prefix_len)
  }

  pub fn text_format(&self, anchor_col: u16, width: u16) -> TextFormat {
    let available = if anchor_col > self.max_diagnostic_start(width) {
      self.min_diagnostic_width
    } else {
      width
        .saturating_sub(anchor_col)
        .saturating_sub(self.prefix_len)
    }
    .max(1);

    let max_wrap = self.max_wrap.min((available / 4).max(1));
    let mut text_fmt = TextFormat::default();
    text_fmt.soft_wrap = true;
    text_fmt.tab_width = 4;
    text_fmt.max_wrap = max_wrap;
    text_fmt.max_indent_retain = 0;
    text_fmt.wrap_indicator = "".into();
    text_fmt.rebuild_wrap_indicator();
    text_fmt.wrap_indicator_highlight = None;
    text_fmt.viewport_width = available;
    text_fmt.soft_wrap_at_text_width = true;
    text_fmt
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InlineDiagnostic {
  pub start_char_idx: usize,
  pub severity:       DiagnosticSeverity,
  pub message:        Tendril,
}

impl InlineDiagnostic {
  pub fn new(start_char_idx: usize, severity: DiagnosticSeverity, message: impl Into<Tendril>) -> Self {
    Self {
      start_char_idx,
      severity,
      message: message.into(),
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InlineDiagnosticRenderLine {
  pub row:      usize,
  pub col:      usize,
  pub text:     Tendril,
  pub severity: DiagnosticSeverity,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct InlineDiagnosticsRenderData {
  pub lines: Vec<InlineDiagnosticRenderLine>,
}

pub type SharedInlineDiagnosticsRenderData = Rc<RefCell<InlineDiagnosticsRenderData>>;

pub struct InlineDiagnosticsLineAnnotation {
  diagnostics:       Vec<InlineDiagnostic>,
  config:            InlineDiagnosticsConfig,
  cursor_char_idx:   usize,
  viewport_width:    u16,
  horizontal_offset: usize,
  idx:               usize,
  line_stack:        Vec<(InlineDiagnostic, u16)>,
  cursor_line:       bool,
  render_data:       SharedInlineDiagnosticsRenderData,
}

impl InlineDiagnosticsLineAnnotation {
  pub fn new(
    mut diagnostics: Vec<InlineDiagnostic>,
    cursor_char_idx: usize,
    viewport_width: u16,
    horizontal_offset: usize,
    config: InlineDiagnosticsConfig,
    render_data: SharedInlineDiagnosticsRenderData,
  ) -> Self {
    diagnostics.sort_by_key(|diag| diag.start_char_idx);
    Self {
      diagnostics,
      config,
      cursor_char_idx,
      viewport_width,
      horizontal_offset,
      idx: 0,
      line_stack: Vec::new(),
      cursor_line: false,
      render_data,
    }
  }

  fn reset_state(&mut self) {
    self.idx = 0;
    self.cursor_line = false;
    self.line_stack.clear();
    self.render_data.borrow_mut().lines.clear();
  }

  fn next_anchor(&self, current_char_idx: usize) -> usize {
    let next_diag_start = self
      .diagnostics
      .get(self.idx)
      .map_or(usize::MAX, |diag| diag.start_char_idx);

    if (current_char_idx..next_diag_start).contains(&self.cursor_char_idx) {
      self.cursor_char_idx
    } else {
      next_diag_start
    }
  }

  fn process_anchor_impl(&mut self, grapheme_char_idx: usize, visual_col: usize) {
    if grapheme_char_idx == self.cursor_char_idx {
      self.cursor_line = true;
    }

    while self.idx < self.diagnostics.len()
      && self.diagnostics[self.idx].start_char_idx < grapheme_char_idx
    {
      self.idx += 1;
    }

    let anchor_col = visual_col.saturating_sub(self.horizontal_offset);
    while self.idx < self.diagnostics.len()
      && self.diagnostics[self.idx].start_char_idx == grapheme_char_idx
    {
      let diagnostic = self.diagnostics[self.idx].clone();
      if anchor_col < self.viewport_width as usize {
        self.line_stack.push((diagnostic, anchor_col as u16));
      }
      self.idx += 1;
    }
  }
}

impl LineAnnotation for InlineDiagnosticsLineAnnotation {
  fn reset_pos(&mut self, char_idx: usize) -> usize {
    self.reset_state();
    self.idx = self
      .diagnostics
      .partition_point(|diag| diag.start_char_idx < char_idx);
    self.next_anchor(char_idx)
  }

  fn skip_concealed_anchors(&mut self, conceal_end_char_idx: usize) -> usize {
    while self.idx < self.diagnostics.len()
      && self.diagnostics[self.idx].start_char_idx < conceal_end_char_idx
    {
      self.idx += 1;
    }
    self.next_anchor(conceal_end_char_idx)
  }

  fn process_anchor(&mut self, grapheme: &crate::render::FormattedGrapheme) -> usize {
    self.process_anchor_impl(grapheme.char_idx, grapheme.visual_pos.col);
    self.next_anchor(grapheme.char_idx.saturating_add(1))
  }

  fn insert_virtual_lines(
    &mut self,
    _line_end_char_idx: usize,
    line_end_visual_pos: Position,
    _doc_line: usize,
  ) -> Position {
    let filter = if self.cursor_line {
      self.config.cursor_line
    } else {
      self.config.other_lines
    };
    self.cursor_line = false;

    let mut diagnostics: Vec<(InlineDiagnostic, u16)> = self
      .line_stack
      .drain(..)
      .filter(|(diag, _)| filter.allows(diag.severity))
      .collect();

    if diagnostics.len() > self.config.max_diagnostics {
      diagnostics.truncate(self.config.max_diagnostics);
    }

    if diagnostics.is_empty() {
      return Position::new(0, 0);
    }

    let max_anchor_start = self.config.max_diagnostic_start(self.viewport_width);
    let mut row = line_end_visual_pos.row.saturating_add(1);
    let row_start = row;

    let mut render_data = self.render_data.borrow_mut();
    for (diagnostic, anchor) in diagnostics {
      let anchor = anchor.min(max_anchor_start);
      let text_col = anchor.saturating_add(self.config.prefix_len) as usize;
      let text_fmt = self.config.text_format(anchor, self.viewport_width);
      let wrapped = soft_wrap_message_lines(diagnostic.message.as_ref(), &text_fmt);
      if wrapped.is_empty() {
        continue;
      }

      for line in wrapped {
        render_data.lines.push(InlineDiagnosticRenderLine {
          row,
          col: text_col,
          text: line,
          severity: diagnostic.severity,
        });
        row = row.saturating_add(1);
      }
    }

    Position::new(row.saturating_sub(row_start), 0)
  }
}

fn diagnostic_severity_rank(severity: DiagnosticSeverity) -> u8 {
  match severity {
    DiagnosticSeverity::Error => 4,
    DiagnosticSeverity::Warning => 3,
    DiagnosticSeverity::Information => 2,
    DiagnosticSeverity::Hint => 1,
  }
}

fn soft_wrap_message_lines(message: &str, text_fmt: &TextFormat) -> Vec<Tendril> {
  let message = message.trim();
  if message.is_empty() {
    return Vec::new();
  }

  let rope = Rope::from(message);
  let mut annotations = TextAnnotations::default();
  let mut formatter =
    DocumentFormatter::new_at_prev_checkpoint(rope.slice(..), text_fmt, &mut annotations, 0);

  let mut rows: Vec<String> = Vec::new();
  for grapheme in &mut formatter {
    if grapheme.source.is_eof() {
      break;
    }

    match grapheme.raw {
      Grapheme::Newline => {},
      Grapheme::Tab { width } => {
        while rows.len() <= grapheme.visual_pos.row {
          rows.push(String::new());
        }
        rows[grapheme.visual_pos.row].push_str(&" ".repeat(width));
      },
      Grapheme::Other { ref g } => {
        while rows.len() <= grapheme.visual_pos.row {
          rows.push(String::new());
        }
        rows[grapheme.visual_pos.row].push_str(&g.to_string());
      },
    }
  }

  if rows.is_empty() {
    return vec![message.to_string().into()];
  }

  rows
    .into_iter()
    .map(|row| row.into())
    .collect()
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn inline_diagnostics_prepare_disables_when_too_narrow() {
    let config = InlineDiagnosticsConfig::default();
    let prepared = config.prepare(8, true);
    assert!(prepared.disabled());
  }

  #[test]
  fn inline_diagnostics_wraps_message_to_multiple_rows() {
    let mut fmt = TextFormat::default();
    fmt.soft_wrap = true;
    fmt.tab_width = 4;
    fmt.max_wrap = 8;
    fmt.max_indent_retain = 0;
    fmt.wrap_indicator = "".into();
    fmt.rebuild_wrap_indicator();
    fmt.wrap_indicator_highlight = None;
    fmt.viewport_width = 12;
    fmt.soft_wrap_at_text_width = true;
    let lines = soft_wrap_message_lines(
      "memory allocation APIs are documented inline diagnostics",
      &fmt,
    );
    assert!(lines.len() > 1);
  }

  #[test]
  fn inline_diagnostics_filter_respects_minimum_severity() {
    let filter = InlineDiagnosticFilter::Enable(DiagnosticSeverity::Warning);
    assert!(filter.allows(DiagnosticSeverity::Error));
    assert!(filter.allows(DiagnosticSeverity::Warning));
    assert!(!filter.allows(DiagnosticSeverity::Information));
  }
}
