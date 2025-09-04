use std::collections::HashMap;

use ropey::Rope;

use crate::core::{
  selection::{
    Range,
    Selection,
  },
  transaction::{
    Assoc,
    Transaction,
  },
};

pub type ViewId = usize;

pub struct Document {
  text:       Rope,
  selections: HashMap<ViewId, Selection>,
}

impl Document {
  pub fn new() -> Self {
    Self {
      text:       Rope::new(),
      selections: HashMap::default(),
    }
  }

  pub fn with_text<S: AsRef<str>>(s: S) -> Self {
    Self {
      text:       Rope::from_str(s.as_ref()),
      selections: HashMap::default(),
    }
  }

  pub fn set_text<S: AsRef<str>>(&mut self, s: S) {
    self.text = Rope::from_str(s.as_ref());
  }

  pub fn text(&self) -> &Rope {
    &self.text
  }

  /// Get or initialize the selection for a view.
  pub fn selection(&mut self, view: ViewId) -> &Selection {
    self
      .selections
      .entry(view)
      .or_insert_with(|| Selection::point(0))
  }

  pub fn selection_ref(&self, view: ViewId) -> Option<&Selection> {
    self.selections.get(&view)
  }

  pub fn set_selection(&mut self, view: ViewId, selection: Selection) {
    self.selections.insert(view, selection);
  }

  /// Apply a Transaction to the document text and remap the view's selection.
  pub fn apply(&mut self, view: ViewId, transaction: &Transaction) {
    let changes = transaction.changes().clone();
    let old_selection = self
      .selection_ref(view)
      .cloned()
      .unwrap_or_else(|| Selection::point(0));

    // Apply text edits to the underlying Rope
    let success = transaction.apply(&mut self.text);
    if !success {
      eprintln!(
        "Transaction failed to apply! Document len: {}, Transaction expected len: {}",
        self.text.len_chars(),
        transaction.changes().len()
      );
    }

    // If the transaction explicitly set a selection, honor it.
    if let Some(sel) = transaction.selection() {
      self.set_selection(view, sel.clone());
      return;
    }

    // Otherwise, map the previous selection through the changes.
    let new_ranges: Vec<Range> = old_selection
      .ranges()
      .iter()
      .map(|r| {
        let new_head = changes.map_pos(r.head, Assoc::After);
        let new_anchor = changes.map_pos(r.anchor, Assoc::After);
        Range::new(new_head, new_anchor)
      })
      .collect();

    let new_selection = Selection::new(
      smallvec::SmallVec::from_vec(new_ranges),
      old_selection.primary_index(),
    );
    self.set_selection(view, new_selection);
  }
}
