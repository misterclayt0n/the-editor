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

## Important consequence

The Electron client currently uses Rust as a:

> viewport-aware cell-grid projection engine

not as a full renderer.

JS / Electron still owns:

- font measurement
- canvas painting
- line numbers / gutters
- decorations / overlays
- any richer product UI

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
