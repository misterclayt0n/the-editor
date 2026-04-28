use ropey::RopeSlice;

use crate::{
  movement::Direction,
  selection::{
    CursorId,
    CursorRange,
    Range,
    Selection,
  },
  syntax::{
    Syntax,
    tree_sitter::Node,
  },
};

pub fn expand_selection(syntax: &Syntax, text: RopeSlice, selection: Selection) -> Selection {
  selection.transform(|range| {
    let Some(mut node) = node_for_range(syntax, text, range) else {
      return range;
    };

    let byte_range = byte_range(text, range);
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

    let byte_range = byte_range(text, range);
    let Some(child) = named_child_containing(node, byte_range.clone()) else {
      return range;
    };

    Range::from_node(child, text, range.direction())
  })
}

pub fn select_next_sibling(syntax: &Syntax, text: RopeSlice, selection: Selection) -> Selection {
  select_sibling(syntax, text, selection, Direction::Forward)
}

pub fn select_prev_sibling(syntax: &Syntax, text: RopeSlice, selection: Selection) -> Selection {
  select_sibling(syntax, text, selection, Direction::Backward)
}

pub fn select_all_siblings(syntax: &Syntax, text: RopeSlice, selection: Selection) -> Selection {
  selection
    .flat_map_entries(|cursor| {
      let Some(node) = node_for_range(syntax, text, cursor.range) else {
        return vec![cursor].into_iter();
      };

      let Some(parent) = nearest_parent_with_multiple_named_children(node) else {
        return vec![cursor].into_iter();
      };

      ranges_to_cursors(
        named_child_ranges(parent, text, cursor.range.direction()),
        cursor.id,
        cursor.range,
      )
      .into_iter()
    })
    .expect("object selection fan-out must preserve non-empty unique cursor IDs")
}

pub fn select_all_children(syntax: &Syntax, text: RopeSlice, selection: Selection) -> Selection {
  selection
    .flat_map_entries(|cursor| {
      let Some(node) = node_for_range(syntax, text, cursor.range) else {
        return vec![cursor].into_iter();
      };

      ranges_to_cursors(
        named_child_ranges(node, text, cursor.range.direction()),
        cursor.id,
        cursor.range,
      )
      .into_iter()
    })
    .expect("object selection fan-out must preserve non-empty unique cursor IDs")
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
        return Range::from_node(sibling, text, direction);
      }

      let Some(parent) = node.parent() else {
        return range;
      };
      node = parent;
    }
  })
}

fn node_for_range<'a>(syntax: &'a Syntax, text: RopeSlice<'a>, range: Range) -> Option<Node<'a>> {
  let byte_range = byte_range(text, range);
  let root = syntax
    .tree_for_byte_range(byte_range.start, byte_range.end)
    .root_node();

  root
    .named_descendant_for_byte_range(byte_range.start, byte_range.end)
    .or_else(|| root.descendant_for_byte_range(byte_range.start, byte_range.end))
}

fn byte_range(text: RopeSlice, range: Range) -> std::ops::Range<u32> {
  let (from, to) = range.into_byte_range(&text);
  from as u32..to as u32
}

fn named_child_containing<'a>(
  node: Node<'a>,
  byte_range: std::ops::Range<u32>,
) -> Option<Node<'a>> {
  node.children().find(|child| {
    child.is_named()
      && child.byte_range().start <= byte_range.start
      && child.byte_range().end >= byte_range.end
  })
}

fn nearest_parent_with_multiple_named_children(mut node: Node<'_>) -> Option<Node<'_>> {
  while let Some(parent) = node.parent() {
    if parent.named_child_count() > 1 {
      return Some(parent);
    }
    node = parent;
  }
  None
}

fn named_child_ranges(node: Node<'_>, text: RopeSlice, direction: Direction) -> Vec<Range> {
  let ranges = node
    .children()
    .filter(|child| child.is_named())
    .map(|child| Range::from_node(child, text, direction))
    .collect::<Vec<_>>();

  ranges
}

fn ranges_to_cursors(ranges: Vec<Range>, first_id: CursorId, fallback: Range) -> Vec<CursorRange> {
  if ranges.is_empty() {
    return vec![CursorRange {
      id:    first_id,
      range: fallback,
    }];
  }

  ranges
    .into_iter()
    .enumerate()
    .map(|(idx, range)| {
      let id = if idx == 0 {
        first_id
      } else {
        CursorId::fresh()
      };
      CursorRange { id, range }
    })
    .collect()
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

#[cfg(test)]
mod test {
  use std::{
    borrow::Cow,
    num::NonZeroU64,
  };

  use ropey::Rope;
  use smallvec::smallvec;

  use super::*;
  use crate::{
    selection::{
      CursorRange,
      Range,
      Selection,
    },
    syntax::{
      FileType,
      LanguageSyntaxConfig,
      QueryKind,
      SyntaxConfiguration,
      SyntaxResources,
      tree_sitter::Grammar,
    },
  };

  #[derive(Debug, Clone, Copy)]
  struct JsonResources {
    grammar: Grammar,
  }

  impl SyntaxResources for JsonResources {
    fn grammar(&self, grammar_name: &str) -> Option<Grammar> {
      (grammar_name == "json").then_some(self.grammar)
    }

    fn query(&self, _language_id: &str, _kind: QueryKind) -> Option<Cow<'_, str>> {
      None
    }
  }

  fn cursor(id: u64, anchor: usize, head: usize) -> CursorRange {
    CursorRange {
      id:    CursorId::new(NonZeroU64::new(id).unwrap()),
      range: Range::new(anchor, head),
    }
  }

  fn json_syntax(text: &str) -> (Rope, Syntax) {
    let grammar = Grammar::try_from(tree_sitter_json::LANGUAGE).unwrap();
    let registry = crate::syntax::LanguageRegistry::new(
      SyntaxConfiguration {
        language: vec![LanguageSyntaxConfig {
          language_id:          "json".into(),
          scope:                "source.json".into(),
          file_types:           vec![FileType::Extension("json".into())],
          shebangs:             Vec::new(),
          comment_tokens:       None,
          block_comment_tokens: None,
          text_width:           None,
          soft_wrap:            None,
          grammar:              Some("json".into()),
          injection_regex:      None,
          indent:               None,
          auto_pairs:           None,
          rulers:               None,
          rainbow_brackets:     None,
        }],
      },
      JsonResources { grammar },
    )
    .unwrap();

    let rope = Rope::from(text);
    let syntax = Syntax::new(rope.slice(..), crate::syntax::LanguageId::new(0), &registry).unwrap();
    (rope, syntax)
  }

  #[test]
  fn expand_selection_expands_an_exact_node_to_its_parent_and_preserves_direction() {
    let (text, syntax) = json_syntax("[1,22,333]");
    let selection = Selection::from_cursors(smallvec![cursor(7, 5, 3)]).unwrap();

    let expanded = expand_selection(&syntax, text.slice(..), selection);
    let cursor = expanded.get(0).unwrap();

    assert_eq!(*cursor.range, Range::new(10, 0));
    assert_eq!(cursor.id.get(), 7);
  }

  #[test]
  fn sibling_navigation_moves_to_adjacent_named_nodes_and_sets_move_direction() {
    let (text, syntax) = json_syntax("[1,22,333]");

    let next = select_next_sibling(
      &syntax,
      text.slice(..),
      Selection::from_cursors(smallvec![cursor(1, 2, 1)]).unwrap(),
    );
    assert_eq!(*next.get(0).unwrap().range, Range::new(3, 5));

    let prev = select_prev_sibling(
      &syntax,
      text.slice(..),
      Selection::from_cursors(smallvec![cursor(2, 6, 9)]).unwrap(),
    );
    assert_eq!(*prev.get(0).unwrap().range, Range::new(5, 3));
  }

  #[test]
  fn select_all_children_splits_one_cursor_preserving_the_first_id() {
    let (text, syntax) = json_syntax("[1,22,333]");
    let selection = Selection::from_cursors(smallvec![cursor(9, 0, 10)]).unwrap();

    let children = select_all_children(&syntax, text.slice(..), selection);
    let cursors = children.iter().collect::<Vec<_>>();

    assert_eq!(cursors.len(), 3);
    assert_eq!(*cursors[0].range, Range::new(1, 2));
    assert_eq!(*cursors[1].range, Range::new(3, 5));
    assert_eq!(*cursors[2].range, Range::new(6, 9));
    assert_eq!(cursors[0].id.get(), 9);
    assert_ne!(cursors[1].id, cursors[0].id);
    assert_ne!(cursors[2].id, cursors[0].id);
    assert_ne!(cursors[1].id, cursors[2].id);
  }

  #[test]
  fn select_all_siblings_climbs_past_single_child_parents() {
    let (text, syntax) = json_syntax("[[1],[2]]");
    let selection = Selection::from_cursors(smallvec![cursor(5, 2, 3)]).unwrap();

    let siblings = select_all_siblings(&syntax, text.slice(..), selection);
    let cursors = siblings.iter().collect::<Vec<_>>();

    assert_eq!(cursors.len(), 2);
    assert_eq!(*cursors[0].range, Range::new(1, 4));
    assert_eq!(*cursors[1].range, Range::new(5, 8));
    assert_eq!(cursors[0].id.get(), 5);
    assert_ne!(cursors[1].id, cursors[0].id);
  }

  #[test]
  fn select_all_children_is_a_no_op_for_leaf_nodes() {
    let (text, syntax) = json_syntax("[1,22,333]");
    let selection = Selection::from_cursors(smallvec![cursor(3, 3, 5)]).unwrap();

    let result = select_all_children(&syntax, text.slice(..), selection.clone());

    assert_eq!(result.ranges(), selection.ranges());
    assert_eq!(result.cursor_ids(), selection.cursor_ids());
  }
}
