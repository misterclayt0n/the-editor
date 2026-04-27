# History Module Architecture

This document defines the target architecture for `the-lib/history.rs`.

It answers four questions:

1. What is history in the new editor?
2. What should the public API feel like?
3. What belongs in `history.rs`, and what does not?
4. What should be removed from the old pasted implementation?

This document is intentionally opinionated. `history.rs` is a backbone module, but it should stay smaller and simpler than `selection.rs` and `transaction.rs`. It should be explicit, deterministic, and hard to misuse.

---

## Purpose

`history.rs` defines the native model for **editing-session history**.

It is responsible for:
- storing a revision graph of document changes
- storing the inverse transaction needed to move backward
- storing the exact committed selection snapshot for each revision
- preparing undo/redo/jump operations without mutating external editor state
- validating that history state stays consistent with document lengths and selection bounds
- supporting branch-local and time-based history navigation

It is **not** responsible for:
- representing document changes themselves
- owning selection semantics globally across many surfaces
- applying document changes itself
- inventing post-edit selections by inference
- command behavior or UI workflow
- becoming a giant editor-session state bag

---

## Core position

`History` is **session / surface history over one document**, not a canonical multi-surface document timeline.

That means this module intentionally models:
- one editing session
- one revision graph
- one exact committed selection snapshot per revision

This matters because we already decided:
- `Transaction` is a **pure document delta**
- `Selection` is conceptually **surface-owned**
- undo/redo should restore the **exact committed selection snapshot**

Those three decisions fit together cleanly only if this history module is understood as **single-session history**.

So the desired split is:

```txt
Transaction: pure document delta
Selection:  directional cursor/range state
History:    revision graph + exact committed selection snapshots for one session
```

---

## Relationship to `transaction.rs` and `selection.rs`

### `Transaction`
A transaction means:
- how text changes
- how to apply/invert/compose/map a document delta

A transaction does **not** mean:
- cursor movement
- selection restoration
- session history state

### `Selection`
A selection means:
- directional ranges
- stable cursor identity
- view/session-local cursor state

### `History`
A history revision means:
- how to get here textually
- how to get back
- what exact selection snapshot this session should restore when landing here

This is why selection restoration belongs **explicitly in history**, not hidden inside `Transaction`.

---

## Main decisions

### 1. `History` is session-local, not canonical document-global history
**Decision:** `the-lib/history.rs` models the undo/redo history for one editing session / surface.

**Reasoning:**
If exact committed selections are restored by history, then the history cannot honestly be a shared multi-surface document timeline. A single document may be shown by many surfaces with different selections.

Implication:
- this module may store `Selection` snapshots
- that does not mean `Transaction` should carry selection
- a future multi-surface design can split document history from surface history if needed

---

### 2. `Transaction` remains document-only
**Decision:** history must not reintroduce selection payloads into `Transaction`.

**Reasoning:**
We already established that `Transaction` should stay a pure document delta. Undo/redo correctness should not be bought by contaminating the core edit primitive.

Implication:
- remove all `transaction.selection()` logic
- remove all `tx.with_selection(...)` logic
- do not encode selection restoration into transactions

---

### 3. History restores exact committed selection snapshots
**Decision:** history stores and restores the exact selection snapshot of each landed revision.

**Reasoning:**
This is simpler and more deterministic than trying to derive selection by mapping the current selection through forward or inverse transactions.

Implication:
- commit must receive the exact `selection_after`
- jumps should return the exact target selection
- undo/redo should not infer selection from text deltas

---

### 4. `History` should not use `Default`
**Decision:** root history state must be initialized explicitly.

**Reasoning:**
If root is a real landed revision with a real exact selection snapshot, then `Default` is dishonest. The root state should reflect the actual initial editor state.

Preferred constructors:

```rust
History::new(initial_len, initial_selection)
History::new_at_timestamp(initial_len, initial_selection, timestamp)
```

---

### 5. Revision identity should use a newtype
**Decision:** public history APIs should use a `RevisionId` newtype, not raw `usize`.

**Reasoning:**
History APIs already live in a world full of other `usize`s: text positions, lengths, range indices, counts. Revision identity is important enough to deserve its own type.

Preferred shape:

```rust
pub struct RevisionId(usize);
```

Use raw `usize` only internally for `Vec` indexing.

---

### 6. `commit` should take the transaction by value
**Decision:** history should own the committed transaction directly.

**Reasoning:**
History stores the forward transaction anyway, so taking it by value is cleaner than taking `&Transaction` and cloning internally.

Preferred shape:

```rust
pub fn commit(
  &mut self,
  before: &Rope,
  transaction: Transaction,
  selection_after: Selection,
) -> Result<RevisionId>
```

---

### 7. `commit` should happen after successful application
**Decision:** callers should commit only after the transaction has already been successfully applied to the real document and the final `selection_after` is known.

**Reasoning:**
This matches the same safety story as prepared jumps: history state should only change after the real editor state changed successfully.

Preferred flow:
1. caller keeps `before_doc`
2. caller computes transaction
3. caller applies transaction to the real document
4. caller computes/finalizes exact `selection_after`
5. caller calls `history.commit(&before_doc, transaction, selection_after)`

This avoids divergence between actual editor state and history state.

---

### 8. `commit` should take the pre-change document, not a whole state object
**Decision:** `commit` should take `&Rope` for the pre-change document, not a whole `State` wrapper.

**Reasoning:**
History only needs the original document contents to compute the inversion. It does not need arbitrary session state.

Implication:
- remove public `State`
- keep the API narrow and honest

---

### 9. Each revision stores only `selection_after`
**Decision:** a revision should store the landed selection snapshot, not both before/after selections.

**Reasoning:**
Undo/redo/jumps always land on some revision. The only selection needed is the one that belongs to that landed state.

Preferred revision shape:

```rust
struct Revision {
  parent:          usize,
  last_child:      Option<NonZeroUsize>,
  transaction:     Transaction,
  inversion:       Transaction,
  timestamp:       Instant,
  selection_after: Selection,
}
```

---

### 10. Root should remain a normal revision record
**Decision:** keep root as a normal stored revision with empty transactions.

**Reasoning:**
This avoids `Option<Transaction>` and other special-case plumbing everywhere.

Preferred root initialization:

```rust
Revision {
  parent: 0,
  last_child: None,
  transaction: Transaction::empty(initial_len),
  inversion: Transaction::empty(initial_len),
  timestamp,
  selection_after: initial_selection,
}
```

This is much cleaner than the old fake `ChangeSet::new(...)` approach.

---

### 11. Branching history should remain
**Decision:** keep the revision graph model.

**Reasoning:**
The current module already has a real branching structure, and that structure is useful:
- undo to an older revision
- make a new edit
- keep old branches reachable
- keep simple redo deterministic
- support graph-aware `changes_since`

Do not collapse this into a linear stack.

---

### 12. Redo should follow `last_child`
**Decision:** plain redo and branch-local forward stepping should follow only the revision’s preferred `last_child`.

**Reasoning:**
This is deterministic and avoids dragging branch-choice UI concerns into core history.

Semantics:
- `undo()` moves to `parent`
- `redo()` moves to `last_child`
- committing from an older revision updates that revision’s `last_child`
- older sibling branches still exist for graph-aware operations like `jump_to` and `changes_since`

---

### 13. Jump application should remain a two-phase protocol
**Decision:** keep “prepare jump” separate from “confirm jump.”

**Reasoning:**
`History` should not own real document mutation orchestration. It should produce a jump plan; the caller applies it; then history is updated.

Preferred flow:
1. `history.undo()` / `redo()` / `jump_*()` returns a `HistoryJump`
2. caller applies all jump transactions externally
3. caller installs `jump.selection`
4. caller calls `history.apply_jump(&jump)`

This is explicit and hard to misuse.

---

### 14. `HistoryJump` should carry source, target, transactions, and selection
**Decision:** prepared jumps should contain the exact destination selection and the source revision they were prepared from.

**Reasoning:**
This makes jump application deterministic and enables stale-jump protection.

Preferred shape:

```rust
pub struct HistoryJump {
  pub source:       RevisionId,
  pub target:       RevisionId,
  pub transactions: Vec<Transaction>,
  pub selection:    Selection,
}
```

---

### 15. `apply_jump` should reject stale jumps
**Decision:** `apply_jump` should verify that the jump was prepared from the current revision.

**Reasoning:**
Without this check, a caller could prepare a jump, mutate history in some other way, and then apply a stale jump that no longer matches reality.

Required validation:
- `jump.source == self.current_revision()`
- `jump.target` is a valid revision

---

### 16. `changes_since` stays document-only and graph-aware
**Decision:** keep `changes_since(revision) -> Result<Option<Transaction>>`, but make it strictly document-only.

**Reasoning:**
This method is still useful, but it should not pretend to carry selection semantics. The graph-aware LCA behavior is worth keeping.

Semantics:
- return the document delta from `revision` to the current revision
- work even if `revision` is on another branch
- do not attach selection or hidden editor-state payloads

---

### 17. Time-based history navigation stays
**Decision:** keep `UndoKind::{Steps, TimePeriod}` and the timestamp-based navigation APIs.

**Reasoning:**
The feature is real, the current implementation is small enough, and the timestamp field is already justified by it.

Keep it simple:
- linear nearest-timestamp search is fine for now
- no indexing/caching/optimization yet

---

### 18. Length and selection invariants should be enforced explicitly
**Decision:** history should validate obvious misuse at construction and commit time.

**Reasoning:**
This module is backbone infrastructure. Cheap sanity checks are worth it.

At minimum:
- `History::new(initial_len, initial_selection)` validates that selection fits `initial_len`
- `commit(before, tx, selection_after)` validates:
  - `before.len_chars() == tx.changes().len()`
  - `before.len_chars() == current_len()`
  - `selection_after` fits `tx.changes().len_after()`

This will not catch wrong-content-same-length misuse, but it catches a lot of obvious mistakes cheaply.

---

### 19. Keep small read APIs
**Decision:** history should expose small, concrete accessors instead of exposing revision internals wholesale.

Preferred read APIs:

```rust
pub fn current_revision(&self) -> RevisionId;
pub fn current_selection(&self) -> &Selection;
pub fn selection(&self, revision: RevisionId) -> Result<&Selection>;
pub fn current_len(&self) -> usize;
pub fn revision_len(&self, revision: RevisionId) -> Result<usize>;
pub fn current_timestamp(&self) -> Instant;
pub fn revision_timestamp(&self, revision: RevisionId) -> Result<Instant>;
```

**Reasoning:**
These are enough for tests and higher layers without opening the whole representation.

---

### 20. Remove `State`
**Decision:** remove the old `State { doc, selection }` type from the public history API.

**Reasoning:**
It no longer reflects the real module boundary. History should not pretend to own/apply arbitrary editor state.

---

### 21. Remove `last_edit_pos()` for now
**Decision:** do not preserve the old `last_edit_pos()` API in the first clean rewrite.

**Reasoning:**
Its old implementation depended on transaction-attached selection and tuple-shaped change iteration. Its semantics are fuzzy enough that it should only come back if there is a real caller and a clearly defined contract.

---

## Ideal public interface

This is the target public-facing shape for `history.rs`.
The exact implementation can vary, but the interface should feel close to this.

```rust
use std::{
  num::NonZeroUsize,
  time::{Duration, Instant},
};

use ropey::Rope;
use thiserror::Error;

use crate::{
  selection::Selection,
  transaction::{Transaction, TransactionError},
};

pub type Result<T> = std::result::Result<T, HistoryError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RevisionId(usize);

impl RevisionId {
  pub fn new(index: usize) -> Self;
  pub fn get(self) -> usize;
}

#[derive(Debug, Error)]
pub enum HistoryError {
  #[error(transparent)]
  Transaction(#[from] TransactionError),

  #[error("revision {revision:?} is out of bounds (max: {max:?})")]
  RevisionOutOfBounds { revision: RevisionId, max: RevisionId },

  #[error("stale history jump: expected source {expected:?}, got {got:?}")]
  StaleJump { expected: RevisionId, got: RevisionId },

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
  pub fn is_empty(&self) -> bool;
  pub fn len(&self) -> usize;
}

#[derive(Debug)]
pub struct History {
  revisions: Vec<Revision>,
  current:   usize,
}

#[derive(Debug, Clone)]
struct Revision {
  parent:          usize,
  last_child:      Option<NonZeroUsize>,
  transaction:     Transaction,
  inversion:       Transaction,
  timestamp:       Instant,
  selection_after: Selection,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum UndoKind {
  Steps(usize),
  TimePeriod(Duration),
}

impl History {
  pub fn new(initial_len: usize, initial_selection: Selection) -> Result<Self>;
  pub fn new_at_timestamp(
    initial_len: usize,
    initial_selection: Selection,
    timestamp: Instant,
  ) -> Result<Self>;

  pub fn current_revision(&self) -> RevisionId;
  pub fn at_root(&self) -> bool;
  pub fn len(&self) -> usize;
  pub fn is_empty(&self) -> bool;

  pub fn current_selection(&self) -> &Selection;
  pub fn selection(&self, revision: RevisionId) -> Result<&Selection>;

  pub fn current_len(&self) -> usize;
  pub fn revision_len(&self, revision: RevisionId) -> Result<usize>;

  pub fn current_timestamp(&self) -> Instant;
  pub fn revision_timestamp(&self, revision: RevisionId) -> Result<Instant>;

  pub fn commit(
    &mut self,
    before: &Rope,
    transaction: Transaction,
    selection_after: Selection,
  ) -> Result<RevisionId>;

  pub fn changes_since(&self, revision: RevisionId) -> Result<Option<Transaction>>;

  pub fn undo(&self) -> Option<HistoryJump>;
  pub fn redo(&self) -> Option<HistoryJump>;
  pub fn jump_to(&self, revision: RevisionId) -> Result<HistoryJump>;
  pub fn jump_backward(&self, steps: usize) -> Result<HistoryJump>;
  pub fn jump_forward(&self, steps: usize) -> Result<HistoryJump>;
  pub fn jump_duration_backward(&self, duration: Duration) -> Result<HistoryJump>;
  pub fn jump_duration_forward(&self, duration: Duration) -> Result<HistoryJump>;
  pub fn earlier(&self, kind: UndoKind) -> Result<HistoryJump>;
  pub fn later(&self, kind: UndoKind) -> Result<HistoryJump>;

  pub fn apply_jump(&mut self, jump: &HistoryJump) -> Result<()>;
}
```

---

## Core invariants

These invariants should be easy to state and easy to defend in tests.

### Revision graph invariants
- `revisions` is never empty
- `current` is always a valid revision index
- root revision exists at index `0`
- root revision has `parent == 0`
- every non-root revision’s `parent` is a valid earlier revision
- `last_child`, when present, points to a valid revision

### Transaction/length invariants
- every revision stores a forward `transaction` from its parent to itself
- every revision stores a matching `inversion` from itself back to its parent
- root forward/inverse transactions are both `Transaction::empty(initial_len)`
- a revision’s landed document length is well-defined
- `commit` only accepts transactions whose input length matches both:
  - the supplied `before` document length
  - the current revision length

### Selection invariants
- every revision stores exactly one landed `selection_after`
- every stored selection fits the landed document length of that revision
- `HistoryJump.selection` always equals the target revision’s `selection_after`

### Jump invariants
- prepared jumps are pure plans; they do not mutate history
- `apply_jump` only succeeds if the jump was prepared from the current revision
- undo moves to `parent`
- redo moves to `last_child`

---

## Behavior semantics

### Undo / redo
- `undo()` returns `None` at root
- otherwise returns a jump containing the current revision’s inversion and the parent selection snapshot
- `redo()` follows `last_child` only

### Branch-local stepping
- `jump_backward(n)` follows `parent` links
- `jump_forward(n)` follows `last_child` links
- this is branch-local navigation, not vector-index navigation

### Jump to revision
- `jump_to(revision)` may require:
  - walking inversions up to the lowest common ancestor
  - then forward transactions down to the target
- returned transactions are ordered for application to the current document state

### Time-based navigation
- `earlier(TimePeriod(d))` searches for the revision nearest `current_time - d`
- `later(TimePeriod(d))` searches for the revision nearest `current_time + d`
- tie-breaking should prefer:
  - earlier/backward revisions for `earlier(...)`
  - later/forward revisions for `later(...)`

### `changes_since`
- returns the document delta from the given revision to the current revision
- may cross branches using lowest-common-ancestor composition
- returns `None` only when the requested revision already equals current
- does not carry selection semantics

---

## What should be removed from the old pasted implementation

The old pasted `history.rs` is carrying assumptions from the old API. The clean rewrite should remove them directly instead of preserving them for compatibility.

### Remove transaction-attached selection assumptions
Delete logic that depends on:
- `transaction.selection()`
- `tx.with_selection(...)`
- storing selection on inversions or forward transactions

### Remove `State`
Do not keep a public `State { doc, selection }` wrapper as the central history API.

### Remove fake default root initialization
Replace old root setup like:

```rust
Transaction::from(ChangeSet::new("".into()))
```

with honest empty transactions built from the real initial length.

### Remove `last_edit_pos()`
Do not carry it forward unless a concrete caller justifies it and its semantics are specified clearly.

### Remove commit-before-apply expectations
Tests and docs should assume commit happens **after** successful application.

---

## Refactoring guidance for the implementation

When refactoring `the-lib/history.rs`, prefer these principles:

- keep the data model small
- make revision state explicit
- validate obvious misuse early
- do not re-embed selection into transactions
- keep navigation code graph-aware but straightforward
- prefer concrete helpers over clever abstraction

Reasonable private helpers include things like:
- revision-id/index conversion
- selection-bounds validation against a document length
- landed length lookup for a revision
- jump construction from source/target
- lowest common ancestor and path walking

But avoid introducing speculative abstraction layers unless they become unmistakably useful.

---

## Testing priorities

When the module is rewritten, tests should focus on behavior and invariants, not legacy internal details.

High-value tests include:
- explicit root initialization and root selection restoration
- commit validates input length and selection bounds
- commit returns new `RevisionId`
- undo/redo restore both text and exact selection snapshot
- stale jumps are rejected
- redo follows `last_child`
- branch-local backward/forward stepping
- graph-aware `changes_since`
- time-based navigation and tie-breaking
- current/revision length accessors
- current/revision selection accessors

The goal is the same as for `transaction.rs` and `selection.rs`:
- fewer, stronger, behavior-focused tests
- clear invariants
- high confidence per line
