# Transaction Module Architecture

This document defines the target architecture for `the-lib/transaction.rs`.

It answers four questions:

1. What is a transaction in the new editor?
2. What should the public API feel like?
3. What belongs in `transaction.rs`, and what does not?
4. What should be refactored out of the current ported implementation?

This document is intentionally opinionated. `transaction.rs` is one of the backbone modules of the editor. It should be small, obvious, composable, and hard to misuse.

---

## Purpose

A transaction is a **pure document delta**.

It describes how text changes.

It does **not** describe:
- selection changes
- surface/view updates
- command behavior
- app intent
- UI state

That means a transaction should answer:

> Given a document of length `N`, what text spans are replaced, and what does that lower to as a canonical native changeset?

---

## Core position

`Transaction` is the main public editing object.
`ChangeSet` is the lower-level canonical operation machine.

```txt
public ergonomic API:   Transaction + Replacement + Span
lower-level machinery:  ChangeSet + Operation + mapping/inversion/compose
```

This is the desired split.

### Why

Most callers should think in:
- replace this span
- delete these spans
- insert this text here
- compose these document edits

They should not have to think in:
- retain 4
- insert X
- delete 3
- retain 12

The op-stream form is still the right canonical representation for:
- composition
- inversion
- position mapping
- application to rope text

But it is not the primary ergonomic surface.

---

## Non-goals

`transaction.rs` should **not** become:
- a selection workflow module
- a command helper dump
- a view/surface update mechanism
- a high-level editing behavior facade
- a place for comment/surround/auto-pair/syntax-specific logic

Those higher-level features should produce transactions. They should not be absorbed into transaction definitions.

---

## Main decisions

### 1. A transaction is document-only
**Decision:** `Transaction` must not carry `Selection`.

**Reasoning:**
- selection belongs to `Surface`, not `Document`
- many surfaces may share one document
- embedding selection into transaction blurs document and view responsibilities
- undo/redo history should describe text evolution, not arbitrary per-surface view state

Implication:
- remove `selection: Option<Selection>` from `Transaction`
- selection mapping should happen outside the transaction object itself

---

### 2. The public edit primitive should be named and explicit
**Decision:** replace tuple `Change = (usize, usize, Option<Tendril>)` with a named struct.

**Reasoning:**
Tuple aliases are too opaque for such a fundamental API.

This is clearer:

```rust
Replacement::delete(span)
Replacement::insert(at, "hello")
Replacement::replace(span, "world")
```

than this:

```rust
(4, 8, None)
(10, 10, Some("hello".into()))
```

---

### 3. Text edits should use `Span`, not selection `Range`
**Decision:** use:

```rust
pub type Span = the_stdx::range::Range<usize>;
```

**Reasoning:**
A selection `Range` means:
- `anchor`
- `head`
- directional selection semantics

A text edit span means:
- `start`
- `end`
- normalized document extent

These are different concepts and should not share the same primary type.

### Distinction
- `Range` = directional selection primitive
- `Span` = normalized text extent used for mutation/query

This distinction should be documented prominently in both modules.

---

### 4. `Transaction` should be the main public API
**Decision:** callers should mostly construct and compose `Transaction`s, not handcraft `ChangeSet`s.

**Reasoning:**
This makes transactions the common language for:
- editing primitives
- undo/redo
- runtime-applied edits
- LSP/code action/formatter edits
- TypeScript-triggered native edits

`ChangeSet` remains public and important, but lower-level.

---

### 5. Main transaction constructors should be length-based
**Decision:** the primary constructors should require document length, not a full `&Rope`, unless text content is truly needed.

**Reasoning:**
Most transaction construction only needs:
- valid spans
- document char length

It does not need the rope itself.

This keeps the API:
- simpler
- less coupled
- easier to test
- more future-proof for runtime/FFI use

Text-dependent operations like inversion still naturally require the original rope.

---

### 6. Overlap behavior should be explicit
**Decision:** overlap handling must be explicit in construction APIs.

**Reasoning:**
Silent overlap behavior hides bugs.
This is backbone infrastructure.
The default should be strict.

Default stance:
- overlapping replacements are an error unless the caller explicitly opts into another behavior

---

### 7. Generic replacement overlap should not pretend to support universal merge semantics
**Decision:** `Merge` should not be part of the generic replacement API.

**Reasoning:**
Merging overlapping deletions is well-defined.
Merging overlapping arbitrary replacements often is not.

So:
- generic replacement APIs should support `Reject` and `IgnoreLater`
- deletion/span-specific APIs may support `Merge`

This keeps the API honest.

---

### 8. `ChangeSet` remains builder + value in one type
**Decision:** no separate `ChangeSetBuilder`.

**Reasoning:**
The current combined model is simple and obvious.
A separate builder type does not add enough value right now.

---

### 9. Position mapping belongs in the transaction layer
**Decision:** mapping logic stays conceptually inside the transaction/changeset system.

**Reasoning:**
A complete native document delta should own:
- apply
- compose
- invert
- map positions through the delta

That said, internal refactors for clarity are welcome.

---

### 10. Delete tracking stays, but the API should become clearer
**Decision:** keep delete tracking, but likely refactor the current large `Assoc` enum into a more policy-like model later.

**Reasoning:**
Delete tracking is valuable and core-worthy.
But the current enum mixes too many dimensions:
- side association
- stickiness
- word behavior
- deletion tracking mode

The behavior should remain. The API surface should become easier to reason about.

---

## Ideal public interface

This is the target public-facing shape for `transaction.rs`.
The exact implementation can vary, but the interface should feel close to this.

```rust
use ropey::Rope;

use crate::Tendril;

pub type Span = the_stdx::range::Range<usize>;

pub type Result<T> = std::result::Result<T, TransactionError>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Replacement {
  pub span: Span,
  pub text: Option<Tendril>,
}

impl Replacement {
  pub fn insert(at: usize, text: impl Into<Tendril>) -> Self;
  pub fn delete(span: Span) -> Self;
  pub fn replace(span: Span, text: impl Into<Tendril>) -> Self;

  pub fn is_insert(&self) -> bool;
  pub fn is_delete(&self) -> bool;
  pub fn is_replace(&self) -> bool;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplacementOverlapPolicy {
  Reject,
  IgnoreLater,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeletionOverlapPolicy {
  Reject,
  IgnoreLater,
  Merge,
}

#[derive(Debug, Error, PartialEq, Eq)]
#[non_exhaustive]
pub enum TransactionError {
  #[error("document length mismatch: expected {expected}, got {actual}")]
  LengthMismatch { expected: usize, actual: usize },

  #[error("invalid span: start {start} is after end {end}")]
  InvalidSpan { start: usize, end: usize },

  #[error("span {start}..{end} is out of bounds for document length {len}")]
  SpanOutOfBounds { start: usize, end: usize, len: usize },

  #[error("replacement span {start}..{end} overlaps previous end {prev_end}")]
  OverlappingReplacement {
    prev_end: usize,
    start:    usize,
    end:      usize,
  },

  #[error(
    "changeset compose length mismatch: left output {left_len_after}, right input {right_len}"
  )]
  ComposeLengthMismatch {
    left_len_after: usize,
    right_len:      usize,
  },

  #[error("positions {positions:?} are out of bounds for changeset length {len}")]
  PositionsOutOfBounds {
    positions: Vec<usize>,
    len:       usize,
  },

  #[error(
    "delete-tracking mapping mode cannot be used with batch update_positions; use \
     map_pos_tracked instead"
  )]
  TrackingAssocInBatchUpdate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Operation {
  Retain(usize),
  Delete(usize),
  Insert(Tendril),
}

impl Operation {
  pub fn len_chars(&self) -> usize;
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Assoc {
  Before,
  After,
  BeforeSticky,
  AfterSticky,
  BeforeWord,
  AfterWord,

  TrackDel,
  TrackDelSticky,
  TrackDelBefore,
  TrackDelAfter,
  TrackDelBeforeSticky,
  TrackDelAfterSticky,
}

impl Assoc {
  pub fn sticky(self) -> bool;
  pub fn tracks_deletion(self) -> bool;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MappedPos {
  Pos(usize),
  Deleted {
    pos:    usize,
    offset: usize,
    len:    usize,
  },
}

impl MappedPos {
  pub fn position(&self) -> usize;
  pub fn is_deleted(&self) -> bool;
  pub fn unwrap(self) -> usize;
  pub fn ok(self) -> Option<usize>;
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ChangeSet {
  // canonical op stream
}

impl ChangeSet {
  pub fn with_capacity(capacity: usize) -> Self;
  pub fn new_len(len: usize) -> Self;

  pub fn changes(&self) -> &[Operation];
  pub fn len(&self) -> usize;
  pub fn len_after(&self) -> usize;
  pub fn is_empty(&self) -> bool;

  pub fn retain(&mut self, n: usize);
  pub fn delete(&mut self, n: usize);
  pub fn insert(&mut self, fragment: Tendril);

  pub fn compose(self, other: Self) -> Result<Self>;
  pub fn invert(&self, original_doc: &Rope) -> Result<Self>;
  pub fn apply(&self, text: &mut Rope) -> Result<()>;
  pub fn apply_to(&self, text: &Rope) -> Result<Rope>;

  pub fn update_positions<'a>(
    &self,
    positions: impl Iterator<Item = (&'a mut usize, Assoc)>,
  ) -> Result<()>;
  pub fn map_pos(&self, pos: usize, assoc: Assoc) -> Result<usize>;
  pub fn map_pos_tracked(&self, pos: usize, assoc: Assoc) -> Result<MappedPos>;
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Transaction {
  // pure document delta only
}

impl Transaction {
  pub fn empty(len: usize) -> Self;

  pub fn from_replacements(
    len: usize,
    replacements: impl IntoIterator<Item = Replacement>,
    policy: ReplacementOverlapPolicy,
  ) -> Result<Self>;

  pub fn replace(len: usize, span: Span, text: impl Into<Tendril>) -> Result<Self>;
  pub fn insert_at(len: usize, at: usize, text: impl Into<Tendril>) -> Result<Self>;
  pub fn delete(len: usize, span: Span) -> Result<Self>;

  pub fn delete_spans(
    len: usize,
    spans: impl IntoIterator<Item = Span>,
    policy: DeletionOverlapPolicy,
  ) -> Result<Self>;

  pub fn changes(&self) -> &ChangeSet;
  pub fn apply(&self, doc: &mut Rope) -> Result<()>;
  pub fn apply_to(&self, doc: &Rope) -> Result<Rope>;
  pub fn invert(&self, original: &Rope) -> Result<Self>;
  pub fn compose(self, other: Self) -> Result<Self>;
}
```

---

## Interface notes

### `Span` is a type alias, not a newtype
This is intentional.

Reasoning:
- simpler API
- less ceremony
- sufficient for now

If confusion persists later, this can be revisited. It is not needed at the start.

### `Replacement` is transparent data with helper constructors
Reasoning:
- good call-site readability
- still easy to pattern-match or construct directly

### `Transaction::empty(len)`
This should replace rope-based constructors like `Transaction::new(&Rope)` for the common pure case.

Reasoning:
- transaction construction usually only needs length
- `empty(len)` is explicit and simple

### `ChangeSet::new_len(len)`
The lower-level type should also gain a length-based constructor.

Reasoning:
- same purity/coupling reasons as above
- avoids needless `RopeSlice` dependency in simple build paths

---

## Things that should move out of `Transaction`

These patterns are useful, but they are not transaction essence:
- selection-derived replacement generation
- selection overlap resolution with selection reconstruction
- insert-at-each-cursor convenience

Examples from the current port that should not stay on `Transaction` long-term:
- `change_by_selection`
- `change_by_selection_ignore_overlapping`
- `delete_by_selection`
- `insert(doc, selection, text)`
- `with_selection`
- `selection()`

### Why
These functions mix:
- selection semantics
- cursor identity handling
- overlap policy
- transaction construction

That is one layer above pure document-delta definition.

The exact destination does not need to be settled yet. What matters is that `Transaction` itself stays crisp.

---

## Things that should stay in `transaction.rs`

These are fundamental:
- `Span`
- `Replacement`
- canonical operation representation
- application to text
- composition
- inversion
- position mapping
- deletion tracking machinery
- overlap-aware lowering from replacements/spans into canonical changesets

Reasoning:
These are core document-delta semantics, not feature-specific helpers.

---

## Current implementation notes

The current port has several good foundations:
- `ChangeSet` as canonical op stream is good
- mapping logic is correctly colocated with changeset semantics
- inversion and composition belong here
- delete tracking is worth keeping

The main problems to address are API-shape problems, not conceptual ownership problems.

### Main refactor targets
- remove selection from `Transaction`
- replace tuple `Change` with `Replacement`
- replace rope-based constructors with length-based ones where possible
- make overlap behavior explicit in the main constructors
- reduce the convenience-method sprawl on `Transaction`
- eventually simplify/clarify `Assoc`

---

## Decision tree, resolved

### 1. Should transaction carry selection?
**Decision:** no.

### 2. Should the user-facing edit primitive be a tuple alias?
**Decision:** no; use `Replacement`.

### 3. Should text edits use selection `Range`?
**Decision:** no; use `Span`.

### 4. Should `Transaction` or `ChangeSet` be the main public edit API?
**Decision:** `Transaction`.

### 5. Should transaction constructors require `&Rope` by default?
**Decision:** no; use document length where possible.

### 6. Should overlap behavior be implicit?
**Decision:** no; make it explicit.

### 7. Should generic overlap policy include universal `Merge`?
**Decision:** no.

### 8. Should `ChangeSet` split into builder and value types?
**Decision:** no.

### 9. Should mapping stay in the transaction layer?
**Decision:** yes.

### 10. Should delete tracking stay?
**Decision:** yes, but with a clearer eventual API.

### 11. Should `Transaction` have a wide convenience-heavy API?
**Decision:** no; keep it small and opinionated.

---

## Summary

The target mental model is:

```txt
Span         = normalized text extent
Replacement  = replace this span with this optional text
Transaction  = public pure document delta
ChangeSet    = canonical low-level op stream
```

And the most important rule is:

> `Transaction` describes text changes only.
> It must not become a carrier for selection, surface state, or higher-level editing behavior.
