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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GutterType {
  Diagnostics,
  Spacer,
  LineNumbers,
  Diff,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct CustomGutterSlot {
  pub id:    String,
  pub width: usize,
}

impl CustomGutterSlot {
  pub fn new(id: impl Into<String>, width: usize) -> Self {
    Self {
      id:    id.into(),
      width: width.max(1),
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum GutterSlot {
  Builtin(GutterType),
  Custom(CustomGutterSlot),
}

impl GutterSlot {
  pub fn builtin(kind: GutterType) -> Self {
    Self::Builtin(kind)
  }

  pub fn custom(id: impl Into<String>, width: usize) -> Self {
    Self::Custom(CustomGutterSlot::new(id, width))
  }

  pub fn builtin_kind(&self) -> Option<GutterType> {
    match self {
      Self::Builtin(kind) => Some(*kind),
      Self::Custom(_) => None,
    }
  }

  pub fn custom_id(&self) -> Option<&str> {
    match self {
      Self::Builtin(_) => None,
      Self::Custom(slot) => Some(slot.id.as_str()),
    }
  }

  pub fn is_builtin(&self, kind: GutterType) -> bool {
    matches!(self, Self::Builtin(slot_kind) if *slot_kind == kind)
  }

  pub fn width(&self, line_number_width: usize) -> u16 {
    match self {
      Self::Builtin(GutterType::Diagnostics | GutterType::Diff | GutterType::Spacer) => 1,
      Self::Builtin(GutterType::LineNumbers) => line_number_width as u16,
      Self::Custom(slot) => slot.width as u16,
    }
  }
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
  pub layout:       Vec<GutterSlot>,
  pub line_numbers: GutterLineNumbersConfig,
}

impl Default for GutterConfig {
  fn default() -> Self {
    Self {
      layout:       vec![
        GutterSlot::builtin(GutterType::Diagnostics),
        GutterSlot::builtin(GutterType::Spacer),
        GutterSlot::builtin(GutterType::LineNumbers),
        GutterSlot::builtin(GutterType::Spacer),
        GutterSlot::builtin(GutterType::Diff),
      ],
      line_numbers: GutterLineNumbersConfig::default(),
    }
  }
}

impl GutterConfig {
  pub fn contains_builtin(&self, kind: GutterType) -> bool {
    self.layout.iter().any(|slot| slot.is_builtin(kind))
  }

  pub fn remove_builtin(&mut self, kind: GutterType) {
    self.layout.retain(|slot| !slot.is_builtin(kind));
  }
}

#[cfg(test)]
mod tests {
  use super::{
    GutterConfig,
    GutterLineNumbersConfig,
    GutterSlot,
    GutterType,
    LineNumberMode,
  };

  #[test]
  fn default_layout_matches_helix_style_order() {
    let config = GutterConfig::default();
    assert_eq!(config.layout, vec![
      GutterSlot::builtin(GutterType::Diagnostics),
      GutterSlot::builtin(GutterType::Spacer),
      GutterSlot::builtin(GutterType::LineNumbers),
      GutterSlot::builtin(GutterType::Spacer),
      GutterSlot::builtin(GutterType::Diff),
    ]);
  }

  #[test]
  fn line_numbers_defaults() {
    let config = GutterLineNumbersConfig::default();
    assert_eq!(config.min_width, 3);
    assert_eq!(config.mode, LineNumberMode::Absolute);
  }
}
