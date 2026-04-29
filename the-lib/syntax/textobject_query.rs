use std::{
  iter,
  ops,
};

use ropey::RopeSlice;

use super::{
  TREE_SITTER_MATCH_LIMIT,
  tree_sitter::{
    Node,
    Query,
    RopeInput,
  },
};

#[derive(Debug)]
pub enum CapturedNode<'a> {
  Single(Node<'a>),
  Grouped(Vec<Node<'a>>),
}

impl CapturedNode<'_> {
  pub fn start_byte(&self) -> usize {
    match self {
      Self::Single(node) => node.start_byte() as usize,
      Self::Grouped(nodes) => nodes[0].start_byte() as usize,
    }
  }

  pub fn end_byte(&self) -> usize {
    match self {
      Self::Single(node) => node.end_byte() as usize,
      Self::Grouped(nodes) => nodes.last().unwrap().end_byte() as usize,
    }
  }

  pub fn byte_range(&self) -> ops::Range<usize> {
    self.start_byte()..self.end_byte()
  }
}

#[derive(Debug)]
pub struct TextObjectQuery {
  pub(crate) query: Query,
}

impl TextObjectQuery {
  pub fn new(query: Query) -> Self {
    Self { query }
  }

  pub fn capture_nodes<'a>(
    &'a self,
    capture_name: &'a str,
    node: &Node<'a>,
    slice: RopeSlice<'a>,
  ) -> Option<impl Iterator<Item = CapturedNode<'a>> + 'a> {
    self.capture_nodes_any([capture_name], node, slice)
  }

  pub fn capture_nodes_any<'a, I>(
    &'a self,
    capture_names: I,
    node: &Node<'a>,
    slice: RopeSlice<'a>,
  ) -> Option<impl Iterator<Item = CapturedNode<'a>> + 'a>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let capture = capture_names
      .into_iter()
      .find_map(|capture_name| self.query.get_capture(capture_name))?;

    let mut cursor =
      super::tree_sitter::InactiveQueryCursor::new(0..u32::MAX, TREE_SITTER_MATCH_LIMIT)
        .execute_query(&self.query, node, RopeInput::new(slice));

    let capture_node = iter::from_fn(move || {
      let (mat, _) = cursor.next_matched_node()?;
      Some(mat.nodes_for_capture(capture).cloned().collect::<Vec<_>>())
    })
    .filter_map(|nodes| {
      if nodes.len() > 1 {
        Some(CapturedNode::Grouped(nodes))
      } else {
        nodes.into_iter().map(CapturedNode::Single).next()
      }
    });

    Some(capture_node)
  }
}
