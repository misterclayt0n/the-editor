use std::collections::{BTreeMap, BTreeSet};

use crate::selection::{CursorId, Selection};

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Direction {
  Forward,
  Backward,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum MotionMode {
  /// Extend the selection.
  Extend,

  /// Move the selection, collapsing to the moved cursor.
  Move,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VisualGoal {
  pub col: u32,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct VisualGoals {
  goals: BTreeMap<CursorId, VisualGoal>,
}

impl VisualGoals {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn is_empty(&self) -> bool {
    self.goals.is_empty()
  }

  pub fn get(&self, cursor_id: CursorId) -> Option<VisualGoal> {
    self.goals.get(&cursor_id).copied()
  }

  pub fn insert(&mut self, cursor_id: CursorId, goal: VisualGoal) {
    self.goals.insert(cursor_id, goal);
  }

  pub fn remove(&mut self, cursor_id: CursorId) {
    self.goals.remove(&cursor_id);
  }

  pub fn iter(&self) -> impl Iterator<Item = (CursorId, VisualGoal)> + '_ {
    self
      .goals
      .iter()
      .map(|(cursor_id, goal)| (*cursor_id, *goal))
  }

  pub fn retain_ids(&mut self, ids: impl IntoIterator<Item = CursorId>) {
    let ids = ids.into_iter().collect::<BTreeSet<_>>();
    self.goals.retain(|cursor_id, _| ids.contains(cursor_id));
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MotionResult {
  pub selection:    Selection,
  pub visual_goals: VisualGoals,
}
