//! Pure split-tree state for multi-pane editor layouts.
//!
//! This module intentionally contains only deterministic data/model logic.
//! Rendering, input routing, and client UI concerns stay outside `the-lib`.

use std::{
  collections::{
    BTreeMap,
    BTreeSet,
  },
  num::NonZeroUsize,
};

use crate::render::graphics::Rect;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PaneId(NonZeroUsize);

impl PaneId {
  pub const fn new(id: NonZeroUsize) -> Self {
    Self(id)
  }

  pub const fn get(self) -> NonZeroUsize {
    self.0
  }
}

impl From<NonZeroUsize> for PaneId {
  fn from(value: NonZeroUsize) -> Self {
    Self::new(value)
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SplitNodeId(NonZeroUsize);

impl SplitNodeId {
  pub const fn new(id: NonZeroUsize) -> Self {
    Self(id)
  }

  pub const fn get(self) -> NonZeroUsize {
    self.0
  }
}

impl From<NonZeroUsize> for SplitNodeId {
  fn from(value: NonZeroUsize) -> Self {
    Self::new(value)
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitAxis {
  Horizontal,
  Vertical,
}

impl SplitAxis {
  pub const fn transpose(self) -> Self {
    match self {
      Self::Horizontal => Self::Vertical,
      Self::Vertical => Self::Horizontal,
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaneDirection {
  Up,
  Down,
  Left,
  Right,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SplitNode {
  Leaf {
    pane: PaneId,
  },
  Branch {
    axis:   SplitAxis,
    ratio:  f32,
    first:  SplitNodeId,
    second: SplitNodeId,
  },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitTreeError {
  UnknownPane(PaneId),
  LastPane,
  Corrupt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvariantError {
  EmptyTree,
  MissingRoot,
  RootHasParent,
  ParentMismatch,
  MissingNode,
  UnknownChild,
  DuplicateVisit,
  UnreachableNode,
  PaneMismatch,
  MissingActivePane,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct NodeState {
  parent: Option<SplitNodeId>,
  node:   SplitNode,
}

#[derive(Debug, Clone)]
pub struct SplitTree {
  root:         SplitNodeId,
  active:       PaneId,
  nodes:        BTreeMap<SplitNodeId, NodeState>,
  pane_nodes:   BTreeMap<PaneId, SplitNodeId>,
  next_node_id: NonZeroUsize,
  next_pane_id: NonZeroUsize,
}

impl Default for SplitTree {
  fn default() -> Self {
    Self::new()
  }
}

impl SplitTree {
  pub fn new() -> Self {
    let root = SplitNodeId::new(NonZeroUsize::new(1).expect("nonzero"));
    let active = PaneId::new(NonZeroUsize::new(1).expect("nonzero"));

    let mut nodes = BTreeMap::new();
    nodes.insert(root, NodeState {
      parent: None,
      node:   SplitNode::Leaf { pane: active },
    });

    let mut pane_nodes = BTreeMap::new();
    pane_nodes.insert(active, root);

    Self {
      root,
      active,
      nodes,
      pane_nodes,
      next_node_id: NonZeroUsize::new(2).expect("nonzero"),
      next_pane_id: NonZeroUsize::new(2).expect("nonzero"),
    }
  }

  pub fn root(&self) -> SplitNodeId {
    self.root
  }

  pub fn active_pane(&self) -> PaneId {
    self.active
  }

  pub fn pane_count(&self) -> usize {
    self.pane_nodes.len()
  }

  pub fn node_count(&self) -> usize {
    self.nodes.len()
  }

  pub fn contains_pane(&self, pane: PaneId) -> bool {
    self.pane_nodes.contains_key(&pane)
  }

  pub fn set_active_pane(&mut self, pane: PaneId) -> bool {
    if !self.contains_pane(pane) {
      return false;
    }
    self.active = pane;
    true
  }

  pub fn node(&self, id: SplitNodeId) -> Option<SplitNode> {
    self.nodes.get(&id).map(|state| state.node)
  }

  pub fn pane_order(&self) -> Vec<PaneId> {
    self
      .leaf_order()
      .into_iter()
      .filter_map(|id| self.leaf_pane(id))
      .collect()
  }

  pub fn split_active(&mut self, axis: SplitAxis) -> PaneId {
    self
      .split_pane(self.active, axis)
      .expect("active pane is always valid")
  }

  pub fn split_pane(&mut self, pane: PaneId, axis: SplitAxis) -> Result<PaneId, SplitTreeError> {
    let leaf_id = *self
      .pane_nodes
      .get(&pane)
      .ok_or(SplitTreeError::UnknownPane(pane))?;

    let parent = self.node_parent(leaf_id);
    let first_leaf = self.alloc_node_id();
    let second_leaf = self.alloc_node_id();
    let new_pane = self.alloc_pane_id();

    self.nodes.insert(first_leaf, NodeState {
      parent: Some(leaf_id),
      node:   SplitNode::Leaf { pane },
    });
    self.nodes.insert(second_leaf, NodeState {
      parent: Some(leaf_id),
      node:   SplitNode::Leaf { pane: new_pane },
    });

    let ratio = 0.5;
    self.nodes.insert(leaf_id, NodeState {
      parent,
      node: SplitNode::Branch {
        axis,
        ratio,
        first: first_leaf,
        second: second_leaf,
      },
    });

    self.pane_nodes.insert(pane, first_leaf);
    self.pane_nodes.insert(new_pane, second_leaf);
    self.active = new_pane;

    debug_assert!(self.validate().is_ok());
    Ok(new_pane)
  }

  pub fn close_active(&mut self) -> Result<PaneId, SplitTreeError> {
    if self.pane_count() <= 1 {
      return Err(SplitTreeError::LastPane);
    }

    let closing_pane = self.active;
    let closing_leaf = *self
      .pane_nodes
      .get(&closing_pane)
      .ok_or(SplitTreeError::UnknownPane(closing_pane))?;
    let parent = self
      .node_parent(closing_leaf)
      .ok_or(SplitTreeError::Corrupt)?;

    let (first, second) = match self.node(parent).ok_or(SplitTreeError::Corrupt)? {
      SplitNode::Branch { first, second, .. } => (first, second),
      SplitNode::Leaf { .. } => return Err(SplitTreeError::Corrupt),
    };
    let sibling = if first == closing_leaf { second } else { first };

    let grand_parent = self.node_parent(parent);

    self.nodes.remove(&closing_leaf);
    self.pane_nodes.remove(&closing_pane);
    self.nodes.remove(&parent);

    if let Some(gp) = grand_parent {
      let gp_state = self.nodes.get_mut(&gp).ok_or(SplitTreeError::Corrupt)?;
      match gp_state.node {
        SplitNode::Branch {
          first: ref mut gp_first,
          second: ref mut gp_second,
          ..
        } => {
          if *gp_first == parent {
            *gp_first = sibling;
          } else if *gp_second == parent {
            *gp_second = sibling;
          } else {
            return Err(SplitTreeError::Corrupt);
          }
        },
        SplitNode::Leaf { .. } => return Err(SplitTreeError::Corrupt),
      }
      self.set_parent(sibling, Some(gp))?;
    } else {
      self.root = sibling;
      self.set_parent(sibling, None)?;
    }

    let next_active = self
      .first_leaf_pane(sibling)
      .ok_or(SplitTreeError::Corrupt)?;
    self.active = next_active;

    debug_assert!(self.validate().is_ok());
    Ok(next_active)
  }

  pub fn only_active(&mut self) {
    let active = self.active;
    let root = self.alloc_node_id();
    self.nodes.clear();
    self.nodes.insert(root, NodeState {
      parent: None,
      node:   SplitNode::Leaf { pane: active },
    });
    self.pane_nodes.clear();
    self.pane_nodes.insert(active, root);
    self.root = root;

    debug_assert!(self.validate().is_ok());
  }

  pub fn rotate_focus(&mut self, next: bool) -> bool {
    let panes = self.pane_order();
    if panes.len() <= 1 {
      return false;
    }
    let Some(current) = panes.iter().position(|pane| *pane == self.active) else {
      return false;
    };
    let next_index = if next {
      (current + 1) % panes.len()
    } else {
      (current + panes.len() - 1) % panes.len()
    };
    self.active = panes[next_index];
    true
  }

  pub fn jump_active(&mut self, direction: PaneDirection) -> bool {
    let Some(target) = self.find_pane_in_direction(self.active, direction) else {
      return false;
    };
    if target == self.active {
      return false;
    }
    self.active = target;
    true
  }

  pub fn swap_active(&mut self, direction: PaneDirection) -> bool {
    let active = self.active;
    let Some(target) = self.find_pane_in_direction(active, direction) else {
      return false;
    };
    if target == active {
      return false;
    }

    let Some(active_leaf) = self.pane_nodes.get(&active).copied() else {
      return false;
    };
    let Some(target_leaf) = self.pane_nodes.get(&target).copied() else {
      return false;
    };

    let Some(active_pane) = self.leaf_pane(active_leaf) else {
      return false;
    };
    let Some(target_pane) = self.leaf_pane(target_leaf) else {
      return false;
    };

    if let Some(state) = self.nodes.get_mut(&active_leaf) {
      state.node = SplitNode::Leaf { pane: target_pane };
    } else {
      return false;
    }
    if let Some(state) = self.nodes.get_mut(&target_leaf) {
      state.node = SplitNode::Leaf { pane: active_pane };
    } else {
      return false;
    }

    self.pane_nodes.insert(active_pane, target_leaf);
    self.pane_nodes.insert(target_pane, active_leaf);
    debug_assert!(self.validate().is_ok());
    true
  }

  pub fn transpose_active_branch(&mut self) -> bool {
    let Some(leaf_id) = self.pane_nodes.get(&self.active).copied() else {
      return false;
    };
    let Some(parent) = self.node_parent(leaf_id) else {
      return false;
    };
    let Some(state) = self.nodes.get_mut(&parent) else {
      return false;
    };
    let SplitNode::Branch {
      axis: ref mut branch_axis,
      ..
    } = state.node
    else {
      return false;
    };

    *branch_axis = branch_axis.transpose();
    true
  }

  pub fn active_parent_axis(&self) -> Option<SplitAxis> {
    let leaf_id = self.pane_nodes.get(&self.active).copied()?;
    let parent = self.node_parent(leaf_id)?;
    match self.node(parent)? {
      SplitNode::Branch { axis, .. } => Some(axis),
      SplitNode::Leaf { .. } => None,
    }
  }

  pub fn layout(&self, area: Rect) -> Vec<(PaneId, Rect)> {
    let mut panes = Vec::with_capacity(self.pane_count());
    let mut stack = vec![(self.root, area)];
    while let Some((node_id, rect)) = stack.pop() {
      let Some(node) = self.node(node_id) else {
        continue;
      };
      match node {
        SplitNode::Leaf { pane } => panes.push((pane, rect)),
        SplitNode::Branch {
          axis,
          ratio,
          first,
          second,
        } => {
          let (first_rect, second_rect) = split_rect(rect, axis, ratio);
          // Preserve leaf order by traversing first before second.
          stack.push((second, second_rect));
          stack.push((first, first_rect));
        },
      }
    }
    panes
  }

  pub fn validate(&self) -> Result<(), InvariantError> {
    if self.nodes.is_empty() {
      return Err(InvariantError::EmptyTree);
    }
    if !self.nodes.contains_key(&self.root) {
      return Err(InvariantError::MissingRoot);
    }
    if self.node_parent(self.root).is_some() {
      return Err(InvariantError::RootHasParent);
    }

    let mut visited = BTreeSet::new();
    let mut seen_panes = BTreeMap::new();
    let mut stack = vec![(self.root, None)];

    while let Some((id, expected_parent)) = stack.pop() {
      if !visited.insert(id) {
        return Err(InvariantError::DuplicateVisit);
      }
      let Some(state) = self.nodes.get(&id).copied() else {
        return Err(InvariantError::MissingNode);
      };
      if state.parent != expected_parent {
        return Err(InvariantError::ParentMismatch);
      }

      match state.node {
        SplitNode::Leaf { pane } => {
          seen_panes.insert(pane, id);
        },
        SplitNode::Branch { first, second, .. } => {
          if !self.nodes.contains_key(&first) || !self.nodes.contains_key(&second) {
            return Err(InvariantError::UnknownChild);
          }
          stack.push((first, Some(id)));
          stack.push((second, Some(id)));
        },
      }
    }

    if visited.len() != self.nodes.len() {
      return Err(InvariantError::UnreachableNode);
    }
    if seen_panes.len() != self.pane_nodes.len() {
      return Err(InvariantError::PaneMismatch);
    }
    for (pane, node) in &seen_panes {
      if self.pane_nodes.get(pane) != Some(node) {
        return Err(InvariantError::PaneMismatch);
      }
    }
    if !self.pane_nodes.contains_key(&self.active) {
      return Err(InvariantError::MissingActivePane);
    }

    Ok(())
  }

  fn alloc_node_id(&mut self) -> SplitNodeId {
    let id = self.next_node_id;
    let next = self.next_node_id.get().saturating_add(1);
    self.next_node_id = NonZeroUsize::new(next).unwrap_or(self.next_node_id);
    SplitNodeId::new(id)
  }

  fn alloc_pane_id(&mut self) -> PaneId {
    let id = self.next_pane_id;
    let next = self.next_pane_id.get().saturating_add(1);
    self.next_pane_id = NonZeroUsize::new(next).unwrap_or(self.next_pane_id);
    PaneId::new(id)
  }

  fn leaf_order(&self) -> Vec<SplitNodeId> {
    let mut order = Vec::with_capacity(self.pane_count());
    let mut stack = vec![self.root];
    while let Some(id) = stack.pop() {
      let Some(state) = self.nodes.get(&id) else {
        continue;
      };
      match state.node {
        SplitNode::Leaf { .. } => order.push(id),
        SplitNode::Branch { first, second, .. } => {
          stack.push(second);
          stack.push(first);
        },
      }
    }
    order
  }

  fn leaf_pane(&self, leaf: SplitNodeId) -> Option<PaneId> {
    let state = self.nodes.get(&leaf)?;
    match state.node {
      SplitNode::Leaf { pane } => Some(pane),
      SplitNode::Branch { .. } => None,
    }
  }

  fn first_leaf_pane(&self, root: SplitNodeId) -> Option<PaneId> {
    let mut current = root;
    loop {
      let state = self.nodes.get(&current)?;
      match state.node {
        SplitNode::Leaf { pane } => return Some(pane),
        SplitNode::Branch { first, .. } => current = first,
      }
    }
  }

  fn node_parent(&self, id: SplitNodeId) -> Option<SplitNodeId> {
    self.nodes.get(&id).and_then(|state| state.parent)
  }

  fn set_parent(
    &mut self,
    child: SplitNodeId,
    parent: Option<SplitNodeId>,
  ) -> Result<(), SplitTreeError> {
    let state = self.nodes.get_mut(&child).ok_or(SplitTreeError::Corrupt)?;
    state.parent = parent;
    Ok(())
  }

  fn find_pane_in_direction(&self, pane: PaneId, direction: PaneDirection) -> Option<PaneId> {
    let start_leaf = self.pane_nodes.get(&pane).copied()?;
    let origins = self.node_origins();
    let (current_x, current_y) = origins.get(&start_leaf).copied()?;
    let target_leaf = self.find_leaf_in_direction(start_leaf, direction, current_x, current_y, &origins)?;
    self.leaf_pane(target_leaf)
  }

  fn find_leaf_in_direction(
    &self,
    id: SplitNodeId,
    direction: PaneDirection,
    current_x: f32,
    current_y: f32,
    origins: &BTreeMap<SplitNodeId, (f32, f32)>,
  ) -> Option<SplitNodeId> {
    let parent = self.node_parent(id)?;
    let parent_axis = match self.node(parent)? {
      SplitNode::Branch { axis, .. } => axis,
      SplitNode::Leaf { .. } => return None,
    };

    if !Self::direction_possible_in_axis(direction, parent_axis) {
      return self.find_leaf_in_direction(parent, direction, current_x, current_y, origins);
    }

    let child = self.find_adjacent_child(parent, id, direction);
    let Some(child) = child else {
      return self.find_leaf_in_direction(parent, direction, current_x, current_y, origins);
    };

    self.descend_nearest_leaf(child, current_x, current_y, origins)
  }

  fn direction_possible_in_axis(direction: PaneDirection, axis: SplitAxis) -> bool {
    match axis {
      SplitAxis::Horizontal => matches!(direction, PaneDirection::Up | PaneDirection::Down),
      SplitAxis::Vertical => matches!(direction, PaneDirection::Left | PaneDirection::Right),
    }
  }

  fn find_adjacent_child(
    &self,
    parent: SplitNodeId,
    child: SplitNodeId,
    direction: PaneDirection,
  ) -> Option<SplitNodeId> {
    let (first, second) = match self.node(parent)? {
      SplitNode::Branch { first, second, .. } => (first, second),
      SplitNode::Leaf { .. } => return None,
    };

    match direction {
      PaneDirection::Up | PaneDirection::Left => {
        if second == child {
          Some(first)
        } else {
          None
        }
      },
      PaneDirection::Down | PaneDirection::Right => {
        if first == child {
          Some(second)
        } else {
          None
        }
      },
    }
  }

  fn descend_nearest_leaf(
    &self,
    start: SplitNodeId,
    current_x: f32,
    current_y: f32,
    origins: &BTreeMap<SplitNodeId, (f32, f32)>,
  ) -> Option<SplitNodeId> {
    let mut node = start;
    loop {
      match self.node(node)? {
        SplitNode::Leaf { .. } => return Some(node),
        SplitNode::Branch {
          axis,
          first,
          second,
          ..
        } => {
          let (first_x, first_y) = origins.get(&first).copied()?;
          let (second_x, second_y) = origins.get(&second).copied()?;
          let first_delta = match axis {
            SplitAxis::Vertical => (current_x - first_x).abs(),
            SplitAxis::Horizontal => (current_y - first_y).abs(),
          };
          let second_delta = match axis {
            SplitAxis::Vertical => (current_x - second_x).abs(),
            SplitAxis::Horizontal => (current_y - second_y).abs(),
          };
          node = if first_delta <= second_delta {
            first
          } else {
            second
          };
        },
      }
    }
  }

  fn node_origins(&self) -> BTreeMap<SplitNodeId, (f32, f32)> {
    let mut origins = BTreeMap::new();
    let mut stack = vec![(self.root, 0.0f32, 0.0f32, 1.0f32, 1.0f32)];
    while let Some((id, x, y, width, height)) = stack.pop() {
      origins.insert(id, (x, y));
      let Some(node) = self.node(id) else {
        continue;
      };
      let SplitNode::Branch {
        axis,
        ratio,
        first,
        second,
      } = node
      else {
        continue;
      };

      let ratio = ratio.clamp(0.0, 1.0);
      match axis {
        SplitAxis::Vertical => {
          let first_width = width * ratio;
          stack.push((second, x + first_width, y, width - first_width, height));
          stack.push((first, x, y, first_width, height));
        },
        SplitAxis::Horizontal => {
          let first_height = height * ratio;
          stack.push((second, x, y + first_height, width, height - first_height));
          stack.push((first, x, y, width, first_height));
        },
      }
    }
    origins
  }
}

fn split_rect(rect: Rect, axis: SplitAxis, ratio: f32) -> (Rect, Rect) {
  let ratio = ratio.clamp(0.0, 1.0);
  match axis {
    SplitAxis::Vertical => {
      let total = rect.width;
      if total <= 1 {
        let first = Rect::new(rect.x, rect.y, total, rect.height);
        let second = Rect::new(rect.right(), rect.y, 0, rect.height);
        return (first, second);
      }
      let mut first_width = ((total as f32) * ratio).round() as u16;
      first_width = first_width.clamp(1, total - 1);
      let second_width = total - first_width;
      let first = Rect::new(rect.x, rect.y, first_width, rect.height);
      let second = Rect::new(rect.x + first_width, rect.y, second_width, rect.height);
      (first, second)
    },
    SplitAxis::Horizontal => {
      let total = rect.height;
      if total <= 1 {
        let first = Rect::new(rect.x, rect.y, rect.width, total);
        let second = Rect::new(rect.x, rect.bottom(), rect.width, 0);
        return (first, second);
      }
      let mut first_height = ((total as f32) * ratio).round() as u16;
      first_height = first_height.clamp(1, total - 1);
      let second_height = total - first_height;
      let first = Rect::new(rect.x, rect.y, rect.width, first_height);
      let second = Rect::new(rect.x, rect.y + first_height, rect.width, second_height);
      (first, second)
    },
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::render::graphics::Rect;

  #[test]
  fn new_tree_has_single_leaf_and_valid_invariants() {
    let tree = SplitTree::new();
    assert_eq!(tree.pane_count(), 1);
    assert_eq!(tree.node_count(), 1);
    assert_eq!(tree.pane_order(), vec![tree.active_pane()]);
    assert_eq!(tree.validate(), Ok(()));
  }

  #[test]
  fn split_active_adds_pane_and_focuses_new_leaf() {
    let mut tree = SplitTree::new();
    let original = tree.active_pane();
    let new_pane = tree.split_active(SplitAxis::Vertical);

    assert_ne!(new_pane, original);
    assert_eq!(tree.active_pane(), new_pane);
    assert_eq!(tree.pane_count(), 2);
    assert_eq!(tree.pane_order(), vec![original, new_pane]);
    assert_eq!(tree.validate(), Ok(()));
  }

  #[test]
  fn close_active_collapses_branch_and_keeps_valid_state() {
    let mut tree = SplitTree::new();
    let first = tree.active_pane();
    let second = tree.split_active(SplitAxis::Vertical);
    let third = tree.split_active(SplitAxis::Horizontal);

    assert_eq!(tree.pane_order(), vec![first, second, third]);
    assert_eq!(tree.active_pane(), third);

    let active = tree.close_active().expect("close active pane");
    assert_eq!(active, second);
    assert_eq!(tree.pane_count(), 2);
    assert_eq!(tree.pane_order(), vec![first, second]);
    assert_eq!(tree.validate(), Ok(()));
  }

  #[test]
  fn close_last_pane_is_rejected() {
    let mut tree = SplitTree::new();
    let err = tree.close_active().expect_err("cannot close last pane");
    assert_eq!(err, SplitTreeError::LastPane);
    assert_eq!(tree.validate(), Ok(()));
  }

  #[test]
  fn only_active_reduces_tree_to_single_leaf() {
    let mut tree = SplitTree::new();
    let first = tree.active_pane();
    let _second = tree.split_active(SplitAxis::Vertical);
    let third = tree.split_active(SplitAxis::Horizontal);
    assert_eq!(tree.active_pane(), third);

    tree.only_active();

    assert_eq!(tree.pane_count(), 1);
    assert_eq!(tree.node_count(), 1);
    assert_eq!(tree.pane_order(), vec![third]);
    assert!(!tree.contains_pane(first));
    assert_eq!(tree.validate(), Ok(()));
  }

  #[test]
  fn rotate_focus_moves_between_panes_in_leaf_order() {
    let mut tree = SplitTree::new();
    let first = tree.active_pane();
    let second = tree.split_active(SplitAxis::Vertical);
    let third = tree.split_active(SplitAxis::Horizontal);

    assert_eq!(tree.pane_order(), vec![first, second, third]);
    assert_eq!(tree.active_pane(), third);

    assert!(tree.rotate_focus(true));
    assert_eq!(tree.active_pane(), first);
    assert!(tree.rotate_focus(false));
    assert_eq!(tree.active_pane(), third);
    assert_eq!(tree.validate(), Ok(()));
  }

  #[test]
  fn transpose_active_branch_toggles_parent_axis() {
    let mut tree = SplitTree::new();
    let _ = tree.split_active(SplitAxis::Vertical);

    assert_eq!(tree.active_parent_axis(), Some(SplitAxis::Vertical));
    assert!(tree.transpose_active_branch());
    assert_eq!(tree.active_parent_axis(), Some(SplitAxis::Horizontal));
    assert!(tree.transpose_active_branch());
    assert_eq!(tree.active_parent_axis(), Some(SplitAxis::Vertical));
    assert_eq!(tree.validate(), Ok(()));
  }

  #[test]
  fn set_active_pane_rejects_unknown_ids() {
    let mut tree = SplitTree::new();
    let unknown = PaneId::new(NonZeroUsize::new(999).expect("nonzero"));
    assert!(!tree.set_active_pane(unknown));
    assert_eq!(tree.validate(), Ok(()));
  }

  #[test]
  fn jump_active_moves_to_pane_in_direction() {
    let mut tree = SplitTree::new();
    let left = tree.active_pane();
    let right_top = tree.split_active(SplitAxis::Vertical);
    let right_bottom = tree.split_active(SplitAxis::Horizontal);

    assert_eq!(tree.active_pane(), right_bottom);
    assert!(tree.jump_active(PaneDirection::Up));
    assert_eq!(tree.active_pane(), right_top);

    assert!(tree.jump_active(PaneDirection::Left));
    assert_eq!(tree.active_pane(), left);

    assert!(tree.jump_active(PaneDirection::Right));
    assert_eq!(tree.active_pane(), right_top);

    assert!(!tree.jump_active(PaneDirection::Up));
    assert_eq!(tree.active_pane(), right_top);
    assert_eq!(tree.validate(), Ok(()));
  }

  #[test]
  fn swap_active_swaps_pane_positions() {
    let mut tree = SplitTree::new();
    let left = tree.active_pane();
    let right_top = tree.split_active(SplitAxis::Vertical);
    let right_bottom = tree.split_active(SplitAxis::Horizontal);

    assert_eq!(tree.pane_order(), vec![left, right_top, right_bottom]);
    assert_eq!(tree.active_pane(), right_bottom);

    assert!(tree.swap_active(PaneDirection::Up));
    assert_eq!(tree.active_pane(), right_bottom);
    assert_eq!(tree.pane_order(), vec![left, right_bottom, right_top]);

    assert!(tree.jump_active(PaneDirection::Down));
    assert_eq!(tree.active_pane(), right_top);
    assert_eq!(tree.validate(), Ok(()));
  }

  #[test]
  fn invariants_hold_after_mixed_operations() {
    let mut tree = SplitTree::new();
    let _ = tree.split_active(SplitAxis::Vertical);
    let _ = tree.split_active(SplitAxis::Vertical);
    let _ = tree.rotate_focus(true);
    let _ = tree.transpose_active_branch();
    let _ = tree.split_active(SplitAxis::Horizontal);
    let _ = tree.close_active().expect("close");
    let _ = tree.rotate_focus(false);
    tree.only_active();

    assert_eq!(tree.validate(), Ok(()));
  }

  #[test]
  fn layout_covers_root_area_without_overlap() {
    let mut tree = SplitTree::new();
    let first = tree.active_pane();
    let second = tree.split_active(SplitAxis::Vertical);
    let third = tree.split_active(SplitAxis::Horizontal);

    let area = Rect::new(0, 0, 120, 40);
    let panes = tree.layout(area);
    assert_eq!(panes.len(), 3);
    assert_eq!(
      panes.iter().map(|(pane, _)| *pane).collect::<Vec<_>>(),
      vec![first, second, third]
    );

    let total_area: usize = panes
      .iter()
      .map(|(_, rect)| rect.width as usize * rect.height as usize)
      .sum();
    assert_eq!(total_area, area.width as usize * area.height as usize);

    for i in 0..panes.len() {
      for j in (i + 1)..panes.len() {
        let a = panes[i].1;
        let b = panes[j].1;
        let overlap_x = a.x < b.right() && b.x < a.right();
        let overlap_y = a.y < b.bottom() && b.y < a.bottom();
        assert!(
          !(overlap_x && overlap_y),
          "pane rects overlap: {:?} and {:?}",
          a,
          b
        );
      }
    }
  }
}
