# Indent Module Architecture

This document defines the target architecture for `the-lib/indent.rs`.

It answers four questions:

1. What is indentation in the new editor?
2. What should the public API feel like?
3. What belongs in `indent.rs`, and what does not?
4. What should be kept or removed from the old pasted implementation?

This document is intentionally opinionated. `indent.rs` may remain a large module for now, but it should still have a clear boundary: pure indentation computation only.

---

## Purpose

`indent.rs` defines pure indentation decisions and helpers.

It is responsible for:
- indentation style representation and detection
- whitespace width / indentation width helpers
- indentation normalization to tabs or spaces
- computing indentation for a new line
- consuming syntax/query-derived indentation information when available
- applying indentation heuristics purely

It is **not** responsible for:
- applying indentation edits to a document
- producing `Transaction` or `Edit`
- selection transforms
- command parsing or modal behavior
- syntax registry/query lookup policy
- generic syntax-tree inspection helpers unrelated to indentation
- runtime/history/UI integration

---

## Core position

`indent.rs` is a **pure indentation computation module**.

It inspects:
- text
- indentation style/config
- optional syntax/query data
- local position/line context

and returns:
- indentation styles
- indentation levels
- normalized indentation strings
- the indentation string for a new line

It should answer questions like:

> What indentation style does this document most likely use?

> What should the indentation string for a new line be here?

> How should this leading whitespace be normalized for the configured indent style?

It should **not** answer:

> Apply this indentation change to the document.

That editing/planning layer can exist elsewhere later if needed.

---

## Relationship to `syntax.rs`

Indentation may consume syntax data, but it should not own syntax lookup.

That means `indent.rs` should consume explicit pure inputs like:
- `Option<&Syntax>`
- `Option<&IndentQuery>`
- `IndentationHeuristic`

It should **not** take:
- `LanguageRegistry`
- old `Loader`-style syntax registries
- syntax lifecycle state like `DocumentSyntax`

### Why
This keeps the boundary clean:
- `syntax` owns query compilation/lookup
- `indent` owns indentation computation

That makes `indent.rs` easier to reason about and easier to test.

---

## Relationship to `transaction.rs` and `edit.rs`

`indent.rs` should not produce document edits directly.

It may return strings or widths that a higher layer later turns into a `Transaction` or `Edit`, but it should not own that planning itself by default.

### Why
The core job here is:
- compute the desired indentation

not:
- apply or orchestrate indentation edits

That keeps the module from absorbing too many responsibilities while it is already large.

---

## Non-goals

`indent.rs` should **not** become:
- a reindent transaction planner
- a generic formatting engine
- a syntax registry wrapper
- a command behavior module
- a home for random syntax inspection helpers

Keep it focused on indentation computation.

---

## Main decisions

### 1. Keep `indent.rs` as one module for now
**Decision:** yes.

**Reasoning:**
Even though the file is large, it still represents one coherent domain:
- indentation style
- indentation width
- newline indentation computation
- syntax-guided indentation heuristics

Splitting should wait until a more obvious seam emerges.

---

### 2. Keep the module pure and computation-only
**Decision:** yes.

**Reasoning:**
The module should compute indentation decisions, not apply them as edits.

This keeps it:
- testable
- composable
- reusable by future editing/planning layers

---

### 3. Consume explicit `IndentQuery` input, not registry lookup
**Decision:** yes.

Preferred shape for syntax-aware newline indentation:
- accept `Option<&IndentQuery>` explicitly
- do not perform language/query lookup internally

**Reasoning:**
Query lookup belongs to syntax infrastructure. Indent computation belongs here.

---

### 4. Keep syntax optional
**Decision:** yes.

**Reasoning:**
Indentation computation should still work without syntax/query data.
When syntax is unavailable or queries are absent, the module should fall back to simpler indentation heuristics.

---

### 5. Remove generic syntax helpers like `get_scopes(...)`
**Decision:** yes.

If such a helper is still useful, it should move to syntax-side helpers near/on `Syntax`.

**Reasoning:**
It is generic syntax-tree inspection, not indentation logic. Keeping it here weakens the module boundary.

---

### 6. Preserve the current working semantics before chasing elegance
**Decision:** yes.

**Reasoning:**
This module is large and battle-tested. The first refactor goal should be:
- boundary cleanup
- dependency cleanup
- compile repair
- preserving behavior

Only after that should deeper algorithm decomposition be considered.

---

## Ideal public interface

The exact implementation can vary, but the public surface should feel close to this:

```rust
use ropey::{Rope, RopeSlice};

use crate::syntax::{IndentQuery, Syntax, IndentationHeuristic};

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum IndentStyle {
  Tabs,
  Spaces(u8),
}

pub fn auto_detect_indent_style(document_text: &Rope) -> Option<IndentStyle>;
pub fn indent_level_for_line(line: RopeSlice, tab_width: usize, indent_width: usize) -> usize;

pub fn normalize_indentation(
  prefix: RopeSlice,
  line: RopeSlice,
  dst: &mut Tendril,
  indent_style: IndentStyle,
  tab_width: usize,
) -> usize;

pub fn indent_for_newline(
  indent_query: Option<&IndentQuery>,
  syntax: Option<&Syntax>,
  indent_heuristic: &IndentationHeuristic,
  indent_style: &IndentStyle,
  tab_width: usize,
  text: RopeSlice,
  line_before: usize,
  line_before_end_pos: usize,
  current_line: usize,
) -> String;
```

### Notes
- pure computation only
- syntax/query inputs are explicit
- no registry lookup inside
- no `Transaction`/`Edit`

---

## What should be kept from the old implementation

### Keep
- `IndentStyle`
- auto-detect indentation style
- indentation width helpers
- normalization helpers
- tree-sitter query-based indentation computation
- fallback/simple/hybrid heuristic behavior

### Keep with cleanup
- the query evaluation machinery
- internal indentation accumulator logic
- helper routines that are truly indentation-specific

### Remove or move
- old `Loader`/registry dependency
- old `the_core` imports that no longer match the new crate layout
- generic syntax helpers like `get_scopes(...)`

---

## Testing priorities

The most important tests should prove:

1. **indent style detection**
   - tabs vs spaces
   - confidence heuristics
   - mixed indentation edge cases

2. **indent width helpers**
   - tab width handling
   - normalization preserving visual width

3. **newline indentation**
   - fallback behavior without syntax
   - syntax-query-driven behavior when query data exists
   - hybrid heuristic behavior remains stable

4. **internal indentation accumulation semantics**
   - indent/outdent/align interactions
   - relative indentation behavior

Prefer preserving proven behavior over aggressively rewriting the algorithm just to make it prettier.

---

## Summary

`indent.rs` should remain the pure indentation computation module.

Keep it:
- one module for now
- pure and computation-only
- explicit about syntax/query inputs
- independent from edit planning
- free of generic syntax helpers that do not belong here

The immediate refactor goal is boundary cleanup and compile repair without losing the behavior that already works.
