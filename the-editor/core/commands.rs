use std::{
  borrow::Cow,
  num::NonZeroUsize,
};

use ropey::{
  Rope,
  RopeSlice,
};
use smallvec::SmallVec;
use the_editor_stdx::rope::RopeSliceExt;

use crate::{
  core::{
    Tendril,
    auto_pairs,
    comment,
    grapheme,
    indent,
    line_ending::line_end_char_index,
    movement::{
      self,
      Direction,
      Movement,
      move_horizontally,
      move_vertically,
      move_vertically_visual,
    },
    position::char_idx_at_visual_offset,
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
  keymap::Mode,
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

enum Operation {
  Delete,
  Change,
}

enum YankAction {
  Yank,
  NoYank,
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

pub fn move_visual_line_up(cx: &mut Context) {
  move_impl(
    cx,
    move_vertically_visual,
    Direction::Backward,
    Movement::Move,
  )
}

pub fn move_char_down(cx: &mut Context) {
  move_impl(cx, move_vertically, Direction::Forward, Movement::Move)
}

pub fn move_visual_line_down(cx: &mut Context) {
  move_impl(
    cx,
    move_vertically_visual,
    Direction::Forward,
    Movement::Move,
  )
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

pub fn extend_visual_line_up(cx: &mut Context) {
  move_impl(
    cx,
    move_vertically_visual,
    Direction::Backward,
    Movement::Extend,
  )
}

pub fn extend_char_down(cx: &mut Context) {
  move_impl(cx, move_vertically, Direction::Forward, Movement::Extend)
}

pub fn extend_visual_line_down(cx: &mut Context) {
  move_impl(
    cx,
    move_vertically_visual,
    Direction::Forward,
    Movement::Extend,
  )
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

pub fn scroll(cx: &mut Context, offset: usize, direction: Direction, sync_cursor: bool) {
  use Direction::*;

  let config = cx.editor.config();
  let (view, doc) = current!(cx.editor);
  let mut view_offset = doc.view_offset(view.id);

  let range = doc.selection(view.id).primary();
  let cursor = {
    let text = doc.text().slice(..);
    range.cursor(text)
  };
  let height = view.inner_height();

  let scrolloff = config.scrolloff.min(height.saturating_sub(1) / 2);
  let offset = match direction {
    Forward => offset as isize,
    Backward => -(offset as isize),
  };

  let viewport = view.inner_area(doc);
  let text_fmt = doc.text_format(viewport.width, None);
  {
    let doc_text = doc.text().slice(..);
    (view_offset.anchor, view_offset.vertical_offset) = char_idx_at_visual_offset(
      doc_text,
      view_offset.anchor,
      view_offset.vertical_offset as isize + offset,
      0,
      &text_fmt,
      &view.text_annotations(&*doc, None),
    );
  }
  doc.set_view_offset(view.id, view_offset);

  let doc_text = doc.text().slice(..);
  let mut annotations = view.text_annotations(&*doc, None);

  if sync_cursor {
    let movement = match cx.editor.mode {
      Mode::Select => Movement::Extend,
      _ => Movement::Move,
    };
    let selection = doc.selection(view.id).clone().transform(|range| {
      move_vertically_visual(
        doc_text,
        range,
        direction,
        offset.unsigned_abs(),
        movement,
        &text_fmt,
        &mut annotations,
      )
    });
    drop(annotations);
    doc.set_selection(view.id, selection);
    return;
  }

  let view_offset = doc.view_offset(view.id);

  let mut head;
  match direction {
    Forward => {
      let off;
      (head, off) = char_idx_at_visual_offset(
        doc_text,
        view_offset.anchor,
        (view_offset.vertical_offset + scrolloff) as isize,
        0,
        &text_fmt,
        &annotations,
      );
      head += (off != 0) as usize;
      if head <= cursor {
        return;
      }
    },
    Backward => {
      head = char_idx_at_visual_offset(
        doc_text,
        view_offset.anchor,
        (view_offset.vertical_offset + height - scrolloff - 1) as isize,
        0,
        &text_fmt,
        &annotations,
      )
      .0;
      if head >= cursor {
        return;
      }
    },
  }

  let anchor = if cx.editor.mode == Mode::Select {
    range.anchor
  } else {
    head
  };

  let prim_sel = Range::new(anchor, head);
  let mut sel = doc.selection(view.id).clone();
  let idx = sel.primary_index();
  sel = sel.replace(idx, prim_sel);
  drop(annotations);
  doc.set_selection(view.id, sel);
}

pub fn delete_char_backward(cx: &mut Context) {
  insert::delete_char_backward_impl(cx);
}

fn delete_selection_impl(cx: &mut Context, op: Operation, yank: YankAction) {
  let (view, doc) = current!(cx.editor);
  let selection = doc.selection(view.id);
  let only_whole_lines = selection_is_linewise(selection, doc.text());

  if cx.register != Some('_') && matches!(yank, YankAction::Yank) {
    // yank the selection
    let text = doc.text().slice(..);
    let values: Vec<String> = selection.fragments(text).map(Cow::into_owned).collect();
    let reg_name = cx
      .register
      .unwrap_or_else(|| cx.editor.config.load().default_yank_register);

    if let Err(err) = cx.editor.registers.write(reg_name, values) {
      cx.editor.set_error(err.to_string());
      return;
    }
  }

  let transaction =
    Transaction::delete_by_selection(doc.text(), selection, |range| (range.from(), range.to()));
  doc.apply(&transaction, view.id);

  match op {
    Operation::Delete => {
      // exit select mode, if currently in select mode
      exit_select_mode(cx);
    },
    Operation::Change => {
      if only_whole_lines {
        open(cx, Open::Above, CommentContinuation::Disabled);
      } else {
        enter_insert_mode(cx);
      }
    },
  }
}

pub fn delete_selection(cx: &mut Context) {
  delete_selection_impl(cx, Operation::Delete, YankAction::Yank);
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

fn selection_is_linewise(selection: &Selection, text: &Rope) -> bool {
  selection.ranges().iter().all(|range| {
    let text = text.slice(..);
    if range.slice(text).len_lines() < 2 {
      return false;
    }

    // If the start of the selection is at the start of a line and the end at the
    // end of a line.
    let (start_line, end_line) = range.line_range(text);
    let start = text.line_to_char(start_line);
    let end = text.line_to_char((end_line + 1).min(text.len_lines()));
    start == range.from() && end == range.to()
  })
}

// Mode switching
//

fn exit_select_mode(cx: &mut Context) {
  if cx.editor.mode == Mode::Select {
    cx.editor.mode = Mode::Normal;
  }
}

fn enter_insert_mode(cx: &mut Context) {
  cx.editor.mode = Mode::Insert;
}

// Inserts at the start of each selection.
fn insert_mode(cx: &mut Context) {
  enter_insert_mode(cx);
  let (view, doc) = current!(cx.editor);

  log::trace!(
    "entering insert mode with sel: {:?}, text: {:?}",
    doc.selection(view.id),
    doc.text().to_string()
  );

  let selection = doc
    .selection(view.id)
    .clone()
    .transform(|range| Range::new(range.to(), range.from()));

  doc.set_selection(view.id, selection);
}

#[derive(PartialEq, Eq)]
pub enum Open {
  Below,
  Above,
}

#[derive(PartialEq)]
pub enum CommentContinuation {
  Enabled,
  Disabled,
}

fn open(cx: &mut Context, open: Open, comment_continuation: CommentContinuation) {
  let count = cx.count();
  enter_insert_mode(cx);
  let config = cx.editor.config();
  let (view, doc) = current!(cx.editor);
  let loader = cx.editor.syn_loader.load();

  let text = doc.text().slice(..);
  let contents = doc.text();
  let selection = doc.selection(view.id);
  let mut offs = 0;

  let mut ranges = SmallVec::with_capacity(selection.len());

  let continue_comment_tokens =
    if comment_continuation == CommentContinuation::Enabled && config.continue_comments {
      doc
        .language_config()
        .and_then(|config| config.comment_tokens.as_ref())
    } else {
      None
    };

  let mut transaction = Transaction::change_by_selection(contents, selection, |range| {
    // the line number, where the cursor is currently
    let curr_line_num = text.char_to_line(match open {
      Open::Below => grapheme::prev_grapheme_boundary(text, range.to()),
      Open::Above => range.from(),
    });

    // the next line number, where the cursor will be, after finishing the
    // transaction
    let next_new_line_num = match open {
      Open::Below => curr_line_num + 1,
      Open::Above => curr_line_num,
    };

    let above_next_new_line_num = next_new_line_num.saturating_sub(1);

    let continue_comment_token = continue_comment_tokens
      .and_then(|tokens| comment::get_comment_token(text, tokens, curr_line_num));

    // Index to insert newlines after, as well as the char width
    // to use to compensate for those inserted newlines.
    let (above_next_line_end_index, above_next_line_end_width) = if next_new_line_num == 0 {
      (0, 0)
    } else {
      (
        line_end_char_index(&text, above_next_new_line_num),
        doc.line_ending.len_chars(),
      )
    };

    let line = text.line(curr_line_num);
    let indent = match line.first_non_whitespace_char() {
      Some(pos) if continue_comment_token.is_some() => line.slice(..pos).to_string(),
      _ => {
        indent::indent_for_newline(
          &loader,
          doc.syntax(),
          &config.indent_heuristic,
          &doc.indent_style,
          doc.tab_width(),
          text,
          above_next_new_line_num,
          above_next_line_end_index,
          curr_line_num,
        )
      },
    };

    let indent_len = indent.len();
    let mut text = String::with_capacity(1 + indent_len);

    if open == Open::Above && next_new_line_num == 0 {
      text.push_str(&indent);
      if let Some(token) = continue_comment_token {
        text.push_str(token);
        text.push(' ');
      }
      text.push_str(doc.line_ending.as_str());
    } else {
      text.push_str(doc.line_ending.as_str());
      text.push_str(&indent);

      if let Some(token) = continue_comment_token {
        text.push_str(token);
        text.push(' ');
      }
    }

    let text = text.repeat(count);

    // calculate new selection ranges
    let pos = offs + above_next_line_end_index + above_next_line_end_width;
    let comment_len = continue_comment_token
            .map(|token| token.len() + 1) // `+ 1` for the extra space added
            .unwrap_or_default();
    for i in 0..count {
      // pos                     -> beginning of reference line,
      // + (i * (line_ending_len + indent_len + comment_len)) -> beginning of i'th
      //   line from pos (possibly including comment token)
      // + indent_len + comment_len ->        -> indent for i'th line
      ranges.push(Range::point(
        pos
          + (i * (doc.line_ending.len_chars() + indent_len + comment_len))
          + indent_len
          + comment_len,
      ));
    }

    // update the offset for the next range
    offs += text.chars().count();

    (
      above_next_line_end_index,
      above_next_line_end_index,
      Some(text.into()),
    )
  });

  transaction = transaction.with_selection(Selection::new(ranges, selection.primary_index()));

  doc.apply(&transaction, view.id);
}
