use the_lib::{
  fuzzy::{
    MatchMode,
    fuzzy_match,
  },
  render::graphics::Color,
};

#[derive(Debug, Clone)]
pub struct CommandPaletteItem {
  pub title:       String,
  pub subtitle:    Option<String>,
  pub description: Option<String>,
  pub shortcut:    Option<String>,
  pub badge:       Option<String>,
  pub leading_icon: Option<String>,
  pub leading_color: Option<Color>,
  pub symbols:     Option<Vec<String>>,
  pub emphasis:    bool,
}

impl CommandPaletteItem {
  pub fn new(title: impl Into<String>) -> Self {
    Self {
      title:       title.into(),
      subtitle:    None,
      description: None,
      shortcut:    None,
      badge:       None,
      leading_icon: None,
      leading_color: None,
      symbols:     None,
      emphasis:    false,
    }
  }
}

#[derive(Debug, Clone)]
pub struct CommandPaletteState {
  pub is_open:     bool,
  pub query:       String,
  pub selected:    Option<usize>,
  pub items:       Vec<CommandPaletteItem>,
  pub max_results: usize,
}

impl Default for CommandPaletteState {
  fn default() -> Self {
    Self {
      is_open:     false,
      query:       String::new(),
      selected:    None,
      items:       Vec::new(),
      max_results: 10,
    }
  }
}

pub fn command_palette_filtered_indices(state: &CommandPaletteState) -> Vec<usize> {
  let mut filtered: Vec<usize> = if state.query.is_empty() {
    (0..state.items.len()).collect()
  } else {
    struct PaletteKey {
      index: usize,
      text:  String,
    }

    impl AsRef<str> for PaletteKey {
      fn as_ref(&self) -> &str {
        &self.text
      }
    }

    let keys: Vec<PaletteKey> = state
      .items
      .iter()
      .enumerate()
      .map(|(idx, item)| PaletteKey {
        index: idx,
        text:  item.title.clone(),
      })
      .collect();

    fuzzy_match(&state.query, keys.iter(), MatchMode::Plain)
      .into_iter()
      .map(|(key, _)| key.index)
      .collect()
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

pub fn command_palette_default_selected(state: &CommandPaletteState) -> Option<usize> {
  if state.query.is_empty() {
    None
  } else {
    command_palette_filtered_indices(state).first().copied()
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
  pub layout:         CommandPaletteLayout,
  pub theme:          CommandPaletteTheme,
}

impl Default for CommandPaletteStyle {
  fn default() -> Self {
    Self {
      layout:         CommandPaletteLayout::Floating,
      theme:          CommandPaletteTheme::default(),
    }
  }
}

impl CommandPaletteStyle {
  pub fn floating(theme: CommandPaletteTheme) -> Self {
    Self {
      layout:         CommandPaletteLayout::Floating,
      theme,
    }
  }

  pub fn bottom(theme: CommandPaletteTheme) -> Self {
    Self {
      layout:         CommandPaletteLayout::Bottom,
      theme,
    }
  }

  pub fn top(theme: CommandPaletteTheme) -> Self {
    Self {
      layout:         CommandPaletteLayout::Top,
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
