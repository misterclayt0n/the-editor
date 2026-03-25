use the_lib::render::graphics::Color;

use crate::{
  BuiltinCompletionMenuKind,
  DefaultContext,
};

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

  pub fn detail(mut self, detail: impl Into<String>) -> Self {
    self.detail = Some(detail.into());
    self
  }

  pub fn documentation(mut self, documentation: impl Into<String>) -> Self {
    self.documentation = Some(documentation.into());
    self
  }

  pub fn kind(mut self, icon: impl Into<String>, color: Color) -> Self {
    self.kind_icon = Some(icon.into());
    self.kind_color = Some(color);
    self
  }
}

#[derive(Debug, Clone, Default)]
pub struct CompletionMenuState {
  pub active:      bool,
  pub items:       Vec<CompletionMenuItem>,
  pub selected:    Option<usize>,
  pub scroll:      usize,
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
  ctx.completion_menu_closed();
  ctx.request_render();
}

pub fn show_completion_menu<Ctx: DefaultContext>(ctx: &mut Ctx, items: Vec<CompletionMenuItem>) {
  show_completion_menu_impl(ctx, items);
}

pub fn show_builtin_completion_menu<Ctx: DefaultContext>(
  ctx: &mut Ctx,
  kind: BuiltinCompletionMenuKind,
) {
  let items = ctx.builtin_completion_menu_items(kind);
  if items.is_empty() {
    close_completion_menu(ctx);
    return;
  }
  show_completion_menu_impl(ctx, items);
}

fn show_completion_menu_impl<Ctx: DefaultContext>(ctx: &mut Ctx, items: Vec<CompletionMenuItem>) {
  let selected = {
    let state = ctx.completion_menu_mut();
    state.set_items(items);
    state.clamp();
    state.selected
  };
  if let Some(index) = selected {
    notify_selection_changed(ctx, index);
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
    notify_selection_changed(ctx, index);
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
    notify_selection_changed(ctx, index);
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
    ctx.completion_menu_closed();
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
      state.docs_scroll.saturating_sub(delta.unsigned_abs())
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

fn notify_selection_changed<Ctx: DefaultContext>(ctx: &mut Ctx, index: usize) {
  ctx.completion_selection_changed(index);
}

pub fn set_completion_docs_scroll<Ctx: DefaultContext>(ctx: &mut Ctx, scroll: usize) {
  let changed = {
    let state = ctx.completion_menu_mut();
    if !state.active || state.items.is_empty() {
      return;
    }
    if state.docs_scroll == scroll {
      false
    } else {
      state.docs_scroll = scroll;
      true
    }
  };
  if changed {
    ctx.request_render();
  }
}
