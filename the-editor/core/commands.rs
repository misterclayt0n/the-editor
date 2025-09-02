use ropey::RopeSlice;

use crate::core::{
  document::Document,
  movement::{
    self,
    Direction,
    Movement,
  },
  selection::Range,
  text_annotations::TextAnnotations,
  text_format::TextFormat,
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
