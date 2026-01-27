use std::path::Path;

use smallvec::SmallVec;
use the_dispatch::define;
use the_lib::{
  Tendril,
  editor::Editor,
  movement::{
    Direction as MoveDir,
    Movement,
    move_horizontally,
    move_vertically,
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
use the_core::grapheme::prev_grapheme_boundary;

use crate::{
  Command,
  Direction,
  KeyEvent,
  command_for_key,
};

pub trait DefaultContext {
  fn editor(&mut self) -> &mut Editor;
  fn file_path(&self) -> Option<&Path>;
  fn request_render(&mut self);
  fn request_quit(&mut self);
}

define! {
  Default {
    insert_char: char,
    delete_char: (),
    move_cursor: Direction,
    add_cursor: Direction,
    save: (),
    quit: (),
  }
}

pub fn build_dispatch<Ctx>() -> DefaultDispatch<Ctx,
  fn(&mut Ctx, char),
  fn(&mut Ctx, ()),
  fn(&mut Ctx, Direction),
  fn(&mut Ctx, Direction),
  fn(&mut Ctx, ()),
  fn(&mut Ctx, ()),
>
where
  Ctx: DefaultContext,
{
  DefaultDispatch::new()
    .with_insert_char(insert_char::<Ctx> as fn(&mut Ctx, char))
    .with_delete_char(delete_char::<Ctx> as fn(&mut Ctx, ()))
    .with_move_cursor(move_cursor::<Ctx> as fn(&mut Ctx, Direction))
    .with_add_cursor(add_cursor::<Ctx> as fn(&mut Ctx, Direction))
    .with_save(save::<Ctx> as fn(&mut Ctx, ()))
    .with_quit(quit::<Ctx> as fn(&mut Ctx, ()))
}

pub fn handle_command<Ctx, D>(dispatch: &D, ctx: &mut Ctx, command: Command)
where
  Ctx: DefaultContext,
  D: DefaultApi<Ctx>,
{
  match command {
    Command::InsertChar(ch) => dispatch.insert_char(ctx, ch),
    Command::DeleteChar => dispatch.delete_char(ctx, ()),
    Command::Move(dir) => dispatch.move_cursor(ctx, dir),
    Command::AddCursor(dir) => dispatch.add_cursor(ctx, dir),
    Command::Save => dispatch.save(ctx, ()),
    Command::Quit => dispatch.quit(ctx, ()),
  }
}

pub fn handle_key<Ctx, D>(dispatch: &D, ctx: &mut Ctx, key: KeyEvent)
where
  Ctx: DefaultContext,
  D: DefaultApi<Ctx>,
{
  if let Some(command) = command_for_key(key) {
    handle_command(dispatch, ctx, command);
  }
}

fn insert_char<Ctx: DefaultContext>(ctx: &mut Ctx, ch: char) {
  let doc = ctx.editor().document_mut();
  let selection = doc.selection().clone();

  let mut text = Tendril::new();
  text.push(ch);

  let tx = Transaction::change_by_selection(doc.text(), &selection, |range| {
    if range.is_empty() {
      (range.head, range.head, Some(text.clone()))
    } else {
      (range.from(), range.to(), Some(text.clone()))
    }
  });

  let Ok(tx) = tx else {
    return;
  };

  if doc.apply_transaction(&tx).is_ok() {
    let _ = doc.commit();
    ctx.request_render();
  }
}

fn delete_char<Ctx: DefaultContext>(ctx: &mut Ctx, _unit: ()) {
  let doc = ctx.editor().document_mut();
  let selection = doc.selection().clone();
  let slice = doc.text().slice(..);

  let mut deletions = Vec::new();
  for range in selection.ranges() {
    if !range.is_empty() {
      deletions.push((range.from(), range.to()));
      continue;
    }

    let cursor = range.cursor(slice);
    if cursor == 0 {
      continue;
    }
    let from = prev_grapheme_boundary(slice, cursor);
    deletions.push((from, cursor));
  }

  if deletions.is_empty() {
    return;
  }

  let Ok(tx) = Transaction::delete(doc.text(), deletions) else {
    return;
  };

  if doc.apply_transaction(&tx).is_ok() {
    let _ = doc.commit();
    ctx.request_render();
  }
}

fn move_cursor<Ctx: DefaultContext>(ctx: &mut Ctx, direction: Direction) {
  let mut changed = false;

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
          move_vertically(slice, *range, dir, 1, Movement::Move, &text_fmt, &mut annotations)
        } else {
          move_horizontally(slice, *range, dir, 1, Movement::Move, &text_fmt, &mut annotations)
        };
        ranges.push(new_range);
        ids.push(cursor_id);
      }

      Selection::new_with_ids(ranges, ids).ok()
    };

    if let Some(selection) = new_selection {
      let _ = doc.set_selection(selection);
      changed = true;
    }
  }

  if changed {
    ctx.request_render();
  }
}

fn add_cursor<Ctx: DefaultContext>(ctx: &mut Ctx, direction: Direction) {
  let dir = match direction {
    Direction::Up => MoveDir::Backward,
    Direction::Down => MoveDir::Forward,
    _ => return,
  };
  let mut changed = false;

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
        let moved = move_vertically(slice, *range, dir, 1, Movement::Move, &text_fmt, &mut annotations);
        ranges.push(moved);
        ids.push(CursorId::fresh());
      }

      Selection::new_with_ids(ranges, ids).ok()
    };

    if let Some(selection) = new_selection {
      let _ = doc.set_selection(selection);
      changed = true;
    }
  }

  if changed {
    ctx.request_render();
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
