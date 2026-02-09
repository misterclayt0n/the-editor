use the_default::{
  Command,
  DefaultContext,
  DefaultDispatchStatic,
  Direction,
  Key,
  KeyEvent,
  Keymaps,
  build_dispatch as default_dispatch,
  default_pre_on_keypress,
};

/// Build the dispatch pipeline for the editor.
///
/// This is a fallback config used when no user config is installed.
pub fn build_dispatch<Ctx>() -> DefaultDispatchStatic<Ctx>
where
  Ctx: DefaultContext,
{
  default_dispatch::<Ctx>().with_pre_on_keypress(pre_on_keypress::<Ctx> as fn(&mut Ctx, KeyEvent))
}

fn pre_on_keypress<Ctx: DefaultContext>(ctx: &mut Ctx, key: KeyEvent) {
  if key.modifiers.ctrl() && matches!(key.key, Key::Char('j')) {
    ctx
      .dispatch()
      .post_on_keypress(ctx, Command::Move(Direction::Down));
    return;
  }

  default_pre_on_keypress(ctx, key);
}

/// Build the default keymaps.
pub fn build_keymaps() -> Keymaps {
  Keymaps::default()
}
