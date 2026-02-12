use the_lib::render::{
  LayoutIntent,
  UiColor,
  UiColorToken,
  UiConstraints,
  UiContainer,
  UiList,
  UiListItem,
  UiNode,
  UiPanel,
  UiText,
  graphics::Color,
};

use crate::DefaultContext;

const MAX_VISIBLE_ITEMS: usize = 10;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionMenuItem {
  pub label:         String,
  pub detail:        Option<String>,
  pub documentation: Option<String>,
  pub kind_icon:     Option<String>,
  pub kind_color:    Option<Color>,
}

impl CompletionMenuItem {
  pub fn new(label: impl Into<String>) -> Self {
    Self {
      label:         label.into(),
      detail:        None,
      documentation: None,
      kind_icon:     None,
      kind_color:    None,
    }
  }
}

#[derive(Debug, Clone, Default)]
pub struct CompletionMenuState {
  pub active:   bool,
  pub items:    Vec<CompletionMenuItem>,
  pub selected: Option<usize>,
  pub scroll:   usize,
  pub docs_scroll: usize,
}

impl CompletionMenuState {
  pub fn clear(&mut self) {
    self.active = false;
    self.items.clear();
    self.selected = None;
    self.scroll = 0;
    self.docs_scroll = 0;
  }

  pub fn set_items(&mut self, items: Vec<CompletionMenuItem>) {
    self.items = items;
    self.active = !self.items.is_empty();
    self.selected = if self.items.is_empty() { None } else { Some(0) };
    self.scroll = 0;
    self.docs_scroll = 0;
  }

  fn clamp(&mut self) {
    if self.items.is_empty() {
      self.clear();
      return;
    }

    let max_index = self.items.len() - 1;
    let selected = self.selected.unwrap_or(0).min(max_index);
    self.selected = Some(selected);

    if selected < self.scroll {
      self.scroll = selected;
    } else {
      let visible_end = self
        .scroll
        .saturating_add(MAX_VISIBLE_ITEMS)
        .saturating_sub(1);
      if selected > visible_end {
        self.scroll = selected + 1 - MAX_VISIBLE_ITEMS;
      }
    }

    let max_scroll = self.items.len().saturating_sub(MAX_VISIBLE_ITEMS);
    self.scroll = self.scroll.min(max_scroll);
  }
}

pub fn close_completion_menu<Ctx: DefaultContext>(ctx: &mut Ctx) {
  if !ctx.completion_menu().active {
    return;
  }
  ctx.completion_menu_mut().clear();
  ctx.request_render();
}

pub fn show_completion_menu<Ctx: DefaultContext>(ctx: &mut Ctx, items: Vec<CompletionMenuItem>) {
  let selected = {
    let state = ctx.completion_menu_mut();
    state.set_items(items);
    state.clamp();
    state.selected
  };
  if let Some(index) = selected {
    ctx.completion_selection_changed(index);
  }
  ctx.request_render();
}

pub fn completion_next<Ctx: DefaultContext>(ctx: &mut Ctx) {
  let selected = {
    let state = ctx.completion_menu_mut();
    if !state.active || state.items.is_empty() {
      return;
    }

    let current = state.selected.unwrap_or(0);
    let next = if current + 1 >= state.items.len() {
      0
    } else {
      current + 1
    };
    state.selected = Some(next);
    state.docs_scroll = 0;
    state.clamp();
    state.selected
  };
  if let Some(index) = selected {
    ctx.completion_selection_changed(index);
  }
  ctx.request_render();
}

pub fn completion_prev<Ctx: DefaultContext>(ctx: &mut Ctx) {
  let selected = {
    let state = ctx.completion_menu_mut();
    if !state.active || state.items.is_empty() {
      return;
    }

    let current = state.selected.unwrap_or(0);
    let next = if current == 0 {
      state.items.len() - 1
    } else {
      current - 1
    };
    state.selected = Some(next);
    state.docs_scroll = 0;
    state.clamp();
    state.selected
  };
  if let Some(index) = selected {
    ctx.completion_selection_changed(index);
  }
  ctx.request_render();
}

pub fn completion_accept<Ctx: DefaultContext>(ctx: &mut Ctx) {
  let index = {
    let state = ctx.completion_menu();
    if !state.active || state.items.is_empty() {
      return;
    }
    state
      .selected
      .unwrap_or(0)
      .min(state.items.len().saturating_sub(1))
  };

  let applied = ctx.completion_accept_selected(index);
  if applied {
    ctx.completion_menu_mut().clear();
  }
  ctx.request_render();
}

pub fn completion_docs_scroll<Ctx: DefaultContext>(ctx: &mut Ctx, delta: isize) {
  let changed = {
    let state = ctx.completion_menu_mut();
    if !state.active || state.items.is_empty() {
      return;
    }
    let next = if delta.is_negative() {
      state
        .docs_scroll
        .saturating_sub(delta.unsigned_abs())
    } else {
      state.docs_scroll.saturating_add(delta as usize)
    };
    if next == state.docs_scroll {
      false
    } else {
      state.docs_scroll = next;
      true
    }
  };
  if changed {
    ctx.request_render();
  }
}

pub fn build_completion_menu_ui<Ctx: DefaultContext>(ctx: &mut Ctx) -> Vec<UiNode> {
  let state = ctx.completion_menu_mut();
  state.clamp();
  if !state.active || state.items.is_empty() {
    return Vec::new();
  }

  let docs = state
    .selected
    .and_then(|index| state.items.get(index))
    .and_then(|item| item.documentation.as_ref())
    .map(|value| value.trim().to_string())
    .filter(|value| !value.is_empty());

  let list_items = state
    .items
    .iter()
    .map(|item| {
      UiListItem {
        title:         item.label.clone(),
        subtitle:      item.detail.clone(),
        description:   None,
        shortcut:      None,
        badge:         None,
        leading_icon:  item.kind_icon.clone(),
        leading_color: item.kind_color.map(UiColor::Value),
        symbols:       None,
        match_indices: None,
        emphasis:      false,
        action:        None,
      }
    })
    .collect();

  let mut list = UiList::new("completion_list", list_items);
  list.selected = state.selected;
  list.scroll = state.scroll;
  list.max_visible = Some(MAX_VISIBLE_ITEMS);
  list.style = list.style.with_role("completion");
  list.style.accent = Some(UiColor::Token(UiColorToken::SelectedBg));
  list.style.border = Some(UiColor::Token(UiColorToken::SelectedText));

  let mut container = UiContainer::column("completion_container", 0, vec![UiNode::List(list)]);
  container.style = container.style.with_role("completion");

  let mut panel = UiPanel::new(
    "completion",
    LayoutIntent::Custom("completion".to_string()),
    UiNode::Container(container),
  );
  panel.style = panel.style.with_role("completion");
  panel.constraints = UiConstraints::panel();
  panel.constraints.min_width = Some(28);
  panel.constraints.max_width = Some(64);
  panel.constraints.max_height = Some((MAX_VISIBLE_ITEMS as u16).saturating_add(4));

  let mut overlays = vec![UiNode::Panel(panel)];

  if let Some(docs) = docs {
    let mut docs_text = UiText::new("completion_docs_text", docs);
    docs_text.style = docs_text.style.with_role("completion_docs");
    docs_text.clip = false;

    let mut docs_container = UiContainer::column(
      "completion_docs_container",
      0,
      vec![UiNode::Text(docs_text)],
    );
    docs_container.style = docs_container.style.with_role("completion_docs");

    let mut docs_panel = UiPanel::new(
      "completion_docs",
      LayoutIntent::Custom("completion_docs".to_string()),
      UiNode::Container(docs_container),
    );
    docs_panel.style = docs_panel.style.with_role("completion_docs");
    docs_panel.constraints = UiConstraints::panel();
    docs_panel.constraints.min_width = Some(28);
    docs_panel.constraints.max_width = Some(84);
    docs_panel.constraints.max_height = Some(18);
    overlays.push(UiNode::Panel(docs_panel));
  }

  overlays
}
