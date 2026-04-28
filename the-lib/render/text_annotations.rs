use std::mem::replace;

use ropey::{
  Rope,
  RopeSlice,
};
use thiserror::Error;
use unicode_segmentation::UnicodeSegmentation;

use crate::{
  Tendril,
  grapheme::{
    Grapheme,
    GraphemeStr,
  },
  render::{
    doc_formatter::DocumentFormatter,
    text_format::TextFormat,
  },
};

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum TextAnnotationError {
  #[error("inline annotations must not contain line breaks")]
  InlineAnnotationContainsLineBreak,
  #[error("grapheme substitutions must not contain line breaks")]
  GraphemeSubstitutionContainsLineBreak,
  #[error("grapheme substitutions must contain exactly one grapheme")]
  GraphemeSubstitutionMustBeSingleGrapheme,
}

/// Continuous text inserted before the document grapheme at `char_idx`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InlineAnnotation {
  pub char_idx: usize,
  pub text:     Tendril,
}

impl InlineAnnotation {
  pub fn new(char_idx: usize, text: impl Into<Tendril>) -> Result<Self, TextAnnotationError> {
    let text = text.into();
    if contains_line_break(&text) {
      return Err(TextAnnotationError::InlineAnnotationContainsLineBreak);
    }

    Ok(Self { char_idx, text })
  }
}

/// A single-grapheme visual substitution for one document grapheme.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphemeSubstitution {
  pub char_idx: usize,
  pub grapheme: Tendril,
}

impl GraphemeSubstitution {
  pub fn new(char_idx: usize, grapheme: impl Into<Tendril>) -> Result<Self, TextAnnotationError> {
    let grapheme = grapheme.into();
    if contains_line_break(&grapheme) {
      return Err(TextAnnotationError::GraphemeSubstitutionContainsLineBreak);
    }
    if !is_single_grapheme(&grapheme) {
      return Err(TextAnnotationError::GraphemeSubstitutionMustBeSingleGrapheme);
    }

    Ok(Self { char_idx, grapheme })
  }
}

/// Extra visual text inserted after a document line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VirtualLineSpec {
  pub doc_line:       usize,
  pub col:            usize,
  pub text:           Tendril,
  pub wrap_to_view:   bool,
  pub max_wrap_width: Option<usize>,
}

impl VirtualLineSpec {
  pub fn after(doc_line: usize) -> Self {
    Self {
      doc_line,
      col: 0,
      text: Tendril::new(),
      wrap_to_view: true,
      max_wrap_width: None,
    }
  }

  pub fn col(mut self, col: usize) -> Self {
    self.col = col;
    self
  }

  pub fn text(mut self, text: impl Into<Tendril>) -> Self {
    self.text = text.into();
    self
  }

  pub fn wrap_to_viewport(mut self) -> Self {
    self.wrap_to_view = true;
    self
  }

  pub fn single_line(mut self) -> Self {
    self.wrap_to_view = false;
    self
  }

  pub fn max_wrap_width(mut self, width: usize) -> Self {
    self.max_wrap_width = Some(width.max(1));
    self
  }
}

/// Owned layout annotations consumed by the render kernel.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct TextAnnotations {
  inline_annotations:     Vec<InlineAnnotation>,
  grapheme_substitutions: Vec<GraphemeSubstitution>,
  virtual_lines:          Vec<VirtualLineSpec>,
}

impl TextAnnotations {
  pub fn is_empty(&self) -> bool {
    self.inline_annotations.is_empty()
      && self.grapheme_substitutions.is_empty()
      && self.virtual_lines.is_empty()
  }

  pub fn inline_annotations(&self) -> &[InlineAnnotation] {
    &self.inline_annotations
  }

  pub fn grapheme_substitutions(&self) -> &[GraphemeSubstitution] {
    &self.grapheme_substitutions
  }

  pub fn virtual_lines(&self) -> &[VirtualLineSpec] {
    &self.virtual_lines
  }

  pub fn add_inline_annotation(&mut self, annotation: InlineAnnotation) -> &mut Self {
    self.inline_annotations.push(annotation);
    self
      .inline_annotations
      .sort_by_key(|annotation| annotation.char_idx);
    self
  }

  pub fn add_inline_annotations(
    &mut self,
    annotations: impl IntoIterator<Item = InlineAnnotation>,
  ) -> &mut Self {
    self.inline_annotations.extend(annotations);
    self
      .inline_annotations
      .sort_by_key(|annotation| annotation.char_idx);
    self
  }

  pub fn add_inline_text(
    &mut self,
    char_idx: usize,
    text: impl Into<Tendril>,
  ) -> Result<&mut Self, TextAnnotationError> {
    let annotation = InlineAnnotation::new(char_idx, text)?;
    Ok(self.add_inline_annotation(annotation))
  }

  pub fn add_grapheme_substitution(&mut self, substitution: GraphemeSubstitution) -> &mut Self {
    self.grapheme_substitutions.push(substitution);
    self
      .grapheme_substitutions
      .sort_by_key(|substitution| substitution.char_idx);
    self
  }

  pub fn add_grapheme_substitutions(
    &mut self,
    substitutions: impl IntoIterator<Item = GraphemeSubstitution>,
  ) -> &mut Self {
    self.grapheme_substitutions.extend(substitutions);
    self
      .grapheme_substitutions
      .sort_by_key(|substitution| substitution.char_idx);
    self
  }

  pub fn add_grapheme_substitution_text(
    &mut self,
    char_idx: usize,
    grapheme: impl Into<Tendril>,
  ) -> Result<&mut Self, TextAnnotationError> {
    let substitution = GraphemeSubstitution::new(char_idx, grapheme)?;
    Ok(self.add_grapheme_substitution(substitution))
  }

  pub fn add_virtual_line(&mut self, spec: VirtualLineSpec) -> &mut Self {
    if !spec.text.is_empty() {
      self.virtual_lines.push(spec);
      self.virtual_lines.sort_by_key(|spec| spec.doc_line);
    }
    self
  }

  pub fn add_virtual_lines(
    &mut self,
    specs: impl IntoIterator<Item = VirtualLineSpec>,
  ) -> &mut Self {
    self
      .virtual_lines
      .extend(specs.into_iter().filter(|spec| !spec.text.is_empty()));
    self.virtual_lines.sort_by_key(|spec| spec.doc_line);
    self
  }

  pub(crate) fn cursor_at<'a>(
    &'a self,
    text: RopeSlice<'_>,
    char_idx: usize,
  ) -> TextAnnotationsCursor<'a> {
    let mut cursor = TextAnnotationsCursor::new(text, self);
    cursor.reset_pos(char_idx);
    cursor
  }
}

#[derive(Debug, Clone)]
struct ResolvedVirtualLineSpec {
  spec:            VirtualLineSpec,
  anchor_char_idx: usize,
}

#[derive(Debug)]
pub(crate) struct TextAnnotationsCursor<'a> {
  inline_annotations:          &'a [InlineAnnotation],
  inline_index:                usize,
  grapheme_substitutions:      &'a [GraphemeSubstitution],
  grapheme_substitution_index: usize,
  virtual_lines:               Vec<ResolvedVirtualLineSpec>,
  virtual_line_index:          usize,
  pending_virtual_text:        Vec<GraphemeStr<'static>>,
  pending_virtual_index:       usize,
}

impl<'a> TextAnnotationsCursor<'a> {
  fn new(text: RopeSlice<'_>, annotations: &'a TextAnnotations) -> Self {
    let line_count = text.len_lines().max(1);
    let virtual_lines = annotations
      .virtual_lines
      .iter()
      .cloned()
      .map(|spec| {
        let anchor_char_idx = if spec.doc_line + 1 < line_count {
          text.line_to_char(spec.doc_line + 1)
        } else {
          text.len_chars()
        };
        ResolvedVirtualLineSpec {
          spec,
          anchor_char_idx,
        }
      })
      .collect();

    Self {
      inline_annotations: &annotations.inline_annotations,
      inline_index: 0,
      grapheme_substitutions: &annotations.grapheme_substitutions,
      grapheme_substitution_index: 0,
      virtual_lines,
      virtual_line_index: 0,
      pending_virtual_text: Vec::new(),
      pending_virtual_index: 0,
    }
  }

  fn reset_pos(&mut self, char_idx: usize) {
    self.inline_index = self
      .inline_annotations
      .partition_point(|annotation| annotation.char_idx < char_idx);
    self.grapheme_substitution_index = self
      .grapheme_substitutions
      .partition_point(|substitution| substitution.char_idx < char_idx);
    self.virtual_line_index = self
      .virtual_lines
      .partition_point(|spec| spec.anchor_char_idx < char_idx);
    self.pending_virtual_text.clear();
    self.pending_virtual_index = 0;
  }

  pub(crate) fn next_inline_annotation_at(
    &mut self,
    char_idx: usize,
  ) -> Option<&'a InlineAnnotation> {
    while self.inline_index < self.inline_annotations.len()
      && self.inline_annotations[self.inline_index].char_idx < char_idx
    {
      self.inline_index += 1;
    }

    let annotation = self.inline_annotations.get(self.inline_index)?;
    if annotation.char_idx == char_idx {
      self.inline_index += 1;
      Some(annotation)
    } else {
      None
    }
  }

  pub(crate) fn grapheme_substitution_at(
    &mut self,
    char_idx: usize,
  ) -> Option<&'a GraphemeSubstitution> {
    while self.grapheme_substitution_index < self.grapheme_substitutions.len()
      && self.grapheme_substitutions[self.grapheme_substitution_index].char_idx < char_idx
    {
      self.grapheme_substitution_index += 1;
    }

    let mut matched = None;
    while self.grapheme_substitution_index < self.grapheme_substitutions.len() {
      let substitution = &self.grapheme_substitutions[self.grapheme_substitution_index];
      if substitution.char_idx != char_idx {
        break;
      }
      matched = Some(substitution);
      self.grapheme_substitution_index += 1;
    }
    matched
  }

  pub(crate) fn next_virtual_text_grapheme_at(
    &mut self,
    char_idx: usize,
    text_format: &TextFormat,
    wrap_width: usize,
  ) -> Option<GraphemeStr<'static>> {
    loop {
      if self.pending_virtual_index < self.pending_virtual_text.len() {
        let index = self.pending_virtual_index;
        self.pending_virtual_index += 1;
        let grapheme = replace(&mut self.pending_virtual_text[index], GraphemeStr::from(""));
        return Some(grapheme);
      }

      self.pending_virtual_text.clear();
      self.pending_virtual_index = 0;

      while self.virtual_line_index < self.virtual_lines.len()
        && self.virtual_lines[self.virtual_line_index].anchor_char_idx < char_idx
      {
        self.virtual_line_index += 1;
      }

      if self.virtual_line_index >= self.virtual_lines.len()
        || self.virtual_lines[self.virtual_line_index].anchor_char_idx != char_idx
      {
        return None;
      }

      let start = self.virtual_line_index;
      while self.virtual_line_index < self.virtual_lines.len()
        && self.virtual_lines[self.virtual_line_index].anchor_char_idx == char_idx
      {
        self.virtual_line_index += 1;
      }

      let block = build_virtual_text_block(
        &self.virtual_lines[start..self.virtual_line_index],
        text_format,
        wrap_width,
      );

      self.pending_virtual_text = UnicodeSegmentation::graphemes(block.as_str(), true)
        .map(|grapheme| GraphemeStr::from(grapheme.to_owned()))
        .collect();
    }
  }
}

fn build_virtual_text_block(
  specs: &[ResolvedVirtualLineSpec],
  text_format: &TextFormat,
  wrap_width: usize,
) -> String {
  let mut block = String::new();

  for spec in specs {
    for row in render_virtual_line_rows(&spec.spec, text_format, wrap_width) {
      block.push_str(&row);
      block.push('\n');
    }
  }

  block
}

fn render_virtual_line_rows(
  spec: &VirtualLineSpec,
  text_format: &TextFormat,
  wrap_width: usize,
) -> Vec<String> {
  if spec.text.is_empty() {
    return Vec::new();
  }

  let available_width = spec
    .max_wrap_width
    .unwrap_or_else(|| wrap_width.saturating_sub(spec.col).max(1))
    .max(1);

  let mut virtual_format = text_format.clone();
  virtual_format.soft_wrap = spec.wrap_to_view;

  let rope = Rope::from(spec.text.as_str());
  let annotations = TextAnnotations::default();
  let mut formatter = DocumentFormatter::new(
    rope.slice(..),
    &virtual_format,
    available_width,
    &annotations,
  );

  let mut rows = Vec::<String>::new();
  for grapheme in &mut formatter {
    if grapheme.source.is_eof() {
      break;
    }

    while rows.len() <= grapheme.visual_pos.row {
      rows.push(String::new());
    }

    match grapheme.raw {
      Grapheme::Newline => {},
      Grapheme::Tab { width } => rows[grapheme.visual_pos.row].push_str(&" ".repeat(width)),
      Grapheme::Other { ref g } => rows[grapheme.visual_pos.row].push_str(g.as_ref()),
    }
  }

  if rows.is_empty() {
    rows.push(String::new());
  }

  let indent = " ".repeat(spec.col);
  for row in &mut rows {
    row.insert_str(0, &indent);
  }

  rows
}

fn contains_line_break(text: &str) -> bool {
  text.contains('\n') || text.contains('\r')
}

fn is_single_grapheme(text: &str) -> bool {
  let mut graphemes = UnicodeSegmentation::graphemes(text, true);
  graphemes.next().is_some() && graphemes.next().is_none()
}

#[cfg(test)]
mod tests {
  use ropey::Rope;

  use super::{
    GraphemeSubstitution,
    InlineAnnotation,
    TextAnnotationError,
    TextAnnotations,
    VirtualLineSpec,
  };
  use crate::{
    grapheme::Grapheme,
    render::{
      doc_formatter::DocumentFormatter,
      text_format::TextFormat,
    },
  };

  fn formatted_text(text: &str, annotations: &TextAnnotations) -> Vec<(usize, String)> {
    let rope = Rope::from(text);
    let format = TextFormat::default();
    let mut rows = Vec::<String>::new();

    for grapheme in DocumentFormatter::new(rope.slice(..), &format, 20, annotations) {
      if grapheme.source.is_eof() {
        break;
      }

      while rows.len() <= grapheme.visual_pos.row {
        rows.push(String::new());
      }

      match grapheme.raw {
        Grapheme::Newline => {},
        Grapheme::Tab { width } => rows[grapheme.visual_pos.row].push_str(&" ".repeat(width)),
        Grapheme::Other { ref g } => rows[grapheme.visual_pos.row].push_str(g.as_ref()),
      }
    }

    rows.into_iter().enumerate().collect()
  }

  #[test]
  fn inline_annotations_reject_line_breaks() {
    assert_eq!(
      InlineAnnotation::new(0, "a\n").unwrap_err(),
      TextAnnotationError::InlineAnnotationContainsLineBreak
    );
  }

  #[test]
  fn grapheme_substitutions_reject_multiple_graphemes() {
    assert_eq!(
      GraphemeSubstitution::new(0, "ab").unwrap_err(),
      TextAnnotationError::GraphemeSubstitutionMustBeSingleGrapheme
    );
  }

  #[test]
  fn annotations_sort_their_inputs() {
    let mut annotations = TextAnnotations::default();
    annotations
      .add_inline_annotation(InlineAnnotation::new(4, "b").unwrap())
      .add_inline_annotation(InlineAnnotation::new(1, "a").unwrap())
      .add_grapheme_substitution(GraphemeSubstitution::new(3, "x").unwrap())
      .add_grapheme_substitution(GraphemeSubstitution::new(2, "y").unwrap())
      .add_virtual_line(VirtualLineSpec::after(2).text("c"))
      .add_virtual_line(VirtualLineSpec::after(0).text("a"));

    assert_eq!(
      annotations
        .inline_annotations()
        .iter()
        .map(|annotation| annotation.char_idx)
        .collect::<Vec<_>>(),
      vec![1, 4]
    );
    assert_eq!(
      annotations
        .grapheme_substitutions()
        .iter()
        .map(|substitution| substitution.char_idx)
        .collect::<Vec<_>>(),
      vec![2, 3]
    );
    assert_eq!(
      annotations
        .virtual_lines()
        .iter()
        .map(|spec| spec.doc_line)
        .collect::<Vec<_>>(),
      vec![0, 2]
    );
  }

  #[test]
  fn virtual_lines_are_emitted_as_text_rows() {
    let mut annotations = TextAnnotations::default();
    annotations.add_virtual_line(VirtualLineSpec::after(0).text("ghost line").single_line());

    let rows = formatted_text("one\ntwo", &annotations);

    assert_eq!(rows, vec![
      (0, "one".to_string()),
      (1, "ghost line".to_string()),
      (2, "two".to_string())
    ]);
  }

  #[test]
  fn virtual_lines_respect_column_offsets() {
    let mut annotations = TextAnnotations::default();
    annotations.add_virtual_line(VirtualLineSpec::after(0).col(2).text("x").single_line());

    let rows = formatted_text("one\ntwo", &annotations);

    assert_eq!(rows[1].1, "  x");
  }
}
