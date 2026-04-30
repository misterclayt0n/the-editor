use ropey::RopeSlice;
use thiserror::Error;
use unicode_segmentation::UnicodeSegmentation;

use crate::Tendril;

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

/// Owned layout annotations consumed by the render kernel.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct TextAnnotations {
  inline_annotations:     Vec<InlineAnnotation>,
  grapheme_substitutions: Vec<GraphemeSubstitution>,
}

impl TextAnnotations {
  pub fn is_empty(&self) -> bool {
    self.inline_annotations.is_empty() && self.grapheme_substitutions.is_empty()
  }

  pub fn inline_annotations(&self) -> &[InlineAnnotation] {
    &self.inline_annotations
  }

  pub fn grapheme_substitutions(&self) -> &[GraphemeSubstitution] {
    &self.grapheme_substitutions
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

  pub(crate) fn cursor_at<'a>(
    &'a self,
    _text: RopeSlice<'_>,
    char_idx: usize,
  ) -> TextAnnotationsCursor<'a> {
    let mut cursor = TextAnnotationsCursor::new(self);
    cursor.reset_pos(char_idx);
    cursor
  }
}

#[derive(Debug)]
pub(crate) struct TextAnnotationsCursor<'a> {
  inline_annotations:          &'a [InlineAnnotation],
  inline_index:                usize,
  grapheme_substitutions:      &'a [GraphemeSubstitution],
  grapheme_substitution_index: usize,
}

impl<'a> TextAnnotationsCursor<'a> {
  fn new(annotations: &'a TextAnnotations) -> Self {
    Self {
      inline_annotations:          &annotations.inline_annotations,
      inline_index:                0,
      grapheme_substitutions:      &annotations.grapheme_substitutions,
      grapheme_substitution_index: 0,
    }
  }

  fn reset_pos(&mut self, char_idx: usize) {
    self.inline_index = self
      .inline_annotations
      .partition_point(|annotation| annotation.char_idx < char_idx);
    self.grapheme_substitution_index = self
      .grapheme_substitutions
      .partition_point(|substitution| substitution.char_idx < char_idx);
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
}

fn contains_line_break(text: &str) -> bool {
  text.contains(['\n', '\r'])
}

fn is_single_grapheme(text: &str) -> bool {
  let mut graphemes = UnicodeSegmentation::graphemes(text, true);
  graphemes.next().is_some() && graphemes.next().is_none()
}

#[cfg(test)]
mod tests {
  use super::{
    GraphemeSubstitution,
    InlineAnnotation,
    TextAnnotationError,
    TextAnnotations,
  };

  #[test]
  fn inline_annotations_sort_by_char_idx() {
    let mut annotations = TextAnnotations::default();
    annotations
      .add_inline_annotation(InlineAnnotation::new(3, "c").unwrap())
      .add_inline_annotation(InlineAnnotation::new(1, "a").unwrap());

    assert_eq!(annotations.inline_annotations()[0].char_idx, 1);
    assert_eq!(annotations.inline_annotations()[1].char_idx, 3);
  }

  #[test]
  fn grapheme_substitutions_sort_by_char_idx() {
    let mut annotations = TextAnnotations::default();
    annotations
      .add_grapheme_substitution(GraphemeSubstitution::new(3, "c").unwrap())
      .add_grapheme_substitution(GraphemeSubstitution::new(1, "a").unwrap());

    assert_eq!(annotations.grapheme_substitutions()[0].char_idx, 1);
    assert_eq!(annotations.grapheme_substitutions()[1].char_idx, 3);
  }

  #[test]
  fn inline_annotation_rejects_line_breaks() {
    let err = InlineAnnotation::new(0, "x\ny").unwrap_err();
    assert_eq!(err, TextAnnotationError::InlineAnnotationContainsLineBreak);
  }

  #[test]
  fn substitution_rejects_line_breaks() {
    let err = GraphemeSubstitution::new(0, "x\ny").unwrap_err();
    assert_eq!(
      err,
      TextAnnotationError::GraphemeSubstitutionContainsLineBreak
    );
  }

  #[test]
  fn substitution_rejects_multiple_graphemes() {
    let err = GraphemeSubstitution::new(0, "xy").unwrap_err();
    assert_eq!(
      err,
      TextAnnotationError::GraphemeSubstitutionMustBeSingleGrapheme
    );
  }
}
