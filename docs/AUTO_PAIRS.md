# Auto Pairs Module Architecture

This document defines the target architecture for `the-lib/auto_pairs.rs`.

It answers four questions:

1. What is auto-pairs in the new editor?
2. What should the public API feel like?
3. What belongs in `auto_pairs.rs`, and what does not?
4. What should be removed from the old pasted implementation?

This document is intentionally opinionated. `auto_pairs.rs` should stay small, deterministic, and narrowly scoped. It is editing behavior, but it should not grow into a general typing pipeline.

---

## Purpose

`auto_pairs.rs` defines a pure planner for pair behavior during typing and backspace.

It is responsible for:
- pair insertion for typed characters
- closer skipping when a matching closer already exists at the insertion edge
- pair-aware backspace deletion
- computing the exact resulting selection for those edits
- supporting configurable multi-character pairs
- preserving cursor identity across the edit

It is **not** responsible for:
- applying edits
- mutating editor state
- talking to history/runtime/UI
- surround behavior
- syntax-aware or language-aware rule engines
- indentation/newline expansion
- general insert-mode command orchestration

---

## Core position

`auto_pairs.rs` is a **pure edit-planning module**.

It inspects:
- a document
- a selection
- a typed character or backspace action
- a configured set of pairs

and returns:
- a document `Transaction`
- the exact resulting `Selection`

It does not apply the edit itself.

This means `auto_pairs.rs` should fit cleanly alongside the other backbone modules:

```txt
Transaction: pure document delta
Selection:  directional cursor/range state
AutoPairs:  pair-behavior planner producing transaction + resulting selection
History:    revision graph + exact committed selection snapshots
```

---

## Relationship to `transaction.rs` and `selection.rs`

### `Transaction`
A transaction means:
- how text changes
- how to apply/invert/compose/map a document delta

Auto-pairs should produce a `Transaction`, but should not hide selection inside it.

### `Selection`
A selection means:
- directional ranges
- stable cursor identity
- view/session-local cursor state

Auto-pairs should compute the exact resulting `Selection` explicitly, because cursor landing behavior is part of the auto-pair semantics.

---

## Main decisions

### 1. Return a small module-local edit result
**Decision:** return a concrete module-local result containing both `Transaction` and `Selection`.

Preferred shape:

```rust
pub struct AutoPairEdit {
  pub transaction: Transaction,
  pub selection:   Selection,
}
```

**Reasoning:**
Auto-pairs inherently decides both:
- what text edit should happen
- where the resulting cursor/selection should land

Returning only `Transaction` would be incomplete. A tuple would work, but a tiny named struct is clearer and still minimal.

Do **not** extract a shared abstraction yet. If another module needs to return the same pair, revisit then.

---

### 2. Keep the module pure and planning-only
**Decision:** yes.

**Reasoning:**
`auto_pairs.rs` should:
- inspect inputs
- plan an edit
- return `Option<AutoPairEdit>`

It should **not**:
- apply the edit
- mutate document state
- integrate with history directly
- become part of a larger imperative command pipeline

This keeps it deterministic and easy to test.

---

### 3. Keep the scope narrow
**Decision:** `auto_pairs.rs` should remain specifically about pair insertion / skip / pair-delete behavior.

**Reasoning:**
This module should not grow into a general “typing behavior” dump.

It should own things like:
- insert `(` and maybe `)`
- type `)` and maybe skip over an existing `)`
- backspace between `()` and delete both
- same behavior for configured multi-char pairs

It should not absorb:
- surround operations
- syntax-smart typing rules
- indentation or newline expansion
- language-specific behavior engines

---

### 4. Rename vague hook-style APIs
**Decision:** replace old `hook` / `delete_hook` naming with explicit edit planners.

Preferred public API:

```rust
pub fn edit_for_typed_char(...)
pub fn edit_for_backspace(...)
```

**Reasoning:**
These functions are not event handlers. They are pure planners returning a possible edit.

`edit_for_typed_char` and `edit_for_backspace` say what they produce and when they apply.

---

### 5. Keep typed-char handling single-character only
**Decision:** yes.

**Reasoning:**
This module models pair behavior during typing, not arbitrary inserted text or paste behavior.

Implication:
- typed insertion API should stay `char`-based
- do not generalize this module to arbitrary inserted strings

---

### 6. Backspace behavior is cursor-based pair delete, not general delete
**Decision:** yes.

**Reasoning:**
The delete-side API should model the specific user action that triggers it:
- backspace at a cursor between a pair

This module should not become a general deletion planner.

---

### 7. `Option` means “planner applies or not”
**Decision:** return `Ok(None)` whenever auto-pairs simply does not apply.

**Reasoning:**
This keeps the module ergonomic for higher-level callers:
- ask auto-pairs first
- if it returns `None`, fall back to ordinary typing/backspace behavior

Errors should be reserved for:
- actual invalid inputs/invariants
- selection construction failures
- transaction construction failures

---

### 8. Multi-cursor behavior is all-or-nothing
**Decision:** yes.

**Reasoning:**
If one cursor can auto-pair and another cannot, the module should decline the whole operation with `Ok(None)`.

This avoids partial “special behavior for some cursors, default behavior for others” inside the planner itself.

If more complex per-cursor fallback is ever needed, that belongs in a higher-level command combiner, not here.

---

### 9. Compute the resulting selection explicitly
**Decision:** yes.

**Reasoning:**
Auto-pair selection landing is part of the behavior itself.

Examples:
- insert `(` and place cursor between `()`
- type `)` and skip over an existing closer
- backspace between `()` and land at the start of the deleted pair

These are not generic selection-mapping consequences that should be inferred afterward. They should be computed directly.

---

### 10. Internal planning should be cursor-entry-centric
**Decision:** yes.

**Reasoning:**
The public selection model is cursor-entry-centric and cursor IDs must be preserved. The internal planning model should reflect that.

Auto-pairs should operate on:
- `CursorId`
- `Range`

not on bare ranges alone.

---

### 11. Preserve cursor IDs exactly
**Decision:** yes.

**Reasoning:**
Auto-pairs transforms an existing selection. It does not invent new cursors.

Implication:
- every surviving cursor should keep its original `CursorId`
- final selection construction should preserve those IDs unless normal selection normalization merges cursors

---

### 12. Let normal selection normalization apply
**Decision:** yes.

**Reasoning:**
If the resulting cursor entries overlap or collapse together, the ordinary `Selection` normalization rules should decide what survives.

Auto-pairs should not fight or special-case those semantics.

Rule:
- preserve IDs in planned outputs
- construct final `Selection` normally
- accept normalization/merge behavior if it happens

---

### 13. Rewrite around `Replacement` and `Span`
**Decision:** yes.

**Reasoning:**
The old implementation depended on the old tuple-shaped `Change` API and `Transaction::change_by_selection(...)`.

The new implementation should instead:
- plan per-cursor replacement-style edits
- lower them once through `Transaction::from_replacements(...)` or `Transaction::delete_spans(...)`

This keeps the module aligned with the new transaction API.

---

### 14. Keep a two-phase internal structure
**Decision:** yes.

**Reasoning:**
The cleanest internal architecture is:

1. decide per-cursor behavior and resulting range
2. lower all cursor plans into one `Transaction` + final `Selection`

This separates:
- matching/behavior semantics
- transaction construction
- selection reconstruction

---

### 15. Represent skip explicitly
**Decision:** yes.

**Reasoning:**
Skip is a first-class auto-pair behavior. It should not be faked as a no-op replacement.

Preferred internal shape:

```rust
enum CursorPlan {
  Replace {
    replacement: Replacement,
    range: Range,
  },
  Move {
    range: Range,
  },
}
```

This cleanly distinguishes:
- text-changing behavior
- selection-only cursor movement

---

### 16. Typed-char edits act at the insertion edge (`head`)
**Decision:** yes.

**Reasoning:**
Auto-pairs runs on insert-mode typing. It should use the selection’s insertion edge (`head`), not a block-cursor edge derived from `cursor()`.

Implications:
- do not surround non-empty selections
- do not replace selected text
- type at `range.head`
- compute resulting range relative to that insertion edge

This preserves the old behavior and keeps the module out of surround semantics.

---

### 17. Backspace only applies between matching pairs
**Decision:** yes.

**Reasoning:**
`edit_for_backspace(...)` should only apply when:
- every range is empty
- every cursor is exactly between a matching opener and closer

Otherwise it returns `Ok(None)`.

This keeps the behavior crisp and unsurprising.

---

### 18. Use strict overlap rejection when lowering
**Decision:** yes.

**Reasoning:**
If the planner produces overlapping per-cursor edits, that is likely a real bug or unsupported scenario. The module should not silently hide it.

Preferred lowering policies:
- typed-char path:
  `Transaction::from_replacements(..., ReplacementOverlapPolicy::Reject)`
- backspace path:
  `Transaction::delete_spans(..., DeletionOverlapPolicy::Reject)`

---

### 19. Longest matching pair wins
**Decision:** yes.

**Reasoning:**
Multi-character pairs only work well if longer applicable matches beat shorter ones.

Examples:
- `""` before cursor + typed `"` should prefer `"""` over `"`
- close-side matching should similarly prefer `"""` over `"`

Rule:
- longest open match wins
- longest close match wins

---

### 20. Config order breaks equal-length ties
**Decision:** yes.

**Reasoning:**
If multiple equal-length pairs match, preserve the configured pair order as the tie-breaker.

This is deterministic, simple, and makes the ordered list semantically meaningful.

---

### 21. Preserve current close-eligibility heuristics
**Decision:** yes.

Rules:
- different opener/closer pairs auto-close only if the next character is not alphanumeric
- same pairs (quotes/backticks) auto-close only if:
  - the next character is not alphanumeric
  - and the previous character is not alphanumeric

**Reasoning:**
This is a small, deterministic heuristic that prevents obviously bad behavior like pairing quotes inside words.

---

### 22. If pairing is not allowed, still insert the typed character
**Decision:** yes.

**Reasoning:**
If the typed character is recognized as an opener, but close-eligibility conditions fail, the planner should still insert the typed character by itself.

That means:
- recognized opener + close disallowed → single-character insert
- not `Ok(None)`

`Ok(None)` remains for cases where auto-pairs does not want to handle the edit at all.

---

### 23. Skip existing closers by moving only
**Decision:** yes.

**Reasoning:**
When the typed character matches a configured closer already present at the insertion edge, auto-pairs should:
- not insert anything
- move the cursor/selection forward by that closer’s length

This is core auto-pair UX and must work for both single-char and multi-char closers.

---

### 24. Backspace lands at the start of the deleted pair
**Decision:** yes.

**Reasoning:**
For pair-delete/backspace, the resulting cursor should collapse to the start of the deleted pair span.

Rule:
- delete `[from, to)`
- resulting range is `Range::point(from)`

---

### 25. Reject empty pairs at construction time
**Decision:** yes.

**Reasoning:**
Pairs with empty open or close components are not meaningful auto-pairs and only complicate matching logic.

So `AutoPairs::new(...)` should validate and reject:
- empty opener
- empty closer

This is a straightforward “make invalid states impossible” improvement.

---

### 26. `AutoPairs::new(...)` should be fallible
**Decision:** yes.

Preferred shape:

```rust
pub fn new<V, A>(pairs: V) -> Result<Self>
where
  V: IntoIterator<Item = A>,
  A: Into<Pair>,
```

**Reasoning:**
If empty pairs are invalid, construction should be the place where that is enforced.

`Default` can remain infallible because `DEFAULT_PAIRS` is known-valid.

---

### 27. `AutoPairs` remains a simple validated ordered list
**Decision:** yes.

**Reasoning:**
Do not turn `AutoPairs` into a rule engine.

Keep it as:
- `Pair { open, close }`
- `AutoPairs(Vec<Pair>)`
- current built-in heuristics
- ordered config list semantics

No callbacks, predicates, syntax node rules, or per-pair custom logic should be introduced here.

---

## Ideal public interface

This is the target public-facing shape for `auto_pairs.rs`.
The exact implementation can vary, but the interface should feel close to this.

```rust
use ropey::Rope;
use thiserror::Error;

use crate::{
  Tendril,
  selection::{Selection, SelectionError},
  transaction::{
    Transaction,
    TransactionError,
    Replacement,
    ReplacementOverlapPolicy,
    DeletionOverlapPolicy,
    Span,
  },
};

pub const DEFAULT_PAIRS: &[(&str, &str)];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pair {
  pub open:  Tendril,
  pub close: Tendril,
}

#[derive(Debug, Clone)]
pub struct AutoPairs(Vec<Pair>);

#[derive(Debug, Clone)]
pub struct AutoPairEdit {
  pub transaction: Transaction,
  pub selection:   Selection,
}

pub type Result<T> = std::result::Result<T, AutoPairsError>;

#[derive(Debug, Error, PartialEq, Eq)]
#[non_exhaustive]
pub enum AutoPairsError {
  #[error("auto-pair open string must not be empty")]
  EmptyOpenPair,

  #[error("auto-pair close string must not be empty")]
  EmptyClosePair,

  #[error(transparent)]
  Selection(#[from] SelectionError),

  #[error(transparent)]
  Transaction(#[from] TransactionError),
}

impl Pair {
  pub fn same(&self) -> bool;
  pub fn open_len(&self) -> usize;
  pub fn close_len(&self) -> usize;
  pub fn open_last_char(&self) -> Option<char>;
  pub fn close_first_char(&self) -> Option<char>;
  pub fn should_close(&self, doc: &Rope, range: &crate::selection::Range) -> bool;
}

impl AutoPairs {
  pub fn new<V, A>(pairs: V) -> Result<Self>
  where
    V: IntoIterator<Item = A>,
    A: Into<Pair>;

  pub fn pairs(&self) -> &[Pair];
  pub fn matches_char(&self, ch: char) -> bool;
}

impl Default for AutoPairs {
  fn default() -> Self;
}

pub fn edit_for_typed_char(
  doc: &Rope,
  selection: &Selection,
  ch: char,
  pairs: &AutoPairs,
) -> Result<Option<AutoPairEdit>>;

pub fn edit_for_backspace(
  doc: &Rope,
  selection: &Selection,
  pairs: &AutoPairs,
) -> Result<Option<AutoPairEdit>>;
```

---

## Core invariants

These invariants should be easy to state and easy to defend in tests.

### Pair invariants
- every pair has a non-empty opener
- every pair has a non-empty closer
- pair order is meaningful and preserved

### Planner invariants
- `Ok(None)` means auto-pairs declined the edit cleanly
- `Ok(Some(edit))` means auto-pairs owns the whole edit for the whole selection
- multi-cursor edits are all-or-nothing
- resulting selection is computed explicitly
- surviving cursor IDs are preserved unless normal selection normalization merges them

### Lowering invariants
- typed-char edits lower through replacement-style transaction construction
- backspace edits lower through deletion-span transaction construction
- overlap is rejected strictly

---

## Behavior semantics

### Typed character path
For each cursor entry:
- use `range.head` as the insertion edge
- if typed char is an applicable closer already present at the insertion edge:
  - produce a move-only plan
- else if typed char is an applicable opener:
  - if closing conditions hold, insert opener + closer
  - otherwise insert opener only
- else:
  - if the char is not relevant to any configured pair, decline the whole operation with `Ok(None)`

### Backspace path
For each cursor entry:
- selection range must be empty
- cursor must be exactly between a matching opener and closer
- delete the entire matched pair span
- resulting cursor lands at the start of that deleted span

If any cursor does not qualify, return `Ok(None)` for the whole operation.

### Pair precedence
- longest matching pair wins
- equal-length ties preserve config order

---

## What should be removed from the old pasted implementation

The old pasted `auto_pairs.rs` still depends on the old transaction/selection model. The clean rewrite should remove those assumptions directly.

### Remove transaction-attached selection assumptions
Delete logic that depends on:
- `transaction.with_selection(...)`
- `tx.selection()`

### Remove old tuple-shaped `Change` usage
Do not preserve old `(usize, usize, Option<Tendril>)`-based edit planning as the conceptual center.

### Remove `Transaction::change_by_selection(...)`
Rewrite planning around cursor-entry plans plus `Replacement` / `Span` lowering.

### Remove old delete transaction construction shape
Do not preserve old `Transaction::delete(doc, iter)` usage.

### Remove range-only internal planning
Do not keep an internal model that ignores cursor IDs and reconstructs them later as an afterthought.

### Remove support for zero-length pair weirdness
Once construction validation is added, matching logic should no longer need to tiptoe around empty pair strings as if they were meaningful runtime inputs.

---

## Refactoring guidance for the implementation

When refactoring `the-lib/auto_pairs.rs`, prefer these principles:

- keep the public surface small
- keep matching logic separate from lowering logic
- plan per cursor entry, not per bare range
- preserve cursor IDs explicitly
- compute resulting selections directly
- lower once through the new transaction API
- reject invalid configuration early

Reasonable private helpers include things like:
- longest-match lookup helpers
- backspace pair-span detection
- cursor-plan construction for typed-char behavior
- cursor-plan construction for backspace behavior
- result-selection construction from planned cursor outputs

Avoid introducing a generic command abstraction unless the pattern clearly repeats elsewhere.

---

## Testing priorities

When this module is rewritten, tests should focus on behavior and invariants, not on the old transaction attachment model.

High-value tests include:
- invalid empty pair rejection in `AutoPairs::new`
- single-char opener insert with pair
- same-pair quote behavior inside/outside words
- closer skip behavior
- multi-char pair insertion and skip behavior
- longest-match precedence
- config-order tie-breaking for equal-length pairs
- backspace pair deletion for single-char and multi-char pairs
- all-or-nothing multi-cursor behavior
- cursor ID preservation across edits
- non-empty selection typed-char behavior uses insertion edge rather than surround semantics
- resulting selection values after insert, skip, and delete

The goal is the same as for the other backbone modules:
- behavior-first tests
- explicit semantics
- few strong tests over many weak ones
- high confidence per line
