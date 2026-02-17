use std::{
  cell::RefCell,
  cmp::Ordering,
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
  pub lines:       Vec<InlineDiagnosticRenderLine>,
  pub last_trace:  Option<InlineDiagnosticsRenderTrace>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InlineDiagnosticsRenderTrace {
  pub doc_line:               usize,
  pub cursor_doc_line:        Option<usize>,
  pub cursor_anchor_hit:      bool,
  pub stack_len:              usize,
  pub filtered_len:           usize,
  pub emitted_line_count:     usize,
  pub row_delta:              usize,
  pub used_cursor_line_filter: bool,
}

pub type SharedInlineDiagnosticsRenderData = Rc<RefCell<InlineDiagnosticsRenderData>>;

pub struct InlineDiagnosticsLineAnnotation {
  diagnostics:       Vec<InlineDiagnostic>,
  config:            InlineDiagnosticsConfig,
  cursor_char_idx:   usize,
  cursor_doc_line:   Option<usize>,
  line_start_char:   usize,
  viewport_width:    u16,
  horizontal_offset: usize,
  idx:               usize,
  line_stack:        Vec<(InlineDiagnostic, u16)>,
  cursor_line:       bool,
  render_data:       SharedInlineDiagnosticsRenderData,
}

const BL_CORNER: &str = "┘";
const TR_CORNER: &str = "┌";
const BR_CORNER: &str = "└";
const STACK: &str = "├";
const MULTI: &str = "┴";
const HOR_BAR: &str = "─";
const VER_BAR: &str = "│";

impl InlineDiagnosticsLineAnnotation {
  pub fn new(
    mut diagnostics: Vec<InlineDiagnostic>,
    cursor_char_idx: usize,
    cursor_doc_line: Option<usize>,
    viewport_width: u16,
    horizontal_offset: usize,
    config: InlineDiagnosticsConfig,
    render_data: SharedInlineDiagnosticsRenderData,
  ) -> Self {
    {
      let mut render_data = render_data.borrow_mut();
      render_data.lines.clear();
      render_data.last_trace = None;
    }
    diagnostics.sort_by_key(|diag| diag.start_char_idx);
    Self {
      diagnostics,
      config,
      cursor_char_idx,
      cursor_doc_line,
      line_start_char: 0,
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
    self.line_start_char = 0;
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
    self.line_start_char = char_idx;
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
    line_end_char_idx: usize,
    line_end_visual_pos: Position,
    doc_line: usize,
  ) -> Position {
    let cursor_anchor_hit = self.cursor_line;
    let use_cursor_line_filter = self.cursor_line || self.cursor_doc_line == Some(doc_line);
    let filter = if use_cursor_line_filter {
      self.config.cursor_line
    } else {
      self.config.other_lines
    };
    self.cursor_line = false;
    let stack_len = self.line_stack.len();

    let mut diagnostics: Vec<(InlineDiagnostic, u16)> = self
      .line_stack
      .drain(..)
      .filter(|(diag, _)| filter.allows(diag.severity))
      .collect();

    if diagnostics.is_empty() && line_end_char_idx > self.line_start_char {
      diagnostics = self
        .diagnostics
        .iter()
        .filter(|diag| {
          diag.start_char_idx >= self.line_start_char && diag.start_char_idx < line_end_char_idx
        })
        .filter(|diag| filter.allows(diag.severity))
        .map(|diag| {
          let anchor = diag
            .start_char_idx
            .saturating_sub(self.line_start_char)
            .min(self.viewport_width.saturating_sub(1) as usize) as u16;
          (diag.clone(), anchor)
        })
        .collect();
    }
    let filtered_len = diagnostics.len();

    if diagnostics.len() > self.config.max_diagnostics {
      diagnostics.truncate(self.config.max_diagnostics);
    }

    if diagnostics.is_empty() {
      self.render_data.borrow_mut().last_trace = Some(InlineDiagnosticsRenderTrace {
        doc_line,
        cursor_doc_line: self.cursor_doc_line,
        cursor_anchor_hit,
        stack_len,
        filtered_len,
        emitted_line_count: 0,
        row_delta: 0,
        used_cursor_line_filter: use_cursor_line_filter,
      });
      self.line_start_char = line_end_char_idx;
      return Position::new(0, 0);
    }

    let row_start = line_end_visual_pos.row.saturating_add(1);
    let mut row = row_start;

    let mut render_data = self.render_data.borrow_mut();
    let base_line_count = render_data.lines.len();
    let fallback_diagnostics = diagnostics.clone();
    for (_, anchor) in &mut diagnostics {
      *anchor = (*anchor).min(self.viewport_width.saturating_sub(1));
    }
    draw_multi_diagnostics(
      &mut render_data.lines,
      &mut diagnostics,
      &self.config,
      self.viewport_width,
      &mut row,
    );
    draw_diagnostics(
      &mut render_data.lines,
      &mut diagnostics,
      row_start,
      &self.config,
      self.viewport_width,
      &mut row,
    );

    if render_data.lines.len() == base_line_count {
      let max_anchor_start = self.config.max_diagnostic_start(self.viewport_width);
      for (diagnostic, anchor) in fallback_diagnostics {
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
    }

    let row_delta = row.saturating_sub(row_start);
    let emitted_line_count = render_data.lines.len().saturating_sub(base_line_count);
    render_data.last_trace = Some(InlineDiagnosticsRenderTrace {
      doc_line,
      cursor_doc_line: self.cursor_doc_line,
      cursor_anchor_hit,
      stack_len,
      filtered_len,
      emitted_line_count,
      row_delta,
      used_cursor_line_filter: use_cursor_line_filter,
    });
    self.line_start_char = line_end_char_idx;

    Position::new(row_delta, 0)
  }
}

fn draw_multi_diagnostics(
  render_lines: &mut Vec<InlineDiagnosticRenderLine>,
  diagnostics: &mut Vec<(InlineDiagnostic, u16)>,
  config: &InlineDiagnosticsConfig,
  viewport_width: u16,
  row: &mut usize,
) {
  let Some((last_diagnostic, last_anchor)) = diagnostics.last().cloned() else {
    return;
  };

  let start = config.max_diagnostic_start(viewport_width);
  if last_anchor <= start {
    return;
  }

  let mut severity = last_diagnostic.severity;
  let mut last_anchor = last_anchor;
  push_connector(render_lines, *row, last_anchor, BL_CORNER, severity);

  let mut stacked_count = 1usize;
  for (diagnostic, anchor) in diagnostics.iter().rev().skip(1) {
    let symbol = match anchor.cmp(&start) {
      Ordering::Less => break,
      Ordering::Equal => STACK,
      Ordering::Greater => MULTI,
    };

    stacked_count = stacked_count.saturating_add(1);
    let previous_severity = severity;
    severity = max_diagnostic_severity(severity, diagnostic.severity);
    if *anchor == last_anchor && severity == previous_severity {
      continue;
    }

    for col in anchor.saturating_add(1)..last_anchor {
      push_connector(render_lines, *row, col, HOR_BAR, previous_severity);
    }
    push_connector(render_lines, *row, *anchor, symbol, severity);
    last_anchor = *anchor;
  }

  if last_anchor != start {
    for col in start.saturating_add(1)..last_anchor {
      push_connector(render_lines, *row, col, HOR_BAR, severity);
    }
    push_connector(render_lines, *row, start, TR_CORNER, severity);
  }

  *row = row.saturating_add(1);

  let split_at = diagnostics.len().saturating_sub(stacked_count);
  let stacked = diagnostics.split_off(split_at);
  for index in (0..stacked.len()).rev() {
    let (diagnostic, _) = &stacked[index];
    let next_severity = max_diagnostic_severity_slice(&stacked[..index]);
    draw_diagnostic_entry(
      render_lines,
      row,
      start,
      diagnostic,
      next_severity,
      config,
      viewport_width,
    );
  }
}

fn draw_diagnostics(
  render_lines: &mut Vec<InlineDiagnosticRenderLine>,
  diagnostics: &mut Vec<(InlineDiagnostic, u16)>,
  row_start: usize,
  config: &InlineDiagnosticsConfig,
  viewport_width: u16,
  row: &mut usize,
) {
  let mut reversed = diagnostics.drain(..).rev().peekable();
  let mut last_anchor = viewport_width;
  let mut last_severity: Option<DiagnosticSeverity> = None;
  let mut trailing_anchors: Vec<(u16, DiagnosticSeverity)> = Vec::new();
  while let Some((diagnostic, anchor)) = reversed.next() {
    if anchor != last_anchor {
      for draw_row in row_start..*row {
        push_connector(render_lines, draw_row, anchor, VER_BAR, diagnostic.severity);
      }
      if last_anchor != viewport_width
        && let Some(severity) = last_severity
      {
        trailing_anchors.push((last_anchor, severity));
      }
    }

    let next_severity = reversed.peek().and_then(|(next_diagnostic, next_anchor)| {
      (*next_anchor == anchor).then_some(next_diagnostic.severity)
    });
    draw_diagnostic_entry(
      render_lines,
      row,
      anchor,
      &diagnostic,
      next_severity,
      config,
      viewport_width,
    );
    last_anchor = anchor;
    last_severity = Some(diagnostic.severity);
  }

  if !trailing_anchors.is_empty() {
    for (anchor, severity) in trailing_anchors {
      push_connector(render_lines, *row, anchor, VER_BAR, severity);
    }
    *row = row.saturating_add(1);
  }
}

fn draw_diagnostic_entry(
  render_lines: &mut Vec<InlineDiagnosticRenderLine>,
  row: &mut usize,
  anchor_col: u16,
  diagnostic: &InlineDiagnostic,
  next_severity: Option<DiagnosticSeverity>,
  config: &InlineDiagnosticsConfig,
  viewport_width: u16,
) {
  let severity = diagnostic.severity;
  let (corner, corner_severity) = if let Some(next) = next_severity {
    (STACK, max_diagnostic_severity(next, severity))
  } else {
    (BR_CORNER, severity)
  };
  push_connector(render_lines, *row, anchor_col, corner, corner_severity);
  for offset in 0..config.prefix_len {
    push_connector(
      render_lines,
      *row,
      anchor_col.saturating_add(offset).saturating_add(1),
      HOR_BAR,
      severity,
    );
  }

  let text_col = anchor_col.saturating_add(config.prefix_len).saturating_add(1);
  let text_fmt = config.text_format(text_col, viewport_width);
  let wrapped = soft_wrap_message_lines(diagnostic.message.as_ref(), &text_fmt);
  if wrapped.is_empty() {
    return;
  }
  let wrapped_len = wrapped.len();
  for (idx, line) in wrapped.into_iter().enumerate() {
    render_lines.push(InlineDiagnosticRenderLine {
      row: row.saturating_add(idx),
      col: text_col as usize,
      text: line,
      severity,
    });
  }

  let extra_rows = wrapped_len.saturating_sub(1);
  *row = row.saturating_add(1);
  if let Some(next) = next_severity {
    for _ in 0..extra_rows {
      push_connector(render_lines, *row, anchor_col, VER_BAR, next);
      *row = row.saturating_add(1);
    }
  } else {
    *row = row.saturating_add(extra_rows);
  }
}

fn push_connector(
  render_lines: &mut Vec<InlineDiagnosticRenderLine>,
  row: usize,
  col: u16,
  glyph: &'static str,
  severity: DiagnosticSeverity,
) {
  render_lines.push(InlineDiagnosticRenderLine {
    row,
    col: col as usize,
    text: glyph.into(),
    severity,
  });
}

fn max_diagnostic_severity(
  left: DiagnosticSeverity,
  right: DiagnosticSeverity,
) -> DiagnosticSeverity {
  if diagnostic_severity_rank(left) >= diagnostic_severity_rank(right) {
    left
  } else {
    right
  }
}

fn max_diagnostic_severity_slice(
  diagnostics: &[(InlineDiagnostic, u16)],
) -> Option<DiagnosticSeverity> {
  diagnostics
    .iter()
    .map(|(diagnostic, _)| diagnostic.severity)
    .max_by_key(|severity| diagnostic_severity_rank(*severity))
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
  use std::{
    cell::RefCell,
    rc::Rc,
  };

  use ropey::Rope;

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

  #[test]
  fn inline_diagnostics_annotation_collects_render_lines() {
    let diagnostics = vec![InlineDiagnostic::new(
      0,
      DiagnosticSeverity::Warning,
      "this diagnostic should wrap into multiple lines for rendering",
    )];
    let config = InlineDiagnosticsConfig {
      cursor_line: InlineDiagnosticFilter::Disable,
      other_lines: InlineDiagnosticFilter::Enable(DiagnosticSeverity::Hint),
      min_diagnostic_width: 12,
      prefix_len: 1,
      max_wrap: 8,
      max_diagnostics: 5,
    };
    let render_data: SharedInlineDiagnosticsRenderData = Rc::new(RefCell::new(Default::default()));
    let annotation = InlineDiagnosticsLineAnnotation::new(
      diagnostics,
      usize::MAX,
      None,
      20,
      0,
      config,
      render_data.clone(),
    );

    let text = Rope::from("abc\n");
    let mut text_fmt = TextFormat::default();
    text_fmt.soft_wrap = false;
    text_fmt.viewport_width = 20;

    let mut annotations = TextAnnotations::default();
    let _ = annotations.add_line_annotation(Box::new(annotation));
    {
      let mut formatter =
        DocumentFormatter::new_at_prev_checkpoint(text.slice(..), &text_fmt, &mut annotations, 0);
      while formatter.next().is_some() {}
    }

    let lines = render_data.borrow().lines.clone();
    assert!(!lines.is_empty());
    assert!(lines.windows(2).all(|pair| pair[0].row <= pair[1].row));
  }

  #[test]
  fn inline_diagnostics_annotation_collects_render_lines_multiline_doc() {
    let text = Rope::from("line1\nline2 error here\nline3\n");
    let error_char_idx = text.line_to_char(1).saturating_add(6);
    let diagnostics = vec![InlineDiagnostic::new(
      error_char_idx,
      DiagnosticSeverity::Error,
      "sample inline diagnostic message",
    )];
    let config = InlineDiagnosticsConfig {
      cursor_line: InlineDiagnosticFilter::Enable(DiagnosticSeverity::Warning),
      other_lines: InlineDiagnosticFilter::Disable,
      min_diagnostic_width: 12,
      prefix_len: 1,
      max_wrap: 8,
      max_diagnostics: 5,
    };
    let render_data: SharedInlineDiagnosticsRenderData = Rc::new(RefCell::new(Default::default()));
    let annotation = InlineDiagnosticsLineAnnotation::new(
      diagnostics,
      text.line_to_char(1).saturating_add(12),
      Some(1),
      80,
      0,
      config,
      render_data.clone(),
    );

    let mut text_fmt = TextFormat::default();
    text_fmt.soft_wrap = false;
    text_fmt.viewport_width = 80;

    let mut annotations = TextAnnotations::default();
    let _ = annotations.add_line_annotation(Box::new(annotation));
    {
      let mut formatter =
        DocumentFormatter::new_at_prev_checkpoint(text.slice(..), &text_fmt, &mut annotations, 0);
      while formatter.next().is_some() {}
    }

    let lines = render_data.borrow().lines.clone();
    assert!(!lines.is_empty());
  }

  #[test]
  fn inline_diagnostics_draws_bottom_trailing_connectors_for_multiple_anchors() {
    let config = InlineDiagnosticsConfig::default();
    let mut row = 0usize;
    let mut render_lines = Vec::new();
    let mut diagnostics = vec![
      (
        InlineDiagnostic::new(0, DiagnosticSeverity::Warning, "left"),
        2,
      ),
      (
        InlineDiagnostic::new(4, DiagnosticSeverity::Error, "right"),
        6,
      ),
    ];

    draw_diagnostics(
      &mut render_lines,
      &mut diagnostics,
      0,
      &config,
      80,
      &mut row,
    );

    assert!(
      render_lines
        .iter()
        .any(|line| line.row == 2 && line.col == 6 && line.text.as_str() == VER_BAR)
    );
    assert_eq!(row, 3);
  }

  #[test]
  fn inline_diagnostics_render_lines_survive_visual_position_queries() {
    let text = Rope::from("line1\nline2 error here\nline3\n");
    let error_char_idx = text.line_to_char(1).saturating_add(6);
    let diagnostics = vec![InlineDiagnostic::new(
      error_char_idx,
      DiagnosticSeverity::Error,
      "sample inline diagnostic message",
    )];
    let config = InlineDiagnosticsConfig {
      cursor_line: InlineDiagnosticFilter::Enable(DiagnosticSeverity::Warning),
      other_lines: InlineDiagnosticFilter::Disable,
      min_diagnostic_width: 12,
      prefix_len: 1,
      max_wrap: 8,
      max_diagnostics: 5,
    };
    let render_data: SharedInlineDiagnosticsRenderData = Rc::new(RefCell::new(Default::default()));
    let annotation = InlineDiagnosticsLineAnnotation::new(
      diagnostics,
      text.line_to_char(1).saturating_add(12),
      Some(1),
      80,
      0,
      config,
      render_data.clone(),
    );

    let mut text_fmt = TextFormat::default();
    text_fmt.soft_wrap = false;
    text_fmt.viewport_width = 80;

    let mut annotations = TextAnnotations::default();
    let _ = annotations.add_line_annotation(Box::new(annotation));
    {
      let mut formatter =
        DocumentFormatter::new_at_prev_checkpoint(text.slice(..), &text_fmt, &mut annotations, 0);
      while formatter.next().is_some() {}
    }

    let before_query = render_data.borrow().lines.len();
    assert!(before_query > 0);

    let _ = crate::render::visual_position::visual_pos_at_char(
      text.slice(..),
      &text_fmt,
      &mut annotations,
      text.len_chars(),
    );

    let after_query = render_data.borrow().lines.len();
    assert!(
      after_query >= before_query,
      "render lines were unexpectedly dropped after visual position query"
    );
  }
}
