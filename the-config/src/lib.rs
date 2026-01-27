use the_default::{
  DefaultContext,
  DefaultPlugin,
};
use the_dispatch::{
  DispatchPlugin,
  editor::{
    Command,
    KeyOutcome,
    KeyPipelineApi,
    KeyEvent,
    default_key_pipeline,
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
  default_key_pipeline::<Ctx>()
}

// Example: override the `j` key to move down.
#[allow(dead_code)]
fn example_override<Ctx: DefaultContext>(_ctx: &mut Ctx, key: KeyEvent) -> KeyOutcome {
  match key.key {
    the_dispatch::editor::Key::Char('j') => KeyOutcome::Command(Command::Move(the_dispatch::editor::Direction::Down)),
    _ => KeyOutcome::Continue,
  }
}
