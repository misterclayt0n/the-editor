# Cell-grid Rendering

This document describes the **current** built-in render kernel.

It is intentionally small.

## Scope

Rust owns a pure **cell-grid projection** path for monospace rendering:

- soft-wrap layout
- document visual-row indexing
- viewport-local visible row extraction
- row metadata
- selection projection
- cursor projection
- syntax projection
- full visible-frame projection

It does **not** try to be a full UI render system.

## Rust owns

Core modules:

- `render/text_format.rs`
- `render/doc_formatter.rs`
- `render/layout_index.rs`
- `render/visible_layout.rs`
- `render/layer_projection.rs`
- `render/surface.rs`

These are used to turn:

- document text
- wrap policy
- viewport
- cell metrics
- selection
- syntax spans

into a `RenderFrame`.

## Rust does not own

These are no longer part of the built-in render kernel:

- widgets
- gutters
- overlays
- text annotations / virtual text
- render manager
- retained presentation helpers
- snapshot/present render APIs

Those concerns belong in higher layers, typically TypeScript / client code.

## Public shape

### Surface view config

```rust
pub struct SurfaceViewSpec {
  pub viewport: Viewport,
  pub format: TextFormat,
  pub measurements: CellGridMetrics,
  pub cursor_semantics: CursorSemantics,
}
```

### Main render seam

```rust
impl Surface {
  pub fn raw_frame_delta(
    &mut self,
    document: &Document,
    syntax_registry: Option<&LanguageRegistry>,
  ) -> RenderFrame;
}
```

### Runtime seam

```rust
impl RuntimeSession {
  pub fn render_surface(
    &mut self,
    surface: SurfaceHandle,
  ) -> Result<RenderFrame>;
}
```

The Electron render bridge calls this through a single frame request that carries concrete renderer measurements:

```ts
renderBridge.renderFrame({
  surfaceId,
  viewport,
  metrics,
});
```

The native bridge applies `metrics` and `viewport` to the `Surface`, then renders. This is intentionally stateful: `Surface` remains the owner of viewport/metrics/reveal/render state.

## Important consequence

The Electron client uses Rust as a:

> viewport-aware cell-grid projection engine

not as a full renderer.

JS / Electron still owns:

- font measurement
- canvas painting
- line numbers / gutters
- decorations / overlays
- any richer product UI

But normal visible editor frames are not part of the general workbench/app snapshot. The workbench snapshot identifies which `SurfaceId` belongs in which pane. The renderer requests `RenderFrame` payloads through the explicit render bridge after surface invalidation, scroll, or resize.

The normal paint path is:

```txt
surface invalidated / scrolled / resized
  -> renderer computes viewport + metrics
  -> renderer calls renderBridge.renderFrame(surfaceId, viewport, metrics)
  -> Electron main calls Rust
  -> Rust updates Surface viewport/metrics and returns RenderFrame
  -> renderer paints canvas
```

The policy worker may still request frames for utility cases such as picker previews, file previews, or headless tests, but it must not broker normal visible editor painting.

## Electron ligature shaping

The current Electron renderer intentionally allows browser/font shaping, including ligatures.

This lives in:

- `apps/electron/src/renderer/src/textSurface.ts`

### How it works

Rust still projects the visible frame in **cell-grid coordinates**:

- `RenderRow.text`
- selection spans in cols
- cursor placements in cols
- syntax spans in cols

Electron then paints that frame on canvas.

The important renderer change was:

- **before**: draw one cell at a time with `fillText(cell, x, y)`
- **now**: draw contiguous row/span text with one `fillText(text, x, y)` call

That gives the browser a whole shaped text run like:

- `===`
- `->`
- `!=`

instead of isolated single-cell glyph draws, so ligatures can form.

### Concrete implementation details

In `textSurface.ts` we do two important things:

1. enable normal font shaping
   - `ctx.fontKerning = "normal"`
2. paint text runs instead of per-cell glyphs
   - `drawCells(...)` now joins the requested cell slice into a string and paints it with one `fillText(...)`

`paintRows(...)` still respects syntax spans, but each span is now painted as a shaped run instead of character-by-character.

### What did **not** change

We did **not** change the Rust render contract.

Rust still decides:

- row breaks
- wrap layout
- visible rows
- selection columns
- cursor columns
- syntax columns

Electron still uses the cell-grid metrics from the current font to configure the viewport and to place:

- selections
- decorations
- cursors

So the architecture is still:

- Rust = cell-grid projection truth
- Electron = shaped painting policy

### Debugging note

If ligature rendering introduces visual bugs, the most likely place to inspect first is:

- `apps/electron/src/renderer/src/textSurface.ts`

especially these responsibilities:

- `measureCellWidth(...)`
- `paintRows(...)`
- `drawCells(...)`
- cursor painting vs shaped text positioning
- selection rect painting vs shaped text positioning

The likely class of bugs is:

- text visually shaped as one run
- but cursor/selection rectangles still positioned from cell-grid columns

That tradeoff is currently intentional.

## Why `LayoutIndex` still matters

Even after slimming the render system down, `LayoutIndex` is still important because `Surface` depends on it for:

- reveal / scrolling decisions
- char -> visual row/col mapping
- visible-window resume points for wrapped layout

That is the main remaining coupling between `Surface` and the render kernel.

## Design rule

If a feature changes **document-to-cell-grid truth**, it may belong in this kernel.

If a feature is primarily:

- UI composition
- decoration
- app chrome
- product behavior
- client-owned visual policy

it should stay out of this kernel.

Performance rules:

- do not serialize normal visible `RenderFrame` payloads as JSON long-term
- do not copy full document text to TypeScript during normal typing/render refresh
- do not send visible `RenderFrame` payloads inside workbench snapshots
- surface invalidation events trigger frame requests
- renderer frame caches are presentation caches only, not semantic truth
