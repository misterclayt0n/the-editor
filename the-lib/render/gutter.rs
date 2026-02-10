use serde::{
  Deserialize,
  Serialize,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LineNumberMode {
  Absolute,
  Relative,
}

impl Default for LineNumberMode {
  fn default() -> Self {
    Self::Absolute
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GutterType {
  Diagnostics,
  Spacer,
  LineNumbers,
  Diff,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", default, deny_unknown_fields)]
pub struct GutterLineNumbersConfig {
  pub min_width: usize,
  pub mode:      LineNumberMode,
}

impl Default for GutterLineNumbersConfig {
  fn default() -> Self {
    Self {
      min_width: 3,
      mode:      LineNumberMode::Absolute,
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", default, deny_unknown_fields)]
pub struct GutterConfig {
  pub layout:       Vec<GutterType>,
  pub line_numbers: GutterLineNumbersConfig,
}

impl Default for GutterConfig {
  fn default() -> Self {
    Self {
      layout:       vec![
        GutterType::Diagnostics,
        GutterType::Spacer,
        GutterType::LineNumbers,
        GutterType::Spacer,
        GutterType::Diff,
      ],
      line_numbers: GutterLineNumbersConfig::default(),
    }
  }
}

#[cfg(test)]
mod tests {
  use super::{
    GutterConfig,
    GutterLineNumbersConfig,
    GutterType,
    LineNumberMode,
  };

  #[test]
  fn default_layout_matches_helix_style_order() {
    let config = GutterConfig::default();
    assert_eq!(config.layout, vec![
      GutterType::Diagnostics,
      GutterType::Spacer,
      GutterType::LineNumbers,
      GutterType::Spacer,
      GutterType::Diff,
    ]);
  }

  #[test]
  fn line_numbers_defaults() {
    let config = GutterLineNumbersConfig::default();
    assert_eq!(config.min_width, 3);
    assert_eq!(config.mode, LineNumberMode::Absolute);
  }
}
