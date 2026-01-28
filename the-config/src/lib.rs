use the_default::{
  DefaultApi,
  DefaultContext,
  Key,
  KeyEvent,
  KeyOutcome,
  KeyPipelineApi,
  build_dispatch as default_dispatch,
  default_key_pipeline,
};
use the_lib::command::{
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
}

/// Build the key pipeline (pre/on/post hooks).
///
/// Return `KeyOutcome::Command(...)` to override the default keymap.
pub fn build_key_pipeline<Ctx>() -> impl KeyPipelineApi<Ctx> {
  default_key_pipeline::<Ctx>()
}

// Example: override the `j` key to move down.
#[allow(dead_code)]
fn example_override<Ctx: DefaultContext>(_ctx: &mut Ctx, key: KeyEvent) -> KeyOutcome {
  match key.key {
    Key::Char('j') => KeyOutcome::Command(Command::Move(Direction::Down)),
    _ => KeyOutcome::Continue,
  }
}
