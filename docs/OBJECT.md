# Object Module Architecture

This document defines the target architecture for `the-lib/object.rs`.

It answers four questions:

1. What is `object.rs` in the new editor?
2. How should it relate to `selection.rs` and `syntax.rs`?
3. What should the public API feel like?
4. What should be kept or removed from the old pasted implementation?

This document is intentionally opinionated. `object.rs` should stay very small, pure, and structural. It is not the future text-object module.

---

## Purpose

`object.rs` defines pure structural selection transforms over syntax trees.

It is responsible for:
- expanding a selection to a containing syntax node
- shrinking a selection to a contained named child when that is well-defined
- moving to previous/next named siblings
- selecting all named children of a node
- selecting all named siblings of a node
- preserving cursor identity where possible
- returning ordinary normalized `Selection`s

It is **not** responsible for:
- word / paragraph / pair text objects
- query-backed named text objects like `function.inside`
- command parsing or modal behavior
- syntax lifecycle state like deferred/unavailable syntax
- movement system abstractions
- runtime/UI/history integration

---

## Core position

`object.rs` is a **pure structural syntax-selection module**.

It inspects:
- an active `&Syntax`
- document text
- a `Selection`

and returns:
- a new `Selection`

It does not apply edits.
It does not own syntax availability policy.
It does not parse command intent.

That keeps it aligned with the rest of the architecture:

```txt
Selection:  directional cursor/range state + cursor IDs
Syntax:     document-owned parse/query truth
Object:     structural syntax selection/navigation
TextObject: later semantic object-selection module
```

---

## Relationship to `selection.rs`

`object.rs` should be selection-centric publicly.

That means the public API should take and return `Selection`, not force higher layers to loop over ranges manually.

Important behavior:
- one-to-one operations preserve the original `CursorId`
- fan-out operations preserve the first `CursorId` and assign fresh IDs to additional outputs
- per-cursor failure is a no-op for that cursor
- final results use ordinary `Selection` normalization

`object.rs` should not invent a second non-normalized selection mode.

---

## Relationship to `syntax.rs`

`object.rs` should operate on **already-active syntax only**.

It should consume:
- `&Syntax`
- `RopeSlice`
- `Selection`

It should **not** consume:
- `DocumentSyntax`
- deferred/unavailable syntax states
- runtime resource loaders

If syntax is unavailable, the caller should decide not to invoke the behavior.

---

## Relationship to the future `text_object.rs`

This split should remain explicit.

### `object.rs`
Owns structural tree behaviors like:
- expand to containing node
- shrink to child node
- move to sibling
- select all children / siblings

### `text_object.rs`
Will own semantic target selection like:
- word
- paragraph
- inside pair
- around pair
- query-backed named objects like `function.inside`

### Why split them
These are related, but not the same abstraction.

`object.rs` is about syntax tree shape.
`text_object.rs` is about user-meaningful semantic targets.

If they are merged, the boundary gets vague quickly.

---

## Non-goals

`object.rs` should **not** become:
- a generic text-object dispatcher
- a query-backed object-selection system
- a mode/keymap command layer
- a syntax lifecycle wrapper
- a feature dump for every selection transform touching syntax

Keep it narrow.

---

## Main decisions

### 1. Keep `object.rs` structural-only
**Decision:** yes.

**Reasoning:**
This module should stay about parent/child/sibling tree structure, not semantic text-object targeting.

---

### 2. Keep the module pure and free-function-based
**Decision:** yes.

Preferred shape:

```rust
pub fn expand_selection(...)
pub fn shrink_selection(...)
pub fn select_next_sibling(...)
pub fn select_prev_sibling(...)
pub fn select_all_children(...)
pub fn select_all_siblings(...)
```

**Reasoning:**
The module is tiny, stateless, and does not justify an `ObjectNavigator` or similar abstraction.

---

### 3. Use named nodes as the structural target
**Decision:** yes.

**Reasoning:**
Structural results should normally be at the named-node level, because that is the level users actually mean.

That means:
- child selection uses named children
- sibling navigation uses named siblings
- fan-out operations emit named children

---

### 4. Allow unnamed-node lookup fallback
**Decision:** yes.

**Reasoning:**
Users may place the cursor on punctuation or another unnamed token.
Operations should still be able to find a useful starting node.

So lookup should:
- first try named descendant lookup
- then fall back to generic descendant lookup

But the structural targets/results should still prefer named nodes.

---

### 5. Per-cursor no-op on failure
**Decision:** yes.

**Reasoning:**
If one cursor has no meaningful structural move, that cursor should remain unchanged.
The whole operation should not fail.

Examples:
- no next sibling
- no child to shrink into
- no named children

---

### 6. Fan-out keeps the first cursor ID
**Decision:** yes.

For:
- `select_all_children`
- `select_all_siblings`

use this rule:
- first emitted range keeps the original `CursorId`
- additional emitted ranges get fresh `CursorId`s

**Reasoning:**
This preserves identity where there is an obvious continuation while still maintaining the invariant that cursor IDs are unique.

---

### 7. Preserve direction except for explicit directional navigation
**Decision:** yes.

Use this rule:
- `expand_selection` preserves original direction
- `shrink_selection` preserves original direction
- `select_all_children` preserves original direction
- `select_all_siblings` preserves original direction
- `select_next_sibling` returns a forward-facing range
- `select_prev_sibling` returns a backward-facing range

**Reasoning:**
Expand/shrink/fan-out are structural reshapes.
Next/prev sibling are directional navigation.
Those are meaningfully different.

---

### 8. Normal `Selection` normalization applies
**Decision:** yes.

**Reasoning:**
If multiple outputs overlap or collapse together, normal `Selection` behavior should win.
`object.rs` should not create a special exception to selection invariants.

---

## Ideal public interface

The exact implementation can vary, but the public API should feel close to this:

```rust
use ropey::RopeSlice;

use crate::{
  selection::Selection,
  syntax::Syntax,
};

pub fn expand_selection(
  syntax: &Syntax,
  text: RopeSlice,
  selection: Selection,
) -> Selection;

pub fn shrink_selection(
  syntax: &Syntax,
  text: RopeSlice,
  selection: Selection,
) -> Selection;

pub fn select_next_sibling(
  syntax: &Syntax,
  text: RopeSlice,
  selection: Selection,
) -> Selection;

pub fn select_prev_sibling(
  syntax: &Syntax,
  text: RopeSlice,
  selection: Selection,
) -> Selection;

pub fn select_all_children(
  syntax: &Syntax,
  text: RopeSlice,
  selection: Selection,
) -> Selection;

pub fn select_all_siblings(
  syntax: &Syntax,
  text: RopeSlice,
  selection: Selection,
) -> Selection;
```

Notes:
- active `Syntax` only
- `Selection` in, `Selection` out
- no command parsing layer
- no query-backed semantic text objects here

---

## What should be removed from the old implementation

### Keep
- structural expand/shrink behavior
- named sibling navigation
- all-children / all-siblings fan-out
- per-cursor no-op fallback
- unnamed lookup fallback with named structural targets

### Remove
- any implication that this module is the general text-object system
- old API assumptions from the pre-refactor `Selection` model
- command-layer or mode-layer baggage

### Defer
- semantic query-backed text objects
- word / paragraph / pair object selection
- object-name vocabularies like `function` / `class` / `parameter`

Those belong in the later `text_object.rs` module.

---

## Testing priorities

The most important tests should prove:

1. **expand behavior**
   - exact node expands to parent
   - original direction is preserved

2. **sibling navigation**
   - skips unnamed siblings
   - result direction follows navigation direction
   - no-op when no sibling exists

3. **fan-out behavior**
   - children/siblings emit the expected structural ranges
   - first cursor ID is preserved
   - additional cursor IDs are fresh

4. **ancestor climbing for sibling fan-out**
   - if the immediate parent has only one named child, climb until a useful sibling set exists

5. **selection semantics**
   - normal normalization still applies
   - leaf-node fan-out is a no-op

Prefer a few strong behavioral tests over many shallow tree-shape snapshots.

---

## Summary

`object.rs` should stay a tiny pure module for structural syntax selection/navigation.

Keep it:
- structural-only
- active-syntax-only
- selection-centric publicly
- free-function-based
- named-target-oriented with unnamed lookup fallback
- simple about cursor IDs and direction semantics

It is not the text-object system. It is the small structural companion to syntax and selection.
