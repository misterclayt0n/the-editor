use the_default::{
  DefaultContext,
  DefaultDispatchStatic,
  Key,
  KeyEvent,
  Keymaps,
  build_dispatch as default_dispatch,
};
use the_lib::command::{
  Command,
  Direction,
};

/// Build the dispatch pipeline for the editor.
///
/// Replace this with your own bindings/logic as needed.
pub fn build_dispatch<Ctx>() -> DefaultDispatchStatic<Ctx>
where
  Ctx: DefaultContext,
{
  default_dispatch::<Ctx>()
    .with_pre_on_keypress(pre_on_keypress::<Ctx>)
}

// pre -> on -> post
// pre -> default_keymap -> post

fn pre_on_keypress<Ctx: DefaultContext>(ctx: &mut Ctx, key: KeyEvent) {
  if key.modifiers.ctrl() && matches!(key.key, Key::Char('j')) {
    ctx.dispatch().post_on_keypress(ctx, Command::Move(Direction::Down));
    println!("Custom Ctrl+J binding triggered!");
    return;
  }

  ctx.dispatch().on_keypress(ctx, key);
}

/// Build the default keymaps.
///
/// Replace this to provide your own layout.
pub fn build_keymaps() -> Keymaps {
  Keymaps::default()
}
