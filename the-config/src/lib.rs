//! Wrapper crate that re-exports the user config crate.
//!
//! The actual config lives in the user config directory (e.g.
//! ~/.config/the-editor). This crate exists so the workspace can depend on a
//! stable path.

pub use the_config_user::*;

/// Build the current config as an assembled editor surface.
///
/// This bridges the existing config entrypoints (`build_dispatch` and
/// `build_keymaps`) into the initial compile-time assembly model.
pub fn build_editor_assembly<Ctx>() -> the_default::EditorAssembly<Ctx>
where
  Ctx: the_default::DefaultContext,
{
  the_default::EditorAssembly::new(build_dispatch::<Ctx>(), build_keymaps())
}

pub mod defaults {
  pub fn build_file_picker_config() -> the_default::FilePickerConfig {
    the_default::FilePickerConfig::default()
  }
}
