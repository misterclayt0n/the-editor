use std::{
  borrow::Cow,
  char::{
    ToLowercase,
    ToUppercase,
  },
  num::NonZeroUsize,
};

use once_cell::sync::Lazy;
use regex::Regex;
use ropey::{
  Rope,
  RopeSlice,
};
use smallvec::SmallVec;
use the_editor_renderer::{
  Key,
  KeyPress,
};
use the_editor_stdx::rope::RopeSliceExt;

use crate::{
  core::{
    Tendril,
    auto_pairs,
    comment,
    document::Document,
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
    search::{
      self,
      CharMatcher,
    },
    selection::{
      Range,
      Selection,
    },
    text_annotations::TextAnnotations,
    text_format::TextFormat,
    transaction::Transaction,
    view::View,
  },
  current,
  current_ref,
  editor::Editor,
  event::PostInsertChar,
  keymap::Mode,
};

type MoveFn =
  fn(RopeSlice, Range, Direction, usize, Movement, &TextFormat, &mut TextAnnotations) -> Range;

pub type OnKeyCallback = Box<dyn FnOnce(&mut Context, KeyPress) + 'static>;

static LINE_ENDING_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"\r\n|\r|\n").unwrap());

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum OnKeyCallbackKind {
  Pending,
  Fallback,
}

pub struct Context<'a> {
  pub register:             Option<char>,
  pub count:                Option<NonZeroUsize>,
  pub editor:               &'a mut Editor,
  pub on_next_key_callback: Option<(OnKeyCallback, OnKeyCallbackKind)>,
  // NOTE: We're ignoring these for now.
  // pub callback: Vec<crate::compositor::Callback>,
  // pub jobs:     &'a mut Jobs,
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

  #[inline]
  pub fn on_next_key(
    &mut self,
    on_next_key_callback: impl FnOnce(&mut Context, KeyPress) + 'static,
  ) {
    self.on_next_key_callback = Some((Box::new(on_next_key_callback), OnKeyCallbackKind::Pending));
  }

  #[inline]
  pub fn on_next_key_fallback(
    &mut self,
    on_next_key_callback: impl FnOnce(&mut Context, KeyPress) + 'static,
  ) {
    self.on_next_key_callback = Some((Box::new(on_next_key_callback), OnKeyCallbackKind::Fallback));
  }

  #[inline]
  pub fn take_on_next_key(&mut self) -> Option<(OnKeyCallback, OnKeyCallbackKind)> {
    self.on_next_key_callback.take()
  }
}

// Store a jump on the jumplist.
fn push_jump(view: &mut View, doc: &mut Document) {
  doc.append_changes_to_history(view);
  let jump = (doc.id(), doc.selection(view.id).clone());
  view.jumps.push(jump);
}

#[derive(Clone, Copy, Debug)]
pub struct FindCharPending {
  pub direction: Direction,
  pub inclusive: bool,
  pub extend:    bool,
  pub count:     usize,
}

#[derive(Clone, Copy, Debug)]
pub enum FindCharInput {
  LineEnding,
  Char(char),
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

// Find
//

fn find_char(cx: &mut Context, direction: Direction, inclusive: bool, extend: bool) {
  let pending = FindCharPending {
    direction,
    inclusive,
    extend,
    count: cx.count(),
  };

  cx.on_next_key(move |cx, event| {
    if !event.pressed {
      return;
    }

    match event.code {
      Key::Enter => {
        perform_find_char(cx.editor, pending, FindCharInput::LineEnding);
      },
      Key::Char(ch) => {
        perform_find_char(cx.editor, pending, FindCharInput::Char(ch));
      },
      _ => {},
    }
  });
}

#[inline]
fn find_char_impl<F, M: CharMatcher + Clone + Copy>(
  editor: &mut Editor,
  search_fn: &F,
  pending: FindCharPending,
  char_matcher: M,
) where
  F: Fn(RopeSlice, M, usize, usize, bool) -> Option<usize> + 'static,
{
  let (view, doc) = current!(editor);
  let text = doc.text().slice(..);

  let selection = doc.selection(view.id).clone().transform(|range| {
    // TODO: use `Range::cursor()` here instead.  However, that works in terms of
    // graphemes, whereas this function doesn't yet.  So we're doing the same logic
    // here, but just in terms of chars instead.
    let search_start_pos = if range.anchor < range.head {
      range.head - 1
    } else {
      range.head
    };

    search_fn(
      text,
      char_matcher,
      search_start_pos,
      pending.count,
      pending.inclusive,
    )
    .map_or(range, |pos| {
      if pending.extend {
        range.put_cursor(text, pos, true)
      } else {
        Range::point(range.cursor(text)).put_cursor(text, pos, true)
      }
    })
  });
  doc.set_selection(view.id, selection);
}

fn find_char_line_ending(editor: &mut Editor, pending: FindCharPending) {
  let (view, doc) = current!(editor);
  let text = doc.text().slice(..);

  let selection = doc.selection(view.id).clone().transform(|range| {
    let cursor = range.cursor(text);
    let cursor_line = range.cursor_line(text);

    let find_on_line = match pending.direction {
      Direction::Forward => {
        let on_edge = line_end_char_index(&text, cursor_line) == cursor;
        let line = cursor_line + pending.count - 1 + (on_edge as usize);
        if line >= text.len_lines() - 1 {
          return range;
        } else {
          line
        }
      },
      Direction::Backward => {
        let on_edge = text.line_to_char(cursor_line) == cursor && !pending.inclusive;
        let line = cursor_line as isize - (pending.count as isize - 1 + on_edge as isize);
        if line <= 0 {
          return range;
        } else {
          line as usize
        }
      },
    };

    let pos = match (pending.direction, pending.inclusive) {
      (Direction::Forward, true) => line_end_char_index(&text, find_on_line),
      (Direction::Forward, false) => line_end_char_index(&text, find_on_line) - 1,
      (Direction::Backward, true) => line_end_char_index(&text, find_on_line - 1),
      (Direction::Backward, false) => text.line_to_char(find_on_line),
    };

    if pending.extend {
      range.put_cursor(text, pos, true)
    } else {
      Range::point(range.cursor(text)).put_cursor(text, pos, true)
    }
  });
  doc.set_selection(view.id, selection);
}

fn find_next_char_impl(
  text: RopeSlice,
  ch: char,
  pos: usize,
  n: usize,
  inclusive: bool,
) -> Option<usize> {
  let pos = (pos + 1).min(text.len_chars());
  if inclusive {
    search::find_nth_next(text, ch, pos, n)
  } else {
    let n = match text.get_char(pos) {
      Some(next_ch) if next_ch == ch => n + 1,
      _ => n,
    };
    search::find_nth_next(text, ch, pos, n).map(|n| n.saturating_sub(1))
  }
}

fn find_prev_char_impl(
  text: RopeSlice,
  ch: char,
  pos: usize,
  n: usize,
  inclusive: bool,
) -> Option<usize> {
  if inclusive {
    search::find_nth_prev(text, ch, pos, n)
  } else {
    let n = match text.get_char(pos.saturating_sub(1)) {
      Some(next_ch) if next_ch == ch => n + 1,
      _ => n,
    };
    search::find_nth_prev(text, ch, pos, n).map(|n| (n + 1).min(text.len_chars()))
  }
}

pub fn perform_find_char(editor: &mut Editor, pending: FindCharPending, input: FindCharInput) {
  editor.apply_motion(move |editor| {
    match input {
      FindCharInput::LineEnding => find_char_line_ending(editor, pending),
      FindCharInput::Char(ch) => {
        match pending.direction {
          Direction::Forward => find_char_impl(editor, &find_next_char_impl, pending, ch),
          Direction::Backward => find_char_impl(editor, &find_prev_char_impl, pending, ch),
        }
      },
    }
  });
}

pub fn find_till_char(cx: &mut Context) {
  find_char(cx, Direction::Forward, false, false);
}

pub fn find_next_char(cx: &mut Context) {
  find_char(cx, Direction::Forward, true, false)
}

pub fn extend_till_char(cx: &mut Context) {
  find_char(cx, Direction::Forward, false, true)
}

pub fn extend_next_char(cx: &mut Context) {
  find_char(cx, Direction::Forward, true, true)
}

pub fn till_prev_char(cx: &mut Context) {
  find_char(cx, Direction::Backward, false, false)
}

pub fn find_prev_char(cx: &mut Context) {
  find_char(cx, Direction::Backward, true, false)
}

pub fn extend_till_prev_char(cx: &mut Context) {
  find_char(cx, Direction::Backward, false, true)
}

pub fn extend_prev_char(cx: &mut Context) {
  find_char(cx, Direction::Backward, true, true)
}

pub fn repeat_last_motion(cx: &mut Context) {
  cx.editor.repeat_last_motion(cx.count());
}

pub mod insert {
  use std::borrow::Cow;

  use ropey::Rope;
  use unicode_width::UnicodeWidthChar;

  use super::*;
  use crate::{
    core::grapheme::{
      nth_next_grapheme_boundary,
      nth_prev_grapheme_boundary,
    },
    editor::SmartTabConfig,
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

  pub fn smart_tab(cx: &mut Context) {
    let (view, doc) = current_ref!(cx.editor);
    let view_id = view.id;

    if matches!(
      cx.editor.config().smart_tab,
      Some(SmartTabConfig { enable: true, .. })
    ) {
      let cursors_after_whitespace = doc.selection(view_id).ranges().iter().all(|range| {
        let cursor = range.cursor(doc.text().slice(..));
        let current_line_num = doc.text().char_to_line(cursor);
        let current_line_start = doc.text().line_to_char(current_line_num);
        let left = doc.text().slice(current_line_start..cursor);
        left.chars().all(|c| c.is_whitespace())
      });

      if !cursors_after_whitespace {
        if doc.active_snippet.is_some() {
          goto_next_tabstop(cx);
        } else {
          move_parent_node_end(cx);
        }
        return;
      }
    }

    insert_tab(cx);
  }

  pub fn insert_tab(cx: &mut Context) {
    insert_tab_impl(cx, 1)
  }

  fn insert_tab_impl(cx: &mut Context, count: usize) {
    let (view, doc) = current!(cx.editor);
    // TODO: round out to nearest indentation level (for example a line with 3
    // spaces should indent by one to reach 4 spaces).

    let indent = Tendril::from(doc.indent_style.as_str().repeat(count));
    let transaction = Transaction::insert(
      doc.text(),
      &doc.selection(view.id).clone().cursors(doc.text().slice(..)),
      indent,
    );
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

pub fn select_mode(cx: &mut Context) {
  let (view, doc) = current!(cx.editor);
  let text = doc.text().slice(..);

  // NOTE: Make sure end-of-document selections are also 1-width.
  //       With the exception of being in an empty document, of course.
  let selection = doc.selection(view.id).clone().transform(|range| {
    if range.is_empty() && range.head == text.len_chars() {
      Range::new(
        grapheme::prev_grapheme_boundary(text, range.anchor),
        range.head,
      )
    } else {
      range
    }
  });
  doc.set_selection(view.id, selection);

  cx.editor.mode = Mode::Select;
}

fn exit_select_mode(cx: &mut Context) {
  if cx.editor.mode == Mode::Select {
    cx.editor.mode = Mode::Normal;
  }
}

fn enter_insert_mode(cx: &mut Context) {
  cx.editor.mode = Mode::Insert;
}

pub fn command_mode(cx: &mut Context) {
  cx.editor.mode = Mode::Command;
  
  // Initialize command prompt if needed
  if cx.editor.command_prompt.is_none() {
    cx.editor.init_command_prompt();
  }
}

// Inserts at the start of each selection.
pub fn insert_mode(cx: &mut Context) {
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

// Inserts at the end of each selection
pub fn append_mode(cx: &mut Context) {
  enter_insert_mode(cx);
  let (view, doc) = current!(cx.editor);
  doc.restore_cursor = true;
  let text = doc.text().slice(..);

  // Make sure there's room at the end of the document if the last
  // selection butts up against it.
  let end = text.len_chars();
  let last_range = doc
    .selection(view.id)
    .iter()
    .last()
    .expect("selection should always have at least one range");
  if !last_range.is_empty() && last_range.to() == end {
    let transaction = Transaction::change(
      doc.text(),
      [(end, end, Some(doc.line_ending.as_str().into()))].into_iter(),
    );
    doc.apply(&transaction, view.id);
  }

  let selection = doc.selection(view.id).clone().transform(|range| {
    Range::new(
      range.from(),
      grapheme::next_grapheme_boundary(doc.text().slice(..), range.to()),
    )
  });
  doc.set_selection(view.id, selection);
}

/// Fallback position to use for [`insert_with_indent`].
enum IndentFallbackPos {
  LineStart,
  LineEnd,
}

// `I` inserts at the first nonwhitespace character of each line with a
// selection. If the line is empty, automatically indent.
pub fn insert_at_line_start(cx: &mut Context) {
  insert_with_indent(cx, IndentFallbackPos::LineStart);
}

// `A` inserts at the end of each line with a selection.
// If the line is empty, automatically indent.
pub fn insert_at_line_end(cx: &mut Context) {
  insert_with_indent(cx, IndentFallbackPos::LineEnd);
}

// Enter insert mode and auto-indent the current line if it is empty.
// If the line is not empty, move the cursor to the specified fallback position.
fn insert_with_indent(cx: &mut Context, cursor_fallback: IndentFallbackPos) {
  enter_insert_mode(cx);

  let (view, doc) = current!(cx.editor);
  let loader = cx.editor.syn_loader.load();

  let text = doc.text().slice(..);
  let contents = doc.text();
  let selection = doc.selection(view.id);

  let syntax = doc.syntax();
  let tab_width = doc.tab_width();

  let mut ranges = SmallVec::with_capacity(selection.len());
  let mut offs = 0;

  let mut transaction = Transaction::change_by_selection(contents, selection, |range| {
    let cursor_line = range.cursor_line(text);
    let cursor_line_start = text.line_to_char(cursor_line);

    if line_end_char_index(&text, cursor_line) == cursor_line_start {
      // line is empty => auto indent
      let line_end_index = cursor_line_start;

      let indent = indent::indent_for_newline(
        &loader,
        syntax,
        &doc.config.load().indent_heuristic,
        &doc.indent_style,
        tab_width,
        text,
        cursor_line,
        line_end_index,
        cursor_line,
      );

      // calculate new selection ranges
      let pos = offs + cursor_line_start;
      let indent_width = indent.chars().count();
      ranges.push(Range::point(pos + indent_width));
      offs += indent_width;

      (line_end_index, line_end_index, Some(indent.into()))
    } else {
      // move cursor to the fallback position
      let pos = match cursor_fallback {
        IndentFallbackPos::LineStart => {
          text
            .line(cursor_line)
            .first_non_whitespace_char()
            .map(|ws_offset| ws_offset + cursor_line_start)
            .unwrap_or(cursor_line_start)
        },
        IndentFallbackPos::LineEnd => line_end_char_index(&text, cursor_line),
      };

      ranges.push(range.put_cursor(text, pos + offs, cx.editor.mode == Mode::Select));

      (cursor_line_start, cursor_line_start, None)
    }
  });

  transaction = transaction.with_selection(Selection::new(ranges, selection.primary_index()));
  doc.apply(&transaction, view.id);
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

// 'o' inserts a new line after each line with a selection.
pub fn open_below(cx: &mut Context) {
  open(cx, Open::Below, CommentContinuation::Enabled)
}

// 'O' inserts a new line before each line with a selection.
pub fn open_above(cx: &mut Context) {
  open(cx, Open::Above, CommentContinuation::Enabled)
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

pub fn replace(cx: &mut Context) {
  let mut buf = [0u8; 4]; // To hold UTF-8 encoded characters.

  // Gotta wait for the next key.
  cx.on_next_key(move |cx, event| {
    if !event.pressed {
      return;
    }

    let (view, doc) = current!(cx.editor);
    let ch: Option<&str> = match event.code {
      Key::Char(ch) => Some(ch.encode_utf8(&mut buf)),
      Key::Enter => Some(doc.line_ending.as_str()),
      _ => None, // Everything else just cancels it.
    };

    if let Some(ch) = ch {
      let selection = doc.selection(view.id);
      let transaction = Transaction::change_by_selection(doc.text(), selection, |range| {
        if range.is_empty() {
          (range.from(), range.to(), None)
        } else {
          let text: Tendril = doc
            .text()
            .slice(range.from()..range.to())
            .graphemes()
            .map(|_| ch)
            .collect();

          (range.from(), range.to(), Some(text))
        }
      });

      doc.apply(&transaction, view.id);
      exit_select_mode(cx);
    }
  });
}

pub fn replace_with_yanked(cx: &mut Context) {
  let register = cx
    .register
    .unwrap_or_else(|| cx.editor.config.load().default_yank_register);
  let count = cx.count();

  replace_with_yanked_impl(cx.editor, register, count);
  exit_select_mode(cx);
}

fn replace_with_yanked_impl(editor: &mut Editor, register: char, count: usize) {
  let Some(values) = editor
    .registers
    .read(register, editor)
    .filter(|values| values.len() > 0)
  else {
    return;
  };

  let scrolloff = editor.config().scrolloff;
  let (view, doc) = current_ref!(editor);

  let map_value = |value: &Cow<str>| {
    let value = LINE_ENDING_REGEX.replace_all(value, doc.line_ending.as_str());
    let mut out = Tendril::from(value.as_ref());
    for _ in 1..count {
      out.push_str(&value);
    }

    out
  };

  let mut values_rev = values.rev().peekable();

  // `values` is asserted to have at least one entry above.
  let last = values_rev.peek().unwrap();
  let repeat = std::iter::repeat(map_value(last));
  let mut values = values_rev
    .rev()
    .map(|value| map_value(&value))
    .chain(repeat);
  let selection = doc.selection(view.id);
  let transaction = Transaction::change_by_selection(doc.text(), selection, |range| {
    if !range.is_empty() {
      (range.from(), range.to(), Some(values.next().unwrap()))
    } else {
      (range.from(), range.to(), None)
    }
  });
  drop(values);

  let (view, doc) = current!(editor);
  doc.apply(&transaction, view.id);
  doc.append_changes_to_history(view);
  view.ensure_cursor_in_view(doc, scrolloff);
}

// Case switching
//

enum CaseSwitcher {
  Upper(ToUppercase),
  Lower(ToLowercase),
  Keep(Option<char>),
}

impl Iterator for CaseSwitcher {
  type Item = char;

  fn next(&mut self) -> Option<Self::Item> {
    match self {
      CaseSwitcher::Upper(upper) => upper.next(),
      CaseSwitcher::Lower(lower) => lower.next(),
      CaseSwitcher::Keep(ch) => ch.take(),
    }
  }

  fn size_hint(&self) -> (usize, Option<usize>) {
    match self {
      CaseSwitcher::Upper(upper) => upper.size_hint(),
      CaseSwitcher::Lower(lower) => lower.size_hint(),
      CaseSwitcher::Keep(ch) => {
        let n = if ch.is_some() { 1 } else { 0 };
        (n, Some(n))
      },
    }
  }
}

pub fn switch_case(cx: &mut Context) {
  switch_case_impl(cx, |string| {
    string
      .chars()
      .flat_map(|ch| {
        if ch.is_lowercase() {
          CaseSwitcher::Upper(ch.to_uppercase())
        } else if ch.is_uppercase() {
          CaseSwitcher::Lower(ch.to_lowercase())
        } else {
          CaseSwitcher::Keep(Some(ch))
        }
      })
      .collect()
  });
}

fn switch_case_impl<F>(cx: &mut Context, change_fn: F)
where
  F: Fn(RopeSlice) -> Tendril,
{
  let (view, doc) = current!(cx.editor);
  let selection = doc.selection(view.id);
  let transaction = Transaction::change_by_selection(doc.text(), selection, |range| {
    let text: Tendril = change_fn(range.slice(doc.text().slice(..)));

    (range.from(), range.to(), Some(text))
  });

  doc.apply(&transaction, view.id);
  exit_select_mode(cx);
}

pub fn switch_to_uppercase(cx: &mut Context) {
  switch_case_impl(cx, |string| {
    string.chunks().map(|chunk| chunk.to_uppercase()).collect()
  });
}

pub fn switch_to_lowercase(cx: &mut Context) {
  switch_case_impl(cx, |string| {
    string.chunks().map(|chunk| chunk.to_lowercase()).collect()
  });
}

// Goto
//

pub fn goto_file_start(cx: &mut Context) {
  goto_file_start_impl(cx, Movement::Move);
}

fn goto_file_start_impl(cx: &mut Context, movement: Movement) {
  if cx.count.is_some() {
    goto_line_impl(cx, movement);
  } else {
    let (view, doc) = current!(cx.editor);
    let text = doc.text().slice(..);
    let selection = doc
      .selection(view.id)
      .clone()
      .transform(|range| range.put_cursor(text, 0, movement == Movement::Extend));
    push_jump(view, doc);
    doc.set_selection(view.id, selection);
  }
}

pub fn goto_last_line(cx: &mut Context) {
  goto_last_line_impl(cx, Movement::Move)
}

fn goto_last_line_impl(cx: &mut Context, movement: Movement) {
  let (view, doc) = current!(cx.editor);
  let text = doc.text().slice(..);
  let line_idx = if text.line(text.len_lines() - 1).len_chars() == 0 {
    // If the last line is blank, don't jump to it.
    text.len_lines().saturating_sub(2)
  } else {
    text.len_lines() - 1
  };
  let pos = text.line_to_char(line_idx);
  let selection = doc
    .selection(view.id)
    .clone()
    .transform(|range| range.put_cursor(text, pos, movement == Movement::Extend));

  push_jump(view, doc);
  doc.set_selection(view.id, selection);
}

pub fn goto_line(cx: &mut Context) {
  goto_line_impl(cx, Movement::Move);
}

fn goto_line_impl(cx: &mut Context, movement: Movement) {
  if cx.count.is_some() {
    let (view, doc) = current!(cx.editor);
    push_jump(view, doc);

    goto_line_without_jumplist(cx.editor, cx.count, movement);
  }
}

fn goto_line_without_jumplist(
  editor: &mut Editor,
  count: Option<NonZeroUsize>,
  movement: Movement,
) {
  if let Some(count) = count {
    let (view, doc) = current!(editor);
    let text = doc.text().slice(..);
    let max_line = if text.line(text.len_lines() - 1).len_chars() == 0 {
      // If the last line is blank, don't jump to it.
      text.len_lines().saturating_sub(2)
    } else {
      text.len_lines() - 1
    };
    let line_idx = std::cmp::min(count.get() - 1, max_line);
    let pos = text.line_to_char(line_idx);
    let selection = doc
      .selection(view.id)
      .clone()
      .transform(|range| range.put_cursor(text, pos, movement == Movement::Extend));

    doc.set_selection(view.id, selection);
  }
}

pub fn goto_line_start(cx: &mut Context) {
  let (view, doc) = current!(cx.editor);
  goto_line_start_impl(
    view,
    doc,
    if cx.editor.mode == Mode::Select {
      Movement::Extend
    } else {
      Movement::Move
    },
  )
}

fn goto_line_start_impl(view: &mut View, doc: &mut Document, movement: Movement) {
  let text = doc.text().slice(..);

  let selection = doc.selection(view.id).clone().transform(|range| {
    let line = range.cursor_line(text);

    // Adjust to start of the line.
    let pos = text.line_to_char(line);
    range.put_cursor(text, pos, movement == Movement::Extend)
  });
  doc.set_selection(view.id, selection);
}

pub fn goto_line_end(cx: &mut Context) {
  let (view, doc) = current!(cx.editor);
  goto_line_end_impl(
    view,
    doc,
    if cx.editor.mode == Mode::Select {
      Movement::Extend
    } else {
      Movement::Move
    },
  )
}

fn goto_line_end_impl(view: &mut View, doc: &mut Document, movement: Movement) {
  let text = doc.text().slice(..);

  let selection = doc.selection(view.id).clone().transform(|range| {
    let line = range.cursor_line(text);
    let line_start = text.line_to_char(line);

    let pos =
      grapheme::prev_grapheme_boundary(text, line_end_char_index(&text, line)).max(line_start);

    range.put_cursor(text, pos, movement == Movement::Extend)
  });
  doc.set_selection(view.id, selection);
}

pub fn goto_column(cx: &mut Context) {
  goto_column_impl(cx, Movement::Move);
}

fn goto_column_impl(cx: &mut Context, movement: Movement) {
  let count = cx.count();
  let (view, doc) = current!(cx.editor);
  let text = doc.text().slice(..);
  let selection = doc.selection(view.id).clone().transform(|range| {
    let line = range.cursor_line(text);
    let line_start = text.line_to_char(line);
    let line_end = line_end_char_index(&text, line);
    let pos = grapheme::nth_next_grapheme_boundary(text, line_start, count - 1).min(line_end);
    range.put_cursor(text, pos, movement == Movement::Extend)
  });
  doc.set_selection(view.id, selection);
}

pub fn toggle_debug_panel(cx: &mut Context) {
  cx.editor.ui_components.toggle_component("debug_panel");
}

pub fn goto_next_tabstop(cx: &mut Context) {
  goto_next_tabstop_impl(cx, Direction::Forward)
}

fn goto_next_tabstop_impl(cx: &mut Context, direction: Direction) {
  let (view, doc) = current!(cx.editor);
  let view_id = view.id;
  let Some(mut snippet) = doc.active_snippet.take() else {
    cx.editor.set_error("no snippet is currently active");
    return;
  };
  let tabstop = match direction {
    Direction::Forward => Some(snippet.next_tabstop(doc.selection(view_id))),
    Direction::Backward => {
      snippet
        .prev_tabstop(doc.selection(view_id))
        .map(|selection| (selection, false))
    },
  };
  let Some((selection, last_tabstop)) = tabstop else {
    return;
  };
  doc.set_selection(view_id, selection);
  if !last_tabstop {
    doc.active_snippet = Some(snippet)
  }
  if cx.editor.mode() == Mode::Insert {
    cx.on_next_key_fallback(|cx, event| {
      let ch = match event.code {
        Key::Char(ch) => Some(ch),
        _ => None,
      };

      if let Some(c) = ch {
        let (view, doc) = current!(cx.editor);
        if let Some(snippet) = &doc.active_snippet {
          doc.apply(&snippet.delete_placeholder(doc.text()), view.id);
        }
        insert::insert_char(cx, c);
      }
    })
  }
}

pub fn move_parent_node_end(cx: &mut Context) {
  move_node_bound_impl(cx, Direction::Forward, Movement::Move)
}

fn move_node_bound_impl(cx: &mut Context, dir: Direction, movement: Movement) {
  let motion = move |editor: &mut Editor| {
    let (view, doc) = current!(editor);

    if let Some(syntax) = doc.syntax() {
      let text = doc.text().slice(..);
      let current_selection = doc.selection(view.id);

      let selection =
        movement::move_parent_node_end(syntax, text, current_selection.clone(), dir, movement);

      doc.set_selection(view.id, selection);
    }
  };

  cx.editor.apply_motion(motion);
}

pub fn insert_newline(cx: &mut Context) {
  let config = cx.editor.config();
  let (view, doc) = current_ref!(cx.editor);
  let loader = cx.editor.syn_loader.load();
  let text = doc.text().slice(..);
  let line_ending = doc.line_ending.as_str();

  let contents = doc.text();
  let selection = doc.selection(view.id);
  let mut ranges = SmallVec::with_capacity(selection.len());

  // TODO: this is annoying, but we need to do it to properly calculate pos after
  // edits
  let mut global_offs = 0;
  let mut new_text = String::new();

  let continue_comment_tokens = if config.continue_comments {
    doc
      .language_config()
      .and_then(|config| config.comment_tokens.as_ref())
  } else {
    None
  };

  let mut last_pos = 0;
  let mut transaction = Transaction::change_by_selection(contents, selection, |range| {
    // Tracks the number of trailing whitespace characters deleted by this
    // selection.
    let mut chars_deleted = 0;
    let pos = range.cursor(text);

    let prev = if pos == 0 {
      ' '
    } else {
      contents.char(pos - 1)
    };
    let curr = contents.get_char(pos).unwrap_or(' ');

    let current_line = text.char_to_line(pos);
    let line_start = text.line_to_char(current_line);

    let continue_comment_token = continue_comment_tokens
      .and_then(|tokens| comment::get_comment_token(text, tokens, current_line));

    let (from, to, local_offs) =
      if let Some(idx) = text.slice(line_start..pos).last_non_whitespace_char() {
        let first_trailing_whitespace_char = (line_start + idx + 1).clamp(last_pos, pos);
        last_pos = pos;
        let line = text.line(current_line);

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
              current_line,
              pos,
              current_line,
            )
          },
        };

        // If we are between pairs (such as brackets), we want to
        // insert an additional line which is indented one level
        // more and place the cursor there
        let on_auto_pair = doc
          .auto_pairs(cx.editor)
          .and_then(|pairs| pairs.get(prev))
          .is_some_and(|pair| pair.open == prev && pair.close == curr);

        let local_offs = if let Some(token) = continue_comment_token {
          new_text.reserve_exact(line_ending.len() + indent.len() + token.len() + 1);
          new_text.push_str(line_ending);
          new_text.push_str(&indent);
          new_text.push_str(token);
          new_text.push(' ');
          new_text.chars().count()
        } else if on_auto_pair {
          // line where the cursor will be
          let inner_indent = indent.clone() + doc.indent_style.as_str();
          new_text.reserve_exact(line_ending.len() * 2 + indent.len() + inner_indent.len());
          new_text.push_str(line_ending);
          new_text.push_str(&inner_indent);

          // line where the matching pair will be
          let local_offs = new_text.chars().count();
          new_text.push_str(line_ending);
          new_text.push_str(&indent);

          local_offs
        } else {
          new_text.reserve_exact(line_ending.len() + indent.len());
          new_text.push_str(line_ending);
          new_text.push_str(&indent);

          new_text.chars().count()
        };

        // Note that `first_trailing_whitespace_char` is at least `pos` so this unsigned
        // subtraction cannot underflow.
        chars_deleted = pos - first_trailing_whitespace_char;

        (
          first_trailing_whitespace_char,
          pos,
          local_offs as isize - chars_deleted as isize,
        )
      } else {
        // If the current line is all whitespace, insert a line ending at the beginning
        // of the current line. This makes the current line empty and the new
        // line contain the indentation of the old line.
        new_text.push_str(line_ending);

        (line_start, line_start, new_text.chars().count() as isize)
      };

    let new_range = if range.cursor(text) > range.anchor {
      // when appending, extend the range by local_offs
      Range::new(
        (range.anchor as isize + global_offs) as usize,
        (range.head as isize + local_offs + global_offs) as usize,
      )
    } else {
      // when inserting, slide the range by local_offs
      Range::new(
        (range.anchor as isize + local_offs + global_offs) as usize,
        (range.head as isize + local_offs + global_offs) as usize,
      )
    };

    // TODO: range replace or extend
    // range.replace(|range| range.is_empty(), head); -> fn extend if cond true, new
    // head pos can be used with cx.mode to do replace or extend on most changes
    ranges.push(new_range);
    global_offs += new_text.chars().count() as isize - chars_deleted as isize;
    let tendril = Tendril::from(&new_text);
    new_text.clear();

    (from, to, Some(tendril))
  });

  transaction = transaction.with_selection(Selection::new(ranges, selection.primary_index()));

  let (view, doc) = current!(cx.editor);
  doc.apply(&transaction, view.id);
}
