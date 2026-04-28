use unicode_segmentation::UnicodeSegmentation;

use crate::grapheme::GraphemeStr;

/// Pure visual layout policy for document formatting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextFormat {
  pub soft_wrap:               bool,
  pub tab_width:               u16,
  pub max_wrap:                u16,
  pub max_indent_retain:       u16,
  pub soft_wrap_at_text_width: bool,
  wrap_indicator:              Box<str>,
  wrap_indicator_graphemes:    Vec<GraphemeStr<'static>>,
}

impl TextFormat {
  pub fn wrap_indicator(&self) -> &str {
    &self.wrap_indicator
  }

  pub fn set_wrap_indicator(&mut self, indicator: impl Into<Box<str>>) {
    self.wrap_indicator = indicator.into();
    self.wrap_indicator_graphemes = tokenize_wrap_indicator(&self.wrap_indicator);
  }

  pub(crate) fn wrap_indicator_graphemes(&self) -> &[GraphemeStr<'static>] {
    &self.wrap_indicator_graphemes
  }
}

impl Default for TextFormat {
  fn default() -> Self {
    let wrap_indicator = Box::<str>::from(" ");
    let wrap_indicator_graphemes = tokenize_wrap_indicator(&wrap_indicator);

    Self {
      soft_wrap: false,
      tab_width: 4,
      max_wrap: 3,
      max_indent_retain: 4,
      soft_wrap_at_text_width: false,
      wrap_indicator,
      wrap_indicator_graphemes,
    }
  }
}

fn tokenize_wrap_indicator(indicator: &str) -> Vec<GraphemeStr<'static>> {
  UnicodeSegmentation::graphemes(indicator, true)
    .map(|grapheme| GraphemeStr::from(grapheme.to_owned()))
    .collect()
}

#[cfg(test)]
mod tests {
  use super::TextFormat;

  #[test]
  fn default_wrap_indicator_is_available() {
    let format = TextFormat::default();

    assert_eq!(format.wrap_indicator(), " ");
    assert_eq!(format.wrap_indicator_graphemes().len(), 1);
  }

  #[test]
  fn set_wrap_indicator_rebuilds_cached_graphemes() {
    let mut format = TextFormat::default();

    format.set_wrap_indicator("↪ ");

    assert_eq!(format.wrap_indicator(), "↪ ");
    assert_eq!(format.wrap_indicator_graphemes().len(), 2);
  }
}
