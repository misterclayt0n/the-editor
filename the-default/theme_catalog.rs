use std::{
  collections::{
    BTreeSet,
    HashSet,
  },
  fs,
  path::{
    Path,
    PathBuf,
  },
};

use the_lib::render::theme::{
  Theme,
  base16_default_theme,
  default_theme,
};
use toml::{
  Value,
  map::Map,
};
use tracing::warn;

#[derive(Clone, Debug, Default)]
pub struct ThemeCatalog {
  theme_dirs: Vec<PathBuf>,
  names:      BTreeSet<String>,
}

impl ThemeCatalog {
  pub fn load(workspace_root: Option<&Path>) -> Self {
    let mut theme_dirs = Vec::new();

    if let Some(workspace_root) = workspace_root {
      theme_dirs.push(workspace_root.join(".the-editor").join("themes"));
    }

    theme_dirs.push(the_loader::config_dir().join("themes"));
    theme_dirs.extend(
      the_loader::runtime_dirs()
        .iter()
        .map(|dir| dir.join("themes")),
    );

    let mut names = BTreeSet::from([
      default_theme().name().to_string(),
      base16_default_theme().name().to_string(),
    ]);

    for dir in &theme_dirs {
      for name in Self::read_names(dir) {
        names.insert(name);
      }
    }

    Self { theme_dirs, names }
  }

  pub fn names(&self) -> Vec<String> {
    self.names.iter().cloned().collect()
  }

  pub fn get(&self, name: &str) -> Option<Theme> {
    self.load_theme(name)
  }

  pub fn load_theme(&self, name: &str) -> Option<Theme> {
    self
      .load_theme_impl(name)
      .map_err(|error| {
        warn!("Failed to load theme '{name}': {error}");
        error
      })
      .ok()
  }

  fn load_theme_impl(&self, name: &str) -> Result<Theme, String> {
    match name {
      "default" => return Ok(default_theme().clone()),
      "base16_default" => return Ok(base16_default_theme().clone()),
      _ => {},
    }

    let mut visited_paths = HashSet::new();
    let value = self.load_theme_value(name, &mut visited_paths)?;
    Ok(Theme::from_named_value(name, value))
  }

  fn load_theme_value(
    &self,
    name: &str,
    visited_paths: &mut HashSet<PathBuf>,
  ) -> Result<Value, String> {
    let path = self.path(name, visited_paths)?;
    let theme_toml = self.load_toml(&path)?;

    let inherits = theme_toml.get("inherits");
    if let Some(parent_theme_name) = inherits {
      let parent_theme_name = parent_theme_name
        .as_str()
        .ok_or_else(|| format!("expected 'inherits' to be a string in '{}'", path.display()))?;

      let parent_theme_toml = match parent_theme_name {
        "default" => {
          toml::from_str(include_str!("../the-lib/theme.toml"))
            .map_err(|error| format!("failed to parse embedded default theme: {error}"))?
        },
        "base16_default" => {
          toml::from_str(include_str!("../the-lib/base16_theme.toml"))
            .map_err(|error| format!("failed to parse embedded base16 theme: {error}"))?
        },
        _ => self.load_theme_value(parent_theme_name, visited_paths)?,
      };

      Ok(self.merge_themes(parent_theme_toml, theme_toml))
    } else {
      Ok(theme_toml)
    }
  }

  fn read_names(path: &Path) -> Vec<String> {
    fs::read_dir(path)
      .map(|entries| {
        entries
          .filter_map(Result::ok)
          .filter_map(|entry| {
            let path = entry.path();
            (path.extension().and_then(|ext| ext.to_str()) == Some("toml")).then(|| {
              path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .map(str::to_string)
            })?
          })
          .collect()
      })
      .unwrap_or_default()
  }

  fn merge_themes(&self, parent_theme_toml: Value, theme_toml: Value) -> Value {
    let parent_palette = parent_theme_toml.get("palette");
    let palette = theme_toml.get("palette");

    let palette_values = match (parent_palette, palette) {
      (Some(parent_palette), Some(palette)) => {
        the_loader::merge_toml_values(parent_palette.clone(), palette.clone(), 2)
      },
      (Some(parent_palette), None) => parent_palette.clone(),
      (None, Some(palette)) => palette.clone(),
      (None, None) => Map::new().into(),
    };

    let parent_ghostty = parent_theme_toml.get("ghostty");
    let ghostty = theme_toml.get("ghostty");
    let ghostty_values = match (parent_ghostty, ghostty) {
      (Some(parent_ghostty), Some(ghostty)) => {
        the_loader::merge_toml_values(parent_ghostty.clone(), ghostty.clone(), 2)
      },
      (Some(parent_ghostty), None) => parent_ghostty.clone(),
      (None, Some(ghostty)) => ghostty.clone(),
      (None, None) => Value::Table(Map::new()),
    };

    let mut nested = Map::new();
    nested.insert(String::from("palette"), palette_values);
    if parent_ghostty.is_some() || ghostty.is_some() {
      nested.insert(String::from("ghostty"), ghostty_values);
    }

    let theme = the_loader::merge_toml_values(parent_theme_toml, theme_toml, 1);
    the_loader::merge_toml_values(theme, Value::Table(nested), 1)
  }

  fn load_toml(&self, path: &Path) -> Result<Value, String> {
    let contents = fs::read_to_string(path)
      .map_err(|error| format!("failed to read theme '{}': {error}", path.display()))?;
    toml::from_str(&contents)
      .map_err(|error| format!("failed to parse theme '{}': {error}", path.display()))
  }

  fn path(&self, name: &str, visited_paths: &mut HashSet<PathBuf>) -> Result<PathBuf, String> {
    let filename = format!("{name}.toml");
    let mut cycle_found = false;

    self
      .theme_dirs
      .iter()
      .find_map(|dir| {
        let path = dir.join(&filename);
        if !path.exists() {
          None
        } else if visited_paths.contains(&path) {
          cycle_found = true;
          None
        } else {
          visited_paths.insert(path.clone());
          Some(path)
        }
      })
      .ok_or_else(|| {
        if cycle_found {
          format!("cycle found while inheriting theme '{name}'")
        } else {
          format!("theme '{name}' not found")
        }
      })
  }
}

#[cfg(test)]
mod tests {
  use std::{
    fs,
    time::{
      SystemTime,
      UNIX_EPOCH,
    },
  };

  use super::*;

  fn unique_temp_dir(label: &str) -> PathBuf {
    let nanos = SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .expect("system clock should be after unix epoch")
      .as_nanos();
    std::env::temp_dir().join(format!(
      "the-editor-theme-catalog-{label}-{}-{nanos}",
      std::process::id()
    ))
  }

  #[test]
  fn load_theme_merges_inherited_ghostty_palette() {
    let root = unique_temp_dir("inherit");
    let low = root.join("low");
    let high = root.join("high");
    fs::create_dir_all(low.join("themes")).expect("low themes dir");
    fs::create_dir_all(high.join("themes")).expect("high themes dir");

    fs::write(
      low.join("themes").join("parent.toml"),
      r##"
[ghostty]
background = "#111111"

[ghostty.palette]
"0" = "#222222"
"1" = "#333333"
"##,
    )
    .expect("write parent theme");

    fs::write(
      high.join("themes").join("child.toml"),
      r##"
inherits = "parent"
keyword = "#abcdef"

[ghostty.palette]
"1" = "#444444"
"2" = "#555555"
"##,
    )
    .expect("write child theme");

    let catalog = ThemeCatalog {
      theme_dirs: vec![high.join("themes"), low.join("themes")],
      names:      BTreeSet::from(["child".to_string(), "parent".to_string()]),
    };

    let theme = catalog
      .load_theme("child")
      .expect("child theme should load");
    let ghostty = theme.ghostty();
    assert_eq!(
      ghostty.background(),
      Some(the_lib::render::theme::Color::Rgb(0x11, 0x11, 0x11))
    );
    assert_eq!(
      ghostty.palette_color(0),
      Some(the_lib::render::theme::Color::Rgb(0x22, 0x22, 0x22))
    );
    assert_eq!(
      ghostty.palette_color(1),
      Some(the_lib::render::theme::Color::Rgb(0x44, 0x44, 0x44))
    );
    assert_eq!(
      ghostty.palette_color(2),
      Some(the_lib::render::theme::Color::Rgb(0x55, 0x55, 0x55))
    );

    fs::remove_dir_all(root).ok();
  }
}
