use std::{
  num::NonZeroUsize,
  time::{
    Duration,
    Instant,
  },
};

use ropey::Rope;
use thiserror::Error;

use crate::{
  selection::{
    Range,
    Selection,
    SelectionError,
  },
  transaction::{
    Assoc,
    ChangeSet,
    Transaction,
    TransactionError,
  },
};

/// Result type for history operations.
pub type Result<T> = std::result::Result<T, HistoryError>;

/// Errors that can occur during history operations.
#[derive(Debug, Error)]
pub enum HistoryError {
  #[error("transaction error: {0}")]
  Transaction(#[from] TransactionError),
  #[error("selection error: {0}")]
  Selection(#[from] SelectionError),
  #[error("revision index {index} is out of bounds (max: {max})")]
  RevisionOutOfBounds { index: usize, max: usize },
}

#[derive(Debug, Clone)]
pub struct State {
  pub doc:       Rope,
  pub selection: Selection,
}

/// Represents a pending jump in history that has not yet been applied.
///
/// This struct is returned by history navigation methods (undo, redo, earlier,
/// later) and contains the transactions to apply along with the target
/// revision. The caller must apply all transactions successfully before calling
/// [`History::apply_jump`] to update the history state.
///
/// This design ensures that history state only changes after successful
/// transaction application, preventing divergence between history and document.
#[derive(Debug, Clone)]
pub struct HistoryJump {
  /// The transactions to apply, in order.
  pub transactions: Vec<Transaction>,
  /// The target revision index after the jump.
  pub target:       usize,
}

impl HistoryJump {
  /// Returns true if this jump has no transactions to apply.
  #[inline]
  pub fn is_empty(&self) -> bool {
    self.transactions.is_empty()
  }

  /// Returns the number of transactions in this jump.
  #[inline]
  pub fn len(&self) -> usize {
    self.transactions.len()
  }
}

/// Stores the history of changes to a buffer.
///
/// Currently the history is represented as a vector of revisions. The vector
/// always has at least one element: the empty root revision. Each revision
/// with the exception of the root has a parent revision, a [Transaction]
/// that can be applied to its parent to transition from the parent to itself,
/// and an inversion of that transaction to transition from the parent to its
/// latest child.
///
/// When using `u` to undo a change, an inverse of the stored transaction will
/// be applied which will transition the buffer to the parent state.
///
/// Each revision with the exception of the last in the vector also has a
/// last child revision. When using `U` to redo a change, the last child
/// transaction will be applied to the current state of the buffer.
///
/// The current revision is the one currently displayed in the buffer.
///
/// Committing a new revision to the history will update the last child of the
/// current revision, and push a new revision to the end of the vector.
///
/// Revisions are committed with a timestamp. Navigation by time finds the
/// closest revision to a moment in time relative to the timestamp of the
/// current revision.
///
/// Limitations:
///  * Changes in selections currently don't commit history changes. The
///    selection will only be updated to the state after a committed buffer
///    change.
///  * The vector of history revisions is currently unbounded. This might cause
///    the memory consumption to grow significantly large during long editing
///    sessions.
///  * Because delete transactions currently don't store the text that they
///    delete, we also store an inversion of the transaction.
///
/// Using time to navigate the history: <https://github.com/helix-editor/helix/pull/194>
#[derive(Debug)]
pub struct History {
  revisions: Vec<Revision>,
  current:   usize,
}

/// A single point in history. See [History] for more information.
#[derive(Debug, Clone)]
struct Revision {
  parent:      usize,
  last_child:  Option<NonZeroUsize>,
  transaction: Transaction,
  // We need an inversion for undos because delete transactions don't store
  // the deleted text.
  inversion:   Transaction,
  timestamp:   Instant,
  /// The selection state at this revision (after the transaction).
  selection:   Option<Selection>,
}

/// Direction hint for breaking ties when finding the nearest revision.
#[derive(Clone, Copy, PartialEq, Eq)]
enum TimeDirection {
  /// Prefer revisions at or before the target instant (for `earlier`).
  Backward,
  /// Prefer revisions at or after the target instant (for `later`).
  Forward,
}

impl Default for History {
  fn default() -> Self {
    // Add a dummy root revision with empty transaction
    Self {
      revisions: vec![Revision {
        parent:      0,
        last_child:  None,
        transaction: Transaction::from(ChangeSet::new("".into())),
        inversion:   Transaction::from(ChangeSet::new("".into())),
        timestamp:   Instant::now(),
        selection:   None,
      }],
      current:   0,
    }
  }
}

impl History {
  pub fn commit_revision(&mut self, transaction: &Transaction, original: &State) -> Result<()> {
    self.commit_revision_at_timestamp(transaction, original, Instant::now())
  }

  pub fn commit_revision_at_timestamp(
    &mut self,
    transaction: &Transaction,
    original: &State,
    timestamp: Instant,
  ) -> Result<()> {
    let selection_after = match transaction.selection() {
      Some(selection) => selection.clone(),
      None => original.selection.clone().map(transaction.changes())?,
    };
    let inversion = transaction
      .invert(&original.doc)?
      // Store the current cursor position
      .with_selection(original.selection.clone());

    let new_current = self.revisions.len();
    self.revisions[self.current].last_child = NonZeroUsize::new(new_current);
    self.revisions.push(Revision {
      parent: self.current,
      last_child: None,
      transaction: transaction.clone().with_selection(selection_after.clone()),
      inversion,
      timestamp,
      selection: Some(selection_after),
    });
    self.current = new_current;
    Ok(())
  }

  #[inline]
  pub fn current_revision(&self) -> usize {
    self.current
  }

  #[inline]
  pub const fn at_root(&self) -> bool {
    self.current == 0
  }

  /// Returns the number of revisions in the history.
  #[inline]
  pub fn len(&self) -> usize {
    self.revisions.len()
  }

  /// Returns whether the history is empty (only has the root revision).
  #[inline]
  pub fn is_empty(&self) -> bool {
    self.revisions.len() <= 1
  }

  /// Validates that a revision index is in bounds.
  fn validate_revision(&self, revision: usize) -> Result<()> {
    if revision >= self.revisions.len() {
      return Err(HistoryError::RevisionOutOfBounds {
        index: revision,
        max:   self.revisions.len().saturating_sub(1),
      });
    }
    Ok(())
  }

  /// Returns the changes since the given revision composed into a transaction.
  /// Returns None if there are no changes between the current and given
  /// revisions.
  ///
  /// The returned transaction preserves selection from the current revision
  /// when available.
  ///
  /// # Errors
  /// Returns an error if the revision is out of bounds or if transaction
  /// composition fails.
  pub fn changes_since(&self, revision: usize) -> Result<Option<Transaction>> {
    self.validate_revision(revision)?;

    if revision == self.current {
      return Ok(None);
    }

    let lca = self.lowest_common_ancestor(revision, self.current);

    // Path from `revision` up to LCA - we need inversions to "undo" these
    let up = self.path_up(revision, lca);
    // Path from `current` up to LCA - we need transactions to "redo" these
    let down = self.path_up(self.current, lca);

    // Build the composed transaction:
    // 1. First, undo from `revision` to LCA (apply inversions in forward order)
    // 2. Then, redo from LCA to `current` (apply transactions in reverse order)
    let mut composed: Option<Transaction> = None;

    // Undo path: from revision towards LCA
    for &n in &up {
      let tx = self.revisions[n].inversion.clone();
      composed = Some(match composed {
        None => tx,
        Some(acc) => acc.compose(tx)?,
      });
    }

    // Redo path: from LCA towards current (reverse the down path)
    for &n in down.iter().rev() {
      let tx = self.revisions[n].transaction.clone();
      composed = Some(match composed {
        None => tx,
        Some(acc) => acc.compose(tx)?,
      });
    }

    // Preserve selection from the current revision if available
    if let Some(mut tx) = composed {
      if let Some(sel) = &self.revisions[self.current].selection {
        tx = tx.with_selection(sel.clone());
      }
      Ok(Some(tx))
    } else {
      Ok(None)
    }
  }

  /// Prepare an undo operation without mutating history state.
  ///
  /// Returns `None` if already at root, otherwise returns a [`HistoryJump`]
  /// containing the transaction to apply and the target revision.
  ///
  /// After successfully applying the transaction, call [`apply_jump`] to
  /// update the history state.
  pub fn undo(&self) -> Option<HistoryJump> {
    if self.at_root() {
      return None;
    }

    let current_revision = &self.revisions[self.current];
    Some(HistoryJump {
      transactions: vec![current_revision.inversion.clone()],
      target:       current_revision.parent,
    })
  }

  /// Prepare a redo operation without mutating history state.
  ///
  /// Returns `None` if there's no redo available (no last_child),
  /// otherwise returns a [`HistoryJump`] containing the transaction to apply
  /// and the target revision.
  ///
  /// After successfully applying the transaction, call [`apply_jump`] to
  /// update the history state.
  pub fn redo(&self) -> Option<HistoryJump> {
    let current_revision = &self.revisions[self.current];
    let last_child = current_revision.last_child?;

    Some(HistoryJump {
      transactions: vec![self.revisions[last_child.get()].transaction.clone()],
      target:       last_child.get(),
    })
  }

  /// Apply a jump, updating the current revision.
  ///
  /// This should only be called after successfully applying all transactions
  /// from the [`HistoryJump`].
  ///
  ///
  /// # Errors
  /// Returns an error if the jump target is out of bounds.
  pub fn apply_jump(&mut self, jump: &HistoryJump) -> Result<()> {
    self.validate_revision(jump.target)?;
    self.current = jump.target;
    Ok(())
  }

  /// Get the position of last change.
  ///
  /// Returns `None` if at the root revision or if the transaction has no
  /// changes.
  pub fn last_edit_pos(&self) -> Option<usize> {
    if self.current == 0 {
      return None;
    }

    let current_revision = &self.revisions[self.current];

    // Get the primary selection from the inversion, if available
    let primary_selection = current_revision.inversion.selection()?.primary();

    // Try to find a change that matches the primary selection
    let change = current_revision
      .transaction
      .changes_iter()
      .find(|(from, to, _)| Range::new(*from, *to).overlaps(&primary_selection))
      .or_else(|| current_revision.transaction.changes_iter().next())?;

    let (_from, to, _fragment) = change;

    // Map the position through the changeset, returning None on error
    current_revision
      .transaction
      .changes()
      .map_pos(to, Assoc::After)
      .ok()
  }

  fn lowest_common_ancestor(&self, mut a: usize, mut b: usize) -> usize {
    use std::collections::HashSet;
    let mut a_path_set = HashSet::new();
    let mut b_path_set = HashSet::new();
    loop {
      a_path_set.insert(a);
      b_path_set.insert(b);
      if a_path_set.contains(&b) {
        return b;
      }
      if b_path_set.contains(&a) {
        return a;
      }
      a = self.revisions[a].parent; // Relies on the parent of 0 being 0.
      b = self.revisions[b].parent; // Same as above.
    }
  }

  /// List of nodes on the way from `n` to 'a`. Doesn't include `a`.
  /// Includes `n` unless `a == n`. `a` must be an ancestor of `n`.
  fn path_up(&self, mut n: usize, a: usize) -> Vec<usize> {
    let mut path = Vec::new();
    while n != a {
      path.push(n);
      n = self.revisions[n].parent;
    }
    path
  }

  /// Create a jump to a specific revision in the history.
  ///
  /// Returns the transactions to apply and the target revision index.
  ///
  /// # Errors
  /// Returns an error if the target revision is out of bounds.
  fn jump_to(&self, to: usize) -> Result<HistoryJump> {
    self.validate_revision(to)?;

    if to == self.current {
      return Ok(HistoryJump {
        transactions: vec![],
        target:       to,
      });
    }

    let lca = self.lowest_common_ancestor(self.current, to);
    let up = self.path_up(self.current, lca);
    let down = self.path_up(to, lca);

    let up_txns = up.iter().map(|&n| self.revisions[n].inversion.clone());
    let down_txns = down
      .iter()
      .rev()
      .map(|&n| self.revisions[n].transaction.clone());

    Ok(HistoryJump {
      transactions: up_txns.chain(down_txns).collect(),
      target:       to,
    })
  }

  /// Walk backward along the branch `n` steps (following parent links).
  ///
  /// This is branch-local: it follows the lineage of the current revision,
  /// not vector indices.
  fn walk_parents(&self, mut from: usize, steps: usize) -> usize {
    for _ in 0..steps {
      if from == 0 {
        break;
      }
      from = self.revisions[from].parent;
    }
    from
  }

  /// Walk forward along the branch `n` steps (following last_child links).
  ///
  /// This is branch-local: it follows the lineage of the current revision,
  /// not vector indices.
  fn walk_children(&self, mut from: usize, steps: usize) -> usize {
    for _ in 0..steps {
      match self.revisions[from].last_child {
        Some(child) => from = child.get(),
        None => break,
      }
    }
    from
  }

  /// Prepare a jump backward by `n` steps along the current branch.
  ///
  /// This follows parent links (branch-local), not vector indices.
  pub fn jump_backward(&self, steps: usize) -> Result<HistoryJump> {
    let target = self.walk_parents(self.current, steps);
    self.jump_to(target)
  }

  /// Prepare a jump forward by `n` steps along the current branch.
  ///
  /// This follows last_child links (branch-local), not vector indices.
  pub fn jump_forward(&self, steps: usize) -> Result<HistoryJump> {
    let target = self.walk_children(self.current, steps);
    self.jump_to(target)
  }

  /// Find the revision closest to the given instant using linear search.
  ///
  /// This handles potentially unsorted timestamps safely, unlike binary search.
  /// The `direction` parameter is used to break ties: when two revisions are
  /// equally close to the target instant, prefer the one in the given
  /// direction.
  ///
  /// Note: This is O(n) in the number of revisions. For very large histories,
  /// consider using a sorted index or bucketed timestamps.
  fn find_revision_nearest_instant(&self, instant: Instant, direction: TimeDirection) -> usize {
    if self.revisions.is_empty() {
      return 0;
    }

    let mut best_idx = 0;
    let mut best_diff: Option<Duration> = None;
    let mut best_is_after: bool = false;

    for (idx, rev) in self.revisions.iter().enumerate() {
      // Calculate absolute difference and whether revision is after target
      let is_after = rev.timestamp >= instant;
      let diff = if is_after {
        rev.timestamp.duration_since(instant)
      } else {
        instant.duration_since(rev.timestamp)
      };

      let dominated = match best_diff {
        None => true,                      // No best yet, always take this one
        Some(best) if diff < best => true, // Strictly closer
        Some(best) if diff == best => {
          // Tie-breaker based on direction
          match direction {
            TimeDirection::Forward => is_after && !best_is_after,
            TimeDirection::Backward => !is_after && best_is_after,
          }
        },
        _ => false,
      };

      if dominated {
        best_idx = idx;
        best_diff = Some(diff);
        best_is_after = is_after;
      }
    }

    best_idx
  }

  /// Prepare a jump to a revision created at around `instant`.
  ///
  /// The `direction` parameter is used to break ties when two revisions
  /// are equally close to the target instant.
  fn jump_instant(&self, instant: Instant, direction: TimeDirection) -> Result<HistoryJump> {
    let revision = self.find_revision_nearest_instant(instant, direction);
    self.jump_to(revision)
  }

  /// Prepare a jump to a revision created `duration` ago from the current
  /// revision's timestamp.
  pub fn jump_duration_backward(&self, duration: Duration) -> Result<HistoryJump> {
    match self.revisions[self.current].timestamp.checked_sub(duration) {
      Some(instant) => self.jump_instant(instant, TimeDirection::Backward),
      None => self.jump_to(0),
    }
  }

  /// Prepare a jump to a revision created `duration` in the future from the
  /// current revision's timestamp.
  pub fn jump_duration_forward(&self, duration: Duration) -> Result<HistoryJump> {
    match self.revisions[self.current].timestamp.checked_add(duration) {
      Some(instant) => self.jump_instant(instant, TimeDirection::Forward),
      None => self.jump_to(self.revisions.len() - 1),
    }
  }

  /// Prepare an "earlier" navigation (undo direction).
  ///
  /// - `Steps(n)`: Jump backward n steps along the current branch (parent
  ///   links)
  /// - `TimePeriod(d)`: Jump to the revision closest to `current_time - d`
  pub fn earlier(&self, uk: UndoKind) -> Result<HistoryJump> {
    match uk {
      UndoKind::Steps(n) => self.jump_backward(n),
      UndoKind::TimePeriod(d) => self.jump_duration_backward(d),
    }
  }

  /// Prepare a "later" navigation (redo direction).
  ///
  /// - `Steps(n)`: Jump forward n steps along the current branch (last_child
  ///   links)
  /// - `TimePeriod(d)`: Jump to the revision closest to `current_time + d`
  pub fn later(&self, uk: UndoKind) -> Result<HistoryJump> {
    match uk {
      UndoKind::Steps(n) => self.jump_forward(n),
      UndoKind::TimePeriod(d) => self.jump_duration_forward(d),
    }
  }

  /// Get the timestamp of the current revision.
  pub fn current_timestamp(&self) -> Instant {
    self.revisions[self.current].timestamp
  }

  /// Get the timestamp of a specific revision.
  ///
  /// # Errors
  /// Returns an error if the revision is out of bounds.
  pub fn revision_timestamp(&self, revision: usize) -> Result<Instant> {
    self.validate_revision(revision)?;
    Ok(self.revisions[revision].timestamp)
  }
}

/// Whether to undo by a number of edits or a duration of time.
///
/// Note: Parsing of this type (e.g., from user input like "5s" or "2m")
/// should be handled in a higher layer, not in the history module.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum UndoKind {
  /// Number of steps to move along the branch.
  Steps(usize),
  /// Duration to travel in time.
  TimePeriod(Duration),
}

#[cfg(test)]
mod test {
  use super::*;

  /// Helper to apply a HistoryJump to state and history.
  fn apply_jump_to_state(history: &mut History, state: &mut State, jump: HistoryJump) {
    for txn in &jump.transactions {
      txn.apply(&mut state.doc).unwrap();
      if let Some(sel) = txn.selection() {
        state.selection = sel.clone();
      }
    }
    history.apply_jump(&jump).unwrap();
  }

  #[test]
  fn test_undo_redo() {
    let mut history = History::default();
    let doc = Rope::from("hello");
    let mut state = State {
      doc,
      selection: Selection::point(0),
    };

    let transaction1 =
      Transaction::change(&state.doc, vec![(5, 5, Some(" world!".into()))]).unwrap();

    // Need to commit before applying!
    history.commit_revision(&transaction1, &state).unwrap();
    transaction1.apply(&mut state.doc).unwrap();
    assert_eq!("hello world!", state.doc);

    // ---

    let transaction2 = Transaction::change(&state.doc, vec![(6, 11, Some("世界".into()))]).unwrap();

    // Need to commit before applying!
    history.commit_revision(&transaction2, &state).unwrap();
    transaction2.apply(&mut state.doc).unwrap();
    assert_eq!("hello 世界!", state.doc);

    // --- Test undo/redo with explicit jump pattern
    fn undo(history: &mut History, state: &mut State) {
      if let Some(jump) = history.undo() {
        apply_jump_to_state(history, state, jump);
      }
    }
    fn redo(history: &mut History, state: &mut State) {
      if let Some(jump) = history.redo() {
        apply_jump_to_state(history, state, jump);
      }
    }

    undo(&mut history, &mut state);
    assert_eq!("hello world!", state.doc);
    redo(&mut history, &mut state);
    assert_eq!("hello 世界!", state.doc);
    undo(&mut history, &mut state);
    undo(&mut history, &mut state);
    assert_eq!("hello", state.doc);

    // undo at root is a no-op
    undo(&mut history, &mut state);
    assert_eq!("hello", state.doc);
  }

  #[test]
  fn test_undo_does_not_mutate_before_apply() {
    let mut history = History::default();
    let doc = Rope::from("hello");
    let mut state = State {
      doc,
      selection: Selection::point(0),
    };

    let transaction = Transaction::change(&state.doc, vec![(5, 5, Some(" world".into()))]).unwrap();
    history.commit_revision(&transaction, &state).unwrap();
    transaction.apply(&mut state.doc).unwrap();

    assert_eq!(history.current_revision(), 1);

    // Get undo jump but don't apply it
    let jump = history.undo().unwrap();
    assert_eq!(jump.target, 0);

    // History should NOT have changed yet
    assert_eq!(history.current_revision(), 1);

    // Now apply the jump
    for txn in &jump.transactions {
      txn.apply(&mut state.doc).unwrap();
    }
    history.apply_jump(&jump).unwrap();

    // Now history should have changed
    assert_eq!(history.current_revision(), 0);
    assert_eq!("hello", state.doc);
  }

  #[test]
  fn test_redo_does_not_mutate_before_apply() {
    let mut history = History::default();
    let doc = Rope::from("hello");
    let mut state = State {
      doc,
      selection: Selection::point(0),
    };

    let transaction = Transaction::change(&state.doc, vec![(5, 5, Some(" world".into()))]).unwrap();
    history.commit_revision(&transaction, &state).unwrap();
    transaction.apply(&mut state.doc).unwrap();

    // Undo to get back to root
    let undo_jump = history.undo().unwrap();
    for txn in &undo_jump.transactions {
      txn.apply(&mut state.doc).unwrap();
    }
    history.apply_jump(&undo_jump).unwrap();
    assert_eq!(history.current_revision(), 0);

    // Get redo jump but don't apply it
    let redo_jump = history.redo().unwrap();
    assert_eq!(redo_jump.target, 1);

    // History should NOT have changed yet
    assert_eq!(history.current_revision(), 0);

    // Now apply the jump
    for txn in &redo_jump.transactions {
      txn.apply(&mut state.doc).unwrap();
    }
    history.apply_jump(&redo_jump).unwrap();

    // Now history should have changed
    assert_eq!(history.current_revision(), 1);
    assert_eq!("hello world", state.doc);
  }

  #[test]
  fn test_branch_local_steps() {
    // Test that Steps(n) follows branch lineage, not vector indices
    //
    // We'll create this structure:
    //   0 -> 1 -> 2 -> 3
    //             \-> 4 (branch from 2)
    //
    // At revision 4, Steps(1) backward should go to 2, not 3
    // At revision 3, Steps(1) forward should stay at 3 (no child)

    let mut history = History::default();
    let doc = Rope::from("a");
    let mut state = State {
      doc,
      selection: Selection::point(0),
    };

    // Create revision 1
    let tx1 = Transaction::change(&state.doc, vec![(1, 1, Some("b".into()))]).unwrap();
    history.commit_revision(&tx1, &state).unwrap();
    tx1.apply(&mut state.doc).unwrap();
    assert_eq!("ab", state.doc);

    // Create revision 2
    let tx2 = Transaction::change(&state.doc, vec![(2, 2, Some("c".into()))]).unwrap();
    history.commit_revision(&tx2, &state).unwrap();
    tx2.apply(&mut state.doc).unwrap();
    assert_eq!("abc", state.doc);

    // Create revision 3
    let tx3 = Transaction::change(&state.doc, vec![(3, 3, Some("d".into()))]).unwrap();
    history.commit_revision(&tx3, &state).unwrap();
    tx3.apply(&mut state.doc).unwrap();
    assert_eq!("abcd", state.doc);
    assert_eq!(history.current_revision(), 3);

    // Undo back to revision 2
    let jump = history.undo().unwrap();
    apply_jump_to_state(&mut history, &mut state, jump);
    assert_eq!(history.current_revision(), 2);
    assert_eq!("abc", state.doc);

    // Create revision 4 (branch from 2)
    let tx4 = Transaction::change(&state.doc, vec![(3, 3, Some("e".into()))]).unwrap();
    history.commit_revision(&tx4, &state).unwrap();
    tx4.apply(&mut state.doc).unwrap();
    assert_eq!("abce", state.doc);
    assert_eq!(history.current_revision(), 4);

    // Now at revision 4, Steps(1) backward should go to 2 (parent), not 3
    let jump = history.jump_backward(1).unwrap();
    assert_eq!(jump.target, 2);
    apply_jump_to_state(&mut history, &mut state, jump);
    assert_eq!("abc", state.doc);

    // From revision 2, Steps(1) forward should go to 4 (last_child), not 3
    let jump = history.jump_forward(1).unwrap();
    assert_eq!(jump.target, 4);
    apply_jump_to_state(&mut history, &mut state, jump);
    assert_eq!("abce", state.doc);

    // Now let's go to revision 3 and verify forward goes nowhere
    let jump = history.jump_to(3).unwrap();
    apply_jump_to_state(&mut history, &mut state, jump);
    assert_eq!(history.current_revision(), 3);
    assert_eq!("abcd", state.doc);

    // Steps(1) forward from 3 should stay at 3 (no last_child)
    let jump = history.jump_forward(1).unwrap();
    assert_eq!(jump.target, 3);
    assert!(jump.is_empty()); // No transactions needed
  }

  #[test]
  fn test_earlier_later_time_based() {
    let mut history = History::default();
    let doc = Rope::from("a\n");
    let mut state = State {
      doc,
      selection: Selection::point(0),
    };

    fn commit_change(
      history: &mut History,
      state: &mut State,
      change: crate::transaction::Change,
      instant: Instant,
    ) {
      let txn = Transaction::change(&state.doc, vec![change]).unwrap();
      history
        .commit_revision_at_timestamp(&txn, state, instant)
        .unwrap();
      txn.apply(&mut state.doc).unwrap();
    }

    let t0 = Instant::now();
    let t = |n| t0.checked_add(Duration::from_secs(n)).unwrap();

    commit_change(&mut history, &mut state, (1, 1, Some(" b".into())), t(0));
    assert_eq!("a b\n", state.doc);

    commit_change(&mut history, &mut state, (3, 3, Some(" c".into())), t(10));
    assert_eq!("a b c\n", state.doc);

    commit_change(&mut history, &mut state, (5, 5, Some(" d".into())), t(20));
    assert_eq!("a b c d\n", state.doc);

    // Undo to revision 2
    let jump = history.undo().unwrap();
    apply_jump_to_state(&mut history, &mut state, jump);
    assert_eq!("a b c\n", state.doc);

    commit_change(&mut history, &mut state, (5, 5, Some(" e".into())), t(30));
    assert_eq!("a b c e\n", state.doc);

    // Undo twice to revision 1
    let jump = history.undo().unwrap();
    apply_jump_to_state(&mut history, &mut state, jump);
    let jump = history.undo().unwrap();
    apply_jump_to_state(&mut history, &mut state, jump);
    assert_eq!("a b\n", state.doc);

    commit_change(&mut history, &mut state, (1, 3, None), t(40));
    assert_eq!("a\n", state.doc);

    commit_change(&mut history, &mut state, (1, 1, Some(" f".into())), t(50));
    assert_eq!("a f\n", state.doc);

    // Test time-based navigation
    // Current is at t(50), go back 30s to find revision near t(20)
    let jump = history
      .jump_duration_backward(Duration::from_secs(30))
      .unwrap();
    apply_jump_to_state(&mut history, &mut state, jump);
    assert_eq!("a b c d\n", state.doc); // revision 3 at t(20)

    // From t(20), go forward 20s to t(40)
    let jump = history
      .jump_duration_forward(Duration::from_secs(20))
      .unwrap();
    apply_jump_to_state(&mut history, &mut state, jump);
    assert_eq!("a\n", state.doc); // revision 5 at t(40)
  }

  #[test]
  fn test_time_tie_breaking() {
    // Test that tie-breaking works: earlier prefers backward, later prefers forward
    let mut history = History::default();
    let doc = Rope::from("a");
    let mut state = State {
      doc,
      selection: Selection::point(0),
    };

    let t0 = Instant::now();
    let t = |n| t0.checked_add(Duration::from_secs(n)).unwrap();

    // Create revisions at t(10) and t(20)
    let tx1 = Transaction::change(&state.doc, vec![(1, 1, Some("b".into()))]).unwrap();
    history
      .commit_revision_at_timestamp(&tx1, &state, t(10))
      .unwrap();
    tx1.apply(&mut state.doc).unwrap();

    let tx2 = Transaction::change(&state.doc, vec![(2, 2, Some("c".into()))]).unwrap();
    history
      .commit_revision_at_timestamp(&tx2, &state, t(20))
      .unwrap();
    tx2.apply(&mut state.doc).unwrap();
    assert_eq!("abc", state.doc);

    // Go to revision 1 (t=10)
    let jump = history.jump_to(1).unwrap();
    apply_jump_to_state(&mut history, &mut state, jump);

    // From t(10), jump forward 5s to t(15)
    // t(15) is equidistant from t(10) and t(20)
    // With Forward direction, should pick t(20) = revision 2
    let jump = history
      .jump_duration_forward(Duration::from_secs(5))
      .unwrap();
    assert_eq!(jump.target, 2);
    apply_jump_to_state(&mut history, &mut state, jump);

    // From t(20), jump backward 5s to t(15)
    // With Backward direction, should pick t(10) = revision 1
    let jump = history
      .jump_duration_backward(Duration::from_secs(5))
      .unwrap();
    assert_eq!(jump.target, 1);
  }

  #[test]
  fn test_changes_since_preserves_selection() {
    let mut history = History::default();
    let doc = Rope::from("hello");
    let mut state = State {
      doc,
      selection: Selection::point(0),
    };

    // Commit with a specific selection
    state.selection = Selection::point(3);
    let tx = Transaction::change(&state.doc, vec![(5, 5, Some(" world".into()))]).unwrap();
    history.commit_revision(&tx, &state).unwrap();
    tx.apply(&mut state.doc).unwrap();

    // Get changes since root
    let changes = history.changes_since(0).unwrap().unwrap();

    // The selection should be preserved
    assert!(changes.selection().is_some());
  }

  #[test]
  fn test_bounds_checking() {
    let history = History::default();

    // Valid revision
    assert!(history.changes_since(0).is_ok());

    // Invalid revision
    assert!(matches!(
      history.changes_since(100),
      Err(HistoryError::RevisionOutOfBounds { .. })
    ));

    // Jump to invalid revision
    assert!(matches!(
      history.jump_to(100),
      Err(HistoryError::RevisionOutOfBounds { .. })
    ));
  }

  #[test]
  fn test_last_edit_pos_empty() {
    let history = History::default();
    // At root, should return None
    assert!(history.last_edit_pos().is_none());
  }

  #[test]
  fn test_history_jump_is_empty() {
    let history = History::default();

    // Jump to current revision should be empty
    let jump = history.jump_to(0).unwrap();
    assert!(jump.is_empty());
    assert_eq!(jump.len(), 0);
  }

  #[test]
  fn test_walk_parents_and_children() {
    let mut history = History::default();
    let doc = Rope::from("a");
    let mut state = State {
      doc,
      selection: Selection::point(0),
    };

    // Create linear history: 0 -> 1 -> 2 -> 3
    for i in 1..=3 {
      let ch = char::from_digit(i, 10).unwrap();
      let tx = Transaction::change(&state.doc, vec![(
        state.doc.len_chars(),
        state.doc.len_chars(),
        Some(ch.to_string().into()),
      )])
      .unwrap();
      history.commit_revision(&tx, &state).unwrap();
      tx.apply(&mut state.doc).unwrap();
    }
    assert_eq!(history.current_revision(), 3);

    // walk_parents from 3 by 2 should reach 1
    assert_eq!(history.walk_parents(3, 2), 1);

    // walk_parents from 3 by 10 should reach 0 (root)
    assert_eq!(history.walk_parents(3, 10), 0);

    // walk_children from 0 by 2 should reach 2
    assert_eq!(history.walk_children(0, 2), 2);

    // walk_children from 0 by 10 should reach 3 (last)
    assert_eq!(history.walk_children(0, 10), 3);
  }

  #[test]
  fn test_earlier_later_steps_branch_local() {
    // Verify that UndoKind::Steps uses branch-local navigation
    let mut history = History::default();
    let doc = Rope::from("a");
    let mut state = State {
      doc,
      selection: Selection::point(0),
    };

    // Create: 0 -> 1 -> 2
    let tx1 = Transaction::change(&state.doc, vec![(1, 1, Some("b".into()))]).unwrap();
    history.commit_revision(&tx1, &state).unwrap();
    tx1.apply(&mut state.doc).unwrap();

    let tx2 = Transaction::change(&state.doc, vec![(2, 2, Some("c".into()))]).unwrap();
    history.commit_revision(&tx2, &state).unwrap();
    tx2.apply(&mut state.doc).unwrap();

    // Undo to 1, then create branch: 1 -> 3
    let jump = history.undo().unwrap();
    apply_jump_to_state(&mut history, &mut state, jump);

    let tx3 = Transaction::change(&state.doc, vec![(2, 2, Some("d".into()))]).unwrap();
    history.commit_revision(&tx3, &state).unwrap();
    tx3.apply(&mut state.doc).unwrap();
    assert_eq!(history.current_revision(), 3);
    assert_eq!("abd", state.doc);

    // earlier(Steps(1)) from 3 should go to 1 (parent), not 2
    let jump = history.earlier(UndoKind::Steps(1)).unwrap();
    assert_eq!(jump.target, 1);
    apply_jump_to_state(&mut history, &mut state, jump);
    assert_eq!("ab", state.doc);

    // later(Steps(1)) from 1 should go to 3 (last_child), not 2
    let jump = history.later(UndoKind::Steps(1)).unwrap();
    assert_eq!(jump.target, 3);
    apply_jump_to_state(&mut history, &mut state, jump);
    assert_eq!("abd", state.doc);
  }

  #[test]
  fn test_current_timestamp() {
    let mut history = History::default();
    let doc = Rope::from("a");
    let state = State {
      doc,
      selection: Selection::point(0),
    };

    let t0 = Instant::now();
    let t1 = t0.checked_add(Duration::from_secs(10)).unwrap();

    let tx = Transaction::change(&state.doc, vec![(1, 1, Some("b".into()))]).unwrap();
    history
      .commit_revision_at_timestamp(&tx, &state, t1)
      .unwrap();

    assert_eq!(history.current_timestamp(), t1);
    assert_eq!(history.revision_timestamp(1).unwrap(), t1);
    assert!(history.revision_timestamp(100).is_err());
  }
}
