//! Dispatch wiring for the terminal client.

pub use the_default::{
  Key,
  KeyEvent,
  Modifiers,
  handle_key,
};

use the_default::DefaultPlugin;

/// Concrete dispatch type for the application.
pub type AppDispatch = DefaultPlugin;

/// Build the default dispatch plugin.
pub fn build_dispatch() -> AppDispatch {
  DefaultPlugin::new()
}
