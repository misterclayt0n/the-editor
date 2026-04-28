# Surface Module Architecture

This document defines the target architecture for the future `the-lib/surface.rs`.

It answers five questions:

1. What is a `Surface` in the new editor?
2. What belongs on `Surface`, and what does not?
3. How should `Surface` relate to `Document` and `RenderSurface`?
4. What should the main editing/session API look like?
5. Why do we need `Surface` at all instead of stopping at `Document` + `RenderSurface`?

This document is intentionally opinionated. `Surface` should be the real per-view/editor-session object in `the-lib`.

---

## Purpose

`Surface` is the pure owner of per-view/editor-session state for one live view on a document.

It is responsible for:
- owning the current `Selection`
- owning the active/primary cursor identity
- owning `VisualGoals`
- owning per-surface `History`
- owning logical viewport/scroll state
- owning reveal policy/helpers
- owning retained rendering state via `RenderSurface`
- coordinating text edits with document mutation, selection updates, history, cursor repair, visual goals, and render invalidation

It is **not** responsible for:
- owning document text truth
- owning file/resource/path identity
- owning app/pane/tab/workspace layout
- owning runtime async services
- owning save/readonly/modified policy
- defining the larger TS command/mode/product language

---

## Core position

The architecture should read like this:

```txt
Resource      = external-world identity
Document      = shared text truth
Surface       = one live per-view/editor session on that text
RenderSurface = retained rendering subsystem owned by Surface
```

A single `Document` may be shown by many `Surface`s.
That is the whole reason `Surface` exists.

If two splits show the same text, they still need independent:
- selections
- active cursors
- visual goals
- histories
- scroll positions
- retained render state

Those things therefore cannot live on `Document`.

---

## Why `Surface` exists

Without `Surface`, the architecture becomes awkward:

```txt
Document
+ RenderSurface
+ selection state
+ history
+ viewport
+ visual goals
= ???
```

That is exactly the ghostly in-between state we want to avoid.

`Surface` is the missing object that makes the architecture obvious:

```txt
Document = shared text truth
Surface = one native editing/view session on that text
RenderSurface = render subsystem inside Surface
```

This is also why the old `view.rs` should not come back as a separate legacy-shaped module.
The *concept* of per-view state is real, but the cleaner noun is `Surface`.

---

## What belongs on `Surface`

### 1. Selection
```rust
selection: Selection
```
Selection is per-view state.

### 2. Active cursor identity
```rust
active_cursor: Option<CursorId>
```
This is explicit surface state, not something callers should constantly derive from selection order.

The invariant should be simple:
- if `active_cursor` is `Some(id)`, that `id` must exist in `selection`
- otherwise the surface falls back to first-cursor behavior

### 3. Visual goals
```rust
visual_goals: VisualGoals
```
Preferred visual columns for vertical motion are real interaction/session state and belong on `Surface`.

### 4. History
```rust
history: History
```
History restores exact committed selection snapshots.
That makes it surface/session-local, not document truth.

### 5. Logical viewport / scroll state
`Surface` should own canonical logical viewport/scroll state.

This may be a small surface-specific type such as:
- `SurfaceViewport`
- `ScrollState`

It does not need to be identical to the render-layer `Viewport` request struct forever.

### 6. Reveal policy/helpers
`Surface` should own logical reveal behavior because it owns both:
- selection/cursor state
- viewport/scroll state

### 7. Retained render state
`Surface` should own a `RenderSurface`.

That means the following remain per-surface retained inputs/state:
- `TextFormat`
- `TextAnnotations`
- syntax highlight payloads
- widgets
- measurements
- retained rows/layers/cache/dirtiness

---

## What does **not** belong on `Surface`

### Document text truth
`Surface` should not own the `Document` contents.
`Document` remains the text truth object.

### Resource/runtime identity
No:
- file path
- URI
- display name
- source/resource handle
- runtime-owned document attachment metadata

### Save / readonly / modified policy
These are higher-level runtime/source/session concerns.

### App layout / pane tree / tabs / workspace
These are product composition concerns and should remain above `the-lib`.

### Full command/mode language
`Surface` is the native editing/session object, not the TS product layer.
It should orchestrate canonical editing mechanics, not define the whole editor personality.

### Surface/session identity types for now
For now, `Surface` should not introduce a new top-level `SurfaceId` in `the-lib`.
Higher layers can own identity/registry concerns.

`RenderSurface` already has a render-local `SurfaceId`; that does not force the higher-level `Surface` object to expose another ID type yet.

---

## Relationship to `Document`

Architecturally:
- one `Surface` is attached to exactly one primary `Document`

Mechanically in `the-lib` for now:
- `Surface` does **not** need to store a `DocumentId`
- `Surface` does **not** need to own a `Document`
- higher layers may pair them
- surface methods can take `&Document` / `&mut Document` as needed

This keeps the pure object graph simple while preserving the intended ownership model.

### Why not store document identity now?
Because we already decided:
- no `DocumentId` on `Document` for now
- higher-level registries belong above this layer

The important truth is the architectural attachment, not forced identity machinery in the first implementation.

---

## Relationship to `RenderSurface`

`RenderSurface` is not the whole future `Surface`.
It is the retained rendering subsystem owned by `Surface`.

That means:
- normal callers should think in terms of `Surface`
- `Surface` should usually be the render entrypoint
- `Surface` forwards into its owned `RenderSurface`

Conceptually:

```rust
pub struct Surface {
  selection: Selection,
  active_cursor: Option<CursorId>,
  visual_goals: VisualGoals,
  history: History,
  viewport: SurfaceViewport,
  render: RenderSurface,
}
```

Not necessarily exactly that, but close.

### Why own `RenderSurface` directly?
Because render state is already clearly per-surface state:
- retained row identity
- retained layout cache
- selection/cursor/syntax layers
- widgets
- measurements
- viewport-shaped rendering behavior

Keeping `RenderSurface` as a sibling peer object would create fake separation and more synchronization glue.

---

## Main responsibility

`Surface` should become the main native editing/session API.

That means typical editor operations should go through `Surface`, not manually coordinate:
- document mutation
- selection replacement
- history commit/jump
- active cursor repair
- visual goal lifecycle
- reveal behavior
- render invalidation

This keeps `Document` small while still giving the native core one honest session object.

---

## Two mutation families

### 1. Text-editing path
`Surface` should have one canonical low-level apply path for normal edits.

That path should be centered on the shared:

```rust
pub struct Edit {
  pub transaction: Transaction,
  pub selection: Selection,
}
```

So `Surface` should likely expose something like:
- `apply_edit(document, edit)`
- maybe lower-level `apply_transaction_and_selection(document, transaction, selection)`

That path should:
1. apply the transaction to `Document`
2. replace the surface selection
3. commit history
4. repair `active_cursor`
5. clear/repair `VisualGoals`
6. reveal as needed
7. sync/invalidate owned render state

This should be the canonical path used by pure planners like:
- `auto_pairs`
- `comment`
- future editing helpers returning `Edit`

### 2. Selection-only path
`Surface` also needs selection/session mutation paths that do **not** edit text.

Examples:
- movement
- text objects
- object expansion/shrinking
- bracket jumps
- search landing
- reveal-only cursor motion

Those operations should still update surface-owned state such as:
- selection
- active cursor
- visual goals
- viewport/reveal
- render selection/cursor layers

But they should not mutate document text or commit text history.

---

## Active cursor semantics

`Surface` should expose small public helpers for primary/active cursor semantics.

Examples:
- `active_cursor_id()`
- `primary_cursor()`
- `primary_range()`
- `primary_index()`

The fallback policy should be explicit and centralized:
- use `active_cursor` if it is valid
- otherwise use the first cursor in selection

### Repair on mutation
When selection changes, especially after edits:
- keep the old active cursor if its `CursorId` still exists
- otherwise choose a sensible replacement
- never leave dangling active-cursor state

This belongs on `Surface`, not on every caller.

---

## Visual goal semantics

`Surface` should own visual-goal lifecycle.

That means:
- vertical motion updates `VisualGoals`
- non-vertical motions usually clear or repair them
- text edits usually clear or repair them

Low-level movement helpers may still return updated goal information, but `Surface` should own the stored state and commit lifecycle.

---

## Viewport and reveal semantics

`Surface` should own canonical logical viewport/scroll state.

Render still lowers that state into render-layer `Viewport` requests, but the editor/session truth should live above render.

`Surface` should also own reveal helpers/policies such as:
- do nothing
- minimal scroll to keep cursor visible
- center cursor
- other explicit reveal modes

These are still pure editor/view behaviors and belong with surface state.

---

## Rendering semantics

Normal callers should render through `Surface`.

That means `Surface` should likely expose things like:
- access to render settings/state when needed
- surface-level render calls that use the owned `RenderSurface`
- viewport-setting APIs above the raw render request

This makes `Surface` feel like the real per-view object rather than forcing callers to juggle two peer objects for one view.

`RenderSurface` may remain public and focused, but it should increasingly feel like a subsystem.

---

## Ideal public shape

The exact implementation can vary, but the surface API should feel roughly like this:

```rust
pub struct Surface {
  selection: Selection,
  active_cursor: Option<CursorId>,
  visual_goals: VisualGoals,
  history: History,
  viewport: SurfaceViewport,
  render: RenderSurface,
}

impl Surface {
  pub fn new(...) -> Self;

  pub fn selection(&self) -> &Selection;
  pub fn set_selection(&mut self, selection: Selection);

  pub fn active_cursor(&self) -> Option<CursorId>;
  pub fn set_active_cursor(&mut self, cursor: Option<CursorId>);

  pub fn visual_goals(&self) -> &VisualGoals;
  pub fn history(&self) -> &History;
  pub fn history_mut(&mut self) -> &mut History;

  pub fn viewport(&self) -> &SurfaceViewport;
  pub fn set_viewport(&mut self, viewport: SurfaceViewport);

  pub fn apply_edit(&mut self, document: &mut Document, edit: Edit) -> Result<()>;
  pub fn apply_selection(&mut self, document: &Document, selection: Selection);

  pub fn render(&mut self, document: &Document, since_generation: Option<u64>) -> RenderFrameDelta;
}
```

This is illustrative, not frozen.

---

## Why `Surface` should live in `the-lib`

`Surface` should be a real pure `the-lib` object because the state it owns is still pure editor state:
- selection
- history
- viewport
- reveal logic
- visual goals
- retained render state

These are not runtime services.
These are not product chrome.
These are editor semantics.

If we stop at `Document` + `RenderSurface`, the missing session object just reappears elsewhere under another name.
That would recreate the old `view.rs` problem in a less honest form.

---

## What `Surface` is not

`Surface` is not:
- a pane tree
- a tab
- a workspace
- an app window
- a runtime document registry
- a source/resource object
- the full TS command system

It is the native per-view/editor-session object.

---

## Test priorities

When implemented, tests should focus on behavior.

Priority coverage:
- selection-only updates preserve surface invariants
- active cursor fallback/repair is correct
- edit application mutates document + selection + history coherently
- vertical vs non-vertical motion updates visual goals correctly
- reveal helpers adjust viewport sanely
- render state stays in sync with selection/viewport/render inputs
- two surfaces can operate independently on the same document state model

---

## Non-goals

`surface.rs` should **not** become:
- a second document model
- a runtime/resource/session registry
- a pane/tab/app layout system
- a giant god object for every editor concern

It should stay focused on one job:

> one live editing/view session on one document
