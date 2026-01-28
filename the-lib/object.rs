//! Syntax-based selection expansion and structural navigation.
//!
//! This module provides tree-sitter-backed selection transforms such as
//! expanding to parent nodes, shrinking to child nodes, and selecting siblings.
//! All operations are pure transforms: selection in → selection out.
//!
//! # Example: expand a selection to its parent node
//!
//! ```no_run
//! use ropey::Rope;
//! use the_lib::object::expand_selection;
//! use the_lib::selection::{Range, Selection};
//! use the_lib::syntax::{Loader, Syntax};
//! # use std::collections::HashMap;
//! # use the_lib::syntax::config::{
//! #   Configuration, FileType, LanguageConfiguration, LanguageServicesConfig, SyntaxLanguageConfig,
//! # };
//! # use the_lib::syntax::resources::NullResources;
//!
//! let text = Rope::from("fn main() { 1 + 2 }");
//! let selection = Selection::point(3); // cursor on `m` in `main`
//!
//! # let language = LanguageConfiguration {
//! #   syntax: SyntaxLanguageConfig {
//! #     language_id: "rust".into(),
//! #     scope: "source.rust".into(),
//! #     file_types: vec![FileType::Extension("rs".into())],
//! #     shebangs: Vec::new(),
//! #     comment_tokens: None,
//! #     block_comment_tokens: None,
//! #     text_width: None,
//! #     soft_wrap: None,
//! #     auto_format: false,
//! #     path_completion: None,
//! #     word_completion: None,
//! #     grammar: None,
//! #     injection_regex: None,
//! #     indent: None,
//! #     auto_pairs: None,
//! #     rulers: None,
//! #     rainbow_brackets: None,
//! #   },
//! #   services: LanguageServicesConfig::default(),
//! # };
//! # let config = Configuration { language: vec![language], language_server: HashMap::new() };
//! # let loader = Loader::new(config, NullResources).expect("loader");
//! # let language = loader.language_for_name("rust").expect("lang");
//! let syntax = Syntax::new(text.slice(..), language, &loader).expect("syntax");
//! let expanded = expand_selection(&syntax, text.slice(..), selection);
//! # let _ = expanded;
//! ```
use ropey::RopeSlice;
use tree_house::tree_sitter::Node;

use crate::{
  movement::Direction,
  selection::{
    Range,
    Selection,
  },
  syntax::Syntax,
};

pub fn expand_selection(syntax: &Syntax, text: RopeSlice, selection: Selection) -> Selection {
  selection.transform(|range| {
    let (from, to) = range.into_byte_range(&text);
    let from = from as u32;
    let to = to as u32;
    let byte_range = from..to;

    let Some(mut node) = node_for_range(syntax, text, range) else {
      return range;
    };

    while node.byte_range() == byte_range {
      let Some(parent) = node.parent() else {
        break;
      };
      node = parent;
    }

    Range::from_node(node, text, range.direction())
  })
}

pub fn shrink_selection(syntax: &Syntax, text: RopeSlice, selection: Selection) -> Selection {
  selection.transform(|range| {
    let Some(node) = node_for_range(syntax, text, range) else {
      return range;
    };
    let (from, to) = range.into_byte_range(&text);
    let from = from as u32;
    let to = to as u32;

    let Some(child) = named_child_containing(node, from, to) else {
      return range;
    };

    Range::from_node(child, text, range.direction())
  })
}

pub fn select_next_sibling(syntax: &Syntax, text: RopeSlice, selection: Selection) -> Selection {
  select_sibling(syntax, text, selection, Direction::Forward)
}

pub fn select_all_siblings(syntax: &Syntax, text: RopeSlice, selection: Selection) -> Selection {
  let fallback = selection.clone();
  selection
    .transform_iter(move |range| {
      let Some(mut node) = node_for_range(syntax, text, range) else {
        return vec![range].into_iter();
      };

      let mut parent = node.parent();
      while let Some(current) = parent {
        if current.named_child_count() > 1 {
          node = current;
          break;
        }
        parent = current.parent();
      }

      if node.named_child_count() <= 1 {
        return vec![range].into_iter();
      }

      select_children(node, text, range).into_iter()
    })
    .unwrap_or(fallback)
}

pub fn select_all_children(syntax: &Syntax, text: RopeSlice, selection: Selection) -> Selection {
  let fallback = selection.clone();
  selection
    .transform_iter(move |range| {
      let Some(node) = node_for_range(syntax, text, range) else {
        return vec![range].into_iter();
      };
      select_children(node, text, range).into_iter()
    })
    .unwrap_or(fallback)
}

fn select_children(node: Node<'_>, text: RopeSlice, range: Range) -> Vec<Range> {
  let children = node
    .children()
    .filter(|child| child.is_named())
    .map(|child| Range::from_node(child, text, range.direction()))
    .collect::<Vec<_>>();

  if !children.is_empty() {
    children
  } else {
    vec![range]
  }
}

pub fn select_prev_sibling(syntax: &Syntax, text: RopeSlice, selection: Selection) -> Selection {
  select_sibling(syntax, text, selection, Direction::Backward)
}

fn node_for_range<'a>(syntax: &'a Syntax, text: RopeSlice<'a>, range: Range) -> Option<Node<'a>> {
  let (from, to) = range.into_byte_range(&text);
  let from = from as u32;
  let to = to as u32;
  let root = syntax.tree_for_byte_range(from, to).root_node();

  root
    .named_descendant_for_byte_range(from, to)
    .or_else(|| root.descendant_for_byte_range(from, to))
}

fn named_child_containing<'a>(node: Node<'a>, from: u32, to: u32) -> Option<Node<'a>> {
  node.children().find(|child| {
    child.is_named() && child.byte_range().start <= from && child.byte_range().end >= to
  })
}

fn next_named_sibling(node: Node<'_>, direction: Direction) -> Option<Node<'_>> {
  let mut sibling = match direction {
    Direction::Forward => node.next_sibling(),
    Direction::Backward => node.prev_sibling(),
  };

  while let Some(next) = sibling {
    if next.is_named() {
      return Some(next);
    }
    sibling = match direction {
      Direction::Forward => next.next_sibling(),
      Direction::Backward => next.prev_sibling(),
    };
  }
  None
}

fn select_sibling(
  syntax: &Syntax,
  text: RopeSlice,
  selection: Selection,
  direction: Direction,
) -> Selection {
  selection.transform(|range| {
    let Some(mut node) = node_for_range(syntax, text, range) else {
      return range;
    };

    loop {
      if let Some(sibling) = next_named_sibling(node.clone(), direction) {
        node = sibling;
        break;
      }
      let Some(parent) = node.parent() else {
        return range;
      };
      node = parent;
    }

    Range::from_node(node, text, direction)
  })
}
