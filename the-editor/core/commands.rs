use std::{
  borrow::Cow,
  char::{
    ToLowercase,
    ToUppercase,
  },
  collections::HashSet,
  num::NonZeroUsize,
};

use anyhow::{
  anyhow,
  bail,
  ensure,
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
use the_editor_stdx::{
  path::{
    self,
    find_paths,
  },
  rope::RopeSliceExt,
};
use the_editor_vcs::Hunk;
use url::Url;

use crate::{
  core::{
    Tendril,
    ViewId,
    auto_pairs,
    chars::char_is_word,
    comment,
    document::Document,
    global_search::{
      self as global_search_utils,
      FileResult,
      MatchControl,
      SearchOptions,
    },
    grapheme::{
      self,
      next_grapheme_boundary,
    },
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
    object,
    position::{
      Position,
      char_idx_at_visual_offset,
    },
    search::{
      self,
      CharMatcher,
    },
    selection::{
      self,
      Range,
      Selection,
    },
    surround,
    syntax::{
      Syntax,
      config::BlockCommentToken,
    },
    text_annotations::{
      Overlay,
      TextAnnotations,
    },
    text_format::TextFormat,
    textobject,
    transaction::{
      Deletion,
      Transaction,
    },
    tree,
    view::{
      Align,
      View,
      align_view,
    },
  },
  current,
  current_ref,
  doc,
  doc_mut,
  editor::{
    Action,
    Editor,
  },
  event::PostInsertChar,
  keymap::{
    KeyBinding,
    Mode,
  },
  view,
  view_mut,
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

fn extend_word_impl<F>(cx: &mut Context, extend_fn: F)
where
  F: Fn(RopeSlice, Range, usize) -> Range,
{
  let count = cx.count();
  let (view, doc) = current!(cx.editor);
  let text = doc.text().slice(..);

  let selection = doc.selection(view.id).clone().transform(|range| {
    let word = extend_fn(text, range, count);
    let pos = word.cursor(text);
    range.put_cursor(text, pos, true)
  });
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

pub fn extend_to_file_start(cx: &mut Context) {
  goto_file_start_impl(cx, Movement::Extend);
}

pub fn extend_to_last_line(cx: &mut Context) {
  goto_last_line_impl(cx, Movement::Extend)
}

pub fn extend_to_column(cx: &mut Context) {
  goto_column_impl(cx, Movement::Extend);
}

pub fn extend_line_up(cx: &mut Context) {
  move_impl(cx, move_vertically, Direction::Backward, Movement::Extend)
}

pub fn extend_line_down(cx: &mut Context) {
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

pub fn extend_next_word_start(cx: &mut Context) {
  extend_word_impl(cx, movement::move_next_word_start)
}

pub fn extend_prev_word_start(cx: &mut Context) {
  extend_word_impl(cx, movement::move_prev_word_start)
}

pub fn extend_next_word_end(cx: &mut Context) {
  extend_word_impl(cx, movement::move_next_word_end)
}

pub fn extend_prev_word_end(cx: &mut Context) {
  extend_word_impl(cx, movement::move_prev_word_end)
}

pub fn extend_next_long_word_start(cx: &mut Context) {
  extend_word_impl(cx, movement::move_next_long_word_start)
}

pub fn extend_prev_long_word_start(cx: &mut Context) {
  extend_word_impl(cx, movement::move_prev_long_word_start)
}

pub fn extend_prev_long_word_end(cx: &mut Context) {
  extend_word_impl(cx, movement::move_prev_long_word_end)
}

pub fn extend_next_long_word_end(cx: &mut Context) {
  extend_word_impl(cx, movement::move_next_long_word_end)
}

pub fn extend_next_sub_word_start(cx: &mut Context) {
  extend_word_impl(cx, movement::move_next_sub_word_start)
}

pub fn extend_prev_sub_word_start(cx: &mut Context) {
  extend_word_impl(cx, movement::move_prev_sub_word_start)
}

pub fn extend_prev_sub_word_end(cx: &mut Context) {
  extend_word_impl(cx, movement::move_prev_sub_word_end)
}

pub fn extend_next_sub_word_end(cx: &mut Context) {
  extend_word_impl(cx, movement::move_next_sub_word_end)
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

use unicode_width::UnicodeWidthChar;

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

pub fn delete_char_backward(cx: &mut Context) {
  let count = cx.count();
  let (view, doc) = current_ref!(cx.editor);
  let text = doc.text().slice(..);
  let tab_width = doc.tab_width();
  let indent_width = doc.indent_width();
  let auto_pairs = doc.auto_pairs(cx.editor);

  let transaction = Transaction::delete_by_selection(doc.text(), doc.selection(view.id), |range| {
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

  // Dispatch PostCommand event after command execution
  the_editor_event::dispatch(crate::event::PostCommand {
    command: "delete_char_backward",
    cx,
  });
}

pub fn delete_char_forward(cx: &mut Context) {
  let count = cx.count();
  delete_by_selection_insert_mode(
    cx,
    |text, range| {
      let pos = range.cursor(text);
      (pos, grapheme::nth_next_grapheme_boundary(text, pos, count))
    },
    Direction::Forward,
  )
}

fn exclude_cursor(text: RopeSlice, range: Range, cursor: Range) -> Range {
  if range.to() == cursor.to() && text.len_chars() != cursor.to() {
    Range::new(
      range.from(),
      grapheme::prev_grapheme_boundary(text, cursor.to()),
    )
  } else {
    range
  }
}

pub fn delete_word_backward(cx: &mut Context) {
  let count = cx.count();
  delete_by_selection_insert_mode(
    cx,
    |text, range| {
      let anchor = movement::move_prev_word_start(text, *range, count).from();
      let next = Range::new(anchor, range.cursor(text));
      let range = exclude_cursor(text, next, *range);
      (range.from(), range.to())
    },
    Direction::Backward,
  );
}

pub fn delete_word_forward(cx: &mut Context) {
  let count = cx.count();
  delete_by_selection_insert_mode(
    cx,
    |text, range| {
      let head = movement::move_next_word_end(text, *range, count).to();
      (range.cursor(text), head)
    },
    Direction::Forward,
  );
}

pub fn kill_to_line_end(cx: &mut Context) {
  delete_by_selection_insert_mode(
    cx,
    |text, range| {
      let line = range.cursor_line(text);
      let line_end_pos = line_end_char_index(&text, line);
      let pos = range.cursor(text);

      // if the cursor is on the newline char delete that
      if pos == line_end_pos {
        (pos, text.line_to_char(line + 1))
      } else {
        (pos, line_end_pos)
      }
    },
    Direction::Forward,
  );
}

pub fn kill_to_line_start(cx: &mut Context) {
  delete_by_selection_insert_mode(
    cx,
    move |text, range| {
      let line = range.cursor_line(text);
      let first_char = text.line_to_char(line);
      let anchor = range.cursor(text);
      let head = if anchor == first_char && line != 0 {
        // select until previous line
        line_end_char_index(&text, line - 1)
      } else if let Some(pos) = text.line(line).first_non_whitespace_char() {
        if first_char + pos < anchor {
          // select until first non-blank in line if cursor is after it
          first_char + pos
        } else {
          // select until start of line
          first_char
        }
      } else {
        // select until start of line
        first_char
      };
      (head, anchor)
    },
    Direction::Backward,
  );
}

fn delete_by_selection_insert_mode(
  cx: &mut Context,
  mut f: impl FnMut(RopeSlice, &Range) -> Deletion,
  direction: Direction,
) {
  let (view, doc) = current!(cx.editor);
  let text = doc.text().slice(..);
  let mut selection = SmallVec::new();
  let mut insert_newline = false;
  let text_len = text.len_chars();
  let mut transaction =
    Transaction::delete_by_selection(doc.text(), doc.selection(view.id), |range| {
      let (start, end) = f(text, range);
      if direction == Direction::Forward {
        let mut range = *range;
        if range.head > range.anchor {
          insert_newline |= end == text_len;
          // move the cursor to the right so that the selection
          // doesn't shrink when deleting forward (so the text appears to
          // move to  left)
          // += 1 is enough here as the range is normalized to grapheme boundaries
          // later anyway
          range.head += 1;
        }
        selection.push(range);
      }
      (start, end)
    });

  // in case we delete the last character and the cursor would be moved to the EOF
  // char insert a newline, just like when entering append mode
  if insert_newline {
    transaction = transaction.insert_at_eof(doc.line_ending.as_str().into());
  }

  if direction == Direction::Forward {
    doc.set_selection(
      view.id,
      Selection::new(selection, doc.selection(view.id).primary_index()),
    );
  }
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
  use std::sync::Arc;

  // Set editor mode to Command so statusline shows COMMAND
  cx.editor.set_mode(Mode::Command);

  // Clone the registry for use in the completion function
  let registry = cx.editor.command_registry.clone();

  // Create completion function that uses the command registry
  let completion_fn = Arc::new(move |editor: &crate::editor::Editor, input: &str| {
    registry.complete_command_line(editor, input)
  });

  // Create prompt with completion function
  let prompt =
    crate::ui::components::prompt::Prompt::new(String::new()).with_completion(completion_fn);

  cx.callback.push(Box::new(|compositor, _cx| {
    // Find the statusline and trigger slide animation
    for layer in compositor.layers.iter_mut() {
      if let Some(statusline) = layer
        .as_any_mut()
        .downcast_mut::<crate::ui::components::statusline::StatusLine>()
      {
        statusline.slide_for_prompt(true);
        break;
      }
    }

    compositor.push(Box::new(prompt));
  }));
}

pub fn normal_mode(cx: &mut Context) {
  cx.editor.set_mode(Mode::Normal);
}

pub fn select_regex(cx: &mut Context) {
  // Set custom mode string
  cx.editor.set_custom_mode_str("SELECT".to_string());

  // Set mode to Command so prompt is shown
  cx.editor.set_mode(Mode::Command);

  // Capture the original selection before starting the prompt
  let (view, doc) = current_ref!(cx.editor);
  let text = doc.text().slice(..);
  let original_selection = doc.selection(view.id).clone();

  // If original selection is just a point (cursor), expand to whole document
  // Otherwise, search within the existing selection
  let search_selection = if original_selection.primary().is_empty() {
    crate::core::selection::Selection::single(0, text.len_chars())
  } else {
    original_selection
  };

  // Create prompt with callback
  let prompt =
    crate::ui::components::Prompt::new(String::new()).with_callback(move |cx, input, event| {
      use crate::ui::components::prompt::PromptEvent;

      // Handle events
      match event {
        PromptEvent::Update | PromptEvent::Validate => {
          if matches!(event, PromptEvent::Validate) {
            // Clear custom mode string on validation
            cx.editor.clear_custom_mode_str();
          }

          // Skip empty input
          if input.is_empty() {
            return;
          }

          // Parse regex
          let regex = match the_editor_stdx::rope::Regex::new(input) {
            Ok(regex) => regex,
            Err(err) => {
              cx.editor.set_error(format!("Invalid regex: {}", err));
              return;
            },
          };

          let (view, doc) = current!(cx.editor);
          let text = doc.text().slice(..);

          // Use the captured original search selection
          // Apply select_on_matches
          if let Some(new_selection) =
            crate::core::selection::select_on_matches(text, &search_selection, &regex)
          {
            doc.set_selection(view.id, new_selection);
          } else if matches!(event, PromptEvent::Validate) {
            cx.editor.set_error("No matches found");
          }
        },
        PromptEvent::Abort => {
          // Clear custom mode string on abort
          cx.editor.clear_custom_mode_str();
        },
      }
    });

  // Push prompt to compositor with statusline slide animation
  cx.callback.push(Box::new(|compositor, _cx| {
    // Find the statusline and trigger slide animation
    for layer in compositor.layers.iter_mut() {
      if let Some(statusline) = layer
        .as_any_mut()
        .downcast_mut::<crate::ui::components::statusline::StatusLine>()
      {
        statusline.slide_for_prompt(true);
        break;
      }
    }

    compositor.push(Box::new(prompt));
  }));
}

pub fn file_picker(cx: &mut Context) {
  use std::{
    path::PathBuf,
    sync::{
      Arc,
      Mutex,
    },
  };

  let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
  let selected_file = Arc::new(Mutex::new(None::<PathBuf>));
  let selected_file_clone = selected_file.clone();

  cx.callback.push(Box::new(move |compositor, _cx| {
    let picker = crate::ui::file_picker(cwd, move |path: &PathBuf| {
      *selected_file.lock().unwrap() = Some(path.clone());
    });

    // Create a wrapper picker that will open the file when closed
    struct PickerWrapper {
      picker:        crate::ui::components::Picker<PathBuf, crate::ui::FilePickerData>,
      selected_file: Arc<Mutex<Option<PathBuf>>>,
    }

    impl crate::ui::compositor::Component for PickerWrapper {
      fn handle_event(
        &mut self,
        event: &crate::ui::compositor::Event,
        ctx: &mut crate::ui::compositor::Context,
      ) -> crate::ui::compositor::EventResult {
        let result = self.picker.handle_event(event, ctx);

        // If the picker is closing, check if a file was selected and open it
        if let crate::ui::compositor::EventResult::Consumed(Some(callback)) = result {
          let selected = self.selected_file.lock().unwrap().take();
          return crate::ui::compositor::EventResult::Consumed(Some(Box::new(
            move |compositor, ctx| {
              // First execute the picker's close callback
              callback(compositor, ctx);

              // Then open the selected file if any
              if let Some(path) = selected {
                use crate::editor::Action;
                if let Err(e) = ctx.editor.open(&path, Action::Replace) {
                  ctx.editor.set_error(format!("Failed to open file: {}", e));
                }
              }
            },
          )));
        }

        result
      }

      fn render(
        &mut self,
        area: crate::core::graphics::Rect,
        surface: &mut crate::ui::compositor::Surface,
        ctx: &mut crate::ui::compositor::Context,
      ) {
        self.picker.render(area, surface, ctx);
      }

      fn cursor(
        &self,
        area: crate::core::graphics::Rect,
        editor: &crate::editor::Editor,
      ) -> (
        Option<crate::core::position::Position>,
        crate::core::graphics::CursorKind,
      ) {
        self.picker.cursor(area, editor)
      }
    }

    compositor.push(Box::new(PickerWrapper {
      picker,
      selected_file: selected_file_clone,
    }));
  }));
}

pub fn buffer_picker(cx: &mut Context) {
  use std::path::{
    Path,
    PathBuf,
  };

  use crate::core::{
    DocumentId,
    document::SCRATCH_BUFFER_NAME,
  };

  let current = view!(cx.editor).doc;

  struct BufferMeta {
    id:          DocumentId,
    path:        Option<PathBuf>,
    is_modified: bool,
    is_current:  bool,
    focused_at:  std::time::Instant,
  }

  let new_meta = |doc: &Document| {
    BufferMeta {
      id:          doc.id(),
      path:        doc.path().cloned(),
      is_modified: doc.is_modified(),
      is_current:  doc.id() == current,
      focused_at:  doc.focused_at,
    }
  };

  let mut items = cx
    .editor
    .documents
    .values()
    .map(new_meta)
    .collect::<Vec<BufferMeta>>();

  // mru
  items.sort_unstable_by_key(|item| std::cmp::Reverse(item.focused_at));

  use crate::ui::components::{
    Column,
    Picker,
  };

  let columns = [
    Column::new("id", |meta: &BufferMeta, _| meta.id.to_string()),
    Column::new("flags", |meta: &BufferMeta, _| {
      let mut flags = String::new();
      if meta.is_modified {
        flags.push('+');
      }
      if meta.is_current {
        flags.push('*');
      }
      flags
    }),
    Column::new("path", |meta: &BufferMeta, _| {
      let path = meta
        .path
        .as_deref()
        .map(the_editor_stdx::path::get_relative_path);
      path
        .as_deref()
        .and_then(Path::to_str)
        .unwrap_or(SCRATCH_BUFFER_NAME)
        .to_string()
    }),
  ];
  use crate::{
    editor::Action,
    ui::components::PickerAction,
  };

  let action_handler = std::sync::Arc::new(
    move |meta: &BufferMeta, _: &(), picker_action: PickerAction| {
      let doc_id = meta.id;
      let action = match picker_action {
        PickerAction::Primary => Action::Replace,
        PickerAction::Secondary => Action::HorizontalSplit,
        PickerAction::Tertiary => Action::VerticalSplit,
      };

      crate::ui::job::dispatch_blocking(move |editor, _compositor| {
        editor.switch(doc_id, action);
      });

      true // Close picker
    },
  );

  let picker = Picker::new(columns, 2, items, (), |_| {})
    .with_action_handler(action_handler)
    .with_preview(|meta: &BufferMeta| {
      // Return the document's actual path for preview
      meta.path.as_ref().map(|path| (path.clone(), None))
    });

  cx.callback.push(Box::new(move |compositor, _cx| {
    compositor.push(Box::new(picker));
  }));
}

pub fn jumplist_picker(cx: &mut Context) {
  use std::path::{
    Path,
    PathBuf,
  };

  use crate::{
    core::{
      DocumentId,
      document::SCRATCH_BUFFER_NAME,
    },
    ui::components::{
      Column,
      Picker,
    },
  };

  struct JumpMeta {
    id:         DocumentId,
    path:       Option<PathBuf>,
    selection:  Selection,
    text:       String,
    line:       usize,
    is_current: bool,
  }

  for (view, _) in cx.editor.tree.views_mut() {
    for doc_id in view.jumps.iter().map(|e| e.0).collect::<Vec<_>>().iter() {
      let doc = doc_mut!(cx.editor, doc_id);
      view.sync_changes(doc);
    }
  }

  let new_meta = |view: &View, doc_id: DocumentId, selection: Selection| {
    let doc = &cx.editor.documents.get(&doc_id);
    let (text, line) = doc.map_or(("".into(), 0), |d| {
      let text = selection
        .fragments(d.text().slice(..))
        .map(Cow::into_owned)
        .collect::<Vec<_>>()
        .join(" ");
      let line = selection.primary().cursor_line(d.text().slice(..));
      (text, line)
    });

    JumpMeta {
      id: doc_id,
      path: doc.and_then(|d| d.path().cloned()),
      selection,
      text,
      line,
      is_current: view.doc == doc_id,
    }
  };

  let columns = [
    Column::new("id", |item: &JumpMeta, _| item.id.to_string()),
    Column::new("path", |item: &JumpMeta, _| {
      let path = item
        .path
        .as_deref()
        .map(the_editor_stdx::path::get_relative_path);
      path
        .as_deref()
        .and_then(Path::to_str)
        .unwrap_or(SCRATCH_BUFFER_NAME)
        .to_string()
    }),
    Column::new("flags", |item: &JumpMeta, _| {
      let mut flags = Vec::new();
      if item.is_current {
        flags.push("*");
      }

      if flags.is_empty() {
        "".to_string()
      } else {
        format!(" ({})", flags.join(""))
      }
    }),
    Column::new("contents", |item: &JumpMeta, _| item.text.clone()),
  ];

  use crate::{
    editor::Action,
    ui::components::PickerAction,
  };

  let action_handler = std::sync::Arc::new(
    move |meta: &JumpMeta, _: &(), picker_action: PickerAction| {
      let doc_id = meta.id;
      let selection = meta.selection.clone();
      let action = match picker_action {
        PickerAction::Primary => Action::Replace,
        PickerAction::Secondary => Action::HorizontalSplit,
        PickerAction::Tertiary => Action::VerticalSplit,
      };

      crate::ui::job::dispatch_blocking(move |editor, _compositor| {
        editor.switch(doc_id, action);
        let config = editor.config();
        let view = editor.tree.get_mut(editor.tree.focus);
        let doc = editor.documents.get_mut(&doc_id).unwrap();
        doc.set_selection(view.id, selection);
        view.ensure_cursor_in_view_center(doc, config.scrolloff);
      });

      true // Close picker
    },
  );

  let picker = Picker::new(
    columns,
    1, // path
    cx.editor.tree.views().flat_map(|(view, _)| {
      view
        .jumps
        .iter()
        .rev()
        .map(|(doc_id, selection)| new_meta(view, *doc_id, selection.clone()))
    }),
    (),
    |_| {},
  )
  .with_action_handler(action_handler)
  .with_preview(|meta: &JumpMeta| {
    // Return the document's actual path and the line from the jump
    meta
      .path
      .as_ref()
      .map(|path| (path.clone(), Some((meta.line, meta.line))))
  });

  cx.callback.push(Box::new(move |compositor, _cx| {
    compositor.push(Box::new(picker));
  }));
}

pub fn changed_file_picker(cx: &mut Context) {
  use std::path::PathBuf;

  use the_editor_vcs::FileChange;

  use crate::ui::components::{
    Column,
    Picker,
  };

  pub struct FileChangeData {
    cwd: PathBuf,
  }

  let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
  if !cwd.exists() {
    cx.editor
      .set_error("Current working directory does not exist".to_string());
    return;
  }

  let columns = [
    Column::new("change", |change: &FileChange, _data: &FileChangeData| {
      match change {
        FileChange::Untracked { .. } => "+ untracked".to_string(),
        FileChange::Modified { .. } => "~ modified".to_string(),
        FileChange::Conflict { .. } => "x conflict".to_string(),
        FileChange::Deleted { .. } => "- deleted".to_string(),
        FileChange::Renamed { .. } => "> renamed".to_string(),
      }
    }),
    Column::new("path", |change: &FileChange, data: &FileChangeData| {
      let display_path = |path: &PathBuf| {
        path
          .strip_prefix(&data.cwd)
          .unwrap_or(path)
          .display()
          .to_string()
      };
      match change {
        FileChange::Untracked { path } => display_path(path),
        FileChange::Modified { path } => display_path(path),
        FileChange::Conflict { path } => display_path(path),
        FileChange::Deleted { path } => display_path(path),
        FileChange::Renamed { from_path, to_path } => {
          format!("{} -> {}", display_path(from_path), display_path(to_path))
        },
      }
    }),
  ];

  use crate::{
    editor::Action,
    ui::components::PickerAction,
  };

  let action_handler = std::sync::Arc::new(
    move |meta: &FileChange, _: &FileChangeData, picker_action: PickerAction| {
      let path_to_open = meta.path().to_path_buf();
      let action = match picker_action {
        PickerAction::Primary => Action::Replace,
        PickerAction::Secondary => Action::HorizontalSplit,
        PickerAction::Tertiary => Action::VerticalSplit,
      };

      crate::ui::job::dispatch_blocking(move |editor, _compositor| {
        if let Err(e) = editor.open(&path_to_open, action) {
          editor.set_error(format!("Failed to open {}: {}", path_to_open.display(), e));
        }
      });

      true // Close picker
    },
  );

  let picker = Picker::new(
    columns,
    1, // path
    [],
    FileChangeData { cwd: cwd.clone() },
    |_| {},
  )
  .with_action_handler(action_handler)
  .with_preview(|meta: &FileChange| Some((meta.path().to_path_buf(), None)));
  let injector = picker.injector();

  cx.editor
    .diff_providers
    .clone()
    .for_each_changed_file(cwd, move |change| {
      match change {
        Ok(change) => injector.push(change).is_ok(),
        Err(err) => {
          log::error!("Failed to get file changes: {}", err);
          true
        },
      }
    });

  cx.callback.push(Box::new(move |compositor, _cx| {
    compositor.push(Box::new(picker));
  }));
}

pub fn global_search(cx: &mut Context) {
  use std::{
    sync::Arc,
    thread,
  };

  use grep_regex::RegexMatcherBuilder;
  use the_editor_stdx::{
    env,
    path,
  };

  use crate::{
    editor::Action,
    ui::components::{
      Column,
      Picker,
      PickerAction,
      picker::Injector,
    },
  };

  #[derive(Clone)]
  struct GlobalSearchConfig {
    search_options: Arc<SearchOptions>,
  }

  let editor_config = cx.editor.config();
  let documents_snapshot: Vec<_> = cx
    .editor
    .documents()
    .map(|doc| (doc.path().cloned(), doc.text().clone()))
    .collect();

  let search_options = Arc::new(SearchOptions {
    smart_case:  editor_config.search.smart_case,
    file_picker: editor_config.file_picker.clone(),
    documents:   Arc::new(documents_snapshot),
  });

  let config = Arc::new(GlobalSearchConfig {
    search_options: Arc::clone(&search_options),
  });

  let columns = [Column::new(
    "path",
    |item: &FileResult, _config: &Arc<GlobalSearchConfig>| {
      let relative = path::get_relative_path(&item.path);
      let relative = relative.as_ref();

      let directories = relative
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map(|p| format!("{}{}", p.display(), std::path::MAIN_SEPARATOR))
        .unwrap_or_default();

      let filename = relative.file_name().unwrap_or_default().to_string_lossy();
      let snippet = item.line_text.trim();

      if snippet.is_empty() {
        format!("{}{}:{}", directories, filename, item.line_num + 1)
      } else {
        format!(
          "{}{}:{}  {}",
          directories,
          filename,
          item.line_num + 1,
          snippet
        )
      }
    },
  )];

  let reg = cx.register.unwrap_or('/');
  cx.editor.registers.last_search_register = reg;

  let query_callback = {
    let config = Arc::clone(&config);
    Arc::new(
      move |query: String, injector: Injector<FileResult, Arc<GlobalSearchConfig>>| {
        let query = query.trim().to_string();
        if query.is_empty() {
          return;
        }

        let config = Arc::clone(&config);
        let injector = injector.clone();

        if RegexMatcherBuilder::new()
          .case_smart(config.search_options.smart_case)
          .build(&query)
          .is_err()
        {
          log::info!(
            "Failed to compile search pattern in global search: {}",
            query
          );
          return;
        }

        thread::spawn(move || {
          let search_root = env::current_working_dir();
          let handler = Arc::new(move |result: FileResult| -> MatchControl {
            if injector.push(result.clone()).is_err() {
              MatchControl::Stop
            } else {
              MatchControl::Continue
            }
          });

          if let Err(err) = global_search_utils::walk_workspace_matches(
            &query,
            &search_root,
            &config.search_options,
            handler,
          ) {
            if err.downcast_ref::<regex::Error>().is_some() {
              log::info!("Failed to compile search pattern in global search: {}", err);
            } else {
              log::error!("Global search failed: {}", err);
            }
          }
        });
      },
    )
  };

  let action_handler = std::sync::Arc::new(
    move |item: &FileResult,
          _config: &Arc<GlobalSearchConfig>,
          picker_action: PickerAction|
          -> bool {
      let path = item.path.clone();
      let line = item.line_num;
      let action = match picker_action {
        PickerAction::Primary => Action::Replace,
        PickerAction::Secondary => Action::HorizontalSplit,
        PickerAction::Tertiary => Action::VerticalSplit,
      };

      crate::ui::job::dispatch_blocking(move |editor, _compositor| {
        match editor.open(&path, action) {
          Ok(doc_id) => {
            let mut invalid_line = false;

            {
              if let Some(doc) = editor.documents.get_mut(&doc_id) {
                let text = doc.text();
                if line >= text.len_lines() {
                  invalid_line = true;
                } else {
                  let start = text.line_to_char(line);
                  let end = text.line_to_char((line + 1).min(text.len_lines()));
                  let view_id = editor.tree.focus;
                  let view = editor.tree.get_mut(view_id);
                  doc.set_selection(view.id, Selection::single(start, end));

                  if action.align_view(view, doc.id()) {
                    align_view(doc, view, Align::Center);
                  }
                }
              } else {
                invalid_line = true;
              }
            }

            if invalid_line {
              editor.set_error(
                "The line you jumped to does not exist anymore because the file has changed."
                  .to_string(),
              );
            }
          },
          Err(err) => {
            editor.set_error(format!("Failed to open file '{}': {}", path.display(), err));
          },
        }
      });

      true
    },
  );

  let picker = Picker::new(
    columns,
    0,
    Vec::<FileResult>::new(),
    Arc::clone(&config),
    |_| {},
  )
  .with_action_handler(action_handler)
  .with_preview(|item: &FileResult| Some((item.path.clone(), Some((item.line_num, item.line_num)))))
  .with_history_register(
    reg,
    |item: &FileResult, _config: &Arc<GlobalSearchConfig>| {
      format!(
        "{}:{} {}",
        item.path.display(),
        item.line_num + 1,
        item.line_text.trim()
      )
    },
  )
  .with_dynamic_query(query_callback)
  .with_debounce(275);

  cx.callback.push(Box::new(move |compositor, _cx| {
    compositor.push(Box::new(picker));
  }));
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

fn goto_window(cx: &mut Context, align: Align) {
  let count = cx.count.map(|c| c.get()).unwrap_or(1).saturating_sub(1);
  let config = cx.editor.config();
  let (view, doc) = current!(cx.editor);
  let view_offset = doc.view_offset(view.id);

  let height = view.inner_height();

  // respect user given count if any
  // - 1 so we have at least one gap in the middle.
  // a height of 6 with padding of 3 on each side will keep shifting the view back
  // and forth as we type
  let scrolloff = config.scrolloff.min(height.saturating_sub(1) / 2);

  let last_visual_line = view.last_visual_line(doc);

  let visual_line = match align {
    Align::Top => view_offset.vertical_offset + scrolloff + count,
    Align::Center => view_offset.vertical_offset + (last_visual_line / 2),
    Align::Bottom => {
      view_offset.vertical_offset + last_visual_line.saturating_sub(scrolloff + count)
    },
  };
  let visual_line = visual_line
    .max(view_offset.vertical_offset + scrolloff)
    .min(view_offset.vertical_offset + last_visual_line.saturating_sub(scrolloff));

  let pos = view
    .pos_at_visual_coords(doc, visual_line as u16, 0, false)
    .expect("visual_line was constrained to the view area");

  let text = doc.text().slice(..);
  let selection = doc
    .selection(view.id)
    .clone()
    .transform(|range| range.put_cursor(text, pos, cx.editor.mode == Mode::Select));
  doc.set_selection(view.id, selection);
}

pub fn goto_window_top(cx: &mut Context) {
  goto_window(cx, Align::Top);
}

pub fn goto_window_center(cx: &mut Context) {
  goto_window(cx, Align::Center);
}

pub fn goto_window_bottom(cx: &mut Context) {
  goto_window(cx, Align::Bottom);
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

pub fn extend_to_line_start(cx: &mut Context) {
  let (view, doc) = current!(cx.editor);
  goto_line_start_impl(view, doc, Movement::Extend)
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

pub fn extend_to_line_end(cx: &mut Context) {
  let (view, doc) = current!(cx.editor);
  goto_line_end_impl(view, doc, Movement::Extend)
}

pub fn goto_line_end_newline(cx: &mut Context) {
  let (view, doc) = current!(cx.editor);
  goto_line_end_newline_impl(
    view,
    doc,
    if cx.editor.mode == Mode::Select {
      Movement::Extend
    } else {
      Movement::Move
    },
  )
}

fn goto_line_end_newline_impl(view: &mut View, doc: &mut Document, movement: Movement) {
  let text = doc.text().slice(..);

  let selection = doc.selection(view.id).clone().transform(|range| {
    let line = range.cursor_line(text);
    let pos = line_end_char_index(&text, line);

    range.put_cursor(text, pos, movement == Movement::Extend)
  });
  doc.set_selection(view.id, selection);
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
        insert_char(cx, c);
      }
    })
  }
}

pub fn move_parent_node_end(cx: &mut Context) {
  move_node_bound_impl(cx, Direction::Forward, Movement::Move)
}

pub fn extend_parent_node_end(cx: &mut Context) {
  move_node_bound_impl(cx, Direction::Forward, Movement::Extend)
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
  cx.callback.push(Box::new(|compositor, _cx| {
    use crate::ui::components::button::Button;
    for layer in compositor.layers.iter_mut() {
      if let Some(button) = layer.as_any_mut().downcast_mut::<Button>() {
        button.toggle_visible();
        break; // Toggle first button found
      }
    }
  }));
}

pub fn toggle_statusline(cx: &mut Context) {
  // Toggle visibility of statusline with animation
  cx.callback.push(Box::new(|compositor, _cx| {
    use crate::ui::components::statusline::StatusLine;
    for layer in compositor.layers.iter_mut() {
      if let Some(statusline) = layer.as_any_mut().downcast_mut::<StatusLine>() {
        statusline.toggle();
        break;
      }
    }
  }));
}

pub fn increase_font_size(cx: &mut Context) {
  let new_size = (cx
    .editor
    .font_size_override
    .unwrap_or(cx.editor.config().font_size)
    + 2.0)
    .min(72.0);
  cx.editor.font_size_override = Some(new_size);
  cx.editor.set_status(format!("Font size: {}", new_size));
}

pub fn decrease_font_size(cx: &mut Context) {
  let new_size = (cx
    .editor
    .font_size_override
    .unwrap_or(cx.editor.config().font_size)
    - 2.0)
    .max(8.0);
  cx.editor.font_size_override = Some(new_size);
  cx.editor.set_status(format!("Font size: {}", new_size));
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

pub fn commit_undo_checkpoint(cx: &mut Context) {
  let (view, doc) = current!(cx.editor);
  doc.append_changes_to_history(view);
}

/// Toggle line numbers gutter
pub fn toggle_line_numbers(cx: &mut Context) {
  cx.callback.push(Box::new(|compositor, cx| {
    if let Some(editor_view) = compositor.find::<crate::ui::editor_view::EditorView>() {
      let toggled = editor_view.gutter_manager.toggle_gutter("line-numbers");
      if toggled {
        // Force redraw of all lines since text positions will change
        editor_view.mark_all_dirty();
        cx.editor.set_status("Toggled line numbers");
      }
    }
  }));
}

/// Toggle diagnostics gutter
pub fn toggle_diagnostics_gutter(cx: &mut Context) {
  cx.callback.push(Box::new(|compositor, cx| {
    if let Some(editor_view) = compositor.find::<crate::ui::editor_view::EditorView>() {
      let toggled = editor_view.gutter_manager.toggle_gutter("diagnostics");
      if toggled {
        // Force redraw of all lines since text positions will change
        editor_view.mark_all_dirty();
        cx.editor.set_status("Toggled diagnostics gutter");
      }
    }
  }));
}

/// Toggle diff gutter
pub fn toggle_diff_gutter(cx: &mut Context) {
  cx.callback.push(Box::new(|compositor, cx| {
    if let Some(editor_view) = compositor.find::<crate::ui::editor_view::EditorView>() {
      let toggled = editor_view.gutter_manager.toggle_gutter("diff");
      if toggled {
        // Force redraw of all lines since text positions will change
        editor_view.mark_all_dirty();
        cx.editor.set_status("Toggled diff gutter");
      }
    }
  }));
}

/// Show list of all gutters and their state
pub fn list_gutters(cx: &mut Context) {
  cx.callback.push(Box::new(|compositor, cx| {
    if let Some(editor_view) = compositor.find::<crate::ui::editor_view::EditorView>() {
      let gutters = editor_view.gutter_manager.list_gutters();
      let status = gutters
        .iter()
        .map(|(_id, name, enabled)| format!("{}: {}", name, if *enabled { "ON" } else { "OFF" }))
        .collect::<Vec<_>>()
        .join(", ");
      cx.editor.set_status(format!("Gutters: {}", status));
    }
  }));
}

/// Manually trigger completion (bound to C-x in insert mode)
pub fn completion(cx: &mut Context) {
  let (view, doc) = current!(cx.editor);
  let text = doc.text().slice(..);
  let cursor = doc.selection(view.id).primary().cursor(text);

  log::info!("Manual completion command called");
  log::info!("  Cursor position: {}", cursor);
  log::info!("  Document ID: {:?}", doc.id);
  log::info!("  View ID: {:?}", view.id);
  log::info!("  Mode: {:?}", cx.editor.mode);

  // Log some context around cursor
  let start = cursor.saturating_sub(10);
  let end = (cursor + 10).min(text.len_chars());
  let context = text.slice(start..end).to_string();
  log::info!("  Context: {:?}", context);

  // Trigger manual completion via handlers
  cx.editor
    .handlers
    .trigger_completions(cursor, doc.id, view.id);

  // Dispatch PostCommand event
  the_editor_event::dispatch(crate::event::PostCommand {
    command: "completion",
    cx,
  });
}

// pub fn insert_register(cx: &mut Context) {
//   cx.editor.autoinfo = Some(Info::from_registers(
//     "Insert register",
//     &cx.editor.registers,
//   ));
//   cx.on_next_key(move |cx, event| {
//     cx.editor.autoinfo = None;
//     if let Some(Key::Char(ch)) = event.code {
//       cx.register = Some(ch);
//       paste(
//         cx.editor,
//         cx.register
//           .unwrap_or(cx.editor.config().default_yank_register),
//         Paste::Cursor,
//         cx.count(),
//       );
//     }
//   })
// }

/// Show LSP hover information at cursor
pub fn hover(_cx: &mut Context) {
  // Spawn async task to request hover information
  tokio::spawn(async move {
    let _ = crate::handlers::hover::request_hover().await;
  });
}

/// Goto next diagnostic
pub fn goto_next_diag(cx: &mut Context) {
  let (view, doc) = current!(cx.editor);

  let cursor_pos = doc
    .selection(view.id)
    .primary()
    .cursor(doc.text().slice(..));

  let diag = doc
    .diagnostics()
    .iter()
    .find(|diag| diag.range.start > cursor_pos);

  let Some(diag) = diag else {
    cx.editor.set_status("No next diagnostic");
    return;
  };

  doc.set_selection(view.id, Selection::single(diag.range.start, diag.range.end));
  crate::core::view::align_view(doc, view, crate::core::view::Align::Center);
}

/// Goto previous diagnostic
pub fn goto_prev_diag(cx: &mut Context) {
  let (view, doc) = current!(cx.editor);

  let cursor_pos = doc
    .selection(view.id)
    .primary()
    .cursor(doc.text().slice(..));

  let diag = doc
    .diagnostics()
    .iter()
    .rev()
    .find(|diag| diag.range.start < cursor_pos);

  let Some(diag) = diag else {
    cx.editor.set_status("No previous diagnostic");
    return;
  };

  // Selection is reversed to match Helix behavior (going backwards)
  doc.set_selection(view.id, Selection::single(diag.range.end, diag.range.start));
  crate::core::view::align_view(doc, view, crate::core::view::Align::Center);
}

pub fn goto_file_impl(cx: &mut Context, action: Action) {
  let (view, doc) = current_ref!(cx.editor);
  let text = doc.text().slice(..);
  let selections = doc.selection(view.id);
  let primary = selections.primary();
  let rel_path = doc
    .relative_path()
    .map(|path| {
      path
        .parent()
        .unwrap_or(std::path::Path::new("."))
        .to_path_buf()
    })
    .unwrap_or_default();

  let paths: Vec<_> = if selections.len() == 1 && primary.len() == 1 {
    // Cap the search at roughly 1k bytes around the cursor
    let lookaround = 1000;
    let pos = text.char_to_byte(primary.cursor(text));
    let pos_line = text.byte_to_line(pos);

    // Calculate search bounds
    let search_start = if pos < lookaround {
      0
    } else {
      text.line_to_byte(text.byte_to_line(pos.saturating_sub(lookaround)))
    };

    let search_end_line = (pos_line + 1).min(text.len_lines().saturating_sub(1));
    let search_end = text
      .line_to_byte(search_end_line)
      .min((pos + lookaround).min(text.len_bytes()));

    let search_range = text.byte_slice(search_start..search_end);

    // Try to find a path near the cursor
    let path = find_paths(search_range, true)
      .take_while(|range| search_start + range.start <= pos + 1)
      .find(|range| pos <= search_start + range.end)
      .map(|range| Cow::from(search_range.byte_slice(range)));

    log::debug!("goto_file auto-detected path: {path:?}");
    let path = path.unwrap_or_else(|| primary.fragment(text));
    vec![path.into_owned()]
  } else {
    // Otherwise use each selection, trimmed.
    selections
      .fragments(text)
      .map(|sel| sel.trim().to_owned())
      .filter(|sel| !sel.is_empty())
      .collect()
  };

  for sel in paths {
    // Try parsing as URL first
    if let Ok(url) = Url::parse(&sel) {
      if url.scheme() == "file" {
        // Extract path from file:// URL
        if let Ok(path) = url.to_file_path() {
          let final_path = if path.is_absolute() {
            path
          } else {
            rel_path.join(path)
          };

          if let Err(e) = cx.editor.open(&final_path, action) {
            cx.editor
              .set_error(format!("Failed to open {}: {}", final_path.display(), e));
          }
        }
      } else {
        // Non-file URLs - just show an error for now
        cx.editor.set_error(format!("Cannot open URL: {}", url));
      }
      continue;
    }

    // Treat as a regular file path
    let path = path::expand(&sel);
    let final_path = if path.is_absolute() {
      path.into_owned()
    } else {
      rel_path.join(path)
    };

    if final_path.is_dir() {
      cx.editor
        .set_error(format!("{} is a directory", final_path.display()));
    } else if let Err(e) = cx.editor.open(&final_path, action) {
      cx.editor
        .set_error(format!("Failed to open {}: {}", final_path.display(), e));
    }
  }
}

/// Open the file under the cursor or selection (gf)
pub fn goto_file(cx: &mut Context) {
  goto_file_impl(cx, Action::Replace);
}

pub fn goto_last_accessed_file(cx: &mut Context) {
  let view = view_mut!(cx.editor);
  if let Some(alt) = view.docs_access_history.pop() {
    cx.editor.switch(alt, Action::Replace);
  } else {
    cx.editor.set_error("no last accessed buffer")
  }
}

pub fn goto_last_modified_file(cx: &mut Context) {
  let view = view!(cx.editor);
  let alternate_file = view
    .last_modified_docs
    .into_iter()
    .flatten()
    .find(|&id| id != view.doc);
  if let Some(alt) = alternate_file {
    cx.editor.switch(alt, Action::Replace);
  } else {
    cx.editor.set_error("no last modified buffer")
  }
}

pub fn goto_next_buffer(cx: &mut Context) {
  goto_buffer(cx.editor, Direction::Forward, cx.count());
}

pub fn goto_previous_buffer(cx: &mut Context) {
  goto_buffer(cx.editor, Direction::Backward, cx.count());
}

fn goto_buffer(editor: &mut Editor, direction: Direction, count: usize) {
  let current = view!(editor).doc;

  let id = match direction {
    Direction::Forward => {
      let iter = editor.documents.keys();
      // skip 'count' times past current buffer
      iter.cycle().skip_while(|id| *id != &current).nth(count)
    },
    Direction::Backward => {
      let iter = editor.documents.keys();
      // skip 'count' times past current buffer
      iter
        .rev()
        .cycle()
        .skip_while(|id| *id != &current)
        .nth(count)
    },
  }
  .unwrap();

  let id = *id;

  editor.switch(id, Action::Replace);
}

pub fn move_line_up(cx: &mut Context) {
  move_impl(cx, move_vertically, Direction::Backward, Movement::Move)
}

pub fn move_line_down(cx: &mut Context) {
  move_impl(cx, move_vertically, Direction::Forward, Movement::Move)
}

pub fn goto_last_modification(cx: &mut Context) {
  let (view, doc) = current!(cx.editor);
  let pos = doc.history.get_mut().last_edit_pos();
  let text = doc.text().slice(..);
  if let Some(pos) = pos {
    let selection = doc
      .selection(view.id)
      .clone()
      .transform(|range| range.put_cursor(text, pos, cx.editor.mode == Mode::Select));
    doc.set_selection(view.id, selection);
  }
}

fn jump_to_word(cx: &mut Context, behaviour: Movement) {
  // Calculate the jump candidates: ranges for any visible words with two or
  // more characters.
  let alphabet = &cx.editor.config().jump_label_alphabet;
  if alphabet.is_empty() {
    return;
  }

  let jump_label_limit = alphabet.len() * alphabet.len();
  let mut words = Vec::with_capacity(jump_label_limit);
  let (view, doc) = current_ref!(cx.editor);
  let text = doc.text().slice(..);

  // This is not necessarily exact if there is virtual text like soft wrap.
  // It's ok though because the extra jump labels will not be rendered.
  let start = text.line_to_char(text.char_to_line(doc.view_offset(view.id).anchor));
  let end = text.line_to_char(view.estimate_last_doc_line(doc) + 1);

  let primary_selection = doc.selection(view.id).primary();
  let cursor = primary_selection.cursor(text);
  let mut cursor_fwd = Range::point(cursor);
  let mut cursor_rev = Range::point(cursor);
  if text.get_char(cursor).is_some_and(|c| !c.is_whitespace()) {
    let cursor_word_end = movement::move_next_word_end(text, cursor_fwd, 1);
    //  single grapheme words need a special case
    if cursor_word_end.anchor == cursor {
      cursor_fwd = cursor_word_end;
    }
    let cursor_word_start = movement::move_prev_word_start(text, cursor_rev, 1);
    if cursor_word_start.anchor == next_grapheme_boundary(text, cursor) {
      cursor_rev = cursor_word_start;
    }
  }
  'outer: loop {
    let mut changed = false;
    while cursor_fwd.head < end {
      cursor_fwd = movement::move_next_word_end(text, cursor_fwd, 1);
      // The cursor is on a word that is atleast two graphemes long and
      // madeup of word characters. The latter condition is needed because
      // move_next_word_end simply treats a sequence of characters from
      // the same char class as a word so `=<` would also count as a word.
      let add_label = text
        .slice(..cursor_fwd.head)
        .graphemes_rev()
        .take(2)
        .take_while(|g| g.chars().all(char_is_word))
        .count()
        == 2;
      if !add_label {
        continue;
      }
      changed = true;
      // skip any leading whitespace
      cursor_fwd.anchor += text
        .chars_at(cursor_fwd.anchor)
        .take_while(|&c| !char_is_word(c))
        .count();
      words.push(cursor_fwd);
      if words.len() == jump_label_limit {
        break 'outer;
      }
      break;
    }
    while cursor_rev.head > start {
      cursor_rev = movement::move_prev_word_start(text, cursor_rev, 1);
      // The cursor is on a word that is atleast two graphemes long and
      // madeup of word characters. The latter condition is needed because
      // move_prev_word_start simply treats a sequence of characters from
      // the same char class as a word so `=<` would also count as a word.
      let add_label = text
        .slice(cursor_rev.head..)
        .graphemes()
        .take(2)
        .take_while(|g| g.chars().all(char_is_word))
        .count()
        == 2;
      if !add_label {
        continue;
      }
      changed = true;
      cursor_rev.anchor -= text
        .chars_at(cursor_rev.anchor)
        .reversed()
        .take_while(|&c| !char_is_word(c))
        .count();
      words.push(cursor_rev);
      if words.len() == jump_label_limit {
        break 'outer;
      }
      break;
    }
    if !changed {
      break;
    }
  }
  jump_to_label(cx, words, behaviour)
}

fn jump_to_label(cx: &mut Context, labels: Vec<Range>, behaviour: Movement) {
  let doc = doc!(cx.editor);
  let alphabet = &cx.editor.config().jump_label_alphabet;
  if labels.is_empty() {
    return;
  }
  let alphabet_char = |i| {
    let mut res = Tendril::new();
    res.push(alphabet[i]);
    res
  };

  // Add label for each jump candidate to the View as virtual text.
  let text = doc.text().slice(..);
  let mut overlays: Vec<_> = labels
    .iter()
    .enumerate()
    .flat_map(|(i, range)| {
      [
        Overlay::new(range.from(), alphabet_char(i / alphabet.len())),
        Overlay::new(
          grapheme::next_grapheme_boundary(text, range.from()),
          alphabet_char(i % alphabet.len()),
        ),
      ]
    })
    .collect();
  overlays.sort_unstable_by_key(|overlay| overlay.char_idx);
  let (view, doc) = current!(cx.editor);
  doc.set_jump_labels(view.id, overlays);

  // Accept two characters matching a visible label. Jump to the candidate
  // for that label if it exists.
  let primary_selection = doc.selection(view.id).primary();
  let view = view.id;
  let doc = doc.id();
  cx.on_next_key(move |cx, event| {
    let alphabet = &cx.editor.config().jump_label_alphabet;
    // Extract char from KeyPress and check for no modifiers
    let ch = match event.code {
      the_editor_renderer::Key::Char(c) if !event.shift && !event.ctrl && !event.alt => c,
      _ => {
        doc_mut!(cx.editor, &doc).remove_jump_labels(view);
        return;
      },
    };
    let Some(i) = alphabet.iter().position(|&it| it == ch) else {
      doc_mut!(cx.editor, &doc).remove_jump_labels(view);
      return;
    };
    let outer = i * alphabet.len();
    // Bail if the given character cannot be a jump label.
    if outer > labels.len() {
      doc_mut!(cx.editor, &doc).remove_jump_labels(view);
      return;
    }
    cx.on_next_key(move |cx, event| {
      doc_mut!(cx.editor, &doc).remove_jump_labels(view);
      let alphabet = &cx.editor.config().jump_label_alphabet;
      // Extract char from KeyPress and check for no modifiers
      let ch = match event.code {
        the_editor_renderer::Key::Char(c) if !event.shift && !event.ctrl && !event.alt => c,
        _ => return,
      };
      let Some(inner) = alphabet.iter().position(|&it| it == ch) else {
        return;
      };
      if let Some(mut range) = labels.get(outer + inner).copied() {
        range = if behaviour == Movement::Extend {
          let anchor = if range.anchor < range.head {
            let from = primary_selection.from();
            if range.anchor < from {
              range.anchor
            } else {
              from
            }
          } else {
            let to = primary_selection.to();
            if range.anchor > to { range.anchor } else { to }
          };
          Range::new(anchor, range.head)
        } else {
          range.with_direction(Direction::Forward)
        };
        doc_mut!(cx.editor, &doc).set_selection(view, range.into());
      }
    });
  });
}

pub fn goto_word(cx: &mut Context) {
  jump_to_word(cx, Movement::Move)
}

pub fn extend_to_word(cx: &mut Context) {
  jump_to_word(cx, Movement::Extend)
}

pub fn split_selection_on_newline(cx: &mut Context) {
  let (view, doc) = current!(cx.editor);
  let text = doc.text().slice(..);
  let selection = selection::split_on_newline(text, doc.selection(view.id));
  doc.set_selection(view.id, selection);
}

pub fn merge_selections(cx: &mut Context) {
  let (view, doc) = current!(cx.editor);
  let selection = doc.selection(view.id).clone().merge_ranges();
  doc.set_selection(view.id, selection);
}

pub fn merge_consecutive_selections(cx: &mut Context) {
  let (view, doc) = current!(cx.editor);
  let selection = doc.selection(view.id).clone().merge_consecutive_ranges();
  doc.set_selection(view.id, selection);
}

pub fn split_selection(cx: &mut Context) {
  // Set custom mode string
  cx.editor.set_custom_mode_str("SPLIT".to_string());

  // Set mode to Command so prompt is shown
  cx.editor.set_mode(Mode::Command);

  // Create prompt with callback
  let prompt =
    crate::ui::components::Prompt::new(String::new()).with_callback(|cx, input, event| {
      use crate::ui::components::prompt::PromptEvent;

      // Handle events
      match event {
        PromptEvent::Update | PromptEvent::Validate => {
          if matches!(event, PromptEvent::Validate) {
            // Clear custom mode string on validation
            cx.editor.clear_custom_mode_str();
          }

          // Skip empty input
          if input.is_empty() {
            return;
          }

          // Parse regex
          let regex = match the_editor_stdx::rope::Regex::new(input) {
            Ok(regex) => regex,
            Err(err) => {
              cx.editor.set_error(format!("Invalid regex: {}", err));
              return;
            },
          };

          let (view, doc) = current!(cx.editor);
          let text = doc.text().slice(..);
          let selection =
            crate::core::selection::split_on_matches(text, doc.selection(view.id), &regex);
          doc.set_selection(view.id, selection);
        },
        PromptEvent::Abort => {
          // Clear custom mode string on abort
          cx.editor.clear_custom_mode_str();
        },
      }
    });

  // Push prompt to compositor with statusline slide animation
  cx.callback.push(Box::new(|compositor, _cx| {
    // Find the statusline and trigger slide animation
    for layer in compositor.layers.iter_mut() {
      if let Some(statusline) = layer
        .as_any_mut()
        .downcast_mut::<crate::ui::components::statusline::StatusLine>()
      {
        statusline.slide_for_prompt(true);
        break;
      }
    }

    compositor.push(Box::new(prompt));
  }));
}

pub fn collapse_selection(cx: &mut Context) {
  let (view, doc) = current!(cx.editor);
  let text = doc.text().slice(..);

  let selection = doc.selection(view.id).clone().transform(|range| {
    let pos = range.cursor(text);
    Range::new(pos, pos)
  });
  doc.set_selection(view.id, selection);
}

pub fn flip_selections(cx: &mut Context) {
  let (view, doc) = current!(cx.editor);

  let selection = doc
    .selection(view.id)
    .clone()
    .transform(|range| range.flip());
  doc.set_selection(view.id, selection);
}

pub fn expand_selection(cx: &mut Context) {
  let motion = |editor: &mut Editor| {
    let (view, doc) = current!(editor);

    if let Some(syntax) = doc.syntax() {
      let text = doc.text().slice(..);

      let current_selection = doc.selection(view.id);
      let selection = object::expand_selection(syntax, text, current_selection.clone());

      // check if selection is different from the last one
      if *current_selection != selection {
        // save current selection so it can be restored using shrink_selection
        view.object_selections.push(current_selection.clone());

        doc.set_selection(view.id, selection);
      }
    }
  };
  cx.editor.apply_motion(motion);
}

pub fn shrink_selection(cx: &mut Context) {
  let motion = |editor: &mut Editor| {
    let (view, doc) = current!(editor);
    let current_selection = doc.selection(view.id);
    // try to restore previous selection
    if let Some(prev_selection) = view.object_selections.pop() {
      if current_selection.contains(&prev_selection) {
        doc.set_selection(view.id, prev_selection);
        return;
      } else {
        // clear existing selection as they can't be shrunk to anyway
        view.object_selections.clear();
      }
    }
    // if not previous selection, shrink to first child
    if let Some(syntax) = doc.syntax() {
      let text = doc.text().slice(..);
      let selection = object::shrink_selection(syntax, text, current_selection.clone());
      doc.set_selection(view.id, selection);
    }
  };
  cx.editor.apply_motion(motion);
}

fn select_all_impl<F>(editor: &mut Editor, select_fn: F)
where
  F: Fn(&Syntax, RopeSlice, Selection) -> Selection,
{
  let (view, doc) = current!(editor);

  if let Some(syntax) = doc.syntax() {
    let text = doc.text().slice(..);
    let current_selection = doc.selection(view.id);
    let selection = select_fn(syntax, text, current_selection.clone());
    doc.set_selection(view.id, selection);
  }
}

fn select_sibling_impl<F>(cx: &mut Context, sibling_fn: F)
where
  F: Fn(&Syntax, RopeSlice, Selection) -> Selection + 'static,
{
  let motion = move |editor: &mut Editor| {
    let (view, doc) = current!(editor);

    if let Some(syntax) = doc.syntax() {
      let text = doc.text().slice(..);
      let current_selection = doc.selection(view.id);
      let selection = sibling_fn(syntax, text, current_selection.clone());
      doc.set_selection(view.id, selection);
    }
  };
  cx.editor.apply_motion(motion);
}

pub fn select_all_children(cx: &mut Context) {
  let motion = |editor: &mut Editor| {
    select_all_impl(editor, object::select_all_children);
  };

  cx.editor.apply_motion(motion);
}

pub fn select_all_siblings(cx: &mut Context) {
  let motion = |editor: &mut Editor| {
    select_all_impl(editor, object::select_all_siblings);
  };

  cx.editor.apply_motion(motion);
}

pub fn select_next_sibling(cx: &mut Context) {
  select_sibling_impl(cx, object::select_next_sibling)
}

pub fn select_prev_sibling(cx: &mut Context) {
  select_sibling_impl(cx, object::select_prev_sibling)
}

pub fn move_parent_node_start(cx: &mut Context) {
  move_node_bound_impl(cx, Direction::Backward, Movement::Move)
}

pub fn extend_parent_node_start(cx: &mut Context) {
  move_node_bound_impl(cx, Direction::Backward, Movement::Extend)
}

pub fn goto_first_diag(cx: &mut Context) {
  let (view, doc) = current!(cx.editor);
  let selection = match doc.diagnostics().first() {
    Some(diag) => Selection::single(diag.range.start, diag.range.end),
    None => return,
  };
  doc.set_selection(view.id, selection);
  view
    .diagnostics_handler
    .immediately_show_diagnostic(doc, view.id);
}

pub fn goto_last_diag(cx: &mut Context) {
  let (view, doc) = current!(cx.editor);
  let selection = match doc.diagnostics().last() {
    Some(diag) => Selection::single(diag.range.start, diag.range.end),
    None => return,
  };
  doc.set_selection(view.id, selection);
  view
    .diagnostics_handler
    .immediately_show_diagnostic(doc, view.id);
}

pub fn goto_next_change(cx: &mut Context) {
  goto_next_change_impl(cx, Direction::Forward)
}

pub fn goto_prev_change(cx: &mut Context) {
  goto_next_change_impl(cx, Direction::Backward)
}

fn goto_next_change_impl(cx: &mut Context, direction: Direction) {
  let count = cx.count() as u32 - 1;
  let motion = move |editor: &mut Editor| {
    let (view, doc) = current!(editor);
    let doc_text = doc.text().slice(..);
    let diff_handle = if let Some(diff_handle) = doc.diff_handle() {
      diff_handle
    } else {
      editor.set_status("Diff is not available in current buffer");
      return;
    };

    let selection = doc.selection(view.id).clone().transform(|range| {
      let cursor_line = range.cursor_line(doc_text) as u32;

      let diff = diff_handle.load();
      let hunk_idx = match direction {
        Direction::Forward => {
          diff
            .next_hunk(cursor_line)
            .map(|idx| (idx + count).min(diff.len() - 1))
        },
        Direction::Backward => {
          diff
            .prev_hunk(cursor_line)
            .map(|idx| idx.saturating_sub(count))
        },
      };
      let Some(hunk_idx) = hunk_idx else {
        return range;
      };
      let hunk = diff.nth_hunk(hunk_idx);
      let new_range = hunk_range(hunk, doc_text);
      if editor.mode == Mode::Select {
        let head = if new_range.head < range.anchor {
          new_range.anchor
        } else {
          new_range.head
        };

        Range::new(range.anchor, head)
      } else {
        new_range.with_direction(direction)
      }
    });

    doc.set_selection(view.id, selection)
  };
  cx.editor.apply_motion(motion);
}

/// Returns the [Range] for a [Hunk] in the given text.
/// Additions and modifications cover the added and modified ranges.
/// Deletions are represented as the point at the start of the deletion hunk.
fn hunk_range(hunk: Hunk, text: RopeSlice) -> Range {
  let anchor = text.line_to_char(hunk.after.start as usize);
  let head = if hunk.after.is_empty() {
    anchor + 1
  } else {
    text.line_to_char(hunk.after.end as usize)
  };

  Range::new(anchor, head)
}

pub fn goto_first_change(cx: &mut Context) {
  goto_first_change_impl(cx, false);
}

pub fn goto_last_change(cx: &mut Context) {
  goto_first_change_impl(cx, true);
}

fn goto_first_change_impl(cx: &mut Context, reverse: bool) {
  let editor = &mut cx.editor;
  let (view, doc) = current!(editor);
  if let Some(handle) = doc.diff_handle() {
    let hunk = {
      let diff = handle.load();
      let idx = if reverse {
        diff.len().saturating_sub(1)
      } else {
        0
      };
      diff.nth_hunk(idx)
    };
    if hunk != Hunk::NONE {
      let range = hunk_range(hunk, doc.text().slice(..));
      doc.set_selection(view.id, Selection::single(range.anchor, range.head));
    }
  }
}

fn goto_ts_object_impl(cx: &mut Context, object: &'static str, direction: Direction) {
  let count = cx.count();
  let motion = move |editor: &mut Editor| {
    let (view, doc) = current!(editor);
    let loader = editor.syn_loader.load();
    if let Some(syntax) = doc.syntax() {
      let text = doc.text().slice(..);
      let root = syntax.tree().root_node();

      let selection = doc.selection(view.id).clone().transform(|range| {
        let new_range = movement::goto_treesitter_object(
          text, range, object, direction, &root, syntax, &loader, count,
        );

        if editor.mode == Mode::Select {
          let head = if new_range.head < range.anchor {
            new_range.anchor
          } else {
            new_range.head
          };

          Range::new(range.anchor, head)
        } else {
          new_range.with_direction(direction)
        }
      });

      doc.set_selection(view.id, selection);
    } else {
      editor.set_status("Syntax-tree is not available in current buffer");
    }
  };
  cx.editor.apply_motion(motion);
}

pub fn goto_next_function(cx: &mut Context) {
  goto_ts_object_impl(cx, "function", Direction::Forward)
}

pub fn goto_prev_function(cx: &mut Context) {
  goto_ts_object_impl(cx, "function", Direction::Backward)
}

pub fn goto_next_class(cx: &mut Context) {
  goto_ts_object_impl(cx, "class", Direction::Forward)
}

pub fn goto_prev_class(cx: &mut Context) {
  goto_ts_object_impl(cx, "class", Direction::Backward)
}

pub fn goto_next_parameter(cx: &mut Context) {
  goto_ts_object_impl(cx, "parameter", Direction::Forward)
}

pub fn goto_prev_parameter(cx: &mut Context) {
  goto_ts_object_impl(cx, "parameter", Direction::Backward)
}

pub fn goto_next_comment(cx: &mut Context) {
  goto_ts_object_impl(cx, "comment", Direction::Forward)
}

pub fn goto_prev_comment(cx: &mut Context) {
  goto_ts_object_impl(cx, "comment", Direction::Backward)
}

pub fn goto_next_test(cx: &mut Context) {
  goto_ts_object_impl(cx, "test", Direction::Forward)
}

pub fn goto_prev_test(cx: &mut Context) {
  goto_ts_object_impl(cx, "test", Direction::Backward)
}

pub fn goto_next_xml_element(cx: &mut Context) {
  goto_ts_object_impl(cx, "xml-element", Direction::Forward)
}

pub fn goto_prev_xml_element(cx: &mut Context) {
  goto_ts_object_impl(cx, "xml-element", Direction::Backward)
}

pub fn goto_next_entry(cx: &mut Context) {
  goto_ts_object_impl(cx, "entry", Direction::Forward)
}

pub fn goto_prev_entry(cx: &mut Context) {
  goto_ts_object_impl(cx, "entry", Direction::Backward)
}

pub fn goto_prev_paragraph(cx: &mut Context) {
  goto_para_impl(cx, movement::move_prev_paragraph)
}

pub fn goto_next_paragraph(cx: &mut Context) {
  goto_para_impl(cx, movement::move_next_paragraph)
}

fn goto_para_impl<F>(cx: &mut Context, move_fn: F)
where
  F: Fn(RopeSlice, Range, usize, Movement) -> Range + 'static,
{
  let count = cx.count();
  let motion = move |editor: &mut Editor| {
    let (view, doc) = current!(editor);
    let text = doc.text().slice(..);
    let behavior = if editor.mode == Mode::Select {
      Movement::Extend
    } else {
      Movement::Move
    };

    let selection = doc
      .selection(view.id)
      .clone()
      .transform(|range| move_fn(text, range, count, behavior));
    doc.set_selection(view.id, selection);
  };
  cx.editor.apply_motion(motion)
}

pub fn add_newline_above(cx: &mut Context) {
  add_newline_impl(cx, Open::Above);
}

pub fn add_newline_below(cx: &mut Context) {
  add_newline_impl(cx, Open::Below)
}

fn add_newline_impl(cx: &mut Context, open: Open) {
  let count = cx.count();
  let (view, doc) = current!(cx.editor);
  let selection = doc.selection(view.id);
  let text = doc.text();
  let slice = text.slice(..);

  let changes = selection.into_iter().map(|range| {
    let (start, end) = range.line_range(slice);
    let line = match open {
      Open::Above => start,
      Open::Below => end + 1,
    };
    let pos = text.line_to_char(line);
    (
      pos,
      pos,
      Some(doc.line_ending.as_str().repeat(count).into()),
    )
  });

  let transaction = Transaction::change(text, changes);
  doc.apply(&transaction, view.id);
}

pub fn search_next(cx: &mut Context) {
  search_next_or_prev_impl(cx, Movement::Move, Direction::Forward);
}

pub fn search_prev(cx: &mut Context) {
  search_next_or_prev_impl(cx, Movement::Move, Direction::Backward);
}

pub fn extend_search_next(cx: &mut Context) {
  search_next_or_prev_impl(cx, Movement::Extend, Direction::Forward);
}

pub fn extend_search_prev(cx: &mut Context) {
  search_next_or_prev_impl(cx, Movement::Extend, Direction::Backward);
}

fn search_next_or_prev_impl(cx: &mut Context, movement: Movement, direction: Direction) {
  let count = cx.count();
  let register = cx
    .register
    .unwrap_or(cx.editor.registers.last_search_register);
  let config = cx.editor.config();
  let scrolloff = config.scrolloff;
  if let Some(query) = cx.editor.registers.first(register, cx.editor) {
    let search_config = &config.search;
    let case_insensitive = if search_config.smart_case {
      !query.chars().any(char::is_uppercase)
    } else {
      false
    };
    let wrap_around = search_config.wrap_around;
    if let Ok(regex) = the_editor_stdx::rope::RegexBuilder::new()
      .syntax(
        the_editor_stdx::rope::Config::new()
          .case_insensitive(case_insensitive)
          .multi_line(true),
      )
      .build(&query)
    {
      for _ in 0..count {
        // Get current selection for each iteration to move from current position
        let (view, doc) = current_ref!(cx.editor);
        let current_selection = doc.selection(view.id).clone();

        search_impl(
          cx.editor,
          &regex,
          movement,
          direction,
          scrolloff,
          wrap_around,
          true,
          &current_selection,
        );
      }
    } else {
      let error = format!("Invalid regex: {}", query);
      cx.editor.set_error(error);
    }
  }
}

fn search_completions(cx: &Context, reg: Option<char>) -> Vec<String> {
  let mut items = reg
    .and_then(|reg| cx.editor.registers.read(reg, &cx.editor))
    .map_or(Vec::new(), |reg| reg.take(200).collect());
  items.sort_unstable();
  items.dedup();
  items.into_iter().map(|value| value.to_string()).collect()
}

pub fn search(cx: &mut Context) {
  searcher(cx, Direction::Forward)
}

pub fn rsearch(cx: &mut Context) {
  searcher(cx, Direction::Backward)
}

pub fn search_selection_detect_word_boundaries(cx: &mut Context) {
  search_selection_impl(cx, true)
}

pub fn search_selection(cx: &mut Context) {
  search_selection_impl(cx, false)
}

fn searcher(cx: &mut Context, direction: Direction) {
  let reg = cx.register.unwrap_or('/');
  let config = cx.editor.config();
  let scrolloff = config.scrolloff;
  let wrap_around = config.search.wrap_around;
  let movement = if cx.editor.mode() == Mode::Select {
    Movement::Extend
  } else {
    Movement::Move
  };

  // Set custom mode string
  cx.editor.set_custom_mode_str("SEARCH".to_string());

  // Set mode to Command so prompt is shown
  cx.editor.set_mode(Mode::Command);

  // Capture the original cursor position before starting the prompt
  let (view, doc) = current_ref!(cx.editor);
  let original_selection = doc.selection(view.id).clone();

  // Create prompt with callback
  let prompt =
    crate::ui::components::Prompt::new(String::new()).with_callback(move |cx, input, event| {
      use crate::ui::components::prompt::PromptEvent;

      // Handle events
      match event {
        PromptEvent::Update | PromptEvent::Validate => {
          // Skip empty input
          if input.is_empty() {
            return;
          }

          // Parse regex
          let regex = match the_editor_stdx::rope::Regex::new(input) {
            Ok(regex) => regex,
            Err(err) => {
              cx.editor.set_error(format!("Invalid regex: {}", err));
              return;
            },
          };

          // Store the search query in the register on validation
          if matches!(event, PromptEvent::Validate) {
            if let Err(err) = cx.editor.registers.push(reg, input.to_string()) {
              cx.editor.set_error(err.to_string());
              return;
            }
            cx.editor.registers.last_search_register = reg;
            // Clear custom mode string on validation
            cx.editor.clear_custom_mode_str();
          }

          search_impl(
            &mut cx.editor,
            &regex,
            movement,
            direction,
            scrolloff,
            wrap_around,
            matches!(event, PromptEvent::Validate),
            &original_selection,
          );
        },
        PromptEvent::Abort => {
          // Clear custom mode string on abort
          cx.editor.clear_custom_mode_str();
        },
      }
    });

  cx.callback.push(Box::new(|compositor, _cx| {
    // Find the statusline and trigger slide animation
    for layer in compositor.layers.iter_mut() {
      if let Some(statusline) = layer
        .as_any_mut()
        .downcast_mut::<crate::ui::components::statusline::StatusLine>()
      {
        statusline.slide_for_prompt(true);
        break;
      }
    }

    compositor.push(Box::new(prompt));
  }));
}

fn search_selection_impl(cx: &mut Context, detect_word_boundaries: bool) {
  fn is_at_word_start(text: RopeSlice, index: usize) -> bool {
    // This can happen when the cursor is at the last character in
    // the document +1 (ge + j), in this case text.char(index) will panic as
    // it will index out of bounds. See https://github.com/helix-editor/helix/issues/12609
    if index == text.len_chars() {
      return false;
    }
    let ch = text.char(index);
    if index == 0 {
      return char_is_word(ch);
    }
    let prev_ch = text.char(index - 1);

    !char_is_word(prev_ch) && char_is_word(ch)
  }

  fn is_at_word_end(text: RopeSlice, index: usize) -> bool {
    if index == 0 || index == text.len_chars() {
      return false;
    }
    let ch = text.char(index);
    let prev_ch = text.char(index - 1);

    char_is_word(prev_ch) && !char_is_word(ch)
  }

  let register = cx.register.unwrap_or('/');
  let (view, doc) = current!(cx.editor);
  let text = doc.text().slice(..);

  let regex = doc
        .selection(view.id)
        .iter()
        .map(|selection| {
            let add_boundary_prefix =
                detect_word_boundaries && is_at_word_start(text, selection.from());
            let add_boundary_suffix =
                detect_word_boundaries && is_at_word_end(text, selection.to());

            let prefix = if add_boundary_prefix { "\\b" } else { "" };
            let suffix = if add_boundary_suffix { "\\b" } else { "" };

            let word = regex::escape(&selection.fragment(text));
            format!("{}{}{}", prefix, word, suffix)
        })
        .collect::<HashSet<_>>() // Collect into hashset to deduplicate identical regexes
        .into_iter()
        .collect::<Vec<_>>()
        .join("|");

  let msg = format!("register '{}' set to '{}'", register, &regex);
  match cx.editor.registers.push(register, regex) {
    Ok(_) => {
      cx.editor.registers.last_search_register = register;
      cx.editor.set_status(msg)
    },
    Err(err) => cx.editor.set_error(err.to_string()),
  }
}

fn search_impl(
  editor: &mut Editor,
  regex: &the_editor_stdx::rope::Regex,
  movement: Movement,
  direction: Direction,
  scrolloff: usize,
  wrap_around: bool,
  show_warnings: bool,
  original_selection: &Selection,
) {
  let (_view, doc) = current!(editor);
  let text = doc.text().slice(..);

  // Use the original selection to determine start position, not the current
  // cursor
  let start = match direction {
    Direction::Forward => {
      text.char_to_byte(grapheme::ensure_grapheme_boundary_next(
        text,
        original_selection.primary().to(),
      ))
    },
    Direction::Backward => {
      text.char_to_byte(grapheme::ensure_grapheme_boundary_prev(
        text,
        original_selection.primary().from(),
      ))
    },
  };

  // A regex::Match returns byte-positions in the str. In the case where we
  // do a reverse search and wraparound to the end, we don't need to search
  // the text before the current cursor position for matches, but by slicing
  // it out, we need to add it back to the position of the selection.
  let doc = doc!(editor).text().slice(..);

  // use find_at to find the next match after the cursor, loop around the end
  // Careful, `Regex` uses `bytes` as offsets, not character indices!
  let mut mat = match direction {
    Direction::Forward => regex.find(doc.regex_input_at_bytes(start..)),
    Direction::Backward => regex.find_iter(doc.regex_input_at_bytes(..start)).last(),
  };

  if mat.is_none() {
    if wrap_around {
      mat = match direction {
        Direction::Forward => regex.find(doc.regex_input()),
        Direction::Backward => regex.find_iter(doc.regex_input_at_bytes(start..)).last(),
      };
    }
    if show_warnings {
      if wrap_around && mat.is_some() {
        editor.set_status("Wrapped around document");
      } else {
        editor.set_error("No more matches");
      }
    }
  }

  let (view, doc) = current!(editor);
  let text = doc.text().slice(..);

  if let Some(mat) = mat {
    let start = text.byte_to_char(mat.start());
    let end = text.byte_to_char(mat.end());

    if end == 0 {
      // skip empty matches that don't make sense
      return;
    }

    // Determine range direction based on the original primary range
    let primary = original_selection.primary();
    let range = Range::new(start, end).with_direction(primary.direction());

    let selection = match movement {
      Movement::Extend => original_selection.clone().push(range),
      Movement::Move => {
        original_selection
          .clone()
          .replace(original_selection.primary_index(), range)
      },
    };

    doc.set_selection(view.id, selection);
    view.ensure_cursor_in_view_center(doc, scrolloff);
  };
}

pub fn join_selections(cx: &mut Context) {
  join_selections_impl(cx, false)
}

pub fn join_selections_space(cx: &mut Context) {
  join_selections_impl(cx, true)
}

fn join_selections_impl(cx: &mut Context, select_space: bool) {
  let (view, doc) = current!(cx.editor);
  let text = doc.text();
  let slice = text.slice(..);

  let comment_tokens = doc
    .language_config()
    .and_then(|config| config.comment_tokens.as_deref())
    .unwrap_or(&[]);
  // Sort by length to handle Rust's /// vs //
  let mut comment_tokens: Vec<&str> = comment_tokens.iter().map(|x| x.as_str()).collect();
  comment_tokens.sort_unstable_by_key(|x| std::cmp::Reverse(x.len()));

  let mut changes = Vec::new();

  for selection in doc.selection(view.id) {
    let (start, mut end) = selection.line_range(slice);
    if start == end {
      end = (end + 1).min(text.len_lines() - 1);
    }
    let lines = start..end;

    changes.reserve(lines.len());

    let first_line_idx = slice.line_to_char(start);
    let first_line_idx = movement::skip_while(slice, first_line_idx, |ch| matches!(ch, ' ' | '\t'))
      .unwrap_or(first_line_idx);
    let first_line = slice.slice(first_line_idx..);
    let mut current_comment_token = comment_tokens
      .iter()
      .find(|token| first_line.starts_with(token));

    for line in lines {
      let start = line_end_char_index(&slice, line);
      let mut end = text.line_to_char(line + 1);
      end = movement::skip_while(slice, end, |ch| matches!(ch, ' ' | '\t')).unwrap_or(end);
      let slice_from_end = slice.slice(end..);
      if let Some(token) = comment_tokens
        .iter()
        .find(|token| slice_from_end.starts_with(token))
      {
        if Some(token) == current_comment_token {
          end += token.chars().count();
          end = movement::skip_while(slice, end, |ch| matches!(ch, ' ' | '\t')).unwrap_or(end);
        } else {
          // update current token, but don't delete this one.
          current_comment_token = Some(token);
        }
      }

      let separator = if end == line_end_char_index(&slice, line + 1) {
        // the joining line contains only space-characters => don't include a whitespace
        // when joining
        None
      } else {
        Some(Tendril::from(" "))
      };
      changes.push((start, end, separator));
    }
  }

  // nothing to do, bail out early to avoid crashes later
  if changes.is_empty() {
    return;
  }

  changes.sort_unstable_by_key(|(from, _to, _text)| *from);
  changes.dedup();

  // select inserted spaces
  let transaction = if select_space {
    let mut offset: usize = 0;
    let ranges: SmallVec<_> = changes
      .iter()
      .filter_map(|change| {
        if change.2.is_some() {
          let range = Range::point(change.0 - offset);
          offset += change.1 - change.0 - 1; // -1 adjusts for the replacement of the range by a space
          Some(range)
        } else {
          offset += change.1 - change.0;
          None
        }
      })
      .collect();
    let t = Transaction::change(text, changes.into_iter());
    if ranges.is_empty() {
      t
    } else {
      let selection = Selection::new(ranges, 0);
      t.with_selection(selection)
    }
  } else {
    Transaction::change(text, changes.into_iter())
  };

  doc.apply(&transaction, view.id);
}

pub fn keep_selections(cx: &mut Context) {
  keep_or_remove_selections_impl(cx, false)
}

pub fn remove_selections(cx: &mut Context) {
  keep_or_remove_selections_impl(cx, true)
}

fn keep_or_remove_selections_impl(cx: &mut Context, remove: bool) {
  // keep or remove selections matching regex

  // Set custom mode string
  cx.editor.set_custom_mode_str(if remove {
    "REMOVE".to_string()
  } else {
    "KEEP".to_string()
  });

  // Set mode to Command so prompt is shown
  cx.editor.set_mode(Mode::Command);

  // Capture the original selection before starting the prompt
  let (view, doc) = current_ref!(cx.editor);
  let original_selection = doc.selection(view.id).clone();

  // Create prompt with callback
  let prompt =
    crate::ui::components::Prompt::new(String::new()).with_callback(move |cx, input, event| {
      use crate::ui::components::prompt::PromptEvent;

      // Handle events
      match event {
        PromptEvent::Update | PromptEvent::Validate => {
          if matches!(event, PromptEvent::Validate) {
            // Clear custom mode string on validation
            cx.editor.clear_custom_mode_str();
          }

          // Skip empty input
          if input.is_empty() {
            return;
          }

          // Parse regex
          let regex = match the_editor_stdx::rope::Regex::new(input) {
            Ok(regex) => regex,
            Err(err) => {
              cx.editor.set_error(format!("Invalid regex: {}", err));
              return;
            },
          };

          let (view, doc) = current!(cx.editor);
          let text = doc.text().slice(..);

          // Apply keep_or_remove_matches on the original selection
          if let Some(selection) = crate::core::selection::keep_or_remove_matches(
            text,
            &original_selection,
            &regex,
            remove,
          ) {
            doc.set_selection(view.id, selection);
          } else if matches!(event, PromptEvent::Validate) {
            cx.editor.set_error("No selections remaining");
          }
        },
        PromptEvent::Abort => {
          // Clear custom mode string on abort
          cx.editor.clear_custom_mode_str();
        },
      }
    });

  cx.callback.push(Box::new(|compositor, _cx| {
    // Find the statusline and trigger slide animation
    for layer in compositor.layers.iter_mut() {
      if let Some(statusline) = layer
        .as_any_mut()
        .downcast_mut::<crate::ui::components::statusline::StatusLine>()
      {
        statusline.slide_for_prompt(true);
        break;
      }
    }

    compositor.push(Box::new(prompt));
  }));
}

#[allow(deprecated)]
pub fn align_selections(cx: &mut Context) {
  use crate::core::position::visual_coords_at_pos;

  let (view, doc) = current!(cx.editor);
  let text = doc.text().slice(..);
  let selection = doc.selection(view.id);

  let tab_width = doc.tab_width();
  let mut column_widths: Vec<Vec<_>> = Vec::new();
  let mut last_line = text.len_lines() + 1;
  let mut col = 0;

  for range in selection {
    let coords = visual_coords_at_pos(text, range.head, tab_width);
    let anchor_coords = visual_coords_at_pos(text, range.anchor, tab_width);

    if coords.row != anchor_coords.row {
      cx.editor
        .set_error("align cannot work with multi line selections");
      return;
    }

    col = if coords.row == last_line { col + 1 } else { 0 };

    if col >= column_widths.len() {
      column_widths.push(Vec::new());
    }
    column_widths[col].push((range.from(), coords.col));

    last_line = coords.row;
  }

  let mut changes = Vec::with_capacity(selection.len());

  // Account for changes on each row
  let len = column_widths.first().map(|cols| cols.len()).unwrap_or(0);
  let mut offs = vec![0; len];

  for col in column_widths {
    let max_col = col
      .iter()
      .enumerate()
      .map(|(row, (_, cursor))| *cursor + offs[row])
      .max()
      .unwrap_or(0);

    for (row, (insert_pos, last_col)) in col.into_iter().enumerate() {
      let ins_count = max_col - (last_col + offs[row]);

      if ins_count == 0 {
        continue;
      }

      offs[row] += ins_count;

      changes.push((insert_pos, insert_pos, Some(" ".repeat(ins_count).into())));
    }
  }

  // The changeset has to be sorted
  changes.sort_unstable_by_key(|(from, ..)| *from);

  let transaction = Transaction::change(doc.text(), changes.into_iter());
  doc.apply(&transaction, view.id);
  exit_select_mode(cx);
}

pub fn trim_selections(cx: &mut Context) {
  let (view, doc) = current!(cx.editor);
  let text = doc.text().slice(..);

  let ranges: SmallVec<[Range; 1]> = doc
    .selection(view.id)
    .iter()
    .filter_map(|range| {
      if range.is_empty() || range.slice(text).chars().all(|ch| ch.is_whitespace()) {
        return None;
      }
      let mut start = range.from();
      let mut end = range.to();
      start = movement::skip_while(text, start, |x| x.is_whitespace()).unwrap_or(start);
      end = movement::backwards_skip_while(text, end, |x| x.is_whitespace()).unwrap_or(end);
      Some(Range::new(start, end).with_direction(range.direction()))
    })
    .collect();

  if !ranges.is_empty() {
    let primary = doc.selection(view.id).primary();
    let idx = ranges
      .iter()
      .position(|range| range.overlaps(&primary))
      .unwrap_or(ranges.len() - 1);
    doc.set_selection(view.id, Selection::new(ranges, idx));
  } else {
    collapse_selection(cx);
    keep_primary_selection(cx);
  };
}

fn rotate_selections(cx: &mut Context, direction: Direction) {
  let count = cx.count();
  let (view, doc) = current!(cx.editor);
  let mut selection = doc.selection(view.id).clone();
  let index = selection.primary_index();
  let len = selection.len();
  selection.set_primary_index(match direction {
    Direction::Forward => (index + count) % len,
    Direction::Backward => (index + (len.saturating_sub(count) % len)) % len,
  });
  doc.set_selection(view.id, selection);
}

pub fn rotate_selections_forward(cx: &mut Context) {
  rotate_selections(cx, Direction::Forward)
}

pub fn rotate_selections_backward(cx: &mut Context) {
  rotate_selections(cx, Direction::Backward)
}

#[derive(Debug)]
enum ReorderStrategy {
  RotateForward,
  RotateBackward,
  Reverse,
}

pub fn rotate_selection_contents_forward(cx: &mut Context) {
  reorder_selection_contents(cx, ReorderStrategy::RotateForward)
}

pub fn rotate_selection_contents_backward(cx: &mut Context) {
  reorder_selection_contents(cx, ReorderStrategy::RotateBackward)
}

fn reorder_selection_contents(cx: &mut Context, strategy: ReorderStrategy) {
  let count = cx.count;
  let (view, doc) = current!(cx.editor);
  let text = doc.text().slice(..);

  let selection = doc.selection(view.id);

  let mut ranges: Vec<_> = selection
    .slices(text)
    .map(|fragment| fragment.chunks().collect())
    .collect();

  let rotate_by = count.map_or(1, |count| count.get().min(ranges.len()));

  let primary_index = match strategy {
    ReorderStrategy::RotateForward => {
      ranges.rotate_right(rotate_by);
      // Like `usize::wrapping_add`, but provide a custom range from `0` to
      // `ranges.len()`
      (selection.primary_index() + ranges.len() + rotate_by) % ranges.len()
    },
    ReorderStrategy::RotateBackward => {
      ranges.rotate_left(rotate_by);
      // Like `usize::wrapping_sub`, but provide a custom range from `0` to
      // `ranges.len()`
      (selection.primary_index() + ranges.len() - rotate_by) % ranges.len()
    },
    ReorderStrategy::Reverse => {
      if rotate_by % 2 == 0 {
        // nothing changed, if we reverse something an even
        // amount of times, the output will be the same
        return;
      }
      ranges.reverse();
      // -1 to turn 1-based len into 0-based index
      (ranges.len() - 1) - selection.primary_index()
    },
  };

  let transaction = Transaction::change(
    doc.text(),
    selection
      .ranges()
      .iter()
      .zip(ranges)
      .map(|(range, fragment)| (range.from(), range.to(), Some(fragment))),
  );

  doc.set_selection(
    view.id,
    Selection::new(selection.ranges().into(), primary_index),
  );
  doc.apply(&transaction, view.id);
}

pub fn page_up(cx: &mut Context) {
  let view = view!(cx.editor);
  let offset = view.inner_height();
  scroll(cx, offset, Direction::Backward, false);
}

pub fn page_down(cx: &mut Context) {
  let view = view!(cx.editor);
  let offset = view.inner_height();
  scroll(cx, offset, Direction::Forward, false);
}

pub fn half_page_up(cx: &mut Context) {
  let view = view!(cx.editor);
  let offset = view.inner_height() / 2;
  scroll(cx, offset, Direction::Backward, false);
}

pub fn half_page_down(cx: &mut Context) {
  let view = view!(cx.editor);
  let offset = view.inner_height() / 2;
  scroll(cx, offset, Direction::Forward, false);
}

pub fn page_cursor_up(cx: &mut Context) {
  let view = view!(cx.editor);
  let offset = view.inner_height();
  scroll(cx, offset, Direction::Backward, true);
}

pub fn page_cursor_down(cx: &mut Context) {
  let view = view!(cx.editor);
  let offset = view.inner_height();
  scroll(cx, offset, Direction::Forward, true);
}

pub fn page_cursor_half_up(cx: &mut Context) {
  let view = view!(cx.editor);
  let offset = view.inner_height() / 2;
  scroll(cx, offset, Direction::Backward, true);
}

pub fn page_cursor_half_down(cx: &mut Context) {
  let view = view!(cx.editor);
  let offset = view.inner_height() / 2;
  scroll(cx, offset, Direction::Forward, true);
}

pub fn jump_view_right(cx: &mut Context) {
  cx.editor.focus_direction(tree::Direction::Right)
}

pub fn jump_view_left(cx: &mut Context) {
  cx.editor.focus_direction(tree::Direction::Left)
}

pub fn jump_view_up(cx: &mut Context) {
  cx.editor.focus_direction(tree::Direction::Up)
}

pub fn jump_view_down(cx: &mut Context) {
  cx.editor.focus_direction(tree::Direction::Down)
}

pub fn hsplit(cx: &mut Context) {
  split(cx.editor, Action::HorizontalSplit);
}

pub fn hsplit_new(cx: &mut Context) {
  cx.editor.new_file(Action::HorizontalSplit);
}

pub fn vsplit(cx: &mut Context) {
  split(cx.editor, Action::VerticalSplit);
}

pub fn vsplit_new(cx: &mut Context) {
  cx.editor.new_file(Action::VerticalSplit);
}

fn split(editor: &mut Editor, action: Action) {
  let (view, doc) = current!(editor);
  let id = doc.id();
  let selection = doc.selection(view.id).clone();
  let offset = doc.view_offset(view.id);

  editor.switch(id, action);

  // match the selection in the previous view
  let (view, doc) = current!(editor);
  doc.set_selection(view.id, selection);
  // match the view scroll offset (switch doesn't handle this fully
  // since the selection is only matched after the split)
  doc.set_view_offset(view.id, offset);
}

pub fn rotate_view(cx: &mut Context) {
  cx.editor.focus_next()
}

pub fn transpose_view(cx: &mut Context) {
  cx.editor.transpose_view()
}

pub fn goto_file_hsplit(cx: &mut Context) {
  goto_file_impl(cx, Action::HorizontalSplit);
}

pub fn goto_file_vsplit(cx: &mut Context) {
  goto_file_impl(cx, Action::VerticalSplit);
}

pub fn wclose(cx: &mut Context) {
  if cx.editor.tree.views().count() == 1 {
    if let Err(err) = crate::core::command_registry::buffers_remaining_impl(cx.editor) {
      cx.editor.set_error(err.to_string());
      return;
    }
  }
  let view_id = view!(cx.editor).id;
  // close current split
  cx.editor.close(view_id);
}

pub fn wonly(cx: &mut Context) {
  let views = cx
    .editor
    .tree
    .views()
    .map(|(v, focus)| (v.id, focus))
    .collect::<Vec<_>>();
  for (view_id, focus) in views {
    if !focus {
      cx.editor.close(view_id);
    }
  }
}

pub fn swap_view_right(cx: &mut Context) {
  cx.editor.swap_split_in_direction(tree::Direction::Right)
}

pub fn swap_view_left(cx: &mut Context) {
  cx.editor.swap_split_in_direction(tree::Direction::Left)
}

pub fn swap_view_up(cx: &mut Context) {
  cx.editor.swap_split_in_direction(tree::Direction::Up)
}

pub fn swap_view_down(cx: &mut Context) {
  cx.editor.swap_split_in_direction(tree::Direction::Down)
}

type CommentTransactionFn = fn(
  line_token: Option<&str>,
  block_tokens: Option<&[BlockCommentToken]>,
  doc: &Rope,
  selection: &Selection,
) -> Transaction;

fn toggle_comments_impl(cx: &mut Context, comment_transaction: CommentTransactionFn) {
  let (view, doc) = current!(cx.editor);
  let line_token: Option<&str> = doc
    .language_config()
    .and_then(|lc| lc.comment_tokens.as_ref())
    .and_then(|tc| tc.first())
    .map(|tc| tc.as_str());
  let block_tokens: Option<&[BlockCommentToken]> = doc
    .language_config()
    .and_then(|lc| lc.block_comment_tokens.as_ref())
    .map(|tc| &tc[..]);

  let transaction =
    comment_transaction(line_token, block_tokens, doc.text(), doc.selection(view.id));

  doc.apply(&transaction, view.id);
  exit_select_mode(cx);
}

/// commenting behavior:
/// 1. only line comment tokens -> line comment
/// 2. each line block commented -> uncomment all lines
/// 3. whole selection block commented -> uncomment selection
/// 4. all lines not commented and block tokens -> comment uncommented lines
/// 5. no comment tokens and not block commented -> line comment
pub fn toggle_comments(cx: &mut Context) {
  toggle_comments_impl(cx, |line_token, block_tokens, doc, selection| {
    let text = doc.slice(..);

    // only have line comment tokens
    if line_token.is_some() && block_tokens.is_none() {
      return comment::toggle_line_comments(doc, selection, line_token);
    }

    let split_lines = comment::split_lines_of_selection(text, selection);

    let default_block_tokens = &[BlockCommentToken::default()];
    let block_comment_tokens = block_tokens.unwrap_or(default_block_tokens);

    let (line_commented, line_comment_changes) =
      comment::find_block_comments(block_comment_tokens, text, &split_lines);

    // block commented by line would also be block commented so check this first
    if line_commented {
      return comment::create_block_comment_transaction(
        doc,
        &split_lines,
        line_commented,
        line_comment_changes,
      )
      .0;
    }

    let (block_commented, comment_changes) =
      comment::find_block_comments(block_comment_tokens, text, selection);

    // check if selection has block comments
    if block_commented {
      return comment::create_block_comment_transaction(
        doc,
        selection,
        block_commented,
        comment_changes,
      )
      .0;
    }

    // not commented and only have block comment tokens
    if line_token.is_none() && block_tokens.is_some() {
      return comment::create_block_comment_transaction(
        doc,
        &split_lines,
        line_commented,
        line_comment_changes,
      )
      .0;
    }

    // not block commented at all and don't have any tokens
    comment::toggle_line_comments(doc, selection, line_token)
  })
}

pub fn jump_forward(cx: &mut Context) {
  let count = cx.count();
  let config = cx.editor.config();
  let view = view_mut!(cx.editor);
  let doc_id = view.doc;

  if let Some((id, selection)) = view.jumps.forward(count) {
    view.doc = *id;
    let selection = selection.clone();
    let (view, doc) = current!(cx.editor); // refetch doc

    if doc.id() != doc_id {
      view.add_to_history(doc_id);
    }

    doc.set_selection(view.id, selection);
    // Document we switch to might not have been opened in the view before
    doc.ensure_view_init(view.id);
    view.ensure_cursor_in_view_center(doc, config.scrolloff);
  };
}

pub fn jump_backward(cx: &mut Context) {
  let count = cx.count();
  let config = cx.editor.config();
  let (view, doc) = current!(cx.editor);
  let doc_id = doc.id();

  if let Some((id, selection)) = view.jumps.backward(view.id, doc, count) {
    view.doc = *id;
    let selection = selection.clone();
    let (view, doc) = current!(cx.editor); // refetch doc

    if doc.id() != doc_id {
      view.add_to_history(doc_id);
    }

    doc.set_selection(view.id, selection);
    // Document we switch to might not have been opened in the view before
    doc.ensure_view_init(view.id);
    view.ensure_cursor_in_view_center(doc, config.scrolloff);
  };
}

pub fn save_selection(cx: &mut Context) {
  let (view, doc) = current!(cx.editor);
  push_jump(view, doc);
  cx.editor.set_status("Selection saved to jumplist");
}

pub fn select_register(cx: &mut Context) {
  cx.editor.autoinfo = Some(Info::from_registers(
    "Select register",
    &cx.editor.registers,
  ));
  cx.on_next_key(move |cx, event| {
    cx.editor.autoinfo = None;
    if let Key::Char(ch) = event.code {
      cx.editor.selected_register = Some(ch);
    }
  })
}

#[derive(Eq, PartialEq)]
enum ShellBehavior {
  Replace,
  Ignore,
  Insert,
  Append,
}

pub fn shell_pipe(cx: &mut Context) {
  shell_prompt(cx, "PIPE", ShellBehavior::Replace);
}

pub fn shell_pipe_to(cx: &mut Context) {
  shell_prompt(cx, "PIPE TO", ShellBehavior::Ignore);
}

pub fn shell_insert_output(cx: &mut Context) {
  shell_prompt(cx, "INSERT OUTPUT", ShellBehavior::Insert);
}

pub fn shell_append_output(cx: &mut Context) {
  shell_prompt(cx, "APPEND OUTPUT", ShellBehavior::Append);
}

pub fn shell_keep_pipe(cx: &mut Context) {
  use crate::ui::components::prompt::PromptEvent;

  // Set custom mode string
  cx.editor.set_custom_mode_str("KEEP PIPE".to_string());

  // Set mode to Command so prompt is shown
  cx.editor.set_mode(crate::keymap::Mode::Command);

  // Create prompt with callback
  let prompt =
    crate::ui::components::Prompt::new(String::new()).with_callback(move |cx, input, event| {
      match event {
        PromptEvent::Validate => {
          // Clear custom mode string on validation
          cx.editor.clear_custom_mode_str();

          if input.is_empty() {
            return;
          }

          let config = cx.editor.config();
          let shell = config.shell.clone();
          let (view, doc) = current!(cx.editor);
          let selection = doc.selection(view.id);
          let text = doc.text().slice(..);

          let mut ranges = SmallVec::with_capacity(selection.len());
          let old_index = selection.primary_index();
          let mut index: Option<usize> = None;

          for (i, range) in selection.ranges().iter().enumerate() {
            let fragment = range.slice(text);
            if shell_impl(&shell, input, Some(fragment.into())).is_ok() {
              ranges.push(*range);
              if i >= old_index && index.is_none() {
                index = Some(ranges.len() - 1);
              }
            }
          }

          if ranges.is_empty() {
            cx.editor.set_error("No selections remaining".to_string());
            return;
          }

          let index = index.unwrap_or_else(|| ranges.len() - 1);
          doc.set_selection(view.id, Selection::new(ranges, index));
        },
        PromptEvent::Abort => {
          // Clear custom mode string on abort
          cx.editor.clear_custom_mode_str();
        },
        PromptEvent::Update => {},
      }
    });

  cx.callback.push(Box::new(|compositor, _cx| {
    // Find the statusline and trigger slide animation
    for layer in compositor.layers.iter_mut() {
      if let Some(statusline) = layer
        .as_any_mut()
        .downcast_mut::<crate::ui::components::statusline::StatusLine>()
      {
        statusline.slide_for_prompt(true);
        break;
      }
    }

    // Push the prompt
    compositor.push(Box::new(prompt));
  }));
}

fn shell_prompt(cx: &mut Context, mode_str: &str, behavior: ShellBehavior) {
  use crate::ui::components::prompt::PromptEvent;

  // Set custom mode string
  cx.editor.set_custom_mode_str(mode_str.to_string());

  // Set mode to Command so prompt is shown
  cx.editor.set_mode(crate::keymap::Mode::Command);

  // Create prompt with callback
  let prompt =
    crate::ui::components::Prompt::new(String::new()).with_callback(move |cx, input, event| {
      match event {
        PromptEvent::Validate => {
          // Clear custom mode string on validation
          cx.editor.clear_custom_mode_str();

          if input.is_empty() {
            return;
          }
          shell(cx, input, &behavior);
        },
        PromptEvent::Abort => {
          // Clear custom mode string on abort
          cx.editor.clear_custom_mode_str();
        },
        PromptEvent::Update => {},
      }
    });

  cx.callback.push(Box::new(|compositor, _cx| {
    // Find the statusline and trigger slide animation
    for layer in compositor.layers.iter_mut() {
      if let Some(statusline) = layer
        .as_any_mut()
        .downcast_mut::<crate::ui::components::statusline::StatusLine>()
      {
        statusline.slide_for_prompt(true);
        break;
      }
    }

    // Push the prompt
    compositor.push(Box::new(prompt));
  }));
}

fn shell(cx: &mut compositor::Context, cmd: &str, behavior: &ShellBehavior) {
  let pipe = match behavior {
    ShellBehavior::Replace | ShellBehavior::Ignore => true,
    ShellBehavior::Insert | ShellBehavior::Append => false,
  };

  let config = cx.editor.config();
  let shell = &config.shell;
  let (view, doc) = current!(cx.editor);
  let selection = doc.selection(view.id);

  let mut changes = Vec::with_capacity(selection.len());
  let mut ranges = SmallVec::with_capacity(selection.len());
  let text = doc.text().slice(..);

  let mut shell_output: Option<Tendril> = None;
  let mut offset = 0isize;
  for range in selection.ranges() {
    let output = if let Some(output) = shell_output.as_ref() {
      output.clone()
    } else {
      let input = range.slice(text);
      match shell_impl(shell, cmd, pipe.then(|| input.into())) {
        Ok(mut output) => {
          if !input.ends_with("\n") && output.ends_with('\n') {
            output.pop();
            if output.ends_with('\r') {
              output.pop();
            }
          }

          if !pipe {
            shell_output = Some(output.clone());
          }
          output
        },
        Err(err) => {
          cx.editor.set_error(err.to_string());
          return;
        },
      }
    };

    let output_len = output.chars().count();

    let (from, to, deleted_len) = match behavior {
      ShellBehavior::Replace => (range.from(), range.to(), range.len()),
      ShellBehavior::Insert => (range.from(), range.from(), 0),
      ShellBehavior::Append => (range.to(), range.to(), 0),
      _ => (range.from(), range.from(), 0),
    };

    // These `usize`s cannot underflow because selection ranges cannot overlap.
    let anchor = to
      .checked_add_signed(offset)
      .expect("Selection ranges cannot overlap")
      .checked_sub(deleted_len)
      .expect("Selection ranges cannot overlap");
    let new_range = Range::new(anchor, anchor + output_len).with_direction(range.direction());
    ranges.push(new_range);
    offset = offset
      .checked_add_unsigned(output_len)
      .expect("Selection ranges cannot overlap")
      .checked_sub_unsigned(deleted_len)
      .expect("Selection ranges cannot overlap");

    changes.push((from, to, Some(output)));
  }

  if behavior != &ShellBehavior::Ignore {
    let transaction = Transaction::change(doc.text(), changes.into_iter())
      .with_selection(Selection::new(ranges, selection.primary_index()));
    doc.apply(&transaction, view.id);
    doc.append_changes_to_history(view);
  }

  // after replace cursor may be out of bounds, do this to
  // make sure cursor is in view and update scroll as well
  view.ensure_cursor_in_view(doc, config.scrolloff);
}

fn shell_impl(shell: &[String], cmd: &str, input: Option<Rope>) -> anyhow::Result<Tendril> {
  tokio::task::block_in_place(|| {
    tokio::runtime::Handle::current().block_on(shell_impl_async(shell, cmd, input))
  })
}

async fn shell_impl_async(
  shell: &[String],
  cmd: &str,
  input: Option<Rope>,
) -> anyhow::Result<Tendril> {
  use std::process::Stdio;

  use tokio::process::Command;
  ensure!(!shell.is_empty(), "No shell set");

  let mut process = Command::new(&shell[0]);
  process
    .args(&shell[1..])
    .arg(cmd)
    .stdout(Stdio::piped())
    .stderr(Stdio::piped());

  if input.is_some() || cfg!(windows) {
    process.stdin(Stdio::piped());
  } else {
    process.stdin(Stdio::null());
  }

  let mut process = match process.spawn() {
    Ok(process) => process,
    Err(e) => {
      log::error!("Failed to start shell: {}", e);
      return Err(e.into());
    },
  };
  let output = if let Some(mut stdin) = process.stdin.take() {
    let input_task = tokio::spawn(async move {
      if let Some(input) = input {
        // Write rope chunks to stdin
        use tokio::io::AsyncWriteExt;
        for chunk in input.chunks() {
          stdin.write_all(chunk.as_bytes()).await?;
        }
      }
      anyhow::Ok(())
    });
    let (output, _) = tokio::join! {
        process.wait_with_output(),
        input_task,
    };
    output?
  } else {
    // Process has no stdin, so we just take the output
    process.wait_with_output().await?
  };

  let output = if !output.status.success() {
    if output.stderr.is_empty() {
      match output.status.code() {
        Some(exit_code) => bail!("Shell command failed: status {}", exit_code),
        None => bail!("Shell command failed"),
      }
    }
    String::from_utf8_lossy(&output.stderr)
    // Prioritize `stderr` output over `stdout`
  } else if !output.stderr.is_empty() {
    let stderr = String::from_utf8_lossy(&output.stderr);
    log::debug!("Command printed to stderr: {stderr}");
    stderr
  } else {
    String::from_utf8_lossy(&output.stdout)
  };

  Ok(Tendril::from(output))
}

enum IncrementDirection {
  Increase,
  Decrease,
}

/// Increment objects within selections by count.
pub fn increment(cx: &mut Context) {
  increment_impl(cx, IncrementDirection::Increase);
}

/// Decrement objects within selections by count.
pub fn decrement(cx: &mut Context) {
  increment_impl(cx, IncrementDirection::Decrease);
}

/// Increment objects within selections by `amount`.
/// A negative `amount` will decrement objects within selections.
fn increment_impl(cx: &mut Context, increment_direction: IncrementDirection) {
  let sign = match increment_direction {
    IncrementDirection::Increase => 1,
    IncrementDirection::Decrease => -1,
  };
  let mut amount = sign * cx.count() as i64;
  // If the register is `#` then increase or decrease the `amount` by 1 per
  // element
  let increase_by = if cx.register == Some('#') { sign } else { 0 };

  let (view, doc) = current!(cx.editor);
  let selection = doc.selection(view.id);
  let text = doc.text().slice(..);

  let mut new_selection_ranges = SmallVec::new();
  let mut cumulative_length_diff: i128 = 0;
  let mut changes = vec![];

  for range in selection {
    let selected_text: Cow<str> = range.fragment(text);
    let new_from = ((range.from() as i128) + cumulative_length_diff) as usize;
    let incremented = [crate::increment::integer, crate::increment::date_time]
      .iter()
      .find_map(|incrementor| incrementor(selected_text.as_ref(), amount));

    amount += increase_by;

    match incremented {
      None => {
        let new_range = Range::new(
          new_from,
          (range.to() as i128 + cumulative_length_diff) as usize,
        );
        new_selection_ranges.push(new_range);
      },
      Some(new_text) => {
        let new_range = Range::new(new_from, new_from + new_text.len());
        cumulative_length_diff += new_text.len() as i128 - selected_text.len() as i128;
        new_selection_ranges.push(new_range);
        changes.push((range.from(), range.to(), Some(new_text.into())));
      },
    }
  }

  if !changes.is_empty() {
    let new_selection = Selection::new(new_selection_ranges, selection.primary_index());
    let transaction = Transaction::change(doc.text(), changes.into_iter());
    let transaction = transaction.with_selection(new_selection);
    doc.apply(&transaction, view.id);
    exit_select_mode(cx);
  }
}

// Re-export LSP commands
pub use super::lsp_commands::{
  code_action,
  document_diagnostics,
  document_symbols,
  goto_declaration,
  goto_definition,
  goto_implementation,
  goto_reference,
  goto_type_definition,
  rename_symbol,
  select_references,
  workspace_diagnostics,
  workspace_symbols,
};
