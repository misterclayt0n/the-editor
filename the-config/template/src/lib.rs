use the_default::{
  CommandPaletteLayout,
  DefaultContext,
  DefaultDispatchStatic,
  EditorAssembly,
  Key,
  KeyEvent,
  Keymaps,
  build_dispatch as default_dispatch,
};
use the_default::{
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
    .with_pre_on_keypress(pre_on_keypress::<Ctx> as fn(&mut Ctx, KeyEvent))
    .with_pre_render(pre_render::<Ctx> as fn(&mut Ctx, ()))
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

pub fn build_editor_assembly<Ctx>() -> EditorAssembly<Ctx>
where
  Ctx: DefaultContext,
{
  EditorAssembly::new(build_dispatch::<Ctx>(), build_keymaps())
}
