use ropey::RopeSlice;

use crate::core::{
  Tendril,
  document::Document,
  movement::{
    self,
    Direction,
    Movement,
  },
  selection::Range,
  text_annotations::TextAnnotations,
  text_format::TextFormat,
  transaction::Transaction,
};

type MoveFn =
  fn(RopeSlice, Range, Direction, usize, Movement, &TextFormat, &mut TextAnnotations) -> Range;

fn move_impl(doc: &mut Document, move_fn: MoveFn, dir: Direction, behavior: Movement) {
  let count = 1; // TODO: support counts with context system.
  let view_id = 0usize;
  let text = doc.text().slice(..);
  // For now, generate a default TextFormat and empty annotations. In the
  // future these can be sourced from the active view/layout.
  let text_fmt = TextFormat::default();
  let mut annotations = TextAnnotations::default();

  let current_sel = doc
    .selection_ref(view_id)
    .cloned()
    .unwrap_or_else(|| crate::core::selection::Selection::point(0));

  let new_selection = current_sel.transform(|range| {
    move_fn(
      text,
      range,
      dir,
      count,
      behavior,
      &text_fmt,
      &mut annotations,
    )
  });
  doc.set_selection(view_id, new_selection);
}

// Wrapper shims to adapt movement functions to the unified MoveFn signature.
fn move_horizontally_fmt(
  slice: RopeSlice,
  range: Range,
  dir: Direction,
  count: usize,
  behavior: Movement,
  _text_fmt: &TextFormat,
  _annotations: &mut TextAnnotations,
) -> Range {
  movement::move_horizontally(slice, range, dir, count, behavior)
}

fn move_vertically_fmt(
  slice: RopeSlice,
  range: Range,
  dir: Direction,
  count: usize,
  behavior: Movement,
  text_fmt: &TextFormat,
  annotations: &mut TextAnnotations,
) -> Range {
  movement::move_vertically(slice, range, dir, count, behavior, text_fmt, annotations)
}

pub fn move_char_left(doc: &mut Document) {
  move_impl(
    doc,
    move_horizontally_fmt,
    Direction::Backward,
    Movement::Move,
  )
}

pub fn move_char_right(doc: &mut Document) {
  move_impl(
    doc,
    move_horizontally_fmt,
    Direction::Forward,
    Movement::Move,
  )
}

pub fn move_char_up(doc: &mut Document) {
  move_impl(
    doc,
    move_vertically_fmt,
    Direction::Backward,
    Movement::Move,
  )
}

pub fn move_char_down(doc: &mut Document) {
  move_impl(doc, move_vertically_fmt, Direction::Forward, Movement::Move)
}

/// Insert a string at each selection head.
pub fn insert_text(doc: &mut Document, s: &str) {
  let view_id = 0usize;
  let selection = doc
    .selection_ref(view_id)
    .cloned()
    .unwrap_or_else(|| crate::core::selection::Selection::point(0));

  let txn = Transaction::insert(doc.text(), &selection, Tendril::from(s));
  doc.apply(view_id, &txn);
}

/// Delete the selection if non-empty; otherwise delete one grapheme backward.
pub fn delete_backward(doc: &mut Document) {
  let view_id = 0usize;
  let selection = doc
    .selection_ref(view_id)
    .cloned()
    .unwrap_or_else(|| crate::core::selection::Selection::point(0));

  let rope = doc.text();
  let txn = Transaction::delete_by_selection(rope, &selection, |range: &Range| {
    if range.is_empty() {
      let slice = rope.slice(..);
      let start = crate::core::grapheme::prev_grapheme_boundary(slice, range.head);
      (start, range.head)
    } else {
      (range.from(), range.to())
    }
  });
  doc.apply(view_id, &txn);
}
