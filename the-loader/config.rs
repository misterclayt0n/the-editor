use std::str::from_utf8;

use eyre::{
  Context,
  Result,
};

/// Default built-in languages.toml.
pub fn default_lang_config() -> Result<toml::Value> {
  let default_config = include_bytes!("../languages.toml");
  let config_str =
    from_utf8(default_config).context("built-in languages.toml contains invalid UTF-8")?;
  toml::from_str(config_str).context("failed to parse built-in languages.toml")
}

/// User configured languages.toml file, merged with the default config.
pub fn user_lang_config() -> Result<toml::Value> {
  let default = default_lang_config()?;

  let config = [
    crate::config_dir(),
    crate::find_workspace().0.join(".the-editor"),
  ]
  .into_iter()
  .map(|path| path.join("languages.toml"))
  .filter_map(|file| {
    std::fs::read_to_string(file)
      .map(|config| toml::from_str(&config))
      .ok()
  })
  .collect::<Result<Vec<_>, _>>()
  .context("failed to parse user languages.toml")?
  .into_iter()
  .fold(default, |a, b| crate::merge_toml_values(a, b, 3));

  Ok(config)
}
