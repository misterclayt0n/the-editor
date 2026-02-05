use the_core::grapheme::{
  ensure_grapheme_boundary_next,
  ensure_grapheme_boundary_prev,
};
use the_lib::{
  movement::Movement,
  selection::Range,
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
