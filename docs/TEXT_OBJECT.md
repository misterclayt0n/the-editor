# Text Object Module Architecture

This document defines the target architecture for `the-lib/text_object.rs`.

It answers four questions:

1. What is a text object in the new editor?
2. What should the public API feel like?
3. What belongs in `text_object.rs`, and what does not?
4. What should be kept or removed from the old pasted implementation?

This document is intentionally opinionated. `text_object.rs` should stay small, pure, and selection-centric publicly.

---

## Purpose

`text_object.rs` defines pure semantic selection transforms for text objects like:
- word
- paragraph
- pair surround
- closest enclosing pair
- query-backed named syntax objects

It is responsible for:
- mapping a `Selection` to text-object selections
- preserving cursor identity across one-to-one transforms
- preserving direction for semantic selection reshapes
- using syntax/query data for syntax-backed named objects
- returning ordinary normalized `Selection`s

It is **not** responsible for:
- command parsing like `iw`, `ap`, `af`, etc.
- movement semantics
- structural syntax navigation like parent/child/sibling expansion
- surround editing commands
- syntax lifecycle state
- runtime/UI/history integration

---

## Core position

`text_object.rs` is a **pure selection-transform module**.

It inspects:
- text
- a `Selection`
- a text-object kind like inside/around
- object-specific inputs like a pair character or syntax object name
- optional pure helpers like active `Syntax` and `LanguageRegistry`

and returns:
- a new `Selection`

This keeps it aligned with the rest of the architecture:

```txt
Selection:   directional cursor/range state + cursor IDs
Syntax:      document-owned parse/query truth
Surround:    pair-boundary detection
Object:      structural syntax navigation
TextObject:  semantic selection transforms over text/syntax/pairs
```

---

## Relationship to `selection.rs`

Text-object behavior should be **selection-centric publicly**.

That means the public API should take and return `Selection`, not make higher layers manually loop over ranges.

Internally, the implementation may still use small private `Range -> Range` helpers.

Important behavior:
- one input cursor produces one output range
- original `CursorId`s are preserved
- if no object is found for a cursor, that cursor is a no-op
- final results use normal `Selection` normalization

---

## Relationship to `object.rs`

`object.rs` and `text_object.rs` should stay separate.

### `object.rs`
Owns structural syntax navigation like:
- expand to containing node
- shrink to child node
- next/prev sibling
- all children / all siblings

### `text_object.rs`
Owns semantic object selection like:
- inside word
- around paragraph
- inside pair
- around pair
- query-backed named objects like `function.inside`

### Why split them
Structural tree navigation and semantic text-object selection are related, but not the same abstraction.

---

## Relationship to `surround.rs`

Pair-based text objects should stay in `text_object.rs`.

`surround.rs` should only provide the lower-level surround-pair detection primitive.

That means:
- `surround.rs` finds `(open, close)`
- `text_object.rs` turns that into inside/around pair selection behavior

---

## Relationship to `syntax.rs`

Syntax-backed named text objects should consume:
- active `&Syntax`
- `&LanguageRegistry`

They should **not** consume:
- `DocumentSyntax`
- deferred/unavailable syntax state
- runtime loaders/resources

The syntax-backed rule should remain:
- look up the root language's `TextObjectQuery`
- choose the smallest containing capture
- no-op if no matching capture exists

---

## Non-goals

`text_object.rs` should **not** become:
- a movement module
- a structural object-navigation module
- a command grammar layer
- a registry of product-level command names
- a generic syntax-query dumping ground

Keep it focused on semantic text-object selection.

---

## Main decisions

### 1. Public API is selection-centric
**Decision:** yes.

**Reasoning:**
The editor's public cursor model is `Selection`, so text-object behavior should operate at that level.

---

### 2. Remove `Movement` from the text-object kind enum
**Decision:** yes.

Preferred shape:

```rust
pub enum TextObjectKind {
  Around,
  Inside,
}
```

**Reasoning:**
`Movement` is not a text-object selection kind. It belongs elsewhere.

---

### 3. Keep small named helpers instead of a giant dispatcher
**Decision:** yes.

Preferred style:

```rust
pub fn word(...)
pub fn paragraph(...)
pub fn pair(...)
pub fn closest_pair(...)
pub fn syntax_named(...)
```

**Reasoning:**
Small named helpers are easier to read, test, and evolve.

---

### 4. Pair-based text objects stay here, on top of `surround.rs`
**Decision:** yes.

**Reasoning:**
`surround.rs` should stay a lower-level detection primitive. `text_object.rs` owns the semantic inside/around pair selection behavior.

---

### 5. Syntax-backed named objects use `LanguageRegistry`
**Decision:** yes.

**Reasoning:**
The old `Loader` boundary is gone. `LanguageRegistry` is now the pure registry/query façade.

---

### 6. Smallest containing capture wins
**Decision:** yes.

**Reasoning:**
For syntax-backed named objects, the smallest containing capture is usually what the user means.
If no capture matches, leave that cursor unchanged.

---

### 7. Keep count only where its semantics are already clear
**Decision:** yes.

Keep count for:
- word
- paragraph
- pair
- closest pair

Do **not** define count semantics for syntax-backed named objects yet.

**Reasoning:**
Count is obvious for those object families, but still underspecified for syntax-named objects.

---

## Ideal public interface

The exact implementation can vary, but the public API should feel close to this:

```rust
use ropey::RopeSlice;

use crate::{
  selection::Selection,
  syntax::{LanguageRegistry, Syntax, SyntaxError},
};

pub enum TextObjectKind {
  Around,
  Inside,
}

pub fn word(
  text: RopeSlice,
  selection: Selection,
  kind: TextObjectKind,
  count: usize,
  long: bool,
) -> Selection;

pub fn paragraph(
  text: RopeSlice,
  selection: Selection,
  kind: TextObjectKind,
  count: usize,
) -> Selection;

pub fn pair(
  syntax: Option<&Syntax>,
  text: RopeSlice,
  selection: Selection,
  kind: TextObjectKind,
  ch: char,
  count: usize,
) -> Selection;

pub fn closest_pair(
  syntax: Option<&Syntax>,
  text: RopeSlice,
  selection: Selection,
  kind: TextObjectKind,
  count: usize,
) -> Selection;

pub fn syntax_named(
  syntax: &Syntax,
  registry: &LanguageRegistry,
  text: RopeSlice,
  selection: Selection,
  kind: TextObjectKind,
  object_name: &str,
) -> Result<Selection, SyntaxError>;
```

---

## What should be kept from the old implementation

### Keep
- word object behavior
- paragraph object behavior
- pair-based object behavior
- query-backed syntax object behavior
- no-op fallback when no matching object exists
- direction preservation for semantic text-object reshapes

### Remove
- `Movement` from the text-object kind enum
- old range-only public API shape
- old `Loader` dependency
- command compatibility baggage

### Keep with refactor
- private `Range -> Range` helpers
- paragraph helpers
- pair wrapping helpers built on `surround.rs`

---

## Testing priorities

The most important tests should prove:

1. **word objects**
   - inside vs around
   - whitespace behavior
   - direction and cursor-ID preservation

2. **paragraph objects**
   - inside vs around
   - blank-line handling
   - count behavior

3. **pair objects**
   - inside vs around
   - explicit pair vs closest pair
   - no-op when no pair exists

4. **syntax-backed named objects**
   - smallest containing capture wins
   - no-op when no matching capture exists
   - cursor IDs are preserved

Prefer a few strong behavioral tests over large compatibility tables.

---

## Summary

`text_object.rs` should be a small pure module for semantic selection transforms.

Keep it:
- selection-centric publicly
- small in named helpers
- pair-based on top of `surround.rs`
- query-backed for syntax-defined objects
- free of movement and command baggage

It should stay focused on semantic object selection and stop there.
