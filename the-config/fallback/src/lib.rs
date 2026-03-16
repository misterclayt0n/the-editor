use the_default::{
  Command,
  DefaultApi,
  DefaultContext,
  Direction,
  EditorPreset,
  Key,
  KeyEvent,
  Keymaps,
  build_dispatch as default_dispatch,
  default_editor_preset,
  default_pre_on_keypress,
};

/// Build the dispatch pipeline for the editor.
///
/// This is a fallback config used when no user config is installed.
pub fn build_dispatch<Ctx>() -> impl DefaultApi<Ctx>
where
  Ctx: DefaultContext,
{
  default_dispatch::<Ctx>().with_pre_on_keypress(pre_on_keypress::<Ctx>)
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

pub fn build_editor_preset<Ctx>() -> EditorPreset<Ctx, impl DefaultApi<Ctx>>
where
  Ctx: DefaultContext,
{
  default_editor_preset::<Ctx>()
    .with_dispatch(build_dispatch::<Ctx>())
    .with_keymaps(build_keymaps())
}
