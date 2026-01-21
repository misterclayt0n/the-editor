use std::{
  num::NonZeroUsize,
  time::{
    Duration,
    Instant,
  },
};

use once_cell::sync::Lazy;
use regex::Regex;
use ropey::Rope;
use thiserror::Error;

use crate::{
  selection::{
    Range,
    Selection,
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
  #[error("revision index {index} is out of bounds (max: {max})")]
  RevisionOutOfBounds { index: usize, max: usize },
  #[error("failed to compose transactions: {0}")]
  ComposeFailure(TransactionError),
}

#[derive(Debug, Clone)]
pub struct State {
  pub doc:       Rope,
  pub selection: Selection,
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
/// Revisions are committed with a timestamp. :earlier and :later can be used
/// to jump to the closest revision to a moment in time relative to the
/// timestamp of the current revision plus (:later) or minus (:earlier) the
/// duration given to the command. If a single integer is given, the editor will
/// instead jump the given number of revisions in the vector.
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
    let inversion = transaction
      .invert(&original.doc)?
      // Store the current cursor position
      .with_selection(original.selection.clone());

    let new_current = self.revisions.len();
    self.revisions[self.current].last_child = NonZeroUsize::new(new_current);
    self.revisions.push(Revision {
      parent: self.current,
      last_child: None,
      transaction: transaction.clone(),
      inversion,
      timestamp,
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
        Some(acc) => acc.compose(tx).map_err(HistoryError::ComposeFailure)?,
      });
    }

    // Redo path: from LCA towards current (reverse the down path)
    for &n in down.iter().rev() {
      let tx = self.revisions[n].transaction.clone();
      composed = Some(match composed {
        None => tx,
        Some(acc) => acc.compose(tx).map_err(HistoryError::ComposeFailure)?,
      });
    }

    Ok(composed)
  }

  /// Undo the last edit.
  pub fn undo(&mut self) -> Option<&Transaction> {
    if self.at_root() {
      return None;
    }

    let current_revision = &self.revisions[self.current];
    self.current = current_revision.parent;
    Some(&current_revision.inversion)
  }

  /// Redo the last edit.
  pub fn redo(&mut self) -> Option<&Transaction> {
    let current_revision = &self.revisions[self.current];
    let last_child = current_revision.last_child?;
    self.current = last_child.get();

    Some(&self.revisions[last_child.get()].transaction)
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

  /// Create transactions that will jump to a specific revision in the history.
  ///
  /// Returns the transactions to apply and the target revision index.
  /// The caller must apply all transactions successfully before the history
  /// state is considered changed.
  ///
  /// # Errors
  /// Returns an error if the target revision is out of bounds.
  fn jump_to(&self, to: usize) -> Result<(Vec<Transaction>, usize)> {
    self.validate_revision(to)?;

    let lca = self.lowest_common_ancestor(self.current, to);
    let up = self.path_up(self.current, lca);
    let down = self.path_up(to, lca);

    let up_txns = up.iter().map(|&n| self.revisions[n].inversion.clone());
    let down_txns = down
      .iter()
      .rev()
      .map(|&n| self.revisions[n].transaction.clone());

    Ok((up_txns.chain(down_txns).collect(), to))
  }

  /// Apply a jump, updating the current revision after successful application.
  ///
  /// This is the safe way to jump - it only updates state after returning
  /// transactions that the caller will apply.
  fn apply_jump(&mut self, to: usize) -> Result<Vec<Transaction>> {
    let (txns, target) = self.jump_to(to)?;
    self.current = target;
    Ok(txns)
  }

  /// Creates transactions that will undo `delta` revisions.
  fn jump_backward(&mut self, delta: usize) -> Result<Vec<Transaction>> {
    self.apply_jump(self.current.saturating_sub(delta))
  }

  /// Creates transactions that will redo `delta` revisions.
  fn jump_forward(&mut self, delta: usize) -> Result<Vec<Transaction>> {
    let target = self
      .current
      .saturating_add(delta)
      .min(self.revisions.len() - 1);
    self.apply_jump(target)
  }

  /// Find the revision closest to the given instant using linear search.
  ///
  /// This handles potentially unsorted timestamps safely, unlike binary search.
  /// The `direction` parameter is used to break ties: when two revisions are
  /// equally close to the target instant, prefer the one in the given
  /// direction.
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

  /// Creates transactions that will match a revision created at around
  /// `instant`, preferring the given direction for tie-breaking.
  fn jump_instant(
    &mut self,
    instant: Instant,
    direction: TimeDirection,
  ) -> Result<Vec<Transaction>> {
    let revision = self.find_revision_nearest_instant(instant, direction);
    self.apply_jump(revision)
  }

  /// Creates transactions that will match a revision created `duration`
  /// ago from the timestamp of current revision.
  fn jump_duration_backward(&mut self, duration: Duration) -> Result<Vec<Transaction>> {
    match self.revisions[self.current].timestamp.checked_sub(duration) {
      Some(instant) => self.jump_instant(instant, TimeDirection::Backward),
      None => self.apply_jump(0),
    }
  }

  /// Creates transactions that will match a revision created `duration` in
  /// the future from the timestamp of the current revision.
  fn jump_duration_forward(&mut self, duration: Duration) -> Result<Vec<Transaction>> {
    match self.revisions[self.current].timestamp.checked_add(duration) {
      Some(instant) => self.jump_instant(instant, TimeDirection::Forward),
      None => self.apply_jump(self.revisions.len() - 1),
    }
  }

  /// Creates undo transactions.
  pub fn earlier(&mut self, uk: UndoKind) -> Result<Vec<Transaction>> {
    use UndoKind::*;
    match uk {
      Steps(n) => self.jump_backward(n),
      TimePeriod(d) => self.jump_duration_backward(d),
    }
  }

  /// Creates redo transactions.
  pub fn later(&mut self, uk: UndoKind) -> Result<Vec<Transaction>> {
    use UndoKind::*;
    match uk {
      Steps(n) => self.jump_forward(n),
      TimePeriod(d) => self.jump_duration_forward(d),
    }
  }
}

/// Whether to undo by a number of edits or a duration of time.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum UndoKind {
  Steps(usize),
  TimePeriod(std::time::Duration),
}

/// A subset of systemd.time time span syntax units.
const TIME_UNITS: &[(&[&str], &str, u64)] = &[
  (&["seconds", "second", "sec", "s"], "seconds", 1),
  (&["minutes", "minute", "min", "m"], "minutes", 60),
  (&["hours", "hour", "hr", "h"], "hours", 60 * 60),
  (&["days", "day", "d"], "days", 24 * 60 * 60),
];

/// Checks if the duration input can be turned into a valid duration. It must be
/// a positive integer and denote the [unit of time.](`TIME_UNITS`)
/// Examples of valid durations:
///  * `5 sec`
///  * `5 min`
///  * `5 hr`
///  * `5 days`
static DURATION_VALIDATION_REGEX: Lazy<Regex> =
  Lazy::new(|| Regex::new(r"^(?:\d+\s*[a-z]+\s*)+$").unwrap());

/// Captures both the number and unit as separate capture groups.
static NUMBER_UNIT_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"(\d+)\s*([a-z]+)").unwrap());

/// Parse a string (e.g. "5 sec") and try to convert it into a [`Duration`].
fn parse_human_duration(s: &str) -> std::result::Result<Duration, String> {
  if !DURATION_VALIDATION_REGEX.is_match(s) {
    return Err(
      "duration should be composed of positive integers followed by time units".to_string(),
    );
  }

  let mut specified = [false; TIME_UNITS.len()];
  let mut seconds = 0u64;
  for cap in NUMBER_UNIT_REGEX.captures_iter(s) {
    let (n, unit_str) = (&cap[1], &cap[2]);

    let n: u64 = n.parse().map_err(|_| format!("integer too large: {}", n))?;

    // Reject zero values as they don't make sense for time navigation
    if n == 0 {
      return Err("duration values must be greater than zero".to_string());
    }

    let time_unit = TIME_UNITS
      .iter()
      .enumerate()
      .find(|(_, (forms, ..))| forms.iter().any(|f| f == &unit_str));

    if let Some((i, (_, unit, mul))) = time_unit {
      if specified[i] {
        return Err(format!("{} specified more than once", unit));
      }
      specified[i] = true;

      let new_seconds = n.checked_mul(*mul).and_then(|s| seconds.checked_add(s));
      match new_seconds {
        Some(ns) => seconds = ns,
        None => return Err("duration too large".to_string()),
      }
    } else {
      return Err(format!("incorrect time unit: {}", unit_str));
    }
  }

  Ok(Duration::from_secs(seconds))
}

impl std::str::FromStr for UndoKind {
  type Err = String;

  fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
    let s = s.trim();
    if s.is_empty() {
      Ok(Self::Steps(1usize))
    } else if let Ok(n) = s.parse::<usize>() {
      Ok(UndoKind::Steps(n))
    } else {
      Ok(Self::TimePeriod(parse_human_duration(s)?))
    }
  }
}

#[cfg(test)]
mod test {
  use super::*;

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

    // ---
    fn undo(history: &mut History, state: &mut State) {
      if let Some(transaction) = history.undo() {
        transaction.apply(&mut state.doc).unwrap();
      }
    }
    fn redo(history: &mut History, state: &mut State) {
      if let Some(transaction) = history.redo() {
        transaction.apply(&mut state.doc).unwrap();
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
  fn test_earlier_later() {
    let mut history = History::default();
    let doc = Rope::from("a\n");
    let mut state = State {
      doc,
      selection: Selection::point(0),
    };

    fn undo(history: &mut History, state: &mut State) {
      if let Some(transaction) = history.undo() {
        transaction.apply(&mut state.doc).unwrap();
      }
    }

    fn earlier(history: &mut History, state: &mut State, uk: UndoKind) {
      let txns = history.earlier(uk).unwrap();
      for txn in txns {
        txn.apply(&mut state.doc).unwrap();
      }
    }

    fn later(history: &mut History, state: &mut State, uk: UndoKind) {
      let txns = history.later(uk).unwrap();
      for txn in txns {
        txn.apply(&mut state.doc).unwrap();
      }
    }

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

    undo(&mut history, &mut state);
    assert_eq!("a b c\n", state.doc);

    commit_change(&mut history, &mut state, (5, 5, Some(" e".into())), t(30));
    assert_eq!("a b c e\n", state.doc);

    undo(&mut history, &mut state);
    undo(&mut history, &mut state);
    assert_eq!("a b\n", state.doc);

    commit_change(&mut history, &mut state, (1, 3, None), t(40));
    assert_eq!("a\n", state.doc);

    commit_change(&mut history, &mut state, (1, 1, Some(" f".into())), t(50));
    assert_eq!("a f\n", state.doc);

    use UndoKind::*;

    earlier(&mut history, &mut state, Steps(3));
    assert_eq!("a b c d\n", state.doc.to_string());

    later(&mut history, &mut state, TimePeriod(Duration::new(20, 0)));
    assert_eq!("a\n", state.doc);

    earlier(&mut history, &mut state, TimePeriod(Duration::new(19, 0)));
    assert_eq!("a b c d\n", state.doc);

    earlier(
      &mut history,
      &mut state,
      TimePeriod(Duration::new(10000, 0)),
    );
    assert_eq!("a\n", state.doc);

    later(&mut history, &mut state, Steps(50));
    assert_eq!("a f\n", state.doc);

    earlier(&mut history, &mut state, Steps(4));
    assert_eq!("a b c\n", state.doc);

    later(&mut history, &mut state, TimePeriod(Duration::new(1, 0)));
    assert_eq!("a b c\n", state.doc);

    later(&mut history, &mut state, TimePeriod(Duration::new(5, 0)));
    assert_eq!("a b c d\n", state.doc);

    later(&mut history, &mut state, TimePeriod(Duration::new(6, 0)));
    assert_eq!("a b c e\n", state.doc);

    later(&mut history, &mut state, Steps(1));
    assert_eq!("a\n", state.doc);
  }

  #[test]
  fn test_parse_undo_kind() {
    use UndoKind::*;

    // Default is one step.
    assert_eq!("".parse(), Ok(Steps(1)));

    // An integer means the number of steps.
    assert_eq!("1".parse(), Ok(Steps(1)));
    assert_eq!("  16 ".parse(), Ok(Steps(16)));

    // Duration has a strict format.
    let validation_err =
      Err("duration should be composed of positive integers followed by time units".to_string());
    assert_eq!("  16 33".parse::<UndoKind>(), validation_err);
    assert_eq!("  seconds 22  ".parse::<UndoKind>(), validation_err);
    assert_eq!("  -4 m".parse::<UndoKind>(), validation_err);
    assert_eq!("5s 3".parse::<UndoKind>(), validation_err);

    // Units are u64.
    assert_eq!(
      "18446744073709551616minutes".parse::<UndoKind>(),
      Err("integer too large: 18446744073709551616".to_string())
    );

    // Units are validated.
    assert_eq!(
      "1 millennium".parse::<UndoKind>(),
      Err("incorrect time unit: millennium".to_string())
    );

    // Units can't be specified twice.
    assert_eq!(
      "2 seconds 6s".parse::<UndoKind>(),
      Err("seconds specified more than once".to_string())
    );

    // Zero values are rejected.
    assert_eq!(
      "0s".parse::<UndoKind>(),
      Err("duration values must be greater than zero".to_string())
    );

    // Various formats are correctly handled.
    assert_eq!(
      "4s".parse::<UndoKind>(),
      Ok(TimePeriod(Duration::from_secs(4)))
    );
    assert_eq!(
      "2m".parse::<UndoKind>(),
      Ok(TimePeriod(Duration::from_secs(120)))
    );
    assert_eq!(
      "5h".parse::<UndoKind>(),
      Ok(TimePeriod(Duration::from_secs(5 * 60 * 60)))
    );
    assert_eq!(
      "3d".parse::<UndoKind>(),
      Ok(TimePeriod(Duration::from_secs(3 * 24 * 60 * 60)))
    );
    assert_eq!(
      "1m30s".parse::<UndoKind>(),
      Ok(TimePeriod(Duration::from_secs(90)))
    );
    assert_eq!(
      "1m 20 seconds".parse::<UndoKind>(),
      Ok(TimePeriod(Duration::from_secs(80)))
    );
    assert_eq!(
      "  2 minute 1day".parse::<UndoKind>(),
      Ok(TimePeriod(Duration::from_secs(24 * 60 * 60 + 2 * 60)))
    );
    assert_eq!(
      "3 d 2hour 5 minutes 30sec".parse::<UndoKind>(),
      Ok(TimePeriod(Duration::from_secs(
        3 * 24 * 60 * 60 + 2 * 60 * 60 + 5 * 60 + 30
      )))
    );

    // Sum overflow is handled.
    assert_eq!(
      "18446744073709551615minutes".parse::<UndoKind>(),
      Err("duration too large".to_string())
    );
    assert_eq!(
      "1 minute 18446744073709551615 seconds".parse::<UndoKind>(),
      Err("duration too large".to_string())
    );
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
  }

  #[test]
  fn test_last_edit_pos_empty() {
    let history = History::default();
    // At root, should return None
    assert!(history.last_edit_pos().is_none());
  }
}
