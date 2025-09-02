use ropey::RopeSlice;

use crate::core::{
    document::Document,
    movement::{
      Direction,
      Movement,
      move_horizontally,
      move_vertically,
    },
    selection::Range,
  };

type MoveFn = fn(RopeSlice, Range, Direction, usize, Movement) -> Range;

fn move_impl(doc: &mut Document, move_fn: MoveFn, dir: Direction, behavior: Movement) {
  let count = 1; // TODO: support counts with context system.
  let view_id = 0usize; 

  let current_sel = doc
    .selection_ref(view_id)
    .cloned()
    .unwrap_or_else(|| crate::core::selection::Selection::point(0));

  let text = doc.text().slice(..);
  let new_selection = current_sel.transform(|range| move_fn(text, range, dir, count, behavior));
  doc.set_selection(view_id, new_selection);
}

pub fn move_char_left(doc: &mut Document) {
  move_impl(doc, move_horizontally, Direction::Backward, Movement::Move)
}

pub fn move_char_right(doc: &mut Document) {
  move_impl(doc, move_horizontally, Direction::Forward, Movement::Move)
}

pub fn move_char_up(doc: &mut Document) {
  move_impl(doc, move_vertically, Direction::Backward, Movement::Move)
}

pub fn move_char_down(doc: &mut Document) {
  move_impl(doc, move_vertically, Direction::Forward, Movement::Move)
}
