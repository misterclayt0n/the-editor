# Match Brackets Module Architecture

This document defines the target architecture for `the-lib/match_brackets.rs`.

It answers four questions:

1. What is bracket matching in the new editor?
2. What should the public API feel like?
3. What belongs in `match_brackets.rs`, and what does not?
4. What should be kept or removed from the old pasted implementation?

This document is intentionally opinionated. `match_brackets.rs` should stay tiny, pure, and narrowly scoped.

---

## Purpose

`match_brackets.rs` defines pure position-based bracket matching helpers.

It is responsible for:
- finding the matching bracket when the cursor is directly on a bracket
- finding the closing bracket of the nearest enclosing scope in fuzzy mode
- using active syntax when available to improve structural matching
- falling back to bounded plaintext scanning when syntax is insufficient
- supporting a small canonical set of actual bracket pairs

It is **not** responsible for:
- selections or multi-cursor transforms
- movement/command orchestration
- surround editing
- pair-aware typing behavior
- text-object selection like inside/around pair
- syntax lifecycle state like deferred/unavailable syntax
- quote/string semantics as a primary feature

---

## Core position

`match_brackets.rs` is a **pure single-position query module**.

It inspects:
- an active `&Syntax`
- a document slice
- one cursor position

and returns:
- `Option<usize>`

That means the module answers questions like:

> If the cursor is here, where is the matching bracket?

or:

> If the cursor is inside a bracketed scope, where is the closing bracket of the nearest enclosing scope?

It does not decide what to do with that answer.
Higher layers may use it to:
- move the cursor
- extend a selection
- flash a highlight
- do nothing

---

## Relationship to `selection.rs`

This module should **not** be selection-centric.

It should not take or return `Selection`.
It should not know about cursor IDs.
It should not own multi-cursor behavior.

If a higher layer wants to apply bracket matching across selections, it can map this primitive over cursor positions.

**Reasoning:**
This module is simpler than `object.rs` or `text_object.rs`. Its honest unit of work is one position in, one optional position out.

---

## Relationship to `syntax.rs`

Bracket matching should be **syntax-first with bounded plaintext fallback**.

That means:
- if active syntax can identify a meaningful structural pair, use it
- if syntax cannot help, fall back to a bounded plaintext scan

It should consume:
- `&Syntax`
- `RopeSlice`
- `usize`

It should **not** consume:
- `DocumentSyntax`
- syntax availability/lifecycle state
- runtime loaders/resources

If syntax is unavailable, the caller should decide not to invoke the syntax-backed APIs, or invoke a plaintext-only helper if one remains internal.

---

## Relationship to `surround.rs` and `text_object.rs`

This module is adjacent to surround and pair-based text-object behavior, but it should stay separate.

### `match_brackets.rs`
Owns:
- locating a matching bracket position
- strict under-cursor bracket lookup
- fuzzy enclosing-scope closing-bracket lookup

### `surround.rs` / future `text_object.rs`
Own:
- inside/around pair selection
- nearest-pair text objects
- change/delete surround behavior
- broader pair semantics

### Why split them
Bracket matching is a **lookup primitive**.
Surround and text objects are **semantic editing/selection behaviors**.

If these are merged, `match_brackets.rs` stops being a tiny obvious module.

---

## Non-goals

`match_brackets.rs` should **not** become:
- a general pair semantics module
- a quote/string matching system
- a selection transform module
- a command or movement layer
- a syntax lifecycle wrapper
- a fallback parser for arbitrary structural analysis

Keep it focused on bracket lookup.

---

## Main decisions

### 1. Keep the module position-based and pure
**Decision:** yes.

Preferred public shape:

```rust
pub fn find_matching_bracket(... ) -> Option<usize>
pub fn find_matching_bracket_fuzzy(... ) -> Option<usize>
```

**Reasoning:**
This is the smallest honest boundary. Callers can compose richer behavior on top.

---

### 2. Keep strict and fuzzy matching as separate public APIs
**Decision:** yes.

### Strict matching
Only succeeds when the cursor is directly on a bracket.

### Fuzzy matching
If the cursor is not directly on a bracket, tries to find the nearest enclosing bracketed scope and returns that scope's closing bracket.

**Reasoning:**
These are different user intents and should remain explicit at the API level.

---

### 3. Use syntax first, then bounded plaintext fallback
**Decision:** yes.

**Reasoning:**
Syntax can improve structural accuracy.
Plaintext fallback keeps the feature useful in incomplete or weakly-structured text.

This is the right balance between correctness and pragmatism.

---

### 4. Keep fallback matching bounded
**Decision:** yes.

Bounded constants like:
- `MAX_PLAINTEXT_SCAN`
- `MATCH_LIMIT`

are appropriate.

**Reasoning:**
Bracket matching is a local editor interaction.
The fallback should remain fast and predictable, not turn into an unbounded full-document search.

---

### 5. Match actual brackets, not general symmetric quotes
**Decision:** yes.

Primary bracket vocabulary should remain things like:
- `()`
- `{}`
- `[]`
- `<>`
- supported unicode bracket forms

Do **not** make quote-like pairs such as:
- `""`
- `''`
- `` `` ``

part of the primary public bracket-matching contract.

**Reasoning:**
Quotes have language- and escape-sensitive semantics and belong closer to surround/text-object/string-aware behavior, not simple bracket matching.

---

### 6. Fuzzy matching returns the closing bracket of the enclosing scope
**Decision:** yes.

When the cursor is inside a scope rather than directly on a bracket, fuzzy matching should return:
- the position of the closing bracket for the nearest enclosing matched scope

It should **not** return:
- the opening bracket
- both endpoints
- a span/range

**Reasoning:**
This keeps fuzzy behavior single-purpose and preserves the small `Option<usize>` API.

---

### 7. Keep helper predicates/utilities out of the main public surface
**Decision:** yes.

Helpers like:
- plaintext-only scanners
- bracket classification predicates
- pair lookup utilities

should remain private or `pub(crate)` unless another module clearly needs them.

**Reasoning:**
The real public boundary is the strict/fuzzy matching API. Exposing all helpers publicly would freeze too much incidental API surface.

---

## Ideal public interface

The exact implementation can vary, but the public API should feel close to this:

```rust
use ropey::RopeSlice;

use crate::syntax::Syntax;

pub fn find_matching_bracket(
  syntax: &Syntax,
  text: RopeSlice,
  pos: usize,
) -> Option<usize>;

pub fn find_matching_bracket_fuzzy(
  syntax: &Syntax,
  text: RopeSlice,
  pos: usize,
) -> Option<usize>;
```

### Notes
- one position in
- one optional matching position out
- syntax-aware, but only with already-active syntax
- no `Selection`
- no command semantics

---

## What should be kept from the old implementation

### Keep
- strict matching under cursor
- fuzzy enclosing-scope behavior
- syntax-first lookup strategy
- bounded plaintext fallback
- small local helpers for pair classification and scanning

### Keep with cleanup
- bounded constants
- sibling/parent traversal heuristics
- plaintext nested-bracket counting logic

These are fine if they stay private and understandable.

### Remove or narrow
- broad public helper surface if it is not needed elsewhere
- any implication that this module is a general pair/surround/text-object system
- quote-pair semantics from the primary public contract

---

## Testing priorities

The most important tests should prove:

1. **strict plaintext symmetry**
   - opening bracket finds closing bracket
   - closing bracket finds opening bracket
   - nested brackets behave correctly

2. **strict failure behavior**
   - non-bracket under cursor returns `None`
   - out-of-bounds position returns `None`
   - unmatched bracket returns `None`

3. **fuzzy behavior**
   - inside a bracketed scope returns the closing bracket of the nearest enclosing scope
   - on a bracket, fuzzy matches strict behavior

4. **bounded fallback behavior**
   - local plaintext scan works when syntax lookup is not enough
   - no need for giant coverage tables; just prove the real behavior

5. **vocabulary behavior**
   - supported actual brackets match
   - quote-like symmetric pairs are not part of the primary public behavior

Prefer a few strong tests over lots of tiny predicate tests.

---

## Summary

`match_brackets.rs` should stay a tiny pure module for bracket lookup.

Keep it:
- position-based
- syntax-first with bounded plaintext fallback
- explicit about strict vs fuzzy behavior
- focused on actual brackets
- small in public API surface

It should remain a lookup primitive, not grow into surround, text-object, or command behavior.
