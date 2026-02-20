# Window Splitting Plan

## Goal

Add first-class pane/window splitting with Helix-like `C-w` commands:

- rotate/split/transpose/close/only
- directional jump/swap
- file-open in split (horizontal/vertical)
- scratch split creation

The behavior must be deterministic and shared across `the-term` and `the-swift`.

## Scope and Principles

1. `the-lib` owns pane/split editor state and operations.
2. `the-default` maps key/command semantics to `the-lib` operations.
3. Clients (`the-term`, `the-swift`) render pane geometry and visuals only.
4. No client-specific split state that can diverge from core behavior.
5. Keep migration incremental with temporary compatibility shims.

## Design Inputs

- Helix: `helix-view/src/tree.rs` and command behavior from `helix-term`.
- Ghostty: split tree + spatial navigation patterns and test discipline.
- Project docs:
  - `docs/THE_LIB_PLAN.md`
  - `docs/CRATE_STRUCTURE.md`
  - `docs/COMMANDS_PORTING.md`
  - `docs/DISPLAY_LIST_VS_STATE.md`

## Target Command Set

`C-w` submap:

- `C-w|w`: rotate view
- `C-s|s`: horizontal split
- `C-v|v`: vertical split
- `C-t|t`: transpose view tree at focus branch
- `f`: goto file in horizontal split
- `F`: goto file in vertical split
- `C-q|q`: close active view
- `C-o|o`: keep only active view
- `C-h|h|left`: jump left
- `C-j|j|down`: jump down
- `C-k|k|up`: jump up
- `C-l|l|right`: jump right
- `H/J/K/L`: swap with sibling in direction
- `n s` / `n v`: new scratch split (h/v)

## Core Model (`the-lib`)

Add a split tree and pane model to `Editor`.

### Types

- `PaneId` (stable identity)
- `SplitNodeId` (tree identity)
- `SplitAxis` (`Horizontal`, `Vertical`)
- `SplitNode`:
  - `Leaf(PaneId)`
  - `Branch { axis, ratio, first, second }`
- `PaneState`:
  - `doc_id`
  - `view_state` (scroll/cursors/select state)
  - per-pane render cache handle
- `Editor`:
  - split root
  - active pane id
  - pane storage map

### Required Operations

- `split_active(axis) -> PaneId`
- `split_active_with_doc(axis, doc_id) -> PaneId`
- `split_active_scratch(axis) -> PaneId`
- `close_active() -> Result<()>`
- `only_active()`
- `rotate_focus(next: bool)`
- `transpose_active_branch()`
- `jump_active(direction) -> bool`
- `swap_active(direction) -> bool`
- `set_active_pane(pane_id)`
- `pane_count()`

### Layout

Pure geometry function:

- input: root rect, split tree
- output: `Vec<(PaneId, Rect)>`
- guarantees:
  - non-overlapping leaf rects
  - full area coverage
  - min-size clamping behavior is deterministic

## Render Pipeline Changes

Current render path assumes one active pane. Migrate to a frame-wide plan:

- `FrameRenderPlan { panes: Vec<PaneRenderPlan>, active_pane }`
- `PaneRenderPlan { pane_id, rect, plan }`

Where `plan` is existing document render plan for that pane.

### Compatibility Layer

During migration, keep helper API for single-pane callers:

- `build_active_render_plan()` backed by active pane from frame plan.

This allows staged client updates without blocking core work.

## Command Layer (`the-default`)

Implement `C-w` commands as thin orchestration:

1. call `the-lib` pane/tree operation
2. set status/error when operation is invalid
3. trigger layout/render refresh

No structural split logic should live in `the-default`.

## Client Responsibilities

### the-term

- draw each pane using rect from frame plan
- render split separators/borders
- highlight active pane
- keep prompt/overlay behavior stable on top of split content

### the-swift (via `the-ffi`)

- consume frame plan panes
- render per-pane text views in native layout containers
- style active pane distinctively
- ensure focus/input routes to active pane id

## Behavioral Rules

1. Splitting clones current doc/view state into new pane.
2. Focus moves to the newly created pane.
3. `wclose` on last pane errors (no-op).
4. `wonly` keeps only active pane and preserves its state.
5. Directional jump/swap uses geometric nearest-neighbor, not insertion order.
6. Rotation cycles visible panes deterministically.

## Test Plan

### Phase tests

- Tree invariants after every edit operation.
- Geometry correctness (coverage + non-overlap).
- Command behavior tests for every `C-w` mapping.
- Edge case tests:
  - closing root/last pane
  - jump/swap with no candidate in direction
  - transpose with leaf-only branch
  - split with minimal viewport dimensions

### Cross-surface parity tests

- Same command script over same document state yields same pane tree and active pane.
- FFI snapshot tests verify `the-swift` receives expected pane geometry and active id.

## Rollout Phases

1. **Phase 1: Split Tree Core**
   - add split/pane types + invariants + unit tests.
2. **Phase 2: Multi-Pane Render Plan**
   - frame-wide render output + compatibility shim.
3. **Phase 3: Structural Commands**
   - split/close/only/rotate/transpose.
4. **Phase 4: Directional Navigation**
   - jump/swap in four directions.
5. **Phase 5: File/Scratch Split Commands**
   - `goto_file_hsplit`, `goto_file_vsplit`, `hsplit_new`, `vsplit_new`.
6. **Phase 6: Client Rendering**
   - `the-term` and `the-swift` pane rendering + active-pane UX polish.
7. **Phase 7: Cleanup**
   - remove single-pane temporary paths where no longer needed.

## Commit Strategy

Use small, compiling commits (jj):

- one cohesive behavior slice per commit
- include tests in same commit for each new operation
- run `cargo check` before finishing each slice

## Acceptance Criteria

1. All listed `C-w` commands work in both `the-term` and `the-swift`.
2. Pane tree state is owned by `the-lib` and shared by both clients.
3. No client-only split behavior divergence.
4. Tree/layout invariants covered by automated tests.
5. No regressions for single-pane editing flows.
