# Search Module Architecture

This document defines the target architecture for `the-lib/search.rs`.

It answers four questions:

1. What is search in the new editor?
2. What should the public API feel like?
3. What belongs in `search.rs`, and what does not?
4. What should be kept or removed from the old pasted implementation?

This document is intentionally opinionated. `search.rs` should stay small, pure, and explicitly document-local.

---

## Purpose

`search.rs` defines pure document-local search helpers.

It is responsible for:
- low-level character search within a `RopeSlice`
- building regexes for interactive in-buffer search
- finding the next/previous regex match in a document slice
- updating a `Selection` for one interactive regex search step

It is **not** responsible for:
- workspace/global search
- search sessions or async orchestration
- search UI/panels/prompts
- search history/registers
- persistent search state
- multi-document or resource-oriented search

---

## Core position

`search.rs` is a **pure document-local search module**.

It inspects:
- document text
- simple search inputs like a character predicate or regex
- selection state for the interactive regex helper

and returns:
- positions for primitive search
- a new `Selection` for one interactive regex search step

This fits the architecture split in `docs/ARCHITECTURE.md`:
- document-local search belongs in `the-lib`
- workspace/global search belongs in `the-runtime`
- search UI behavior belongs above Rust

---

## Relationship to `selection.rs`

Most of `search.rs` should remain **position-based**.

That means low-level character search should work in terms of:
- `RopeSlice`
- character predicates
- positions

and return:
- `Option<usize>`

Only the interactive regex helper should be selection-centric.

### Why
Character search is a reusable primitive.
It is useful for movement, surround, and similar document-local behaviors.
It should not be inflated into selection-manipulation helpers unless that need becomes unmistakable.

Regex search is different because the existing helper already models a real interactive editor action:
- pick one cursor/range
- search in a direction
- optionally wrap
- update the selection according to movement semantics

---

## Relationship to `movement.rs`

`search.rs` may consume small movement concepts like:
- `Direction`
- `Movement`

for the interactive regex helper.

But it should not become a movement-command module.

### Why
The regex helper is a convenience for interactive search behavior, not a replacement for movement architecture.
The low-level search primitives should remain independent and reusable.

---

## Non-goals

`search.rs` should **not** become:
- a workspace search layer
- a runtime search session API
- a command/history/search-register module
- a search results UI model
- a "find all matches and build multicursor state everywhere" subsystem
- a general regex-state orchestration layer

Keep it focused on small, pure, in-buffer searching.

---

## Main decisions

### 1. Keep `search.rs` document-local only
**Decision:** yes.

**Reasoning:**
In-buffer deterministic search belongs in `the-lib`.
Workspace/global search is resource/service-oriented and belongs in `the-runtime`.

---

### 2. Keep a two-level API split
**Decision:** yes.

### Low-level primitives
Own:
- `find_nth`
- `find_nth_next`
- `find_nth_prev`
- regex construction

### One higher-level helper
Own:
- `search_regex(...) -> Option<Selection>`

**Reasoning:**
This is a clean split between reusable primitives and one genuinely editor-shaped helper.

---

### 3. Keep character search position-based
**Decision:** yes.

Character search should take positions and return positions.

**Reasoning:**
This keeps the primitive reusable and avoids bloating the module with selection-specific wrappers that other layers can compose if needed.

---

### 4. Keep regex search as a single-match interactive helper
**Decision:** yes.

`search_regex(...)` should:
- find one next/previous match
- update one picked cursor/range
- either replace or extend according to `Movement`

It should **not** become:
- find-all
- global multicursor expansion
- persistent search session state

**Reasoning:**
This keeps the helper honest and small.

---

### 5. Keep regex building pure and small
**Decision:** yes.

Preferred helper shape:

```rust
pub fn build_regex(query: &str, smart_case: bool) -> Result<Regex, String>
```

**Reasoning:**
Interactive search wants a tiny pure regex-compilation helper with smart-case behavior. That is a good fit for this module.

---

### 6. Keep wrap-around an explicit caller choice
**Decision:** yes.

**Reasoning:**
Whether search wraps is real behavior, not an implementation detail.
It should stay explicit in the search helper API.

---

## Ideal public interface

The exact implementation can vary, but the public API should feel close to this:

```rust
use ropey::RopeSlice;
use the_stdx::rope::Regex;

use crate::{
  movement::{Direction, Movement},
  selection::{CursorPick, Selection},
};

pub trait CharMatcher {
  fn char_match(&mut self, ch: char) -> bool;
}

pub enum SearchDirection {
  Next,
  Prev,
}

pub enum SearchStart {
  Inclusive,
  Exclusive,
}

pub fn find_nth<M: CharMatcher>(
  text: RopeSlice,
  matcher: M,
  pos: usize,
  n: usize,
  direction: SearchDirection,
  start: SearchStart,
) -> Option<usize>;

pub fn find_nth_next<M: CharMatcher>(
  text: RopeSlice,
  matcher: M,
  pos: usize,
  n: usize,
) -> Option<usize>;

pub fn find_nth_prev<M: CharMatcher>(
  text: RopeSlice,
  matcher: M,
  pos: usize,
  n: usize,
) -> Option<usize>;

pub fn build_regex(query: &str, smart_case: bool) -> Result<Regex, String>;

pub fn search_regex(
  text: RopeSlice,
  selection: &Selection,
  pick: CursorPick,
  regex: &Regex,
  movement: Movement,
  direction: Direction,
  wrap_around: bool,
) -> Option<Selection>;
```

---

## What should be kept from the old implementation

### Keep
- the low-level nth-match primitives
- `CharMatcher` as the rope-friendly character matcher abstraction
- smart-case regex building
- one interactive regex search helper returning updated `Selection`
- explicit wrap-around behavior

### Keep with cleanup
- the regex search helper's picked-cursor update logic
- grapheme-boundary handling around search start positions

### Remove or avoid
- any growth into workspace/global search
- any expansion into search session/state management
- selection-shaped helpers for all primitive char search operations unless clearly justified later

---

## Testing priorities

The most important tests should prove:

1. **character search primitives**
   - next vs previous
   - inclusive vs exclusive start behavior
   - predicate-based search
   - edge cases like out-of-range and `n == 0`

2. **regex building**
   - smart-case lowers case sensitivity when appropriate
   - uppercase query disables smart-case folding

3. **interactive regex search**
   - wrap-around behavior
   - backward search behavior
   - picked cursor/range is the one updated
   - `Movement::Move` replaces, `Movement::Extend` adds

Prefer a few strong behavioral tests over a large matrix of tiny variants.

---

## Summary

`search.rs` should stay a small pure module for document-local search.

Keep it:
- document-local only
- primitive for character search
- modestly editor-shaped for one regex search helper
- pure in regex building
- explicit about direction and wrap-around

It should not grow into runtime search, UI search, or persistent search-session machinery.
