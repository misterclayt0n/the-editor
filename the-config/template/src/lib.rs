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
  the_default::builtin_keymaps()
}

pub fn build_editor_preset<Ctx>() -> EditorPreset<Ctx, impl DefaultApi<Ctx>>
where
  Ctx: DefaultContext,
{
  default_editor_preset::<Ctx>()
    // Simple startup defaults can live directly on the preset.
    // .with_defaults(
    //   the_default::ConfigDefaults::new()
    //     .theme("onedark")
    //     .line_numbers(the_default::LineNumberMode::Relative)
    //     .cursor_shapes(the_default::CursorShapes::new(
    //       the_default::CursorKind::Bar,
    //       the_default::CursorKind::Block,
    //       the_default::CursorKind::Underline,
    //     ))
    //     .file_picker(the_default::FilePickerConfig {
    //       hidden: false,
    //       ..Default::default()
    //     })
    //     .term(the_default::TermDefaults::new().mouse(false)),
    // )
    .with_dispatch(build_dispatch::<Ctx>())
    .with_keymaps(build_keymaps())
}
