use std::{
  collections::HashMap,
  fmt,
  str::FromStr,
};

use smallvec::SmallVec;
use the_core::grapheme::prev_grapheme_boundary;
use the_lib::selection::Range;

use crate::{
  Command,
  CommandPaletteItem,
  DefaultContext,
  Key,
  KeyEvent,
  KeyOutcome,
  Modifiers,
  command_from_name,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct KeyBinding {
  pub code:  Key,
  pub shift: bool,
  pub ctrl:  bool,
  pub alt:   bool,
}

impl KeyBinding {
  pub const fn new(code: Key) -> Self {
    Self {
      code,
      shift: false,
      ctrl: false,
      alt: false,
    }
  }

  pub const fn with_modifiers(mut self, shift: bool, ctrl: bool, alt: bool) -> Self {
    self.shift = shift;
    self.ctrl = ctrl;
    self.alt = alt;
    self
  }

  pub const fn from_key_event(event: &KeyEvent) -> Self {
    Self {
      code:  event.key,
      shift: event.modifiers.shift(),
      ctrl:  event.modifiers.ctrl(),
      alt:   event.modifiers.alt(),
    }
  }

  #[must_use]
  pub fn to_key_event(&self) -> KeyEvent {
    let mut modifiers = Modifiers::empty();
    if self.ctrl {
      modifiers.insert(Modifiers::CTRL);
    }
    if self.alt {
      modifiers.insert(Modifiers::ALT);
    }
    if self.shift {
      modifiers.insert(Modifiers::SHIFT);
    }

    KeyEvent {
      key: self.code,
      modifiers,
    }
  }
}

impl fmt::Display for KeyBinding {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    let mut result = String::new();

    if self.ctrl {
      result.push_str("C-");
    }
    if self.alt {
      result.push_str("A-");
    }
    if self.shift {
      result.push_str("S-");
    }

    let key_str = match self.code {
      Key::Char(' ') => "space".to_string(),
      Key::Char(c) => c.to_string(),
      Key::Enter => "ret".to_string(),
      Key::NumpadEnter => "numpadret".to_string(),
      Key::Escape => "esc".to_string(),
      Key::Backspace => "bs".to_string(),
      Key::Tab => "tab".to_string(),
      Key::Delete => "del".to_string(),
      Key::Insert => "ins".to_string(),
      Key::Home => "home".to_string(),
      Key::End => "end".to_string(),
      Key::PageUp => "pgup".to_string(),
      Key::PageDown => "pgdown".to_string(),
      Key::Left => "left".to_string(),
      Key::Right => "right".to_string(),
      Key::Up => "up".to_string(),
      Key::Down => "down".to_string(),
      Key::F1 => "F1".to_string(),
      Key::F2 => "F2".to_string(),
      Key::F3 => "F3".to_string(),
      Key::F4 => "F4".to_string(),
      Key::F5 => "F5".to_string(),
      Key::F6 => "F6".to_string(),
      Key::F7 => "F7".to_string(),
      Key::F8 => "F8".to_string(),
      Key::F9 => "F9".to_string(),
      Key::F10 => "F10".to_string(),
      Key::F11 => "F11".to_string(),
      Key::F12 => "F12".to_string(),
      Key::Other => "other".to_string(),
    };

    result.push_str(&key_str);
    write!(f, "{}", result)
  }
}

#[derive(Debug)]
pub struct ParseKeyBindingError(pub String);

impl fmt::Display for ParseKeyBindingError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{}", self.0)
  }
}

impl std::error::Error for ParseKeyBindingError {}

impl FromStr for KeyBinding {
  type Err = ParseKeyBindingError;

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
      return Err(ParseKeyBindingError("empty key literal".into()));
    }

    if trimmed == "-" {
      return Ok(KeyBinding::new(Key::Char('-')));
    }

    let mut tokens: Vec<_> = trimmed.split('-').collect();
    let key_token = tokens
      .pop()
      .ok_or_else(|| ParseKeyBindingError("missing key token".into()))?;

    let mut shift = false;
    let mut ctrl = false;
    let mut alt = false;

    for token in tokens {
      let modifier = token.trim();
      if modifier.is_empty() {
        continue;
      }

      match modifier.to_ascii_uppercase().as_str() {
        "S" | "SHIFT" => {
          if shift {
            return Err(ParseKeyBindingError(format!(
              "repeated key modifier '{}-'",
              modifier
            )));
          }
          shift = true;
        },
        "C" | "CTRL" | "CONTROL" => {
          if ctrl {
            return Err(ParseKeyBindingError(format!(
              "repeated key modifier '{}-'",
              modifier
            )));
          }
          ctrl = true;
        },
        "A" | "ALT" => {
          if alt {
            return Err(ParseKeyBindingError(format!(
              "repeated key modifier '{}-'",
              modifier
            )));
          }
          alt = true;
        },
        invalid => {
          return Err(ParseKeyBindingError(format!(
            "invalid key modifier '{}-'",
            invalid
          )));
        },
      }
    }

    let code = parse_key_token(key_token)?;

    Ok(KeyBinding {
      code,
      shift,
      ctrl,
      alt,
    })
  }
}

fn parse_key_token(token: &str) -> Result<Key, ParseKeyBindingError> {
  if token.len() == 1 {
    return Ok(Key::Char(token.chars().next().unwrap()));
  }

  match token.to_ascii_lowercase().as_str() {
    "space" => Ok(Key::Char(' ')),
    "minus" => Ok(Key::Char('-')),
    "underscore" => Ok(Key::Char('_')),
    "comma" => Ok(Key::Char(',')),
    "period" | "dot" => Ok(Key::Char('.')),
    "slash" => Ok(Key::Char('/')),
    "backslash" | "bslash" => Ok(Key::Char('\\')),
    "semicolon" => Ok(Key::Char(';')),
    "quote" | "apostrophe" => Ok(Key::Char('\'')),
    "doublequote" | "dquote" => Ok(Key::Char('"')),
    "enter" | "ret" | "return" => Ok(Key::Enter),
    "numpadenter" | "numpadret" | "kpenter" | "numenter" => Ok(Key::NumpadEnter),
    "esc" | "escape" => Ok(Key::Escape),
    "backspace" | "bs" => Ok(Key::Backspace),
    "tab" => Ok(Key::Tab),
    "delete" | "del" => Ok(Key::Delete),
    "insert" | "ins" => Ok(Key::Insert),
    "home" => Ok(Key::Home),
    "end" => Ok(Key::End),
    "pageup" | "pgup" => Ok(Key::PageUp),
    "pagedown" | "pgdown" => Ok(Key::PageDown),
    "left" => Ok(Key::Left),
    "right" => Ok(Key::Right),
    "up" => Ok(Key::Up),
    "down" => Ok(Key::Down),
    "f1" => Ok(Key::F1),
    "f2" => Ok(Key::F2),
    "f3" => Ok(Key::F3),
    "f4" => Ok(Key::F4),
    "f5" => Ok(Key::F5),
    "f6" => Ok(Key::F6),
    "f7" => Ok(Key::F7),
    "f8" => Ok(Key::F8),
    "f9" => Ok(Key::F9),
    "f10" => Ok(Key::F10),
    "f11" => Ok(Key::F11),
    "f12" => Ok(Key::F12),
    "other" => Ok(Key::Other),
    invalid => Err(ParseKeyBindingError(format!("unknown key '{invalid}'"))),
  }
}

pub trait IntoKeyBinding {
  fn into_binding(self) -> Result<KeyBinding, ParseKeyBindingError>;
}

impl IntoKeyBinding for char {
  fn into_binding(self) -> Result<KeyBinding, ParseKeyBindingError> {
    Ok(KeyBinding::new(Key::Char(self)))
  }
}

impl IntoKeyBinding for &'static str {
  fn into_binding(self) -> Result<KeyBinding, ParseKeyBindingError> {
    KeyBinding::from_str(self)
  }
}

pub fn binding_from_literal<L: IntoKeyBinding>(literal: L) -> KeyBinding {
  literal
    .into_binding()
    .unwrap_or_else(|err| panic!("invalid key literal: {err}"))
}

#[allow(dead_code)]
pub fn binding_from_ident(name: &str) -> KeyBinding {
  KeyBinding::from_str(name).unwrap_or_else(|err| panic!("invalid key identifier '{name}': {err}"))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Mode {
  Normal,
  Insert,
  Select,
  Command,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyAction {
  Command(Command),
  Mode(Mode),
  Named(&'static str),
}

#[derive(Debug, Clone)]
pub enum KeyTrie {
  Command(KeyAction),
  Sequence(Vec<KeyAction>),
  Node(KeyTrieNode),
}

#[derive(Debug, Clone, Default)]
pub struct KeyTrieNode {
  pub name:      String,
  pub map:       HashMap<KeyBinding, KeyTrie>,
  pub order:     Vec<KeyBinding>,
  pub is_sticky: bool,
}

impl KeyTrieNode {
  pub fn new(name: &str, map: HashMap<KeyBinding, KeyTrie>, order: Vec<KeyBinding>) -> Self {
    Self {
      name: name.to_string(),
      map,
      order,
      is_sticky: false,
    }
  }

  pub fn merge(&mut self, mut other: Self) {
    for (k, v) in std::mem::take(&mut other.map) {
      if let Some(KeyTrie::Node(node)) = self.map.get_mut(&k)
        && let KeyTrie::Node(other_node) = v
      {
        node.merge(other_node);
        continue;
      }
      self.map.insert(k, v);
    }
    for &k in self.map.keys() {
      if !self.order.contains(&k) {
        self.order.push(k);
      }
    }
  }
}

impl KeyTrie {
  pub fn node(&self) -> Option<&KeyTrieNode> {
    match self {
      Self::Node(n) => Some(n),
      _ => None,
    }
  }

  pub fn node_mut(&mut self) -> Option<&mut KeyTrieNode> {
    match self {
      Self::Node(n) => Some(n),
      _ => None,
    }
  }

  pub fn merge_nodes(&mut self, mut other: Self) {
    let node = std::mem::take(other.node_mut().expect("expected node"));
    self.node_mut().expect("expected node").merge(node);
  }

  pub fn search(&self, keys: &[KeyBinding]) -> Option<&KeyTrie> {
    let mut trie = self;
    for key in keys {
      trie = match trie {
        Self::Node(map) => map.map.get(key)?,
        Self::Command(_) | Self::Sequence(_) => return None,
      };
    }
    Some(trie)
  }
}

#[derive(Debug, Clone)]
pub enum KeymapResult {
  Pending(KeyTrieNode),
  Matched(KeyAction),
  MatchedSequence(Vec<KeyAction>),
  NotFound,
  Cancelled(Vec<KeyBinding>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyHintKind {
  Action,
  Prefix,
}

impl KeyHintKind {
  pub const fn as_str(self) -> &'static str {
    match self {
      Self::Action => "action",
      Self::Prefix => "prefix",
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyHintOption {
  pub key:   KeyBinding,
  pub label: String,
  pub kind:  KeyHintKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingKeyHints {
  pub pending: Vec<KeyBinding>,
  pub scope:   Option<String>,
  pub options: Vec<KeyHintOption>,
}

#[derive(Debug, Clone)]
pub struct Keymaps {
  pub map:    HashMap<Mode, KeyTrie>,
  state:      Vec<KeyBinding>,
  pub sticky: Option<KeyTrieNode>,
}

impl Keymaps {
  pub fn new(map: HashMap<Mode, KeyTrie>) -> Self {
    Self {
      map,
      state: Vec::new(),
      sticky: None,
    }
  }

  pub fn pending(&self) -> &[KeyBinding] {
    &self.state
  }

  pub fn sticky(&self) -> Option<&KeyTrieNode> {
    self.sticky.as_ref()
  }

  pub fn pending_hints(&self, mode: Mode) -> Option<PendingKeyHints> {
    if self.state.is_empty() {
      return None;
    }

    let keymap = self.map.get(&mode)?;
    let first = self.state.first().copied()?;
    let base = match &self.sticky {
      Some(trie) => KeyTrie::Node(trie.clone()),
      None => keymap.clone(),
    };
    let trie = base.search(&[first])?;
    let node = trie.search(&self.state[1..])?.node()?;

    let options = node
      .order
      .iter()
      .filter_map(|key| {
        let entry = node.map.get(key)?;
        let (label, kind) = key_hint_for_entry(entry);
        Some(KeyHintOption {
          key: *key,
          label,
          kind,
        })
      })
      .collect();

    Some(PendingKeyHints {
      pending: self.state.clone(),
      scope: non_empty_scope_name(&node.name),
      options,
    })
  }

  pub fn contains_key(&self, mode: Mode, binding: KeyBinding) -> bool {
    let keymap = self.map.get(&mode).expect("mode not in keymap");
    keymap
      .search(self.pending())
      .and_then(KeyTrie::node)
      .is_some_and(|n| n.map.contains_key(&binding))
  }

  pub fn get(&mut self, mode: Mode, key_event: &KeyEvent) -> KeymapResult {
    let keymap = match self.map.get(&mode) {
      Some(k) => k,
      None => return KeymapResult::NotFound,
    };

    let binding = KeyBinding::from_key_event(key_event);

    if matches!(binding.code, Key::Escape) {
      if !self.state.is_empty() {
        return KeymapResult::Cancelled(self.state.drain(..).collect());
      }
      self.sticky = None;
    }

    let first = self.state.first().copied().unwrap_or(binding);
    let base = match &self.sticky {
      Some(trie) => KeyTrie::Node(trie.clone()),
      None => keymap.clone(),
    };

    let trie = match base.search(&[first]) {
      Some(KeyTrie::Command(cmd)) => return KeymapResult::Matched(*cmd),
      Some(KeyTrie::Sequence(cmds)) => return KeymapResult::MatchedSequence(cmds.clone()),
      None => return KeymapResult::NotFound,
      Some(t) => t,
    };

    self.state.push(binding);
    match trie.search(&self.state[1..]) {
      Some(KeyTrie::Node(map)) => {
        if map.is_sticky {
          self.state.clear();
          self.sticky = Some(map.clone());
        }
        KeymapResult::Pending(map.clone())
      },
      Some(KeyTrie::Command(cmd)) => {
        self.state.clear();
        KeymapResult::Matched(*cmd)
      },
      Some(KeyTrie::Sequence(cmds)) => {
        self.state.clear();
        KeymapResult::MatchedSequence(cmds.clone())
      },
      None => KeymapResult::Cancelled(self.state.drain(..).collect()),
    }
  }
}

impl Default for Keymaps {
  fn default() -> Self {
    Self::new(default())
  }
}

pub fn handle_key<Ctx: DefaultContext>(ctx: &mut Ctx, key: KeyEvent) -> KeyOutcome {
  let mode = ctx.mode();
  let result = ctx.keymaps().get(mode, &key);

  match result {
    KeymapResult::Matched(action) => apply_actions(ctx, &[action]),
    KeymapResult::MatchedSequence(actions) => apply_actions(ctx, &actions),
    KeymapResult::Pending(_) | KeymapResult::Cancelled(_) => KeyOutcome::Handled,
    KeymapResult::NotFound => fallback_key(ctx, key),
  }
}

fn apply_actions<Ctx: DefaultContext>(ctx: &mut Ctx, actions: &[KeyAction]) -> KeyOutcome {
  let mut commands: SmallVec<[Command; 4]> = SmallVec::new();

  for action in actions {
    match *action {
      KeyAction::Command(command) => commands.push(command),
      KeyAction::Mode(mode) => apply_mode(ctx, mode),
      KeyAction::Named(name) => {
        if let Some(command) = command_from_name(name) {
          commands.push(command);
        } else if let Some(mode) = mode_from_name(name) {
          apply_mode(ctx, mode);
        }
      },
    }
  }

  match commands.len() {
    0 => KeyOutcome::Handled,
    1 => KeyOutcome::Command(commands[0]),
    _ => KeyOutcome::Commands(commands),
  }
}

fn apply_mode<Ctx: DefaultContext>(ctx: &mut Ctx, mode: Mode) {
  let previous_mode = ctx.mode();
  if previous_mode == Mode::Insert && mode != Mode::Insert {
    let _ = ctx.editor().document_mut().commit();
  }

  if mode != Mode::Insert {
    ctx.completion_menu_mut().clear();
    if let Some(state) = ctx.signature_help_mut() {
      state.clear();
    }
  }

  if mode == Mode::Select {
    let doc = ctx.editor().document_mut();
    let text = doc.text().slice(..);
    let selection = doc.selection().clone().transform(|range| {
      if range.is_empty() && range.head == text.len_chars() {
        Range::new(prev_grapheme_boundary(text, range.anchor), range.head)
      } else {
        range
      }
    });
    let _ = doc.set_selection(selection);
  }

  if mode == Mode::Command {
    ctx.command_prompt_mut().clear();
  }

  let palette_open = mode == Mode::Command;
  let palette_query = if palette_open {
    ctx.command_prompt_ref().input.clone()
  } else {
    String::new()
  };
  let palette_items = if palette_open {
    ctx
      .command_registry_ref()
      .all_commands()
      .into_iter()
      .map(|cmd| {
        let mut item = CommandPaletteItem::new(cmd.name);
        item.description = Some(cmd.doc.to_string());
        if !cmd.aliases.is_empty() {
          item.aliases = cmd.aliases.iter().map(|alias| alias.to_string()).collect();
        }
        item
      })
      .collect()
  } else {
    Vec::new()
  };

  {
    let palette = ctx.command_palette_mut();
    palette.is_open = palette_open;
    if palette.is_open {
      palette.query = palette_query;
      palette.items = palette_items;
      palette.selected = None;
    } else {
      palette.query.clear();
      palette.selected = None;
    }
  }

  ctx.set_mode(mode);
  if mode == Mode::Insert {
    ctx.lsp_signature_help();
  }
  ctx.request_render();
}

fn fallback_key<Ctx: DefaultContext>(ctx: &mut Ctx, key: KeyEvent) -> KeyOutcome {
  if ctx.mode() != Mode::Insert {
    return KeyOutcome::Continue;
  }

  let ctrl = key.modifiers.ctrl();
  let alt = key.modifiers.alt();

  match key.key {
    Key::Char(c) if !ctrl && !alt => KeyOutcome::Command(Command::InsertChar(c)),
    Key::Enter if !ctrl && !alt => KeyOutcome::Command(Command::InsertChar('\n')),
    Key::Backspace => KeyOutcome::Command(Command::DeleteChar),
    _ => KeyOutcome::Continue,
  }
}

fn mode_from_name(name: &str) -> Option<Mode> {
  match name {
    "normal_mode" => Some(Mode::Normal),
    "insert_mode" => Some(Mode::Insert),
    "select_mode" => Some(Mode::Select),
    "command_mode" => Some(Mode::Command),
    _ => None,
  }
}

fn non_empty_scope_name(name: &str) -> Option<String> {
  let trimmed = name.trim();
  if trimmed.is_empty() {
    None
  } else {
    Some(trimmed.to_string())
  }
}

fn key_hint_for_entry(entry: &KeyTrie) -> (String, KeyHintKind) {
  match entry {
    KeyTrie::Node(node) => {
      let label = node
        .name
        .trim()
        .strip_prefix(" ")
        .unwrap_or(node.name.trim());
      let label = if label.is_empty() {
        "more keys".to_string()
      } else {
        humanize_identifier(label)
      };
      (label, KeyHintKind::Prefix)
    },
    KeyTrie::Command(action) => (key_action_hint_label(*action), KeyHintKind::Action),
    KeyTrie::Sequence(actions) => {
      let mut labels = actions
        .iter()
        .map(|action| key_action_hint_label(*action))
        .collect::<Vec<_>>();
      let label = match labels.len() {
        0 => "sequence".to_string(),
        1 => labels.remove(0),
        n => format!("{} (+{} more)", labels.remove(0), n - 1),
      };
      (label, KeyHintKind::Action)
    },
  }
}

fn key_action_hint_label(action: KeyAction) -> String {
  match action {
    KeyAction::Command(command) => command_hint_label(command),
    KeyAction::Mode(mode) => {
      match mode {
        Mode::Normal => "normal mode".to_string(),
        Mode::Insert => "insert mode".to_string(),
        Mode::Select => "select mode".to_string(),
        Mode::Command => "command mode".to_string(),
      }
    },
    KeyAction::Named(name) => humanize_identifier(name),
  }
}

fn command_hint_label(command: Command) -> String {
  match command {
    Command::FilePicker => "find file".to_string(),
    Command::Search => "search".to_string(),
    Command::RSearch => "search backward".to_string(),
    Command::Save => "write file".to_string(),
    Command::Quit => "quit".to_string(),
    Command::LspGotoDefinition => "goto definition".to_string(),
    Command::LspHover => "hover".to_string(),
    Command::LspReferences => "references".to_string(),
    Command::LspDocumentSymbols => "document symbols".to_string(),
    Command::LspWorkspaceSymbols => "workspace symbols".to_string(),
    Command::LspCompletion => "completion".to_string(),
    Command::CompletionNext => "completion next".to_string(),
    Command::CompletionPrev => "completion previous".to_string(),
    Command::CompletionAccept => "completion accept".to_string(),
    Command::CompletionCancel => "completion cancel".to_string(),
    Command::CompletionDocsScrollUp => "completion docs up".to_string(),
    Command::CompletionDocsScrollDown => "completion docs down".to_string(),
    Command::LspSignatureHelp => "signature help".to_string(),
    Command::LspCodeActions => "code action".to_string(),
    Command::LspFormat => "format document".to_string(),
    _ => {
      let variant = format!("{command:?}");
      let head = variant.split(['(', '{']).next().unwrap_or("command").trim();
      humanize_identifier(head)
    },
  }
}

fn humanize_identifier(input: &str) -> String {
  let mut out = String::new();
  let mut last_was_space = false;
  let mut previous_is_lower_or_digit = false;

  for ch in input.chars() {
    if ch == '_' || ch == '-' {
      if !last_was_space && !out.is_empty() {
        out.push(' ');
      }
      last_was_space = true;
      previous_is_lower_or_digit = false;
      continue;
    }

    if ch.is_uppercase() {
      if previous_is_lower_or_digit && !last_was_space {
        out.push(' ');
      }
      for lower in ch.to_lowercase() {
        out.push(lower);
      }
      last_was_space = false;
      previous_is_lower_or_digit = false;
      continue;
    }

    out.push(ch.to_ascii_lowercase());
    last_was_space = false;
    previous_is_lower_or_digit = ch.is_ascii_lowercase() || ch.is_ascii_digit();
  }

  out.trim().to_string()
}

pub fn action_from_name(name: &'static str) -> KeyAction {
  if let Some(mode) = mode_from_name(name) {
    return KeyAction::Mode(mode);
  }

  if let Some(command) = command_from_name(name) {
    return KeyAction::Command(command);
  }

  KeyAction::Named(name)
}

pub fn default() -> HashMap<Mode, KeyTrie> {
  let normal = crate::keymap!({ "Normal mode"
    "h" | "left" => move_char_left,
    "j" | "down" => move_visual_line_down,
    "k" | "up" => move_visual_line_up,
    "l" | "right" => move_char_right,

    "t" => find_till_char,
    "f" => find_next_char,
    "T" => till_prev_char,
    "F" => find_prev_char,
    "r" => replace,
    "R" => replace_with_yanked,
    "A-." =>  repeat_last_motion,

    "~" => switch_case,
    "`" => switch_to_lowercase,
    "A-`" => switch_to_uppercase,

    "home" => goto_line_start,
    "end" => goto_line_end,

    "w" => move_next_word_start,
    "b" => move_prev_word_start,
    "e" => move_next_word_end,

    "W" => move_next_long_word_start,
    "B" => move_prev_long_word_start,
    "E" => move_next_long_word_end,

    "v" => select_mode,
    // "G" => goto_line,
    "g" => { "Goto"
      "g" => goto_file_start,
      "|" => goto_column,
      "e" => goto_last_line,
      // "f" => goto_file, // REIMAGINE THIS
      "h" => goto_line_start,
      "l" => goto_line_end,
      "s" => goto_first_nonwhitespace,
      "d" => lsp_goto_definition,
      // "D" => goto_declaration,
      // "y" => goto_type_definition,
      "r" => lsp_references,
      // "i" => goto_implementation,
      "t" => goto_window_top,
      "c" => goto_window_center,
      "b" => goto_window_bottom,
      "a" => goto_last_accessed_file,
      "m" => goto_last_modified_file,
      "n" => goto_next_buffer,
      "p" => goto_previous_buffer,
      // "k" => move_line_up,
      // "j" => move_line_down,
      "." => goto_last_modification,
      // "w" => goto_word,
    },
    ":" => command_mode,

    "i" => insert_mode,
    "I" => insert_at_line_start,
    "a" => append_mode,
    "A" => insert_at_line_end,
    "o" => open_below,
    "O" => open_above,

    "d" => delete_selection,
    "A-d" => delete_selection_noyank,
    "c" => change_selection,
    "A-c" => change_selection_noyank,

    "C" => copy_selection_on_next_line,
    "A-C" => copy_selection_on_prev_line,

    "s" => select_regex,
    // "A-s" => split_selection_on_newline,
    // "A-minus" => merge_selections,
    // "A-_" => merge_consecutive_selections,
    // "S" => split_selection,
    // ";" => collapse_selection,
    // "A-;" => flip_selections,
    // "A-o" | "A-up" => expand_selection,
    // "A-i" | "A-down" => shrink_selection,
    // "A-I" | "A-S-down" => select_all_children,
    // "A-p" | "A-left" => select_prev_sibling,
    // "A-n" | "A-right" => select_next_sibling,
    "A-e" => move_parent_node_end,
    "A-b" => move_parent_node_start,
    // "A-a" => select_all_siblings,

    "%" => select_all,
    "x" => extend_line_below,
    "X" => extend_to_line_bounds,
    "A-x" => shrink_to_line_bounds,

    "m" => { "Match"
      "m" => match_brackets,
      "s" => surround_add,
      "r" => surround_replace,
      "d" => surround_delete,
      "a" => select_textobject_around,
      "i" => select_textobject_inner,
    },
    "[" => { "Left bracket"
      // "d" => goto_prev_diag,
      // "D" => goto_first_diag,
      // "g" => goto_prev_change,
      // "G" => goto_first_change,
      // "f" => goto_prev_function,
      // "t" => goto_prev_class,
      // "a" => goto_prev_parameter,
      // "c" => goto_prev_comment,
      // "e" => goto_prev_entry,
      // "T" => goto_prev_test,
      // "p" => goto_prev_paragraph,
      // "x" => goto_prev_xml_element,
      // "space" => add_newline_above,
    },
    "]" => { "Right bracket"
      // "d" => goto_next_diag,
      // "D" => goto_last_diag,
      // "g" => goto_next_change,
      // "G" => goto_last_change,
      // "f" => goto_next_function,
      // "t" => goto_next_class,
      // "a" => goto_next_parameter,
      // "c" => goto_next_comment,
      // "e" => goto_next_entry,
      // "T" => goto_next_test,
      // "p" => goto_next_paragraph,
      // "x" => goto_next_xml_element,
      // "space" => add_newline_below,
    },

    "/" => search,
    "?" => rsearch,
    "n" => search_next,
    "N" => search_prev,
    // "*" => search_selection_detect_word_boundaries,
    // "A-*" => search_selection,

    "u" => undo,
    "U" => redo,
    "A-u" => earlier,
    "A-U" => later,

    "y" => yank,
    // yank_all
    "p" => paste_after,
    // paste_all
    "P" => paste_before,

    "Q" => record_macro,
    "q" => replay_macro,

    ">" => indent,
    "<" => unindent,
    "=" => lsp_format,
    // "J" => join_selections,
    // "A-J" => join_selections_space,
    // "K" => keep_selections,
    // "A-K" => remove_selections,

    // "," => keep_primary_selection,
    // "A-," => remove_primary_selection,

    // "&" => align_selections,
    // "_" => trim_selections,

    // "(" => rotate_selections_backward,
    // ")" => rotate_selections_forward,
    // "A-(" => rotate_selection_contents_backward,
    // "A-)" => rotate_selection_contents_forward,

    // "A-:" => ensure_selections_forward,

    "esc" => normal_mode,
    "C-b" | "pageup" => page_up,
    "C-f" | "pagedown" => page_down,
    // "C-u" => page_cursor_half_up,
    // "C-d" => page_cursor_half_down,

    "C-w" => { "Window"
      // "C-w" | "w" => rotate_view,
      // "C-s" | "s" => hsplit,
      // "C-v" | "v" => vsplit,
      // "C-t" | "t" => transpose_view,
      // "f" => goto_file_hsplit,
      // "F" => goto_file_vsplit,
      // "C-q" | "q" => wclose,
      // "C-o" | "o" => wonly,
      // "C-h" | "h" | "left" => jump_view_left,
      // "C-j" | "j" | "down" => jump_view_down,
      // "C-k" | "k" | "up" => jump_view_up,
      // "C-l" | "l" | "right" => jump_view_right,
      // "L" => swap_view_right,
      // "K" => swap_view_up,
      // "H" => swap_view_left,
      // "J" => swap_view_down,
      // "n" => { "New split scratch buffer"
      //   "C-s" | "s" => hsplit_new,
      //   "C-v" | "v" => vsplit_new,
      // },
    },

    // "C-c" => toggle_comments,

    // "C-i" | "tab" => jump_forward,
    // "C-o" => jump_backward,
    // "C-s" => save_selection,

    "space" => { "Space"
      "f" => file_picker,
      // "F" => file_picker_in_current_directory,
      // "e" => file_explorer,
      // "E" => file_explorer_in_current_buffer_directory,
      // "b" => buffer_picker,
      // "j" => jumplist_picker,
      "s" => lsp_document_symbols,
      "S" => lsp_workspace_symbols,
      // "d" => diagnostics_picker,
      // "D" => workspace_diagnostics_picker,
      // "g" => changed_file_picker,
      "a" => lsp_code_actions,
      // "'" => last_picker,
      // "G" => { "Debug (experimental)" sticky=true
      //   "l" => dap_launch,
      //   "r" => dap_restart,
      //   "b" => dap_toggle_breakpoint,
      //   "c" => dap_continue,
      //   "h" => dap_pause,
      //   "i" => dap_step_in,
      //   "o" => dap_step_out,
      //   "n" => dap_next,
      //   "v" => dap_variables,
      //   "t" => dap_terminate,
      //   "C-c" => dap_edit_condition,
      //   "C-l" => dap_edit_log,
      //   "s" => { "Switch"
      //     "t" => dap_switch_thread,
      //     "f" => dap_switch_stack_frame,
      //   },
      //   "e" => dap_enable_exceptions,
      //   "E" => dap_disable_exceptions,
      // },
      // "w" => { "Window"
      //   "C-w" | "w" => rotate_view,
      //   "C-s" | "s" => hsplit,
      //   "C-v" | "v" => vsplit,
      //   "C-t" | "t" => transpose_view,
      //   "f" => goto_file_hsplit,
      //   "F" => goto_file_vsplit,
      //   "C-q" | "q" => wclose,
      //   "C-o" | "o" => wonly,
      //   "C-h" | "h" | "left" => jump_view_left,
      //   "C-j" | "j" | "down" => jump_view_down,
      //   "C-k" | "k" | "up" => jump_view_up,
      //   "C-l" | "l" | "right" => jump_view_right,
      //   "H" => swap_view_left,
      //   "J" => swap_view_down,
      //   "K" => swap_view_up,
      //   "L" => swap_view_right,
      //   "n" => { "New split scratch buffer"
      //     "C-s" | "s" => hsplit_new,
      //     "C-v" | "v" => vsplit_new,
      //   },
      // },
      // "y" => yank_to_clipboard,
      // "Y" => yank_main_selection_to_clipboard,
      // "p" => paste_clipboard_after,
      // "P" => paste_clipboard_before,
      // "R" => replace_selections_with_clipboard,
      // "/" => global_search,
      "k" => lsp_hover,
      // "r" => rename_symbol,
      // "h" => select_references_to_symbol_under_cursor,
      // "c" => toggle_comments,
      // "C" => toggle_block_comments,
      // "A-c" => toggle_line_comments,
      // "?" => command_palette,
    },
    "z" => { "View"
      // "z" | "c" => align_view_center,
      // "t" => align_view_top,
      // "b" => align_view_bottom,
      // "m" => align_view_middle,
      // "k" | "up" => scroll_up,
      // "j" | "down" => scroll_down,
      "C-b" | "pageup" => page_up,
      "C-f" | "pagedown" => page_down,
      // "C-u" | "backspace" => page_cursor_half_up,
      // "C-d" | "space" => page_cursor_half_down,

      // "/" => search,
      // "?" => rsearch,
      // "n" => search_next,
      // "N" => search_prev,
    },
    "Z" => { "View" sticky=true
      // "z" | "c" => align_view_center,
      // "t" => align_view_top,
      // "b" => align_view_bottom,
      // "m" => align_view_middle,
      // "k" | "up" => scroll_up,
      // "j" | "down" => scroll_down,
      "C-b" | "pageup" => page_up,
      "C-f" | "pagedown" => page_down,
      // "C-u" | "backspace" => page_cursor_half_up,
      // "C-d" | "space" => page_cursor_half_down,

      // "/" => search,
      // "?" => rsearch,
      // "n" => search_next,
      // "N" => search_prev,
    },

    // "\"" => select_register,
    // "|" => shell_pipe,
    // "A-|" => shell_pipe_to,
    // "!" => shell_insert_output,
    // "A-!" => shell_append_output,
    // "$" => shell_keep_pipe,
    // "C-z" => suspend,

    // "C-a" => increment,
    // "C-x" => decrement,
  });
  let mut select = normal.clone();
  select.merge_nodes(crate::keymap!({ "Select mode"
    "h" | "left" => extend_char_left,
    "j" | "down" => extend_visual_line_down,
    "k" | "up" => extend_visual_line_up,
    "l" | "right" => extend_char_right,

    "w" => extend_next_word_start,
    "b" => extend_prev_word_start,
    "e" => extend_next_word_end,
    "W" => extend_next_long_word_start,
    "B" => extend_prev_long_word_start,
    "E" => extend_next_long_word_end,

    "A-e" => extend_parent_node_end,
    "A-b" => extend_parent_node_start,

    "n" => extend_search_next,
    "N" => extend_search_prev,

    "t" => extend_till_char,
    "f" => extend_next_char,
    "T" => extend_till_prev_char,
    "F" => extend_prev_char,

    "home" => extend_to_line_start,
    "end" => extend_to_line_end,
    // "esc" => exit_select_mode,

    "v" => normal_mode,
    "g" => { "Goto"
      "g" => extend_to_file_start,
      "|" => extend_to_column,
      "e" => extend_to_last_line,
      "k" => extend_line_up,
      "j" => extend_line_down,
      // "w" => extend_to_word,
    },
  }));
  let insert = crate::keymap!({ "Insert mode"
    "esc" => normal_mode,

    "C-s" => commit_undo_checkpoint,
    "C-x" => lsp_completion,
    "C-n" => completion_next,
    "C-p" => completion_prev,
    "C-y" => completion_accept,
    "C-e" => completion_cancel,
    "A-k" => lsp_signature_help,
    // "C-r" => insert_register,

    "C-w" | "A-backspace" => delete_word_backward,
    "A-d" | "A-del" => delete_word_forward,
    "C-u" => kill_to_line_start,
    "C-k" => kill_to_line_end,
    "C-h" | "backspace" | "S-backspace" => delete_char_backward,
    "C-d" | "del" => delete_char_forward,
    "C-j" | "ret" => insert_newline,
    "tab" => smart_tab,
    "S-tab" => insert_tab,

    "up" => move_visual_line_up,
    "down" => move_visual_line_down,
    "left" => move_char_left,
    "right" => move_char_right,
    "pageup" => completion_docs_scroll_up,
    "pagedown" => completion_docs_scroll_down,
    "home" => goto_line_start,
    // "end" => goto_line_end_newline,
  });

  let mut command = normal.clone();
  command.merge_nodes(crate::keymap!({ "Command"
    "esc" => normal_mode,
  }));

  let mut map = HashMap::new();
  map.insert(Mode::Normal, normal);
  map.insert(Mode::Select, select);
  map.insert(Mode::Insert, insert);
  map.insert(Mode::Command, command);
  map
}

#[macro_export]
macro_rules! key {
  ($name:ident) => {{ $crate::keymap::binding_from_ident(stringify!($name)) }};
  ($lit:literal) => {{ $crate::keymap::binding_from_literal($lit) }};
}

#[macro_export]
macro_rules! keymap {
  ({ $name:literal $($rest:tt)* }) => {
    {
      use std::collections::HashMap;
      let mut _map: HashMap<$crate::keymap::KeyBinding, $crate::keymap::KeyTrie> = HashMap::new();
      let mut _order: Vec<$crate::keymap::KeyBinding> = Vec::new();
      $crate::keymap!(@pairs _map, _order; $($rest)*);
      $crate::keymap::KeyTrie::Node($crate::keymap::KeyTrieNode::new($name, _map, _order))
    }
  };

  (@pairs $map:ident, $order:ident; sticky=true $($rest:tt)*) => {
    $crate::keymap!(@pairs $map, $order; $($rest)* );
  };

  (@pairs $map:ident, $order:ident; $($k:tt)|+ => $cmd:ident, $($rest:tt)*) => {
    $(
      let _k = $crate::key!($k);
      let _cmd = $crate::keymap::action_from_name(stringify!($cmd));
      if $map.insert(_k, $crate::keymap::KeyTrie::Command(_cmd)).is_none() { $order.push(_k); }
    )+
    $crate::keymap!(@pairs $map, $order; $($rest)*);
  };

  (@pairs $map:ident, $order:ident; $k:tt => $cmd:ident, $($rest:tt)*) => {
    let _k = $crate::key!($k);
    let _cmd = $crate::keymap::action_from_name(stringify!($cmd));
    if $map.insert(_k, $crate::keymap::KeyTrie::Command(_cmd)).is_none() { $order.push(_k); }
    $crate::keymap!(@pairs $map, $order; $($rest)*);
  };

  (@pairs $map:ident, $order:ident; $k:tt => { $name:literal $($inner:tt)* }, $($rest:tt)*) => {
    let _k = $crate::key!($k);
    let _node = $crate::keymap!({ $name $($inner)* });
    if $map.insert(_k, _node).is_none() { $order.push(_k); }
    $crate::keymap!(@pairs $map, $order; $($rest)*);
  };

  (@pairs $map:ident, $order:ident; ) => {};
}
