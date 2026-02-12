use the_lib::render::{
  LayoutIntent,
  UiConstraints,
  UiContainer,
  UiList,
  UiListItem,
  UiNode,
  UiPanel,
  UiText,
};

use crate::DefaultContext;

const MAX_VISIBLE_ITEMS: usize = 10;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionMenuItem {
  pub label:         String,
  pub detail:        Option<String>,
  pub documentation: Option<String>,
}

impl CompletionMenuItem {
  pub fn new(label: impl Into<String>) -> Self {
    Self {
      label:         label.into(),
      detail:        None,
      documentation: None,
    }
  }
}

#[derive(Debug, Clone, Default)]
pub struct CompletionMenuState {
  pub active:   bool,
  pub items:    Vec<CompletionMenuItem>,
  pub selected: Option<usize>,
  pub scroll:   usize,
}

impl CompletionMenuState {
  pub fn clear(&mut self) {
    self.active = false;
    self.items.clear();
    self.selected = None;
    self.scroll = 0;
  }

  pub fn set_items(&mut self, items: Vec<CompletionMenuItem>) {
    self.items = items;
    self.active = !self.items.is_empty();
    self.selected = if self.items.is_empty() { None } else { Some(0) };
    self.scroll = 0;
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
        leading_icon:  None,
        leading_color: None,
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

  let mut children = vec![UiNode::List(list)];
  if let Some(docs) = docs {
    let mut docs_text = UiText::new("completion_docs", docs);
    docs_text.style = docs_text.style.with_role("completion");
    docs_text.max_lines = Some(6);
    docs_text.clip = false;
    children.push(UiNode::divider());
    children.push(UiNode::Text(docs_text));
  }

  let mut container = UiContainer::column("completion_container", 0, children);
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
  panel.constraints.max_height = Some((MAX_VISIBLE_ITEMS as u16).saturating_add(8));

  vec![UiNode::Panel(panel)]
}
