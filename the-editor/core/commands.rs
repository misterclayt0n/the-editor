use std::num::NonZeroUsize;

use ropey::RopeSlice;

use crate::{
  core::{
    Tendril,
    auto_pairs,
    movement::{
      self,
      Direction,
      Movement,
      move_horizontally,
      move_vertically,
    },
    selection::{
      Range,
      Selection,
    },
    text_annotations::TextAnnotations,
    text_format::TextFormat,
    transaction::Transaction,
  },
  current,
  current_ref,
  editor::Editor,
  event::PostInsertChar,
};

type MoveFn =
  fn(RopeSlice, Range, Direction, usize, Movement, &TextFormat, &mut TextAnnotations) -> Range;

pub struct Context<'a> {
  pub register: Option<char>,
  pub count:    Option<NonZeroUsize>,
  pub editor:   &'a mut Editor,
  // NOTE: We're ignoring these for now.
  // pub callback:             Vec<crate::compositor::Callback>,
  // pub on_next_key_callback: Option<(OnKeyCallback, OnKeyCallbackKind)>,
  // pub jobs:                 &'a mut Jobs,
}

impl Context<'_> {
  /// Returns 1 if no explicit count was provided.
  #[inline]
  pub fn count(&self) -> usize {
    self.count.map_or(1, |v| v.get())
  }
}

fn move_impl(cx: &mut Context, move_fn: MoveFn, dir: Direction, behavior: Movement) {
  let count = cx.count();
  let (view, doc) = current!(cx.editor);
  let slice = doc.text().slice(..);
  let text_fmt = doc.text_format(view.inner_area(doc).width, None);
  let mut annotations = view.text_annotations(doc, None);

  let selection = doc.selection(view.id).clone().transform(|range| {
    move_fn(
      slice,
      range,
      dir,
      count,
      behavior,
      &text_fmt,
      &mut annotations,
    )
  });

  drop(annotations);
  doc.set_selection(view.id, selection);
}

fn move_word_impl<F>(cx: &mut Context, move_fn: F)
where
  F: Fn(RopeSlice, Range, usize) -> Range,
{
  let count = cx.count();
  let (view, doc) = current!(cx.editor);
  let slice = doc.text().slice(..);
  let selection = doc
    .selection(view.id)
    .clone()
    .transform(|range| move_fn(slice, range, count));

  doc.set_selection(view.id, selection);
}

pub fn move_char_left(cx: &mut Context) {
  move_impl(cx, move_horizontally, Direction::Backward, Movement::Move)
}

pub fn move_char_right(cx: &mut Context) {
  move_impl(cx, move_horizontally, Direction::Forward, Movement::Move)
}

pub fn move_char_up(cx: &mut Context) {
  move_impl(cx, move_vertically, Direction::Backward, Movement::Move)
}

pub fn move_char_down(cx: &mut Context) {
  move_impl(cx, move_vertically, Direction::Forward, Movement::Move)
}

pub fn extend_char_left(cx: &mut Context) {
  move_impl(cx, move_horizontally, Direction::Backward, Movement::Extend)
}

pub fn extend_char_right(cx: &mut Context) {
  move_impl(cx, move_horizontally, Direction::Forward, Movement::Extend)
}

pub fn extend_char_up(cx: &mut Context) {
  move_impl(cx, move_vertically, Direction::Backward, Movement::Extend)
}

pub fn extend_char_down(cx: &mut Context) {
  move_impl(cx, move_vertically, Direction::Forward, Movement::Extend)
}

/// Delete the selection if non-empty; otherwise delete one grapheme backward.
// pub fn delete_backward(doc: &mut Document) {
//   let view_id = 0usize;
//   let selection = doc
//     .selection_ref(view_id)
//     .cloned()
//     .unwrap_or_else(|| crate::core::selection::Selection::point(0));

//   let rope = doc.text();
//   let txn = Transaction::delete_by_selection(rope, &selection, |range:
// &Range| {     if range.is_empty() {
//       let slice = rope.slice(..);
//       let start = crate::core::grapheme::prev_grapheme_boundary(slice,
// range.head);       (start, range.head)
//     } else {
//       (range.from(), range.to())
//     }
//   });
//   doc.apply(view_id, &txn);
// }

pub fn move_next_word_start(cx: &mut Context) {
  move_word_impl(cx, movement::move_next_word_start)
}

pub fn move_prev_word_start(cx: &mut Context) {
  move_word_impl(cx, movement::move_prev_word_start)
}

pub fn move_prev_word_end(cx: &mut Context) {
  move_word_impl(cx, movement::move_prev_word_end)
}

pub fn move_next_word_end(cx: &mut Context) {
  move_word_impl(cx, movement::move_next_word_end)
}

pub fn move_next_long_word_start(cx: &mut Context) {
  move_word_impl(cx, movement::move_next_long_word_start)
}

pub fn move_prev_long_word_start(cx: &mut Context) {
  move_word_impl(cx, movement::move_prev_long_word_start)
}

pub fn move_prev_long_word_end(cx: &mut Context) {
  move_word_impl(cx, movement::move_prev_long_word_end)
}

pub fn move_next_long_word_end(cx: &mut Context) {
  move_word_impl(cx, movement::move_next_long_word_end)
}

pub fn move_next_sub_word_start(cx: &mut Context) {
  move_word_impl(cx, movement::move_next_sub_word_start)
}

pub fn move_prev_sub_word_start(cx: &mut Context) {
  move_word_impl(cx, movement::move_prev_sub_word_start)
}

pub fn move_prev_sub_word_end(cx: &mut Context) {
  move_word_impl(cx, movement::move_prev_sub_word_end)
}

pub fn move_next_sub_word_end(cx: &mut Context) {
  move_word_impl(cx, movement::move_next_sub_word_end)
}

pub fn delete_char_backward(cx: &mut Context) {
  insert::delete_char_backward_impl(cx);
}

pub mod insert {
  use std::borrow::Cow;

  use ropey::Rope;
  use unicode_width::UnicodeWidthChar;

  use super::*;
  use crate::core::grapheme::{
    nth_next_grapheme_boundary,
    nth_prev_grapheme_boundary,
  };

  fn insert(rope: &Rope, selection: &Selection, ch: char) -> Option<Transaction> {
    let cursors = selection.clone().cursors(rope.slice(..));
    let mut t = Tendril::new();
    t.push(ch);
    let transaction = Transaction::insert(rope, &cursors, t);
    Some(transaction)
  }

  pub fn insert_char(cx: &mut Context, c: char) {
    let (view, doc) = current_ref!(cx.editor);
    let text = doc.text();
    let selection = doc.selection(view.id);
    let auto_pairs = doc.auto_pairs(cx.editor);

    let transaction = auto_pairs
      .as_ref()
      .and_then(|ap| auto_pairs::hook(text, selection, c, ap))
      .or_else(|| insert(text, selection, c));

    let (view, doc) = current!(cx.editor);
    if let Some(t) = transaction {
      doc.apply(&t, view.id);
    }

    the_editor_event::dispatch(PostInsertChar { c, cx });
  }

  pub fn delete_char_backward_impl(cx: &mut Context) {
    let count = cx.count();
    let (view, doc) = current_ref!(cx.editor);
    let text = doc.text().slice(..);
    let tab_width = doc.tab_width();
    let indent_width = doc.indent_width();
    let auto_pairs = doc.auto_pairs(cx.editor);

    let transaction =
      Transaction::delete_by_selection(doc.text(), doc.selection(view.id), |range| {
        let pos = range.cursor(text);
        if pos == 0 {
          return (pos, pos);
        }
        let line_start_pos = text.line_to_char(range.cursor_line(text));
        // consider to delete by indent level if all characters before `pos` are indent
        // units.
        let fragment = Cow::from(text.slice(line_start_pos..pos));
        if !fragment.is_empty() && fragment.chars().all(|ch| ch == ' ' || ch == '\t') {
          if text.get_char(pos.saturating_sub(1)) == Some('\t') {
            // fast path, delete one char
            (nth_prev_grapheme_boundary(text, pos, 1), pos)
          } else {
            let width: usize = fragment
              .chars()
              .map(|ch| {
                if ch == '\t' {
                  tab_width
                } else {
                  // it can be none if it still meet control characters other than '\t'
                  // here just set the width to 1 (or some value better?).
                  ch.width().unwrap_or(1)
                }
              })
              .sum();
            let mut drop = width % indent_width; // round down to nearest unit
            if drop == 0 {
              drop = indent_width
            }; // if it's already at a unit, consume a whole unit
            let mut chars = fragment.chars().rev();
            let mut start = pos;
            for _ in 0..drop {
              // delete up to `drop` spaces
              match chars.next() {
                Some(' ') => start -= 1,
                _ => break,
              }
            }
            (start, pos) // delete!
          }
        } else {
          match (
            text.get_char(pos.saturating_sub(1)),
            text.get_char(pos),
            auto_pairs,
          ) {
            (Some(_x), Some(_y), Some(ap))
              if range.is_single_grapheme(text)
                && ap.get(_x).is_some()
                && ap.get(_x).unwrap().open == _x
                && ap.get(_x).unwrap().close == _y =>
            // delete both autopaired characters
            {
              (
                nth_prev_grapheme_boundary(text, pos, count),
                nth_next_grapheme_boundary(text, pos, count),
              )
            },
            _ =>
            // delete 1 char
            {
              (nth_prev_grapheme_boundary(text, pos, count), pos)
            },
          }
        }
      });
    let (view, doc) = current!(cx.editor);
    doc.apply(&transaction, view.id);
  }
}
