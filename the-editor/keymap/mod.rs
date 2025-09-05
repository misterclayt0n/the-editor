use std::collections::HashMap;

use the_editor_renderer::Key;

use crate::core::document::Document;

pub mod default;
pub mod macros;

// macros are exported at crate root via #[macro_export]

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Mode {
  Normal,
  Insert,
  Visual,
}

#[derive(Clone, Copy, PartialEq)]
pub enum Command {
  Execute(fn(&mut Document)),
  EnterInsertMode,
  ExitInsertMode,
  EnterVisualMode,
  ExitVisualMode,
}

impl std::fmt::Debug for Command {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Command::Execute(_) => write!(f, "Execute(..)"),
      Command::EnterInsertMode => write!(f, "EnterInsertMode"),
      Command::ExitInsertMode => write!(f, "ExitInsertMode"),
      Command::EnterVisualMode => write!(f, "EnterVisualMode"),
      Command::ExitVisualMode => write!(f, "ExitVisualMode"),
    }
  }
}

#[derive(Debug, Clone, PartialEq)]
pub enum KeyTrie {
  Command(Command),
  Node(KeyTrieNode),
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct KeyTrieNode {
  pub name:      String,
  pub map:       HashMap<Key, KeyTrie>,
  pub order:     Vec<Key>,
  pub is_sticky: bool,
}

impl KeyTrieNode {
  pub fn new(name: &str, map: HashMap<Key, KeyTrie>, order: Vec<Key>) -> Self {
    Self {
      name: name.to_string(),
      map,
      order,
      is_sticky: false,
    }
  }

  pub fn merge(&mut self, mut other: Self) {
    for (k, v) in std::mem::take(&mut other.map) {
      if let Some(KeyTrie::Node(node)) = self.map.get_mut(&k) {
        if let KeyTrie::Node(other_node) = v {
          node.merge(other_node);
          continue;
        }
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

  pub fn search(&self, keys: &[Key]) -> Option<&KeyTrie> {
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

#[derive(Debug, Clone, PartialEq)]
pub enum KeymapResult {
  Pending(KeyTrieNode),
  Matched(Command),
  NotFound,
  Cancelled(Vec<Key>),
}

pub struct Keymaps {
  pub map:    HashMap<Mode, KeyTrie>,
  state:      Vec<Key>,
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

  pub fn pending(&self) -> &[Key] {
    &self.state
  }

  // Backwards-compat: keep signature for now, but unused
  #[allow(dead_code)]
  pub fn pending_keys(&self) -> &[Key] {
    &self.state
  }
  pub fn sticky(&self) -> Option<&KeyTrieNode> {
    self.sticky.as_ref()
  }

  pub fn contains_key(&self, mode: Mode, key: Key) -> bool {
    let keymap = self.map.get(&mode).expect("mode not in keymap");
    keymap
      .search(self.pending())
      .and_then(KeyTrie::node)
      .map_or(false, |n| n.map.contains_key(&key))
  }

  pub fn get(&mut self, mode: Mode, key: Key) -> KeymapResult {
    let keymap = match self.map.get(&mode) {
      Some(k) => k,
      None => return KeymapResult::NotFound,
    };

    // ESC cancels pending and clears sticky if no pending
    if matches!(key, Key::Escape) {
      if !self.state.is_empty() {
        return KeymapResult::Cancelled(self.state.drain(..).collect());
      }
      self.sticky = None;
    }

    let first = self.state.first().copied().unwrap_or(key);
    let base = match &self.sticky {
      Some(trie) => KeyTrie::Node(trie.clone()),
      None => keymap.clone(),
    };

    let trie = match base.search(&[first]) {
      Some(KeyTrie::Command(cmd)) => return KeymapResult::Matched(*cmd),
      None => return KeymapResult::NotFound,
      Some(t) => t,
    };

    self.state.push(key);
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
