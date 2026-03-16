use std::collections::HashMap;

use serde::{
  Deserialize,
  Serialize,
};

use crate::render::graphics::Color;

fn default_clip() -> bool {
  true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiTree {
  pub root:     UiNode,
  pub overlays: Vec<UiNode>,
  pub focus:    Option<UiFocus>,
}

impl UiTree {
  pub fn new() -> Self {
    Self {
      root:     UiNode::Container(UiContainer::default()),
      overlays: Vec::new(),
      focus:    None,
    }
  }
}

impl Default for UiTree {
  fn default() -> Self {
    Self::new()
  }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum UiNode {
  Container(UiContainer),
  Panel(UiPanel),
  Text(UiText),
  List(UiList),
  Input(UiInput),
  Divider(UiDivider),
  Spacer(UiSpacer),
  Tooltip(UiTooltip),
  StatusBar(UiStatusBar),
}

impl UiNode {
  pub fn text(id: impl Into<String>, content: impl Into<String>) -> Self {
    UiNode::Text(UiText::new(id, content))
  }

  pub fn input(id: impl Into<String>, value: impl Into<String>) -> Self {
    UiNode::Input(UiInput::new(id, value))
  }

  pub fn list(id: impl Into<String>, items: Vec<UiListItem>) -> Self {
    UiNode::List(UiList::new(id, items))
  }

  pub fn divider() -> Self {
    UiNode::Divider(UiDivider { id: None })
  }

  pub fn spacer(size: u16) -> Self {
    UiNode::Spacer(UiSpacer { id: None, size })
  }

  pub fn container(id: impl Into<String>, layout: UiLayout, children: Vec<UiNode>) -> Self {
    UiNode::Container(UiContainer::new(id, layout, children))
  }

  pub fn panel(id: impl Into<String>, intent: LayoutIntent, child: UiNode) -> Self {
    UiNode::Panel(UiPanel::new(id, intent, child))
  }

  pub fn panel_floating(id: impl Into<String>, child: UiNode) -> Self {
    UiNode::Panel(UiPanel::floating(id, child))
  }

  pub fn panel_bottom(id: impl Into<String>, child: UiNode) -> Self {
    UiNode::Panel(UiPanel::bottom(id, child))
  }

  pub fn panel_top(id: impl Into<String>, child: UiNode) -> Self {
    UiNode::Panel(UiPanel::top(id, child))
  }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UiState {
  panels:    HashMap<String, UiPanelState>,
  nodes:     HashMap<String, UiNodeState>,
  pub focus: Option<UiFocus>,
}

impl UiState {
  pub fn panel_mut(&mut self, id: impl Into<String>) -> &mut UiPanelState {
    let id = id.into();
    self.panels.entry(id).or_default()
  }

  pub fn panel(&self, id: &str) -> Option<&UiPanelState> {
    self.panels.get(id)
  }

  pub fn panel_visible(&self, id: &str) -> bool {
    self
      .panels
      .get(id)
      .map(|state| state.visible)
      .unwrap_or(false)
  }

  pub fn show_panel(&mut self, id: impl Into<String>) {
    self.panel_mut(id).show();
  }

  pub fn hide_panel(&mut self, id: impl Into<String>) {
    self.panel_mut(id).hide();
  }

  pub fn toggle_panel(&mut self, id: impl Into<String>) {
    self.panel_mut(id).toggle();
  }

  pub fn node_mut(&mut self, id: impl Into<String>) -> &mut UiNodeState {
    let id = id.into();
    self.nodes.entry(id).or_default()
  }

  pub fn node(&self, id: &str) -> Option<&UiNodeState> {
    self.nodes.get(id)
  }

  pub fn set_focus(&mut self, focus: Option<UiFocus>) {
    self.focus = focus;
  }

  pub fn focus(&self) -> Option<&UiFocus> {
    self.focus.as_ref()
  }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct UiPanelState {
  pub visible: bool,
}

impl Default for UiPanelState {
  fn default() -> Self {
    Self { visible: false }
  }
}

impl UiPanelState {
  pub fn show(&mut self) {
    self.visible = true;
  }

  pub fn hide(&mut self) {
    self.visible = false;
  }

  pub fn toggle(&mut self) {
    self.visible = !self.visible;
  }

  pub fn is_visible(&self) -> bool {
    self.visible
  }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UiNodeState {
  pub scroll:   usize,
  pub selected: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiContainer {
  pub id:          Option<String>,
  pub layout:      UiLayout,
  pub children:    Vec<UiNode>,
  pub style:       UiStyle,
  pub constraints: UiConstraints,
}

impl Default for UiContainer {
  fn default() -> Self {
    Self {
      id:          None,
      layout:      UiLayout::Stack {
        axis: UiAxis::Vertical,
        gap:  0,
      },
      children:    Vec::new(),
      style:       UiStyle::default(),
      constraints: UiConstraints::default(),
    }
  }
}

impl UiContainer {
  pub fn new(id: impl Into<String>, layout: UiLayout, children: Vec<UiNode>) -> Self {
    Self {
      id: Some(id.into()),
      layout,
      children,
      style: UiStyle::default(),
      constraints: UiConstraints::default(),
    }
  }

  pub fn stack(id: impl Into<String>, axis: UiAxis, gap: u16, children: Vec<UiNode>) -> Self {
    Self {
      id: Some(id.into()),
      layout: UiLayout::Stack { axis, gap },
      children,
      style: UiStyle::default(),
      constraints: UiConstraints::default(),
    }
  }

  pub fn column(id: impl Into<String>, gap: u16, children: Vec<UiNode>) -> Self {
    Self::stack(id, UiAxis::Vertical, gap, children)
  }

  pub fn row(id: impl Into<String>, gap: u16, children: Vec<UiNode>) -> Self {
    Self::stack(id, UiAxis::Horizontal, gap, children)
  }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiPanel {
  pub id:          String,
  pub title:       Option<String>,
  pub intent:      LayoutIntent,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub source:      Option<String>,
  pub style:       UiStyle,
  pub constraints: UiConstraints,
  pub layer:       UiLayer,
  pub child:       Box<UiNode>,
}

impl UiPanel {
  pub fn new(id: impl Into<String>, intent: LayoutIntent, child: UiNode) -> Self {
    Self {
      id: id.into(),
      title: None,
      intent,
      source: None,
      style: UiStyle::panel(),
      constraints: UiConstraints::panel(),
      layer: UiLayer::Overlay,
      child: Box::new(child),
    }
  }

  pub fn floating(id: impl Into<String>, child: UiNode) -> Self {
    Self {
      id:          id.into(),
      title:       None,
      intent:      LayoutIntent::Floating,
      source:      None,
      style:       UiStyle::panel(),
      constraints: UiConstraints::floating_default(),
      layer:       UiLayer::Overlay,
      child:       Box::new(child),
    }
  }

  pub fn bottom(id: impl Into<String>, child: UiNode) -> Self {
    Self {
      id:          id.into(),
      title:       None,
      intent:      LayoutIntent::Bottom,
      source:      None,
      style:       UiStyle::panel(),
      constraints: UiConstraints::panel(),
      layer:       UiLayer::Overlay,
      child:       Box::new(child),
    }
  }

  pub fn top(id: impl Into<String>, child: UiNode) -> Self {
    Self {
      id:          id.into(),
      title:       None,
      intent:      LayoutIntent::Top,
      source:      None,
      style:       UiStyle::panel(),
      constraints: UiConstraints::panel(),
      layer:       UiLayer::Overlay,
      child:       Box::new(child),
    }
  }
}

#[derive(Debug, Clone)]
pub struct UiPopupBuilder {
  panel_id:    String,
  text_id:     String,
  text:        String,
  source:      Option<String>,
  role:        Option<String>,
  intent:      LayoutIntent,
  layer:       UiLayer,
  constraints: UiConstraints,
  border:      bool,
  clip:        bool,
}

impl UiPopupBuilder {
  pub fn floating(
    panel_id: impl Into<String>,
    text_id: impl Into<String>,
    text: impl Into<String>,
  ) -> Self {
    Self {
      panel_id:    panel_id.into(),
      text_id:     text_id.into(),
      text:        text.into(),
      source:      None,
      role:        None,
      intent:      LayoutIntent::Floating,
      layer:       UiLayer::Overlay,
      constraints: UiConstraints::floating_default(),
      border:      false,
      clip:        false,
    }
  }

  pub fn panel_id(&self) -> &str {
    &self.panel_id
  }

  pub fn text_id(&self) -> &str {
    &self.text_id
  }

  pub fn source(mut self, source: impl Into<String>) -> Self {
    self.source = Some(source.into());
    self
  }

  pub fn role(mut self, role: impl Into<String>) -> Self {
    self.role = Some(role.into());
    self
  }

  pub fn intent(mut self, intent: LayoutIntent) -> Self {
    self.intent = intent;
    self
  }

  pub fn layer(mut self, layer: UiLayer) -> Self {
    self.layer = layer;
    self
  }

  pub fn constraints(mut self, constraints: UiConstraints) -> Self {
    self.constraints = constraints;
    self
  }

  pub fn border(mut self, border: bool) -> Self {
    self.border = border;
    self
  }

  pub fn clip(mut self, clip: bool) -> Self {
    self.clip = clip;
    self
  }

  pub fn build(self) -> UiNode {
    let mut text = UiText::new(self.text_id, self.text);
    text.source = self.source.clone();
    if let Some(role) = self.role.as_ref() {
      text.style = text.style.with_role(role.clone());
    }
    text.clip = self.clip;

    let mut container = UiContainer::column(format!("{}_container", self.panel_id), 0, vec![
      UiNode::Text(text),
    ]);
    if let Some(role) = self.role.as_ref() {
      container.style = container.style.with_role(role.clone());
    }

    let mut panel = UiPanel::new(self.panel_id, self.intent, UiNode::Container(container));
    panel.source = self.source;
    if let Some(role) = self.role {
      panel.style = panel.style.with_role(role);
    }
    if !self.border {
      panel.style.border = None;
    }
    panel.layer = self.layer;
    panel.constraints = self.constraints;
    UiNode::Panel(panel)
  }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiText {
  pub id:        Option<String>,
  pub content:   String,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub source:    Option<String>,
  pub style:     UiStyle,
  #[serde(default)]
  pub max_lines: Option<u16>,
  #[serde(default = "default_clip")]
  pub clip:      bool,
}

impl UiText {
  pub fn new(id: impl Into<String>, content: impl Into<String>) -> Self {
    Self {
      id:        Some(id.into()),
      content:   content.into(),
      source:    None,
      style:     UiStyle::default(),
      max_lines: None,
      clip:      true,
    }
  }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiList {
  pub id:            String,
  pub items:         Vec<UiListItem>,
  pub selected:      Option<usize>,
  pub scroll:        usize,
  #[serde(default)]
  pub virtual_total: Option<usize>,
  #[serde(default)]
  pub virtual_start: usize,
  pub fill_width:    bool,
  pub style:         UiStyle,
  #[serde(default)]
  pub max_visible:   Option<usize>,
  #[serde(default = "default_clip")]
  pub clip:          bool,
}

impl UiList {
  pub fn new(id: impl Into<String>, items: Vec<UiListItem>) -> Self {
    Self {
      id: id.into(),
      items,
      selected: None,
      scroll: 0,
      virtual_total: None,
      virtual_start: 0,
      fill_width: true,
      style: UiStyle::default(),
      max_visible: None,
      clip: true,
    }
  }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiListItem {
  pub title:         String,
  pub subtitle:      Option<String>,
  pub description:   Option<String>,
  pub shortcut:      Option<String>,
  pub badge:         Option<String>,
  #[serde(default)]
  pub leading_icon:  Option<String>,
  #[serde(default)]
  pub leading_color: Option<UiColor>,
  #[serde(default)]
  pub symbols:       Option<Vec<String>>,
  #[serde(default)]
  pub match_indices: Option<Vec<usize>>,
  pub emphasis:      bool,
  #[serde(default)]
  pub action:        Option<String>,
}

impl UiListItem {
  pub fn new(title: impl Into<String>) -> Self {
    Self {
      title:         title.into(),
      subtitle:      None,
      description:   None,
      shortcut:      None,
      badge:         None,
      leading_icon:  None,
      leading_color: None,
      symbols:       None,
      match_indices: None,
      emphasis:      false,
      action:        None,
    }
  }

  pub fn with_action(mut self, action: impl Into<String>) -> Self {
    self.action = Some(action.into());
    self
  }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiInput {
  pub id:          String,
  pub value:       String,
  pub placeholder: Option<String>,
  pub cursor:      usize,
  pub style:       UiStyle,
}

impl UiInput {
  pub fn new(id: impl Into<String>, value: impl Into<String>) -> Self {
    Self {
      id:          id.into(),
      value:       value.into(),
      placeholder: None,
      cursor:      0,
      style:       UiStyle::default(),
    }
  }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiDivider {
  pub id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiSpacer {
  pub id:   Option<String>,
  pub size: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiTooltip {
  pub id:        Option<String>,
  pub target:    Option<String>,
  pub placement: LayoutIntent,
  pub content:   String,
  pub style:     UiStyle,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiStyledSpan {
  pub text:  String,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub style: Option<UiStyle>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum DocsPanelSource {
  #[default]
  Completion,
  Hover,
  Signature,
  CommandPalette,
}

impl DocsPanelSource {
  pub const fn as_str(self) -> &'static str {
    match self {
      Self::Completion => "completion",
      Self::Hover => "hover",
      Self::Signature => "signature",
      Self::CommandPalette => "command_palette",
    }
  }

  pub fn parse(value: &str) -> Option<Self> {
    match value.trim().to_ascii_lowercase().as_str() {
      "completion" => Some(Self::Completion),
      "hover" => Some(Self::Hover),
      "signature" | "signature_help" | "signature-help" => Some(Self::Signature),
      "command_palette" | "commandpalette" | "command-palette" | "palette" => {
        Some(Self::CommandPalette)
      },
      _ => None,
    }
  }
}

fn docs_panel_source_from_hint(hint: &str) -> Option<DocsPanelSource> {
  let hint = hint.trim().to_ascii_lowercase();
  if hint.is_empty() {
    return None;
  }
  let has_docs = hint.contains("docs") || hint.contains("doc");
  if hint.contains("hover") || hint.contains("tooltip") {
    return Some(DocsPanelSource::Hover);
  }
  if hint.contains("signature") {
    return Some(DocsPanelSource::Signature);
  }
  if has_docs && hint.contains("command") && hint.contains("palette") {
    return Some(DocsPanelSource::CommandPalette);
  }
  if has_docs && hint.contains("completion") {
    return Some(DocsPanelSource::Completion);
  }
  None
}

fn docs_panel_source_from_role(role: Option<&str>) -> Option<DocsPanelSource> {
  let role = role?;
  match role {
    "completion_docs" => Some(DocsPanelSource::Completion),
    "hover_docs" | "lsp_hover" => Some(DocsPanelSource::Hover),
    "signature_help" | "signature_docs" => Some(DocsPanelSource::Signature),
    "command_palette_docs" | "term_command_palette_docs" => Some(DocsPanelSource::CommandPalette),
    _ => {
      if role.contains("docs") || role.contains("doc") {
        docs_panel_source_from_hint(role)
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
    "signature_help" => Some(DocsPanelSource::Signature),
    "term_command_palette_docs" => Some(DocsPanelSource::CommandPalette),
    _ => docs_panel_source_from_hint(id),
  }
}

pub fn docs_panel_source_from_text_id(id: &str) -> Option<DocsPanelSource> {
  match id {
    "completion_docs_text" => Some(DocsPanelSource::Completion),
    "lsp_hover_text" => Some(DocsPanelSource::Hover),
    "signature_help_text" => Some(DocsPanelSource::Signature),
    "term_command_palette_docs_text" => Some(DocsPanelSource::CommandPalette),
    _ => docs_panel_source_from_hint(id),
  }
}

pub fn docs_panel_source_from_panel(panel: &UiPanel) -> Option<DocsPanelSource> {
  panel
    .source
    .as_deref()
    .and_then(DocsPanelSource::parse)
    .or_else(|| docs_panel_source_from_panel_id(panel.id.as_str()))
    .or_else(|| docs_panel_source_from_role(panel.style.role.as_deref()))
    .or_else(|| {
      match &panel.intent {
        LayoutIntent::Custom(name) => docs_panel_source_from_hint(name),
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
    .or_else(|| docs_panel_source_from_role(text.style.role.as_deref()))
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

  pub fn signature_help_docs(panel_id: &'a str, text_id: &'a str, intent: LayoutIntent) -> Self {
    let mut config = Self::completion_docs(panel_id, text_id, intent);
    config.source = DocsPanelSource::Signature;
    config
  }
}

pub fn build_docs_panel(config: DocsPanelConfig<'_>, docs: String) -> UiNode {
  UiPopupBuilder::floating(config.panel_id, config.text_id, docs)
    .source(config.source.as_str())
    .role(config.role)
    .intent(config.intent)
    .layer(config.layer)
    .constraints(UiConstraints {
      min_width:  config.min_width,
      max_width:  config.max_width,
      min_height: config.min_height,
      max_height: config.max_height,
      padding:    config.padding,
      align:      config.align,
    })
    .border(config.border)
    .clip(config.clip)
    .build()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiStatusBar {
  pub id:             Option<String>,
  pub left:           String,
  pub center:         String,
  pub right:          String,
  pub style:          UiStyle,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub left_icon:      Option<String>,
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub right_segments: Vec<UiStyledSpan>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiAxis {
  Horizontal,
  Vertical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum UiLayout {
  Stack { axis: UiAxis, gap: u16 },
  Split { axis: UiAxis, ratios: Vec<u16> },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum LayoutIntent {
  Floating,
  Bottom,
  Top,
  SidebarLeft,
  SidebarRight,
  Fullscreen,
  Custom(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiEmphasis {
  Normal,
  Muted,
  Strong,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiRadius {
  None,
  Small,
  Medium,
  Large,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiAlign {
  Start,
  Center,
  End,
  Stretch,
}

impl Default for UiAlign {
  fn default() -> Self {
    UiAlign::Start
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct UiAlignPair {
  pub horizontal: UiAlign,
  pub vertical:   UiAlign,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct UiInsets {
  pub left:   u16,
  pub right:  u16,
  pub top:    u16,
  pub bottom: u16,
}

impl UiInsets {
  pub fn horizontal(&self) -> u16 {
    self.left.saturating_add(self.right)
  }

  pub fn vertical(&self) -> u16 {
    self.top.saturating_add(self.bottom)
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct UiConstraints {
  pub min_width:  Option<u16>,
  pub max_width:  Option<u16>,
  pub min_height: Option<u16>,
  pub max_height: Option<u16>,
  pub padding:    UiInsets,
  pub align:      UiAlignPair,
}

impl UiConstraints {
  pub fn panel() -> Self {
    Self {
      padding: UiInsets {
        left:   1,
        right:  1,
        top:    1,
        bottom: 1,
      },
      ..Self::default()
    }
  }

  pub fn floating_default() -> Self {
    Self {
      min_width:  Some(40),
      max_width:  Some(70),
      min_height: Some(8),
      max_height: Some(22),
      padding:    UiInsets {
        left:   1,
        right:  1,
        top:    1,
        bottom: 1,
      },
      align:      UiAlignPair {
        horizontal: UiAlign::Center,
        vertical:   UiAlign::Center,
      },
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiLayer {
  Background,
  Overlay,
  Tooltip,
}

impl Default for UiLayer {
  fn default() -> Self {
    UiLayer::Overlay
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiColorToken {
  Text,
  MutedText,
  PanelBg,
  PanelBorder,
  Accent,
  SelectedBg,
  SelectedText,
  Divider,
  Placeholder,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum UiColor {
  Token(UiColorToken),
  Value(Color),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiStyle {
  pub fg:       Option<UiColor>,
  pub bg:       Option<UiColor>,
  pub border:   Option<UiColor>,
  pub accent:   Option<UiColor>,
  pub emphasis: UiEmphasis,
  pub radius:   UiRadius,
  #[serde(default)]
  pub role:     Option<String>,
}

impl Default for UiStyle {
  fn default() -> Self {
    Self {
      fg:       None,
      bg:       None,
      border:   None,
      accent:   None,
      emphasis: UiEmphasis::Normal,
      radius:   UiRadius::None,
      role:     None,
    }
  }
}

impl UiStyle {
  pub fn panel() -> Self {
    Self {
      fg:       Some(UiColor::Token(UiColorToken::Text)),
      bg:       Some(UiColor::Token(UiColorToken::PanelBg)),
      border:   Some(UiColor::Token(UiColorToken::PanelBorder)),
      accent:   None,
      emphasis: UiEmphasis::Normal,
      radius:   UiRadius::Small,
      role:     None,
    }
  }

  pub fn with_role(mut self, role: impl Into<String>) -> Self {
    self.role = Some(role.into());
    self
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiFocus {
  pub id:     String,
  pub kind:   UiFocusKind,
  pub cursor: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum UiFocusKind {
  Input,
  List,
  Panel,
  Custom(String),
}

impl UiFocus {
  pub fn input(id: impl Into<String>, cursor: Option<usize>) -> Self {
    Self {
      id: id.into(),
      kind: UiFocusKind::Input,
      cursor,
    }
  }

  pub fn list(id: impl Into<String>) -> Self {
    Self {
      id:     id.into(),
      kind:   UiFocusKind::List,
      cursor: None,
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct UiModifiers {
  pub ctrl:  bool,
  pub alt:   bool,
  pub shift: bool,
  pub meta:  bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum UiKey {
  Char(char),
  Enter,
  Escape,
  Tab,
  Backspace,
  Delete,
  Up,
  Down,
  Left,
  Right,
  Home,
  End,
  PageUp,
  PageDown,
  Unknown(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiKeyEvent {
  pub key:       UiKey,
  pub modifiers: UiModifiers,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum UiEventKind {
  Key(UiKeyEvent),
  Command(String),
  Activate,
  Dismiss,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiEvent {
  pub target: Option<String>,
  pub kind:   UiEventKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiEventOutcome {
  pub handled: bool,
  pub focus:   Option<UiFocus>,
}

impl UiEventOutcome {
  pub fn handled() -> Self {
    Self {
      handled: true,
      focus:   None,
    }
  }

  pub fn r#continue() -> Self {
    Self {
      handled: false,
      focus:   None,
    }
  }

  pub fn focus(focus: UiFocus) -> Self {
    Self {
      handled: true,
      focus:   Some(focus),
    }
  }
}

impl Default for UiEventOutcome {
  fn default() -> Self {
    Self::r#continue()
  }
}
