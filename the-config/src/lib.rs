//! Wrapper crate that re-exports the user config crate.
//!
//! The actual config lives in the user config directory (e.g.
//! ~/.config/the-editor). This crate exists so the workspace can depend on a
//! stable path.

pub use the_config_user::*;

pub mod defaults {
  pub fn build_file_picker_config() -> the_default::FilePickerConfig {
    the_default::FilePickerConfig::default()
  }
}
