use std::collections::HashMap;

use the_lib::{
  command_line::split,
  fuzzy::{
    MatchMode,
    fuzzy_match,
  },
  render::graphics::Color,
};

use crate::{
  Command,
  DefaultContext,
  Mode,
  NamedActionHandle,
};

#[derive(Debug, Clone)]
pub enum CommandPaletteAction {
  StaticCommand(Command),
  NamedAction(String),
  NamedActionHandle(NamedActionHandle),
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

  pub fn subtitle(mut self, subtitle: impl Into<String>) -> Self {
    self.subtitle = Some(subtitle.into());
    self
  }

  pub fn description(mut self, description: impl Into<String>) -> Self {
    self.description = Some(description.into());
    self
  }

  pub fn alias(mut self, alias: impl Into<String>) -> Self {
    self.aliases.push(alias.into());
    self
  }

  pub fn aliases<I, S>(mut self, aliases: I) -> Self
  where
    I: IntoIterator<Item = S>,
    S: Into<String>,
  {
    self.aliases = aliases.into_iter().map(Into::into).collect();
    self
  }

  pub fn shortcut(mut self, shortcut: impl Into<String>) -> Self {
    self.shortcut = Some(shortcut.into());
    self
  }

  pub fn badge(mut self, badge: impl Into<String>) -> Self {
    self.badge = Some(badge.into());
    self
  }

  pub fn leading_icon(mut self, icon: impl Into<String>) -> Self {
    self.leading_icon = Some(icon.into());
    self
  }

  pub fn leading_color(mut self, color: Color) -> Self {
    self.leading_color = Some(color);
    self
  }

  pub fn symbols<I, S>(mut self, symbols: I) -> Self
  where
    I: IntoIterator<Item = S>,
    S: Into<String>,
  {
    self.symbols = Some(symbols.into_iter().map(Into::into).collect());
    self
  }

  pub fn emphasize(mut self) -> Self {
    self.emphasis = true;
    self
  }

  pub fn on_static_command(mut self, command: Command) -> Self {
    self.action = Some(CommandPaletteAction::StaticCommand(command));
    self
  }

  pub fn on_named_action(mut self, name: impl Into<String>) -> Self {
    self.action = Some(CommandPaletteAction::NamedAction(name.into()));
    self
  }

  pub fn on_named_action_handle(mut self, handle: NamedActionHandle) -> Self {
    self.action = Some(CommandPaletteAction::NamedActionHandle(handle));
    self
  }

  pub fn on_typable_command(mut self, name: impl Into<String>, args: impl Into<String>) -> Self {
    self.action = Some(CommandPaletteAction::TypableCommand {
      name: name.into(),
      args: args.into(),
    });
    self
  }
}

#[derive(Debug, Clone)]
pub struct CommandPaletteState {
  pub is_open:       bool,
  pub source:        CommandPaletteSource,
  pub source_mode:   Mode,
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

impl Default for CommandPaletteState {
  fn default() -> Self {
    Self {
      is_open:       false,
      source:        CommandPaletteSource::CommandLine,
      source_mode:   Mode::Command,
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
  use crate::{
    Mode,
    NamedActionHandle,
  };

  #[test]
  fn alias_exact_match_is_ranked_above_title_fuzzy_match() {
    let watch = CommandPaletteItem::new("watch-conflict");
    let mut write = CommandPaletteItem::new("write");
    write.aliases = vec!["w".to_string()];

    let state = CommandPaletteState {
      is_open:       true,
      source:        CommandPaletteSource::CommandLine,
      source_mode:   Mode::Command,
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
      source_mode:   Mode::Command,
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

  #[test]
  fn item_builder_sets_palette_metadata_and_action() {
    let item = CommandPaletteItem::new("Open Demo")
      .subtitle("Demo")
      .description("Open the demo surface")
      .aliases(["demo.open", "demo"])
      .shortcut("g p")
      .badge("custom")
      .leading_icon("sparkles")
      .emphasize()
      .on_named_action("demo.open");

    assert_eq!(item.subtitle.as_deref(), Some("Demo"));
    assert_eq!(item.description.as_deref(), Some("Open the demo surface"));
    assert_eq!(item.aliases, vec!["demo.open", "demo"]);
    assert_eq!(item.shortcut.as_deref(), Some("g p"));
    assert_eq!(item.badge.as_deref(), Some("custom"));
    assert_eq!(item.leading_icon.as_deref(), Some("sparkles"));
    assert!(item.emphasis);
    assert!(matches!(
      item.action,
      Some(super::CommandPaletteAction::NamedAction(ref name)) if name == "demo.open"
    ));
  }

  #[test]
  fn item_builder_supports_named_action_handles() {
    let handle = NamedActionHandle::new("demo.handle");
    let item = CommandPaletteItem::new("Handle Demo").on_named_action_handle(handle);

    assert!(matches!(
      item.action,
      Some(super::CommandPaletteAction::NamedActionHandle(bound)) if bound == handle
    ));
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
