use the_default::{
  DefaultApi,
  DefaultContext,
  EditorPreset,
  Key,
  KeyEvent,
  Keymaps,
  build_dispatch as default_dispatch,
  default_pre_on_keypress,
  default_editor_preset,
};
use the_default::{
  Command,
  Direction,
};

/// Build the dispatch pipeline for the editor.
///
/// Replace this with your own bindings/logic as needed.
pub fn build_dispatch<Ctx>() -> impl DefaultApi<Ctx>
where
  Ctx: DefaultContext,
{
  default_dispatch::<Ctx>().with_pre_on_keypress(pre_on_keypress::<Ctx>)
}

// pre -> on -> post
// pre -> default_keymap -> post

fn pre_on_keypress<Ctx: DefaultContext>(ctx: &mut Ctx, key: KeyEvent) {
  if key.modifiers.ctrl() && matches!(key.key, Key::Char('j')) {
    ctx
      .dispatch()
      .post_on_keypress(ctx, Command::Move(Direction::Down));
    println!("Custom Ctrl+J binding triggered!");
    return;
  }

  default_pre_on_keypress(ctx, key);
}

/// Build the default keymaps.
///
/// Replace this to provide your own layout.
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
