use the_default::{
  DefaultApi,
  DefaultContext,
  Key,
  KeyEvent,
  KeyOutcome,
  KeyPipelineApi,
  KeyPipelineDispatch,
  key_hook,
  build_dispatch as default_dispatch,
};
use the_lib::command::{
  Command,
  Direction,
};
};

/// Build the dispatch pipeline for the editor.
///
/// Replace this with your own bindings/logic as needed.
pub fn build_dispatch<Ctx>() -> impl DefaultApi<Ctx>
where
  Ctx: DefaultContext,
{
  default_dispatch::<Ctx>()
}

/// Build the key pipeline (pre/on/post hooks).
///
/// Return `KeyOutcome::Command(...)` to override the default keymap.
pub fn build_key_pipeline<Ctx>() -> impl KeyPipelineApi<Ctx> {
  KeyPipelineDispatch::new()
    .with_pre(key_hook!(|_ctx: &mut Ctx, key: KeyEvent| match key.key {
      Key::Char('j') => KeyOutcome::Command(Command::Move(Direction::Down)),
      _ => KeyOutcome::Continue,
    }))
    .with_on(key_hook!(|_, _| KeyOutcome::Continue))
    .with_post(key_hook!(|_, _| KeyOutcome::Continue))
}
