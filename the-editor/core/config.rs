use std::{
  fs,
  io::Error as IOError,
};

use serde::Deserialize;
use toml::de::Error as TomlError;

use crate::{
  core::{
    // Per-file .editorconfig is handled in core/editor_config.rs
    // The top-level application EditorConfig lives in crate::editor
    syntax::{
      Loader,
      LoaderError,
      config::Configuration,
    },
  },
  editor::EditorConfig,
  keymap::Keymaps,
};

/// Language configuration based on built-in languages.toml.
pub fn default_lang_config() -> Configuration {
  the_editor_loader::config::default_lang_config()
    .try_into()
    .expect("Could not deserialize built-in languages.toml")
}

/// Language configuration loader based on built-in languages.toml.
pub fn default_lang_loader() -> Loader {
  Loader::new(default_lang_config()).expect("Could not compile loader for default config")
}

#[derive(Debug)]
pub enum LanguageLoaderError {
  DeserializeError(toml::de::Error),
  LoaderError(LoaderError),
}

impl std::fmt::Display for LanguageLoaderError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::DeserializeError(err) => write!(f, "Failed to parse language config: {err}"),
      Self::LoaderError(err) => write!(f, "Failed to compile language config: {err}"),
    }
  }
}

impl std::error::Error for LanguageLoaderError {}

/// Language configuration based on user configured languages.toml.
pub fn user_lang_config() -> Result<Configuration, toml::de::Error> {
  the_editor_loader::config::user_lang_config()?.try_into()
}

/// Language configuration loader based on user configured languages.toml.
pub fn user_lang_loader() -> Result<Loader, LanguageLoaderError> {
  let config: Configuration = the_editor_loader::config::user_lang_config()
    .map_err(LanguageLoaderError::DeserializeError)?
    .try_into()
    .map_err(LanguageLoaderError::DeserializeError)?;

  Loader::new(config).map_err(LanguageLoaderError::LoaderError)
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case", default, deny_unknown_fields)]
#[derive(Default)]
pub struct Config {
  pub theme:  Option<String>,
  #[serde(skip)]
  pub keymap: Keymaps,
  pub editor: EditorConfig,
}

// #[derive(Debug, Clone, PartialEq, Deserialize)]
// #[serde(deny_unknown_fields)]
// pub struct ConfigRaw {
//   pub theme:  Option<String>,
//   pub keys:   Option<Keymaps>,
//   pub editor: Option<toml::Value>,
// }

#[derive(Debug)]
pub enum ConfigLoadError {
  BadConfig(TomlError),
  Error(IOError),
}

impl Config {
  /// Load user config from ~/.config/the-editor/config.toml using loader's
  /// config_dir.
  pub fn load_user() -> Result<Config, ConfigLoadError> {
    let path = the_editor_loader::config_file();
    match fs::read_to_string(&path) {
      Ok(contents) => toml::from_str::<Config>(&contents).map_err(ConfigLoadError::BadConfig),
      Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Config::default()),
      Err(e) => Err(ConfigLoadError::Error(e)),
    }
  }

  // pub fn load(
  //   global: Result<String, ConfigLoadError>,
  //   local: Result<String, ConfigLoadError>,
  // ) -> Result<Config, ConfigLoadError> {
  //   let global_config: Result<ConfigRaw, ConfigLoadError> =
  //     global.and_then(|file|
  // toml::from_str(&file).map_err(ConfigLoadError::BadConfig));

  //   let local_config: Result<ConfigRaw, ConfigLoadError> =
  //     local.and_then(|file|
  // toml::from_str(&file).map_err(ConfigLoadError::BadConfig));

  //   let res = match (global_config, local_config) {
  //     (Ok(global), Ok(local)) => {
  //       let mut keys = keymap::default();
  //       if let Some(global_keys) = global.keys {
  //         merge_keys(&mut keys, global_keys)
  //       }
  //       if let Some(local_keys) = local.keys {
  //         merge_keys(&mut keys, local_keys)
  //       }

  //       let editor = match (global.editor, local.editor) {
  //         (None, None) => helix_view::editor::Config::default(),
  //         (None, Some(val)) | (Some(val), None) => {
  //           val.try_into().map_err(ConfigLoadError::BadConfig)?
  //         },
  //         (Some(global), Some(local)) => {
  //           merge_toml_values(global, local, 3)
  //             .try_into()
  //             .map_err(ConfigLoadError::BadConfig)?
  //         },
  //       };

  //       Config {
  //         theme: local.theme.or(global.theme),
  //         keys,
  //         editor,
  //       }
  //     },
  //     // if any configs are invalid return that first
  //     (_, Err(ConfigLoadError::BadConfig(err))) |
  // (Err(ConfigLoadError::BadConfig(err)), _) => {       return
  // Err(ConfigLoadError::BadConfig(err));     },
  //     (Ok(config), Err(_)) | (Err(_), Ok(config)) => {
  //       let mut keys = keymap::default();
  //       if let Some(keymap) = config.keys {
  //         merge_keys(&mut keys, keymap);
  //       }
  //       Config {
  //         theme: config.theme,
  //         keys,
  //         editor: config.editor.map_or_else(
  //           || Ok(helix_view::editor::Config::default()),
  //           |val| val.try_into().map_err(ConfigLoadError::BadConfig),
  //         )?,
  //       }
  //     },

  //     // these are just two io errors return the one for the global config
  //     (Err(err), Err(_)) => return Err(err),
  //   };

  //   Ok(res)
  // }

  // pub fn load_default() -> Result<Config, ConfigLoadError> {
  //   let global_config =
  //     fs::read_to_string(the_editor_loader::config_file()).
  // map_err(ConfigLoadError::Error);

  //   let local_config =
  // fs::read_to_string(the_editor_loader::workspace_config_file())
  //     .map_err(ConfigLoadError::Error);
  //   Config::load(global_config, local_config)
  //   Config::default()
  // }
}
