use unicode_segmentation::UnicodeSegmentation;

use the_core::grapheme::GraphemeStr;

use crate::syntax::Highlight;

// TODO
#[derive(Debug, Clone)]
pub struct TextFormat {
  pub soft_wrap: bool,
  pub tab_width: u16,
  pub max_wrap: u16,
  pub max_indent_retain: u16,
  /// Visual marker inserted at wrap boundaries.
  ///
  /// If you mutate this field directly, call `rebuild_wrap_indicator`.
  pub wrap_indicator: Box<str>,
  /// Pre-tokenized graphemes for `wrap_indicator`.
  ///
  /// This is derived from `wrap_indicator` and should be kept in sync.
  pub wrap_indicator_graphemes: Vec<GraphemeStr<'static>>,
  pub wrap_indicator_highlight: Option<Highlight>,
  pub viewport_width: u16,
  pub soft_wrap_at_text_width: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TextFormatSignature {
  soft_wrap: bool,
  tab_width: u16,
  max_wrap: u16,
  max_indent_retain: u16,
  wrap_indicator: Box<str>,
  viewport_width: u16,
  soft_wrap_at_text_width: bool,
}

impl TextFormat {
  pub fn rebuild_wrap_indicator(&mut self) {
    self.wrap_indicator_graphemes = compute_wrap_indicator_graphemes(&self.wrap_indicator);
  }

  pub fn set_wrap_indicator(&mut self, indicator: impl Into<Box<str>>) {
    self.wrap_indicator = indicator.into();
    self.rebuild_wrap_indicator();
  }

  pub(crate) fn signature(&self) -> TextFormatSignature {
    TextFormatSignature {
      soft_wrap: self.soft_wrap,
      tab_width: self.tab_width,
      max_wrap: self.max_wrap,
      max_indent_retain: self.max_indent_retain,
      wrap_indicator: self.wrap_indicator.clone(),
      viewport_width: self.viewport_width,
      soft_wrap_at_text_width: self.soft_wrap_at_text_width,
    }
  }
}

// test implementation is basically only used for testing or when softwrap is
// always disabled
impl Default for TextFormat {
  fn default() -> Self {
    let wrap_indicator = Box::from(" ");
    let wrap_indicator_graphemes = compute_wrap_indicator_graphemes(&wrap_indicator);
    TextFormat {
      soft_wrap: false,
      tab_width: 4,
      max_wrap: 3,
      max_indent_retain: 4,
      wrap_indicator,
      wrap_indicator_graphemes,
      viewport_width: 17,
      wrap_indicator_highlight: None,
      soft_wrap_at_text_width: false,
    }
  }
}

fn compute_wrap_indicator_graphemes(indicator: &str) -> Vec<GraphemeStr<'static>> {
  UnicodeSegmentation::graphemes(indicator, true)
    .map(|g| GraphemeStr::from(g.to_string()))
    .collect()
}
