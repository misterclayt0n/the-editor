use crate::render::graphics::Color;

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone)]
pub struct UiPanel {
  pub id: String,
  pub title: Option<String>,
  pub intent: LayoutIntent,
  pub style: UiStyle,
  pub constraints: UiConstraints,
  pub layer: UiLayer,
  pub child: Box<UiNode>,
}

#[derive(Debug, Clone)]
pub struct UiText {
  pub id: Option<String>,
  pub content: String,
  pub style: UiStyle,
}

#[derive(Debug, Clone)]
pub struct UiList {
  pub id: String,
  pub items: Vec<UiListItem>,
  pub selected: Option<usize>,
  pub scroll: usize,
  pub fill_width: bool,
  pub style: UiStyle,
}

#[derive(Debug, Clone)]
pub struct UiListItem {
  pub title: String,
  pub subtitle: Option<String>,
  pub description: Option<String>,
  pub shortcut: Option<String>,
  pub badge: Option<String>,
  pub emphasis: bool,
}

#[derive(Debug, Clone)]
pub struct UiInput {
  pub id: String,
  pub value: String,
  pub placeholder: Option<String>,
  pub cursor: usize,
  pub style: UiStyle,
}

#[derive(Debug, Clone)]
pub struct UiDivider {
  pub id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct UiSpacer {
  pub id: Option<String>,
  pub size: u16,
}

#[derive(Debug, Clone)]
pub struct UiTooltip {
  pub id: Option<String>,
  pub target: Option<String>,
  pub placement: LayoutIntent,
  pub content: String,
  pub style: UiStyle,
}

#[derive(Debug, Clone)]
pub struct UiStatusBar {
  pub id: Option<String>,
  pub left: String,
  pub center: String,
  pub right: String,
  pub style: UiStyle,
}

#[derive(Debug, Clone, Copy)]
pub enum UiAxis {
  Horizontal,
  Vertical,
}

#[derive(Debug, Clone)]
pub enum UiLayout {
  Stack { axis: UiAxis, gap: u16 },
  Split { axis: UiAxis, ratios: Vec<u16> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LayoutIntent {
  Floating,
  Bottom,
  Top,
  SidebarLeft,
  SidebarRight,
  Fullscreen,
  Custom(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiEmphasis {
  Normal,
  Muted,
  Strong,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiRadius {
  None,
  Small,
  Medium,
  Large,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct UiAlignPair {
  pub horizontal: UiAlign,
  pub vertical: UiAlign,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct UiConstraints {
  pub min_width: Option<u16>,
  pub max_width: Option<u16>,
  pub min_height: Option<u16>,
  pub max_height: Option<u16>,
  pub padding: UiInsets,
  pub align: UiAlignPair,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiColor {
  Token(UiColorToken),
  Value(Color),
}

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiFocus {
  pub id: String,
  pub cursor: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct UiModifiers {
  pub ctrl: bool,
  pub alt: bool,
  pub shift: bool,
  pub meta: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiKeyEvent {
  pub key: UiKey,
  pub modifiers: UiModifiers,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UiEventKind {
  Key(UiKeyEvent),
  Command(String),
  Activate,
  Dismiss,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiEvent {
  pub target: Option<String>,
  pub kind: UiEventKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
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
