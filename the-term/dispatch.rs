//! Dispatch wiring for the terminal client.

use the_default::{
  Command,
  DefaultContext,
};
use the_dispatch::DispatchPlugin;

pub use the_default::{
  Key,
  KeyEvent,
  Modifiers,
  handle_key,
};

pub fn build_dispatch<Ctx>() -> impl DispatchPlugin<Ctx, Command>
where
  Ctx: DefaultContext,
{
  the_config::build_dispatch::<Ctx>()
}
