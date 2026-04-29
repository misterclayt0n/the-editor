# Movement Module Architecture

This document defines the target architecture for `the-lib/movement.rs`.

It answers four questions:

1. What belongs in `movement.rs` in the new editor?
2. What should the public API feel like?
3. What should stay in Rust, and what should stay in TypeScript?
4. What should be kept, rewritten, or moved out of the old pasted implementation?

This document is intentionally opinionated. `movement.rs` should be the pure home for **core text/layout cursor travel**, not the editor's full motion language.

---

## Purpose

`movement.rs` defines pure multi-cursor movement over document text and visual layout.

It is responsible for:
- moving selections horizontally by grapheme
- moving selections vertically by logical line
- moving selections vertically by wrapped visual row
- moving by word / long-word / subword boundaries
- moving by paragraph boundaries
- moving to logical line anchors
- moving to document anchors
- preserving `CursorId`s
- carrying explicit per-cursor visual goal state for vertical motions
- returning ordinary normalized `Selection`s

It is **not** responsible for:
- command parsing
- keymaps or modal behavior
- defining the editor's complete motion vocabulary
- syntax tree structural navigation
- syntax-query object motions
- runtime/UI/history integration
- hidden mutable interaction state

---

## Core position

`movement.rs` is a **pure selection-transform module with explicit vertical motion state**.

It inspects:
- document text
- a `Selection`
- a small built-in movement primitive
- one `MotionMode`
- a pure layout context
- explicit per-cursor visual goals

and returns:
- a new `Selection`
- updated per-cursor visual goals

That keeps it aligned with the rest of the architecture:

```txt
Selection:    directional cursor/range state + cursor IDs
Render:       visual row/column mapping helpers
Movement:     core text/layout cursor travel
Object:       structural syntax navigation
TextObject:   semantic object targeting
TS:           larger motion language and command composition
```

---

## The key architecture split

The old editor blurred together two different things under the word “motion”.

### 1. Core movement primitives
Examples:
- left/right
- up/down
- visual up/down
- next word start
- paragraph movement
- line start/end
- document start/end

These are:
- semantic
- correctness-sensitive
- multi-cursor-sensitive
- render/layout-sensitive
- often hot-path

These belong in `the-lib`.

### 2. The editor's larger motion language
Examples:
- matching bracket jump
- char-find / till-char
- next function/class/tag
- next diagnostic
- next diff hunk
- page up/down
- half-page / viewport-relative movement
- jump-list movement
- mode-specific/operator-pending composition

These do **not** all belong in `movement.rs`.

They come from different modules and layers:
- `movement.rs`
- `object.rs`
- `text_object.rs`
- `match_brackets.rs`
- `search.rs`
- later `diff.rs` / diagnostics / runtime services / surface logic
- TS command composition

### Decision
`movement.rs` should expose a **small built-in set of core text/layout movement primitives**, not a closed enum claiming to represent the full editor motion system.

TypeScript should own the larger motion language by composing Rust primitives.

---

## Relationship to TypeScript

TypeScript should be able to define richer user-facing motions without requiring every one of them to become a Rust enum variant.

Examples:
- `w` may map to `movement::move_next_word(...)`
- `%` may map to `match_brackets::find_matching_bracket(...)`
- `]m` may map to `text_object::move_syntax_named(...)`
- `f x` may map to `search::find_nth(...)` with user-provided parameters
- next diagnostic may map to a later diagnostics helper

So the rule should be:

- Rust owns the **motion-capable primitive semantics**
- TypeScript owns the **larger command/motion language**

This keeps Rust authoritative for correctness-sensitive cursor travel while keeping TS extensible.

---

## Relationship to `selection.rs`

Movement should be **selection-centric publicly**.

The public API should operate on full `Selection`s, not require higher layers to loop over ranges manually.

Important behavior:
- one input cursor produces one output cursor range
- original `CursorId`s are preserved
- movement is per-cursor independent
- per-cursor failure is a no-op for that cursor
- final results use normal `Selection` normalization

Internally, the implementation may still use small private `Range -> Range` or `usize -> usize` helpers.

---

## Relationship to render/layout code

Movement is one of the main consumers of the pure render kernel.

Vertical and visual motions depend on:
- `TextFormat`
- `TextAnnotations`
- wrap width
- document-visual mapping helpers in `render/layout_index.rs`

`render/visual_position.rs` may remain as a thin compatibility adapter, but it is not the authoritative seam.

The movement module should consume these as **immutable explicit inputs**.

It should **not**:
- mutate annotations
- own viewport state
- own a render surface
- own persistent interaction state

The visual-goal state belongs to the caller, typically as surface-local interaction state above `movement.rs`.

---

## Relationship to syntax-aware movement

The old `movement.rs` mixed plain text movement with syntax-aware behavior.

That split should stay undone.

### Keep in `movement.rs`
- grapheme left/right
- logical up/down
- visual up/down
- word / long-word / subword motions
- paragraph motions
- logical line start/end motions
- first-non-whitespace-on-line motion
- document start/end motions

### Move out of `movement.rs`
- parent-node boundary motion
- tree-sitter object jumping
- other syntax/object/query-shaped motions

### Where they go
- **structural syntax navigation** belongs in `object.rs`
- **semantic syntax object motions** belong in `text_object.rs` for now, or a later tiny dedicated syntax-motion module only if that boundary becomes unmistakable

This is a hard requirement for the port: syntax-based motions should be preserved, but rehomed to the correct modules instead of remaining inside `movement.rs`.

---

## Main decisions

### 1. Public API is selection-centric
**Decision:** yes.

Movement is a real multi-cursor editor behavior, so it should take and return `Selection`.

---

### 2. Remove the big public `Motion` enum
**Decision:** yes.

Do **not** use one large public dispatcher enum that pretends to be the editor's full motion language.

Instead, expose:
- small named functions
- small focused enums where they truly help

Reasoning:
- keeps the Rust boundary honest
- avoids freezing an oversized “motion IR” too early
- makes it obvious that TS owns the larger motion language

---

### 3. Keep `MotionMode` separate from movement primitives
**Decision:** yes.

Preferred split:
- movement primitive = where to move
- `MotionMode` = move vs extend

Reasoning:
- these are different axes
- this avoids duplicating APIs unnecessarily

---

### 4. Vertical visual-goal state is explicit
**Decision:** yes.

Movement should not hide vertical preferred-column state internally.

Preferred public types:

```rust
pub struct VisualGoal {
  pub col: u32,
}

pub struct VisualGoals { ... }
```

This state is:
- explicit
- pure
- caller-owned
- keyed per cursor

---

### 5. Visual goals are per-cursor, keyed by `CursorId`
**Decision:** yes.

Reasoning:
- multi-cursor vertical movement must preserve independent preferred columns
- shared state for the whole selection would be wrong

---

### 6. Store only preferred visual column
**Decision:** yes.

Preferred shape:

```rust
pub struct VisualGoal {
  pub col: u32,
}
```

Do **not** store persistent row goals.

Reasoning:
- row is transient per move
- column is the stable semantic goal
- simpler public model

---

### 7. Non-vertical motions clear visual goals for moved cursors
**Decision:** yes.

Reasoning:
- after horizontal/word/paragraph/line/document movement, the new cursor position should become the basis for future up/down motion
- stale preferred columns are surprising

Vertical motions update goals.
Non-vertical motions clear goals for the affected cursor IDs.

---

### 8. Use one unified movement context
**Decision:** yes.

Even though many motions do not need layout data, the public functions should take one explicit movement context containing the render/layout inputs.

Reasoning:
- simpler call sites
- uniformity across core movement primitives
- movement already naturally depends on layout in some cases

---

### 9. `Up`/`Down` and `UpVisual`/`DownVisual` stay distinct
**Decision:** yes.

Use this split:
- logical up/down = logical line motion
- visual up/down = wrapped visual-row motion

Reasoning:
- explicit semantics
- avoids ambiguity under soft wrap

---

### 10. Include classic text-local anchor motions
**Decision:** yes.

Include:
- logical line start
- logical line end
- first non-whitespace on logical line
- document start
- document end

These belong naturally in `movement.rs` and help reach a working editor faster.

---

### 11. Line anchor motions are logical-line-only for the first port
**Decision:** yes.

Do **not** include wrapped visual-row home/end in the first movement port.

Reasoning:
- logical line semantics are foundational and obvious
- visual-row home/end can be added later if needed

---

### 12. Multi-cursor behavior is per-cursor independent
**Decision:** yes.

If one cursor cannot move meaningfully, that cursor is a no-op.
The operation does not fail for the whole selection.

Reasoning:
- ordinary movement should not be brittle in multi-cursor cases

---

## Ideal public interface

The exact implementation can vary, but the public API should feel close to this:

```rust
use ropey::RopeSlice;

use crate::{
  render::{TextAnnotations, TextFormat},
  selection::{CursorId, Selection},
};

pub enum Direction {
  Forward,
  Backward,
}

pub enum MotionMode {
  Move,
  Extend,
}

pub enum WordKind {
  Word,
  LongWord,
  SubWord,
}

pub enum WordTarget {
  Start,
  End,
}

pub struct VisualGoal {
  pub col: u32,
}

pub struct VisualGoals { ... }

pub struct MovementContext<'a> {
  pub text_format: &'a TextFormat,
  pub annotations: &'a TextAnnotations,
  pub wrap_width: usize,
}

pub struct MotionResult {
  pub selection: Selection,
  pub visual_goals: VisualGoals,
}

pub fn move_left(... ) -> MotionResult;
pub fn move_right(... ) -> MotionResult;

pub fn move_up(... ) -> MotionResult;
pub fn move_down(... ) -> MotionResult;
pub fn move_up_visual(... ) -> MotionResult;
pub fn move_down_visual(... ) -> MotionResult;

pub fn move_next_word(..., kind: WordKind, target: WordTarget, ...) -> MotionResult;
pub fn move_prev_word(..., kind: WordKind, target: WordTarget, ...) -> MotionResult;

pub fn move_prev_paragraph(... ) -> MotionResult;
pub fn move_next_paragraph(... ) -> MotionResult;

pub fn move_line_start(... ) -> MotionResult;
pub fn move_line_end(... ) -> MotionResult;
pub fn move_first_non_whitespace(... ) -> MotionResult;

pub fn move_document_start(... ) -> MotionResult;
pub fn move_document_end(... ) -> MotionResult;
```

Notes:
- `MotionMode::Move` collapses to a cursor move
- `MotionMode::Extend` extends from the existing anchor
- functions that ignore layout still receive `MovementContext`
- non-vertical motions clear visual goals for moved cursors
- vertical motions update visual goals for moved cursors
- TS composes larger motions from these primitives and other modules

---

## Internal structure

The module should still be decomposed internally.

A good internal split is:
- public selection-centric wrappers
- horizontal grapheme helpers
- vertical logical helpers
- vertical visual helpers
- word-boundary helpers
- paragraph helpers
- line/document anchor helpers
- visual-goal helpers

The public API should stay small and honest even if the implementation shares many private helpers.

---

## What to keep from the old pasted file

Keep, conceptually:
- `Direction`
- separate logical vs visual vertical behavior
- word / long-word / subword boundary semantics
- paragraph movement semantics
- explicit move vs extend behavior
- grapheme-aware horizontal motion

Rewrite:
- public API shape
- selection-centric behavior
- visual-goal handling
- render helper integration to current `render/layout_index.rs` document-visual mapping seam
- annotation handling to immutable `TextAnnotations`
- tests to current behavior-first style

Move out:
- `move_parent_node_end(...)`
- `goto_treesitter_object(...)`
- old syntax loader/query coupling

---

## What should happen to the old syntax-aware functions

### `move_parent_node_end(...)`
This is structural syntax navigation.
It should be ported into `object.rs` under a name and API that fit that module's modern structural-selection boundary.

### `goto_treesitter_object(...)`
This is semantic syntax-object motion.
It should be ported on top of the modern syntax/text-object stack, most likely in `text_object.rs` for now, or a tiny dedicated syntax-motion module later if the vocabulary grows large enough.

These functions should not remain as public API in `movement.rs`.

---

## Test priorities

Tests should focus on behavior, not implementation details.

Priority coverage:

### Horizontal motion
- forward/backward grapheme movement
- multi-grapheme counts
- boundary no-op behavior
- extend vs move semantics

### Vertical motion
- logical up/down preserves preferred visual column
- visual up/down follows wrapped rows
- per-cursor independent visual goals
- non-vertical motions clear visual goals
- multi-cursor vertical motion preserves cursor IDs

### Word motion
- word start/end
- long-word start/end
- subword camelCase and underscore behavior
- newline/whitespace edge cases
- counts and boundary no-op behavior

### Paragraph motion
- blank-line separated paragraphs
- cursor already at boundary
- move vs extend semantics

### Line/document anchors
- line start/end
- first non-whitespace
- document start/end
- CRLF/newline handling

### Syntax-motion rehoming
Separate tests should be added in the destination modules for the rehomed syntax-based motions.

---

## Non-goals

`movement.rs` should **not** become:
- the editor's entire motion abstraction
- a syntax tree navigation dump
- a text-object command grammar
- a runtime-owned interaction-state container
- a surface/view object
- a generic command dispatcher for unrelated editor actions

Keep it about core cursor travel through text and layout.
