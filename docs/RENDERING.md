# Cell-grid Rendering Architecture

This document is the authoritative rendering plan for the editor's built-in **monospace cell-grid renderer**.

The old fast renderer got one big thing right:

> cell-grid rendering should be **viewport-first**.

The current retained renderer still has one deep mistake:

> it sometimes asks the formatter to rediscover global layout truth.

That is the remaining problem.

This document replaces that model.

---

## Scope

This renderer assumes a uniform cell grid:

- horizontal layout is expressed in visual columns
- vertical layout is expressed in surface rows
- wrapping width is a column count
- tabs, wrap indentation, selections, cursors, and syntax spans are projected through column math
- pixel annotations are derived from `CellGridMetrics`

That makes the renderer appropriate for TUIs, REPLs, tests, and monospace GUI code-editor clients. It deliberately does **not** try to model proportional fonts, browser/native shaping, variable line heights, or framework-owned rich text layout. Clients that need those capabilities should own that layout path instead of treating this renderer as universal.

---

## Final position

Cell-grid rendering is built from **two different native modules**:

1. **`LayoutIndex`** — persistent global layout knowledge
2. **`VisibleLayout`** — transient visible-window paint truth

That split is the architecture.

Rust owns the built-in cell-grid path:

- monospace text layout
- column-based wrap decisions
- document-visual-row indexing
- widget row indexing
- cell-grid viewport math
- visible row building
- retained row/layer diffing
- damage tracking
- projection of selections/cursors/syntax into row-local payloads
- cursor placement semantics (`InsertionPoint` for bar carets, `Cell` for block/underline cursors)

Clients own:

- painting
- terminal / GPU / native UI details
- presentation scenes
- frame pacing
- any non-cell-grid text layout path

TypeScript owns:

- composition
- provider policy
- surface orchestration
- widget providers / decoration providers / product UI

---

## Public API shape

The render story should expose **three Seams** on `Surface`.

### 1. View configuration seam
Callers should configure one view object instead of juggling many setters.

```rust
pub struct SurfaceViewSpec {
  pub viewport: Viewport,
  pub format: TextFormat,
  pub measurements: CellGridMetrics,
  pub cursor_semantics: CursorSemantics,
  pub annotations: TextAnnotations,
  pub widgets: Vec<BlockWidget>,
}

impl Surface {
  pub fn view_spec(&self) -> SurfaceViewSpec;
  pub fn set_view(&mut self, spec: SurfaceViewSpec);
}
```

This keeps render inputs composable while concentrating them behind one Interface.

### 2. Snapshot seam
For text inputs, REPLs, tests, and simple clients:

```rust
impl Surface {
  pub fn snapshot(
    &mut self,
    document: &Document,
    syntax_registry: Option<&LanguageRegistry>,
  ) -> SurfaceFrame;
}
```

`snapshot()` returns the whole visible logical frame:

- visible rows
- row metadata
- selections
- cursors
- syntax spans
- widgets

No client-side delta protocol is required.

### 3. Retained present seam
For normal high-performance clients:

```rust
impl Surface {
  pub fn present(
    &mut self,
    document: &Document,
    syntax_registry: Option<&LanguageRegistry>,
    retained: &mut RetainedFrame,
  ) -> PresentUpdate;
}
```

`RetainedFrame` is library-owned logical retained state.
The client paints from it after each `PresentUpdate`.

The client story becomes:

```rust
surface.set_view(view);
let update = surface.present(document, syntax, &mut retained);
for row in update.redraw_rows(row_count) {
  paint(retained.row(row), retained.selection_spans(row), retained.cursor_placements(row));
}
```

This is still fully client agnostic: Rust owns logical retained presentation state; the client still owns actual painting.

### 4. Raw delta seam
For expert clients and compatibility paths:

```rust
impl Surface {
  pub fn raw_frame_delta(
    &mut self,
    document: &Document,
    syntax_registry: Option<&LanguageRegistry>,
    since_generation: Option<u64>,
  ) -> RenderFrameDelta;
}
```

This preserves the full retained protocol without making it the only public story.

### Runtime convenience
`RuntimeSession` should expose the same shape over handles:

```rust
impl RuntimeSession {
  pub fn snapshot_surface(&mut self, surface: SurfaceHandle) -> Result<SurfaceFrame>;
  pub fn present_surface(
    &mut self,
    surface: SurfaceHandle,
    retained: &mut RetainedFrame,
  ) -> Result<PresentUpdate>;
}
```

The runtime still handles syntax scheduling and demand tracking before rendering.

---

## Core decisions

### 1. No document-wide formatted row cache
There is no architecture centered on:

- formatting the whole document
- caching every formatted row
- slicing the viewport from that cache on every frame

That model is rejected.

### 2. `LayoutIndex` is the seam for global mapping
All off-window mapping goes through `LayoutIndex`, not through ad hoc formatter walks.

Examples:

- char -> document visual row
- char -> surface row
- line -> document visual start row
- line -> surface row
- surface row -> nearest line
- wrapped row count per line
- widget row offsets before/after lines
- nearest resume point for visible layout build

`DocumentFormatter` is not the public oracle for these questions.

### 3. `VisibleLayout` is the seam for local paint truth
`VisibleLayout` is the exact formatted result for the current viewport plus small overscan.

Everything visible derives from it:

- visible rows
- row metadata seeds
- selection spans
- cursor placements
- syntax spans
- widget placements in the window

No extra formatter pass per layer.

### 4. `LayoutIndex` primary truth is document visual space
Primary indexed truth is:

- document text
- wrap policy
- inline inserted text
- grapheme substitutions

This produces **document visual rows**.

Widgets are not part of primary text layout truth.
They are applied second to derive **surface row space**.

### 5. Widgets stay; virtual lines in text annotations stay dead
Block widgets remain real inserted render rows.

Virtual lines do not return to `TextAnnotations`.

### 6. `TextAnnotations` is narrow
`TextAnnotations` means only:

- inline inserted text
- grapheme substitutions

It does not mean:

- row insertion
- virtual lines
- broad decoration systems

### 7. `LayoutIndex` is incremental across edits
Typing must not destroy global layout knowledge.

Small edits should update only the affected line metrics, downstream row counts, and relevant resume points.

The model is:

```txt
text changed -> incrementally repair LayoutIndex
```

not:

```txt
text changed -> clear layout knowledge -> rediscover by formatter walk
```

### 8. Whole-document visual helpers are not the hot path
Rendering, reveal, and movement should use:

- `LayoutIndex` for global mapping
- `VisibleLayout` for local visible truth

Whole-document formatter walks are fallback implementation details only.

---

## The render pipeline

The correct pipeline is:

```txt
Document text
  + TextFormat
  + TextAnnotations
  + widgets
    -> LayoutIndex (persistent global layout knowledge)
    -> VisibleLayoutBuilder builds viewport ± margin
    -> RowDiffEngine diffs visible rows
    -> LayerProjector projects selections/cursors/syntax/widgets
    -> RenderFrameDelta
```

Not:

```txt
Document text
  -> formatter walk until we rediscover target rows
  -> build visible rows
  -> repeat when movement/reveal/features change
```

---

## The two core internal objects

## `LayoutIndex`

`LayoutIndex` is the persistent global layout module.

Recommended file:

- `the-lib/render/layout_index.rs`

This is now the deep module for global mapping.

### `LayoutIndex` owns

- layout signature / staleness checks
- per-line layout metrics
- widget row indexing
- sparse resume points/origins
- global mapping queries
- incremental maintenance across edits

### `LayoutIndex` answers

- where is this char globally?
- what line owns this char?
- what document visual row starts at this line?
- what surface row starts at this line?
- what line is nearest this surface row?
- how many wrapped visual rows does this line occupy?
- how many widget rows precede this line?
- where should visible layout building resume?
- document-visual movement helpers like char <-> visual row/col mapping

### `LayoutIndex` does **not** own

- visible row text for the current frame
- retained row ids/generations
- row-local layer payloads
- client painting

It is global layout knowledge, not a visible frame product.

## `VisibleLayout`

`VisibleLayout` is the exact visible formatting product for the current viewport plus overscan.

Recommended file:

- `the-lib/render/visible_layout.rs`

### `VisibleLayout` owns

- visible row records
- row identity key candidates
- visible row text
- row metadata seeds
- visible units / fine-grained visible mapping
- cursor landing positions in the window
- widget placements in the window
- local facts needed by row diffing and layer projection

### `VisibleLayout` does **not** own

- global mapping truth
- persistent row-height knowledge for the whole document
- global widget offset knowledge
- whole-document layout state

It is the local paint truth.

---

## Coordinate model

The renderer works in two explicit spaces.

### 1. Document visual space
Produced from:

- document text
- wrap policy
- inline inserted text
- grapheme substitutions

This space does **not** include widget row insertion.

This is the correct semantic space for:

- movement
- reveal targets
- cursor semantics
- selection semantics
- editing semantics
- global indexing in `LayoutIndex`

### 2. Surface row space
Produced from:

- document visual rows
- widget row insertion

This is the correct space for:

- retained visible rows
- row ids
- row metadata
- painting
- widget placement
- row-local layer payloads

### Rule
Widgets are real render-row insertions, but they are **non-navigable**.

`LayoutIndex` owns the global derivation from document visual space to surface row space.
`VisibleLayout` owns the exact local visible result.

---

## Per-line metrics

The core unit of global layout indexing is the logical line.

Recommended internal shape:

```txt
LineLayoutMetrics
  - line_idx
  - start_char
  - end_char
  - visual_row_start
  - visual_row_count
```

Why line-based:

- no-wrap: `visual_row_count = 1`
- soft-wrap: row count varies per line
- reveal/movement usually begins from line knowledge
- edits naturally invalidate locally by line range

This is the right granularity for leverage and locality.

---

## Widget row indexing

Widgets are derived second.

Recommended internal shape:

```txt
WidgetIndex
  - rows_before_line
```

The important global queries are:

- widget rows before line
- surface row for line
- nearest line for surface row

This must not be rediscovered by formatter traversal.

---

## Origins and resume points

Origins remain useful, but they are no longer the architecture.

They are subordinate implementation details inside `LayoutIndex`.

### Purpose

- help `VisibleLayoutBuilder` start near the viewport
- support lazy recomputation
- support off-window fallback repair work

### Useful origin fields

- char index
- line index
- document visual row
- signature/version facts as needed

Surface row should be derived from widget indexing where possible, not treated as primary origin truth.

---

## `CellGridRenderSurface` responsibilities

`CellGridRenderSurface` owns only retained visible rendering concerns:

- visible-window retained rows
- visible row ids and generations
- retained visible layer payloads
- damage tracking
- row metadata deltas
- incremental frame delta production
- `VisibleLayout` for the current window

`CellGridRenderSurface` does **not** own:

- global layout truth
- whole-document row mapping
- primary origin/indexing policy
- syntax query policy
- document-owned syntax state

`CellGridRenderSurface::render_frame_stateless(...)` exists only as a tests/compatibility fallback; the real incremental path is the persistent `LayoutIndex`-backed path.

---

## Builder / diff / projection split

The implementation now converges on four internal modules.

### `layout_index.rs`
Owns:

- `LayoutIndex`
- `LineLayoutMetrics`
- widget indexing
- origin indexing
- incremental update logic
- global mapping queries

### `visible_layout.rs`
Owns:

- `VisibleLayout`
- visible row/unit types
- `VisibleLayoutBuilder`
- one shared visible formatting pass

### `row_diff.rs`
Owns:

- retained/changed/removed row diffing
- row metadata delta construction

### `layer_projection.rs`
Owns:

- selection projection
- cursor projection
- syntax projection
- widget projection

This is the correct module graph.

---

## Layer projection

The layer story stays simple.

### Rows
Rows come directly from `VisibleLayout`.

### Row metadata
Seeded from visible rows and diffed incrementally.

### Selection layer
Projected from `VisibleLayout` only.

### Cursor layer
Projected from `VisibleLayout` only.

The cell-grid renderer distinguishes cursor placement semantics from cursor drawing style:

```rust
pub enum CursorSemantics {
  InsertionPoint, // edge-based, suitable for a vertical bar caret
  Cell,           // cell-based, suitable for block/underline cursors
}
```

`CellGridRenderSurface` emits `CursorPlacement { width_px, height_px, semantics, ... }`. Clients still own shape, blink, color, theme, and animation, but they no longer have to guess whether the placement refers to an insertion edge or a cell.

### Syntax layer
Syntax provides document highlight runs.
Render projects them through `VisibleLayout` into row-local `SyntaxSpan`s.

### Widget layer
Widgets are explicit inserted surface rows and emit visible-window widget placements.

No layer gets its own formatter walk.

---

## Syntax/render boundary

Syntax owns:

- language assignment
- parse/injection/enrichment truth
- document highlight runs for visible interest ranges

Render owns:

- global layout indexing
- visible row layout
- row-local projection into `SyntaxSpan`
- retained syntax layer diffing

So the contract is:

```txt
syntax -> document highlight runs
render -> visible row-local syntax spans
```

---

## Reveal and movement

### Global step
Reveal and movement ask `LayoutIndex` first.

The document-visual helpers now live in `layout_index.rs`; `render/visual_position.rs` is only a thin compatibility adapter.

That covers:

- off-window cursor reveal
- viewport start selection
- far jumps
- typing after edits
- movement in large files

### Local step
If the target is inside or near the current window, use `VisibleLayout` for exact local mapping.

### Rule
`DocumentFormatter` is not the public answer to movement or reveal.

---

## Damage and generations

The retained renderer still emits:

- changed rows
- retained rows / row moves
- removed rows
- row metadata deltas
- layer deltas
- damage
- surface generation
- row generations

That output shape stays.

What changes is the producer architecture:

- `LayoutIndex` for global knowledge
- one `VisibleLayout` build for the current window
- retained visible diffing after that

---

## What is explicitly rejected

The following ideas are rejected:

- full document-wide formatted row cache as the architecture center
- using `VisibleLayout` as global layout truth
- formatter pass per visual layer
- virtual lines inside `TextAnnotations`
- treating widgets as text navigation rows
- whole-document visual helper APIs as the normal hot path
- movement/reveal/render using different row models

---

## Implementation shape

The implementation should proceed in this order.

### 1. Introduce `LayoutIndex`
Add the persistent global layout module.

### 2. Add per-line metrics and widget index
This is the core global knowledge.

### 3. Move global mapping out of `CellGridRenderSurface`
Reveal and movement should stop depending on formatter traversal.

### 4. Keep `VisibleLayout` viewport-local
It remains the one shared visible formatting pass.

### 5. Split builder / diff / projection modules
Concentrate locality by responsibility.

### 6. Re-profile large files
Especially:

- cold reveal to EOF
- typing after edits
- widgets enabled
- annotations enabled
- soft wrap enabled

---

## Success criteria

### Correctness

- reveal and paint use the same row truth
- no cursor/render drift
- no wrong insertion positions
- widgets do not poison navigation semantics
- soft wrap, annotations, and widgets compose soundly

### Performance

- huge-file no-wrap typing is fast
- huge-file no-wrap reveal is fast
- widgets do not reintroduce global formatter walks
- annotations do not reintroduce global formatter walks
- soft-wrap performance depends on indexed per-line metrics, not cold document walks

### API quality

- `LayoutIndex` feels like the obvious seam for global mapping
- `VisibleLayout` feels like the obvious seam for visible paint truth
- `CellGridRenderSurface` feels obviously viewport-first and retained
- `TextAnnotations` stays narrow and honest
- widgets remain the only row insertion concept

---

## One-line summary

The built-in cell-grid rendering engine should be a **viewport-first retained visible-window renderer built on top of a persistent incremental `LayoutIndex`**, exposed through layered public Seams: `set_view()` for configuration, `snapshot()` for simple clients, `present()` for normal retained clients, and `raw_frame_delta()` for expert clients.