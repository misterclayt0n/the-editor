use ropey::Rope;
use thiserror::Error;

use crate::{
  indent::{
    IndentStyle,
    auto_detect_indent_style,
  },
  line_ending::{
    LineEnding,
    NATIVE_LINE_ENDING,
    auto_detect_line_ending,
  },
  syntax::{
    DocumentSyntax,
    DocumentSyntaxState,
    SyntaxState,
    generate_edits,
  },
  transaction::{
    Transaction,
    TransactionError,
  },
};

#[derive(Debug, Error)]
pub enum DocumentError {
  #[error(transparent)]
  Transaction(#[from] TransactionError),
}

pub type Result<T> = std::result::Result<T, DocumentError>;

#[derive(Debug, Clone)]
pub struct Document {
  text:         Rope,
  revision:     u64,
  syntax:       DocumentSyntax,
  line_ending:  LineEnding,
  indent_style: IndentStyle,
}

impl Document {
  pub fn new(text: Rope) -> Self {
    Self {
      line_ending: auto_detect_line_ending(&text).unwrap_or(NATIVE_LINE_ENDING),
      indent_style: auto_detect_indent_style(&text).unwrap_or(IndentStyle::Tabs),
      text,
      revision: 0,
      syntax: DocumentSyntax::Unassigned,
    }
  }

  pub fn text(&self) -> &Rope {
    &self.text
  }

  pub fn revision(&self) -> u64 {
    self.revision
  }

  pub fn syntax(&self) -> &DocumentSyntax {
    &self.syntax
  }

  pub fn set_syntax(&mut self, syntax: DocumentSyntax) {
    self.syntax = syntax;
  }

  pub fn clear_syntax(&mut self) {
    self.syntax = DocumentSyntax::Unassigned;
  }

  pub fn line_ending(&self) -> LineEnding {
    self.line_ending
  }

  pub fn set_line_ending(&mut self, line_ending: LineEnding) {
    self.line_ending = line_ending;
  }

  pub fn indent_style(&self) -> IndentStyle {
    self.indent_style
  }

  pub fn set_indent_style(&mut self, indent_style: IndentStyle) {
    self.indent_style = indent_style;
  }

  pub fn refresh_text_metadata(&mut self) {
    if let Some(line_ending) = auto_detect_line_ending(&self.text) {
      self.line_ending = line_ending;
    }
    if let Some(indent_style) = auto_detect_indent_style(&self.text) {
      self.indent_style = indent_style;
    }
  }

  pub fn apply_transaction(&mut self, transaction: &Transaction) -> Result<()> {
    if transaction.changes().is_empty() {
      return Ok(());
    }

    let old_text = self.text.clone();
    transaction.apply(&mut self.text)?;
    self.revision = self.revision.saturating_add(1);
    self.interpolate_syntax(old_text, transaction);

    Ok(())
  }

  fn interpolate_syntax(&mut self, old_text: Rope, transaction: &Transaction) {
    let Some(state) = active_syntax_state(&self.syntax).cloned() else {
      return;
    };

    let edits = generate_edits(old_text.slice(..), transaction.changes());
    if edits.is_empty() {
      return;
    }

    state.interpolate(&edits);
    state.set_parse_pending(true);
  }
}

fn active_syntax_state(syntax: &DocumentSyntax) -> Option<&SyntaxState> {
  match syntax {
    DocumentSyntax::Assigned {
      state: DocumentSyntaxState::Active(state),
      ..
    } => Some(state),
    DocumentSyntax::Unassigned | DocumentSyntax::Assigned { .. } => None,
  }
}

#[cfg(test)]
mod tests {
  use std::borrow::Cow;

  use ropey::Rope;
  use tree_house::tree_sitter::Grammar;

  use super::*;
  use crate::{
    syntax::{
      FileType,
      LanguageRegistry,
      LanguageSyntaxConfig,
      QueryKind,
      SyntaxConfiguration,
      SyntaxResources,
    },
    transaction::Transaction,
  };

  #[derive(Debug, Clone, Copy)]
  struct JsonResources {
    grammar: Grammar,
  }

  impl SyntaxResources for JsonResources {
    fn grammar(&self, grammar_name: &str) -> Option<Grammar> {
      (grammar_name == "json").then_some(self.grammar)
    }

    fn query(&self, _language_id: &str, _kind: QueryKind) -> Option<Cow<'_, str>> {
      None
    }
  }

  fn json_registry() -> LanguageRegistry {
    let grammar = Grammar::try_from(tree_sitter_json::LANGUAGE).unwrap();
    LanguageRegistry::new(
      SyntaxConfiguration {
        language: vec![LanguageSyntaxConfig {
          language_id:          "json".into(),
          scope:                "source.json".into(),
          file_types:           vec![FileType::Extension("json".into())],
          shebangs:             Vec::new(),
          comment_tokens:       None,
          block_comment_tokens: None,
          text_width:           None,
          soft_wrap:            None,
          grammar:              Some("json".into()),
          injection_regex:      None,
          indent:               None,
          auto_pairs:           None,
          rulers:               None,
          rainbow_brackets:     None,
        }],
      },
      JsonResources { grammar },
    )
    .unwrap()
  }

  #[test]
  fn new_document_initializes_text_revision_and_metadata() {
    let document = Document::new(Rope::from("fn main() {\n  x();\n}\n"));

    assert_eq!(document.text().to_string(), "fn main() {\n  x();\n}\n");
    assert_eq!(document.revision(), 0);
    assert_eq!(document.line_ending(), LineEnding::LF);
    assert_eq!(document.indent_style(), IndentStyle::Spaces(2));
    assert!(matches!(document.syntax(), DocumentSyntax::Unassigned));
  }

  #[test]
  fn apply_transaction_updates_text_and_revision() {
    let mut document = Document::new(Rope::from("hello"));
    let tx = Transaction::insert_at(document.text().len_chars(), 5, " world").unwrap();

    document.apply_transaction(&tx).unwrap();

    assert_eq!(document.text().to_string(), "hello world");
    assert_eq!(document.revision(), 1);
  }

  #[test]
  fn empty_transaction_does_not_bump_revision() {
    let mut document = Document::new(Rope::from("hello"));
    let tx = Transaction::empty(document.text().len_chars());

    document.apply_transaction(&tx).unwrap();

    assert_eq!(document.revision(), 0);
  }

  #[test]
  fn apply_transaction_interpolates_active_syntax_and_marks_parse_pending() {
    let registry = json_registry();
    let mut document = Document::new(Rope::from("[1,2,3]"));
    let assignment = registry
      .language_for_name("json")
      .map(|language_id| {
        crate::syntax::LanguageAssignment {
          language_id,
          source: crate::syntax::LanguageAssignmentSource::Name,
        }
      })
      .unwrap();
    let syntax = registry.activate_document_syntax(document.text().slice(..), assignment);
    document.set_syntax(syntax);

    let tx = Transaction::insert_at(document.text().len_chars(), 2, ",4").unwrap();
    document.apply_transaction(&tx).unwrap();

    let state = match document.syntax() {
      DocumentSyntax::Assigned {
        state: DocumentSyntaxState::Active(state),
        ..
      } => state,
      _ => panic!("expected active syntax"),
    };

    assert_eq!(state.interpolated_version(), 1);
    assert!(state.is_parse_pending());
  }

  #[test]
  fn syntax_can_be_cleared_and_metadata_can_be_refreshed() {
    let mut document = Document::new(Rope::from("\tlet x = 1;\r\n"));
    document.clear_syntax();
    document.set_line_ending(LineEnding::LF);
    document.set_indent_style(IndentStyle::Spaces(2));
    document.refresh_text_metadata();

    assert_eq!(document.line_ending(), LineEnding::Crlf);
    assert_eq!(document.indent_style(), IndentStyle::Tabs);
  }
}
