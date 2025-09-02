use std::collections::HashMap;

use ropey::Rope;

use crate::core::selection::Selection;

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
}
