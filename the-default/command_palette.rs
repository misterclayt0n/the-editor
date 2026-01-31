use the_lib::{
  fuzzy::{
    MatchMode,
    fuzzy_match,
  },
  position::Position,
  render::{
    OverlayNode,
    OverlayRect,
    OverlayRectKind,
    OverlayText,
    graphics::{
      Color,
      Rect,
      Style,
    },
  },
};

#[derive(Debug, Clone)]
pub struct CommandPaletteItem {
  pub title:       String,
  pub subtitle:    Option<String>,
  pub description: Option<String>,
  pub shortcut:    Option<String>,
  pub badge:       Option<String>,
}

impl CommandPaletteItem {
  pub fn new(title: impl Into<String>) -> Self {
    Self {
      title:       title.into(),
      subtitle:    None,
      description: None,
      shortcut:    None,
      badge:       None,
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
}

pub fn build_command_palette_overlay(
  state: &CommandPaletteState,
  viewport: Rect,
) -> Vec<OverlayNode> {
  build_command_palette_overlay_with_theme(state, viewport, CommandPaletteTheme::default())
}

pub fn build_command_palette_overlay_with_theme(
  state: &CommandPaletteState,
  viewport: Rect,
  theme: CommandPaletteTheme,
) -> Vec<OverlayNode> {
  if !state.is_open {
    return Vec::new();
  }

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

  let padding_x: u16 = 2;
  let padding_y: u16 = 1;
  let header_height: u16 = 1;
  let divider_height: u16 = 1;
  let row_height: u16 = 1;
  let min_width: u16 = 24;

  let max_rows = {
    let available = viewport
      .height
      .saturating_sub(padding_y * 2 + header_height + divider_height);
    (available / row_height) as usize
  };

  if max_rows == 0 {
    return Vec::new();
  }

  if filtered.len() > state.max_results {
    filtered.truncate(state.max_results);
  }

  if filtered.len() > max_rows {
    filtered.truncate(max_rows);
  }

  let max_title = filtered
    .iter()
    .map(|&idx| state.items[idx].title.len() as u16)
    .max()
    .unwrap_or(0);

  let max_width = viewport.width.saturating_sub(4).max(min_width);
  let panel_width = (max_title + padding_x * 2).max(min_width).min(max_width);
  let panel_height = padding_y * 2 + header_height + divider_height + row_height * filtered.len() as u16;

  let panel_x = viewport.x + (viewport.width.saturating_sub(panel_width)) / 2;
  let panel_y = viewport.y + 1;

  let mut nodes = Vec::new();

  nodes.push(OverlayNode::Rect(OverlayRect {
    rect:   Rect::new(panel_x, panel_y, panel_width, panel_height),
    kind:   OverlayRectKind::Panel,
    radius: 2,
    style:  Style {
      bg: Some(theme.panel_bg),
      fg: None,
      underline_color: None,
      underline_style: None,
      add_modifier: the_lib::render::graphics::Modifier::empty(),
      sub_modifier: the_lib::render::graphics::Modifier::empty(),
    },
  }));

  let placeholder = "Execute a command...";
  let (input_text, input_style) = if state.query.is_empty() {
    (
      placeholder.to_string(),
      Style {
        fg: Some(theme.placeholder),
        ..Style::default()
      },
    )
  } else {
    (
      state.query.clone(),
      Style {
        fg: Some(theme.text),
        ..Style::default()
      },
    )
  };

  let input_row = panel_y + padding_y;
  let input_col = panel_x + padding_x;
  nodes.push(OverlayNode::Text(OverlayText {
    pos:   Position::new(input_row as usize, input_col as usize),
    text:  input_text,
    style: input_style,
  }));

  let divider_row = panel_y + padding_y + header_height;
  nodes.push(OverlayNode::Rect(OverlayRect {
    rect:   Rect::new(panel_x, divider_row, panel_width, divider_height),
    kind:   OverlayRectKind::Divider,
    radius: 0,
    style:  Style {
      fg: Some(theme.divider),
      ..Style::default()
    },
  }));

  let list_start = divider_row + divider_height;

  let selected_index = state.selected.and_then(|sel| filtered.iter().position(|&idx| idx == sel));

  for (row_idx, item_idx) in filtered.iter().enumerate() {
    let row_y = list_start + row_idx as u16;

    if selected_index == Some(row_idx) {
      nodes.push(OverlayNode::Rect(OverlayRect {
        rect:   Rect::new(panel_x + 1, row_y, panel_width.saturating_sub(2), row_height),
        kind:   OverlayRectKind::Highlight,
        radius: 1,
        style:  Style {
          bg: Some(theme.selected_bg),
          fg: Some(theme.selected_border),
          ..Style::default()
        },
      }));
    }

    let style = if selected_index == Some(row_idx) {
      Style {
        fg: Some(theme.selected_text),
        ..Style::default()
      }
    } else {
      Style {
        fg: Some(theme.text),
        ..Style::default()
      }
    };

    nodes.push(OverlayNode::Text(OverlayText {
      pos:   Position::new(row_y as usize, (panel_x + padding_x) as usize),
      text:  state.items[*item_idx].title.clone(),
      style,
    }));
  }

  nodes
}

pub fn build_command_palette_overlay_bottom(
  state: &CommandPaletteState,
  viewport: Rect,
  theme: CommandPaletteTheme,
) -> Vec<OverlayNode> {
  if !state.is_open {
    return Vec::new();
  }

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

  let row_height: u16 = 1;
  let divider_height: u16 = 1;
  let input_height: u16 = 1;
  let list_rows = filtered.len() as u16;
  let panel_height = list_rows
    .saturating_add(divider_height)
    .saturating_add(input_height)
    .min(viewport.height);

  if panel_height == 0 {
    return Vec::new();
  }

  let panel_width = viewport.width;
  let panel_x = viewport.x;
  let panel_y = viewport.y + viewport.height.saturating_sub(panel_height);

  let input_row = panel_y + panel_height.saturating_sub(1);
  let divider_row = input_row.saturating_sub(1);
  let list_start = panel_y;

  let mut nodes = Vec::new();

  nodes.push(OverlayNode::Rect(OverlayRect {
    rect:   Rect::new(panel_x, panel_y, panel_width, panel_height),
    kind:   OverlayRectKind::Panel,
    radius: 0,
    style:  Style {
      bg: Some(theme.panel_bg),
      fg: None,
      underline_color: None,
      underline_style: None,
      add_modifier: the_lib::render::graphics::Modifier::empty(),
      sub_modifier: the_lib::render::graphics::Modifier::empty(),
    },
  }));

  let placeholder = "Execute a command...";
  let (input_text, input_style) = if state.query.is_empty() {
    (
      format!(":{placeholder}"),
      Style {
        fg: Some(theme.placeholder),
        ..Style::default()
      },
    )
  } else {
    (
      format!(":{}", state.query),
      Style {
        fg: Some(theme.text),
        ..Style::default()
      },
    )
  };

  nodes.push(OverlayNode::Text(OverlayText {
    pos:   Position::new(input_row as usize, (panel_x + 1) as usize),
    text:  input_text,
    style: input_style,
  }));

  if divider_row >= panel_y {
    nodes.push(OverlayNode::Rect(OverlayRect {
      rect:   Rect::new(panel_x, divider_row, panel_width, divider_height),
      kind:   OverlayRectKind::Divider,
      radius: 0,
      style:  Style {
        fg: Some(theme.divider),
        ..Style::default()
      },
    }));
  }

  let selected_index = state
    .selected
    .and_then(|sel| filtered.iter().position(|&idx| idx == sel));

  for (row_idx, item_idx) in filtered.iter().enumerate() {
    let row_y = list_start + row_idx as u16;
    if row_y >= divider_row {
      break;
    }

    if selected_index == Some(row_idx) {
      nodes.push(OverlayNode::Rect(OverlayRect {
        rect:   Rect::new(panel_x, row_y, panel_width, row_height),
        kind:   OverlayRectKind::Highlight,
        radius: 0,
        style:  Style {
          bg: Some(theme.selected_bg),
          fg: Some(theme.selected_border),
          ..Style::default()
        },
      }));
    }

    let style = if selected_index == Some(row_idx) {
      Style {
        fg: Some(theme.selected_text),
        ..Style::default()
      }
    } else {
      Style {
        fg: Some(theme.text),
        ..Style::default()
      }
    };

    nodes.push(OverlayNode::Text(OverlayText {
      pos:   Position::new(row_y as usize, (panel_x + 1) as usize),
      text:  state.items[*item_idx].title.clone(),
      style,
    }));
  }

  nodes
}
