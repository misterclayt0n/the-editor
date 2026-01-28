//! Dispatch wiring for the terminal client.

use the_default::{
  DefaultContext,
  DefaultDispatchStatic,
};
pub use the_default::{
  Key,
  KeyEvent,
  Modifiers,
};

pub fn build_dispatch<Ctx>() -> DefaultDispatchStatic<Ctx>
where
  Ctx: DefaultContext,
{
  the_config::build_dispatch::<Ctx>()
}
