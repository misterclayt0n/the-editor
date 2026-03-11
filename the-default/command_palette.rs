use std::collections::HashMap;

use the_core::chars::byte_to_char_idx;
use the_lib::{
  command_line::split,
  fuzzy::{
    MatchMode,
    fuzzy_match,
  },
  render::{
    LayoutIntent,
    UiColor,
    UiConstraints,
    UiContainer,
    UiDivider,
    UiInput,
    UiList,
    UiListItem,
    UiNode,
    UiPanel,
    graphics::Color,
  },
};

use crate::{
  Command,
  DefaultContext,
};

#[derive(Debug, Clone)]
pub enum CommandPaletteAction {
  StaticCommand(Command),
  TypableCommand { name: String, args: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandPaletteSource {
  CommandLine,
  ActionPalette,
}

#[derive(Debug, Clone)]
pub struct CommandPaletteItem {
  pub title:         String,
  pub subtitle:      Option<String>,
  pub description:   Option<String>,
  pub aliases:       Vec<String>,
  pub shortcut:      Option<String>,
  pub badge:         Option<String>,
  pub leading_icon:  Option<String>,
  pub leading_color: Option<Color>,
  pub symbols:       Option<Vec<String>>,
  pub emphasis:      bool,
  pub action:        Option<CommandPaletteAction>,
}

impl CommandPaletteItem {
  pub fn new(title: impl Into<String>) -> Self {
    Self {
      title:         title.into(),
      subtitle:      None,
      description:   None,
      aliases:       Vec::new(),
      shortcut:      None,
      badge:         None,
      leading_icon:  None,
      leading_color: None,
      symbols:       None,
      emphasis:      false,
      action:        None,
    }
  }
}

#[derive(Debug, Clone)]
pub struct CommandPaletteState {
  pub is_open:       bool,
  pub source:        CommandPaletteSource,
  pub query:         String,
  pub selected:      Option<usize>,
  pub items:         Vec<CommandPaletteItem>,
  pub max_results:   usize,
  /// When true, items are already filtered (e.g. argument completions).
  /// Renderers should use `query` as-is instead of overriding from the prompt.
  pub prefiltered:   bool,
  pub scroll_offset: usize,
  /// Display text for the prompt in prefiltered mode (e.g. ":open src/").
  /// When set, `build_command_palette_ui` uses this as the input value.
  pub prompt_text:   Option<String>,
}

fn active_command_line(state: &CommandPaletteState) -> Option<&str> {
  if !matches!(state.source, CommandPaletteSource::CommandLine) {
    return None;
  }

  let line = state.prompt_text.as_deref().unwrap_or(state.query.as_str());
  let line = line.trim().trim_start_matches(':');
  if line.is_empty() {
    return None;
  }

  let (command, _, complete_command_name) = split(line);
  if command.is_empty() || complete_command_name {
    None
  } else {
    Some(command)
  }
}

fn humanize_command_name(name: &str) -> String {
  let mut words = Vec::new();
  for word in name.split('-').filter(|word| !word.is_empty()) {
    let mut chars = word.chars();
    let Some(first) = chars.next() else {
      continue;
    };

    let mut title = String::new();
    title.extend(first.to_uppercase());
    title.push_str(chars.as_str());
    words.push(title);
  }

  if words.is_empty() {
    "Command".to_string()
  } else {
    words.join(" ")
  }
}

fn argument_placeholder_for_command<Ctx>(command: &crate::TypableCommand<Ctx>) -> String
where
  Ctx: 'static,
{
  command
    .palette_placeholder
    .map(str::to_string)
    .unwrap_or_else(|| format!("{}…", humanize_command_name(command.name)))
}

pub fn command_palette_placeholder_text<Ctx: DefaultContext>(ctx: &Ctx) -> String {
  let state = ctx.command_palette();
  match state.source {
    CommandPaletteSource::ActionPalette => "Search commands…".to_string(),
    CommandPaletteSource::CommandLine => {
      if !state.prefiltered {
        return "Execute a command…".to_string();
      }

      let Some(command_name) = active_command_line(state) else {
        return "Execute a command…".to_string();
      };

      ctx
        .command_registry_ref()
        .get(command_name)
        .map(argument_placeholder_for_command)
        .unwrap_or_else(|| format!("{}…", humanize_command_name(command_name)))
    },
  }
}

impl From<CommandPaletteLayout> for LayoutIntent {
  fn from(layout: CommandPaletteLayout) -> Self {
    match layout {
      CommandPaletteLayout::Floating => LayoutIntent::Floating,
      CommandPaletteLayout::Bottom => LayoutIntent::Bottom,
      CommandPaletteLayout::Top => LayoutIntent::Top,
      CommandPaletteLayout::Custom => LayoutIntent::Custom("command_palette".to_string()),
    }
  }
}

pub fn build_command_palette_ui<Ctx: DefaultContext>(ctx: &mut Ctx) -> Vec<UiNode> {
  let placeholder = command_palette_placeholder_text(ctx);
  let state = ctx.command_palette();
  if !state.is_open {
    return Vec::new();
  }

  let palette_style = ctx.command_palette_style();
  let layout = palette_style.layout;
  let theme = palette_style.theme;
  let ui_theme = ctx.ui_theme();
  let selected_scope = ui_theme
    .try_get("ui.command_palette.list.selected")
    .or_else(|| ui_theme.try_get("ui.menu.selected"));
  let selected_bg = selected_scope
    .and_then(|style| style.bg)
    .unwrap_or(theme.selected_bg);
  let selected_text = selected_scope
    .and_then(|style| style.fg)
    .unwrap_or(theme.selected_text);
  let placeholder_color = ui_theme
    .try_get("ui.text.inactive")
    .and_then(|style| style.fg)
    .or_else(|| ui_theme.try_get("ui.virtual").and_then(|style| style.fg))
    .or_else(|| ui_theme.try_get("ui.linenr").and_then(|style| style.fg))
    .unwrap_or(theme.placeholder);

  let filtered = command_palette_filtered_indices(state);
  let selected = command_palette_selected_filtered_index(state);

  let mut items = Vec::with_capacity(filtered.len());
  for &idx in &filtered {
    let item = &state.items[idx];
    let mut description = item.description.clone();
    if !item.aliases.is_empty() {
      let aliases = format!("aliases: {}", item.aliases.join(", "));
      description = match description {
        Some(desc) if !desc.is_empty() => Some(format!("{desc} ({aliases})")),
        _ => Some(format!("({aliases})")),
      };
    }
    items.push(UiListItem {
      title: item.title.clone(),
      subtitle: item.subtitle.clone(),
      description,
      shortcut: item.shortcut.clone(),
      badge: item.badge.clone(),
      leading_icon: item.leading_icon.clone(),
      leading_color: item.leading_color.map(UiColor::Value),
      symbols: item.symbols.clone(),
      match_indices: None,
      emphasis: item.emphasis,
      action: Some(item.title.clone()),
    });
  }

  let display_value = state.prompt_text.clone().unwrap_or_else(|| {
    match state.source {
      CommandPaletteSource::CommandLine => {
        if state.query.is_empty() {
          String::new()
        } else {
          format!(":{}", state.query)
        }
      },
      CommandPaletteSource::ActionPalette => state.query.clone(),
    }
  });
  let mut input = UiInput::new("command_palette_input", display_value.clone());
  input.style = input.style.with_role("command_palette");
  input.style.accent = Some(UiColor::Value(placeholder_color));
  input.placeholder = match state.source {
    CommandPaletteSource::ActionPalette => Some(placeholder),
    CommandPaletteSource::CommandLine => {
      if state.prefiltered {
        Some(placeholder)
      } else {
        Some(format!(":{placeholder}"))
      }
    },
  };
  input.cursor = match state.source {
    CommandPaletteSource::CommandLine => {
      if display_value.is_empty() {
        1
      } else {
        byte_to_char_idx(&display_value, display_value.len()) + 1
      }
    },
    CommandPaletteSource::ActionPalette => {
      if display_value.is_empty() {
        0
      } else {
        byte_to_char_idx(&display_value, display_value.len())
      }
    },
  };
  let input = UiNode::Input(input);

  let mut list = UiList::new("command_palette_list", items);
  list.selected = selected;
  list.style = list.style.with_role("command_palette");
  list.style.accent = Some(UiColor::Value(selected_bg));
  list.style.border = Some(UiColor::Value(selected_text));
  let list = UiNode::List(list);

  let children = if matches!(layout, CommandPaletteLayout::Bottom) {
    vec![list, UiNode::Divider(UiDivider { id: None }), input]
  } else {
    vec![input, UiNode::Divider(UiDivider { id: None }), list]
  };

  let mut container = UiContainer::column("command_palette_container", 0, children);
  container.style = container.style.with_role("command_palette");
  let container = UiNode::Container(container);

  let intent: LayoutIntent = layout.into();

  let mut overlays = Vec::new();
  let mut panel = match layout {
    CommandPaletteLayout::Floating => UiPanel::floating("command_palette", container),
    CommandPaletteLayout::Bottom => UiPanel::bottom("command_palette", container),
    CommandPaletteLayout::Top => UiPanel::top("command_palette", container),
    CommandPaletteLayout::Custom => UiPanel::new("command_palette", intent.clone(), container),
  };
  panel.style = panel.style.with_role("command_palette");
  panel.style.border = None;
  if matches!(layout, CommandPaletteLayout::Floating) {
    panel.constraints = UiConstraints::floating_default();
    panel.constraints.padding.top = 0;
    panel.constraints.padding.bottom = 0;
    panel.constraints.min_height = Some(12);
  }
  overlays.push(UiNode::Panel(panel));

  overlays
}

impl Default for CommandPaletteState {
  fn default() -> Self {
    Self {
      is_open:       false,
      source:        CommandPaletteSource::CommandLine,
      query:         String::new(),
      selected:      None,
      items:         Vec::new(),
      max_results:   usize::MAX,
      prefiltered:   false,
      scroll_offset: 0,
      prompt_text:   None,
    }
  }
}

pub fn command_palette_filtered_indices(state: &CommandPaletteState) -> Vec<usize> {
  let mut filtered: Vec<usize> = if state.query.is_empty() {
    (0..state.items.len()).collect()
  } else {
    struct PaletteKey {
      index:      usize,
      text:       String,
      alias_rank: u8,
    }

    impl AsRef<str> for PaletteKey {
      fn as_ref(&self) -> &str {
        &self.text
      }
    }

    let query_lower = state.query.to_lowercase();
    let keys: Vec<PaletteKey> = state
      .items
      .iter()
      .enumerate()
      .flat_map(|(idx, item)| {
        let mut keys = Vec::with_capacity(1 + item.aliases.len());
        keys.push(PaletteKey {
          index:      idx,
          text:       item.title.clone(),
          alias_rank: 0,
        });
        for alias in &item.aliases {
          let alias_lower = alias.to_lowercase();
          let alias_rank = if alias_lower == query_lower {
            3
          } else if alias_lower.starts_with(&query_lower) {
            2
          } else {
            1
          };
          keys.push(PaletteKey {
            index: idx,
            text: alias.clone(),
            alias_rank,
          });
        }
        keys
      })
      .collect();

    let mut best_per_index: HashMap<usize, (u8, u16, usize)> = HashMap::new();
    for (order, (key, score)) in fuzzy_match(&state.query, keys.iter(), MatchMode::Plain)
      .into_iter()
      .enumerate()
    {
      let candidate = (key.alias_rank, score, order);
      best_per_index
        .entry(key.index)
        .and_modify(|current| {
          if candidate.0 > current.0
            || (candidate.0 == current.0 && candidate.1 > current.1)
            || (candidate.0 == current.0 && candidate.1 == current.1 && candidate.2 < current.2)
          {
            *current = candidate;
          }
        })
        .or_insert(candidate);
    }

    let mut ranked: Vec<(usize, u8, u16, usize)> = best_per_index
      .into_iter()
      .map(|(index, (alias_rank, score, order))| (index, alias_rank, score, order))
      .collect();
    ranked.sort_by(|left, right| {
      right
        .1
        .cmp(&left.1)
        .then_with(|| right.2.cmp(&left.2))
        .then_with(|| left.3.cmp(&right.3))
        .then_with(|| left.0.cmp(&right.0))
    });
    ranked.into_iter().map(|(index, ..)| index).collect()
  };

  if filtered.len() > state.max_results {
    filtered.truncate(state.max_results);
  }

  filtered
}

pub fn command_palette_selected_filtered_index(state: &CommandPaletteState) -> Option<usize> {
  let selected = state.selected?;
  let filtered = command_palette_filtered_indices(state);
  filtered.iter().position(|&idx| idx == selected)
}

pub fn command_palette_default_selected(_state: &CommandPaletteState) -> Option<usize> {
  None
}

#[cfg(test)]
mod tests {
  use super::{
    CommandPaletteItem,
    CommandPaletteSource,
    CommandPaletteState,
    command_palette_default_selected,
    command_palette_filtered_indices,
  };

  #[test]
  fn alias_exact_match_is_ranked_above_title_fuzzy_match() {
    let watch = CommandPaletteItem::new("watch-conflict");
    let mut write = CommandPaletteItem::new("write");
    write.aliases = vec!["w".to_string()];

    let state = CommandPaletteState {
      is_open:       true,
      source:        CommandPaletteSource::CommandLine,
      query:         "w".to_string(),
      selected:      None,
      items:         vec![watch, write],
      max_results:   10,
      prefiltered:   false,
      scroll_offset: 0,
      prompt_text:   None,
    };

    let filtered = command_palette_filtered_indices(&state);
    assert_eq!(filtered.first().copied(), Some(1));
  }

  #[test]
  fn default_selected_is_none_even_with_query_matches() {
    let state = CommandPaletteState {
      is_open:       true,
      source:        CommandPaletteSource::CommandLine,
      query:         "w".to_string(),
      selected:      None,
      items:         vec![CommandPaletteItem::new("write")],
      max_results:   10,
      prefiltered:   false,
      scroll_offset: 0,
      prompt_text:   None,
    };

    assert_eq!(command_palette_default_selected(&state), None);
  }
}

#[derive(Debug, Clone, Copy)]
pub enum CommandPaletteLayout {
  Floating,
  Bottom,
  Top,
  Custom,
}

#[derive(Debug, Clone, Copy)]
pub struct CommandPaletteTheme {
  pub panel_bg:        Color,
  pub panel_border:    Color,
  pub divider:         Color,
  pub text:            Color,
  pub placeholder:     Color,
  pub selected_bg:     Color,
  pub selected_text:   Color,
  pub selected_border: Color,
}

pub struct CommandPaletteStyle {
  pub layout: CommandPaletteLayout,
  pub theme:  CommandPaletteTheme,
}

impl Default for CommandPaletteStyle {
  fn default() -> Self {
    Self {
      layout: CommandPaletteLayout::Floating,
      theme:  CommandPaletteTheme::default(),
    }
  }
}

impl CommandPaletteStyle {
  pub fn floating(theme: CommandPaletteTheme) -> Self {
    Self {
      layout: CommandPaletteLayout::Floating,
      theme,
    }
  }

  pub fn bottom(theme: CommandPaletteTheme) -> Self {
    Self {
      layout: CommandPaletteLayout::Bottom,
      theme,
    }
  }

  pub fn top(theme: CommandPaletteTheme) -> Self {
    Self {
      layout: CommandPaletteLayout::Top,
      theme,
    }
  }

  pub fn helix_bottom() -> Self {
    Self::bottom(CommandPaletteTheme::helix())
  }
}

impl Default for CommandPaletteTheme {
  fn default() -> Self {
    Self {
      panel_bg:        Color::Rgb(24, 24, 24),
      panel_border:    Color::Rgb(60, 60, 60),
      divider:         Color::Rgb(45, 45, 45),
      text:            Color::Rgb(220, 220, 220),
      placeholder:     Color::Rgb(140, 140, 140),
      selected_bg:     Color::Rgb(45, 60, 100),
      selected_text:   Color::Rgb(235, 235, 235),
      selected_border: Color::Rgb(70, 90, 140),
    }
  }
}

impl CommandPaletteTheme {
  pub fn helix() -> Self {
    Self {
      panel_bg:        Color::Rgb(20, 22, 28),
      panel_border:    Color::Rgb(40, 44, 52),
      divider:         Color::Rgb(45, 48, 56),
      text:            Color::Rgb(220, 220, 220),
      placeholder:     Color::Rgb(140, 140, 140),
      selected_bg:     Color::Rgb(55, 70, 110),
      selected_text:   Color::Rgb(235, 235, 235),
      selected_border: Color::Rgb(70, 90, 140),
    }
  }

  pub fn ghostty() -> Self {
    Self {
      panel_bg:        Color::Rgb(24, 24, 24),
      panel_border:    Color::Rgb(56, 56, 56),
      divider:         Color::Rgb(44, 44, 44),
      text:            Color::Rgb(228, 228, 228),
      placeholder:     Color::Rgb(150, 150, 150),
      selected_bg:     Color::Rgb(40, 58, 92),
      selected_text:   Color::Rgb(240, 240, 240),
      selected_border: Color::Rgb(82, 110, 168),
    }
  }
}
