# Comment Module Architecture

This document defines the target architecture for `the-lib/comment.rs`.

It answers four questions:

1. What is comment toggling in the new editor?
2. What should the public API feel like?
3. What belongs in `comment.rs`, and what does not?
4. What should be kept or removed from the old pasted implementation?

This document is intentionally opinionated. `comment.rs` should stay small, pure, token-driven, and narrowly scoped.

---

## Purpose

`comment.rs` defines pure commenting behavior for selections.

It is responsible for:
- toggling line comments over selected lines
- toggling block comments around selected ranges
- computing the document `Transaction` for those changes
- computing the resulting `Selection`
- using explicit provided comment tokens
- preserving ordinary selection invariants and cursor identity where possible

It is **not** responsible for:
- discovering language config on its own
- syntax lifecycle state like deferred/unavailable syntax
- command parsing or modal behavior
- comment-aware keymaps or UI workflows
- surround editing
- generic formatting or indentation policy beyond what is inherent to comment toggling
- runtime/history/UI integration

---

## Core position

`comment.rs` is a **pure edit-planning module**.

It inspects:
- document text
- a `Selection`
- explicit line or block comment tokens

and returns:
- an `Edit`

Preferred shared result shape:

```rust
pub struct Edit {
  pub transaction: Transaction,
  pub selection:   Selection,
}
```

That means `comment.rs` should fit the same core pattern as other pure editing behavior modules:

```txt
Transaction: pure document delta
Selection:   directional cursor/range state
Edit:        transaction + exact resulting selection
Comment:     pure comment toggle planner producing Edit
```

---

## Relationship to `transaction.rs`, `selection.rs`, and `edit.rs`

### `Transaction`
A transaction means:
- how text changes
- document-only delta semantics

Comment toggling should produce a `Transaction`, but should never hide selection inside it.

### `Selection`
A selection means:
- directional ranges
- stable cursor identity
- surface-local cursor state

Comment toggling must compute the resulting `Selection` explicitly.

### `Edit`
An `Edit` means:
- one document `Transaction`
- one exact resulting `Selection`

This is the right return type for comment toggles because both line and block comment behavior affect text and selection together.

---

## Relationship to `syntax.rs`

`comment.rs` should be **token-driven, not syntax-owning**.

It should consume explicit inputs like:
- `Option<&str>` for line comments
- `&[BlockCommentToken]` for block comments

It should **not**:
- inspect `DocumentSyntax`
- query `LanguageRegistry`
- resolve language assignment on its own

Higher layers may obtain tokens from syntax config, but `comment.rs` should only consume the already-resolved pure data it needs.

### Why
This keeps the module:
- pure
- easy to test
- usable even without active syntax
- decoupled from syntax lifecycle and resource concerns

---

## Non-goals

`comment.rs` should **not** become:
- a syntax config lookup module
- a command layer for comment actions
- a generic whitespace formatting engine
- a text-object or surround module
- a place for mode-specific or product-specific comment behavior

Keep it focused on toggling comments.

---

## Main decisions

### 1. Return `Edit`, not `Transaction`
**Decision:** yes.

Preferred shape:

```rust
pub fn toggle_line_comments(...) -> Result<Edit>
pub fn toggle_block_comments(...) -> Result<Edit>
```

**Reasoning:**
Commenting behavior affects both:
- document text
- resulting selection

That is a real editing result, not just a document delta.

---

### 2. Keep the module pure and token-driven
**Decision:** yes.

**Reasoning:**
The module should operate from explicit comment tokens passed in by the caller.
It should not own syntax lookup or language detection.

---

### 3. Keep line and block commenting in one module
**Decision:** yes.

**Reasoning:**
They are one coherent feature domain:
- same conceptual behavior
- same input family
- same output type
- same caller expectation

Splitting them now would add surface area without enough benefit.

---

### 4. Keep the public API tiny
**Decision:** yes.

Preferred public surface:

```rust
pub fn toggle_line_comments(...)
pub fn toggle_block_comments(...)
```

Helper analysis like:
- token finding
- block comment analysis
- selection splitting helpers
- internal change structs

should remain private or `pub(crate)` unless another module clearly needs them.

**Reasoning:**
The real module boundary is comment toggling behavior, not its internal analysis steps.

---

### 5. Line comments use ordinary mapped-selection behavior
**Decision:** yes.

**Reasoning:**
Line comment toggling does not need special semantic cursor landing.
It should return:
- the planned transaction
- the input selection mapped through that transaction

This is simple, honest, and predictable.

---

### 6. Block comments compute resulting selection explicitly
**Decision:** yes.

**Reasoning:**
Block comments are different.
When adding delimiters around a selection, the resulting selection should still correspond to the original content inside the new delimiters, not just a mechanical mapped shell.

This makes explicit selection computation the right behavior.

---

### 7. Keep minimum-column alignment for line-comment insertion
**Decision:** yes.

When commenting multiple non-blank lines, insert the comment token at the minimum first-non-whitespace column across the selected lines.

Example:

```txt
line1
  line2
    line3
```

becomes:

```txt
// line1
//   line2
//     line3
```

not:

```txt
// line1
  // line2
    // line3
```

**Reasoning:**
This treats the selection as one commented block and gives better round-trip behavior.

---

### 8. Blank lines stay pragmatically excluded from line-comment insertion
**Decision:** yes.

**Reasoning:**
Blank lines generally should not receive inserted line comment tokens when toggling a block of lines.
This matches ordinary editor expectations and keeps the behavior clean.

---

### 9. Longest matching token wins when detecting existing comments
**Decision:** yes.

**Reasoning:**
If tokens include overlapping prefixes like:
- `///`
- `//`

then detection should prefer the longest matching token.
This prevents doc-comment sections from being misread as shorter ordinary comments.

---

## Ideal public interface

The exact implementation can vary, but the public API should feel close to this:

```rust
use ropey::Rope;

use crate::{
  edit::Edit,
  selection::Selection,
  syntax::BlockCommentToken,
};

pub fn toggle_line_comments(
  doc: &Rope,
  selection: &Selection,
  token: Option<&str>,
) -> Result<Edit>;

pub fn toggle_block_comments(
  doc: &Rope,
  selection: &Selection,
  tokens: &[BlockCommentToken],
) -> Result<Edit>;
```

### Notes
- both operations return `Edit`
- line comments use mapped resulting selection
- block comments may compute resulting selection explicitly
- explicit tokens in, no syntax lookup inside

---

## What should be kept from the old implementation

### Keep
- longest matching line-token detection
- minimum-column line-comment alignment
- pragmatic blank-line handling
- block-comment token preparation and longest-token preference
- explicit block-comment analysis when toggling around selections

### Keep with refactor
- block-comment internal analysis helpers
- block-comment result-shaping logic
- selection splitting helpers only if still necessary after simplification

### Remove
- old `Transaction` APIs like hidden selection attachment
- old iteration assumptions from the previous `Selection` API
- any broad public helper surface that is not genuinely needed

---

## Testing priorities

The most important tests should prove:

1. **line comment toggling**
   - comment vs uncomment behavior
   - minimum-column alignment
   - blank-line handling
   - 0-margin vs 1-margin uncommenting
   - round-trip behavior

2. **token detection**
   - longest matching token wins
   - unicode/char-boundary-safe behavior

3. **block comment toggling**
   - uncomment existing wrapped selections
   - comment unwrapped selections
   - whitespace-only selections behave sensibly
   - resulting selection preserves the original content semantics inside inserted delimiters

4. **selection behavior**
   - line comments use mapped selection
   - block comments use explicit semantic resulting selection
   - cursor identity is preserved where possible

Prefer a few strong behavior tests over many tiny helper tests.

---

## Summary

`comment.rs` should stay a small pure module for line and block comment toggling.

Keep it:
- pure
- token-driven
- small in public surface
- explicit about returning `Edit`
- simple about line vs block selection behavior
- independent from syntax lifecycle and product-layer command policy

It should plan comment edits, not become the place where comment policy, syntax lookup, or command semantics accumulate.
