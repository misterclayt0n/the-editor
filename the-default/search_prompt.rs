use the_core::chars::{
  byte_to_char_idx,
  next_char_boundary,
  prev_char_boundary,
};
use the_lib::{
  movement::{
    Direction as LibDirection,
    Movement,
  },
  render::{
    UiAlign,
    UiAlignPair,
    UiColor,
    UiColorToken,
    UiConstraints,
    UiContainer,
    UiDivider,
    UiEmphasis,
    UiInput,
    UiInsets,
    UiList,
    UiListItem,
    UiNode,
    UiPanel,
    UiStyle,
  },
  search::{
    build_regex,
    search_regex,
  },
  selection::{
    CursorPick,
    keep_or_remove_matches,
    select_on_matches,
    split_on_matches,
  },
};

use crate::{
  DefaultContext,
  Direction,
  Key,
  KeyEvent,
  Mode,
};

#[derive(Debug, Clone)]
pub struct SearchPromptState {
  pub active:             bool,
  pub kind:               SearchPromptKind,
  pub direction:          Direction,
  pub query:              String,
  pub cursor:             usize,
  pub completions:        Vec<String>,
  pub error:              Option<String>,
  pub register:           char,
  pub extend:             bool,
  pub original_selection: Option<the_lib::selection::Selection>,
  pub selected:           Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchPromptKind {
  Search,
  SelectRegex,
  SplitSelection,
  KeepSelections,
  RemoveSelections,
}

impl SearchPromptState {
  pub fn new() -> Self {
    Self {
      active:             false,
      kind:               SearchPromptKind::Search,
      direction:          Direction::Forward,
      query:              String::new(),
      cursor:             0,
      completions:        Vec::new(),
      error:              None,
      register:           '/',
      extend:             false,
      original_selection: None,
      selected:           None,
    }
  }

  pub fn clear(&mut self) {
    self.active = false;
    self.kind = SearchPromptKind::Search;
    self.query.clear();
    self.cursor = 0;
    self.completions.clear();
    self.error = None;
    self.register = '/';
    self.extend = false;
    self.original_selection = None;
    self.selected = None;
  }
}

impl Default for SearchPromptState {
  fn default() -> Self {
    Self::new()
  }
}

pub fn search_completions<Ctx: DefaultContext>(ctx: &Ctx, reg: Option<char>) -> Vec<String> {
  let mut items = reg
    .and_then(|reg| ctx.registers().read(reg, ctx.editor_ref().document()))
    .map_or(Vec::new(), |reg| reg.take(200).collect());
  items.sort_unstable();
  items.dedup();
  items.into_iter().map(|value| value.to_string()).collect()
}

pub fn open_search_prompt<Ctx: DefaultContext>(ctx: &mut Ctx, direction: Direction) {
  let register = ctx.register().unwrap_or('/');
  let extend = ctx.mode() == Mode::Select;
  let completions = search_completions(ctx, Some(register));

  let original_selection = ctx.editor_ref().document().selection().clone();
  let prompt = ctx.search_prompt_mut();
  prompt.active = true;
  prompt.kind = SearchPromptKind::Search;
  prompt.direction = direction;
  prompt.query.clear();
  prompt.cursor = 0;
  prompt.completions = completions;
  prompt.error = None;
  prompt.register = register;
  prompt.extend = extend;
  prompt.original_selection = Some(original_selection);
  prompt.selected = None;

  ctx.request_render();
}

pub fn open_select_regex_prompt<Ctx: DefaultContext>(ctx: &mut Ctx) {
  open_selection_prompt(ctx, SearchPromptKind::SelectRegex);
}

pub fn open_split_selection_prompt<Ctx: DefaultContext>(ctx: &mut Ctx) {
  open_selection_prompt(ctx, SearchPromptKind::SplitSelection);
}

pub fn open_keep_selections_prompt<Ctx: DefaultContext>(ctx: &mut Ctx) {
  open_selection_prompt(ctx, SearchPromptKind::KeepSelections);
}

pub fn open_remove_selections_prompt<Ctx: DefaultContext>(ctx: &mut Ctx) {
  open_selection_prompt(ctx, SearchPromptKind::RemoveSelections);
}

fn open_selection_prompt<Ctx: DefaultContext>(ctx: &mut Ctx, kind: SearchPromptKind) {
  let original_selection = ctx.editor_ref().document().selection().clone();
  let prompt = ctx.search_prompt_mut();
  prompt.active = true;
  prompt.kind = kind;
  prompt.direction = Direction::Forward;
  prompt.query.clear();
  prompt.cursor = 0;
  prompt.completions.clear();
  prompt.error = None;
  prompt.register = '/';
  prompt.extend = false;
  prompt.original_selection = Some(original_selection);
  prompt.selected = None;

  ctx.request_render();
}

pub fn handle_search_prompt_key<Ctx: DefaultContext>(ctx: &mut Ctx, key: KeyEvent) -> bool {
  if !ctx.search_prompt_ref().active {
    return false;
  }

  let mut should_update = false;

  match key.key {
    Key::Escape => {
      if let Some(selection) = ctx.search_prompt_mut().original_selection.take() {
        let _ = ctx.editor().document_mut().set_selection(selection);
      }
      ctx.search_prompt_mut().clear();
      ctx.request_render();
      return true;
    },
    Key::Enter | Key::NumpadEnter => {
      let should_close = match ctx.search_prompt_ref().kind {
        SearchPromptKind::Search => finalize_search(ctx),
        SearchPromptKind::SelectRegex => finalize_select_regex(ctx),
        SearchPromptKind::SplitSelection => finalize_split_selection(ctx),
        SearchPromptKind::KeepSelections => finalize_keep_selections(ctx),
        SearchPromptKind::RemoveSelections => finalize_remove_selections(ctx),
      };
      if should_close {
        ctx.search_prompt_mut().clear();
      }
      ctx.request_render();
      return true;
    },
    Key::Backspace => {
      let prompt = ctx.search_prompt_mut();
      if prompt.cursor > 0 && prompt.cursor <= prompt.query.len() {
        let prev = prev_char_boundary(&prompt.query, prompt.cursor);
        prompt.query.replace_range(prev..prompt.cursor, "");
        prompt.cursor = prev;
        prompt.selected = None;
        should_update = true;
      }
    },
    Key::Delete => {
      let prompt = ctx.search_prompt_mut();
      if prompt.cursor < prompt.query.len() {
        let next = next_char_boundary(&prompt.query, prompt.cursor);
        prompt.query.replace_range(prompt.cursor..next, "");
        prompt.selected = None;
        should_update = true;
      }
    },
    Key::Left => {
      let prompt = ctx.search_prompt_mut();
      prompt.cursor = prev_char_boundary(&prompt.query, prompt.cursor);
      should_update = true;
    },
    Key::Right => {
      let prompt = ctx.search_prompt_mut();
      prompt.cursor = next_char_boundary(&prompt.query, prompt.cursor);
      should_update = true;
    },
    Key::Home => {
      ctx.search_prompt_mut().cursor = 0;
      should_update = true;
    },
    Key::End => {
      let prompt = ctx.search_prompt_mut();
      prompt.cursor = prompt.query.len();
      should_update = true;
    },
    Key::Up => {
      if ctx.search_prompt_ref().kind != SearchPromptKind::Search {
        return true;
      }
      let filtered: Vec<String> = filtered_completions(ctx.search_prompt_ref())
        .into_iter()
        .cloned()
        .collect();
      if filtered.is_empty() {
        return true;
      }
      let prompt = ctx.search_prompt_mut();
      let current = prompt.selected.unwrap_or(0);
      let next = if current == 0 {
        filtered.len() - 1
      } else {
        current - 1
      };
      prompt.selected = Some(next);
      apply_completion(prompt, &filtered[next]);
      should_update = true;
    },
    Key::Down => {
      if ctx.search_prompt_ref().kind != SearchPromptKind::Search {
        return true;
      }
      let filtered: Vec<String> = filtered_completions(ctx.search_prompt_ref())
        .into_iter()
        .cloned()
        .collect();
      if filtered.is_empty() {
        return true;
      }
      let prompt = ctx.search_prompt_mut();
      let current = prompt.selected.unwrap_or(filtered.len().saturating_sub(1));
      let next = if current + 1 >= filtered.len() {
        0
      } else {
        current + 1
      };
      prompt.selected = Some(next);
      apply_completion(prompt, &filtered[next]);
      should_update = true;
    },
    Key::Tab => {
      if ctx.search_prompt_ref().kind != SearchPromptKind::Search {
        return true;
      }
      let filtered: Vec<String> = filtered_completions(ctx.search_prompt_ref())
        .into_iter()
        .cloned()
        .collect();
      if let Some(first) = filtered.first() {
        let prompt = ctx.search_prompt_mut();
        prompt.selected = Some(0);
        apply_completion(prompt, first);
        should_update = true;
      }
    },
    Key::Char('n') if key.modifiers.ctrl() && !key.modifiers.alt() => {
      if ctx.search_prompt_ref().kind != SearchPromptKind::Search {
        return true;
      }
      step_search_prompt(ctx, Direction::Forward);
      ctx.request_render();
      return true;
    },
    Key::Char('p') if key.modifiers.ctrl() && !key.modifiers.alt() => {
      if ctx.search_prompt_ref().kind != SearchPromptKind::Search {
        return true;
      }
      step_search_prompt(ctx, Direction::Backward);
      ctx.request_render();
      return true;
    },
    Key::Char(ch) => {
      if key.modifiers.ctrl() || key.modifiers.alt() {
        return true;
      }
      let prompt = ctx.search_prompt_mut();
      prompt.query.insert(prompt.cursor, ch);
      prompt.cursor += ch.len_utf8();
      prompt.selected = None;
      should_update = true;
    },
    _ => {},
  }

  if should_update {
    update_search_prompt_preview(ctx);
    ctx.request_render();
  }

  true
}

pub fn step_search_prompt<Ctx: DefaultContext>(ctx: &mut Ctx, direction: Direction) {
  let (query, extend) = {
    let prompt = ctx.search_prompt_ref();
    (prompt.query.clone(), prompt.extend)
  };

  if query.is_empty() {
    return;
  }

  let direction = match to_lib_direction(direction) {
    Some(dir) => dir,
    None => return,
  };

  match build_regex(&query, true) {
    Ok(regex) => {
      ctx.search_prompt_mut().error = None;
      let movement = if extend {
        Movement::Extend
      } else {
        Movement::Move
      };
      let fallback_pick = if extend {
        match direction {
          LibDirection::Forward => CursorPick::Last,
          LibDirection::Backward => CursorPick::First,
        }
      } else {
        CursorPick::First
      };
      let pick = {
        let editor = ctx.editor_ref();
        let selection = editor.document().selection();
        if let Some(active_cursor) = editor.view().active_cursor
          && selection.range_by_id(active_cursor).is_some()
        {
          CursorPick::Id(active_cursor)
        } else {
          fallback_pick
        }
      };
      let doc = ctx.editor_ref().document();
      let text = doc.text().slice(..);
      let selection = doc.selection().clone();
      if let Some(next) = search_regex(text, &selection, pick, &regex, movement, direction, true) {
        let _ = ctx.editor().document_mut().set_selection(next);
      }
    },
    Err(err) => {
      ctx.search_prompt_mut().error = Some(err);
    },
  }
}

pub fn update_search_prompt_preview<Ctx: DefaultContext>(ctx: &mut Ctx) {
  match ctx.search_prompt_ref().kind {
    SearchPromptKind::Search => update_search_preview(ctx),
    SearchPromptKind::SelectRegex => update_select_regex_preview(ctx),
    SearchPromptKind::SplitSelection => update_split_selection_preview(ctx),
    SearchPromptKind::KeepSelections => update_keep_selections_preview(ctx),
    SearchPromptKind::RemoveSelections => update_remove_selections_preview(ctx),
  }
}

pub fn update_search_preview<Ctx: DefaultContext>(ctx: &mut Ctx) {
  let (query, direction, extend) = {
    let prompt = ctx.search_prompt_ref();
    (prompt.query.clone(), prompt.direction, prompt.extend)
  };

  if query.is_empty() {
    ctx.search_prompt_mut().error = None;
    return;
  }

  let direction = match to_lib_direction(direction) {
    Some(dir) => dir,
    None => return,
  };

  match build_regex(&query, true) {
    Ok(regex) => {
      ctx.search_prompt_mut().error = None;
      let movement = if extend {
        Movement::Extend
      } else {
        Movement::Move
      };
      let doc = ctx.editor_ref().document();
      let text = doc.text().slice(..);
      let selection = ctx
        .search_prompt_ref()
        .original_selection
        .clone()
        .unwrap_or_else(|| doc.selection().clone());
      let pick = if let Some(active_cursor) = ctx.editor_ref().view().active_cursor
        && selection.range_by_id(active_cursor).is_some()
      {
        CursorPick::Id(active_cursor)
      } else {
        CursorPick::First
      };
      if let Some(next) = search_regex(text, &selection, pick, &regex, movement, direction, true) {
        let _ = ctx.editor().document_mut().set_selection(next);
      }
    },
    Err(err) => {
      ctx.search_prompt_mut().error = Some(err);
    },
  }
}

pub fn update_select_regex_preview<Ctx: DefaultContext>(ctx: &mut Ctx) {
  let (query, base_selection) = {
    let prompt = ctx.search_prompt_ref();
    (
      prompt.query.clone(),
      prompt
        .original_selection
        .clone()
        .unwrap_or_else(|| ctx.editor_ref().document().selection().clone()),
    )
  };

  if query.is_empty() {
    if let Some(selection) = ctx.search_prompt_ref().original_selection.clone() {
      let _ = ctx.editor().document_mut().set_selection(selection);
    }
    ctx.search_prompt_mut().error = None;
    return;
  }

  match build_regex(&query, true) {
    Ok(regex) => {
      ctx.search_prompt_mut().error = None;
      let doc = ctx.editor_ref().document();
      let text = doc.text().slice(..);
      match select_on_matches(text, &base_selection, &regex) {
        Ok(Some(next)) => {
          let _ = ctx.editor().document_mut().set_selection(next);
        },
        Ok(None) => {},
        Err(err) => {
          ctx.search_prompt_mut().error = Some(err.to_string());
        },
      }
    },
    Err(err) => {
      ctx.search_prompt_mut().error = Some(err);
    },
  }
}

pub fn update_split_selection_preview<Ctx: DefaultContext>(ctx: &mut Ctx) {
  let (query, base_selection) = {
    let prompt = ctx.search_prompt_ref();
    (
      prompt.query.clone(),
      prompt
        .original_selection
        .clone()
        .unwrap_or_else(|| ctx.editor_ref().document().selection().clone()),
    )
  };

  if query.is_empty() {
    if let Some(selection) = ctx.search_prompt_ref().original_selection.clone() {
      let _ = ctx.editor().document_mut().set_selection(selection);
    }
    ctx.search_prompt_mut().error = None;
    return;
  }

  match build_regex(&query, true) {
    Ok(regex) => {
      ctx.search_prompt_mut().error = None;
      let doc = ctx.editor_ref().document();
      let text = doc.text().slice(..);
      match split_on_matches(text, &base_selection, &regex) {
        Ok(next) => {
          let _ = ctx.editor().document_mut().set_selection(next);
        },
        Err(err) => {
          ctx.search_prompt_mut().error = Some(err.to_string());
        },
      }
    },
    Err(err) => {
      ctx.search_prompt_mut().error = Some(err);
    },
  }
}

pub fn update_keep_selections_preview<Ctx: DefaultContext>(ctx: &mut Ctx) {
  update_keep_or_remove_preview(ctx, false);
}

pub fn update_remove_selections_preview<Ctx: DefaultContext>(ctx: &mut Ctx) {
  update_keep_or_remove_preview(ctx, true);
}

fn update_keep_or_remove_preview<Ctx: DefaultContext>(ctx: &mut Ctx, remove: bool) {
  let (query, base_selection) = {
    let prompt = ctx.search_prompt_ref();
    (
      prompt.query.clone(),
      prompt
        .original_selection
        .clone()
        .unwrap_or_else(|| ctx.editor_ref().document().selection().clone()),
    )
  };

  if query.is_empty() {
    if let Some(selection) = ctx.search_prompt_ref().original_selection.clone() {
      let _ = ctx.editor().document_mut().set_selection(selection);
    }
    ctx.search_prompt_mut().error = None;
    return;
  }

  match build_regex(&query, true) {
    Ok(regex) => {
      ctx.search_prompt_mut().error = None;
      let doc = ctx.editor_ref().document();
      let text = doc.text().slice(..);
      match keep_or_remove_matches(text, &base_selection, &regex, remove) {
        Ok(Some(next)) => {
          let _ = ctx.editor().document_mut().set_selection(next);
        },
        Ok(None) => {},
        Err(err) => {
          ctx.search_prompt_mut().error = Some(err.to_string());
        },
      }
    },
    Err(err) => {
      ctx.search_prompt_mut().error = Some(err);
    },
  }
}

pub fn finalize_search<Ctx: DefaultContext>(ctx: &mut Ctx) -> bool {
  let (query, register) = {
    let prompt = ctx.search_prompt_ref();
    (prompt.query.clone(), prompt.register)
  };

  if query.is_empty() {
    return true;
  }

  match build_regex(&query, true) {
    Ok(_) => {
      ctx.search_prompt_mut().error = None;
    },
    Err(err) => {
      ctx.search_prompt_mut().error = Some(err.clone());
      ctx.push_error("search", err);
      return false;
    },
  }

  if let Err(err) = ctx.registers_mut().write(register, vec![query]) {
    let message = err.to_string();
    ctx.search_prompt_mut().error = Some(message.clone());
    ctx.push_error("search", message);
    return false;
  }

  ctx.registers_mut().last_search_register = register;
  true
}

pub fn finalize_select_regex<Ctx: DefaultContext>(ctx: &mut Ctx) -> bool {
  let (query, base_selection) = {
    let prompt = ctx.search_prompt_ref();
    (
      prompt.query.clone(),
      prompt
        .original_selection
        .clone()
        .unwrap_or_else(|| ctx.editor_ref().document().selection().clone()),
    )
  };
  if query.is_empty() {
    return true;
  }

  let regex = match build_regex(&query, true) {
    Ok(regex) => regex,
    Err(err) => {
      ctx.search_prompt_mut().error = Some(err.clone());
      ctx.push_error("select_regex", err);
      return false;
    },
  };

  let text = ctx.editor_ref().document().text().slice(..);
  match select_on_matches(text, &base_selection, &regex) {
    Ok(Some(next)) => {
      ctx.search_prompt_mut().error = None;
      let _ = ctx.editor().document_mut().set_selection(next);
      true
    },
    Ok(None) => {
      let message = "nothing selected".to_string();
      ctx.search_prompt_mut().error = Some(message.clone());
      ctx.push_error("select_regex", message);
      false
    },
    Err(err) => {
      let message = err.to_string();
      ctx.search_prompt_mut().error = Some(message.clone());
      ctx.push_error("select_regex", message);
      false
    },
  }
}

pub fn finalize_split_selection<Ctx: DefaultContext>(ctx: &mut Ctx) -> bool {
  let (query, base_selection) = {
    let prompt = ctx.search_prompt_ref();
    (
      prompt.query.clone(),
      prompt
        .original_selection
        .clone()
        .unwrap_or_else(|| ctx.editor_ref().document().selection().clone()),
    )
  };
  if query.is_empty() {
    return true;
  }

  let regex = match build_regex(&query, true) {
    Ok(regex) => regex,
    Err(err) => {
      ctx.search_prompt_mut().error = Some(err.clone());
      ctx.push_error("split_selection", err);
      return false;
    },
  };

  let text = ctx.editor_ref().document().text().slice(..);
  match split_on_matches(text, &base_selection, &regex) {
    Ok(next) => {
      ctx.search_prompt_mut().error = None;
      let _ = ctx.editor().document_mut().set_selection(next);
      true
    },
    Err(err) => {
      let message = err.to_string();
      ctx.search_prompt_mut().error = Some(message.clone());
      ctx.push_error("split_selection", message);
      false
    },
  }
}

pub fn finalize_keep_selections<Ctx: DefaultContext>(ctx: &mut Ctx) -> bool {
  finalize_keep_or_remove(ctx, false)
}

pub fn finalize_remove_selections<Ctx: DefaultContext>(ctx: &mut Ctx) -> bool {
  finalize_keep_or_remove(ctx, true)
}

fn finalize_keep_or_remove<Ctx: DefaultContext>(ctx: &mut Ctx, remove: bool) -> bool {
  let (query, base_selection) = {
    let prompt = ctx.search_prompt_ref();
    (
      prompt.query.clone(),
      prompt
        .original_selection
        .clone()
        .unwrap_or_else(|| ctx.editor_ref().document().selection().clone()),
    )
  };
  if query.is_empty() {
    return true;
  }

  let regex = match build_regex(&query, true) {
    Ok(regex) => regex,
    Err(err) => {
      ctx.search_prompt_mut().error = Some(err.clone());
      ctx.push_error(
        if remove {
          "remove_selections"
        } else {
          "keep_selections"
        },
        err,
      );
      return false;
    },
  };

  let text = ctx.editor_ref().document().text().slice(..);
  match keep_or_remove_matches(text, &base_selection, &regex, remove) {
    Ok(Some(next)) => {
      ctx.search_prompt_mut().error = None;
      let _ = ctx.editor().document_mut().set_selection(next);
      true
    },
    Ok(None) => {
      let message = "no selections remaining".to_string();
      ctx.search_prompt_mut().error = Some(message.clone());
      ctx.push_error(
        if remove {
          "remove_selections"
        } else {
          "keep_selections"
        },
        message,
      );
      false
    },
    Err(err) => {
      let message = err.to_string();
      ctx.search_prompt_mut().error = Some(message.clone());
      ctx.push_error(
        if remove {
          "remove_selections"
        } else {
          "keep_selections"
        },
        message,
      );
      false
    },
  }
}

fn to_lib_direction(direction: Direction) -> Option<LibDirection> {
  match direction {
    Direction::Forward => Some(LibDirection::Forward),
    Direction::Backward => Some(LibDirection::Backward),
    _ => None,
  }
}

fn filtered_completions(prompt: &SearchPromptState) -> Vec<&String> {
  if prompt.query.is_empty() {
    return prompt.completions.iter().collect();
  }
  prompt
    .completions
    .iter()
    .filter(|item| item.starts_with(&prompt.query))
    .collect()
}

fn apply_completion(prompt: &mut SearchPromptState, completion: &str) {
  prompt.query.clear();
  prompt.query.push_str(completion);
  prompt.cursor = completion.len();
}

pub fn build_search_prompt_ui<Ctx: DefaultContext>(ctx: &mut Ctx) -> Vec<UiNode> {
  let prompt = ctx.search_prompt_ref();
  if !prompt.active {
    return Vec::new();
  }

  let mut input = UiInput::new("search_prompt_input", prompt.query.clone());
  input.placeholder = Some(
    match prompt.kind {
      SearchPromptKind::Search => "search",
      SearchPromptKind::SelectRegex => "select",
      SearchPromptKind::SplitSelection => "split",
      SearchPromptKind::KeepSelections => "keep",
      SearchPromptKind::RemoveSelections => "remove",
    }
    .to_string(),
  );
  input.cursor = byte_to_char_idx(&prompt.query, prompt.cursor);
  input.style = input.style.with_role("search_prompt");
  input.style.accent = Some(UiColor::Token(UiColorToken::Placeholder));

  let mut filtered = filtered_completions(prompt);
  filtered.truncate(6);

  let mut children = vec![UiNode::Input(input)];

  if !filtered.is_empty() {
    let filtered_len = filtered.len();
    let items = filtered
      .into_iter()
      .map(|item| UiListItem::new(item.clone()))
      .collect();
    let mut list = UiList::new("search_prompt_list", items);
    if let Some(selected) = prompt.selected {
      list.selected = Some(selected.min(filtered_len.saturating_sub(1)));
    }
    list.style = list.style.with_role("search_prompt");
    list.style.accent = Some(UiColor::Token(UiColorToken::SelectedBg));
    list.style.border = Some(UiColor::Token(UiColorToken::SelectedText));
    children.push(UiNode::Divider(UiDivider { id: None }));
    children.push(UiNode::List(list));
  }

  if let Some(error) = prompt.error.as_ref().filter(|e| !e.is_empty()) {
    let mut error_text = UiNode::text("search_prompt_error", error.clone());
    if let UiNode::Text(text) = &mut error_text {
      text.style = UiStyle::default().with_role("search_prompt");
      text.style.emphasis = UiEmphasis::Strong;
    }
    children.push(UiNode::Divider(UiDivider { id: None }));
    children.push(error_text);
  }

  let mut container = UiContainer::column("search_prompt_container", 0, children);
  container.style = container.style.with_role("search_prompt");
  container.constraints.align.horizontal = UiAlign::Stretch;
  let container = UiNode::Container(container);

  let mut panel = UiPanel::floating("search_prompt", container);
  panel.style = panel.style.with_role("search_prompt");
  panel.style.border = None;
  panel.constraints = UiConstraints {
    min_width:  Some(50),
    max_width:  Some(65),
    min_height: None,
    max_height: None,
    padding:    UiInsets {
      left:   1,
      right:  1,
      top:    0,
      bottom: 0,
    },
    align:      UiAlignPair {
      horizontal: UiAlign::Center,
      vertical:   UiAlign::Center,
    },
  };

  vec![UiNode::Panel(panel)]
}
