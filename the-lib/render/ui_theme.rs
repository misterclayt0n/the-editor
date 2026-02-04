use std::collections::HashSet;

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
    }
    "text" => {
      scopes.push("ui.text".to_string());
    }
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
    }
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
    }
    "tooltip" => {
      scopes.push("ui.help".to_string());
    }
    "status_bar" => {
      scopes.push("ui.statusline".to_string());
    }
    "divider" => {
      scopes.push("ui.background.separator".to_string());
    }
    _ => {}
  }

  if prop == "accent" {
    scopes.push("ui.text.focus".to_string());
    scopes.push("ui.linenr.selected".to_string());
  }

  scopes
}
