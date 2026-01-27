use std::path::Path;

use the_dispatch::{
  DispatchPlugin,
  DispatchResult,
};
use the_lib::{
  movement::{
    Direction as MoveDir,
    Movement,
    move_horizontally,
    move_vertically,
  },
  editor::Editor,
  render::{
    text_annotations::TextAnnotations,
    text_format::TextFormat,
  },
  transaction::Transaction,
};

use crate::{
  Command,
  Direction,
  KeyEvent,
  keymap::command_for_key,
};

/// Context contract required by the default plugin.
pub trait DefaultContext {
  fn editor(&mut self) -> &mut Editor;
  fn file_path(&self) -> Option<&Path>;
  fn request_render(&mut self);
  fn request_quit(&mut self);
}

/// Default dispatch behavior (commands + editing semantics).
#[derive(Debug, Default, Clone)]
pub struct DefaultPlugin;

impl DefaultPlugin {
  pub fn new() -> Self {
    Self
  }
}

impl<Ctx> DispatchPlugin<Ctx, Command> for DefaultPlugin
where
  Ctx: DefaultContext,
{
  fn dispatch(&mut self, ctx: &mut Ctx, input: Command) -> DispatchResult<()> {
    match input {
      Command::InsertChar(c) => {
        insert_char(ctx, c);
        DispatchResult::Handled
      },
      Command::DeleteChar => {
        delete_char(ctx);
        DispatchResult::Handled
      },
      Command::Move(dir) => {
        move_cursor(ctx, dir);
        DispatchResult::Handled
      },
      Command::AddCursor(dir) => {
        add_cursor(ctx, dir);
        DispatchResult::Handled
      },
      Command::Save => {
        save(ctx);
        DispatchResult::Handled
      },
      Command::Quit => {
        ctx.request_quit();
        DispatchResult::Handled
      },
    }
  }
}

/// Convenience helper for clients that want a tiny input pipeline.
pub fn handle_key<P, Ctx>(plugin: &mut P, ctx: &mut Ctx, key: KeyEvent)
where
  P: DispatchPlugin<Ctx, Command>,
{
  if let Some(command) = command_for_key(key) {
    let _ = plugin.dispatch(ctx, command);
  }
}

/// Dispatch a command directly through the plugin.
pub fn handle_command<P, Ctx>(plugin: &mut P, ctx: &mut Ctx, command: Command)
where
  P: DispatchPlugin<Ctx, Command>,
{
  let _ = plugin.dispatch(ctx, command);
}

fn insert_char<Ctx: DefaultContext>(ctx: &mut Ctx, c: char) {
  let doc = ctx.editor().document_mut();
  let text = doc.text();

  let changes: Vec<_> = doc
    .selection()
    .iter()
    .map(|range: &the_lib::selection::Range| {
      let pos = range.cursor(text.slice(..));
      (pos, pos, Some(c.to_string().into()))
    })
    .collect();

  if let Ok(tx) = Transaction::change(text, changes) {
    let _ = doc.apply_transaction(&tx);
    ctx.request_render();
  }
}

fn delete_char<Ctx: DefaultContext>(ctx: &mut Ctx) {
  let doc = ctx.editor().document_mut();
  let text = doc.text();

  let changes: Vec<_> = doc
    .selection()
    .iter()
    .filter_map(|range: &the_lib::selection::Range| {
      let pos = range.cursor(text.slice(..));
      if pos > 0 { Some((pos - 1, pos, None)) } else { None }
    })
    .collect();

  if !changes.is_empty() {
    if let Ok(tx) = Transaction::change(text, changes) {
      let _ = doc.apply_transaction(&tx);
      ctx.request_render();
    }
  }
}

fn move_cursor<Ctx: DefaultContext>(ctx: &mut Ctx, dir: Direction) {
  let doc = ctx.editor().document_mut();
  let text_fmt = TextFormat::default();

  let selection = {
    let slice = doc.text().slice(..);
    let mut annotations = TextAnnotations::default();
    doc.selection().clone().transform(|range| match dir {
      Direction::Left => move_horizontally(
        slice,
        range,
        MoveDir::Backward,
        1,
        Movement::Move,
        &text_fmt,
        &mut annotations,
      ),
      Direction::Right => move_horizontally(
        slice,
        range,
        MoveDir::Forward,
        1,
        Movement::Move,
        &text_fmt,
        &mut annotations,
      ),
      Direction::Up => move_vertically(
        slice,
        range,
        MoveDir::Backward,
        1,
        Movement::Move,
        &text_fmt,
        &mut annotations,
      ),
      Direction::Down => move_vertically(
        slice,
        range,
        MoveDir::Forward,
        1,
        Movement::Move,
        &text_fmt,
        &mut annotations,
      ),
    })
  };

  let _ = doc.set_selection(selection);
  ctx.request_render();
}

fn add_cursor<Ctx: DefaultContext>(ctx: &mut Ctx, dir: Direction) {
  let doc = ctx.editor().document_mut();
  let text_fmt = TextFormat::default();

  let (primary_cursor, new_cursor, new_range) = {
    let slice = doc.text().slice(..);
    let mut annotations = TextAnnotations::default();
    let primary = doc.selection().ranges()[0];

    let new_range = match dir {
      Direction::Up => move_vertically(
        slice,
        primary,
        MoveDir::Backward,
        1,
        Movement::Move,
        &text_fmt,
        &mut annotations,
      ),
      Direction::Down => move_vertically(
        slice,
        primary,
        MoveDir::Forward,
        1,
        Movement::Move,
        &text_fmt,
        &mut annotations,
      ),
      _ => return,
    };

    (primary.cursor(slice), new_range.cursor(slice), new_range)
  };

  if new_cursor != primary_cursor {
    let selection = doc.selection().clone().push(new_range);
    let _ = doc.set_selection(selection);
    ctx.request_render();
  }
}

fn save<Ctx: DefaultContext>(ctx: &mut Ctx) {
  let Some(path) = ctx.file_path().map(|path| path.to_owned()) else {
    return;
  };
  let text = ctx.editor().document().text().to_string();
  if let Err(e) = std::fs::write(path, text) {
    eprintln!("Failed to save: {e}");
  }
}
