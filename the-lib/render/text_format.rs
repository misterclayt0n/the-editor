use crate::syntax::Highlight;

// TODO
#[derive(Debug, Clone)]
pub struct TextFormat {
  pub soft_wrap: bool,
  pub tab_width: u16,
  pub max_wrap: u16,
  pub max_indent_retain: u16,
  pub wrap_indicator: Box<str>,
  pub wrap_indicator_highlight: Option<Highlight>,
  pub viewport_width: u16,
  pub soft_wrap_at_text_width: bool,
}

// test implementation is basically only used for testing or when softwrap is
// always disabled
impl Default for TextFormat {
  fn default() -> Self {
    TextFormat {
      soft_wrap: false,
      tab_width: 4,
      max_wrap: 3,
      max_indent_retain: 4,
      wrap_indicator: Box::from(" "),
      viewport_width: 17,
      wrap_indicator_highlight: None,
      soft_wrap_at_text_width: false,
    }
  }
}
