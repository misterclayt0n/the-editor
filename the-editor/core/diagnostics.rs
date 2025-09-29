//! LSP diagnostic utility types.
use std::{
  fmt,
  sync::Arc,
};

use serde::{
  Deserialize,
  Serialize,
};
pub use the_editor_stdx::range::Range;

use crate::core::{
  doc_formatter::FormattedGrapheme,
  document::Document,
  position::{
    Position,
    softwrapped_dimensions,
  },
  text_annotations::LineAnnotation,
  text_format::TextFormat,
};

/// Describes the severity level of a [`Diagnostic`].
#[derive(Debug, Clone, Copy, Eq, PartialEq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
  Hint,
  Info,
  Warning,
  Error,
}

impl Default for Severity {
  fn default() -> Self {
    Self::Hint
  }
}

#[derive(Debug, Eq, Hash, PartialEq, Clone, Deserialize, Serialize)]
pub enum NumberOrString {
  Number(i32),
  String(String),
}

#[derive(Debug, Clone)]
pub enum DiagnosticTag {
  Unnecessary,
  Deprecated,
}

/// Corresponds to [`lsp_types::Diagnostic`](https://docs.rs/lsp-types/0.94.0/lsp_types/struct.Diagnostic.html)
#[derive(Debug, Clone)]
pub struct Diagnostic {
  pub range:          Range,
  // whether this diagnostic ends at the end of(or inside) a word
  pub ends_at_word:   bool,
  pub starts_at_word: bool,
  pub zero_width:     bool,
  pub line:           usize,
  pub message:        String,
  pub severity:       Option<Severity>,
  pub code:           Option<NumberOrString>,
  pub provider:       DiagnosticProvider,
  pub tags:           Vec<DiagnosticTag>,
  pub source:         Option<String>,
  pub data:           Option<serde_json::Value>,
}

/// The source of a diagnostic.
///
/// This type is cheap to clone: all data is either `Copy` or wrapped in an
/// `Arc`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum DiagnosticProvider {
  Lsp {
    /// The ID of the language server which sent the diagnostic.
    server_id:  LanguageServerId,
    /// An optional identifier under which diagnostics are managed by the
    /// client.
    ///
    /// `identifier` is a field from the LSP "Pull Diagnostics" feature meant to
    /// provide an optional "namespace" for diagnostics: a language server
    /// can respond to a diagnostics pull request with an identifier and
    /// these diagnostics should be treated as separate
    /// from push diagnostics. Rust-analyzer uses this feature for example to
    /// provide Cargo diagnostics with push and internal diagnostics with
    /// pull. The push diagnostics should not clear the pull diagnostics and
    /// vice-versa.
    identifier: Option<Arc<str>>,
  },
  // Future internal features can go here...
}

impl DiagnosticProvider {
  pub fn language_server_id(&self) -> Option<LanguageServerId> {
    match self {
      Self::Lsp { server_id, .. } => Some(*server_id),
      // _ => None,
    }
  }
}

// while I would prefer having this in the_editor-lsp that necessitates a bunch
// of conversions I would rather not add. I think its fine since this just a
// very trivial newtype wrapper and we would need something similar once we
// define completions in core
slotmap::new_key_type! {
    pub struct LanguageServerId;
}

impl fmt::Display for LanguageServerId {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{:?}", self.0)
  }
}

impl Diagnostic {
  #[inline]
  pub fn severity(&self) -> Severity {
    self.severity.unwrap_or(Severity::Warning)
  }
}

/// Describes the severity level of a [`Diagnostic`].
#[derive(Debug, Clone, Copy, Eq, PartialEq, PartialOrd, Ord)]
pub enum DiagnosticFilter {
  Disable,
  Enable(Severity),
}

impl<'de> Deserialize<'de> for DiagnosticFilter {
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: serde::Deserializer<'de>,
  {
    match &*String::deserialize(deserializer)? {
      "disable" => Ok(DiagnosticFilter::Disable),
      "hint" => Ok(DiagnosticFilter::Enable(Severity::Hint)),
      "info" => Ok(DiagnosticFilter::Enable(Severity::Info)),
      "warning" => Ok(DiagnosticFilter::Enable(Severity::Warning)),
      "error" => Ok(DiagnosticFilter::Enable(Severity::Error)),
      variant => {
        Err(serde::de::Error::unknown_variant(variant, &[
          "disable", "hint", "info", "warning", "error",
        ]))
      },
    }
  }
}

impl Serialize for DiagnosticFilter {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: serde::Serializer,
  {
    let filter = match self {
      DiagnosticFilter::Disable => "disable",
      DiagnosticFilter::Enable(Severity::Hint) => "hint",
      DiagnosticFilter::Enable(Severity::Info) => "info",
      DiagnosticFilter::Enable(Severity::Warning) => "warning",
      DiagnosticFilter::Enable(Severity::Error) => "error",
    };
    filter.serialize(serializer)
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case", deny_unknown_fields)]
pub struct InlineDiagnosticsConfig {
  pub cursor_line:          DiagnosticFilter,
  pub other_lines:          DiagnosticFilter,
  pub min_diagnostic_width: u16,
  pub prefix_len:           u16,
  pub max_wrap:             u16,
  pub max_diagnostics:      usize,
}

impl InlineDiagnosticsConfig {
  pub fn disabled(&self) -> bool {
    matches!(self, Self {
      cursor_line: DiagnosticFilter::Disable,
      other_lines: DiagnosticFilter::Disable,
      ..
    })
  }

  pub fn prepare(&self, width: u16, enable_cursor_line: bool) -> Self {
    let mut config = self.clone();
    if width < self.min_diagnostic_width + self.prefix_len {
      config.cursor_line = DiagnosticFilter::Disable;
      config.other_lines = DiagnosticFilter::Disable;
    } else if !enable_cursor_line {
      config.cursor_line = self.cursor_line.min(self.other_lines);
    }
    config
  }

  pub fn max_diagnostic_start(&self, width: u16) -> u16 {
    width - self.min_diagnostic_width - self.prefix_len
  }

  pub fn text_fmt(&self, anchor_col: u16, width: u16) -> TextFormat {
    let width = if anchor_col > self.max_diagnostic_start(width) {
      self.min_diagnostic_width
    } else {
      width - anchor_col - self.prefix_len
    };

    TextFormat {
      soft_wrap:                true,
      tab_width:                4,
      max_wrap:                 self.max_wrap.min(width / 4),
      max_indent_retain:        0,
      wrap_indicator:           "".into(),
      wrap_indicator_highlight: None,
      viewport_width:           width,
      soft_wrap_at_text_width:  true,
    }
  }
}

impl Default for InlineDiagnosticsConfig {
  fn default() -> Self {
    InlineDiagnosticsConfig {
      cursor_line:          DiagnosticFilter::Enable(Severity::Warning),
      other_lines:          DiagnosticFilter::Disable,
      min_diagnostic_width: 40,
      prefix_len:           1,
      max_wrap:             20,
      max_diagnostics:      10,
    }
  }
}

pub struct InlineDiagnosticAccumulator<'a> {
  idx:         usize,
  doc:         &'a Document,
  pub stack:   Vec<(&'a Diagnostic, u16)>,
  pub config:  InlineDiagnosticsConfig,
  cursor:      usize,
  cursor_line: bool,
}

impl<'a> InlineDiagnosticAccumulator<'a> {
  pub fn new(cursor: usize, doc: &'a Document, config: InlineDiagnosticsConfig) -> Self {
    InlineDiagnosticAccumulator {
      idx: 0,
      doc,
      stack: Vec::new(),
      config,
      cursor,
      cursor_line: false,
    }
  }

  pub fn reset_pos(&mut self, char_idx: usize) -> usize {
    self.idx = 0;
    self.clear();
    self.skip_concealed(char_idx)
  }

  pub fn skip_concealed(&mut self, conceal_end_char_idx: usize) -> usize {
    let diagnostics = &self.doc.diagnostics[self.idx..];
    let idx = diagnostics.partition_point(|diag| diag.range.start < conceal_end_char_idx);
    self.idx += idx;
    self.next_anchor(conceal_end_char_idx)
  }

  pub fn next_anchor(&self, current_char_idx: usize) -> usize {
    let next_diag_start = self
      .doc
      .diagnostics
      .get(self.idx)
      .map_or(usize::MAX, |diag| diag.range.start);
    if (current_char_idx..next_diag_start).contains(&self.cursor) {
      self.cursor
    } else {
      next_diag_start
    }
  }

  pub fn clear(&mut self) {
    self.cursor_line = false;
    self.stack.clear();
  }

  fn process_anchor_impl(
    &mut self,
    grapheme: &FormattedGrapheme,
    width: u16,
    horizontal_off: usize,
  ) -> bool {
    // TODO: doing the cursor tracking here works well but is somewhat
    // duplicate effort/tedious maybe centralize this somewhere?
    // In the DocFormatter?
    if grapheme.char_idx == self.cursor {
      self.cursor_line = true;
      if self
        .doc
        .diagnostics
        .get(self.idx)
        .is_none_or(|diag| diag.range.start != grapheme.char_idx)
      {
        return false;
      }
    }

    let Some(anchor_col) = grapheme.visual_pos.col.checked_sub(horizontal_off) else {
      return true;
    };
    if anchor_col >= width as usize {
      return true;
    }

    for diag in &self.doc.diagnostics[self.idx..] {
      if diag.range.start != grapheme.char_idx {
        break;
      }
      self.stack.push((diag, anchor_col as u16));
      self.idx += 1;
    }
    false
  }

  pub fn proccess_anchor(
    &mut self,
    grapheme: &FormattedGrapheme,
    width: u16,
    horizontal_off: usize,
  ) -> usize {
    if self.process_anchor_impl(grapheme, width, horizontal_off) {
      self.idx += self.doc.diagnostics[self.idx..]
        .iter()
        .take_while(|diag| diag.range.start == grapheme.char_idx)
        .count();
    }
    self.next_anchor(grapheme.char_idx + 1)
  }

  pub fn filter(&self) -> DiagnosticFilter {
    if self.cursor_line {
      self.config.cursor_line
    } else {
      self.config.other_lines
    }
  }

  pub fn compute_line_diagnostics(&mut self) {
    let filter = if self.cursor_line {
      self.cursor_line = false;
      self.config.cursor_line
    } else {
      self.config.other_lines
    };
    let DiagnosticFilter::Enable(filter) = filter else {
      self.stack.clear();
      return;
    };
    self.stack.retain(|(diag, _)| diag.severity() >= filter);
    self.stack.truncate(self.config.max_diagnostics)
  }

  pub fn has_multi(&self, width: u16) -> bool {
    self
      .stack
      .last()
      .is_some_and(|&(_, anchor)| anchor > self.config.max_diagnostic_start(width))
  }
}

pub(crate) struct InlineDiagnostics<'a> {
  state:          InlineDiagnosticAccumulator<'a>,
  width:          u16,
  horizontal_off: usize,
}

impl<'a> InlineDiagnostics<'a> {
  #[allow(clippy::new_ret_no_self)]
  pub(crate) fn new(
    doc: &'a Document,
    cursor: usize,
    width: u16,
    horizontal_off: usize,
    config: InlineDiagnosticsConfig,
  ) -> Box<dyn LineAnnotation + 'a> {
    Box::new(InlineDiagnostics {
      state: InlineDiagnosticAccumulator::new(cursor, doc, config),
      width,
      horizontal_off,
    })
  }
}

impl LineAnnotation for InlineDiagnostics<'_> {
  fn reset_pos(&mut self, char_idx: usize) -> usize {
    self.state.reset_pos(char_idx)
  }

  fn skip_concealed_anchors(&mut self, conceal_end_char_idx: usize) -> usize {
    self.state.skip_concealed(conceal_end_char_idx)
  }

  fn process_anchor(&mut self, grapheme: &FormattedGrapheme) -> usize {
    self
      .state
      .proccess_anchor(grapheme, self.width, self.horizontal_off)
  }

  fn insert_virtual_lines(
    &mut self,
    _line_end_char_idx: usize,
    _line_end_visual_pos: Position,
    _doc_line: usize,
  ) -> Position {
    self.state.compute_line_diagnostics();
    let multi = self.state.has_multi(self.width);
    let diagostic_height: usize = self
      .state
      .stack
      .drain(..)
      .map(|(diag, anchor)| {
        let text_fmt = self.state.config.text_fmt(anchor, self.width);
        softwrapped_dimensions(diag.message.as_str().trim().into(), &text_fmt).0
      })
      .sum();
    Position::new(multi as usize + diagostic_height, 0)
  }
}
