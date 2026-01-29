#![allow(unused_imports)]

use std::path::Path;

use smallvec::SmallVec;
use std::borrow::Cow;

use the_core::grapheme::{
  nth_prev_grapheme_boundary,
  prev_grapheme_boundary,
};
use the_dispatch::define;
use the_lib::{
  Tendril,
  auto_pairs::{
    self,
    AutoPairs,
  },
  editor::Editor,
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
  render::{
    text_annotations::TextAnnotations,
    text_format::TextFormat,
  },
  selection::{
    CursorId,
    Selection,
  },
  transaction::Transaction,
};
pub use the_lib::{
  command::{
    Command,
    Direction,
    Motion,
    WordMotion,
  },
  input::{
    Key,
    KeyEvent,
    KeyOutcome,
    Modifiers,
  },
};

use crate::{
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
    delete_char: (),
    move_cursor: Direction,
    add_cursor: Direction,
    motion: Motion,
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
  fn(&mut Ctx, Direction),
  fn(&mut Ctx, Direction),
  fn(&mut Ctx, Motion),
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
    .with_delete_char(delete_char::<Ctx> as fn(&mut Ctx, ()))
    .with_move_cursor(move_cursor::<Ctx> as fn(&mut Ctx, Direction))
    .with_add_cursor(add_cursor::<Ctx> as fn(&mut Ctx, Direction))
    .with_motion(motion::<Ctx> as fn(&mut Ctx, Motion))
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

fn on_keypress<Ctx: DefaultContext>(ctx: &mut Ctx, key: KeyEvent) {
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
    Command::DeleteChar => ctx.dispatch().delete_char(ctx, ()),
    Command::Move(dir) => ctx.dispatch().move_cursor(ctx, dir),
    Command::AddCursor(dir) => ctx.dispatch().add_cursor(ctx, dir),
    Command::Motion(motion) => ctx.dispatch().motion(ctx, motion),
    Command::Save => ctx.dispatch().save(ctx, ()),
    Command::Quit => ctx.dispatch().quit(ctx, ()),
  }

  ctx.dispatch().post_on_action(ctx, ());
}

fn post_on_action<Ctx: DefaultContext>(ctx: &mut Ctx, _unit: ()) {
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
    if !range.is_empty() {
      return (range.from(), range.to());
    }

    let pos = range.cursor(slice);
    if pos == 0 {
      return (pos, pos);
    }

    let line_start_pos = slice.line_to_char(range.cursor_line(slice));
    let fragment: Cow<'_, str> = Cow::from(slice.slice(line_start_pos..pos));

    if !fragment.is_empty()
      && fragment.chars().all(|ch| ch == ' ' || ch == '\t')
    {
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

fn move_cursor<Ctx: DefaultContext>(ctx: &mut Ctx, direction: Direction) {
  {
    let editor = ctx.editor();
    let doc = editor.document_mut();
    let selection = doc.selection().clone();

    let (dir, vertical) = match direction {
      Direction::Left => (MoveDir::Backward, false),
      Direction::Right => (MoveDir::Forward, false),
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

    _ => None,
  }
}
