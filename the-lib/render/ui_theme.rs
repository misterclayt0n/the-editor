use std::collections::HashSet;

use super::{
  graphics::{
    Color,
    Style,
  },
  theme::Theme,
  ui::{
    UiColor,
    UiColorToken,
    UiFocus,
    UiNode,
    UiStyle,
    UiTree,
  },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UiComponent {
  Container,
  Panel,
  Text,
  List,
  Input,
  Divider,
  Spacer,
  Tooltip,
  StatusBar,
}

impl UiComponent {
  pub fn as_str(self) -> &'static str {
    match self {
      UiComponent::Container => "container",
      UiComponent::Panel => "panel",
      UiComponent::Text => "text",
      UiComponent::List => "list",
      UiComponent::Input => "input",
      UiComponent::Divider => "divider",
      UiComponent::Spacer => "spacer",
      UiComponent::Tooltip => "tooltip",
      UiComponent::StatusBar => "status_bar",
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UiStyleProp {
  Fg,
  Bg,
  Border,
  Accent,
}

impl UiStyleProp {
  pub fn as_str(self) -> &'static str {
    match self {
      UiStyleProp::Fg => "fg",
      UiStyleProp::Bg => "bg",
      UiStyleProp::Border => "border",
      UiStyleProp::Accent => "accent",
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UiState {
  Focused,
  Selected,
  Hovered,
  Disabled,
}

impl UiState {
  pub fn as_str(self) -> &'static str {
    match self {
      UiState::Focused => "focused",
      UiState::Selected => "selected",
      UiState::Hovered => "hovered",
      UiState::Disabled => "disabled",
    }
  }
}

pub fn build_ui_scopes(
  role: Option<&str>,
  component: UiComponent,
  states: &[UiState],
  prop: UiStyleProp,
) -> Vec<String> {
  let mut out = Vec::new();
  let mut seen = HashSet::new();
  let prop = prop.as_str();
  let component = component.as_str();

  let states: Vec<&'static str> = states.iter().map(|state| state.as_str()).collect();

  if let Some(role) = role {
    let role_base = format!("ui.{role}");
    let role_component = format!("{role_base}.{component}");
    push_scopes(&mut out, &mut seen, &role_component, &states, prop);
    push_scopes(&mut out, &mut seen, &role_base, &states, prop);
  }

  let component_base = format!("ui.{component}");
  push_scopes(&mut out, &mut seen, &component_base, &states, prop);

  push_scope(&mut out, &mut seen, format!("ui.{prop}"));

  for scope in legacy_scopes_for_component(component, &states, prop) {
    push_scope(&mut out, &mut seen, scope);
  }

  out
}

pub fn resolve_ui_tree(tree: &mut UiTree, theme: &Theme) {
  let focus = tree.focus.as_ref();
  resolve_node(&mut tree.root, theme, focus, None);
  for overlay in &mut tree.overlays {
    resolve_node(overlay, theme, focus, None);
  }
}

fn resolve_node(
  node: &mut UiNode,
  theme: &Theme,
  focus: Option<&UiFocus>,
  inherited_role: Option<&str>,
) {
  match node {
    UiNode::Container(container) => {
      let states = states_for(focus, container.id.as_deref());
      let role_value = container.style.role.clone();
      let role = role_value.as_deref().or(inherited_role);
      resolve_style(
        &mut container.style,
        theme,
        role,
        UiComponent::Container,
        &states,
      );
      let child_role = container.style.role.as_deref().or(inherited_role);
      for child in &mut container.children {
        resolve_node(child, theme, focus, child_role);
      }
    },
    UiNode::Panel(panel) => {
      let states = states_for(focus, Some(panel.id.as_str()));
      let role_value = panel.style.role.clone();
      let role = role_value.as_deref().or(inherited_role);
      resolve_style(&mut panel.style, theme, role, UiComponent::Panel, &states);
      let child_role = panel.style.role.as_deref().or(inherited_role);
      resolve_node(&mut panel.child, theme, focus, child_role);
    },
    UiNode::Text(text) => {
      let states = states_for(focus, text.id.as_deref());
      let role_value = text.style.role.clone();
      let role = role_value.as_deref().or(inherited_role);
      resolve_style(&mut text.style, theme, role, UiComponent::Text, &states);
    },
    UiNode::List(list) => {
      let states = states_for(focus, Some(list.id.as_str()));
      let role_value = list.style.role.clone();
      let role = role_value.as_deref().or(inherited_role);
      resolve_style(&mut list.style, theme, role, UiComponent::List, &states);
    },
    UiNode::Input(input) => {
      let states = states_for(focus, Some(input.id.as_str()));
      let role_value = input.style.role.clone();
      let role = role_value.as_deref().or(inherited_role);
      resolve_style(&mut input.style, theme, role, UiComponent::Input, &states);
    },
    UiNode::Tooltip(tooltip) => {
      let states = states_for(focus, tooltip.id.as_deref());
      let role_value = tooltip.style.role.clone();
      let role = role_value.as_deref().or(inherited_role);
      resolve_style(
        &mut tooltip.style,
        theme,
        role,
        UiComponent::Tooltip,
        &states,
      );
    },
    UiNode::StatusBar(status_bar) => {
      let states = states_for(focus, status_bar.id.as_deref());
      let role_value = status_bar.style.role.clone();
      let role = role_value.as_deref().or(inherited_role);
      resolve_style(
        &mut status_bar.style,
        theme,
        role,
        UiComponent::StatusBar,
        &states,
      );
    },
    UiNode::Divider(_) | UiNode::Spacer(_) => {},
  }
}

fn states_for(focus: Option<&UiFocus>, id: Option<&str>) -> Vec<UiState> {
  let mut states = Vec::new();
  if is_focused(focus, id) {
    states.push(UiState::Focused);
  }
  states
}

fn is_focused(focus: Option<&UiFocus>, id: Option<&str>) -> bool {
  match (focus, id) {
    (Some(focus), Some(id)) => focus.id == id,
    _ => false,
  }
}

fn resolve_style(
  style: &mut UiStyle,
  theme: &Theme,
  role: Option<&str>,
  component: UiComponent,
  states: &[UiState],
) {
  resolve_color_slot(
    &mut style.fg,
    theme,
    role,
    component,
    states,
    UiStyleProp::Fg,
  );
  resolve_color_slot(
    &mut style.bg,
    theme,
    role,
    component,
    states,
    UiStyleProp::Bg,
  );
  let skip_border =
    matches!(component, UiComponent::Panel) && role == Some("statusline") && style.border.is_none();
  if !skip_border {
    resolve_color_slot(
      &mut style.border,
      theme,
      role,
      component,
      states,
      UiStyleProp::Border,
    );
  }
  resolve_color_slot(
    &mut style.accent,
    theme,
    role,
    component,
    states,
    UiStyleProp::Accent,
  );
}

fn resolve_color_slot(
  slot: &mut Option<UiColor>,
  theme: &Theme,
  role: Option<&str>,
  component: UiComponent,
  states: &[UiState],
  prop: UiStyleProp,
) {
  let token = match slot {
    Some(UiColor::Token(token)) => Some(*token),
    Some(UiColor::Value(_)) => return,
    None => None,
  };

  if let Some(color) = resolve_from_scopes(theme, role, component, states, prop) {
    *slot = Some(UiColor::Value(color));
    return;
  }

  if let Some(token) = token {
    if let Some(color) = resolve_from_token(theme, token, prop) {
      *slot = Some(UiColor::Value(color));
    }
  }
}

fn resolve_from_scopes(
  theme: &Theme,
  role: Option<&str>,
  component: UiComponent,
  states: &[UiState],
  prop: UiStyleProp,
) -> Option<Color> {
  let scopes = build_ui_scopes(role, component, states, prop);
  for scope in &scopes {
    if let Some(style) = theme.try_get(scope) {
      if let Some(color) = color_from_style(style, prop) {
        return Some(color);
      }
    }
  }
  None
}

fn resolve_from_token(theme: &Theme, token: UiColorToken, prop: UiStyleProp) -> Option<Color> {
  for scope in token_scopes(token) {
    if let Some(style) = theme.try_get(scope) {
      if let Some(color) = color_from_style(style, prop) {
        return Some(color);
      }
    }
  }
  None
}

fn color_from_style(style: Style, prop: UiStyleProp) -> Option<Color> {
  match prop {
    UiStyleProp::Fg => style.fg,
    UiStyleProp::Bg => style.bg,
    UiStyleProp::Border | UiStyleProp::Accent => style.fg.or(style.bg),
  }
}

fn token_scopes(token: UiColorToken) -> &'static [&'static str] {
  match token {
    UiColorToken::Text => &["ui.text", "ui.text.focus"],
    UiColorToken::MutedText => &["ui.text.inactive", "ui.text", "ui.virtual"],
    UiColorToken::PanelBg => &["ui.popup", "ui.background"],
    UiColorToken::PanelBorder => &["ui.window", "ui.background.separator"],
    UiColorToken::Accent => &["ui.text.focus", "ui.linenr.selected", "ui.text"],
    UiColorToken::SelectedBg => &["ui.menu.selected", "ui.selection", "ui.menu"],
    UiColorToken::SelectedText => &["ui.menu.selected", "ui.text.focus", "ui.text"],
    UiColorToken::Divider => &["ui.background.separator", "ui.window"],
    UiColorToken::Placeholder => &["ui.text.inactive", "ui.virtual", "ui.linenr"],
  }
}

fn push_scopes(
  out: &mut Vec<String>,
  seen: &mut HashSet<String>,
  base: &str,
  states: &[&'static str],
  prop: &str,
) {
  for state in states {
    push_scope(out, seen, format!("{base}.{state}.{prop}"));
    push_scope(out, seen, format!("{base}.{state}"));
  }
  push_scope(out, seen, format!("{base}.{prop}"));
  push_scope(out, seen, base.to_string());
}

fn push_scope(out: &mut Vec<String>, seen: &mut HashSet<String>, scope: String) {
  if seen.insert(scope.clone()) {
    out.push(scope);
  }
}

fn legacy_scopes_for_component(
  component: &str,
  states: &[&'static str],
  prop: &str,
) -> Vec<String> {
  let mut scopes = Vec::new();
  let is_selected = states.iter().any(|state| *state == "selected");
  let is_focused = states.iter().any(|state| *state == "focused");

  match component {
    "panel" => {
      if prop == "bg" {
        scopes.push("ui.popup".to_string());
        scopes.push("ui.background".to_string());
      }
      if prop == "border" {
        scopes.push("ui.window".to_string());
      }
      if prop == "fg" {
        scopes.push("ui.text".to_string());
      }
    },
    "text" => {
      scopes.push("ui.text".to_string());
    },
    "input" => {
      if prop == "bg" {
        scopes.push("ui.popup".to_string());
      }
      if prop == "border" {
        scopes.push("ui.window".to_string());
      }
      if prop == "fg" {
        scopes.push(if is_focused {
          "ui.text.focus".to_string()
        } else {
          "ui.text".to_string()
        });
      }
    },
    "list" => {
      if is_selected {
        scopes.push("ui.menu.selected".to_string());
        scopes.push("ui.selection".to_string());
      }
      if prop == "bg" {
        scopes.push("ui.menu".to_string());
      }
      if prop == "fg" {
        scopes.push("ui.text".to_string());
      }
    },
    "tooltip" => {
      scopes.push("ui.help".to_string());
    },
    "status_bar" => {
      scopes.push("ui.statusline".to_string());
    },
    "divider" => {
      scopes.push("ui.background.separator".to_string());
    },
    _ => {},
  }

  if prop == "accent" {
    scopes.push("ui.text.focus".to_string());
    scopes.push("ui.linenr.selected".to_string());
  }

  scopes
}
