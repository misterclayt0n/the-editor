# Rendering Architecture Plan

This document records the rendering direction for the new editor.

The short version:

- Rust should own expensive editor-specific layout, dirty tracking, and render-delta production.
- Clients should own measurement backends, retained presentation scenes, and actual painting.
- JS/TS should own composition, extension policy, decoration providers, widget providers, and surface orchestration.
- The Rust/client boundary should not be a full rebuilt snapshot every frame.
- Rendering should be retained, incremental, layered, and surface-oriented.

This is based on the old editor's rendering architecture and the lessons learned from it.

## Background

The old editor used a snapshot/display-list style renderer. Rust produced a `RenderPlan`/`FrameRenderPlan`, and clients such as the terminal client and Swift client consumed that output.

That was directionally correct. A backend-neutral render plan is useful because it allows multiple clients:

- terminal
- SwiftUI/AppKit/Metal
- web/canvas
- GPU/wgpu
- test renderer

However, the old implementation became expensive because the render product was usually treated like a fresh per-frame snapshot. Scrolling or small visual changes could cause broad recomputation and broad FFI export.

The old editor already had some of the right concepts:

```rust
pub struct RenderPlan {
  pub viewport: Rect,
  pub scroll: Position,
  pub content_offset_x: u16,

  pub layout_generation: u64,
  pub text_generation: u64,
  pub decoration_generation: u64,
  pub cursor_generation: u64,
  pub scroll_generation: u64,
  pub theme_generation: u64,

  pub damage_start_row: u16,
  pub damage_end_row: u16,
  pub damage_is_full: bool,
  pub damage_reason: RenderDamageReason,

  pub gutter_columns: Vec<RenderGutterColumn>,
  pub visible_rows: Vec<RenderVisibleRow>,
  pub gutter_lines: Vec<RenderGutterLine>,
  pub lines: Vec<RenderLine>,
  pub cursors: Vec<RenderCursor>,
  pub selections: Vec<RenderSelection>,
  pub overlays: Vec<OverlayNode>,
}
```

It also had generation tracking, damage metadata, a `RenderCache`, and special reuse paths for vertical scroll/resize. Those ideas should be kept.

The issue was that incrementality was bolted onto a per-frame `build_plan(...)` model instead of being the foundation.

## Main decision

Rendering should be split into two stages.

```txt
Stage 1: editor layout / display-list construction
  Owner: Rust

Stage 2: painting / presentation
  Owner: client
```

### Stage 1: Rust layout and display list construction

Rust should compute a structured, client-neutral description of what is visible.

Rust owns:

- text layout
- viewport math
- visual line mapping
- wrapping
- folds
- virtual lines/block annotations
- text run measurement, grapheme positioning, and visual geometry using the active measurement backend
- syntax highlight range application
- decoration merging and priority resolution
- selection/cursor logical geometry
- gutter semantic content
- dirty region tracking
- retained per-surface layout caches

### Stage 2: client painting

The client owns actual rendering.

Clients own:

- terminal cell diffing / ANSI output
- AppKit/Metal drawing
- SwiftUI chrome
- GPU buffers
- glyph atlas
- font shaping and native text APIs if needed
- web canvas / DOM rendering
- animations
- cursor blink presentation, if desired
- platform-specific scrolling/presentation tricks

Rust should not contain Metal/AppKit/SwiftUI/web/terminal-specific painting details.

## Why not full Rust rendering?

Rendering is client-dependent.

A terminal client, a Metal client, a SwiftUI/AppKit client, and a web client want different final data. Trying to make Rust produce the final paint representation for all of them either:

1. makes Rust too client-specific, or
2. makes the display list too low-level and expensive, or
3. forces clients to redo layout work anyway.

The right target is a semantic display list / render delta: low-level enough that clients do not duplicate editor layout, but high-level enough that clients can paint natively.

## Why not client-owned layout?

Client-owned layout sounds flexible, but it pushes hot editor work over the boundary or duplicates state in every client.

The following should not be recomputed independently by every client:

- mapping document positions to visual rows
- soft wrap decisions
- virtual line placement
- syntax/decorations merge
- selection geometry
- gutter content for visible rows
- dirty line calculation

Those are editor semantics, not paint details.

## Lesson from the old editor

The old editor had a function like this:

```rust
pub fn build_plan<'a, 't, H: HighlightProvider>(
  doc: &'a Document,
  view: ViewState,
  text_fmt: &'a TextFormat,
  gutter: &GutterConfig,
  annotations: &'t mut TextAnnotations<'a>,
  highlights: &mut H,
  cache: &mut RenderCache,
  styles: RenderStyles,
) -> RenderPlan
```

This built a viewport render plan.

It also had reuse paths:

```rust
pub fn try_reuse_render_plan_for_vertical_scroll(...) -> Option<RenderPlan>
pub fn try_reuse_render_plan_for_vertical_resize(...) -> Option<RenderPlan>
```

Those reuse paths cloned the previous plan, shifted rows, and built newly exposed rows.

That was a good optimization, but it was not enough. Important features disabled the fast path:

```rust
if text_fmt.soft_wrap || annotations.has_line_annotations() {
  return None;
}
```

This is a core lesson:

> A feature like soft wrap, diagnostics, virtual text, or line annotations must not disable the renderer's fundamental incremental path.

Instead, those features need to be first-class inputs to the retained layout cache.

## Render kernel

Before building retained render surfaces and frame deltas, `the-lib` should first expose a small, compileable render kernel.

This kernel is not the final rendering architecture. It is the pure visual-layout foundation that later retained rendering will consume.

Its job is to answer questions like:

- How does document text plus layout policy become a stream of visual graphemes?
- Where do soft wraps happen?
- How do inline annotations, grapheme substitutions, and virtual lines participate in layout?
- How do we map document character positions to visual row/column positions and back?

It should not answer questions like:

- What rows changed since the last frame?
- What is the surface viewport rectangle in pixels?
- What scene overlays, widgets, or floating UI should be painted?
- What theme/style/color should a client use?

### Phase-1 module set

The phase-1 render kernel should be only:

```txt
the-lib/render/
  mod.rs
  text_format.rs
  text_annotations.rs
  doc_formatter.rs
  visual_position.rs
```

Explicitly not part of this kernel:

- `graphics.rs`
- `grapheme.rs`
- `plan.rs`
- old `overlay.rs`
- `view.rs`
- retained surfaces / frame deltas / dirty tracking
- provider IDs / generations / layer invalidation metadata
- pixel measurement / client shaping protocols

### Kernel responsibilities

#### `text_format.rs`
`TextFormat` is a pure layout-policy type.

It should keep only fields that affect visual layout semantics, such as:

- `soft_wrap`
- `tab_width`
- `max_wrap`
- `max_indent_retain`
- `wrap_indicator`
- `soft_wrap_at_text_width`

It should not contain:

- viewport width
- styling/highlight fields
- client-paint concerns

The currently available wrap width is a surface/layout input and should be passed explicitly to layout operations rather than stored inside `TextFormat`.

#### `text_annotations.rs`
`TextAnnotations` should be a small owned container over exactly three public annotation kinds:

- `InlineAnnotation`
  - inserts text before a document grapheme
- `GraphemeSubstitution`
  - visually substitutes exactly one displayed grapheme at a document `char_idx`
- `VirtualLineSpec`
  - inserts extra visual lines after a document line

This module should be:

- owned-only
- immutable as public input
- self-normalizing / self-validating
- style-free

It should not expose a grand decoration/provider system in phase 1.
It should not expose old `RenderPlan` integration helpers.
It should not expose UI overlays, floating panels, or client-scene abstractions.

#### `doc_formatter.rs`
`DocumentFormatter` is the core low-level visual traversal primitive.

It should remain public, but explicitly low-level.
It is not the final retained rendering API.

It should own:

- grapheme traversal
- tab expansion
- soft-wrap decisions
- wrap-indicator insertion
- indentation carry-over on wrapped rows
- integration of `TextAnnotations`
- visual `(row, col)` progression
- explicit EOF sentinel behavior

It should also own the tiny formatter output vocabulary, including:

- `FormattedGrapheme`
- `GraphemeSource`

These types do not justify a separate `render/grapheme.rs` module.
They belong with the formatter.

Checkpoint/block-start optimizations should remain internal. Public construction should use obvious constructors, not checkpoint-shaped APIs.

#### `visual_position.rs`
This module is the query layer built on top of `DocumentFormatter`.

Its main public helpers should be:

- `visual_pos_at_char(...)`
- `char_at_visual_pos(...)`

Lower-level block-relative movement helpers may exist, but should remain crate-private until a clearer public need appears.

### Coordinate model

The phase-1 kernel should stay entirely in visual rows/columns.

Use a tiny render-local visual position type:

```rust
pub struct Position {
  pub row: usize,
  pub col: usize,
}
```

This is a visual layout coordinate, not a document anchor type, not a selection range type, and not a pixel geometry type.

### Style and presentation boundary

The phase-1 kernel should be intentionally style-free.

Do not carry:

- syntax highlight IDs in formatter output
- overlay/annotation highlight payloads
- color/style/theme structs
- UI overlay scene nodes

Why:

- wrap/layout semantics belong in Rust
- painting belongs to clients
- style/theme mapping belongs in later layered render outputs, not in the kernel itself

The wrap indicator text itself still belongs in the kernel because it affects visible content, width, and visual-position mapping. Its styling does not.

### What is deferred to retained rendering

The render kernel is the foundation, not the destination.
The following are explicitly phase-2 retained-render concerns:

- `RenderSurface`
- `RenderFrameDelta`
- row identities and row moves
- layer deltas
- dirty tracking
- provider generations and invalidation
- gutter/cursor/selection/widget layers
- viewport and placement types
- pixel measurement / shaping protocols

Those should be designed around the retained-surface architecture, not forced into the kernel early.

## New model: retained render surfaces

The primary render object should not be a pure function that rebuilds a plan from scratch.

Instead, each editor view should have a retained render surface.

```rust
pub struct RenderSurface {
  pub id: SurfaceId,
  pub buffer_id: BufferId,
  pub viewport: Viewport,
  pub format: TextFormat,

  base_layout_cache: LineLayoutCache,
  decoration_cache: DecorationCache,
  gutter_cache: GutterCache,
  cursor_layer: CursorLayer,

  dirty: DirtyState,
  generation: RenderGeneration,
}
```

A full code editor, a small text input, a readonly viewer, and a diff view can all have render surfaces. They differ by enabled capabilities, not by having entirely different rendering architectures.

Example TS-side composition:

```ts
const input = editor.instances.create({
  kind: "text-input",
  features: ["text", "history", "render"],
})

const code = editor.instances.create({
  kind: "code-editor",
  features: [
    "text",
    "history",
    "render",
    "syntax",
    "diagnostics",
    "lsp",
    "git-diff",
  ],
})
```

Both can use the same native render surface API.

## Surface API shape

Rust should expose coarse-grained render APIs.

```rust
pub struct FrameRequest {
  pub surface_id: SurfaceId,
  pub viewport: Viewport,
  pub since_generation: Option<u64>,
}

pub struct RenderFrameDelta {
  pub surface_id: SurfaceId,
  pub generation: u64,
  pub full: bool,
  pub retained_rows: Vec<RowMove>,
  pub changed_rows: Vec<RenderRow>,
  pub removed_rows: Vec<RowId>,
  pub layers: LayerDeltaSet,
  pub damage: Damage,
}
```

Example TS usage:

```ts
let generation: number | undefined

function drawFrame(surface: SurfaceHandle) {
  const delta = native.renderFrame({
    surfaceId: surface.id,
    viewport: surface.viewport,
    sinceGeneration: generation,
  })

  generation = delta.generation
  surfaceScene.apply(delta)
  surfacePainter.paint(surfaceScene)
}
```

The client retains its scene. Rust sends deltas.

Do not make the client request individual cells/spans in a loop.

Bad:

```ts
for (let row = 0; row < viewport.height; row++) {
  const line = native.getRenderLine(surface.id, row)
  draw(line)
}
```

Good:

```ts
const delta = native.renderFrame({ surfaceId, viewport, sinceGeneration })
scene.apply(delta)
painter.paint(scene)
```

## Phase 2A: first retained implementation

The first implementation of retained rendering should be a deliberately narrow vertical slice.

It should prove the retained model with the smallest real surface possible:

- one persistent `RenderSurface`
- one primary text source
- one row-major viewport
- stable row IDs
- per-row generations
- row moves on vertical scroll
- changed/removed rows
- `RenderFrameDelta` as the real public delta shape
- rows layer only

It should explicitly defer:

- `RenderManager`
- real layer subsystem implementation
- gutters
- selections
- cursors
- widgets
- style/theme/syntax payloads
- provider IDs / provider generations
- horizontal scrolling
- pixel measurement / shaping
- full document-wide retained layout caches

### Concrete file plan

The first retained slice should add only:

```txt
the-lib/render/
  display_list.rs
  damage.rs
  layers.rs
  surface.rs
```

The existing phase-1 kernel remains the foundation:

- `text_format.rs`
- `text_annotations.rs`
- `doc_formatter.rs`
- `visual_position.rs`

### First viewport model

The first retained slice should use a row-major viewport, not a pixel viewport:

```rust
pub struct Viewport {
  pub first_row: usize,
  pub row_count: usize,
  pub wrap_width: usize,
}
```

This is intentionally not the final viewport model. It exists to prove retained visible-row deltas while the renderer is still column-based.

### First `RenderSurface` model

The first `RenderSurface` should already be a persistent document-attached render object in the architectural sense, even if the actual document object is deferred for now.

That means:

- the surface is long-lived
- it owns its `TextFormat`
- it owns its `TextAnnotations`
- it retains previous visible-row state across frames
- text and `text_revision` may still be passed explicitly to `render_frame(...)`

Conceptually:

```rust
pub struct RenderSurface {
  id: SurfaceId,
  format: TextFormat,
  annotations: TextAnnotations,
  generation: u64,
  // retained visible-row state
}
```

### First `render_frame(...)` contract

The first concrete API should already use the final coarse-grained delta shape:

```rust
pub struct FrameRequest {
  pub viewport: Viewport,
  pub since_generation: Option<u64>,
}

pub struct RenderFrameDelta {
  pub surface_id: SurfaceId,
  pub generation: u64,
  pub full: bool,
  pub retained_rows: Vec<RowMove>,
  pub changed_rows: Vec<RenderRow>,
  pub removed_rows: Vec<RowId>,
  pub layers: LayerDeltaSet,
  pub damage: Damage,
}
```

Even in the first slice, callers should provide explicit text revision information. Retained rendering needs an honest notion of "same text state" vs "new text state" from the start.

### First `RenderRow` shape

The first `RenderRow` should be text-only:

```rust
pub struct RenderRow {
  pub id: RowId,
  pub generation: u64,
  pub placement: RowPlacement,
  pub text: Tendril,
}

pub struct RowPlacement {
  pub surface_row: usize,
}
```

Do not export formatter internals, per-grapheme payloads, syntax/style spans, or presentation data yet.

The first retained slice exists to prove identity, placement, and delta mechanics, not style layering.

### Row identity rules for the first slice

Lock these rules in immediately:

- `RowId` is surface-local
- `RowId` identifies a surviving visual fragment
- row identity is not a logical line number
- row identity is not a screen row number
- row identity is not derived from text hashing
- a moved unchanged row keeps its `RowId`
- a moved unchanged row also keeps its row generation
- only content changes bump row generation

The renderer may use private internal row keys to match old and new visible fragments. Those keys are implementation detail. The public contract remains `RowId` plus row generation.

### First cache scope

The first retained slice should retain only the currently visible viewport window well. It should not attempt the full document-wide retained layout cache yet.

That means `RenderSurface` should remember things like:

- last viewport
- last text revision
- last visible rows
- the row IDs and row generations for those visible fragments

This is intentionally smaller than the final architecture, but it is sufficient to prove:

- retained row identity
- scroll row moves
- changed/new/removed rows
- stable delta protocol shape

### Diff behavior

The first retained slice should follow these conservative rules:

#### Return a full refresh when:
- this is the first frame
- `since_generation` is stale or absent when the caller expects incrementality
- the surface `TextFormat` changed
- the surface `TextAnnotations` changed
- the wrap width changed

#### Return an incremental delta when:
- the caller's `since_generation` matches the surface generation
- format is unchanged
- annotations are unchanged
- wrap width is unchanged

In the incremental case:
- reused surviving rows become `retained_rows` if placement changed
- text-changed rows go to `changed_rows`
- newly visible/newly created rows go to `changed_rows`
- rows no longer visible go to `removed_rows`

### Minimal layer and damage model

The first retained slice should keep the public fields for layers and damage, but their implementation should stay minimal.

`LayerDeltaSet` should exist, but be effectively empty/placeholder until rows-only retention works.

`Damage` should stay simple:
- full refresh flag
- changed visible row range

Richer per-layer damage and reason-rich invalidation come later.

### First implementation order

Recommended order:

1. `display_list.rs`
   - public delta, row, placement, viewport, ID types
2. `damage.rs`
   - minimal damage model
3. `layers.rs`
   - placeholder `LayerDeltaSet`
4. `surface.rs`
   - retained visible-row state
   - row building from `DocumentFormatter`
   - diffing old vs new visible rows
   - full/incremental frame production

### First test priorities

The first retained slice should be tested as a state machine.

Minimum tests:

- first frame returns `full = true`
- second identical frame returns empty incremental delta
- vertical scroll reuses overlapping rows via `retained_rows`
- only newly exposed rows appear in `changed_rows`
- scrolled-out rows appear in `removed_rows`
- text edits in visible rows produce bounded row changes
- moved unchanged rows keep row generation
- changed rows bump row generation
- wrap-width changes force full refresh
- annotation changes force full refresh
- stale generation requests force full refresh

This phase is not the final renderer. It is the first proof that retained row identity and frame deltas are real.

## Stable row identities

The old Swift client had to compute row cache signatures by hashing row contents. That is too expensive as the main identity mechanism.

Rust should provide stable visual-row identities and row generations. The identity contract is:

> A row ID identifies a surviving visual fragment, not merely a screen row and not merely a logical line number.

A row has both identity and placement:

```rust
pub struct RenderRow {
  pub id: RowId,
  pub generation: u64,
  pub placement: RowPlacement,
  pub source: RowSource,
  pub content: RowContent,
}

pub struct RowPlacement {
  pub surface_row: u32,
  pub y_px: f32,
  pub height_px: f32,
}

pub enum RowSource {
  BufferLine {
    buffer_id: BufferId,
    anchor: BufferAnchor,
    logical_line: u32,
    visual_fragment: u32,
  },
  VirtualLine {
    provider_id: ProviderId,
    anchor: BufferAnchor,
    ordinal: u32,
  },
  Widget {
    widget_id: WidgetId,
  },
}
```

The client caches presentation objects by `(row.id, row.generation)` rather than by hashing text and style.

```ts
type RowCacheKey = `${number}:${number}` // row id + generation

function applyRow(row: RenderRow) {
  const key = `${row.id}:${row.generation}`
  let cached = rowCache.get(key)
  if (!cached) {
    cached = buildNativeLinePresentation(row)
    rowCache.set(key, cached)
  }
  scene.placeRow(row.id, row.placement, cached)
}
```

### Row movement and removal

Rows may move without changing contents. For example, a scroll can move rows on the surface while preserving their row IDs and generations. A frame delta should represent this explicitly.

```rust
pub struct RowMove {
  pub id: RowId,
  pub from: Option<RowPlacement>,
  pub to: RowPlacement,
}

pub struct RenderFrameDelta {
  pub surface_id: SurfaceId,
  pub generation: u64,
  pub full: bool,
  pub retained_rows: Vec<RowMove>,
  pub changed_rows: Vec<RenderRow>,
  pub removed_rows: Vec<RowId>,
  pub layers: LayerDeltaSet,
  pub damage: Damage,
}
```

A structural edit may invalidate row identity for a region. That is acceptable. The important invariant is that Rust decides identity and emits explicit removals/moves/changes; clients should not infer identity from text content or screen position.

## Logical lines vs visual rows

The render cache cannot only be keyed by logical line.

Soft wrap, folds, virtual lines, inline diagnostics, block widgets, font metrics, and viewport constraints can cause one logical line to produce many visual rows.

The retained layout cache should model this explicitly:

```rust
pub struct CachedLineLayout {
  pub buffer_id: BufferId,
  pub line_anchor: BufferAnchor,
  pub logical_line_hint: u32,
  pub generation: u64,
  pub inputs: LineLayoutInputFingerprint,
  pub visual_rows: Vec<VisualRow>,
}

pub struct VisualRow {
  pub row_id: RowId,
  pub fragment_index: u32,
  pub source_range: TextRange,
  pub inline_runs: Vec<InlineRun>,
  pub height_px: f32,
}
```

The `logical_line_hint` is allowed to change after edits above the line. The stable part is the anchor/fingerprint/row identity maintained by Rust.

## Cache invalidation model

Avoid global cache keys that accidentally invalidate unrelated lines.

A bad key is:

```rust
pub struct BadCachedLineLayoutKey {
  pub logical_line: u32,
  pub text_revision: u64,
  pub syntax_revision: u64,
  pub decoration_revision: u64,
  pub format_signature: TextFormatSignature,
}
```

This is too blunt. If diagnostics change on line 500, a global `decoration_revision` should not invalidate line 10.

Instead, cache entries should be invalidated by precise dirty ranges and by input fingerprints for the providers that actually affect a line.

```rust
pub struct LineLayoutInputFingerprint {
  pub text: LineTextFingerprint,
  pub layout: LayoutInputFingerprint,
  pub syntax: Option<ProviderFingerprint>,
  pub folds: Option<ProviderFingerprint>,
  pub inline_providers: SmallVec<[ProviderFingerprint; 4]>,
  pub block_providers: SmallVec<[ProviderFingerprint; 2]>,
  pub measurement: MeasurementFingerprint,
}

pub struct ProviderFingerprint {
  pub provider_id: ProviderId,
  pub generation: u64,
}
```

Provider generations are local inputs, not global invalidation hammers. A decoration provider update should include affected ranges:

```rust
pub struct DecorationSetUpdate {
  pub buffer_id: BufferId,
  pub provider_id: ProviderId,
  pub generation: u64,
  pub changed_ranges: Vec<LineRange>,
  pub decorations: Vec<Decoration>,
}
```

Then Rust marks only the affected render surfaces and layers dirty.

## Soft wrap and layout invariants

Soft wrap must be a first-class retained-layout feature. It must not disable incremental rendering.

The layout cache is keyed by all inputs that affect wrapping:

```rust
pub struct LayoutInputFingerprint {
  pub wrap_width_px: OrderedFloat<f32>,
  pub tab_width: u16,
  pub indent_guides: bool,
  pub font_metrics_generation: u64,
  pub text_direction_mode: TextDirectionMode,
  pub line_ending_policy: LineEndingPolicy,
}
```

A width change does not conceptually invalidate the document. It invalidates cached layouts whose wrap width/measurement inputs no longer match. The renderer may keep layouts for multiple widths if useful:

```rust
pub struct LineLayoutCache {
  by_line_and_inputs: HashMap<(BufferAnchor, LayoutInputFingerprint), CachedLineLayout>,
}
```

The invariant is:

> Given the same line content, inline/block layout inputs, measurement fingerprint, and wrap constraints, Rust must produce the same visual-row fragments and row generations.

Viewport lookup under soft wrap should use a visual index, not document-start traversal.

```rust
pub struct VisualIndex {
  // Maps document anchors/line anchors to cumulative visual heights.
  line_heights: BTreeMap<BufferAnchor, VisualExtent>,
}

pub struct VisualExtent {
  pub visual_rows: u32,
  pub height_px: f32,
  pub generation: u64,
}
```

When a line's wrapped height changes, Rust updates cumulative visual extents from that anchor onward. The frame builder can then map viewport y-offsets to visual rows without walking from the beginning of the document.

## Measurement model

Rust owns editor layout, but layout requires measurement. Measurement must be explicit.

```rust
pub enum MeasurementBackendKind {
  CellGrid,
  ClientMeasured,
  NativeShaping,
}

pub struct MeasurementFingerprint {
  pub backend: MeasurementBackendKind,
  pub generation: u64,
  pub font_family_id: FontFamilyId,
  pub font_size_px: OrderedFloat<f32>,
  pub font_features: FontFeatureSetId,
  pub device_scale: OrderedFloat<f32>,
}
```

The ideal model supports multiple measurement backends:

```rust
pub trait TextMeasurer {
  fn measure_text_run(&mut self, run: &TextRunMeasureRequest) -> TextRunMeasurement;
  fn measure_widget(&mut self, widget: &WidgetMeasureRequest) -> WidgetMeasurement;
}
```

For clients where Rust cannot measure accurately, the client provides measurements through a cache/protocol. Rust remains the owner of layout decisions, but layout decisions are based on explicit measurement inputs supplied by the active surface backend.

```ts
native.measurements.provide(surfaceId, {
  generation: fontGeneration,
  textRuns: measuredRuns,
  widgets: measuredWidgets,
})
```

Measurement changes are layout changes. They dirty layout for affected surfaces and may update row heights, wrap points, cursor geometry, and viewport mapping.

## Virtual lines, block widgets, and client-measured content

Extensions need more than inline spans. The ideal model supports three render contribution classes.

### Inline decorations

Inline decorations participate in line layout.

```ts
editor.decorations.set(bufferId, "ai.ghost", [
  {
    kind: "inlineText",
    range,
    text: " suggested text",
    className: "ghost-text",
    priority: 50,
  },
])
```

### Virtual lines

Virtual lines add visual rows before/after buffer lines. They have explicit layout inputs and stable provider identity.

```ts
editor.decorations.set(bufferId, "diagnostics", [
  {
    kind: "virtualLine",
    afterLine: 42,
    content: [{ text: "expected `;`", className: "error" }],
    priority: 100,
  },
])
```

### Block widgets

Block widgets are arbitrary client-rendered content anchored in the editor layout. Rust needs their measured size for viewport math.

```ts
editor.widgets.register("ai.diff-preview", {
  async measure(ctx) {
    return { widthPx: ctx.constraints.widthPx, heightPx: await measurePreview(ctx) }
  },
  render(ctx) {
    return renderPreview(ctx)
  },
})
```

Rust tracks widget placement and measured extent:

```rust
pub struct BlockWidgetLayout {
  pub widget_id: WidgetId,
  pub anchor: BufferAnchor,
  pub placement: WidgetPlacement,
  pub measured_width_px: f32,
  pub measured_height_px: f32,
  pub generation: u64,
}
```

The measurement protocol must avoid unbounded sync callbacks during frame construction. Measurements should be cached and revisioned. If a needed measurement is missing, Rust may use the last known measurement or an explicit estimate and then emit a correction when the true measurement arrives.

## Layered rendering

The old `RenderPlan` packed base text, gutter lines, selections, cursors, overlays, diagnostics, and other decorations into one product.

The new model should be layered.

Recommended conceptual layers:

```txt
layout/text rows
syntax/style information
inline decorations
gutter decorations
selection layer
cursor layer
overlay layer
widget layer
```

Each layer has separate generation and damage.

```rust
pub struct RenderLayerGenerations {
  pub layout: u64,
  pub text: u64,
  pub syntax: u64,
  pub inline_decorations: u64,
  pub gutter: u64,
  pub selections: u64,
  pub cursors: u64,
  pub overlays: u64,
  pub widgets: u64,
  pub theme: u64,
}
```

This allows cheap updates:

- cursor blink should not rebuild base text rows
- cursor movement should not rebuild syntax spans
- selection changes should not rebuild line wrapping
- diagnostics should only dirty touched ranges
- theme changes may require repaint but not relayout
- scrolling should mostly move retained rows and materialize newly visible rows

The delta protocol can still group layers coarsely when practical. The conceptual model is layered so that the implementation can split hot paths without changing the API's semantics.

```rust
pub struct LayerDeltaSet {
  pub rows: Option<RowLayerDelta>,
  pub gutters: Option<GutterLayerDelta>,
  pub selections: Option<SelectionLayerDelta>,
  pub cursors: Option<CursorLayerDelta>,
  pub overlays: Option<OverlayLayerDelta>,
  pub widgets: Option<WidgetLayerDelta>,
}
```

## Dirty state

Dirty state should be tracked per layer. A single cause can dirty multiple ranges, and a single range can be dirty for multiple reasons. Do not encode invalidation logic as one `DirtyRange { reason }` list.

```rust
pub struct DirtyState {
  pub layout: DirtyRanges,
  pub text: DirtyRanges,
  pub syntax: DirtyRanges,
  pub inline_decorations: DirtyRanges,
  pub gutter: DirtyRanges,
  pub selections: DirtyRanges,
  pub cursors: DirtyFlag,
  pub overlays: DirtyRanges,
  pub widgets: DirtyRanges,
}

pub struct DirtyRanges {
  pub full: bool,
  pub ranges: RangeSet<u32>,
}

pub struct DirtyFlag {
  pub dirty: bool,
}
```

Reasons are useful for debugging/profiling, but layer/range membership should drive rendering.

```rust
impl RenderSurface {
  pub fn on_transaction(&mut self, tx: &Transaction) {
    for range in tx.changed_line_ranges() {
      self.dirty.text.ranges.insert(range.clone());
      self.dirty.layout.ranges.insert(range);
    }
  }

  pub fn on_syntax_changed(&mut self, changed_lines: LineRange) {
    self.dirty.syntax.ranges.insert(changed_lines);
  }

  pub fn on_cursor_moved(&mut self) {
    self.dirty.cursors.dirty = true;
  }
}
```

## Decorations

Extensions should be able to add decorations from JS/TS, but Rust should merge and resolve them.

TS should submit structured decorations, not imperatively paint pixels.

```ts
editor.decorations.set(bufferId, "git.diff", [
  {
    range: { start: { line: 10, column: 0 }, end: { line: 10, column: 0 } },
    kind: "gutter",
    text: "+",
    className: "git-added",
    priority: 100,
  },
])

editor.decorations.set(bufferId, "diagnostics", [
  {
    range: diagnostic.range,
    kind: "underline",
    className: "error",
    priority: 200,
  },
])
```

Rust receives these decorations and invalidates touched lines/layers.

```rust
pub struct DecorationSetUpdate {
  pub buffer_id: BufferId,
  pub provider_id: ProviderId,
  pub generation: u64,
  pub changed_ranges: Vec<LineRange>,
  pub decorations: Vec<Decoration>,
}
```

## Rendering and the JS boundary

JS/TS should compose rendering features, but should not perform hot layout loops over large editor state.

Good:

```ts
editor.hooks.on("diagnostics:update", diagnostics => {
  editor.decorations.set(buffer.id, "diagnostics", diagnosticsToDecorations(diagnostics))
})
```

Good:

```ts
const frame = native.renderFrame({
  surfaceId,
  viewport,
  sinceGeneration,
})
```

Bad:

```ts
for (const line of visibleLines) {
  const text = await native.getLine(bufferId, line)
  const tokens = tokenizeInJs(text)
  const wrapped = wrapInJs(tokens)
  draw(wrapped)
}
```

The Rust/JS API should be coarse-grained and batch-oriented.

## Separate render snapshots from app snapshots

The old Swift snapshot path bundled render state with lots of unrelated UI/application state:

- command palette
- file tree
- buffer tabs
- statusline
- docs panel
- completion menu
- render lines
- panes
- diagnostics

That made scroll too broad.

The new API should split state by update class.

```ts
native.renderFrame(...)          // editor surface rendering only
native.getCommandPaletteState()  // command palette only
native.getFileTreeDelta(...)     // file tree only
native.getBufferTabsDelta(...)   // tabs only
native.getCompletionState(...)   // completion menu only
```

Scrolling a text view should not rebuild or export command palette, file tree, buffer tabs, or docs panel state.

## Pane/surface isolation

Each pane/editor view should have its own render surface and invalidation state.

If one pane scrolls, inactive panes should not rebuild their render plans.

```rust
pub struct RenderManager {
  surfaces: HashMap<SurfaceId, RenderSurface>,
}

impl RenderManager {
  pub fn render_frame(&mut self, request: FrameRequest) -> RenderFrameDelta {
    self.surfaces
      .get_mut(&request.surface_id)
      .expect("surface exists")
      .render_frame(request)
  }
}
```

For split panes:

```ts
for (const surface of visibleSurfaces) {
  if (surface.needsFrame) {
    const delta = native.renderFrame({
      surfaceId: surface.id,
      viewport: surface.viewport,
      sinceGeneration: surface.generation,
    })
    surface.apply(delta)
  }
}
```

## Text inputs as editor instances

A small text input should be able to use the same render architecture as a full editor, with fewer capabilities attached.

```ts
const prompt = editor.instances.create({
  kind: "prompt",
  buffer: editor.buffers.memory(""),
  capabilities: [
    editor.capabilities.text(),
    editor.capabilities.history(),
    editor.capabilities.render({
      multiline: false,
      gutters: false,
      syntax: false,
    }),
  ],
})
```

Full editor:

```ts
const code = editor.instances.create({
  kind: "code-editor",
  buffer: await editor.buffers.openFile(path),
  capabilities: [
    editor.capabilities.text(),
    editor.capabilities.history(),
    editor.capabilities.render({ gutters: true, wrap: true }),
    editor.capabilities.syntax({ language: "rust" }),
    editor.capabilities.diagnostics(),
    editor.capabilities.lsp(),
  ],
})
```

Same core primitives. Different composition.

## Rust module sketch

Ideal module layout:

```txt
the-core/
  buffer/
  selection/
  transaction/
  syntax/
  diagnostics/
  render/
    surface.rs
    manager.rs
    layout.rs
    line_cache.rs
    display_list.rs
    layers.rs
    damage.rs
    decoration.rs
    gutter.rs
```

Possible public types:

```rust
pub mod render {
  pub use surface::{RenderSurface, SurfaceId};
  pub use manager::RenderManager;
  pub use display_list::{RenderFrameDelta, RenderRow, RenderSpan};
  pub use damage::{Damage, DirtyState, DirtyRanges, DirtyFlag};
  pub use layers::{LayerDeltaSet, RenderLayerGenerations};
}
```

## Client scene model

Clients should retain presentation state.

```ts
class RenderScene {
  generation = 0
  rows = new Map<number, ClientLinePresentation>()
  cursors: CursorPresentation[] = []
  selections: SelectionPresentation[] = []

  apply(delta: RenderFrameDelta) {
    if (delta.full) {
      this.rows.clear()
    }

    for (const rowId of delta.removedRows) {
      this.rows.delete(rowId)
    }

    for (const row of delta.changedRows) {
      this.rows.set(row.id, buildLinePresentation(row))
    }

    if (delta.layers.cursors) {
      this.cursors = delta.layers.cursors.map(buildCursorPresentation)
    }

    if (delta.layers.selections) {
      this.selections = delta.layers.selections.map(buildSelectionPresentation)
    }

    this.generation = delta.generation
  }
}
```

A terminal client may translate rows to cells. A Metal client may translate rows to glyph quads. A SwiftUI client may translate rows into native text/presentation objects. This is a presentation mirror, not a semantic document model.

The client may retain visual rows, cursor presentations, selection presentations, widget handles, and GPU/native resources. The client must not treat this presentation scene as an editable document model. It should not answer semantic questions like “what text is at logical position X?” from the scene. Semantic queries go back to Rust/core.

## Frame pacing

The client owns the frame loop. Rust owns dirty state and render-delta production.

The ideal model is pull-based:

1. Input, LSP, syntax, extension, or timer events mark surfaces dirty.
2. The client render loop decides when a frame is needed.
3. The client calls `renderFrame` for dirty visible surfaces.
4. Rust synchronously produces a bounded render delta from already-available core state.
5. The client applies the delta to its retained scene and paints using native APIs.

```ts
function onFrame() {
  for (const surface of visibleSurfaces) {
    if (!surface.needsRender) continue

    const delta = native.renderFrame({
      surfaceId: surface.id,
      viewport: surface.viewport,
      sinceGeneration: surface.generation,
    })

    surface.scene.apply(delta)
    surface.generation = delta.generation
  }

  client.requestNextFrameIfNeeded()
}
```

Heavy work should not happen synchronously inside frame construction unless it is explicitly bounded and expected. Tree-sitter parsing, LSP, project search, git diffing, and expensive extension work should complete off the frame path and mark surfaces dirty for a later frame.

Frame deltas should be safe to skip. If the client misses a frame or chooses not to draw, the next `renderFrame` call with the last applied generation should produce a valid delta or a full refresh.

## Thread model

`RenderManager` should be logically single-writer. Background workers should not mutate `RenderSurface` directly.

Recommended ownership model:

```txt
main/core thread:
  owns buffers, editor instances, render surfaces, dirty state
  applies transactions
  consumes background results
  builds render deltas

background workers:
  tree-sitter parsing
  LSP requests/responses
  diagnostics
  project search
  git/diff work
  expensive extension/native tasks
```

Background workers send revisioned events back to the core thread:

```rust
pub enum CoreEvent {
  SyntaxChanged {
    buffer_id: BufferId,
    syntax_revision: u64,
    changed_ranges: Vec<LineRange>,
  },
  DiagnosticsChanged {
    buffer_id: BufferId,
    provider_id: ProviderId,
    generation: u64,
    changed_ranges: Vec<LineRange>,
  },
  MeasurementsChanged {
    surface_id: SurfaceId,
    measurement_generation: u64,
    changed_widgets: Vec<WidgetId>,
    changed_text_runs: Vec<TextRunKey>,
  },
}
```

The core thread applies those events and marks affected surfaces/layers dirty:

```rust
impl Core {
  pub fn apply_event(&mut self, event: CoreEvent) {
    match event {
      CoreEvent::SyntaxChanged { buffer_id, changed_ranges, .. } => {
        for surface in self.render.surfaces_for_buffer(buffer_id) {
          surface.mark_syntax_dirty(changed_ranges.clone());
        }
      }
      CoreEvent::DiagnosticsChanged { buffer_id, provider_id, changed_ranges, .. } => {
        for surface in self.render.surfaces_for_buffer(buffer_id) {
          surface.mark_decoration_provider_dirty(provider_id, changed_ranges.clone());
        }
      }
      CoreEvent::MeasurementsChanged { surface_id, .. } => {
        self.render.surface_mut(surface_id).mark_measurement_dirty();
      }
    }
  }
}
```

This keeps render state deterministic and avoids frame construction blocking on arbitrary worker locks.

## Extension rendering model

Extensions should be able to contribute visual content without owning the renderer.

The ideal model has three contribution levels.

### Decorations

Decorations are structured contributions merged by Rust into existing editor layout/layers:

- syntax-like styles
- underlines
- highlights
- inline ghost text
- gutter marks
- virtual lines

```ts
editor.decorations.set(bufferId, "inline-blame", [
  {
    kind: "inlineText",
    range,
    text: "  You, 2 minutes ago",
    className: "blame-muted",
    priority: 20,
  },
])
```

### Widgets

Widgets are anchored client-rendered content that participates in layout through explicit measurement.

```ts
editor.widgets.mount(bufferId, {
  id: "ai-diff-preview:123",
  anchor: { line: 20, column: 0 },
  placement: "after-line",
  measure: constraints => measureAiPreview(constraints),
  render: target => renderAiPreview(target),
})
```

### Surfaces

Some extensions need independent render surfaces instead of decorations. Examples:

- minimap
- diff overview ruler
- custom side-by-side diff view
- embedded terminal-like surface
- AI/coding-agent transcript with editor-like text behavior

```ts
const minimap = editor.surfaces.create({
  kind: "minimap",
  buffer: code.buffer,
  sourceSurface: code.surface.id,
  layout: { scale: "document-overview" },
})
```

A minimap may share buffer state and some cached line metadata with the main surface, but it should still be a surface with its own viewport, layout inputs, dirty state, and render deltas.

## Testing strategy

Rendering should be tested as a delta-producing state machine, not only as pixels or final snapshots.

A test renderer should be a first-class client. It should apply frame deltas to a retained scene and assert both final state and incremental behavior.

```rust
#[test]
fn typing_invalidates_current_line_and_cursor() {
  let mut editor = TestEditor::new("a\nb\nc\n");
  let surface = editor.create_surface(TestSurfaceConfig::default());

  let initial = editor.render_frame(surface, None);
  editor.insert_text(Position::new(1, 1), "x");
  let delta = editor.render_frame(surface, Some(initial.generation));

  assert!(!delta.full);
  assert_eq!(delta.changed_rows.len(), 1);
  assert!(delta.layers.cursors.is_some());
}
```

Golden tests should cover event sequences:

```txt
open file
render
insert text
render
scroll
render
add diagnostic
render
change width
render
add virtual line
render
```

The expected output should include:

- full vs incremental frame flag
- row IDs
- row generations
- row moves
- changed rows
- removed rows
- layer deltas
- damage regions

This catches invalidation bugs that final visual snapshot tests miss.

## FFI/boundary bandwidth budgets

Boundary size should be measured and budgeted.

Targets are not correctness rules, but they are useful performance alarms:

```txt
steady typing frame:
  one changed row + cursor/selection delta
  expected: roughly 1-4KB

small scroll:
  mostly row moves + newly exposed rows
  expected: roughly 5-20KB

full viewport rebuild:
  buffer switch, major resize, full theme/style reset
  expected: roughly 50-150KB depending on viewport and styles
```

If routine typing or cursor movement exports viewport-sized payloads, the layering model failed. If small scroll exports every row's full contents instead of row moves and newly exposed rows, the retained surface model failed.

## Instrumentation requirements

Rendering changes should be measured from the beginning.

Track:

- total `renderFrame` time
- dirty range count
- number of rows reused
- number of rows rebuilt
- number of spans exported
- bytes crossing Rust/client boundary
- client scene apply time
- client paint time
- active pane vs inactive pane cost
- with/without syntax
- with/without diagnostics
- with/without soft wrap
- with/without virtual lines

Example Rust metrics:

```rust
pub struct RenderFrameMetrics {
  pub total_us: u64,
  pub layout_us: u64,
  pub decoration_us: u64,
  pub rows_reused: u32,
  pub rows_rebuilt: u32,
  pub spans_exported: u32,
  pub bytes_estimated: u64,
}
```

## Non-goals

Do not:

- put Metal/AppKit/SwiftUI details in Rust core
- make the client maintain a second semantic document model
- call into JS/TS per row/grapheme/span
- rebuild unrelated app state on scroll
- make soft wrap or diagnostics disable incremental rendering
- treat the display list as a full fresh snapshot forever

## Final position

The renderer should be display-list driven, but retained and incremental.

```txt
Rust owns editor layout, retained render surfaces, dirty tracking, and render-delta production.
Clients own measurement backends, retained presentation scenes, and native painting.
JS/TS owns composition, extension policy, decoration providers, widget providers, and surface orchestration.
```

The old editor's display-list idea was correct. The expensive part was rebuilding and exporting too much on every frame, plus mixing render updates with broad application snapshots.

The new architecture should preserve the multi-client benefits of `RenderPlan`, but replace the per-frame snapshot model with per-surface retained render state and small frame deltas.
