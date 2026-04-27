# Selection Module Architecture

This document defines the target architecture for `the-lib/selection.rs`.

It answers four questions:

1. What is the role of selection in the new editor?
2. What should the `Range` and `Selection` APIs look like?
3. Which invariants belong here?
4. What currently belongs in `selection.rs`, and what may move later?

This module is part of the native editor backbone. It should be expressive, predictable, and ergonomic under heavy use.

---

## Purpose

`selection.rs` defines the native model for directional selections and cursor sets.

It is responsible for:
- representing directional ranges
- representing non-empty selections
- carrying stable cursor identities
- normalizing multi-range selections
- mapping selections through document changes
- preserving cursor identity across transforms where possible

It is **not** responsible for:
- text mutation representation itself
- command behavior
- product-specific editing workflows
- runtime service state
- app UI state

---

## Core position

A selection belongs to a `Surface`, not a `Document`.

This matters because:
- many surfaces may share one document
- each surface may have its own selections, active cursor, and viewport
- document truth and view truth must stay separate

This document only covers the `selection.rs` module itself, but that architectural split is the reason selection must be designed as a strong independent primitive.

---

## Relationship to `transaction.rs`

The selection model and transaction model are close, but distinct.

### `Range`
A `Range` means:
- `anchor`
- `head`
- directional selection semantics

### `Span`
A `Span` means:
- `start`
- `end`
- normalized text extent
- no directional meaning

### Distinction
- `Range` = directional user-selection primitive
- `Span` = normalized text mutation/query extent

This distinction must be obvious in docs and API naming.

---

## Main decisions

### 1. Keep `Range { anchor, head }`
**Decision:** yes.

**Reasoning:**
Selection direction is fundamental editor state.
It affects:
- extend/shrink semantics
- cursor placement
- sticky mapping
- block-cursor behavior
- many editing operations

Normalizing everything to `start/end` would just force direction to be tracked elsewhere.

---

### 2. Keep `Range` as a lightweight public-field value type
**Decision:** yes.

```rust
pub struct Range {
  pub anchor: usize,
  pub head:   usize,
}
```

**Reasoning:**
A lone range is a flexible value object.
Strong collection invariants belong at the `Selection` level, not on every temporary `Range` value.

This keeps the type:
- cheap
- obvious
- easy to work with in hot-path code

---

### 3. `Selection` is normalized by default
**Decision:** yes.

A normal `Selection` should remain:
- non-empty
- sorted
- non-overlapping
- merged where appropriate

**Reasoning:**
Most code wants strong invariants and easy reasoning.
That should remain the default.

---

### 4. Advanced paths may delay normalization
**Decision:** yes.

There should be a clear lower-level path for:
- bulk construction
- bulk transforms
- map-first-normalize-later flows

**Reasoning:**
Some core algorithms want to do a lot of work and normalize once.
That is a real need.
But it should not replace the default semantics.

---

### 5. Cursor IDs are intrinsic, not optional metadata
**Decision:** yes.

Every selection range should have a stable `CursorId`.

**Reasoning:**
This supports:
- active cursor tracking
- multi-cursor editing
- cursor-specific view state
- TypeScript/provider references
- robust identity across transforms and mapping

Optional cursor IDs would either complicate the API or force a higher layer to reinvent identity mapping.

---

### 6. Public API should become cursor-entry-centric
**Decision:** yes.

Even if storage remains parallel arrays internally, the public API should increasingly present a selection as:
- a collection of cursor entries
- each with `id` and `range`

**Reasoning:**
This is the real conceptual model.
The current parallel-array API leaks implementation details.
A cursor-entry-centric API is easier to understand and use.

---

### 7. Selection transforms should become cursor-entry-centric too
**Decision:** yes.

Transforms should operate on `CursorId + Range` together rather than pretending range and identity are independent.

**Reasoning:**
This makes cursor-ID preservation, splitting, reassignment, and collapse behavior much clearer.

---

### 8. Selection mapping through changes belongs here
**Decision:** yes.

`selection.rs` should own how directional ranges and selections map through a `ChangeSet`.

**Reasoning:**
That is part of selection semantics, not a higher-level convenience layer.
The transaction layer owns the change mapping primitive; selection owns how selection semantics use it.

---

## Ideal public interface

This is the target public-facing shape for `selection.rs`.
The exact implementation can vary, but the interface should feel close to this.

```rust
use std::num::NonZeroU64;

use ropey::RopeSlice;
use smallvec::SmallVec;
use tree_house::tree_sitter::Node;

use crate::{
  movement::Direction,
  transaction::{Assoc, ChangeSet},
};

pub type Result<T> = std::result::Result<T, SelectionError>;

#[derive(Debug, Error, PartialEq, Eq)]
#[non_exhaustive]
pub enum SelectionError {
  #[error("selection must contain at least one cursor range")]
  EmptySelection,

  #[error("range index {index} out of bounds for selection of length {len}")]
  RangeIndexOutOfBounds { index: usize, len: usize },

  #[error("cursor id count {ids} does not match range count {ranges}")]
  CursorIdCountMismatch { ids: usize, ranges: usize },

  #[error("cursor id {id} appears more than once in selection")]
  DuplicateCursorId { id: u64 },

  #[error("cursor id {id} not found in selection")]
  CursorIdNotFound { id: u64 },

  #[error("cannot remove the last range from a selection")]
  RemoveLastRange,

  #[error("selection transform produced no ranges")]
  NoRanges,

  #[error(transparent)]
  Transaction(#[from] TransactionError),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CursorId(NonZeroU64);

impl CursorId {
  pub fn new(id: NonZeroU64) -> Self;
  pub fn fresh() -> Self;
  pub fn get(self) -> u64;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Range {
  pub anchor: usize,
  pub head:   usize,
}

impl Range {
  pub fn new(anchor: usize, head: usize) -> Self;
  pub fn point(head: usize) -> Self;
  pub fn from_node(node: Node, text: RopeSlice, direction: Direction) -> Self;

  // span-like view
  pub fn from(&self) -> usize;
  pub fn to(&self) -> usize;
  pub fn len(&self) -> usize;
  pub fn is_empty(&self) -> bool;
  pub fn contains(&self, pos: usize) -> bool;
  pub fn contains_range(&self, other: &Self) -> bool;
  pub fn overlaps(&self, other: &Self) -> bool;

  // directional view
  pub fn direction(&self) -> Direction;
  pub fn flip(&self) -> Self;
  pub fn with_direction(self, direction: Direction) -> Self;

  // cursor/display helpers
  pub fn cursor(self, text: RopeSlice) -> usize;
  pub fn cursor_line(&self, slice: RopeSlice) -> usize;
  pub fn put_cursor(self, slice: RopeSlice, char_idx: usize, extend: bool) -> Range;

  // text/view helpers
  pub fn line_range(&self, slice: RopeSlice) -> (usize, usize);
  pub fn into_byte_range(self, slice: &RopeSlice) -> (usize, usize);
  pub fn slice<'a, 'b: 'a>(&'a self, text: RopeSlice<'b>) -> RopeSlice<'b>;
  pub fn fragment<'a, 'b: 'a>(&'a self, text: RopeSlice<'b>) -> std::borrow::Cow<'b, str>;

  // normalization-ish helpers
  pub fn grapheme_aligned(&self, slice: RopeSlice) -> Self;
  pub fn min_width_1(&self, slice: RopeSlice) -> Self;

  // transformation/mapping
  pub fn extend(&self, from: usize, to: usize) -> Self;
  pub fn merge(&self, other: Self) -> Self;
  pub fn map(self, changes: &ChangeSet) -> Result<Self>;
  pub fn map_tracked(self, changes: &ChangeSet) -> Result<MappedRange>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CursorRange {
  pub id:    CursorId,
  pub range: Range,
}

#[derive(Debug, Clone, Copy)]
pub struct CursorRangeRef<'a> {
  pub id:    CursorId,
  pub range: &'a Range,
}

#[derive(Debug, Clone, Copy)]
pub enum CursorPick {
  First,
  Last,
  Index(usize),
  Id(CursorId),
  Nearest(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeleteInfo {
  pub offset: usize,
  pub len:    usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MappedRange {
  pub range: Range,
  pub anchor_deleted: Option<DeleteInfo>,
  pub head_deleted:   Option<DeleteInfo>,
}

impl MappedRange {
  pub fn was_deleted(&self) -> bool;
  pub fn fully_deleted(&self) -> bool;
  pub fn into_range(self) -> Range;
}

#[derive(Debug, Clone)]
pub struct MappedSelection {
  pub selection:     Selection,
  pub mapped_ranges: SmallVec<[MappedRange; 1]>,
}

impl MappedSelection {
  pub fn any_deleted(&self) -> bool;
  pub fn all_fully_deleted(&self) -> bool;
  pub fn into_selection(self) -> Selection;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selection {
  // normalized non-empty set of cursor ranges
}

impl Selection {
  // default constructors: normalized, fresh ids when not provided
  pub fn new(ranges: SmallVec<[Range; 1]>) -> Result<Self>;
  pub fn from_cursors(cursors: SmallVec<[CursorRange; 1]>) -> Result<Self>;
  pub fn point(pos: usize) -> Self;
  pub fn single(anchor: usize, head: usize) -> Self;

  // lower-level/internal staging path may exist, but should not define the public mental model

  // primary cursor-entry-centric accessors
  pub fn iter(&self) -> impl Iterator<Item = CursorRangeRef<'_>>;
  pub fn get(&self, idx: usize) -> Result<CursorRangeRef<'_>>;
  pub fn by_id(&self, id: CursorId) -> Option<CursorRangeRef<'_>>;
  pub fn pick(&self, pick: CursorPick) -> Result<CursorRange>;

  // lower-level escape hatches; useful, but not the conceptual center
  pub fn ranges(&self) -> &[Range];
  pub fn cursor_ids(&self) -> &[CursorId];

  // collection properties
  pub fn len(&self) -> usize;
  pub fn is_empty(&self) -> bool;
  pub fn contains(&self, other: &Selection) -> bool;
  pub fn into_single(self) -> Self;

  // collection updates
  pub fn push(self, range: Range) -> Self;
  pub fn push_with_id(self, range: Range, cursor_id: CursorId) -> Result<Self>;
  pub fn replace(self, idx: usize, range: Range) -> Result<Self>;
  pub fn remove(self, idx: usize) -> Result<Self>;
  pub fn collapse(self, pick: CursorPick) -> Result<Self>;

  // change mapping
  pub fn map(self, changes: &ChangeSet) -> Result<Self>;
  pub fn map_no_normalize(self, changes: &ChangeSet) -> Result<Self>;
  pub fn map_tracked(self, changes: &ChangeSet) -> Result<MappedSelection>;

  // selection transforms
  pub fn transform(self, f: impl FnMut(Range) -> Range) -> Self;
  pub fn transform_entries(self, f: impl FnMut(CursorRange) -> CursorRange) -> Result<Self>;
  pub fn flat_map_entries<I>(
    self,
    f: impl FnMut(CursorRange) -> I,
  ) -> Result<Self>
  where
    I: Iterator<Item = CursorRange>;

  // normalization/invariant helpers
  pub fn ensure_invariants(self, slice: RopeSlice) -> Self;
  pub fn merge_ranges(self) -> Self;
  pub fn merge_consecutive_ranges(self) -> Self;
  pub fn cursors(self, slice: RopeSlice) -> Self;

  // text helpers
  pub fn fragments<'a>(
    &'a self,
    slice: RopeSlice<'a>,
  ) -> impl DoubleEndedIterator<Item = std::borrow::Cow<'a, str>> + ExactSizeIterator;

  pub fn slices<'a>(
    &'a self,
    slice: RopeSlice<'a>,
  ) -> impl DoubleEndedIterator<Item = RopeSlice<'a>> + ExactSizeIterator + 'a;

  pub fn line_ranges<'a>(&'a self, slice: RopeSlice<'a>) -> LineRangeIter<'a>;
}
```

---

## Interface notes

### Cursor-entry-centric API is the target mental model
The current implementation stores:
- `ranges`
- `cursor_ids`

as parallel arrays.

That may remain the internal representation for performance reasons.
But the public API should increasingly behave as if the selection is a list of:

```rust
CursorRange {
  id: CursorId,
  range: Range,
}
```

### Why this matters
This is clearer than split access like:
- `range_at(idx)`
- `cursor_id_at(idx)`
- `iter_with_ids()`

The public model should answer:
- “what cursor entry is this?”
not
- “what do I get if I look up both arrays separately?”

---

## `Range` API philosophy

`Range` should expose two views explicitly.

### Directional view
This is about user/editor semantics:
- `anchor`
- `head`
- `direction()`
- `flip()`
- `with_direction()`
- `cursor()`
- `put_cursor()`

### Span-like view
This is about covered extent:
- `from()`
- `to()`
- `len()`
- `contains()`
- `contains_range()`
- `overlaps()`

### Why this is good
It makes the selection model obvious without throwing away direction.

---

## Normalization rules

A normal `Selection` should preserve these invariants:

1. it is never empty
2. ranges are sorted by position
3. overlapping ranges are merged
4. cursor IDs remain unique
5. cursor IDs remain attached to meaningful surviving ranges where possible

Additional text-sensitive invariants may be enforced by explicit helpers like:
- `ensure_invariants(slice)`
- `grapheme_aligned(slice)`
- `min_width_1(slice)`

### Why not force every `Range` to uphold these?
Because a standalone `Range` is often an intermediate value.
The collection type is where strong invariants matter most.

---

## Mapping through changes

This is one of the most important responsibilities of `selection.rs`.

### `Range::map`
Maps a directional range through a changeset using the standard association rules.

### `Range::map_tracked`
Maps a directional range through a changeset while preserving deletion information.

### `Selection::map`
Maps a whole selection and normalizes afterward.

### `Selection::map_no_normalize`
Advanced path for algorithms that want to normalize once at the end.

### `Selection::map_tracked`
Maps a whole selection while preserving delete-tracking information.

### Why this belongs here
The transaction layer owns the low-level change map.
The selection layer owns what that means for directional ranges and stable cursor identities.

---

## Cursor ID preservation semantics

Cursor IDs should remain stable where possible.

Examples:
- simple mapping through changes should preserve the same IDs
- collapsing to one cursor should preserve the chosen cursor ID
- splitting one cursor into many ranges should keep the original ID on the most semantically natural survivor and assign fresh IDs to the rest
- merging overlapping ranges should choose one surviving ID by a consistent rule

### Why this matters
This supports:
- surface-level active cursor tracking
- stable references from higher layers
- predictable multi-cursor semantics

---

## Transform API guidance

### Keep range-only transforms
A simple helper like:

```rust
pub fn transform(self, f: impl FnMut(Range) -> Range) -> Self;
```

is still useful.

### But prefer entry-centric transforms for richer operations
The module should increasingly prefer APIs where `id` and `range` are transformed together.

Examples:
- `transform_entries`
- `flat_map_entries`

### Why
Cursor identity is not incidental metadata. It is part of the real selection semantics.

---

## What belongs in `selection.rs`

These absolutely belong here:
- `Range`
- `CursorId`
- `Selection`
- cursor-entry types
- normalization
- mapping through `ChangeSet`
- delete-tracked selection mapping
- grapheme-aware selection helpers
- cursor-placement helpers

Reasoning:
These are core selection semantics.

---

## What may move later

The current module also includes some regex-driven selection helpers, such as:
- filtering selection by regex match
- selecting regex matches within selections
- splitting selections on matches

These are useful, but they are less obviously part of the pure selection model.
They may eventually belong in a more query/search/selection-utils area.

### Important note
This is not urgent.
The main architectural work is around selection identity, normalization, and mapping.

---

## Current implementation notes

The current port already has a lot going for it:
- the `Range` model is right
- mapping logic belongs here
- delete-tracked mapping is valuable
- normalized-by-default semantics are right
- stable cursor IDs are the right choice

The main improvement area is the public API shape.

### Main refactor targets
- make the public API more cursor-entry-centric
- stop exposing the parallel-array model as the conceptual center
- keep normalized default semantics
- preserve an advanced path for bulk/non-normalized workflows
- keep `Range` lightweight and directional

---

## Decision tree, resolved

### 1. Should `Range` stay directional (`anchor/head`)?
**Decision:** yes.

### 2. Should `Range` fields stay public?
**Decision:** yes.

### 3. Should `Selection` remain normalized by default?
**Decision:** yes.

### 4. Should there also be a delayed-normalization path for advanced internals?
**Decision:** yes.

### 5. Should cursor IDs be intrinsic to selection?
**Decision:** yes.

### 6. Should the public API move toward cursor-entry-centric access?
**Decision:** yes.

### 7. Should transforms also become cursor-entry-centric?
**Decision:** yes.

### 8. Should mapping through `ChangeSet` stay in `selection.rs`?
**Decision:** yes.

### 9. Should `Range` and transaction `Span` be the same type?
**Decision:** no.

### 10. Should regex-heavy selection utilities define the core module shape?
**Decision:** no.

---

## Summary

The target mental model is:

```txt
Range         = directional selection primitive
CursorId      = stable cursor identity
CursorRange   = id + directional range
Selection     = normalized non-empty set of cursor ranges
Mapped*       = selection-aware mapping-through-changes results
```

And the most important rule is:

> `Selection` is not just a list of ranges.
> It is a list of stable cursor identities paired with directional ranges.

That should be the shape the public API communicates clearly.
