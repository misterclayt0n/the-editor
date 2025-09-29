use std::{
  collections::HashMap,
  fmt,
  str::FromStr,
};

use serde::{
  Deserialize,
  Serialize,
};
use the_editor_renderer::{
  Key,
  KeyPress,
};

use crate::core::commands;

pub mod default;
pub mod macros;

// macros are exported at crate root via #[macro_export]

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct KeyBinding {
  pub code:  Key,
  pub shift: bool,
  pub ctrl:  bool,
  pub alt:   bool,
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
      Key::Escape => "esc".to_string(),
      Key::Backspace => "bs".to_string(),
      Key::Tab => "tab".to_string(),
      Key::Delete => "del".to_string(),
      Key::Home => "home".to_string(),
      Key::End => "end".to_string(),
      Key::PageUp => "pgup".to_string(),
      Key::PageDown => "pgdown".to_string(),
      Key::Left => "left".to_string(),
      Key::Right => "right".to_string(),
      Key::Up => "up".to_string(),
      Key::Down => "down".to_string(),
      // Key::F(n) => format!("F{}", n), // Not supported in renderer yet
      Key::Other => "other".to_string(),
    };

    result.push_str(&key_str);
    write!(f, "{}", result)
  }
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

  pub const fn from_key_press(press: &KeyPress) -> Self {
    Self {
      code:  press.code,
      shift: press.shift,
      ctrl:  press.ctrl,
      alt:   press.alt,
    }
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
    "esc" | "escape" => Ok(Key::Escape),
    "backspace" | "bs" => Ok(Key::Backspace),
    "tab" => Ok(Key::Tab),
    "delete" | "del" => Ok(Key::Delete),
    "home" => Ok(Key::Home),
    "end" => Ok(Key::End),
    "pageup" | "pgup" => Ok(Key::PageUp),
    "pagedown" | "pgdown" => Ok(Key::PageDown),
    "left" => Ok(Key::Left),
    "right" => Ok(Key::Right),
    "up" => Ok(Key::Up),
    "down" => Ok(Key::Down),
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

pub fn binding_from_ident(name: &str) -> KeyBinding {
  KeyBinding::from_str(name).unwrap_or_else(|err| panic!("invalid key identifier '{name}': {err}"))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub enum Mode {
  Normal,
  Insert,
  Select,
  Command,
}

#[derive(Clone, Copy)]
pub enum Command {
  Execute(fn(&mut commands::Context)),
}

impl std::fmt::Debug for Command {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Command::Execute(_) => write!(f, "Execute(..)"),
    }
  }
}

#[derive(Debug, Clone)]
pub enum KeyTrie {
  Command(Command),
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
      KeyTrie::Node(n) => Some(n),
      _ => None,
    }
  }
  pub fn node_mut(&mut self) -> Option<&mut KeyTrieNode> {
    match self {
      KeyTrie::Node(n) => Some(n),
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
        KeyTrie::Node(map) => map.map.get(key)?,
        KeyTrie::Command(_) => return None,
      };
    }
    Some(trie)
  }
}

#[derive(Debug, Clone)]
pub enum KeymapResult {
  Pending(KeyTrieNode),
  Matched(Command),
  NotFound,
  Cancelled(Vec<KeyBinding>),
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

  // Backwards-compat: keep signature for now, but unused
  #[allow(dead_code)]
  pub fn pending_keys(&self) -> &[KeyBinding] {
    &self.state
  }
  pub fn sticky(&self) -> Option<&KeyTrieNode> {
    self.sticky.as_ref()
  }

  pub fn contains_key(&self, mode: Mode, binding: KeyBinding) -> bool {
    let keymap = self.map.get(&mode).expect("mode not in keymap");
    keymap
      .search(self.pending())
      .and_then(KeyTrie::node)
      .is_some_and(|n| n.map.contains_key(&binding))
  }

  pub fn get(&mut self, mode: Mode, key_press: &KeyPress) -> KeymapResult {
    let keymap = match self.map.get(&mode) {
      Some(k) => k,
      None => return KeymapResult::NotFound,
    };

    let binding = KeyBinding::from_key_press(key_press);

    // ESC cancels pending and clears sticky if no pending
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
      None => KeymapResult::Cancelled(self.state.drain(..).collect()),
    }
  }
}

impl Default for Keymaps {
  fn default() -> Self {
    Self::new(default::default())
  }
}

/// Merge default config keys with user-provided overrides.
pub fn merge_keys(dst: &mut HashMap<Mode, KeyTrie>, mut delta: HashMap<Mode, KeyTrie>) {
  for (mode, keys) in dst.iter_mut() {
    keys.merge_nodes(
      delta
        .remove(mode)
        .unwrap_or_else(|| KeyTrie::Node(KeyTrieNode::default())),
    );
  }
}
