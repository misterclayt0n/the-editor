use std::{
  collections::HashSet,
  num::NonZeroUsize,
  time::{
    Duration,
    Instant,
  },
};

use ropey::Rope;
use thiserror::Error;

use crate::{
  selection::Selection,
  transaction::{
    Transaction,
    TransactionError,
  },
};

pub type Result<T> = std::result::Result<T, HistoryError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RevisionId(usize);

impl RevisionId {
  pub fn new(index: usize) -> Self {
    Self(index)
  }

  pub fn get(self) -> usize {
    self.0
  }
}

#[derive(Debug, Error)]
pub enum HistoryError {
  #[error(transparent)]
  Transaction(#[from] TransactionError),

  #[error("revision {revision:?} is out of bounds (max: {max:?})")]
  RevisionOutOfBounds {
    revision: RevisionId,
    max:      RevisionId,
  },

  #[error("stale history jump: expected source {expected:?}, got {got:?}")]
  StaleJump {
    expected: RevisionId,
    got:      RevisionId,
  },

  #[error("selection snapshot is out of bounds for document length {len}")]
  SelectionOutOfBounds { len: usize },

  #[error("commit input length mismatch: expected {expected}, got {got}")]
  CommitLengthMismatch { expected: usize, got: usize },
}

#[derive(Debug, Clone)]
pub struct HistoryJump {
  pub source:       RevisionId,
  pub target:       RevisionId,
  pub transactions: Vec<Transaction>,
  pub selection:    Selection,
}

impl HistoryJump {
  pub fn is_empty(&self) -> bool {
    self.transactions.is_empty()
  }

  pub fn len(&self) -> usize {
    self.transactions.len()
  }
}

#[derive(Debug)]
pub struct History {
  revisions: Vec<Revision>,
  current:   usize,
}

#[derive(Debug, Clone)]
struct Revision {
  parent:           usize,
  last_child:       Option<NonZeroUsize>,
  transaction:      Transaction,
  inversion:        Transaction,
  timestamp:        Instant,
  selection_before: Selection,
  selection_after:  Selection,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum UndoKind {
  Steps(usize),
  TimePeriod(Duration),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TimeDirection {
  Backward,
  Forward,
}

impl History {
  pub fn new(initial_len: usize, initial_selection: Selection) -> Result<Self> {
    Self::new_at_timestamp(initial_len, initial_selection, Instant::now())
  }

  pub fn new_at_timestamp(
    initial_len: usize,
    initial_selection: Selection,
    timestamp: Instant,
  ) -> Result<Self> {
    validate_selection_bounds(&initial_selection, initial_len)?;
    Ok(Self {
      revisions: vec![Revision {
        parent: 0,
        last_child: None,
        transaction: Transaction::empty(initial_len),
        inversion: Transaction::empty(initial_len),
        timestamp,
        selection_before: initial_selection.clone(),
        selection_after: initial_selection,
      }],
      current:   0,
    })
  }

  pub fn current_revision(&self) -> RevisionId {
    RevisionId(self.current)
  }

  pub fn at_root(&self) -> bool {
    self.current == 0
  }

  pub fn len(&self) -> usize {
    self.revisions.len()
  }

  pub fn is_empty(&self) -> bool {
    self.revisions.len() <= 1
  }

  pub fn current_selection(&self) -> &Selection {
    &self.revisions[self.current].selection_after
  }

  pub fn selection(&self, revision: RevisionId) -> Result<&Selection> {
    let idx = self.index(revision)?;
    Ok(&self.revisions[idx].selection_after)
  }

  pub fn current_len(&self) -> usize {
    self.revisions[self.current]
      .transaction
      .changes()
      .len_after()
  }

  pub fn revision_len(&self, revision: RevisionId) -> Result<usize> {
    let idx = self.index(revision)?;
    Ok(self.revisions[idx].transaction.changes().len_after())
  }

  pub fn current_timestamp(&self) -> Instant {
    self.revisions[self.current].timestamp
  }

  pub fn revision_timestamp(&self, revision: RevisionId) -> Result<Instant> {
    let idx = self.index(revision)?;
    Ok(self.revisions[idx].timestamp)
  }

  pub fn commit(
    &mut self,
    before: &Rope,
    transaction: Transaction,
    selection_before: Selection,
    selection_after: Selection,
  ) -> Result<RevisionId> {
    self.commit_at_timestamp(
      before,
      transaction,
      selection_before,
      selection_after,
      Instant::now(),
    )
  }

  pub fn commit_at_timestamp(
    &mut self,
    before: &Rope,
    transaction: Transaction,
    selection_before: Selection,
    selection_after: Selection,
    timestamp: Instant,
  ) -> Result<RevisionId> {
    let before_len = before.len_chars();
    let expected = self.current_len();
    if before_len != expected {
      return Err(HistoryError::CommitLengthMismatch {
        expected,
        got: before_len,
      });
    }
    if transaction.changes().len() != expected {
      return Err(HistoryError::CommitLengthMismatch {
        expected,
        got: transaction.changes().len(),
      });
    }
    validate_selection_bounds(&selection_before, transaction.changes().len())?;
    validate_selection_bounds(&selection_after, transaction.changes().len_after())?;

    let inversion = transaction.invert(before)?;
    let new_current = self.revisions.len();
    self.revisions[self.current].last_child = NonZeroUsize::new(new_current);
    self.revisions.push(Revision {
      parent: self.current,
      last_child: None,
      transaction,
      inversion,
      timestamp,
      selection_before,
      selection_after,
    });
    self.current = new_current;
    Ok(RevisionId(new_current))
  }

  pub fn changes_since(&self, revision: RevisionId) -> Result<Option<Transaction>> {
    let from = self.index(revision)?;
    self.composed_between(from, self.current)
  }

  pub fn undo(&self) -> Option<HistoryJump> {
    if self.at_root() {
      return None;
    }
    let current = &self.revisions[self.current];
    Some(HistoryJump {
      source: RevisionId(self.current),
      target: RevisionId(current.parent),
      transactions: vec![current.inversion.clone()],
      selection: current.selection_before.clone(),
    })
  }

  pub fn redo(&self) -> Option<HistoryJump> {
    let child = self.revisions[self.current].last_child?.get();
    Some(self.jump_from_parts(
      self.current,
      child,
      vec![self.revisions[child].transaction.clone()],
      self.revisions[child].selection_after.clone(),
    ))
  }

  pub fn jump_to(&self, revision: RevisionId) -> Result<HistoryJump> {
    let target = self.index(revision)?;
    Ok(self.jump_between(self.current, target))
  }

  pub fn jump_backward(&self, steps: usize) -> Result<HistoryJump> {
    Ok(self.jump_between(self.current, self.walk_parents(self.current, steps)))
  }

  pub fn jump_forward(&self, steps: usize) -> Result<HistoryJump> {
    Ok(self.jump_between(self.current, self.walk_children(self.current, steps)))
  }

  pub fn jump_duration_backward(&self, duration: Duration) -> Result<HistoryJump> {
    match self.revisions[self.current].timestamp.checked_sub(duration) {
      Some(instant) => self.jump_instant(instant, TimeDirection::Backward),
      None => self.jump_to(RevisionId(0)),
    }
  }

  pub fn jump_duration_forward(&self, duration: Duration) -> Result<HistoryJump> {
    match self.revisions[self.current].timestamp.checked_add(duration) {
      Some(instant) => self.jump_instant(instant, TimeDirection::Forward),
      None => self.jump_to(RevisionId(self.revisions.len() - 1)),
    }
  }

  pub fn earlier(&self, kind: UndoKind) -> Result<HistoryJump> {
    match kind {
      UndoKind::Steps(steps) => self.jump_backward(steps),
      UndoKind::TimePeriod(duration) => self.jump_duration_backward(duration),
    }
  }

  pub fn later(&self, kind: UndoKind) -> Result<HistoryJump> {
    match kind {
      UndoKind::Steps(steps) => self.jump_forward(steps),
      UndoKind::TimePeriod(duration) => self.jump_duration_forward(duration),
    }
  }

  pub fn apply_jump(&mut self, jump: &HistoryJump) -> Result<()> {
    let expected = self.current_revision();
    if jump.source != expected {
      return Err(HistoryError::StaleJump {
        expected,
        got: jump.source,
      });
    }
    self.index(jump.target)?;
    self.current = jump.target.get();
    Ok(())
  }

  fn index(&self, revision: RevisionId) -> Result<usize> {
    let idx = revision.get();
    if idx >= self.revisions.len() {
      return Err(HistoryError::RevisionOutOfBounds {
        revision,
        max: RevisionId(self.revisions.len().saturating_sub(1)),
      });
    }
    Ok(idx)
  }

  fn jump_from_parts(
    &self,
    source: usize,
    target: usize,
    transactions: Vec<Transaction>,
    selection: Selection,
  ) -> HistoryJump {
    HistoryJump {
      source: RevisionId(source),
      target: RevisionId(target),
      transactions,
      selection,
    }
  }

  fn jump_between(&self, source: usize, target: usize) -> HistoryJump {
    if source == target {
      return self.jump_from_parts(
        source,
        target,
        Vec::new(),
        self.revisions[target].selection_after.clone(),
      );
    }

    let lca = self.lowest_common_ancestor(source, target);
    let up = self.path_up(source, lca);
    let down = self.path_up(target, lca);

    let transactions = up
      .iter()
      .map(|&idx| self.revisions[idx].inversion.clone())
      .chain(
        down
          .iter()
          .rev()
          .map(|&idx| self.revisions[idx].transaction.clone()),
      )
      .collect();

    let selection = if down.is_empty() {
      self.revisions[*up.last().expect("backward jump must undo at least one revision")]
        .selection_before
        .clone()
    } else {
      self.revisions[target].selection_after.clone()
    };

    self.jump_from_parts(source, target, transactions, selection)
  }

  fn composed_between(&self, from: usize, to: usize) -> Result<Option<Transaction>> {
    if from == to {
      return Ok(None);
    }

    let jump = self.jump_between(from, to);
    let mut composed: Option<Transaction> = None;
    for transaction in jump.transactions {
      composed = Some(match composed {
        None => transaction,
        Some(acc) => acc.compose(transaction)?,
      });
    }
    Ok(composed)
  }

  fn lowest_common_ancestor(&self, mut a: usize, mut b: usize) -> usize {
    let mut a_path = HashSet::new();
    let mut b_path = HashSet::new();
    loop {
      a_path.insert(a);
      b_path.insert(b);
      if a_path.contains(&b) {
        return b;
      }
      if b_path.contains(&a) {
        return a;
      }
      a = self.revisions[a].parent;
      b = self.revisions[b].parent;
    }
  }

  fn path_up(&self, mut revision: usize, ancestor: usize) -> Vec<usize> {
    let mut path = Vec::new();
    while revision != ancestor {
      path.push(revision);
      revision = self.revisions[revision].parent;
    }
    path
  }

  fn walk_parents(&self, mut from: usize, steps: usize) -> usize {
    for _ in 0..steps {
      if from == 0 {
        break;
      }
      from = self.revisions[from].parent;
    }
    from
  }

  fn walk_children(&self, mut from: usize, steps: usize) -> usize {
    for _ in 0..steps {
      match self.revisions[from].last_child {
        Some(child) => from = child.get(),
        None => break,
      }
    }
    from
  }

  fn jump_instant(&self, instant: Instant, direction: TimeDirection) -> Result<HistoryJump> {
    let revision = self.find_revision_nearest_instant(instant, direction);
    self.jump_to(RevisionId(revision))
  }

  fn find_revision_nearest_instant(&self, instant: Instant, direction: TimeDirection) -> usize {
    self
      .revisions
      .iter()
      .enumerate()
      .min_by_key(|(_, revision)| {
        let ts = revision.timestamp;
        let is_after = ts >= instant;
        let diff = if is_after {
          ts.duration_since(instant)
        } else {
          instant.duration_since(ts)
        };

        let tie_break = match direction {
          TimeDirection::Backward => is_after,
          TimeDirection::Forward => !is_after,
        };

        (diff, tie_break)
      })
      .map(|(idx, _)| idx)
      .expect("history must contain at least one revision")
  }
}

fn validate_selection_bounds(selection: &Selection, len: usize) -> Result<()> {
  if selection
    .ranges()
    .iter()
    .any(|range| range.anchor > len || range.head > len)
  {
    return Err(HistoryError::SelectionOutOfBounds { len });
  }
  Ok(())
}

#[cfg(test)]
mod test {
  use super::*;
  use crate::transaction::Span;

  fn span(start: usize, end: usize) -> Span {
    Span { start, end }
  }

  fn apply_jump(doc: &mut Rope, history: &mut History, jump: HistoryJump) -> Selection {
    for transaction in &jump.transactions {
      transaction.apply(doc).unwrap();
    }
    let selection = jump.selection.clone();
    history.apply_jump(&jump).unwrap();
    selection
  }

  fn apply_and_commit(
    history: &mut History,
    doc: &mut Rope,
    transaction: Transaction,
    selection_after: Selection,
  ) -> RevisionId {
    let before = doc.clone();
    let selection_before = history.current_selection().clone();
    transaction.apply(doc).unwrap();
    history
      .commit(&before, transaction, selection_before, selection_after)
      .unwrap()
  }

  #[test]
  fn new_uses_explicit_root_state_and_validates_selection_bounds() {
    let selection = Selection::point(3);
    let history = History::new(5, selection.clone()).unwrap();

    assert_eq!(history.current_revision(), RevisionId::new(0));
    assert_eq!(history.current_len(), 5);
    assert_eq!(history.current_selection(), &selection);
    assert!(history.undo().is_none());

    assert!(matches!(
      History::new(5, Selection::point(99)),
      Err(HistoryError::SelectionOutOfBounds { len: 5 })
    ));
  }

  #[test]
  fn commit_returns_new_revision_and_accessors_expose_revision_state() {
    let t0 = Instant::now();
    let t1 = t0.checked_add(Duration::from_secs(10)).unwrap();

    let root_selection = Selection::point(0);
    let after_selection = Selection::single(5, 6);
    let mut history = History::new_at_timestamp(5, root_selection.clone(), t0).unwrap();
    let mut doc = Rope::from("hello");

    let before = doc.clone();
    let tx = Transaction::insert_at(doc.len_chars(), 5, "!").unwrap();
    tx.apply(&mut doc).unwrap();
    let rev1 = history
      .commit_at_timestamp(&before, tx, root_selection.clone(), after_selection.clone(), t1)
      .unwrap();

    assert_eq!(rev1, RevisionId::new(1));
    assert_eq!(history.current_revision(), rev1);
    assert_eq!(history.current_len(), 6);
    assert_eq!(history.current_selection(), &after_selection);
    assert_eq!(history.current_timestamp(), t1);
    assert_eq!(
      history.selection(RevisionId::new(0)).unwrap(),
      &root_selection
    );
    assert_eq!(history.selection(rev1).unwrap(), &after_selection);
    assert_eq!(history.revision_len(RevisionId::new(0)).unwrap(), 5);
    assert_eq!(history.revision_len(rev1).unwrap(), 6);
    assert_eq!(history.revision_timestamp(rev1).unwrap(), t1);
  }

  #[test]
  fn commit_validates_document_lengths_and_result_selection_bounds() {
    let mut history = History::new(5, Selection::point(0)).unwrap();
    let before = Rope::from("hello");

    assert!(matches!(
      history.commit(
        &Rope::from("hell"),
        Transaction::insert_at(5, 5, "!").unwrap(),
        Selection::point(0),
        Selection::point(6),
      ),
      Err(HistoryError::CommitLengthMismatch {
        expected: 5,
        got:      4,
      })
    ));

    assert!(matches!(
      history.commit(
        &before,
        Transaction::insert_at(4, 4, "!").unwrap(),
        Selection::point(0),
        Selection::point(5),
      ),
      Err(HistoryError::CommitLengthMismatch {
        expected: 5,
        got:      4,
      })
    ));

    assert!(matches!(
      history.commit(
        &before,
        Transaction::insert_at(5, 5, "!").unwrap(),
        Selection::point(0),
        Selection::point(99),
      ),
      Err(HistoryError::SelectionOutOfBounds { len: 6 })
    ));
  }

  #[test]
  fn undo_and_redo_restore_text_and_exact_selection_snapshots() {
    let mut doc = Rope::from("hello");
    let mut history = History::new(doc.len_chars(), Selection::point(0)).unwrap();

    let after_insert = Selection::point(11);
    let after_replace = Selection::single(6, 8);
    apply_and_commit(
      &mut history,
      &mut doc,
      Transaction::insert_at(5, 5, " world").unwrap(),
      after_insert.clone(),
    );
    apply_and_commit(
      &mut history,
      &mut doc,
      Transaction::replace(11, span(6, 11), "世界").unwrap(),
      after_replace.clone(),
    );
    assert_eq!(doc.to_string(), "hello 世界");

    let jump = history.undo().unwrap();
    let selection = apply_jump(&mut doc, &mut history, jump);
    assert_eq!(doc.to_string(), "hello world");
    assert_eq!(selection, after_insert);

    let jump = history.redo().unwrap();
    let selection = apply_jump(&mut doc, &mut history, jump);
    assert_eq!(doc.to_string(), "hello 世界");
    assert_eq!(selection, after_replace);
  }

  #[test]
  fn branching_navigation_and_redo_follow_last_child_lineage() {
    let mut doc = Rope::from("a");
    let mut history = History::new(doc.len_chars(), Selection::point(0)).unwrap();

    apply_and_commit(
      &mut history,
      &mut doc,
      Transaction::insert_at(1, 1, "b").unwrap(),
      Selection::point(2),
    );
    let r2 = apply_and_commit(
      &mut history,
      &mut doc,
      Transaction::insert_at(2, 2, "c").unwrap(),
      Selection::point(3),
    );
    let r3 = apply_and_commit(
      &mut history,
      &mut doc,
      Transaction::insert_at(3, 3, "d").unwrap(),
      Selection::point(4),
    );

    let jump = history.jump_to(r2).unwrap();
    apply_jump(&mut doc, &mut history, jump);
    let r4 = apply_and_commit(
      &mut history,
      &mut doc,
      Transaction::insert_at(3, 3, "e").unwrap(),
      Selection::point(4),
    );

    let jump = history.earlier(UndoKind::Steps(1)).unwrap();
    assert_eq!(jump.target, r2);
    apply_jump(&mut doc, &mut history, jump);
    assert_eq!(doc.to_string(), "abc");

    assert_eq!(history.redo().unwrap().target, r4);

    let jump = history.later(UndoKind::Steps(1)).unwrap();
    assert_eq!(jump.target, r4);
    apply_jump(&mut doc, &mut history, jump);
    assert_eq!(doc.to_string(), "abce");

    let jump = history.jump_to(r3).unwrap();
    apply_jump(&mut doc, &mut history, jump);
    let jump = history.later(UndoKind::Steps(1)).unwrap();
    assert_eq!(jump.target, r3);
    assert!(jump.is_empty());
  }

  #[test]
  fn changes_since_returns_the_document_delta_from_any_revision_to_current() {
    let mut doc = Rope::from("a");
    let mut history = History::new(doc.len_chars(), Selection::point(0)).unwrap();

    apply_and_commit(
      &mut history,
      &mut doc,
      Transaction::insert_at(1, 1, "b").unwrap(),
      Selection::point(2),
    );
    let r2 = apply_and_commit(
      &mut history,
      &mut doc,
      Transaction::insert_at(2, 2, "c").unwrap(),
      Selection::point(3),
    );

    let jump = history.jump_to(RevisionId::new(1)).unwrap();
    apply_jump(&mut doc, &mut history, jump);
    apply_and_commit(
      &mut history,
      &mut doc,
      Transaction::replace(2, span(1, 2), "d").unwrap(),
      Selection::point(2),
    );
    assert_eq!(doc.to_string(), "ad");

    let tx = history.changes_since(r2).unwrap().unwrap();
    let mut from_other_branch = Rope::from("abc");
    tx.apply(&mut from_other_branch).unwrap();
    assert_eq!(from_other_branch.to_string(), "ad");

    assert!(
      history
        .changes_since(history.current_revision())
        .unwrap()
        .is_none()
    );
  }

  #[test]
  fn apply_jump_rejects_stale_jumps_without_moving_history() {
    let mut doc = Rope::from("a");
    let mut history = History::new(doc.len_chars(), Selection::point(0)).unwrap();
    apply_and_commit(
      &mut history,
      &mut doc,
      Transaction::insert_at(1, 1, "b").unwrap(),
      Selection::point(2),
    );

    let undo = history.undo().unwrap();
    for tx in &undo.transactions {
      tx.apply(&mut doc).unwrap();
    }
    history.apply_jump(&undo).unwrap();
    assert_eq!(history.current_revision(), RevisionId::new(0));

    assert!(matches!(
      history.apply_jump(&undo),
      Err(HistoryError::StaleJump { .. })
    ));
    assert_eq!(history.current_revision(), RevisionId::new(0));
  }

  #[test]
  fn time_navigation_chooses_the_nearest_revision_and_breaks_ties_by_direction() {
    let t0 = Instant::now();
    let t = |secs| t0.checked_add(Duration::from_secs(secs)).unwrap();

    let mut doc = Rope::from("a");
    let mut history =
      History::new_at_timestamp(doc.len_chars(), Selection::point(0), t(0)).unwrap();

    let before = doc.clone();
    let tx = Transaction::insert_at(1, 1, "b").unwrap();
    tx.apply(&mut doc).unwrap();
    history
      .commit_at_timestamp(&before, tx, Selection::point(0), Selection::point(2), t(10))
      .unwrap();

    let before = doc.clone();
    let tx = Transaction::insert_at(2, 2, "c").unwrap();
    tx.apply(&mut doc).unwrap();
    history
      .commit_at_timestamp(&before, tx, Selection::point(2), Selection::point(3), t(20))
      .unwrap();

    let jump = history.jump_to(RevisionId::new(1)).unwrap();
    apply_jump(&mut doc, &mut history, jump);

    let jump = history
      .jump_duration_forward(Duration::from_secs(5))
      .unwrap();
    assert_eq!(jump.target, RevisionId::new(2));
    apply_jump(&mut doc, &mut history, jump);

    let jump = history
      .jump_duration_backward(Duration::from_secs(5))
      .unwrap();
    assert_eq!(jump.target, RevisionId::new(1));
  }

  #[test]
  fn accessors_reject_invalid_revision_ids() {
    let history = History::new(3, Selection::point(0)).unwrap();

    assert!(matches!(
      history.selection(RevisionId::new(9)),
      Err(HistoryError::RevisionOutOfBounds { .. })
    ));
    assert!(matches!(
      history.revision_len(RevisionId::new(9)),
      Err(HistoryError::RevisionOutOfBounds { .. })
    ));
    assert!(matches!(
      history.revision_timestamp(RevisionId::new(9)),
      Err(HistoryError::RevisionOutOfBounds { .. })
    ));
  }

  #[test]
  fn deletion_undo_restores_deleted_text_from_the_saved_inversion() {
    let mut doc = Rope::from("abcdef");
    let mut history = History::new(doc.len_chars(), Selection::point(0)).unwrap();

    apply_and_commit(
      &mut history,
      &mut doc,
      Transaction::delete(6, span(2, 4)).unwrap(),
      Selection::point(2),
    );
    assert_eq!(doc.to_string(), "abef");

    let jump = history.undo().unwrap();
    apply_jump(&mut doc, &mut history, jump);
    assert_eq!(doc.to_string(), "abcdef");
  }
}
