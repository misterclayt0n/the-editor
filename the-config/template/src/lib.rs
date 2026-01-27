use the_default::{
  DefaultContext,
  DefaultPlugin,
};
use the_dispatch::{
  DispatchPlugin,
  key_hook,
  editor::{
    Command,
    KeyOutcome,
    KeyPipelineDispatch,
    KeyPipelineApi,
    KeyEvent,
    Direction,
  },
};

/// Build the dispatch pipeline for the editor.
///
/// Replace this with your own bindings/logic as needed.
pub fn build_dispatch<Ctx>() -> impl DispatchPlugin<Ctx, Command>
where
  Ctx: DefaultContext,
{
  DefaultPlugin::new()
}

/// Build the key pipeline (pre/on/post hooks).
///
/// Return `KeyOutcome::Command(...)` to override the default keymap.
pub fn build_key_pipeline<Ctx>() -> impl KeyPipelineApi<Ctx> {
  KeyPipelineDispatch::new()
    .with_pre(key_hook!(|_ctx: &mut Ctx, key: KeyEvent| match key.key {
      the_dispatch::editor::Key::Char('j') => KeyOutcome::Command(Command::Move(Direction::Down)),
      _ => KeyOutcome::Continue,
    }))
    .with_on(key_hook!(|_, _| KeyOutcome::Continue))
    .with_post(key_hook!(|_, _| KeyOutcome::Continue))
}
