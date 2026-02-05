use the_core::grapheme::{
  ensure_grapheme_boundary_next,
  ensure_grapheme_boundary_prev,
};
use the_core::chars::{
  next_char_boundary,
  prev_char_boundary,
};
use the_lib::{
  movement::Movement,
  selection::Range,
  render::{
    UiColor,
    UiColorToken,
    UiContainer,
    UiDivider,
    UiInput,
    UiList,
    UiListItem,
    UiNode,
    UiPanel,
    UiStyle,
    UiEmphasis,
  },
};
use the_stdx::rope::{
  Config,
  Regex,
  RegexBuilder,
  RopeSliceExt,
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
  pub active:      bool,
  pub direction:   Direction,
  pub query:       String,
  pub cursor:      usize,
  pub completions: Vec<String>,
  pub error:       Option<String>,
  pub register:    char,
  pub extend:      bool,
}

impl SearchPromptState {
  pub fn new() -> Self {
    Self {
      active: false,
      direction: Direction::Forward,
      query: String::new(),
      cursor: 0,
      completions: Vec::new(),
      error: None,
      register: '/',
      extend: false,
    }
  }

  pub fn clear(&mut self) {
    self.active = false;
    self.query.clear();
    self.cursor = 0;
    self.completions.clear();
    self.error = None;
    self.register = '/';
    self.extend = false;
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

  let prompt = ctx.search_prompt_mut();
  prompt.active = true;
  prompt.direction = direction;
  prompt.query.clear();
  prompt.cursor = 0;
  prompt.completions = completions;
  prompt.error = None;
  prompt.register = register;
  prompt.extend = extend;

  ctx.request_render();
}

pub fn handle_search_prompt_key<Ctx: DefaultContext>(ctx: &mut Ctx, key: KeyEvent) -> bool {
  if !ctx.search_prompt_ref().active {
    return false;
  }

  let mut should_update = false;

  match key.key {
    Key::Escape => {
      ctx.search_prompt_mut().clear();
      ctx.request_render();
      return true;
    },
    Key::Enter | Key::NumpadEnter => {
      if finalize_search(ctx) {
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
        should_update = true;
      }
    },
    Key::Delete => {
      let prompt = ctx.search_prompt_mut();
      if prompt.cursor < prompt.query.len() {
        let next = next_char_boundary(&prompt.query, prompt.cursor);
        prompt.query.replace_range(prompt.cursor..next, "");
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
    Key::Char(ch) => {
      if key.modifiers.ctrl() || key.modifiers.alt() {
        return true;
      }
      let prompt = ctx.search_prompt_mut();
      prompt.query.insert(prompt.cursor, ch);
      prompt.cursor += ch.len_utf8();
      should_update = true;
    },
    _ => {},
  }

  if should_update {
    update_search_preview(ctx);
    ctx.request_render();
  }

  true
}

pub fn build_search_regex(query: &str) -> Result<Regex, String> {
  let case_insensitive = !query.chars().any(char::is_uppercase);
  RegexBuilder::new()
    .syntax(Config::new().case_insensitive(case_insensitive).multi_line(true))
    .build(query)
    .map_err(|err| err.to_string())
}

pub fn search_impl<Ctx: DefaultContext>(
  ctx: &mut Ctx,
  regex: &Regex,
  movement: Movement,
  direction: Direction,
  wrap_around: bool,
  _show_warnings: bool,
) {
  let doc = ctx.editor_ref().document();
  let text = doc.text().slice(..);
  let selection = doc.selection();
  let Some(primary) = selection.ranges().first().copied() else {
    return;
  };

  let start = match direction {
    Direction::Forward => text.char_to_byte(ensure_grapheme_boundary_next(text, primary.to())),
    Direction::Backward => {
      text.char_to_byte(ensure_grapheme_boundary_prev(text, primary.from()))
    },
    _ => return,
  };

  let doc = doc.text().slice(..);

  let mut mat = match direction {
    Direction::Forward => regex.find(doc.regex_input_at_bytes(start..)),
    Direction::Backward => regex.find_iter(doc.regex_input_at_bytes(..start)).last(),
    _ => None,
  };

  if mat.is_none() && wrap_around {
    mat = match direction {
      Direction::Forward => regex.find(doc.regex_input()),
      Direction::Backward => regex.find_iter(doc.regex_input_at_bytes(start..)).last(),
      _ => None,
    };
  }

  if let Some(mat) = mat {
    let doc = ctx.editor_ref().document();
    let text = doc.text().slice(..);
    let selection = doc.selection();
    let Some(primary) = selection.ranges().first().copied() else {
      return;
    };

    let start = text.byte_to_char(mat.start());
    let end = text.byte_to_char(mat.end());

    if end == 0 {
      return;
    }

    let range = Range::new(start, end).with_direction(primary.direction());
    let next = match movement {
      Movement::Extend => selection.clone().push(range),
      Movement::Move => selection
        .clone()
        .replace(0, range)
        .unwrap_or_else(|_| selection.clone()),
    };

    let _ = ctx.editor().document_mut().set_selection(next);
  }
}

fn update_search_preview<Ctx: DefaultContext>(ctx: &mut Ctx) {
  let (query, direction, extend) = {
    let prompt = ctx.search_prompt_ref();
    (prompt.query.clone(), prompt.direction, prompt.extend)
  };

  if query.is_empty() {
    ctx.search_prompt_mut().error = None;
    return;
  }

  match build_search_regex(&query) {
    Ok(regex) => {
      ctx.search_prompt_mut().error = None;
      let movement = if extend { Movement::Extend } else { Movement::Move };
      search_impl(ctx, &regex, movement, direction, true, false);
    },
    Err(err) => {
      ctx.search_prompt_mut().error = Some(err);
    },
  }
}

fn finalize_search<Ctx: DefaultContext>(ctx: &mut Ctx) -> bool {
  let (query, register) = {
    let prompt = ctx.search_prompt_ref();
    (prompt.query.clone(), prompt.register)
  };

  if query.is_empty() {
    return true;
  }

  match build_search_regex(&query) {
    Ok(regex) => {
      let movement = if ctx.search_prompt_ref().extend {
        Movement::Extend
      } else {
        Movement::Move
      };
      search_impl(ctx, &regex, movement, ctx.search_prompt_ref().direction, true, false);
    },
    Err(err) => {
      ctx.search_prompt_mut().error = Some(err);
      return false;
    },
  }

  if let Err(err) = ctx.registers_mut().write(register, vec![query]) {
    ctx.search_prompt_mut().error = Some(err.to_string());
    return false;
  }

  ctx.registers_mut().last_search_register = register;
  true
}

pub fn build_search_prompt_ui<Ctx: DefaultContext>(ctx: &mut Ctx) -> Vec<UiNode> {
  let prompt = ctx.search_prompt_ref();
  if !prompt.active {
    return Vec::new();
  }

  let prefix = match prompt.direction {
    Direction::Forward => "/",
    Direction::Backward => "?",
    _ => "/",
  };

  let value = if prompt.query.is_empty() {
    String::new()
  } else {
    format!("{prefix}{}", prompt.query)
  };

  let mut input = UiInput::new("search_prompt_input", value);
  input.placeholder = Some(format!("{prefix}search"));
  input.cursor = if prompt.query.is_empty() {
    1
  } else {
    prefix.len() + prompt.cursor
  };
  input.style = input.style.with_role("search_prompt");
  input.style.accent = Some(UiColor::Token(UiColorToken::Placeholder));

  let query = prompt.query.as_str();
  let mut filtered: Vec<&String> = if query.is_empty() {
    prompt.completions.iter().collect()
  } else {
    prompt
      .completions
      .iter()
      .filter(|item| item.starts_with(query))
      .collect()
  };
  filtered.truncate(6);

  let mut children = vec![UiNode::Input(input)];

  if !filtered.is_empty() {
    let items = filtered
      .into_iter()
      .map(|item| UiListItem::new(item.clone()))
      .collect();
    let mut list = UiList::new("search_prompt_list", items);
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
  let container = UiNode::Container(container);

  let mut panel = UiPanel::bottom("search_prompt", container);
  panel.style = panel.style.with_role("search_prompt");

  vec![UiNode::Panel(panel)]
}
