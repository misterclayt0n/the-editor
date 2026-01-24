//! Document core state and transformation API.
//!
//! This module provides a minimal, deterministic document model for the-lib.
//! It owns the text buffer, selection, and history integration, and exposes
//! explicit state transitions via [`Transaction`] and history jumps.
//!
//! # Design
//!
//! - No IO, no LSP, no diagnostics, no background tasks.
//! - Pure state evolution: inputs in, state out.
//! - Selection mapping is explicit and uses [`ChangeSet`].
//!
//! # Example
//!
//! ```no_run
//! use ropey::Rope;
//! use std::num::NonZeroUsize;
//! use the_lib::document::{Document, DocumentId};
//! use the_lib::transaction::Transaction;
//!
//! let id = DocumentId::new(NonZeroUsize::new(1).unwrap());
//! let mut doc = Document::new(id, Rope::from("hello"));
//!
//! let tx = Transaction::change(doc.text(), vec![(5, 5, Some(" world".into()))]).unwrap();
//! doc.apply_transaction(&tx).unwrap();
//! doc.commit().unwrap();
//! ```

use std::num::NonZeroUsize;

use ropey::Rope;
use thiserror::Error;

use the_core::line_ending::{LineEnding, NATIVE_LINE_ENDING};

use crate::{
  history::{History, HistoryError, HistoryJump, State},
  indent::IndentStyle,
  selection::{Range, Selection, SelectionError},
  syntax::Syntax,
  transaction::{ChangeSet, Transaction, TransactionError},
  Tendril,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DocumentId(NonZeroUsize);

impl DocumentId {
  pub const fn new(id: NonZeroUsize) -> Self {
    Self(id)
  }

  pub const fn get(self) -> NonZeroUsize {
    self.0
  }
}

impl From<NonZeroUsize> for DocumentId {
  fn from(value: NonZeroUsize) -> Self {
    Self::new(value)
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ViewId(NonZeroUsize);

impl ViewId {
  pub const fn new(id: NonZeroUsize) -> Self {
    Self(id)
  }

  pub const fn get(self) -> NonZeroUsize {
    self.0
  }
}

impl From<NonZeroUsize> for ViewId {
  fn from(value: NonZeroUsize) -> Self {
    Self::new(value)
  }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct DocumentFlags {
  pub readonly: bool,
  pub modified: bool,
}

#[derive(Debug, Error)]
pub enum DocumentError {
  #[error("document is readonly")]
  Readonly,
  #[error(transparent)]
  Transaction(#[from] TransactionError),
  #[error(transparent)]
  Selection(#[from] SelectionError),
  #[error(transparent)]
  History(#[from] HistoryError),
}

pub type Result<T> = std::result::Result<T, DocumentError>;

#[derive(Debug)]
pub struct Document {
  id: DocumentId,
  text: Rope,
  selection: Selection,
  history: History,
  changes: ChangeSet,
  old_state: Option<State>,
  indent_style: IndentStyle,
  line_ending: LineEnding,
  version: u64,
  flags: DocumentFlags,
  syntax: Option<Syntax>,
}

impl Document {
  pub fn new(id: DocumentId, text: Rope) -> Self {
    let selection = Selection::point(0);
    let changes = ChangeSet::new(text.slice(..));
    Self {
      id,
      text,
      selection,
      history: History::default(),
      changes,
      old_state: None,
      indent_style: IndentStyle::Tabs,
      line_ending: NATIVE_LINE_ENDING,
      version: 0,
      flags: DocumentFlags::default(),
      syntax: None,
    }
  }

  pub fn id(&self) -> DocumentId {
    self.id
  }

  pub fn text(&self) -> &Rope {
    &self.text
  }

  pub fn selection(&self) -> &Selection {
    &self.selection
  }

  pub fn selection_mut(&mut self) -> &mut Selection {
    &mut self.selection
  }

  pub fn set_selection(&mut self, selection: Selection) -> Result<()> {
    self.selection = selection;
    Ok(())
  }

  pub fn indent_style(&self) -> IndentStyle {
    self.indent_style
  }

  pub fn set_indent_style(&mut self, indent_style: IndentStyle) {
    self.indent_style = indent_style;
  }

  pub fn line_ending(&self) -> LineEnding {
    self.line_ending
  }

  pub fn set_line_ending(&mut self, line_ending: LineEnding) {
    self.line_ending = line_ending;
  }

  pub fn version(&self) -> u64 {
    self.version
  }

  pub fn flags(&self) -> DocumentFlags {
    self.flags
  }

  pub fn set_readonly(&mut self, readonly: bool) {
    self.flags.readonly = readonly;
  }

  pub fn syntax(&self) -> Option<&Syntax> {
    self.syntax.as_ref()
  }

  pub fn set_syntax(&mut self, syntax: Syntax) {
    self.syntax = Some(syntax);
  }

  pub fn clear_syntax(&mut self) {
    self.syntax = None;
  }

  pub fn history(&self) -> &History {
    &self.history
  }

  pub fn apply_transaction(&mut self, transaction: &Transaction) -> Result<()> {
    if self.flags.readonly {
      return Err(DocumentError::Readonly);
    }

    if !transaction.changes().is_empty() && self.old_state.is_none() {
      self.old_state = Some(State {
        doc: self.text.clone(),
        selection: self.selection.clone(),
      });
    }

    transaction.apply(&mut self.text)?;

    self.selection = match transaction.selection() {
      Some(selection) => selection.clone(),
      None => self.selection.clone().map(transaction.changes())?,
    };

    let prior = std::mem::take(&mut self.changes);
    self.changes = prior.compose(transaction.changes().clone())?;

    if !transaction.changes().is_empty() {
      self.flags.modified = true;
      self.version = self.version.saturating_add(1);
    }

    Ok(())
  }

  pub fn replace_range(&mut self, range: Range, text: impl Into<Tendril>) -> Result<()> {
    let tx = Transaction::change(
      &self.text,
      vec![(range.from(), range.to(), Some(text.into()))],
    )?;
    self.apply_transaction(&tx)
  }

  pub fn commit(&mut self) -> Result<()> {
    if self.changes.is_empty() {
      self.old_state = None;
      return Ok(());
    }

    let Some(original) = self.old_state.take() else {
      return Ok(());
    };

    let tx = Transaction::from(self.changes.clone()).with_selection(self.selection.clone());
    self.history.commit_revision(&tx, &original)?;

    self.changes = ChangeSet::new(self.text.slice(..));
    self.flags.modified = false;
    Ok(())
  }

  pub fn undo(&mut self) -> Result<bool> {
    let Some(jump) = self.history.undo() else {
      return Ok(false);
    };
    self.apply_history_jump(&jump)?;
    self.history.apply_jump(&jump)?;
    Ok(true)
  }

  pub fn redo(&mut self) -> Result<bool> {
    let Some(jump) = self.history.redo() else {
      return Ok(false);
    };
    self.apply_history_jump(&jump)?;
    self.history.apply_jump(&jump)?;
    Ok(true)
  }

  fn apply_history_jump(&mut self, jump: &HistoryJump) -> Result<()> {
    for txn in &jump.transactions {
      txn.apply(&mut self.text)?;
      if let Some(sel) = txn.selection() {
        self.selection = sel.clone();
      } else {
        self.selection = self.selection.clone().map(txn.changes())?;
      }
    }

    self.changes = ChangeSet::new(self.text.slice(..));
    self.old_state = None;
    self.flags.modified = false;
    self.version = self.version.saturating_add(1);
    Ok(())
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::num::NonZeroUsize;

  #[test]
  fn apply_and_commit_transaction() {
    let id = DocumentId::new(NonZeroUsize::new(1).unwrap());
    let mut doc = Document::new(id, Rope::from("hello"));

    let tx = Transaction::change(doc.text(), vec![(5, 5, Some(" world".into()))]).unwrap();
    doc.apply_transaction(&tx).unwrap();
    doc.commit().unwrap();

    assert_eq!(doc.text().to_string(), "hello world");
    assert_eq!(doc.history().len(), 2);
    assert!(!doc.flags.modified);
  }

  #[test]
  fn undo_redo_roundtrip() {
    let id = DocumentId::new(NonZeroUsize::new(1).unwrap());
    let mut doc = Document::new(id, Rope::from("hello"));

    let tx = Transaction::change(doc.text(), vec![(5, 5, Some("!".into()))]).unwrap();
    doc.apply_transaction(&tx).unwrap();
    doc.commit().unwrap();

    assert_eq!(doc.text().to_string(), "hello!");

    assert!(doc.undo().unwrap());
    assert_eq!(doc.text().to_string(), "hello");

    assert!(doc.redo().unwrap());
    assert_eq!(doc.text().to_string(), "hello!");
  }

  #[test]
  fn selection_maps_through_transaction() {
    let id = DocumentId::new(NonZeroUsize::new(1).unwrap());
    let mut doc = Document::new(id, Rope::from("abc"));
    doc.set_selection(Selection::point(1)).unwrap();

    let tx = Transaction::change(doc.text(), vec![(0, 0, Some("x".into()))]).unwrap();
    doc.apply_transaction(&tx).unwrap();

    assert_eq!(doc.selection().ranges()[0].head, 2);
  }

  #[test]
  fn transaction_selection_overrides_mapping() {
    let id = DocumentId::new(NonZeroUsize::new(1).unwrap());
    let mut doc = Document::new(id, Rope::from("abc"));
    doc.set_selection(Selection::point(1)).unwrap();

    let selection = Selection::point(0);
    let tx = Transaction::change(doc.text(), vec![(2, 2, Some("x".into()))])
      .unwrap()
      .with_selection(selection.clone());
    doc.apply_transaction(&tx).unwrap();

    assert_eq!(doc.selection(), &selection);
  }
}
