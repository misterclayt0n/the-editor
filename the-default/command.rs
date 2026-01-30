#![allow(unused_imports)]

use std::{
  borrow::Cow,
  collections::HashMap,
  path::Path,
  sync::OnceLock,
};

use smallvec::SmallVec;
use the_core::{
  grapheme::{
    nth_next_grapheme_boundary,
    nth_prev_grapheme_boundary,
    prev_grapheme_boundary,
  },
  line_ending::{
    get_line_ending_of_str,
    line_end_char_index,
  },
};
use the_dispatch::define;
use the_stdx::rope::RopeSliceExt;
use the_lib::{
  Tendril,
  auto_pairs::{
    self,
    AutoPairs,
  },
  editor::Editor,
  history::UndoKind,
  indent,
  movement::{
    self,
    Direction as MoveDir,
    Movement,
    move_horizontally,
    move_vertically,
    move_vertically_visual,
  },
  position::{
    Position,
    char_idx_at_coords,
  },
  registers::Registers,
  render::{
    text_annotations::TextAnnotations,
    text_format::TextFormat,
    char_at_visual_pos,
    visual_pos_at_char,
  },
  selection::{
    CursorId,
    Range,
    Selection,
  },
  syntax::{
    Loader,
    config::{
      Configuration,
      IndentationHeuristic,
    },
    resources::NullResources,
  },
  transaction::Transaction,
};

use crate::{
  Command,
  Direction,
  Key,
  KeyEvent,
  KeyOutcome,
  Modifiers,
  Motion,
  PendingInput,
  WordMotion,
  command_registry::{
    CommandPromptState,
    CommandRegistry,
    handle_command_prompt_key,
  },
  keymap::{
    Keymaps,
    Mode,
    handle_key as keymap_handle_key,
  },
};

define! {
  Default {
    pre_on_keypress: KeyEvent => (),
    on_keypress: KeyEvent => (),
    post_on_keypress: Command => (),
    pre_on_action: Command => (),
    on_action: Command => (),
    post_on_action: () => (),
    render_request: () => (),

    insert_char: char,
    insert_newline: (),
    delete_char: (),
    delete_char_forward: usize,
    delete_word_backward: usize,
    delete_word_forward: usize,
    kill_to_line_start: (),
    kill_to_line_end: (),
    insert_tab: (),
    goto_line_start: bool,
    goto_line_end: bool,
    page_up: bool,
    page_down: bool,
    move_cursor: Direction,
    add_cursor: Direction,
    motion: Motion,
    delete_selection: bool,
    change_selection: bool,
    replace_selection: char,
    replace_with_yanked: (),
    yank: (),
    paste: bool,
    switch_case: (),
    save: (),
    quit: (),
  }
}

pub type DefaultDispatchStatic<Ctx> = DefaultDispatch<
  Ctx,
  fn(&mut Ctx, KeyEvent),
  fn(&mut Ctx, KeyEvent),
  fn(&mut Ctx, Command),
  fn(&mut Ctx, Command),
  fn(&mut Ctx, Command),
  fn(&mut Ctx, ()),
  fn(&mut Ctx, ()),
  fn(&mut Ctx, char),
  fn(&mut Ctx, ()),
  fn(&mut Ctx, ()),
  fn(&mut Ctx, usize),
  fn(&mut Ctx, usize),
  fn(&mut Ctx, usize),
  fn(&mut Ctx, ()),
  fn(&mut Ctx, ()),
  fn(&mut Ctx, ()),
  fn(&mut Ctx, bool),
  fn(&mut Ctx, bool),
  fn(&mut Ctx, bool),
  fn(&mut Ctx, bool),
  fn(&mut Ctx, Direction),
  fn(&mut Ctx, Direction),
  fn(&mut Ctx, Motion),
  fn(&mut Ctx, bool),
  fn(&mut Ctx, bool),
  fn(&mut Ctx, char),
  fn(&mut Ctx, ()),
  fn(&mut Ctx, ()),
  fn(&mut Ctx, bool),
  fn(&mut Ctx, ()),
  fn(&mut Ctx, ()),
  fn(&mut Ctx, ()),
>;

#[derive(Clone, Copy)]
pub struct DispatchRef<Ctx> {
  ptr: *const DefaultDispatchStatic<Ctx>,
}

impl<Ctx> DispatchRef<Ctx> {
  pub fn from_ptr(ptr: *const DefaultDispatchStatic<Ctx>) -> Self {
    Self { ptr }
  }
}

impl<Ctx> std::ops::Deref for DispatchRef<Ctx> {
  type Target = DefaultDispatchStatic<Ctx>;

  fn deref(&self) -> &Self::Target {
    // Safety: DispatchRef is constructed from a valid dispatch pointer.
    unsafe { &*self.ptr }
  }
}

pub trait DefaultContext: Sized + 'static {
  fn editor(&mut self) -> &mut Editor;
  fn editor_ref(&self) -> &Editor;
  fn file_path(&self) -> Option<&Path>;
  fn request_render(&mut self);
  fn request_quit(&mut self);
  fn mode(&self) -> Mode;
  fn set_mode(&mut self, mode: Mode);
  fn keymaps(&mut self) -> &mut Keymaps;
  fn command_prompt_mut(&mut self) -> &mut CommandPromptState;
  fn command_prompt_ref(&self) -> &CommandPromptState;
  fn command_registry_mut(&mut self) -> &mut CommandRegistry<Self>;
  fn command_registry_ref(&self) -> &CommandRegistry<Self>;
  fn dispatch(&self) -> DispatchRef<Self>;
  fn pending_input(&self) -> Option<&PendingInput>;
  fn set_pending_input(&mut self, pending: Option<PendingInput>);
  fn registers(&self) -> &Registers;
  fn registers_mut(&mut self) -> &mut Registers;
  fn last_motion(&self) -> Option<Motion>;
  fn set_last_motion(&mut self, motion: Option<Motion>);
  fn text_format(&self) -> TextFormat;
  fn text_annotations(&self) -> TextAnnotations<'_>;
}

pub fn build_dispatch<Ctx>() -> DefaultDispatchStatic<Ctx>
where
  Ctx: DefaultContext,
{
  DefaultDispatch::new()
    .with_pre_on_keypress(pre_on_keypress::<Ctx> as fn(&mut Ctx, KeyEvent))
    .with_on_keypress(on_keypress::<Ctx> as fn(&mut Ctx, KeyEvent))
    .with_post_on_keypress(post_on_keypress::<Ctx> as fn(&mut Ctx, Command))
    .with_pre_on_action(pre_on_action::<Ctx> as fn(&mut Ctx, Command))
    .with_on_action(on_action::<Ctx> as fn(&mut Ctx, Command))
    .with_post_on_action(post_on_action::<Ctx> as fn(&mut Ctx, ()))
    .with_render_request(render_request::<Ctx> as fn(&mut Ctx, ()))
    .with_insert_char(insert_char::<Ctx> as fn(&mut Ctx, char))
    .with_insert_newline(insert_newline::<Ctx> as fn(&mut Ctx, ()))
    .with_delete_char(delete_char::<Ctx> as fn(&mut Ctx, ()))
    .with_delete_char_forward(delete_char_forward::<Ctx> as fn(&mut Ctx, usize))
    .with_delete_word_backward(delete_word_backward::<Ctx> as fn(&mut Ctx, usize))
    .with_delete_word_forward(delete_word_forward::<Ctx> as fn(&mut Ctx, usize))
    .with_kill_to_line_start(kill_to_line_start::<Ctx> as fn(&mut Ctx, ()))
    .with_kill_to_line_end(kill_to_line_end::<Ctx> as fn(&mut Ctx, ()))
    .with_insert_tab(insert_tab::<Ctx> as fn(&mut Ctx, ()))
    .with_goto_line_start(goto_line_start::<Ctx> as fn(&mut Ctx, bool))
    .with_goto_line_end(goto_line_end::<Ctx> as fn(&mut Ctx, bool))
    .with_page_up(page_up::<Ctx> as fn(&mut Ctx, bool))
    .with_page_down(page_down::<Ctx> as fn(&mut Ctx, bool))
    .with_move_cursor(move_cursor::<Ctx> as fn(&mut Ctx, Direction))
    .with_add_cursor(add_cursor::<Ctx> as fn(&mut Ctx, Direction))
    .with_motion(motion::<Ctx> as fn(&mut Ctx, Motion))
    .with_delete_selection(delete_selection::<Ctx> as fn(&mut Ctx, bool))
    .with_change_selection(change_selection::<Ctx> as fn(&mut Ctx, bool))
    .with_replace_selection(replace_selection::<Ctx> as fn(&mut Ctx, char))
    .with_replace_with_yanked(replace_with_yanked::<Ctx> as fn(&mut Ctx, ()))
    .with_yank(yank::<Ctx> as fn(&mut Ctx, ()))
    .with_paste(paste::<Ctx> as fn(&mut Ctx, bool))
    .with_switch_case(switch_case::<Ctx> as fn(&mut Ctx, ()))
    .with_save(save::<Ctx> as fn(&mut Ctx, ()))
    .with_quit(quit::<Ctx> as fn(&mut Ctx, ()))
}

pub fn handle_command<Ctx, D>(dispatch: &D, ctx: &mut Ctx, command: Command)
where
  Ctx: DefaultContext,
  D: DefaultApi<Ctx>,
{
  dispatch.pre_on_action(ctx, command);
}

pub fn handle_key<Ctx, D>(dispatch: &D, ctx: &mut Ctx, key: KeyEvent)
where
  Ctx: DefaultContext,
  D: DefaultApi<Ctx>,
{
  dispatch.pre_on_keypress(ctx, key);
}

fn pre_on_keypress<Ctx: DefaultContext>(ctx: &mut Ctx, key: KeyEvent) {
  ctx.dispatch().on_keypress(ctx, key);
}

fn handle_pending_input<Ctx: DefaultContext>(
  ctx: &mut Ctx,
  pending: PendingInput,
  key: KeyEvent,
) -> bool {
  match pending {
    PendingInput::FindChar { direction, inclusive, extend, count } => {
      if let Key::Char(ch) = key.key {
        find_char_impl(ctx, ch, direction, inclusive, extend, count);
      }
      true
    },
    PendingInput::InsertRegister => true, // TODO
    PendingInput::Placeholder => true,
    PendingInput::ReplaceSelection => {
      match key.key {
        Key::Char(ch) => ctx.dispatch().replace_selection(ctx, ch),
        Key::Enter | Key::NumpadEnter => {
          let line_ending = ctx.editor_ref().document().line_ending().as_str();
          replace_selection_with_str(ctx, line_ending);
        },
        _ => {},
      }
      true
    },
  }
}

fn on_keypress<Ctx: DefaultContext>(ctx: &mut Ctx, key: KeyEvent) {
  if let Some(pending) = ctx.pending_input().cloned() {
    ctx.set_pending_input(None);
    if handle_pending_input(ctx, pending, key) {
      ctx.dispatch().render_request(ctx, ());
      return;
    }
  }

  if ctx.mode() == Mode::Command {
    if handle_command_prompt_key(ctx, key) {
      return;
    }
  }

  match keymap_handle_key(ctx, key) {
    KeyOutcome::Command(command) => ctx.dispatch().post_on_keypress(ctx, command),
    KeyOutcome::Commands(commands) => {
      for command in commands {
        ctx.dispatch().post_on_keypress(ctx, command);
      }
    },
    KeyOutcome::Handled | KeyOutcome::Continue => {},
  }
}

fn post_on_keypress<Ctx: DefaultContext>(ctx: &mut Ctx, command: Command) {
  ctx.dispatch().pre_on_action(ctx, command);
}

fn pre_on_action<Ctx: DefaultContext>(ctx: &mut Ctx, command: Command) {
  ctx.dispatch().on_action(ctx, command);
}

fn on_action<Ctx: DefaultContext>(ctx: &mut Ctx, command: Command) {
  match command {
    Command::InsertChar(ch) => ctx.dispatch().insert_char(ctx, ch),
    Command::InsertNewline => ctx.dispatch().insert_newline(ctx, ()),
    Command::DeleteChar => ctx.dispatch().delete_char(ctx, ()),
    Command::DeleteCharForward { count } => ctx.dispatch().delete_char_forward(ctx, count),
    Command::DeleteWordBackward { count } => ctx.dispatch().delete_word_backward(ctx, count),
    Command::DeleteWordForward { count } => ctx.dispatch().delete_word_forward(ctx, count),
    Command::KillToLineStart => ctx.dispatch().kill_to_line_start(ctx, ()),
    Command::KillToLineEnd => ctx.dispatch().kill_to_line_end(ctx, ()),
    Command::InsertTab => ctx.dispatch().insert_tab(ctx, ()),
    Command::GotoLineStart { extend } => ctx.dispatch().goto_line_start(ctx, extend),
    Command::GotoLineEnd { extend } => ctx.dispatch().goto_line_end(ctx, extend),
    Command::PageUp { extend } => ctx.dispatch().page_up(ctx, extend),
    Command::PageDown { extend } => ctx.dispatch().page_down(ctx, extend),
    Command::FindChar { direction, inclusive, extend } => {
      ctx.set_pending_input(Some(PendingInput::FindChar { direction, inclusive, extend, count: 1 }));
    },
    Command::ParentNodeEnd { extend } => parent_node_end(ctx, extend),
    Command::ParentNodeStart { extend } => parent_node_start(ctx, extend),
    Command::Move(dir) => ctx.dispatch().move_cursor(ctx, dir),
    Command::AddCursor(dir) => ctx.dispatch().add_cursor(ctx, dir),
    Command::Motion(motion) => {
      ctx.set_last_motion(Some(motion));
      ctx.dispatch().motion(ctx, motion);
    },
    Command::DeleteSelection { yank } => ctx.dispatch().delete_selection(ctx, yank),
    Command::ChangeSelection { yank } => ctx.dispatch().change_selection(ctx, yank),
    Command::Replace => {
      ctx.set_pending_input(Some(PendingInput::ReplaceSelection));
    },
    Command::ReplaceWithYanked => ctx.dispatch().replace_with_yanked(ctx, ()),
    Command::Yank => ctx.dispatch().yank(ctx, ()),
    Command::Paste { after } => ctx.dispatch().paste(ctx, after),
    Command::RepeatLastMotion => {
      if let Some(motion) = ctx.last_motion() {
        ctx.dispatch().motion(ctx, motion);
      }
    },
    Command::SwitchCase => ctx.dispatch().switch_case(ctx, ()),
    Command::SwitchToUppercase => switch_to_uppercase(ctx),
    Command::SwitchToLowercase => switch_to_lowercase(ctx),
    Command::InsertAtLineStart => insert_at_line_start(ctx),
    Command::InsertAtLineEnd => insert_at_line_end(ctx),
    Command::AppendMode => append_mode(ctx),
    Command::OpenBelow => open_below(ctx),
    Command::OpenAbove => open_above(ctx),
    Command::CommitUndoCheckpoint => commit_undo_checkpoint(ctx),
    Command::CopySelectionOnNextLine => copy_selection_on_next_line(ctx),
    Command::CopySelectionOnPrevLine => copy_selection_on_prev_line(ctx),
    Command::SelectAll => select_all(ctx),
    Command::ExtendLineBelow { count } => extend_line_below(ctx, count),
    Command::ExtendLineAbove { count } => extend_line_above(ctx, count),
    Command::ExtendToLineBounds => extend_to_line_bounds(ctx),
    Command::ShrinkToLineBounds => shrink_to_line_bounds(ctx),
    Command::Undo { count } => undo(ctx, count),
    Command::Redo { count } => redo(ctx, count),
    Command::Earlier { count } => earlier(ctx, count),
    Command::Later { count } => later(ctx, count),
    Command::Indent { count } => indent(ctx, count),
    Command::Unindent { count } => unindent(ctx, count),
    Command::Save => ctx.dispatch().save(ctx, ()),
    Command::Quit => ctx.dispatch().quit(ctx, ()),
  }

  ctx.dispatch().post_on_action(ctx, ());
}

fn post_on_action<Ctx: DefaultContext>(ctx: &mut Ctx, _unit: ()) {
  if ctx.mode() != Mode::Insert {
    let _ = ctx.editor().document_mut().commit();
  }
  ctx.dispatch().render_request(ctx, ());
}

fn render_request<Ctx: DefaultContext>(ctx: &mut Ctx, _unit: ()) {
  ctx.request_render();
}

fn insert_char<Ctx: DefaultContext>(ctx: &mut Ctx, ch: char) {
  let doc = ctx.editor().document_mut();
  let selection = doc.selection().clone();

  let pairs = AutoPairs::default();
  if let Ok(Some(tx)) = auto_pairs::hook(doc.text(), &selection, ch, &pairs) {
    let _ = doc.apply_transaction(&tx);
    return;
  }

  let mut text = Tendril::new();
  text.push(ch);

  let cursors = selection.clone().cursors(doc.text().slice(..));
  let Ok(tx) = Transaction::insert(doc.text(), &cursors, text) else {
    return;
  };

  let _ = doc.apply_transaction(&tx);
}

fn delete_char<Ctx: DefaultContext>(ctx: &mut Ctx, _unit: ()) {
  let doc = ctx.editor().document_mut();
  let selection = doc.selection().clone();
  let slice = doc.text().slice(..);

  let pairs = AutoPairs::default();
  if let Ok(Some(tx)) = auto_pairs::delete_hook(doc.text(), &selection, &pairs) {
    let _ = doc.apply_transaction(&tx);
    return;
  }

  let tab_width: usize = 4;
  let indent_width = doc.indent_style().indent_width(tab_width);

  let tx = Transaction::delete_by_selection(doc.text(), &selection, |range| {
    let pos = range.cursor(slice);
    if pos == 0 {
      return (pos, pos);
    }

    let line_start_pos = slice.line_to_char(range.cursor_line(slice));
    let fragment: Cow<'_, str> = Cow::from(slice.slice(line_start_pos..pos));

    if !fragment.is_empty() && fragment.chars().all(|ch| ch == ' ' || ch == '\t') {
      if slice.get_char(pos.saturating_sub(1)) == Some('\t') {
        return (nth_prev_grapheme_boundary(slice, pos, 1), pos);
      }

      let width: usize = fragment
        .chars()
        .map(|ch| if ch == '\t' { tab_width } else { 1 })
        .sum();
      let mut drop = width % indent_width;
      if drop == 0 {
        drop = indent_width;
      }

      let mut chars = fragment.chars().rev();
      let mut start = pos;
      for _ in 0..drop {
        match chars.next() {
          Some(' ') => start = start.saturating_sub(1),
          _ => break,
        }
      }
      (start, pos)
    } else {
      let count = 1;
      (nth_prev_grapheme_boundary(slice, pos, count), pos)
    }
  });

  let Ok(tx) = tx else {
    return;
  };

  let _ = doc.apply_transaction(&tx);
}

fn delete_char_forward<Ctx: DefaultContext>(ctx: &mut Ctx, count: usize) {
  let doc = ctx.editor().document_mut();
  let selection = doc.selection().clone();
  let slice = doc.text().slice(..);
  let count = count.max(1);

  let tx = Transaction::delete_by_selection(doc.text(), &selection, |range| {
    let pos = range.cursor(slice);
    (pos, nth_next_grapheme_boundary(slice, pos, count))
  });

  let Ok(tx) = tx else {
    return;
  };

  let _ = doc.apply_transaction(&tx);
}

fn delete_word_backward<Ctx: DefaultContext>(ctx: &mut Ctx, count: usize) {
  let doc = ctx.editor().document_mut();
  let selection = doc.selection().clone();
  let slice = doc.text().slice(..);
  let count = count.max(1);

  let tx = Transaction::delete_by_selection(doc.text(), &selection, |range| {
    let cursor_pos = range.cursor(slice);
    if cursor_pos == 0 {
      return (0, 0);
    }
    let target = movement::move_prev_word_start(slice, *range, count);
    (target.from(), cursor_pos)
  });

  let Ok(tx) = tx else {
    return;
  };

  let _ = doc.apply_transaction(&tx);
}

fn delete_word_forward<Ctx: DefaultContext>(ctx: &mut Ctx, count: usize) {
  let doc = ctx.editor().document_mut();
  let selection = doc.selection().clone();
  let slice = doc.text().slice(..);
  let count = count.max(1);
  let text_len = slice.len_chars();

  let tx = Transaction::delete_by_selection(doc.text(), &selection, |range| {
    let cursor_pos = range.cursor(slice);
    if cursor_pos >= text_len {
      return (cursor_pos, cursor_pos);
    }
    let target = movement::move_next_word_end(slice, *range, count);
    (cursor_pos, target.to())
  });

  let Ok(tx) = tx else {
    return;
  };

  let _ = doc.apply_transaction(&tx);
}

fn kill_to_line_start<Ctx: DefaultContext>(ctx: &mut Ctx, _unit: ()) {
  let doc = ctx.editor().document_mut();
  let selection = doc.selection().clone();
  let slice = doc.text().slice(..);

  let tx = Transaction::delete_by_selection(doc.text(), &selection, |range| {
    let cursor_pos = range.cursor(slice);
    let line = range.cursor_line(slice);
    let line_start = slice.line_to_char(line);

    let head = if cursor_pos == line_start && line != 0 {
      // At line start, delete back to end of previous line (join lines)
      let prev_line = slice.line(line - 1);
      let prev_line_start = slice.line_to_char(line - 1);
      // Line end is before the newline character
      prev_line_start + prev_line.len_chars().saturating_sub(1)
    } else if let Some(first_non_ws) = slice.line(line).first_non_whitespace_char() {
      let first_non_ws_pos = line_start + first_non_ws;
      if first_non_ws_pos < cursor_pos {
        // Cursor is after first non-whitespace, delete to first non-whitespace
        first_non_ws_pos
      } else {
        // Delete to line start
        line_start
      }
    } else {
      // Blank line, delete to line start
      line_start
    };

    (head, cursor_pos)
  });

  let Ok(tx) = tx else {
    return;
  };

  let _ = doc.apply_transaction(&tx);
}

fn kill_to_line_end<Ctx: DefaultContext>(ctx: &mut Ctx, _unit: ()) {
  let doc = ctx.editor().document_mut();
  let selection = doc.selection().clone();
  let slice = doc.text().slice(..);

  let tx = Transaction::delete_by_selection(doc.text(), &selection, |range| {
    let cursor_pos = range.cursor(slice);
    let line = range.cursor_line(slice);
    let line_end_pos = line_end_char_index(&slice, line);

    if cursor_pos == line_end_pos {
      // Cursor is at line end, delete the newline (join with next line)
      let next_line_start = slice.line_to_char(line + 1);
      (cursor_pos, next_line_start)
    } else {
      // Delete from cursor to line end
      (cursor_pos, line_end_pos)
    }
  });

  let Ok(tx) = tx else {
    return;
  };

  let _ = doc.apply_transaction(&tx);
}

fn insert_tab<Ctx: DefaultContext>(ctx: &mut Ctx, _unit: ()) {
  let doc = ctx.editor().document_mut();
  let selection = doc.selection().clone();

  let indent = Tendril::from(doc.indent_style().as_str());
  let cursors = selection.cursors(doc.text().slice(..));

  let Ok(tx) = Transaction::insert(doc.text(), &cursors, indent) else {
    return;
  };

  let _ = doc.apply_transaction(&tx);
}

fn insert_newline<Ctx: DefaultContext>(ctx: &mut Ctx, _unit: ()) {
  let doc = ctx.editor().document_mut();
  let contents = doc.text();
  let text = contents.slice(..);
  let selection = doc.selection().clone();
  let line_ending = doc.line_ending().as_str();
  let indent_unit = doc.indent_style().as_str();

  let mut ranges = SmallVec::with_capacity(selection.len());
  let mut global_offs: isize = 0;
  let mut new_text = String::new();
  let mut last_pos = 0;
  let pairs = AutoPairs::default();

  let tx = Transaction::change_by_selection(contents, &selection, |range| {
    let pos = range.cursor(text);
    let current_line = text.char_to_line(pos);
    let line_start = text.line_to_char(current_line);
    let mut chars_deleted = 0usize;

    let (from, to, local_offs) =
      if let Some(idx) = text.slice(line_start..pos).last_non_whitespace_char() {
        let first_trailing_whitespace_char = (line_start + idx + 1).clamp(last_pos, pos);
        last_pos = pos;
        chars_deleted = pos - first_trailing_whitespace_char;

        let line = text.line(current_line);
        let indent = match line.first_non_whitespace_char() {
          Some(pos) => line.slice(..pos).to_string(),
          None => String::new(),
        };

        let prev = if pos == 0 { ' ' } else { contents.char(pos - 1) };
        let curr = contents.get_char(pos).unwrap_or(' ');

        let on_auto_pair = pairs.pairs().iter().any(|pair| {
          pair.open_last_char() == Some(prev) && pair.close_first_char() == Some(curr)
        });

        if on_auto_pair {
          let inner_indent = indent.clone() + indent_unit;
          new_text.reserve_exact(line_ending.len() * 2 + indent.len() + inner_indent.len());
          new_text.push_str(line_ending);
          new_text.push_str(&inner_indent);
          let local_offs = new_text.chars().count();
          new_text.push_str(line_ending);
          new_text.push_str(&indent);
          (
            first_trailing_whitespace_char,
            pos,
            local_offs as isize - chars_deleted as isize,
          )
        } else {
          new_text.reserve_exact(line_ending.len() + indent.len());
          new_text.push_str(line_ending);
          new_text.push_str(&indent);
          (
            first_trailing_whitespace_char,
            pos,
            new_text.chars().count() as isize - chars_deleted as isize,
          )
        }
      } else {
        new_text.push_str(line_ending);
        (line_start, line_start, new_text.chars().count() as isize)
      };

    let new_range = if range.cursor(text) > range.anchor {
      Range::new(
        (range.anchor as isize + global_offs) as usize,
        (range.head as isize + local_offs + global_offs) as usize,
      )
    } else {
      Range::new(
        (range.anchor as isize + local_offs + global_offs) as usize,
        (range.head as isize + local_offs + global_offs) as usize,
      )
    };

    ranges.push(new_range);
    global_offs += new_text.chars().count() as isize - chars_deleted as isize;

    let tendril = Tendril::from(new_text.as_str());
    new_text.clear();

    (from, to, Some(tendril))
  });

  let Ok(tx) = tx else {
    return;
  };

  let cursor_ids: SmallVec<[CursorId; 1]> =
    selection.cursor_ids().iter().copied().collect();
  let new_selection = Selection::new_with_ids(ranges, cursor_ids).unwrap_or_else(|_| selection);
  let tx = tx.with_selection(new_selection);
  let _ = doc.apply_transaction(&tx);
}

fn goto_line_start<Ctx: DefaultContext>(ctx: &mut Ctx, extend: bool) {
  let doc = ctx.editor().document_mut();
  let selection = doc.selection().clone();
  let slice = doc.text().slice(..);

  let new_selection = selection.transform(|range| {
    let line = range.cursor_line(slice);
    let pos = slice.line_to_char(line);
    range.put_cursor(slice, pos, extend)
  });

  let _ = doc.set_selection(new_selection);
}

fn goto_line_end<Ctx: DefaultContext>(ctx: &mut Ctx, extend: bool) {
  let doc = ctx.editor().document_mut();
  let selection = doc.selection().clone();
  let slice = doc.text().slice(..);

  let new_selection = selection.transform(|range| {
    let line = range.cursor_line(slice);
    let pos = line_end_char_index(&slice, line);
    range.put_cursor(slice, pos, extend)
  });

  let _ = doc.set_selection(new_selection);
}

fn page_up<Ctx: DefaultContext>(ctx: &mut Ctx, extend: bool) {
  let height = ctx.editor().view().viewport.height as usize;
  let count = height.saturating_sub(2).max(1); // Leave some overlap

  let new_selection = {
    let doc = ctx.editor().document_mut();
    let selection = doc.selection().clone();
    let slice = doc.text().slice(..);

    let text_fmt = TextFormat::default();
    let mut annotations = TextAnnotations::default();
    let behavior = if extend {
      Movement::Extend
    } else {
      Movement::Move
    };

    selection.transform(|range| {
      move_vertically(slice, range, MoveDir::Backward, count, behavior, &text_fmt, &mut annotations)
    })
  };

  let _ = ctx.editor().document_mut().set_selection(new_selection);
}

fn page_down<Ctx: DefaultContext>(ctx: &mut Ctx, extend: bool) {
  let height = ctx.editor().view().viewport.height as usize;
  let count = height.saturating_sub(2).max(1); // Leave some overlap

  let new_selection = {
    let doc = ctx.editor().document_mut();
    let selection = doc.selection().clone();
    let slice = doc.text().slice(..);

    let text_fmt = TextFormat::default();
    let mut annotations = TextAnnotations::default();
    let behavior = if extend {
      Movement::Extend
    } else {
      Movement::Move
    };

    selection.transform(|range| {
      move_vertically(slice, range, MoveDir::Forward, count, behavior, &text_fmt, &mut annotations)
    })
  };

  let _ = ctx.editor().document_mut().set_selection(new_selection);
}

fn find_char_impl<Ctx: DefaultContext>(
  ctx: &mut Ctx,
  ch: char,
  direction: Direction,
  inclusive: bool,
  extend: bool,
  count: usize,
) {
  use the_lib::search::{find_nth, SearchDirection, SearchStart};

  let doc = ctx.editor().document_mut();
  let selection = doc.selection().clone();
  let slice = doc.text().slice(..);

  let search_dir = match direction {
    Direction::Forward => SearchDirection::Next,
    Direction::Backward => SearchDirection::Prev,
    _ => return, // Only Forward/Backward make sense for find_char
  };

  let new_selection = selection.transform(|range| {
    let cursor = range.cursor(slice);
    // Start search from position after/before cursor (exclusive of current position)
    let search_pos = match direction {
      Direction::Forward => cursor + 1,
      Direction::Backward => cursor,
      _ => return range, // Should not happen
    };

    if let Some(found) = find_nth(slice, ch, search_pos, count, search_dir, SearchStart::Inclusive) {
      let target = if inclusive {
        found
      } else {
        // "till" - stop one char before the match
        match direction {
          Direction::Forward => found.saturating_sub(1),
          Direction::Backward => found + 1,
          _ => return range, // Should not happen
        }
      };
      range.put_cursor(slice, target, extend)
    } else {
      range // No match found, keep original
    }
  });

  let _ = doc.set_selection(new_selection);
}

fn parent_node_end<Ctx: DefaultContext>(ctx: &mut Ctx, extend: bool) {
  use the_lib::movement::{Direction as MoveDirection, move_parent_node_end};

  let doc = ctx.editor().document_mut();
  let Some(syntax) = doc.syntax() else {
    return; // No syntax tree available
  };

  let selection = doc.selection().clone();
  let slice = doc.text().slice(..);
  let movement = if extend { Movement::Extend } else { Movement::Move };

  let new_selection = move_parent_node_end(syntax, slice, selection, MoveDirection::Forward, movement);
  let _ = doc.set_selection(new_selection);
}

fn parent_node_start<Ctx: DefaultContext>(ctx: &mut Ctx, extend: bool) {
  use the_lib::movement::{Direction as MoveDirection, move_parent_node_end};

  let doc = ctx.editor().document_mut();
  let Some(syntax) = doc.syntax() else {
    return; // No syntax tree available
  };

  let selection = doc.selection().clone();
  let slice = doc.text().slice(..);
  let movement = if extend { Movement::Extend } else { Movement::Move };

  let new_selection = move_parent_node_end(syntax, slice, selection, MoveDirection::Backward, movement);
  let _ = doc.set_selection(new_selection);
}

fn move_cursor<Ctx: DefaultContext>(ctx: &mut Ctx, direction: Direction) {
  {
    let editor = ctx.editor();
    let doc = editor.document_mut();
    let selection = doc.selection().clone();

    let (dir, vertical) = match direction {
      Direction::Left | Direction::Backward => (MoveDir::Backward, false),
      Direction::Right | Direction::Forward => (MoveDir::Forward, false),
      Direction::Up => (MoveDir::Backward, true),
      Direction::Down => (MoveDir::Forward, true),
    };

    let new_selection = {
      let slice = doc.text().slice(..);
      let text_fmt = TextFormat::default();
      let mut annotations = TextAnnotations::default();
      let mut ranges: SmallVec<_> = SmallVec::with_capacity(selection.ranges().len());
      let mut ids: SmallVec<_> = SmallVec::with_capacity(selection.cursor_ids().len());

      for (cursor_id, range) in selection.iter_with_ids() {
        let new_range = if vertical {
          move_vertically(
            slice,
            *range,
            dir,
            1,
            Movement::Move,
            &text_fmt,
            &mut annotations,
          )
        } else {
          move_horizontally(
            slice,
            *range,
            dir,
            1,
            Movement::Move,
            &text_fmt,
            &mut annotations,
          )
        };
        ranges.push(new_range);
        ids.push(cursor_id);
      }

      Selection::new_with_ids(ranges, ids).ok()
    };

    if let Some(selection) = new_selection {
      let _ = doc.set_selection(selection);
    }
  }
}

fn add_cursor<Ctx: DefaultContext>(ctx: &mut Ctx, direction: Direction) {
  let dir = match direction {
    Direction::Up => MoveDir::Backward,
    Direction::Down => MoveDir::Forward,
    _ => return,
  };
  {
    let editor = ctx.editor();
    let doc = editor.document_mut();
    let selection = doc.selection().clone();

    let new_selection = {
      let slice = doc.text().slice(..);
      let text_fmt = TextFormat::default();
      let mut annotations = TextAnnotations::default();
      let mut ranges: SmallVec<_> = selection.ranges().iter().copied().collect();
      let mut ids: SmallVec<_> = selection.cursor_ids().iter().copied().collect();

      for range in selection.ranges() {
        let moved = move_vertically(
          slice,
          *range,
          dir,
          1,
          Movement::Move,
          &text_fmt,
          &mut annotations,
        );
        ranges.push(moved);
        ids.push(CursorId::fresh());
      }

      Selection::new_with_ids(ranges, ids).ok()
    };

    if let Some(selection) = new_selection {
      let _ = doc.set_selection(selection);
    }
  }
}

fn motion<Ctx: DefaultContext>(ctx: &mut Ctx, motion: Motion) {
  let viewport_width = ctx.editor().view().viewport.width;
  let next = {
    let editor = ctx.editor();
    let doc = editor.document_mut();
    let selection = doc.selection().clone();
    let slice = doc.text().slice(..);

    let mut text_fmt = TextFormat::default();
    text_fmt.viewport_width = viewport_width;
    let mut annotations = TextAnnotations::default();

    match motion {
      Motion::Char { dir, extend, count } => {
        let dir = match dir {
          Direction::Left => MoveDir::Backward,
          Direction::Right => MoveDir::Forward,
          _ => return,
        };
        let behavior = if extend {
          Movement::Extend
        } else {
          Movement::Move
        };
        let count = count.max(1);
        selection.clone().transform(|range| {
          move_horizontally(
            slice,
            range,
            dir,
            count,
            behavior,
            &text_fmt,
            &mut annotations,
          )
        })
      },
      Motion::Line { dir, extend, count } => {
        let dir = match dir {
          Direction::Up => MoveDir::Backward,
          Direction::Down => MoveDir::Forward,
          _ => return,
        };
        let behavior = if extend {
          Movement::Extend
        } else {
          Movement::Move
        };
        let count = count.max(1);
        selection.clone().transform(|range| {
          move_vertically(
            slice,
            range,
            dir,
            count,
            behavior,
            &text_fmt,
            &mut annotations,
          )
        })
      },
      Motion::VisualLine { dir, extend, count } => {
        let dir = match dir {
          Direction::Up => MoveDir::Backward,
          Direction::Down => MoveDir::Forward,
          _ => return,
        };
        let behavior = if extend {
          Movement::Extend
        } else {
          Movement::Move
        };
        let count = count.max(1);
        selection.clone().transform(|range| {
          move_vertically_visual(
            slice,
            range,
            dir,
            count,
            behavior,
            &text_fmt,
            &mut annotations,
          )
        })
      },
      Motion::Word {
        kind,
        extend,
        count,
      } => {
        let count = count.max(1);
        if extend {
          selection.clone().transform(|range| {
            let word = match kind {
              WordMotion::NextWordStart => movement::move_next_word_start(slice, range, count),
              WordMotion::PrevWordStart => movement::move_prev_word_start(slice, range, count),
              WordMotion::NextWordEnd => movement::move_next_word_end(slice, range, count),
              WordMotion::PrevWordEnd => movement::move_prev_word_end(slice, range, count),
              WordMotion::NextLongWordStart => {
                movement::move_next_long_word_start(slice, range, count)
              },
              WordMotion::PrevLongWordStart => {
                movement::move_prev_long_word_start(slice, range, count)
              },
              WordMotion::NextLongWordEnd => movement::move_next_long_word_end(slice, range, count),
              WordMotion::PrevLongWordEnd => movement::move_prev_long_word_end(slice, range, count),
              WordMotion::NextSubWordStart => {
                movement::move_next_sub_word_start(slice, range, count)
              },
              WordMotion::PrevSubWordStart => {
                movement::move_prev_sub_word_start(slice, range, count)
              },
              WordMotion::NextSubWordEnd => movement::move_next_sub_word_end(slice, range, count),
              WordMotion::PrevSubWordEnd => movement::move_prev_sub_word_end(slice, range, count),
            };
            let pos = word.cursor(slice);
            range.put_cursor(slice, pos, true)
          })
        } else {
          selection.clone().transform(|range| {
            match kind {
              WordMotion::NextWordStart => movement::move_next_word_start(slice, range, count),
              WordMotion::PrevWordStart => movement::move_prev_word_start(slice, range, count),
              WordMotion::NextWordEnd => movement::move_next_word_end(slice, range, count),
              WordMotion::PrevWordEnd => movement::move_prev_word_end(slice, range, count),
              WordMotion::NextLongWordStart => {
                movement::move_next_long_word_start(slice, range, count)
              },
              WordMotion::PrevLongWordStart => {
                movement::move_prev_long_word_start(slice, range, count)
              },
              WordMotion::NextLongWordEnd => movement::move_next_long_word_end(slice, range, count),
              WordMotion::PrevLongWordEnd => movement::move_prev_long_word_end(slice, range, count),
              WordMotion::NextSubWordStart => {
                movement::move_next_sub_word_start(slice, range, count)
              },
              WordMotion::PrevSubWordStart => {
                movement::move_prev_sub_word_start(slice, range, count)
              },
              WordMotion::NextSubWordEnd => movement::move_next_sub_word_end(slice, range, count),
              WordMotion::PrevSubWordEnd => movement::move_prev_sub_word_end(slice, range, count),
            }
          })
        }
      },
      Motion::FileStart { extend } => {
        selection
          .clone()
          .transform(|range| range.put_cursor(slice, 0, extend))
      },
      Motion::FileEnd { extend } => {
        let pos = slice.len_chars();
        selection
          .clone()
          .transform(|range| range.put_cursor(slice, pos, extend))
      },
      Motion::LastLine { extend } => {
        let line = slice.len_lines().saturating_sub(1);
        let pos = slice.line_to_char(line);
        selection
          .clone()
          .transform(|range| range.put_cursor(slice, pos, extend))
      },
      Motion::Column { col, extend } => {
        let col = col.saturating_sub(1);
        selection.clone().transform(|range| {
          let line = slice.char_to_line(range.cursor(slice));
          let pos = char_idx_at_coords(slice, Position::new(line, col));
          range.put_cursor(slice, pos, extend)
        })
      },
    }
  };

  {
    let doc = ctx.editor().document_mut();
    let _ = doc.set_selection(next);
  }
}

fn save<Ctx: DefaultContext>(ctx: &mut Ctx, _unit: ()) {
  let Some(path) = ctx.file_path().map(|path| path.to_path_buf()) else {
    return;
  };
  let text = ctx.editor().document().text().to_string();
  let _ = std::fs::write(path, text);
}

fn quit<Ctx: DefaultContext>(ctx: &mut Ctx, _unit: ()) {
  ctx.request_quit();
}

fn delete_selection<Ctx: DefaultContext>(ctx: &mut Ctx, yank: bool) {
  let doc = ctx.editor().document_mut();
  let selection = doc.selection().clone();
  let slice = doc.text().slice(..);

  // Collect fragments, treating empty selections as 1-char selections
  let fragments: Vec<String> = selection
    .ranges()
    .iter()
    .map(|range| {
      let (from, to) = if range.is_empty() {
        (range.from(), nth_next_grapheme_boundary(slice, range.from(), 1))
      } else {
        (range.from(), range.to())
      };
      slice.slice(from..to).to_string()
    })
    .collect();

  let tx = Transaction::delete_by_selection(doc.text(), &selection, |range| {
    // For empty selections (cursor only), delete the grapheme at cursor
    if range.is_empty() {
      (range.from(), nth_next_grapheme_boundary(slice, range.from(), 1))
    } else {
      (range.from(), range.to())
    }
  });

  if let Ok(tx) = tx {
    let _ = doc.apply_transaction(&tx);
  }

  if yank {
    let _ = ctx.registers_mut().write('"', fragments);
  }

  ctx.set_mode(Mode::Normal);
  ctx.request_render();
}

fn change_selection<Ctx: DefaultContext>(ctx: &mut Ctx, yank: bool) {
  let doc = ctx.editor().document_mut();
  let selection = doc.selection().clone();
  let slice = doc.text().slice(..);

  // Collect fragments, treating empty selections as 1-char selections
  let fragments: Vec<String> = selection
    .ranges()
    .iter()
    .map(|range| {
      let (from, to) = if range.is_empty() {
        (range.from(), nth_next_grapheme_boundary(slice, range.from(), 1))
      } else {
        (range.from(), range.to())
      };
      slice.slice(from..to).to_string()
    })
    .collect();

  let tx = Transaction::delete_by_selection(doc.text(), &selection, |range| {
    // For empty selections (cursor only), delete the grapheme at cursor
    if range.is_empty() {
      (range.from(), nth_next_grapheme_boundary(slice, range.from(), 1))
    } else {
      (range.from(), range.to())
    }
  });

  if let Ok(tx) = tx {
    let _ = doc.apply_transaction(&tx);
  }

  if yank {
    let _ = ctx.registers_mut().write('"', fragments);
  }

  ctx.set_mode(Mode::Insert);
  ctx.request_render();
}

fn replace_selection<Ctx: DefaultContext>(ctx: &mut Ctx, ch: char) {
  let mut buf = [0u8; 4];
  let replacement = ch.encode_utf8(&mut buf);
  replace_selection_with_str(ctx, replacement);
}

fn replace_selection_with_str<Ctx: DefaultContext>(ctx: &mut Ctx, replacement: &str) {
  let doc = ctx.editor().document_mut();
  let selection = doc.selection().clone();
  let slice = doc.text().slice(..);

  // Create transaction that replaces each range with the character repeated
  let tx = Transaction::change_by_selection(doc.text(), &selection, |range| {
    if range.is_empty() {
      (range.from(), range.to(), None)
    } else {
      let graphemes = slice.slice(range.from()..range.to()).graphemes().count();
      if graphemes == 0 {
        return (range.from(), range.to(), None);
      }
      let mut out = Tendril::new();
      for _ in 0..graphemes {
        out.push_str(replacement);
      }
      (range.from(), range.to(), Some(out))
    }
  });

  if let Ok(tx) = tx {
    let _ = doc.apply_transaction(&tx);
  }

  if ctx.mode() == Mode::Select {
    ctx.set_mode(Mode::Normal);
  }
  ctx.request_render();
}

fn replace_with_yanked<Ctx: DefaultContext>(ctx: &mut Ctx, _unit: ()) {
  // Read the register values using shared references
  let replacement: Option<String> = {
    let doc = ctx.editor_ref().document();
    ctx.registers().read('"', doc).and_then(|mut values| {
      values.next().map(|v| v.into_owned())
    })
  };

  let Some(replacement) = replacement else {
    ctx.set_mode(Mode::Normal);
    ctx.request_render();
    return;
  };

  let doc = ctx.editor().document_mut();
  let selection = doc.selection().clone();
  let slice = doc.text().slice(..);

  // Replace each selection range with the yanked content
  let tx = Transaction::change_by_selection(doc.text(), &selection, |range| {
    // For empty selections (cursor only), replace the grapheme at cursor
    let (from, to) = if range.is_empty() {
      (range.from(), nth_next_grapheme_boundary(slice, range.from(), 1))
    } else {
      (range.from(), range.to())
    };
    (from, to, Some(Tendril::from(replacement.as_str())))
  });

  if let Ok(tx) = tx {
    let _ = doc.apply_transaction(&tx);
  }

  ctx.set_mode(Mode::Normal);
  ctx.request_render();
}

fn yank<Ctx: DefaultContext>(ctx: &mut Ctx, _unit: ()) {
  let doc = ctx.editor().document_mut();
  let selection = doc.selection().clone();
  let slice = doc.text().slice(..);

  let fragments: Vec<String> = selection
    .fragments(slice)
    .map(Cow::into_owned)
    .collect();

  let _ = ctx.registers_mut().write('"', fragments);

  ctx.set_mode(Mode::Normal);
  ctx.request_render();
}

fn paste<Ctx: DefaultContext>(ctx: &mut Ctx, after: bool) {
  let values: Option<Vec<String>> = {
    let doc = ctx.editor_ref().document();
    ctx
      .registers()
      .read('"', doc)
      .map(|iter| iter.map(|v| v.into_owned()).collect())
  };

  let Some(values) = values else {
    ctx.request_render();
    return;
  };

  if values.is_empty() {
    ctx.request_render();
    return;
  }

  let mode = ctx.mode();
  let doc = ctx.editor().document_mut();
  let text = doc.text();
  let selection = doc.selection().clone();
  let line_ending = doc.line_ending().as_str();

  let linewise = values
    .iter()
    .any(|value| get_line_ending_of_str(value).is_some());

  let normalize_line_endings = |value: &str| {
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars().peekable();
    while let Some(ch) = chars.next() {
      match ch {
        '\r' => {
          if chars.peek() == Some(&'\n') {
            chars.next();
          }
          out.push_str(line_ending);
        },
        '\n' => out.push_str(line_ending),
        _ => out.push(ch),
      }
    }
    out
  };

  let map_value = |value: &str| {
    let normalized = normalize_line_endings(value);
    Tendril::from(normalized.as_str())
  };

  let last = map_value(values.last().unwrap());
  let mut values = values
    .iter()
    .map(|value| map_value(value))
    .chain(std::iter::repeat(last));

  let mut offset = 0usize;
  let mut ranges = SmallVec::with_capacity(selection.len());

  let Ok(mut tx) = Transaction::change_by_selection(text, &selection, |range| {
    let pos = if linewise {
      if after {
        let line = range.line_range(text.slice(..)).1;
        text.line_to_char((line + 1).min(text.len_lines()))
      } else {
        text.line_to_char(text.char_to_line(range.from()))
      }
    } else if after {
      range.to()
    } else {
      range.from()
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
  }) else {
    return;
  };

  if mode == Mode::Normal {
    let cursor_ids: SmallVec<[CursorId; 1]> =
      selection.cursor_ids().iter().copied().collect();
    let new_selection =
      Selection::new_with_ids(ranges, cursor_ids).unwrap_or_else(|_| selection.clone());
    tx = tx.with_selection(new_selection);
  }

  let _ = doc.apply_transaction(&tx);

  ctx.set_mode(Mode::Normal);
  ctx.request_render();
}

fn switch_case<Ctx: DefaultContext>(ctx: &mut Ctx, _unit: ()) {
  switch_case_impl(ctx, |s| {
    s.chars()
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

fn switch_to_uppercase<Ctx: DefaultContext>(ctx: &mut Ctx) {
  switch_case_impl(ctx, |s| s.to_uppercase().into());
}

fn switch_to_lowercase<Ctx: DefaultContext>(ctx: &mut Ctx) {
  switch_case_impl(ctx, |s| s.to_lowercase().into());
}

fn switch_case_impl<Ctx, F>(ctx: &mut Ctx, change_fn: F)
where
  Ctx: DefaultContext,
  F: Fn(&str) -> Tendril,
{
  let doc = ctx.editor().document_mut();
  let selection = doc.selection().clone();
  let slice = doc.text().slice(..);

  let tx = Transaction::change_by_selection(doc.text(), &selection, |range| {
    let (from, to) = if range.is_empty() {
      (range.from(), nth_next_grapheme_boundary(slice, range.from(), 1))
    } else {
      (range.from(), range.to())
    };
    let text: Tendril = change_fn(&slice.slice(from..to).to_string());
    (from, to, Some(text))
  });

  if let Ok(tx) = tx {
    let _ = doc.apply_transaction(&tx);
  }

  ctx.set_mode(Mode::Normal);
  ctx.request_render();
}

enum CaseSwitcher {
  Upper(std::char::ToUppercase),
  Lower(std::char::ToLowercase),
  Keep(Option<char>),
}

impl Iterator for CaseSwitcher {
  type Item = char;

  fn next(&mut self) -> Option<Self::Item> {
    match self {
      CaseSwitcher::Upper(u) => u.next(),
      CaseSwitcher::Lower(l) => l.next(),
      CaseSwitcher::Keep(ch) => ch.take(),
    }
  }
}

fn insert_at_line_start<Ctx: DefaultContext>(ctx: &mut Ctx) {
  let doc = ctx.editor().document_mut();
  let selection = doc.selection().clone();
  let slice = doc.text().slice(..);

  let new_selection = selection.transform(|range| {
    let line = range.cursor_line(slice);
    let line_start = slice.line_to_char(line);
    let pos = slice
      .line(line)
      .first_non_whitespace_char()
      .map(|offset| line_start + offset)
      .unwrap_or(line_start);
    range.put_cursor(slice, pos, false)
  });

  let _ = doc.set_selection(new_selection);
  ctx.set_mode(Mode::Insert);
  ctx.request_render();
}

fn insert_at_line_end<Ctx: DefaultContext>(ctx: &mut Ctx) {
  let doc = ctx.editor().document_mut();
  let selection = doc.selection().clone();
  let slice = doc.text().slice(..);

  let new_selection = selection.transform(|range| {
    let line = range.cursor_line(slice);
    let pos = line_end_char_index(&slice, line);
    range.put_cursor(slice, pos, false)
  });

  let _ = doc.set_selection(new_selection);
  ctx.set_mode(Mode::Insert);
  ctx.request_render();
}

fn append_mode<Ctx: DefaultContext>(ctx: &mut Ctx) {
  let doc = ctx.editor().document_mut();
  let selection = doc.selection().clone();
  let slice = doc.text().slice(..);

  let new_selection = selection.transform(|range| {
    let pos = nth_next_grapheme_boundary(slice, range.cursor(slice), 1);
    range.put_cursor(slice, pos, false)
  });

  let _ = doc.set_selection(new_selection);
  ctx.set_mode(Mode::Insert);
  ctx.request_render();
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum OpenDirection {
  Above,
  Below,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CommentContinuation {
  Enabled,
  Disabled,
}

fn syntax_loader() -> &'static Loader {
  static LOADER: OnceLock<Loader> = OnceLock::new();
  LOADER.get_or_init(|| {
    let config = Configuration {
      language: Vec::new(),
      language_server: HashMap::new(),
    };
    Loader::new(config, NullResources).expect("syntax loader")
  })
}

fn open_below<Ctx: DefaultContext>(ctx: &mut Ctx) {
  open(ctx, OpenDirection::Below, CommentContinuation::Enabled);
}

fn open_above<Ctx: DefaultContext>(ctx: &mut Ctx) {
  open(ctx, OpenDirection::Above, CommentContinuation::Enabled);
}

fn commit_undo_checkpoint<Ctx: DefaultContext>(ctx: &mut Ctx) {
  let _ = ctx.editor().document_mut().commit();
}

fn copy_selection_on_next_line<Ctx: DefaultContext>(ctx: &mut Ctx) {
  copy_selection_on_line(ctx, Direction::Forward);
}

fn copy_selection_on_prev_line<Ctx: DefaultContext>(ctx: &mut Ctx) {
  copy_selection_on_line(ctx, Direction::Backward);
}

fn select_all<Ctx: DefaultContext>(ctx: &mut Ctx) {
  let doc = ctx.editor().document_mut();
  let end = doc.text().len_chars();
  let _ = doc.set_selection(Selection::single(0, end));
}

#[derive(Clone, Copy)]
enum ExtendDirection {
  Above,
  Below,
}

fn extend_line_below<Ctx: DefaultContext>(ctx: &mut Ctx, count: usize) {
  extend_line_impl(ctx, ExtendDirection::Below, count.max(1));
}

fn extend_line_above<Ctx: DefaultContext>(ctx: &mut Ctx, count: usize) {
  extend_line_impl(ctx, ExtendDirection::Above, count.max(1));
}

fn extend_to_line_bounds<Ctx: DefaultContext>(ctx: &mut Ctx) {
  let doc = ctx.editor().document_mut();
  let text = doc.text();
  let selection = doc.selection().clone();

  let new_selection = selection.transform(|range| {
    let slice = text.slice(..);
    let (start_line, end_line) = range.line_range(slice);
    let start = text.line_to_char(start_line);
    let end = text.line_to_char((end_line + 1).min(text.len_lines()));

    Range::new(start, end).with_direction(range.direction())
  });

  let _ = doc.set_selection(new_selection);
}

fn shrink_to_line_bounds<Ctx: DefaultContext>(ctx: &mut Ctx) {
  let doc = ctx.editor().document_mut();
  let text = doc.text();
  let selection = doc.selection().clone();

  let new_selection = selection.transform(|range| {
    let slice = text.slice(..);
    let (start_line, end_line) = range.line_range(slice);

    if start_line == end_line {
      return range;
    }

    let mut start = text.line_to_char(start_line);
    let mut end = text.line_to_char((end_line + 1).min(text.len_lines()));

    if start != range.from() {
      start = text.line_to_char((start_line + 1).min(text.len_lines()));
    }

    if end != range.to() {
      end = text.line_to_char(end_line);
    }

    Range::new(start, end).with_direction(range.direction())
  });

  let _ = doc.set_selection(new_selection);
}

fn extend_line_impl<Ctx: DefaultContext>(ctx: &mut Ctx, extend: ExtendDirection, count: usize) {
  let doc = ctx.editor().document_mut();
  let text = doc.text();
  let selection = doc.selection().clone();

  let new_selection = selection.transform(|range| {
    let slice = text.slice(..);
    let (start_line, end_line) = range.line_range(slice);

    let start = text.line_to_char(start_line);
    let end = text.line_to_char((end_line + 1).min(text.len_lines()));

    let (anchor, head) = if range.from() == start && range.to() == end {
      match extend {
        ExtendDirection::Above => (end, text.line_to_char(start_line.saturating_sub(count))),
        ExtendDirection::Below => {
          (start, text.line_to_char((end_line + count + 1).min(text.len_lines())))
        },
      }
    } else {
      match extend {
        ExtendDirection::Above => {
          (end, text.line_to_char(start_line.saturating_sub(count - 1)))
        },
        ExtendDirection::Below => {
          (start, text.line_to_char((end_line + count).min(text.len_lines())))
        },
      }
    };

    Range::new(anchor, head)
  });

  let _ = doc.set_selection(new_selection);
}

fn undo<Ctx: DefaultContext>(ctx: &mut Ctx, count: usize) {
  let doc = ctx.editor().document_mut();
  let count = count.max(1);
  for _ in 0..count {
    if doc.undo().ok() != Some(true) {
      break;
    }
  }
}

fn redo<Ctx: DefaultContext>(ctx: &mut Ctx, count: usize) {
  let doc = ctx.editor().document_mut();
  let count = count.max(1);
  for _ in 0..count {
    if doc.redo().ok() != Some(true) {
      break;
    }
  }
}

fn earlier<Ctx: DefaultContext>(ctx: &mut Ctx, count: usize) {
  let doc = ctx.editor().document_mut();
  let count = count.max(1);
  for _ in 0..count {
    if doc.earlier(UndoKind::Steps(1)).ok() != Some(true) {
      break;
    }
  }
}

fn later<Ctx: DefaultContext>(ctx: &mut Ctx, count: usize) {
  let doc = ctx.editor().document_mut();
  let count = count.max(1);
  for _ in 0..count {
    if doc.later(UndoKind::Steps(1)).ok() != Some(true) {
      break;
    }
  }
}

fn indent<Ctx: DefaultContext>(ctx: &mut Ctx, count: usize) {
  let count = count.max(1);
  let doc = ctx.editor().document_mut();
  let text = doc.text();
  let selection = doc.selection().clone();
  let indent_str = doc.indent_style().as_str().repeat(count);
  let indent = Tendril::from(indent_str.as_str());

  let changes: Vec<_> = selection
    .line_ranges(text.slice(..))
    .flat_map(|(start_line, end_line)| start_line..=end_line)
    .filter_map(|line| {
      let is_blank = text.line(line).chars().all(|ch| ch.is_whitespace());
      if is_blank {
        return None;
      }
      let pos = text.line_to_char(line);
      Some((pos, pos, Some(indent.clone())))
    })
    .collect();

  let Ok(tx) = Transaction::change(doc.text(), changes.into_iter()) else {
    return;
  };
  let _ = doc.apply_transaction(&tx);

  if ctx.mode() == Mode::Select {
    ctx.set_mode(Mode::Normal);
  }
}

fn unindent<Ctx: DefaultContext>(ctx: &mut Ctx, count: usize) {
  let count = count.max(1);
  let tab_width = 4usize;
  let doc = ctx.editor().document_mut();
  let text = doc.text();
  let selection = doc.selection().clone();
  let indent_width = count * doc.indent_style().indent_width(tab_width);

  let changes: Vec<_> = selection
    .line_ranges(text.slice(..))
    .flat_map(|(start_line, end_line)| start_line..=end_line)
    .filter_map(|line_idx| {
      let line = text.line(line_idx);
      let mut width = 0usize;
      let mut pos = 0usize;

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

      if pos > 0 {
        let start = text.line_to_char(line_idx);
        Some((start, start + pos, None))
      } else {
        None
      }
    })
    .collect();

  let Ok(tx) = Transaction::change(doc.text(), changes.into_iter()) else {
    return;
  };
  let _ = doc.apply_transaction(&tx);

  if ctx.mode() == Mode::Select {
    ctx.set_mode(Mode::Normal);
  }
}

fn copy_selection_on_line<Ctx: DefaultContext>(ctx: &mut Ctx, direction: Direction) {
  let count = 1usize;
  let selection = {
    let doc = ctx.editor_ref().document();
    let text = doc.text().slice(..);
    let selection = doc.selection().clone();

    let mut ranges: SmallVec<[Range; 1]> =
      SmallVec::with_capacity(selection.ranges().len() * (count + 1));
    let mut cursor_ids: SmallVec<[CursorId; 1]> =
      SmallVec::with_capacity(selection.ranges().len() * (count + 1));

    ranges.extend_from_slice(selection.ranges());
    cursor_ids.extend_from_slice(selection.cursor_ids());

    let text_fmt = ctx.text_format();
    let mut annotations = ctx.text_annotations();

    for (_cursor_id, range) in selection.iter_with_ids() {
      let (head, anchor) = if range.anchor < range.head {
        (range.head.saturating_sub(1), range.anchor)
      } else {
        (range.head, range.anchor.saturating_sub(1))
      };

      let Some(head_pos) = visual_pos_at_char(text, &text_fmt, &mut annotations, head) else {
        continue;
      };
      let Some(anchor_pos) = visual_pos_at_char(text, &text_fmt, &mut annotations, anchor) else {
        continue;
      };

      let height =
        head_pos.row.max(anchor_pos.row).saturating_sub(head_pos.row.min(anchor_pos.row)) + 1;

      let mut sels = 0;
      let mut i = 0usize;
      while sels < count {
        let offset = (i + 1) * height;
        let anchor_row = match direction {
          Direction::Forward => anchor_pos.row + offset,
          Direction::Backward => anchor_pos.row.saturating_sub(offset),
          _ => anchor_pos.row,
        };
        let head_row = match direction {
          Direction::Forward => head_pos.row + offset,
          Direction::Backward => head_pos.row.saturating_sub(offset),
          _ => head_pos.row,
        };

        if anchor_row >= text.len_lines() || head_row >= text.len_lines() {
          break;
        }

        let Some(anchor_idx) = char_at_visual_pos(
          text,
          &text_fmt,
          &mut annotations,
          Position::new(anchor_row, anchor_pos.col),
        ) else {
          break;
        };
        let Some(head_idx) = char_at_visual_pos(
          text,
          &text_fmt,
          &mut annotations,
          Position::new(head_row, head_pos.col),
        ) else {
          break;
        };

        let anchor_ok =
          visual_pos_at_char(text, &text_fmt, &mut annotations, anchor_idx)
            .is_some_and(|pos| pos.col == anchor_pos.col);
        let head_ok =
          visual_pos_at_char(text, &text_fmt, &mut annotations, head_idx)
            .is_some_and(|pos| pos.col == head_pos.col);

        if anchor_ok && head_ok {
          ranges.push(Range::point(anchor_idx).put_cursor(text, head_idx, true));
          cursor_ids.push(CursorId::fresh());
          sels += 1;
        }

        if anchor_row == 0 && head_row == 0 {
          break;
        }

        i += 1;
      }
    }

    Selection::new_with_ids(ranges, cursor_ids).ok()
  };

  let Some(selection) = selection else {
    return;
  };

  let doc = ctx.editor().document_mut();
  let _ = doc.set_selection(selection);
}

fn open<Ctx: DefaultContext>(
  ctx: &mut Ctx,
  open: OpenDirection,
  comment_continuation: CommentContinuation,
) {
  // NOTE: count support isn't wired yet in the new context.
  let count = 1usize;
  ctx.set_mode(Mode::Insert);

  let doc = ctx.editor().document_mut();
  let contents = doc.text();
  let text = contents.slice(..);
  let selection = doc.selection().clone();
  let line_ending = doc.line_ending();

  let mut offs: usize = 0;
  let mut ranges = SmallVec::with_capacity(selection.len());

  // We don't have language config access yet, so comment continuation is disabled.
  let continue_comment_tokens: Option<&[String]> = match comment_continuation {
    CommentContinuation::Enabled => None,
    CommentContinuation::Disabled => None,
  };

  let tab_width = 4usize;
  let indent_heuristic = IndentationHeuristic::Simple;
  let loader = syntax_loader();
  let loader_language_count = loader.languages().len();

  let tx = Transaction::change_by_selection(contents, &selection, |range| {
    let curr_line_num = text.char_to_line(match open {
      OpenDirection::Below => {
        if range.is_empty() {
          range.cursor(text)
        } else {
          prev_grapheme_boundary(text, range.to())
        }
      },
      OpenDirection::Above => range.from(),
    });

    let next_new_line_num = match open {
      OpenDirection::Below => curr_line_num + 1,
      OpenDirection::Above => curr_line_num,
    };

    let above_next_new_line_num = next_new_line_num.saturating_sub(1);

    let continue_comment_token = continue_comment_tokens
      .and_then(|tokens| the_lib::comment::get_comment_token(text, tokens, curr_line_num));

    let (above_next_line_end_index, above_next_line_end_width) = if next_new_line_num == 0 {
      (0, 0)
    } else {
      (
        line_end_char_index(&text, above_next_new_line_num),
        line_ending.len_chars(),
      )
    };

    let line = text.line(curr_line_num);
    let syntax = if loader_language_count == 0 {
      None
    } else {
      match doc.syntax() {
        Some(syntax) if syntax.root_language().idx() < loader_language_count => Some(syntax),
        _ => None,
      }
    };
    let indent = match line.first_non_whitespace_char() {
      Some(pos) if continue_comment_token.is_some() => line.slice(..pos).to_string(),
      _ => indent::indent_for_newline(
        loader,
        syntax,
        &indent_heuristic,
        &doc.indent_style(),
        tab_width,
        text,
        above_next_new_line_num,
        above_next_line_end_index,
        curr_line_num,
      ),
    };

    let indent_len = indent.chars().count();
    let mut insert = String::with_capacity(1 + indent_len);

    if open == OpenDirection::Above && next_new_line_num == 0 {
      insert.push_str(&indent);
      if let Some(token) = continue_comment_token {
        insert.push_str(token);
        insert.push(' ');
      }
      insert.push_str(line_ending.as_str());
    } else {
      insert.push_str(line_ending.as_str());
      insert.push_str(&indent);
      if let Some(token) = continue_comment_token {
        insert.push_str(token);
        insert.push(' ');
      }
    }

    let insert = insert.repeat(count);

    let pos = offs + above_next_line_end_index + above_next_line_end_width;
    let comment_len = continue_comment_token
      .map(|token| token.len() + 1)
      .unwrap_or_default();
    for i in 0..count {
      ranges.push(Range::point(
        pos
          + (i * (line_ending.len_chars() + indent_len + comment_len))
          + indent_len
          + comment_len,
      ));
    }

    offs += insert.chars().count();

    (
      above_next_line_end_index,
      above_next_line_end_index,
      Some(insert.into()),
    )
  });

  let Ok(tx) = tx else {
    return;
  };

  let cursor_ids: SmallVec<[CursorId; 1]> =
    selection.cursor_ids().iter().copied().collect();
  let new_selection = if cursor_ids.len() == ranges.len() {
    Selection::new_with_ids(ranges, cursor_ids).unwrap_or_else(|_| selection)
  } else {
    Selection::new(ranges).unwrap_or_else(|_| selection)
  };

  let tx = tx.with_selection(new_selection);
  let _ = doc.apply_transaction(&tx);

  // Clamp selection to document bounds to avoid out-of-range cursor panics.
  let max = doc.text().len_chars();
  let clamped = doc
    .selection()
    .clone()
    .transform(|range| Range::new(range.anchor.min(max), range.head.min(max)));
  let _ = doc.set_selection(clamped);

  ctx.request_render();
}

pub fn command_from_name(name: &str) -> Option<Command> {
  match name {
    "move_char_left" => Some(Command::move_char_left(1)),
    "move_char_right" => Some(Command::move_char_right(1)),
    "move_char_up" => Some(Command::move_char_up(1)),
    "move_char_down" => Some(Command::move_char_down(1)),
    "move_visual_line_up" => Some(Command::move_visual_line_up(1)),
    "move_visual_line_down" => Some(Command::move_visual_line_down(1)),

    "extend_char_left" => Some(Command::extend_char_left(1)),
    "extend_char_right" => Some(Command::extend_char_right(1)),
    "extend_char_up" => Some(Command::extend_char_up(1)),
    "extend_char_down" => Some(Command::extend_char_down(1)),
    "extend_visual_line_up" => Some(Command::extend_visual_line_up(1)),
    "extend_visual_line_down" => Some(Command::extend_visual_line_down(1)),
    "extend_line_up" => Some(Command::extend_line_up(1)),
    "extend_line_down" => Some(Command::extend_line_down(1)),

    "move_next_word_start" => Some(Command::move_next_word_start(1)),
    "move_prev_word_start" => Some(Command::move_prev_word_start(1)),
    "move_next_word_end" => Some(Command::move_next_word_end(1)),
    "move_prev_word_end" => Some(Command::move_prev_word_end(1)),
    "move_next_long_word_start" => Some(Command::move_next_long_word_start(1)),
    "move_prev_long_word_start" => Some(Command::move_prev_long_word_start(1)),
    "move_next_long_word_end" => Some(Command::move_next_long_word_end(1)),
    "move_prev_long_word_end" => Some(Command::move_prev_long_word_end(1)),
    "move_next_sub_word_start" => Some(Command::move_next_sub_word_start(1)),
    "move_prev_sub_word_start" => Some(Command::move_prev_sub_word_start(1)),
    "move_next_sub_word_end" => Some(Command::move_next_sub_word_end(1)),
    "move_prev_sub_word_end" => Some(Command::move_prev_sub_word_end(1)),

    "extend_next_word_start" => Some(Command::extend_next_word_start(1)),
    "extend_prev_word_start" => Some(Command::extend_prev_word_start(1)),
    "extend_next_word_end" => Some(Command::extend_next_word_end(1)),
    "extend_prev_word_end" => Some(Command::extend_prev_word_end(1)),
    "extend_next_long_word_start" => Some(Command::extend_next_long_word_start(1)),
    "extend_prev_long_word_start" => Some(Command::extend_prev_long_word_start(1)),
    "extend_next_long_word_end" => Some(Command::extend_next_long_word_end(1)),
    "extend_prev_long_word_end" => Some(Command::extend_prev_long_word_end(1)),
    "extend_next_sub_word_start" => Some(Command::extend_next_sub_word_start(1)),
    "extend_prev_sub_word_start" => Some(Command::extend_prev_sub_word_start(1)),
    "extend_next_sub_word_end" => Some(Command::extend_next_sub_word_end(1)),
    "extend_prev_sub_word_end" => Some(Command::extend_prev_sub_word_end(1)),

    "extend_to_file_start" => Some(Command::extend_to_file_start()),
    "extend_to_file_end" => Some(Command::extend_to_file_end()),
    "extend_to_last_line" => Some(Command::extend_to_last_line()),
    "extend_to_column" => Some(Command::extend_to_column(1)),

    "delete_word_backward" => Some(Command::delete_word_backward(1)),
    "delete_word_forward" => Some(Command::delete_word_forward(1)),
    "kill_to_line_start" => Some(Command::kill_to_line_start()),
    "kill_to_line_end" => Some(Command::kill_to_line_end()),

    "delete_char_backward" => Some(Command::DeleteChar),
    "delete_char_forward" => Some(Command::delete_char_forward(1)),

    "insert_newline" => Some(Command::insert_newline()),
    "insert_tab" | "smart_tab" => Some(Command::insert_tab()),

    "goto_line_start" => Some(Command::goto_line_start()),
    "extend_to_line_start" => Some(Command::extend_to_line_start()),
    "goto_line_end" => Some(Command::goto_line_end()),
    "extend_to_line_end" => Some(Command::extend_to_line_end()),

    "page_up" => Some(Command::page_up()),
    "page_down" => Some(Command::page_down()),

    "find_next_char" => Some(Command::find_next_char()),
    "find_till_char" => Some(Command::find_till_char()),
    "find_prev_char" => Some(Command::find_prev_char()),
    "till_prev_char" => Some(Command::till_prev_char()),
    "extend_next_char" => Some(Command::extend_next_char()),
    "extend_till_char" => Some(Command::extend_till_char()),
    "extend_prev_char" => Some(Command::extend_prev_char()),
    "extend_till_prev_char" => Some(Command::extend_till_prev_char()),

    "move_parent_node_end" => Some(Command::move_parent_node_end()),
    "extend_parent_node_end" => Some(Command::extend_parent_node_end()),
    "move_parent_node_start" => Some(Command::move_parent_node_start()),
    "extend_parent_node_start" => Some(Command::extend_parent_node_start()),

    "delete_selection" => Some(Command::delete_selection()),
    "delete_selection_noyank" => Some(Command::delete_selection_noyank()),
    "change_selection" => Some(Command::change_selection()),
    "change_selection_noyank" => Some(Command::change_selection_noyank()),
    "replace" => Some(Command::replace()),
    "replace_with_yanked" => Some(Command::replace_with_yanked()),
    "repeat_last_motion" => Some(Command::repeat_last_motion()),
    "switch_case" => Some(Command::switch_case()),
    "switch_to_uppercase" => Some(Command::switch_to_uppercase()),
    "switch_to_lowercase" => Some(Command::switch_to_lowercase()),
    "insert_at_line_start" => Some(Command::insert_at_line_start()),
    "insert_at_line_end" => Some(Command::insert_at_line_end()),
    "append_mode" => Some(Command::append_mode()),
    "open_below" => Some(Command::open_below()),
    "open_above" => Some(Command::open_above()),
    "commit_undo_checkpoint" => Some(Command::commit_undo_checkpoint()),
    "yank" => Some(Command::yank()),
    "paste_after" => Some(Command::paste_after()),
    "paste_before" => Some(Command::paste_before()),
    "copy_selection_on_next_line" => Some(Command::copy_selection_on_next_line()),
    "copy_selection_on_prev_line" => Some(Command::copy_selection_on_prev_line()),
    "select_all" => Some(Command::select_all()),
    "extend_line_below" => Some(Command::extend_line_below(1)),
    "extend_line_above" => Some(Command::extend_line_above(1)),
    "extend_to_line_bounds" => Some(Command::extend_to_line_bounds()),
    "shrink_to_line_bounds" => Some(Command::shrink_to_line_bounds()),
    "undo" => Some(Command::undo(1)),
    "redo" => Some(Command::redo(1)),
    "earlier" => Some(Command::earlier(1)),
    "later" => Some(Command::later(1)),
    "indent" => Some(Command::indent(1)),
    "unindent" => Some(Command::unindent(1)),

    _ => None,
  }
}
