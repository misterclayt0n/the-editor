//! Dispatch wiring for the terminal client.

use the_default::{
  DefaultApi,
  DefaultContext,
};
pub use the_default::{
  Key,
  KeyEvent,
  Modifiers,
};

pub fn build_dispatch<Ctx>() -> impl DefaultApi<Ctx>
where
  Ctx: DefaultContext,
{
  the_default::build_dispatch::<Ctx>()
}
