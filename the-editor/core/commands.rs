use std::{
  borrow::Cow,
  char::{
    ToLowercase,
    ToUppercase,
  },
  num::NonZeroUsize,
};

use anyhow::anyhow;
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
    ViewId,
    auto_pairs,
    comment,
    document::Document,
    grapheme,
    history::UndoKind,
    indent,
    info::Info,
    line_ending::{
      get_line_ending_of_str,
      line_end_char_index,
    },
    match_brackets,
    movement::{
      self,
      Direction,
      Movement,
      move_horizontally,
      move_vertically,
      move_vertically_visual,
    },
    position::{
      Position,
      char_idx_at_visual_offset,
    },
    search::{
      self,
      CharMatcher,
    },
    selection::{
      Range,
      Selection,
    },
    surround,
    text_annotations::TextAnnotations,
    text_format::TextFormat,
    textobject,
    transaction::Transaction,
    view::View,
  },
  current,
  current_ref,
  editor::Editor,
  event::PostInsertChar,
  keymap::{
    KeyBinding,
    Mode,
  },
};

type MoveFn =
  fn(RopeSlice, Range, Direction, usize, Movement, &TextFormat, &mut TextAnnotations) -> Range;

pub type OnKeyCallback = Box<dyn FnOnce(&mut Context, KeyPress) + 'static>;

// NOTE: For now we're only adding Context to this callback but I can see how we
// might need to trigger UI elements from this tho.
// Import compositor types
use crate::ui::compositor;

// Callback now takes both Compositor and Context like in Helix
pub type Callback = Box<dyn FnOnce(&mut compositor::Compositor, &mut compositor::Context)>;

// Placeholder for MappableCommand until we implement it fully
#[derive(Debug, Clone, Copy)]
pub enum MappableCommand {
  NormalMode,
}

// Provide a method to match Helix's API
impl MappableCommand {
  pub const fn normal_mode() -> Self {
    MappableCommand::NormalMode
  }
}

static LINE_ENDING_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"\r\n|\r|\n").unwrap());

static SURROUND_HELP_TEXT: [(&str, &str); 6] = [
  ("m", "Nearest matching pair"),
  ("( or )", "Parentheses"),
  ("{ or }", "Curly braces"),
  ("< or >", "Angled brackets"),
  ("[ or ]", "Square brackets"),
  (" ", "... or any character"),
];

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
  pub callback:             Vec<Callback>,
  pub jobs:                 &'a mut crate::ui::job::Jobs,
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

pub fn delete_selection_noyank(cx: &mut Context) {
  delete_selection_impl(cx, Operation::Delete, YankAction::NoYank);
}

pub fn change_selection(cx: &mut Context) {
  delete_selection_impl(cx, Operation::Change, YankAction::Yank);
}

pub fn change_selection_noyank(cx: &mut Context) {
  delete_selection_impl(cx, Operation::Change, YankAction::NoYank);
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

  cx.editor.set_mode(Mode::Select);
}

fn exit_select_mode(cx: &mut Context) {
  if cx.editor.mode == Mode::Select {
    cx.editor.set_mode(Mode::Normal);
  }
}

fn enter_insert_mode(cx: &mut Context) {
  cx.editor.set_mode(Mode::Insert);
}

pub fn command_mode(cx: &mut Context) {
  cx.editor.set_mode(Mode::Command);

  // Initialize command prompt if needed
  if cx.editor.command_prompt.is_none() {
    cx.editor.init_command_prompt();
  }
}

pub fn normal_mode(cx: &mut Context) {
  cx.editor.set_mode(Mode::Normal);
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

pub fn replace_selections_with_clipboard(cx: &mut Context) {
  replace_with_yanked_impl(cx.editor, '+', cx.count());
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

pub fn goto_first_nonwhitespace(cx: &mut Context) {
  let (view, doc) = current!(cx.editor);

  goto_first_nonwhitespace_impl(
    view,
    doc,
    if cx.editor.mode == Mode::Select {
      Movement::Extend
    } else {
      Movement::Move
    },
  )
}

fn goto_first_nonwhitespace_impl(view: &mut View, doc: &mut Document, movement: Movement) {
  let text = doc.text().slice(..);

  let selection = doc.selection(view.id).clone().transform(|range| {
    let line = range.cursor_line(text);

    if let Some(pos) = text.line(line).first_non_whitespace_char() {
      let pos = pos + text.line_to_char(line);
      range.put_cursor(text, pos, movement == Movement::Extend)
    } else {
      range
    }
  });
  doc.set_selection(view.id, selection);
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

pub fn toggle_debug_panel(_cx: &mut Context) {
  // TODO: Implement debug panel toggling through compositor
  // Need access to compositor to toggle UI layers
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

// Yank & Paste
//

pub fn yank(cx: &mut Context) {
  yank_impl(
    cx.editor,
    cx.register
      .unwrap_or(cx.editor.config().default_yank_register),
  );
  exit_select_mode(cx);
}

pub fn yank_to_clipboard(cx: &mut Context) {
  yank_impl(cx.editor, '+');
  exit_select_mode(cx);
}

pub fn yank_main_selection_to_clipboard(cx: &mut Context) {
  yank_primary_selection_impl(cx.editor, '+');
  exit_select_mode(cx);
}

fn yank_primary_selection_impl(editor: &mut Editor, register: char) {
  let (view, doc) = current!(editor);
  let text = doc.text().slice(..);

  let selection = doc.selection(view.id).primary().fragment(text).to_string();

  match editor.registers.write(register, vec![selection]) {
    Ok(_) => editor.set_status(format!("yanked primary selection to register {register}",)),
    Err(err) => editor.set_error(err.to_string()),
  }
}

fn yank_impl(editor: &mut Editor, register: char) {
  let (view, doc) = current!(editor);
  let text = doc.text().slice(..);

  let values: Vec<String> = doc
    .selection(view.id)
    .fragments(text)
    .map(Cow::into_owned)
    .collect();
  let selections = values.len();

  match editor.registers.write(register, values) {
    Ok(_) => {
      editor.set_status(format!(
        "yanked {selections} selection{} to register {register}",
        if selections == 1 { "" } else { "s" }
      ))
    },
    Err(err) => editor.set_error(err.to_string()),
  }
}

#[derive(Copy, Clone)]
enum Paste {
  Before,
  After,
  Cursor,
}

pub fn paste_clipboard_after(cx: &mut Context) {
  paste(cx.editor, '+', Paste::After, cx.count());
  exit_select_mode(cx);
}

pub fn paste_clipboard_before(cx: &mut Context) {
  paste(cx.editor, '+', Paste::Before, cx.count());
  exit_select_mode(cx);
}

pub fn paste_after(cx: &mut Context) {
  paste(
    cx.editor,
    cx.register
      .unwrap_or(cx.editor.config().default_yank_register),
    Paste::After,
    cx.count(),
  );
  exit_select_mode(cx);
}

pub fn paste_before(cx: &mut Context) {
  paste(
    cx.editor,
    cx.register
      .unwrap_or(cx.editor.config().default_yank_register),
    Paste::Before,
    cx.count(),
  );
  exit_select_mode(cx);
}

fn paste(editor: &mut Editor, register: char, pos: Paste, count: usize) {
  let Some(values) = editor.registers.read(register, editor) else {
    return;
  };
  let values: Vec<_> = values.map(|value| value.to_string()).collect();

  let (view, doc) = current!(editor);
  paste_impl(&values, doc, view, pos, count, editor.mode);
}

fn paste_impl(
  values: &[String],
  doc: &mut Document,
  view: &mut View,
  action: Paste,
  count: usize,
  mode: Mode,
) {
  if values.is_empty() {
    return;
  }

  if mode == Mode::Insert {
    doc.append_changes_to_history(view);
  }

  // if any of values ends with a line ending, it's linewise paste
  let linewise = values
    .iter()
    .any(|value| get_line_ending_of_str(value).is_some());

  let map_value = |value| {
    let value = LINE_ENDING_REGEX.replace_all(value, doc.line_ending.as_str());
    let mut out = Tendril::from(value.as_ref());
    for _ in 1..count {
      out.push_str(&value);
    }
    out
  };

  let repeat = std::iter::repeat(
    // `values` is asserted to have at least one entry above.
    map_value(values.last().unwrap()),
  );

  let mut values = values.iter().map(|value| map_value(value)).chain(repeat);

  let text = doc.text();
  let selection = doc.selection(view.id);

  let mut offset = 0;
  let mut ranges = SmallVec::with_capacity(selection.len());

  let mut transaction = Transaction::change_by_selection(text, selection, |range| {
    let pos = match (action, linewise) {
      // paste linewise before
      (Paste::Before, true) => text.line_to_char(text.char_to_line(range.from())),
      // paste linewise after
      (Paste::After, true) => {
        let line = range.line_range(text.slice(..)).1;
        text.line_to_char((line + 1).min(text.len_lines()))
      },
      // paste insert
      (Paste::Before, false) => range.from(),
      // paste append
      (Paste::After, false) => range.to(),
      // paste at cursor
      (Paste::Cursor, _) => range.cursor(text.slice(..)),
    };

    let value = values.next();

    let value_len = value
      .as_ref()
      .map(|content| content.chars().count())
      .unwrap_or_default();
    let anchor = offset + pos;

    let new_range = Range::new(anchor, anchor + value_len).with_direction(range.direction());
    ranges.push(new_range);
    offset += value_len;

    (pos, pos, value)
  });

  if mode == Mode::Normal {
    transaction = transaction.with_selection(Selection::new(ranges, selection.primary_index()));
  }

  doc.apply(&transaction, view.id);
  doc.append_changes_to_history(view);
}

pub fn copy_selection_on_next_line(cx: &mut Context) {
  copy_selection_on_line(cx, Direction::Forward)
}

pub fn copy_selection_on_prev_line(cx: &mut Context) {
  copy_selection_on_line(cx, Direction::Backward)
}

#[allow(deprecated)]
// currently uses the deprecated `visual_coords_at_pos`/`pos_at_visual_coords`
// functions as this function ignores softwrapping (and virtual text) and
// instead only cares about "text visual position"
//
// TODO: implement a variant of that uses visual lines and respects virtual text
fn copy_selection_on_line(cx: &mut Context, direction: Direction) {
  use crate::core::position::{
    pos_at_visual_coords,
    visual_coords_at_pos,
  };

  let count = cx.count();
  let (view, doc) = current!(cx.editor);
  let text = doc.text().slice(..);
  let selection = doc.selection(view.id);
  let mut ranges = SmallVec::with_capacity(selection.ranges().len() * (count + 1));
  ranges.extend_from_slice(selection.ranges());
  let mut primary_index = 0;
  for range in selection.iter() {
    let is_primary = *range == selection.primary();

    // The range is always head exclusive
    let (head, anchor) = if range.anchor < range.head {
      (range.head - 1, range.anchor)
    } else {
      (range.head, range.anchor.saturating_sub(1))
    };

    let tab_width = doc.tab_width();

    let head_pos = visual_coords_at_pos(text, head, tab_width);
    let anchor_pos = visual_coords_at_pos(text, anchor, tab_width);

    let height =
      std::cmp::max(head_pos.row, anchor_pos.row) - std::cmp::min(head_pos.row, anchor_pos.row) + 1;

    if is_primary {
      primary_index = ranges.len();
    }
    ranges.push(*range);

    let mut sels = 0;
    let mut i = 0;
    while sels < count {
      let offset = (i + 1) * height;

      let anchor_row = match direction {
        Direction::Forward => anchor_pos.row + offset,
        Direction::Backward => anchor_pos.row.saturating_sub(offset),
      };

      let head_row = match direction {
        Direction::Forward => head_pos.row + offset,
        Direction::Backward => head_pos.row.saturating_sub(offset),
      };

      if anchor_row >= text.len_lines() || head_row >= text.len_lines() {
        break;
      }

      let anchor = pos_at_visual_coords(text, Position::new(anchor_row, anchor_pos.col), tab_width);
      let head = pos_at_visual_coords(text, Position::new(head_row, head_pos.col), tab_width);

      // skip lines that are too short
      if visual_coords_at_pos(text, anchor, tab_width).col == anchor_pos.col
        && visual_coords_at_pos(text, head, tab_width).col == head_pos.col
      {
        if is_primary {
          primary_index = ranges.len();
        }
        // This is Range::new(anchor, head), but it will place the cursor on the correct
        // column
        ranges.push(Range::point(anchor).put_cursor(text, head, true));
        sels += 1;
      }

      if anchor_row == 0 && head_row == 0 {
        break;
      }

      i += 1;
    }
  }

  let selection = Selection::new(ranges, primary_index);
  doc.set_selection(view.id, selection);
}

pub fn select_all(cx: &mut Context) {
  let (view, doc) = current!(cx.editor);

  let end = doc.text().len_chars();
  doc.set_selection(view.id, Selection::single(0, end))
}

enum Extend {
  Above,
  Below,
}

pub fn extend_line_below(cx: &mut Context) {
  extend_line_impl(cx, Extend::Below);
}

pub fn extend_line_above(cx: &mut Context) {
  extend_line_impl(cx, Extend::Above);
}

pub fn extend_to_line_bounds(cx: &mut Context) {
  let (view, doc) = current!(cx.editor);

  doc.set_selection(
    view.id,
    doc.selection(view.id).clone().transform(|range| {
      let text = doc.text();

      let (start_line, end_line) = range.line_range(text.slice(..));
      let start = text.line_to_char(start_line);
      let end = text.line_to_char((end_line + 1).min(text.len_lines()));

      Range::new(start, end).with_direction(range.direction())
    }),
  );
}

pub fn shrink_to_line_bounds(cx: &mut Context) {
  let (view, doc) = current!(cx.editor);

  doc.set_selection(
    view.id,
    doc.selection(view.id).clone().transform(|range| {
      let text = doc.text();

      let (start_line, end_line) = range.line_range(text.slice(..));

      // Do nothing if the selection is within one line to prevent
      // conditional logic for the behavior of this command
      if start_line == end_line {
        return range;
      }

      let mut start = text.line_to_char(start_line);

      // line_to_char gives us the start position of the line, so
      // we need to get the start position of the next line. In
      // the editor, this will correspond to the cursor being on
      // the EOL whitespace character, which is what we want.
      let mut end = text.line_to_char((end_line + 1).min(text.len_lines()));

      if start != range.from() {
        start = text.line_to_char((start_line + 1).min(text.len_lines()));
      }

      if end != range.to() {
        end = text.line_to_char(end_line);
      }

      Range::new(start, end).with_direction(range.direction())
    }),
  );
}

fn extend_line_impl(cx: &mut Context, extend: Extend) {
  let count = cx.count();
  let (view, doc) = current!(cx.editor);

  let text = doc.text();
  let selection = doc.selection(view.id).clone().transform(|range| {
    let (start_line, end_line) = range.line_range(text.slice(..));

    let start = text.line_to_char(start_line);
    let end = text.line_to_char(
      (end_line + 1) // newline of end_line
        .min(text.len_lines()),
    );

    // extend to previous/next line if current line is selected
    let (anchor, head) = if range.from() == start && range.to() == end {
      match extend {
        Extend::Above => (end, text.line_to_char(start_line.saturating_sub(count))),
        Extend::Below => {
          (
            start,
            text.line_to_char((end_line + count + 1).min(text.len_lines())),
          )
        },
      }
    } else {
      match extend {
        Extend::Above => (end, text.line_to_char(start_line.saturating_sub(count - 1))),
        Extend::Below => {
          (
            start,
            text.line_to_char((end_line + count).min(text.len_lines())),
          )
        },
      }
    };

    Range::new(anchor, head)
  });

  doc.set_selection(view.id, selection);
}

pub fn match_brackets(cx: &mut Context) {
  let (view, doc) = current!(cx.editor);
  let is_select = cx.editor.mode == Mode::Select;
  let text = doc.text();
  let text_slice = text.slice(..);

  let selection = doc.selection(view.id).clone().transform(|range| {
    let pos = range.cursor(text_slice);
    if let Some(matched_pos) = doc.syntax().map_or_else(
      || match_brackets::find_matching_bracket_plaintext(text.slice(..), pos),
      |syntax| match_brackets::find_matching_bracket_fuzzy(syntax, text.slice(..), pos),
    ) {
      range.put_cursor(text_slice, matched_pos, is_select)
    } else {
      range
    }
  });

  doc.set_selection(view.id, selection);
}

pub fn surround_add(cx: &mut Context) {
  cx.on_next_key(move |cx, event| {
    if !event.pressed {
      return;
    }

    cx.editor.autoinfo = None;
    let (view, doc) = current!(cx.editor);
    // surround_len is the number of new characters being added.
    let (open, close, surround_len) = match event.code {
      Key::Char(ch) => {
        let (o, c) = match_brackets::get_pair(ch);
        let mut open = Tendril::new();
        open.push(o);
        let mut close = Tendril::new();
        close.push(c);
        (open, close, 2)
      },
      Key::Enter => {
        (
          doc.line_ending.as_str().into(),
          doc.line_ending.as_str().into(),
          2 * doc.line_ending.len_chars(),
        )
      },
      _ => return,
    };

    let selection = doc.selection(view.id);
    let mut changes = Vec::with_capacity(selection.len() * 2);
    let mut ranges = SmallVec::with_capacity(selection.len());
    let mut offs = 0;

    for range in selection.iter() {
      changes.push((range.from(), range.from(), Some(open.clone())));
      changes.push((range.to(), range.to(), Some(close.clone())));

      ranges.push(
        Range::new(offs + range.from(), offs + range.to() + surround_len)
          .with_direction(range.direction()),
      );

      offs += surround_len;
    }

    let transaction = Transaction::change(doc.text(), changes.into_iter())
      .with_selection(Selection::new(ranges, selection.primary_index()));
    doc.apply(&transaction, view.id);
    exit_select_mode(cx);
  });

  cx.editor.autoinfo = Some(Info::new(
    "Surround selections with",
    &SURROUND_HELP_TEXT[1..],
  ));
}

pub fn surround_replace(cx: &mut Context) {
  let count = cx.count();
  cx.on_next_key(move |cx, event| {
    if !event.pressed {
      return;
    }

    cx.editor.autoinfo = None;
    let surround_ch = match event.code {
      Key::Char('m') => None, // m selects the closest surround pair
      Key::Char(ch) => Some(ch),
      _ => return,
    };
    let (view, doc) = current!(cx.editor);
    let text = doc.text().slice(..);
    let selection = doc.selection(view.id);

    let change_pos =
      match surround::get_surround_pos(doc.syntax(), text, selection, surround_ch, count) {
        Ok(c) => c,
        Err(err) => {
          cx.editor.set_error(err.to_string());
          return;
        },
      };

    let selection = selection.clone();
    let ranges: SmallVec<[Range; 1]> = change_pos.iter().map(|&p| Range::point(p)).collect();
    doc.set_selection(
      view.id,
      Selection::new(ranges, selection.primary_index() * 2),
    );

    cx.on_next_key(move |cx, event| {
      if !event.pressed {
        return;
      }

      cx.editor.autoinfo = None;
      let (view, doc) = current!(cx.editor);
      let to = match event.code {
        Key::Char(to) => to,
        _ => return doc.set_selection(view.id, selection),
      };
      let (open, close) = match_brackets::get_pair(to);

      // the changeset has to be sorted to allow nested surrounds
      let mut sorted_pos: Vec<(usize, char)> = Vec::new();
      for p in change_pos.chunks(2) {
        sorted_pos.push((p[0], open));
        sorted_pos.push((p[1], close));
      }
      sorted_pos.sort_unstable();

      let transaction = Transaction::change(
        doc.text(),
        sorted_pos.iter().map(|&pos| {
          let mut t = Tendril::new();
          t.push(pos.1);
          (pos.0, pos.0 + 1, Some(t))
        }),
      );
      doc.set_selection(view.id, selection);
      doc.apply(&transaction, view.id);
      exit_select_mode(cx);
    });

    cx.editor.autoinfo = Some(Info::new(
      "Replace with a pair of",
      &SURROUND_HELP_TEXT[1..],
    ));
  });

  cx.editor.autoinfo = Some(Info::new(
    "Replace surrounding pair of",
    &SURROUND_HELP_TEXT,
  ));
}

pub fn surround_delete(cx: &mut Context) {
  let count = cx.count();
  cx.on_next_key(move |cx, event| {
    if !event.pressed {
      return;
    }

    cx.editor.autoinfo = None;
    let surround_ch = match event.code {
      Key::Char('m') => None, // m selects the closest surround pair
      Key::Char(ch) => Some(ch),
      _ => return,
    };
    let (view, doc) = current!(cx.editor);
    let text = doc.text().slice(..);
    let selection = doc.selection(view.id);

    let mut change_pos =
      match surround::get_surround_pos(doc.syntax(), text, selection, surround_ch, count) {
        Ok(c) => c,
        Err(err) => {
          cx.editor.set_error(err.to_string());
          return;
        },
      };
    change_pos.sort_unstable(); // the changeset has to be sorted to allow nested surrounds
    let transaction =
      Transaction::change(doc.text(), change_pos.into_iter().map(|p| (p, p + 1, None)));
    doc.apply(&transaction, view.id);
    exit_select_mode(cx);
  });

  cx.editor.autoinfo = Some(Info::new("Delete surrounding pair of", &SURROUND_HELP_TEXT));
}

pub fn select_textobject_around(cx: &mut Context) {
  select_textobject(cx, textobject::TextObject::Around);
}

pub fn select_textobject_inner(cx: &mut Context) {
  select_textobject(cx, textobject::TextObject::Inside);
}

fn select_textobject(cx: &mut Context, objtype: textobject::TextObject) {
  let count = cx.count();

  cx.on_next_key(move |cx, event| {
    if !event.pressed {
      return;
    }

    cx.editor.autoinfo = None;
    if let Key::Char(ch) = event.code {
      let textobject = move |editor: &mut Editor| {
        let (view, doc) = current!(editor);
        let loader = editor.syn_loader.load();
        let text = doc.text().slice(..);

        let textobject_treesitter = |obj_name: &str, range: Range| -> Range {
          let Some(syntax) = doc.syntax() else {
            return range;
          };
          textobject::textobject_treesitter(text, range, objtype, obj_name, syntax, &loader, count)
        };

        if ch == 'g' && doc.diff_handle().is_none() {
          editor.set_status("Diff is not available in current buffer");
          return;
        }

        let textobject_change = |range: Range| -> Range {
          let diff_handle = doc.diff_handle().unwrap();
          let diff = diff_handle.load();
          let line = range.cursor_line(text);
          let hunk_idx = if let Some(hunk_idx) = diff.hunk_at(line as u32, false) {
            hunk_idx
          } else {
            return range;
          };
          let hunk = diff.nth_hunk(hunk_idx).after;

          let start = text.line_to_char(hunk.start as usize);
          let end = text.line_to_char(hunk.end as usize);
          Range::new(start, end).with_direction(range.direction())
        };

        let selection = doc.selection(view.id).clone().transform(|range| {
          match ch {
            'w' => textobject::textobject_word(text, range, objtype, count, false),
            'W' => textobject::textobject_word(text, range, objtype, count, true),
            't' => textobject_treesitter("class", range),
            'f' => textobject_treesitter("function", range),
            'a' => textobject_treesitter("parameter", range),
            'c' => textobject_treesitter("comment", range),
            'T' => textobject_treesitter("test", range),
            'e' => textobject_treesitter("entry", range),
            'x' => textobject_treesitter("xml-element", range),
            'p' => textobject::textobject_paragraph(text, range, objtype, count),
            'm' => {
              textobject::textobject_pair_surround_closest(
                doc.syntax(),
                text,
                range,
                objtype,
                count,
              )
            },
            'g' => textobject_change(range),
            // TODO: cancel new ranges if inconsistent surround matches across lines
            ch if !ch.is_ascii_alphanumeric() => {
              textobject::textobject_pair_surround(doc.syntax(), text, range, objtype, ch, count)
            },
            _ => range,
          }
        });
        doc.set_selection(view.id, selection);
      };
      cx.editor.apply_motion(textobject);
    }
  });

  let title = match objtype {
    textobject::TextObject::Inside => "Match inside",
    textobject::TextObject::Around => "Match around",
    _ => return,
  };
  let help_text = [
    ("w", "Word"),
    ("W", "WORD"),
    ("p", "Paragraph"),
    ("t", "Type definition (tree-sitter)"),
    ("f", "Function (tree-sitter)"),
    ("a", "Argument/parameter (tree-sitter)"),
    ("c", "Comment (tree-sitter)"),
    ("T", "Test (tree-sitter)"),
    ("e", "Data structure entry (tree-sitter)"),
    ("m", "Closest surrounding pair (tree-sitter)"),
    ("g", "Change"),
    ("x", "(X)HTML element (tree-sitter)"),
    (" ", "... or any character acting as a pair"),
  ];

  cx.editor.autoinfo = Some(Info::new(title, &help_text));
}

pub fn undo(cx: &mut Context) {
  let count = cx.count();
  let (view, doc) = current!(cx.editor);
  for _ in 0..count {
    if !doc.undo(view) {
      cx.editor.set_status("Already at oldest change");
      break;
    }
  }
}

pub fn redo(cx: &mut Context) {
  let count = cx.count();
  let (view, doc) = current!(cx.editor);
  for _ in 0..count {
    if !doc.redo(view) {
      cx.editor.set_status("Already at newest change");
      break;
    }
  }
}

pub fn earlier(cx: &mut Context) {
  let count = cx.count();
  let (view, doc) = current!(cx.editor);
  for _ in 0..count {
    // rather than doing in batch we do this so get error halfway
    if !doc.earlier(view, UndoKind::Steps(1)) {
      cx.editor.set_status("Already at oldest change");
      break;
    }
  }
}

pub fn later(cx: &mut Context) {
  let count = cx.count();
  let (view, doc) = current!(cx.editor);
  for _ in 0..count {
    // rather than doing in batch we do this so get error halfway
    if !doc.later(view, UndoKind::Steps(1)) {
      cx.editor.set_status("Already at newest change");
      break;
    }
  }
}

pub fn keep_primary_selection(cx: &mut Context) {
  let (view, doc) = current!(cx.editor);
  // TODO: handle count

  let range = doc.selection(view.id).primary();
  doc.set_selection(view.id, Selection::single(range.anchor, range.head));
}

pub fn remove_primary_selection(cx: &mut Context) {
  let (view, doc) = current!(cx.editor);
  // TODO: handle count

  let selection = doc.selection(view.id);
  if selection.len() == 1 {
    cx.editor.set_error("no selections remaining");
    return;
  }
  let index = selection.primary_index();
  let selection = selection.clone().remove(index);

  doc.set_selection(view.id, selection);
}

fn get_lines(doc: &Document, view_id: ViewId) -> Vec<usize> {
  let mut lines = Vec::new();

  // Get all line numbers
  for range in doc.selection(view_id) {
    let (start, end) = range.line_range(doc.text().slice(..));

    for line in start..=end {
      lines.push(line)
    }
  }
  lines.sort_unstable(); // sorting by usize so _unstable is preferred
  lines.dedup();
  lines
}

pub fn indent(cx: &mut Context) {
  let count = cx.count();
  let (view, doc) = current!(cx.editor);
  let lines = get_lines(doc, view.id);

  // Indent by one level
  let indent = Tendril::from(doc.indent_style.as_str().repeat(count));

  let transaction = Transaction::change(
    doc.text(),
    lines.into_iter().filter_map(|line| {
      let is_blank = doc.text().line(line).chunks().all(|s| s.trim().is_empty());
      if is_blank {
        return None;
      }
      let pos = doc.text().line_to_char(line);
      Some((pos, pos, Some(indent.clone())))
    }),
  );
  doc.apply(&transaction, view.id);
  exit_select_mode(cx);
}

pub fn unindent(cx: &mut Context) {
  let count = cx.count();
  let (view, doc) = current!(cx.editor);
  let lines = get_lines(doc, view.id);
  let mut changes = Vec::with_capacity(lines.len());
  let tab_width = doc.tab_width();
  let indent_width = count * doc.indent_width();

  for line_idx in lines {
    let line = doc.text().line(line_idx);
    let mut width = 0;
    let mut pos = 0;

    for ch in line.chars() {
      match ch {
        ' ' => width += 1,
        '\t' => width = (width / tab_width + 1) * tab_width,
        _ => break,
      }

      pos += 1;

      if width >= indent_width {
        break;
      }
    }

    // now delete from start to first non-blank
    if pos > 0 {
      let start = doc.text().line_to_char(line_idx);
      changes.push((start, start + pos, None))
    }
  }

  let transaction = Transaction::change(doc.text(), changes.into_iter());

  doc.apply(&transaction, view.id);
  exit_select_mode(cx);
}

pub fn record_macro(cx: &mut Context) {
  if let Some((reg, mut keys)) = cx.editor.macro_recording.take() {
    // Remove the keypress which ends the recording
    keys.pop();
    let s = keys
      .into_iter()
      .map(|key| {
        let s = key.to_string();
        if s.chars().count() == 1 {
          s
        } else {
          format!("<{}>", s)
        }
      })
      .collect::<String>();
    match cx.editor.registers.write(reg, vec![s]) {
      Ok(_) => {
        cx.editor
          .set_status(format!("Recorded to register [{}]", reg))
      },
      Err(err) => cx.editor.set_error(err.to_string()),
    }
  } else {
    let reg = cx.register.take().unwrap_or('@');
    cx.editor.macro_recording = Some((reg, Vec::new()));
    cx.editor
      .set_status(format!("Recording to register [{}]", reg));
  }
}

pub fn replay_macro(cx: &mut Context) {
  let reg = cx.register.unwrap_or('@');

  if cx.editor.macro_replaying.contains(&reg) {
    cx.editor.set_error(format!(
      "Cannot replay from register [{}] because already replaying from same register",
      reg
    ));
    return;
  }

  let keys: Vec<KeyBinding> = if let Some(keys) = cx
    .editor
    .registers
    .read(reg, cx.editor)
    .filter(|values| values.len() == 1)
    .map(|mut values| values.next().unwrap())
  {
    match parse_macro(&keys) {
      Ok(keys) => keys,
      Err(err) => {
        cx.editor.set_error(format!("Invalid macro: {}", err));
        return;
      },
    }
  } else {
    cx.editor.set_error(format!("Register [{}] empty", reg));
    return;
  };

  // Once the macro has been fully validated, it's marked as being under replay
  // to ensure we don't fall into infinite recursion.
  cx.editor.macro_replaying.push(reg);

  let count = cx.count();
  cx.callback.push(Box::new(move |compositor, cx| {
    for _ in 0..count {
      for &key in keys.iter() {
        compositor.handle_event(&compositor::Event::Key(key), cx);
      }
    }
    // The macro under replay is cleared at the end of the callback, not in the
    // macro replay context, or it will not correctly protect the user from
    // replaying recursively.
    cx.editor.macro_replaying.pop();
  }));
}

pub fn toggle_button(cx: &mut Context) {
  // Toggle visibility of button components in the compositor
  cx.callback.push(Box::new(|compositor, cx| {
    use crate::ui::components::button::Button;
    for layer in compositor.layers.iter_mut() {
      if let Some(button) = layer.as_any_mut().downcast_mut::<Button>() {
        button.toggle_visible();
        break; // Toggle first button found
      }
    }
  }));
}

pub fn parse_macro(keys_str: &str) -> anyhow::Result<Vec<KeyBinding>> {
  use anyhow::Context;
  let mut keys_res: anyhow::Result<_> = Ok(Vec::new());
  let mut i = 0;
  while let Ok(keys) = &mut keys_res {
    if i >= keys_str.len() {
      break;
    }
    if !keys_str.is_char_boundary(i) {
      i += 1;
      continue;
    }

    let s = &keys_str[i..];
    let mut end_i = 1;
    while !s.is_char_boundary(end_i) {
      end_i += 1;
    }
    let c = &s[..end_i];
    if c == ">" {
      keys_res = Err(anyhow!("Unmatched '>'"));
    } else if c != "<" {
      keys.push(if c == "-" { "minus" } else { c });
      i += end_i;
    } else {
      match s.find('>').context("'>' expected") {
        Ok(end_i) => {
          keys.push(&s[1..end_i]);
          i += end_i + 1;
        },
        Err(err) => keys_res = Err(err),
      }
    }
  }
  keys_res.and_then(|keys| {
    keys
      .into_iter()
      .map(|s| s.parse::<KeyBinding>())
      .collect::<Result<Vec<_>, _>>()
      .map_err(|e| anyhow::anyhow!("Failed to parse key: {}", e))
  })
}
