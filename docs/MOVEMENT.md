# Movement Module Architecture

This document defines the target architecture for `the-lib/movement.rs`.

It answers four questions:

1. What belongs in `movement.rs` in the new editor?
2. What should the public API feel like?
3. How should movement relate to render/layout state?
4. What should be kept, rewritten, or moved out of the old pasted implementation?

This document is intentionally opinionated. `movement.rs` should be the pure home for cursor travel through text and layout, not a catch-all for every command that feels like a motion.

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
- syntax tree structural navigation
- syntax-query object motions
- syntax lifecycle state
- runtime/UI/history integration
- hidden mutable interaction state

---

## Core position

`movement.rs` is a **pure selection-transform module with explicit motion state**.

It inspects:
- document text
- a `Selection`
- one `Motion`
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
Movement:     cursor travel through text/layout
Object:       structural syntax navigation
TextObject:   semantic object targeting
```

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
- visual row/column mapping helpers in `render/visual_position.rs`

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

That split should be undone.

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

This is a hard requirement for the port: those syntax-based motions should be preserved, but rehomed to the correct modules instead of remaining inside `movement.rs`.

---

## Main decisions

### 1. Public API is selection-centric
**Decision:** yes.

Movement is a real multi-cursor editor behavior, so it should take and return `Selection`.

---

### 2. Use a central `Motion` dispatcher API
**Decision:** yes.

Preferred shape:

```rust
move_selection(..., motion, mode, ...)
```

Reasoning:
- movement is one coherent vocabulary
- a dispatcher enum is appropriate here
- it keeps call sites uniform
- it makes future TS command routing simpler

---

### 3. Keep `MotionMode` separate from `Motion`
**Decision:** yes.

Preferred split:
- `Motion` = where to move
- `MotionMode` = move vs extend

Reasoning:
- these are different axes
- this avoids duplicating motion variants

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

Even though many motions do not need layout data, the dispatcher should take one explicit movement context containing the render/layout inputs.

Reasoning:
- simpler call sites
- one coherent API
- movement already naturally depends on layout in some cases

---

### 9. `Up`/`Down` and `UpVisual`/`DownVisual` stay distinct
**Decision:** yes.

Use this split:
- `Up` / `Down` = logical line motion
- `UpVisual` / `DownVisual` = wrapped visual-row motion

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

pub enum Motion {
  Left { count: usize },
  Right { count: usize },

  Up { count: usize },
  Down { count: usize },
  UpVisual { count: usize },
  DownVisual { count: usize },

  NextWordStart { count: usize },
  NextWordEnd { count: usize },
  PrevWordStart { count: usize },
  PrevWordEnd { count: usize },

  NextLongWordStart { count: usize },
  NextLongWordEnd { count: usize },
  PrevLongWordStart { count: usize },
  PrevLongWordEnd { count: usize },

  NextSubWordStart { count: usize },
  NextSubWordEnd { count: usize },
  PrevSubWordStart { count: usize },
  PrevSubWordEnd { count: usize },

  PrevParagraph { count: usize },
  NextParagraph { count: usize },

  LineStart,
  LineEnd,
  FirstNonWhitespace,

  DocumentStart,
  DocumentEnd,
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

pub fn move_selection(
  text: RopeSlice,
  selection: Selection,
  motion: Motion,
  mode: MotionMode,
  visual_goals: &VisualGoals,
  cx: &MovementContext<'_>,
) -> MotionResult;
```

Notes:
- `MotionMode::Move` collapses to a cursor move
- `MotionMode::Extend` extends from the existing anchor
- motions that ignore layout still receive `MovementContext`
- non-vertical motions clear visual goals for moved cursors
- vertical motions update visual goals for moved cursors

---

## Internal structure

The module should still be decomposed internally.

A good internal split is:
- dispatcher
- horizontal grapheme helpers
- vertical logical helpers
- vertical visual helpers
- word-boundary helpers
- paragraph helpers
- line/document anchor helpers
- visual-goal helpers

The public API should stay centralized even if the implementation is factored into private helpers.

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
- render helper integration to current `render/visual_position.rs`
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
- a syntax tree navigation dump
- a text-object command grammar
- a runtime-owned interaction-state container
- a surface/view object
- a generic command dispatcher for unrelated editor actions

Keep it about cursor motion through text and layout.
