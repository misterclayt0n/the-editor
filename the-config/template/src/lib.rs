use the_default::{
  DefaultApi,
  CommandPaletteLayout,
  DefaultContext,
  EditorPreset,
  Key,
  KeyEvent,
  Keymaps,
  build_dispatch as default_dispatch,
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
  default_dispatch::<Ctx>()
    .with_pre_on_keypress(pre_on_keypress::<Ctx>)
    .with_pre_render(pre_render::<Ctx>)
}

fn pre_on_keypress<Ctx: DefaultContext>(ctx: &mut Ctx, key: KeyEvent) {
  if key.modifiers.ctrl() && matches!(key.key, Key::Char('j')) {
    ctx.dispatch().post_on_keypress(ctx, Command::Move(Direction::Down));
    return;
  }

  ctx.dispatch().on_keypress(ctx, key);
}

fn pre_render<Ctx: DefaultContext>(ctx: &mut Ctx, _unit: ()) {
  ctx.command_palette_style_mut().layout = CommandPaletteLayout::Floating;
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
