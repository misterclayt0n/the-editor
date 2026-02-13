use the_lib::render::{
  LayoutIntent,
  UiAlign,
  UiAlignPair,
  UiConstraints,
  UiContainer,
  UiInsets,
  UiLayer,
  UiNode,
  UiPanel,
  UiText,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DocsPanelSource {
  #[default]
  Completion,
  Hover,
  CommandPalette,
}

pub fn docs_panel_source_from_panel_id(id: &str) -> Option<DocsPanelSource> {
  match id {
    "completion_docs" => Some(DocsPanelSource::Completion),
    "lsp_hover" => Some(DocsPanelSource::Hover),
    "term_command_palette_docs" => Some(DocsPanelSource::CommandPalette),
    _ => None,
  }
}

pub fn docs_panel_source_from_text_id(id: &str) -> Option<DocsPanelSource> {
  match id {
    "completion_docs_text" => Some(DocsPanelSource::Completion),
    "lsp_hover_text" => Some(DocsPanelSource::Hover),
    "term_command_palette_docs_text" => Some(DocsPanelSource::CommandPalette),
    _ => None,
  }
}

#[derive(Debug, Clone)]
pub struct DocsPanelConfig<'a> {
  pub panel_id:   &'a str,
  pub text_id:    &'a str,
  pub intent:     LayoutIntent,
  pub role:       &'a str,
  pub layer:      UiLayer,
  pub min_width:  Option<u16>,
  pub max_width:  Option<u16>,
  pub min_height: Option<u16>,
  pub max_height: Option<u16>,
  pub padding:    UiInsets,
  pub align:      UiAlignPair,
  pub border:     bool,
  pub clip:       bool,
}

impl<'a> DocsPanelConfig<'a> {
  pub fn completion_docs(panel_id: &'a str, text_id: &'a str, intent: LayoutIntent) -> Self {
    Self {
      panel_id,
      text_id,
      intent,
      role: "completion_docs",
      layer: UiLayer::Overlay,
      min_width: Some(28),
      max_width: Some(84),
      min_height: None,
      max_height: Some(18),
      padding: UiInsets {
        left:   1,
        right:  1,
        top:    1,
        bottom: 1,
      },
      align: UiAlignPair {
        horizontal: UiAlign::Start,
        vertical:   UiAlign::End,
      },
      border: false,
      clip: false,
    }
  }

  pub fn hover_docs(panel_id: &'a str, text_id: &'a str, intent: LayoutIntent) -> Self {
    let mut config = Self::completion_docs(panel_id, text_id, intent);
    config.layer = UiLayer::Tooltip;
    config.min_width = Some(30);
    config.max_width = Some(100);
    config.max_height = Some(22);
    config
  }

  pub fn command_palette_docs(panel_id: &'a str, text_id: &'a str, intent: LayoutIntent) -> Self {
    Self::completion_docs(panel_id, text_id, intent)
  }
}

pub fn build_docs_panel(config: DocsPanelConfig<'_>, docs: String) -> UiNode {
  let mut docs_text = UiText::new(config.text_id, docs);
  docs_text.style = docs_text.style.with_role(config.role);
  docs_text.clip = config.clip;

  let mut docs_container = UiContainer::column(format!("{}_container", config.panel_id), 0, vec![
    UiNode::Text(docs_text),
  ]);
  docs_container.style = docs_container.style.with_role(config.role);

  let mut docs_panel = UiPanel::new(
    config.panel_id,
    config.intent,
    UiNode::Container(docs_container),
  );
  docs_panel.style = docs_panel.style.with_role(config.role);
  docs_panel.style.border = if config.border {
    docs_panel.style.border
  } else {
    None
  };
  docs_panel.layer = config.layer;
  docs_panel.constraints = UiConstraints {
    min_width:  config.min_width,
    max_width:  config.max_width,
    min_height: config.min_height,
    max_height: config.max_height,
    padding:    config.padding,
    align:      config.align,
  };

  UiNode::Panel(docs_panel)
}
