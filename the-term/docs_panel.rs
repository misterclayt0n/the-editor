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

impl DocsPanelSource {
  pub const fn as_str(self) -> &'static str {
    match self {
      Self::Completion => "completion",
      Self::Hover => "hover",
      Self::CommandPalette => "command_palette",
    }
  }

  pub fn parse(value: &str) -> Option<Self> {
    match value.trim().to_ascii_lowercase().as_str() {
      "completion" => Some(Self::Completion),
      "hover" => Some(Self::Hover),
      "command_palette" | "commandpalette" | "command-palette" | "palette" => {
        Some(Self::CommandPalette)
      },
      _ => None,
    }
  }
}

fn source_from_hint(hint: &str) -> Option<DocsPanelSource> {
  let hint = hint.trim().to_ascii_lowercase();
  if hint.is_empty() {
    return None;
  }
  let has_docs = hint.contains("docs") || hint.contains("doc");
  if hint.contains("hover") || hint.contains("tooltip") {
    return Some(DocsPanelSource::Hover);
  }
  if has_docs && hint.contains("command") && hint.contains("palette") {
    return Some(DocsPanelSource::CommandPalette);
  }
  if has_docs && hint.contains("completion") {
    return Some(DocsPanelSource::Completion);
  }
  None
}

fn source_from_role(role: Option<&str>) -> Option<DocsPanelSource> {
  let role = role?;
  match role {
    "completion_docs" => Some(DocsPanelSource::Completion),
    "hover_docs" | "lsp_hover" => Some(DocsPanelSource::Hover),
    "command_palette_docs" | "term_command_palette_docs" => Some(DocsPanelSource::CommandPalette),
    _ => {
      if role.contains("docs") || role.contains("doc") {
        source_from_hint(role)
      } else {
        None
      }
    },
  }
}

pub fn docs_panel_source_from_panel_id(id: &str) -> Option<DocsPanelSource> {
  match id {
    "completion_docs" => Some(DocsPanelSource::Completion),
    "lsp_hover" => Some(DocsPanelSource::Hover),
    "term_command_palette_docs" => Some(DocsPanelSource::CommandPalette),
    _ => source_from_hint(id),
  }
}

pub fn docs_panel_source_from_text_id(id: &str) -> Option<DocsPanelSource> {
  match id {
    "completion_docs_text" => Some(DocsPanelSource::Completion),
    "lsp_hover_text" => Some(DocsPanelSource::Hover),
    "term_command_palette_docs_text" => Some(DocsPanelSource::CommandPalette),
    _ => source_from_hint(id),
  }
}

pub fn docs_panel_source_from_panel(panel: &UiPanel) -> Option<DocsPanelSource> {
  panel
    .source
    .as_deref()
    .and_then(DocsPanelSource::parse)
    .or_else(|| docs_panel_source_from_panel_id(panel.id.as_str()))
    .or_else(|| source_from_role(panel.style.role.as_deref()))
    .or_else(|| {
      match &panel.intent {
        LayoutIntent::Custom(name) => source_from_hint(name),
        _ => None,
      }
    })
}

pub fn docs_panel_source_from_text(text: &UiText) -> Option<DocsPanelSource> {
  text
    .source
    .as_deref()
    .and_then(DocsPanelSource::parse)
    .or_else(|| text.id.as_deref().and_then(docs_panel_source_from_text_id))
    .or_else(|| source_from_role(text.style.role.as_deref()))
}

#[derive(Debug, Clone)]
pub struct DocsPanelConfig<'a> {
  pub panel_id:   &'a str,
  pub text_id:    &'a str,
  pub source:     DocsPanelSource,
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
      source: DocsPanelSource::Completion,
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
    config.source = DocsPanelSource::Hover;
    config.layer = UiLayer::Tooltip;
    config.min_width = Some(30);
    config.max_width = Some(100);
    config.max_height = Some(22);
    config
  }

  pub fn command_palette_docs(panel_id: &'a str, text_id: &'a str, intent: LayoutIntent) -> Self {
    let mut config = Self::completion_docs(panel_id, text_id, intent);
    config.source = DocsPanelSource::CommandPalette;
    config
  }
}

pub fn build_docs_panel(config: DocsPanelConfig<'_>, docs: String) -> UiNode {
  let mut docs_text = UiText::new(config.text_id, docs);
  docs_text.source = Some(config.source.as_str().to_string());
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
  docs_panel.source = Some(config.source.as_str().to_string());
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

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn build_docs_panel_sets_source_metadata() {
    let node = build_docs_panel(
      DocsPanelConfig::hover_docs(
        "hover_panel",
        "hover_text",
        LayoutIntent::Custom("hover".into()),
      ),
      "hover docs".to_string(),
    );

    let UiNode::Panel(panel) = node else {
      panic!("expected panel");
    };
    assert_eq!(panel.source.as_deref(), Some("hover"));

    let UiNode::Container(container) = panel.child.as_ref() else {
      panic!("expected container child");
    };
    let UiNode::Text(text) = &container.children[0] else {
      panic!("expected text child");
    };
    assert_eq!(text.source.as_deref(), Some("hover"));
  }

  #[test]
  fn source_resolves_from_custom_panel_hints_without_fixed_ids() {
    let panel = UiPanel::new(
      "renamed_hover_panel",
      LayoutIntent::Custom("shared_hover_docs".to_string()),
      UiNode::text("content", "docs"),
    );
    assert_eq!(
      docs_panel_source_from_panel(&panel),
      Some(DocsPanelSource::Hover)
    );
  }

  #[test]
  fn source_resolves_from_text_role_without_fixed_ids() {
    let mut text = UiText::new("shared_docs_text", "docs");
    text.style = text.style.with_role("hover_docs");
    assert_eq!(
      docs_panel_source_from_text(&text),
      Some(DocsPanelSource::Hover)
    );
  }
}
