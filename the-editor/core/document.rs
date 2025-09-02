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
}
