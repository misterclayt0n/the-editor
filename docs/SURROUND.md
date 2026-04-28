# Surround Module Architecture

This document defines the target architecture for `the-lib/surround.rs`.

It answers four questions:

1. What is `surround.rs` in the new editor?
2. What should the public API feel like?
3. What belongs in `surround.rs`, and what does not?
4. What should be kept or removed from the old pasted implementation?

This document is intentionally opinionated. `surround.rs` should stay small, pure, and narrowly scoped.

---

## Purpose

`surround.rs` defines pure surround-pair detection helpers.

It is responsible for:
- finding the positions of an explicit surround pair character around a range
- finding the nearest enclosing surround pair around a range
- doing that across a `Selection`
- normalizing the detected pair positions
- rejecting ambiguous or overlapping multi-cursor results

It is **not** responsible for:
- adding surrounds
- deleting surrounds
- changing surrounds
- producing `Edit`
- owning selection transforms directly
- syntax lifecycle state like deferred/unavailable syntax
- command parsing or modal behavior

---

## Core position

`surround.rs` is a **pure surround-boundary detection module**.

It inspects:
- document text
- one or more ranges
- an optional active `&Syntax`
- either an explicit surround character or a request for nearest enclosing pair detection

and returns:
- normalized surround pair positions

This means the module answers questions like:

> What pair surrounds this range?

or:

> What are the positions of the nearest enclosing surround pair for each selection?

It does not decide what to do with those positions.
Higher layers may later use them for:
- surround editing
- pair-based text objects
- motions or selections

---

## Relationship to `match_brackets.rs`

`match_brackets.rs` and `surround.rs` are related, but they should stay separate.

### `match_brackets.rs`
Owns:
- matching one bracket to its counterpart
- strict under-cursor bracket matching
- fuzzy enclosing-scope closing-bracket lookup

### `surround.rs`
Owns:
- locating full surround pair boundaries around ranges
- explicit pair lookup by chosen character
- nearest enclosing pair detection for later higher-level consumers

### Why split them
`match_brackets.rs` is a bracket lookup primitive.
`surround.rs` is a pair-boundary detection primitive for higher-level surround semantics.

They are adjacent, but not the same abstraction.

---

## Relationship to `selection.rs`

`surround.rs` may accept a `Selection` for batch detection, but it should not become selection-centric behavior.

It should not return `Selection`.
It should not compute resulting cursor landing.
It should not own editing semantics.

The important output is simply:
- surround pair positions

### Why
This module is still a detection primitive, not an editing or selection-shaping module.

---

## Relationship to `syntax.rs`

`surround.rs` should remain **syntax-optional**.

That means:
- if active `Syntax` is available, use it to improve nearest-enclosing-pair detection
- if syntax is unavailable, use plaintext logic

It should consume:
- `Option<&Syntax>`
- `RopeSlice`
- ranges/selections

It should **not** consume:
- `DocumentSyntax`
- syntax lifecycle state
- runtime loaders/resources

### Why
This keeps the module pure, reusable, and aligned with the same syntax-optional boundary used elsewhere.

---

## Non-goals

`surround.rs` should **not** become:
- a full surround editing module
- a text-object module
- a command layer for `ys`/`cs`/`ds`-style behavior
- a syntax lifecycle wrapper
- a place for selection/result `Edit` planning

Keep it focused on detecting pair boundaries.

---

## Main decisions

### 1. Keep `surround.rs` detection-only for now
**Decision:** yes.

**Reasoning:**
The current honest responsibility is locating surround pair bounds.
That is a reusable primitive worth preserving.
Full surround editing can be built later on top if needed.

---

### 2. Return normalized ordered pair positions
**Decision:** yes.

Preferred output shape:

```rust
pub type SurroundPair = (usize, usize); // always (open, close), open <= close
```

**Reasoning:**
This module should return positional truth, not directional selection semantics.
Higher layers can preserve or reapply direction if they need it.

---

### 3. Keep syntax optional
**Decision:** yes.

**Reasoning:**
Nearest enclosing pair detection benefits from syntax when available, but the module should remain usable without syntax.
Explicit pair search for a chosen character remains text-driven.

---

### 4. Keep multi-selection behavior all-or-nothing
**Decision:** yes.

That means:
- if any cursor/range fails to resolve a pair, return an error
- if any detected pair positions overlap across cursors, return an error
- do not partially succeed

**Reasoning:**
This keeps the primitive deterministic and easy for higher layers to reason about.

---

### 5. Keep explicit-pair and nearest-pair detection separate
**Decision:** yes.

Preferred public shape:

```rust
pub fn find_nth_pairs_pos(...)
pub fn find_nth_closest_pairs_pos(...)
pub fn get_surround_pos(...)
```

**Reasoning:**
These are distinct lookup modes and should remain explicit in the API.

---

### 6. Keep ambiguity explicit for symmetric pairs
**Decision:** yes.

If a symmetric pair character is ambiguous at the cursor position, return an error rather than guessing.

**Reasoning:**
This is the safer and more honest behavior for a foundational primitive.

---

## Ideal public interface

The exact implementation can vary, but the public API should feel close to this:

```rust
use smallvec::SmallVec;
use ropey::RopeSlice;

use crate::{
  selection::{Range, Selection},
  syntax::Syntax,
};

pub type SurroundPair = (usize, usize); // normalized (open, close)

pub fn find_nth_pairs_pos(
  text: RopeSlice,
  ch: char,
  range: Range,
  n: usize,
) -> Result<SurroundPair>;

pub fn find_nth_closest_pairs_pos(
  syntax: Option<&Syntax>,
  text: RopeSlice,
  range: Range,
  skip: usize,
) -> Result<SurroundPair>;

pub fn get_surround_pos(
  syntax: Option<&Syntax>,
  text: RopeSlice,
  selection: &Selection,
  ch: Option<char>,
  skip: usize,
) -> Result<SmallVec<[SurroundPair; 1]>>;
```

### Notes
- normalized positions out
- no `Selection` result
- no `Edit`
- syntax optional only where it helps

---

## What should be kept from the old implementation

### Keep
- explicit pair lookup by chosen char
- nearest enclosing pair lookup
- syntax-optional closest-pair behavior
- overlap rejection across multi-selection
- explicit ambiguity errors for symmetric pairs

### Keep with cleanup
- nested pair stepping logic
- skip/count behavior
- helper logic for matching open/close positions

### Remove or avoid
- dependence on now-private `match_brackets` internals
- old `Selection` iteration assumptions
- any drift toward full surround editing responsibility

---

## Testing priorities

The most important tests should prove:

1. **explicit pair lookup**
   - finds expected pair bounds
   - preserves skip/count semantics
   - errors on ambiguous symmetric pair cursor cases

2. **closest pair lookup**
   - finds nearest enclosing pair
   - works with and without syntax

3. **multi-selection behavior**
   - all selections succeed together
   - overlap across detected pairs is rejected
   - partial success does not occur

4. **normalized output**
   - results are always `(open, close)` with `open <= close`

Prefer a few strong behavioral tests over lots of tiny low-level tests.

---

## Summary

`surround.rs` should stay a small pure surround-boundary detection module.

Keep it:
- detection-only
- normalized in output
- syntax-optional
- all-or-nothing across multi-selection
- separate from full surround editing and text-object behavior

It should locate pair bounds cleanly and stop there.
