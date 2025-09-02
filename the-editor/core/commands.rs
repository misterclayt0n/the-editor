use ropey::RopeSlice;

use crate::core::{
  document::Document,
  movement::{move_horizontally, Direction, Movement},
  selection::Range,
};

type MoveFn = fn(RopeSlice, Range, Direction, usize, Movement) -> Range;

fn move_impl_doc(doc: &mut Document, move_fn: MoveFn, dir: Direction, behavior: Movement) {
  let count = 1; // TODO: support counts
  let view_id = 0usize;

  let current_sel = doc
    .selection_ref(view_id)
    .cloned()
    .unwrap_or_else(|| crate::core::selection::Selection::point(0));

  let text = doc.text().slice(..);
  let new_selection = current_sel.transform(|range| move_fn(text, range, dir, count, behavior));
  doc.set_selection(view_id, new_selection);
}

pub fn move_char_left_doc(doc: &mut Document) {
  move_impl_doc(doc, move_horizontally, Direction::Backward, Movement::Move)
}

pub fn move_char_right_doc(doc: &mut Document) {
  move_impl_doc(doc, move_horizontally, Direction::Forward, Movement::Move)
}
