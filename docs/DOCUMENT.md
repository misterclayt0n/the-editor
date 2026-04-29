# Document Module Architecture

This document defines the target architecture for `the-lib/document.rs`.

It answers four questions:

1. What is a `Document` in the new editor?
2. What belongs on `Document`, and what does not?
3. How should `Document` relate to syntax, surfaces, and runtime/source state?
4. What should be kept or removed from the old pasted implementation?

This document is intentionally opinionated. `Document` should be a small pure text-state object, not a mini editor instance.

---

## Purpose

`Document` is the pure owner of semantic text state.

It is responsible for:
- owning the text buffer
- owning a monotonic text revision
- owning document-local syntax state
- owning narrow document-local text policy/metadata like line ending and indent style
- applying pure text `Transaction`s
- keeping document-owned derived state coherent when text changes

It is **not** responsible for:
- selection
- active cursor
- visual goals
- history/undo session behavior
- viewport/scroll state
- render caches
- display name
- readonly policy
- modified/saved state
- file path / URI / resource identity
- runtime resource loading

---

## Core position

`Document` is **text truth plus document-owned derived text state**.

It is not a view.
It is not a pane.
It is not a session.
It is not a save target.

That means the new architecture should read like this:

```txt
Resource  = external-world identity
Document  = text truth
Surface   = per-view/editor-session truth
```

A single document may be shown by many surfaces.
So anything that differs across those views must not live on `Document`.

---

## What belongs on `Document`

### 1. Text
```rust
text: Rope
```
This is the canonical document contents.

### 2. Monotonic revision
```rust
revision: u64
```
Increment when the text actually changes.

### 3. Document-owned syntax state
```rust
syntax: DocumentSyntax
```
Syntax is derived from text and belongs with document truth.

But this should use the **new syntax architecture**, not the old loader attachment model.

### 4. Document-local text policy / metadata
```rust
line_ending: LineEnding
indent_style: IndentStyle
```
These are allowed as narrow document-local properties.

- `line_ending` is the document's newline policy/default
- `indent_style` is the document's current indent heuristic/style

These are still much more document-ish than view/session-ish.

---

## What does **not** belong on `Document`

### Selection
Selection is per-surface.
Two split panes showing the same document must not be forced to share one selection.

### History
Under the current architecture, history restores exact surface/session-local selection snapshots.
That makes history a surface/session concern, not pure document truth.

### Active cursor / visual goals
These are per-view interaction state.

### Display name / readonly / modified / path
These are runtime/source-level concepts, not resource-agnostic document truth.

### `DocumentId`
For now, identity should stay outside `the-lib/document.rs`.
`Document` is just the state object.

---

## Relationship to syntax

`Document` should own `DocumentSyntax` using the new syntax model:
- `DocumentSyntax`
- `DocumentSyntaxState`
- `SyntaxState`
- `LanguageAssignment`

It should **not** own:
- runtime loaders
- runtime-backed resource handles
- file/path-based detection policy

### Applying transactions
When text changes and syntax is active, `Document` should keep syntax coherent automatically.

For the pure core, the minimal correct behavior is:
- interpolate syntax through the text edits
- mark parse-pending if a better parse is needed later

This keeps `Document` pure while still preserving syntax coherence.

Background reparsing / registry-driven activation can happen above this layer.

### Post-v1 syntax evolution
The complete syntax architecture makes this stronger and more explicit.

`Document` should own syntax truth in the form of:
- a persistent syntax layer graph
- explicit invalidated regions
- parse/injection/enrichment generations
- snapshot/apply protocols for runtime-driven advancement

That means `Document` should **not** own:
- visible highlight caches
- row-local render syntax spans
- per-surface extracted syntax payloads

And `Document::apply_transaction(...)` should be understood as:
- cheap text mutation
- syntax interpolation / invalidation update
- no unbounded full reparse on the typing path

---

## Relationship to `Surface`

A future surface/session object should own things like:
- selection
- active cursor
- visual goals
- history
- viewport/scroll
- render surface state

That means `Document` should stay smaller than the old pasted implementation.

---

## Relationship to runtime/source state

Runtime/source state should own things like:
- file path / URI
- display name
- readonly policy
- modified/saved state
- source-document attachment

Those concepts are real, but they are not pure document truth.

---

## Main decisions

### 1. `Document` is small and pure
**Decision:** yes.

It should own text truth and document-owned derived state only.

---

### 2. `Document` is resource-agnostic
**Decision:** yes.

No display name, path, readonly, modified, or save-state concepts.

---

### 3. `Document` owns syntax
**Decision:** yes.

Syntax is derived text truth and belongs on the document.

But it must use the modern syntax model, not old runtime loader attachments.

---

### 4. Keep line ending and indent style
**Decision:** yes.

These are acceptable narrow document-local text properties.

---

### 5. `Document` exposes direct mutating methods
**Decision:** yes.

Preferred style:
- mutate in place
- apply pure transactions
- bump revision
- update owned derived state

---

### 6. `Document::apply_transaction(...)` updates syntax automatically
**Decision:** yes.

If active syntax exists, text changes should automatically keep document-owned syntax coherent.

---

### 7. No `DocumentId` in `the-lib/document.rs` for now
**Decision:** yes.

Keep `Document` as plain state. Higher layers can own registry identity later.

---

## Ideal public interface

The exact implementation can vary, but the public API should feel close to this:

```rust
use ropey::Rope;

use crate::{
  indent::IndentStyle,
  line_ending::LineEnding,
  syntax::DocumentSyntax,
  transaction::Transaction,
};

pub struct Document {
  text: Rope,
  revision: u64,
  syntax: DocumentSyntax,
  line_ending: LineEnding,
  indent_style: IndentStyle,
}

pub fn new(text: Rope) -> Self;

pub fn text(&self) -> &Rope;
pub fn revision(&self) -> u64;

pub fn syntax(&self) -> &DocumentSyntax;
pub fn set_syntax(&mut self, syntax: DocumentSyntax);
pub fn clear_syntax(&mut self);

pub fn line_ending(&self) -> LineEnding;
pub fn set_line_ending(&mut self, line_ending: LineEnding);

pub fn indent_style(&self) -> IndentStyle;
pub fn set_indent_style(&mut self, indent_style: IndentStyle);

pub fn apply_transaction(&mut self, transaction: &Transaction) -> Result<()>;
```

Optional convenience methods are fine if they stay honest, for example:
- `refresh_text_metadata()`
- `set_text(...)` only if it does not blur the transaction model

---

## What should happen on `apply_transaction(...)`

When applying a text transaction:

1. apply the transaction to `text`
2. if nothing changed, do not bump revision
3. if text changed:
   - bump `revision`
   - if syntax is active, interpolate syntax through the edits
   - mark parse-pending for later reparsing if appropriate

It should **not**:
- map a selection
- commit history
- enforce readonly
- update modified/saved flags
- manage resource identity
- rebuild whole-document syntax highlight payloads
- perform scheduler/policy decisions that belong in `the-runtime`

---

## What to remove from the old pasted implementation

Remove from `Document` entirely:
- `DocumentId`
- `ViewId`
- `display_name`
- `selection`
- `history`
- `changes`
- `old_state`
- `flags.readonly`
- `flags.modified`
- loader-attached syntax attachment struct
- undo/redo/jump orchestration methods
- save-state methods like `mark_saved`

Those concerns should live elsewhere.

---

## Test priorities

Tests should focus on behavior, not implementation details.

Priority coverage:
- creation initializes revision and text metadata sanely
- applying a non-empty transaction changes text and bumps revision
- applying an empty transaction does not bump revision
- active syntax is interpolated when text changes
- line ending / indent style setters work
- syntax can be assigned/cleared without runtime coupling

---

## Non-goals

`document.rs` should **not** become:
- a session object
- a view object
- a save-state object
- a resource object
- an undo manager
- a giant convenience shell around the whole editor

Keep it small, pure, and text-centric.
