use std::collections::HashMap;

use crate::render::graphics::Color;
use serde::{
  Deserialize,
  Serialize,
};

fn default_clip() -> bool {
  true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiTree {
  pub root: UiNode,
  pub overlays: Vec<UiNode>,
  pub focus: Option<UiFocus>,
}

impl UiTree {
  pub fn new() -> Self {
    Self {
      root: UiNode::Container(UiContainer::default()),
      overlays: Vec::new(),
      focus: None,
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

  pub fn container(
    id: impl Into<String>,
    layout: UiLayout,
    children: Vec<UiNode>,
  ) -> Self {
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
  panels: HashMap<String, UiPanelState>,
  nodes:  HashMap<String, UiNodeState>,
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
    self.panels.get(id).map(|state| state.visible).unwrap_or(false)
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
  pub id: Option<String>,
  pub layout: UiLayout,
  pub children: Vec<UiNode>,
  pub style: UiStyle,
  pub constraints: UiConstraints,
}

impl Default for UiContainer {
  fn default() -> Self {
    Self {
      id: None,
      layout: UiLayout::Stack {
        axis: UiAxis::Vertical,
        gap: 0,
      },
      children: Vec::new(),
      style: UiStyle::default(),
      constraints: UiConstraints::default(),
    }
  }
}

impl UiContainer {
  pub fn new(
    id: impl Into<String>,
    layout: UiLayout,
    children: Vec<UiNode>,
  ) -> Self {
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
  pub id: String,
  pub title: Option<String>,
  pub intent: LayoutIntent,
  pub style: UiStyle,
  pub constraints: UiConstraints,
  pub layer: UiLayer,
  pub child: Box<UiNode>,
}

impl UiPanel {
  pub fn new(id: impl Into<String>, intent: LayoutIntent, child: UiNode) -> Self {
    Self {
      id: id.into(),
      title: None,
      intent,
      style: UiStyle::panel(),
      constraints: UiConstraints::panel(),
      layer: UiLayer::Overlay,
      child: Box::new(child),
    }
  }

  pub fn floating(id: impl Into<String>, child: UiNode) -> Self {
    Self {
      id: id.into(),
      title: None,
      intent: LayoutIntent::Floating,
      style: UiStyle::panel(),
      constraints: UiConstraints::floating_default(),
      layer: UiLayer::Overlay,
      child: Box::new(child),
    }
  }

  pub fn bottom(id: impl Into<String>, child: UiNode) -> Self {
    Self {
      id: id.into(),
      title: None,
      intent: LayoutIntent::Bottom,
      style: UiStyle::panel(),
      constraints: UiConstraints::panel(),
      layer: UiLayer::Overlay,
      child: Box::new(child),
    }
  }

  pub fn top(id: impl Into<String>, child: UiNode) -> Self {
    Self {
      id: id.into(),
      title: None,
      intent: LayoutIntent::Top,
      style: UiStyle::panel(),
      constraints: UiConstraints::panel(),
      layer: UiLayer::Overlay,
      child: Box::new(child),
    }
  }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiText {
  pub id: Option<String>,
  pub content: String,
  pub style: UiStyle,
  #[serde(default)]
  pub max_lines: Option<u16>,
  #[serde(default = "default_clip")]
  pub clip: bool,
}

impl UiText {
  pub fn new(id: impl Into<String>, content: impl Into<String>) -> Self {
    Self {
      id: Some(id.into()),
      content: content.into(),
      style: UiStyle::default(),
      max_lines: None,
      clip: true,
    }
  }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiList {
  pub id: String,
  pub items: Vec<UiListItem>,
  pub selected: Option<usize>,
  pub scroll: usize,
  pub fill_width: bool,
  pub style: UiStyle,
  #[serde(default)]
  pub max_visible: Option<usize>,
  #[serde(default = "default_clip")]
  pub clip: bool,
}

impl UiList {
  pub fn new(id: impl Into<String>, items: Vec<UiListItem>) -> Self {
    Self {
      id: id.into(),
      items,
      selected: None,
      scroll: 0,
      fill_width: true,
      style: UiStyle::default(),
      max_visible: None,
      clip: true,
    }
  }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiListItem {
  pub title: String,
  pub subtitle: Option<String>,
  pub description: Option<String>,
  pub shortcut: Option<String>,
  pub badge: Option<String>,
  pub emphasis: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiInput {
  pub id: String,
  pub value: String,
  pub placeholder: Option<String>,
  pub cursor: usize,
  pub style: UiStyle,
}

impl UiInput {
  pub fn new(id: impl Into<String>, value: impl Into<String>) -> Self {
    Self {
      id: id.into(),
      value: value.into(),
      placeholder: None,
      cursor: 0,
      style: UiStyle::default(),
    }
  }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiDivider {
  pub id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiSpacer {
  pub id: Option<String>,
  pub size: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiTooltip {
  pub id: Option<String>,
  pub target: Option<String>,
  pub placement: LayoutIntent,
  pub content: String,
  pub style: UiStyle,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiStatusBar {
  pub id: Option<String>,
  pub left: String,
  pub center: String,
  pub right: String,
  pub style: UiStyle,
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
  pub vertical: UiAlign,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct UiInsets {
  pub left: u16,
  pub right: u16,
  pub top: u16,
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
  pub min_width: Option<u16>,
  pub max_width: Option<u16>,
  pub min_height: Option<u16>,
  pub max_height: Option<u16>,
  pub padding: UiInsets,
  pub align: UiAlignPair,
}

impl UiConstraints {
  pub fn panel() -> Self {
    Self {
      padding: UiInsets {
        left: 1,
        right: 1,
        top: 1,
        bottom: 1,
      },
      ..Self::default()
    }
  }

  pub fn floating_default() -> Self {
    Self {
      min_width: Some(40),
      max_width: Some(70),
      min_height: Some(8),
      max_height: Some(22),
      padding: UiInsets {
        left: 1,
        right: 1,
        top: 1,
        bottom: 1,
      },
      align: UiAlignPair {
        horizontal: UiAlign::Center,
        vertical: UiAlign::Center,
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
  pub fg: Option<UiColor>,
  pub bg: Option<UiColor>,
  pub border: Option<UiColor>,
  pub accent: Option<UiColor>,
  pub emphasis: UiEmphasis,
  pub radius: UiRadius,
}

impl Default for UiStyle {
  fn default() -> Self {
    Self {
      fg: None,
      bg: None,
      border: None,
      accent: None,
      emphasis: UiEmphasis::Normal,
      radius: UiRadius::None,
    }
  }
}

impl UiStyle {
  pub fn panel() -> Self {
    Self {
      fg: Some(UiColor::Token(UiColorToken::Text)),
      bg: Some(UiColor::Token(UiColorToken::PanelBg)),
      border: Some(UiColor::Token(UiColorToken::PanelBorder)),
      accent: None,
      emphasis: UiEmphasis::Normal,
      radius: UiRadius::Small,
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiFocus {
  pub id: String,
  pub kind: UiFocusKind,
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
      id: id.into(),
      kind: UiFocusKind::List,
      cursor: None,
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct UiModifiers {
  pub ctrl: bool,
  pub alt: bool,
  pub shift: bool,
  pub meta: bool,
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
  pub key: UiKey,
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
  pub kind: UiEventKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiEventOutcome {
  pub handled: bool,
  pub focus: Option<UiFocus>,
}

impl UiEventOutcome {
  pub fn handled() -> Self {
    Self {
      handled: true,
      focus: None,
    }
  }

  pub fn r#continue() -> Self {
    Self {
      handled: false,
      focus: None,
    }
  }

  pub fn focus(focus: UiFocus) -> Self {
    Self {
      handled: true,
      focus: Some(focus),
    }
  }
}

impl Default for UiEventOutcome {
  fn default() -> Self {
    Self::r#continue()
  }
}
