# Swift Render Surface Refactor Plan

## Goal

Replace the current SwiftUI `Canvas` editor renderer with a Ghostty-style native surface architecture so the macOS client can scroll, type, blink, and repaint like a real desktop app instead of like a SwiftUI drawing demo.

The target outcome is:

1. SwiftUI owns chrome, overlays, menus, and window composition.
2. A custom AppKit surface owns editor drawing, scrolling, focus, and input.
3. The shipping renderer is Metal-backed.
4. Rust keeps ownership of editor semantics and deterministic render output.
5. The existing multi-client architecture stays intact.

This is a complete rendering refactor, not another incremental `Canvas` optimization pass.

## Hard Decision

The final editor path in `the-swift` should not use SwiftUI `Canvas`.

The Ghostty lesson worth copying is not "use Zig" or "copy terminal internals." It is this split:

- SwiftUI shell
- native surface host
- renderer-owned invalidation
- OS-native scrolling/input
- platform renderer attached directly to the view

That is the architecture this plan adopts.

## Why This Direction

Current Swift bottlenecks are architectural, not just local hot spots:

- `EditorModel.refresh()` rebuilds `framePlan` and `plan` every refresh in `the-swift/Sources/TheSwift/EditorModel.swift`.
- The static editor surface identity is tied to `ObjectIdentifier(model.framePlan)` / `ObjectIdentifier(model.plan)` in `the-swift/Sources/TheSwift/EditorView.swift`.
- `drawText(...)` rebuilds attributed strings for every visible line on every draw in `the-swift/Sources/TheSwift/EditorView.swift`.
- Background polling still drives redraw pressure via a 30 Hz timer in `the-swift/Sources/TheSwift/EditorModel.swift`.
- Scroll behavior is still mediated through SwiftUI/update cycles instead of a native scroll container.

We already improved the current path materially, but the logs still show the structural cost of:

- immediate-mode drawing
- broad view invalidation
- per-frame text reconstruction
- SwiftUI-centered scroll/render ownership

Ghostty avoids those problems because it does not ask SwiftUI to be the renderer. SwiftUI just hosts a native surface via `OSViewRepresentable` in `/Users/misterclayt0n/code/ghostty/macos/Sources/Ghostty/Surface View/SurfaceView.swift`, and the renderer installs its own layer directly on the platform view in `/Users/misterclayt0n/code/ghostty/src/renderer/Metal.zig`.

## Non-Negotiable Constraints

This refactor should match Ghostty's client architecture without compromising core editor architecture.

### Must preserve

- `the-lib` stays deterministic and platform-agnostic.
- Rust remains the source of truth for editor semantics, selection state, layout, and visible content.
- FFI remains a bridge layer, not a platform renderer.
- `the-term` must not be forced into a Metal/AppKit-specific model.
- SwiftUI remains the shell for app chrome, not the pixel renderer.

### Must not do

- Do not turn `the-lib` into a Ghostty-style renderer-owned state machine.
- Do not push platform rendering details into Rust.
- Do not keep a second Swift-owned semantic model of the document.
- Do not stop at "custom `NSView` that still redraws everything with CoreText each frame" as the final shipping design.

## Target Architecture

### High-level shape

The Swift client should look like this:

- `EditorView`
  - SwiftUI chrome and overlay composition
  - `EditorSurfaceRepresentable`
    - `EditorSurfaceScrollView` (`NSScrollView` wrapper)
      - `EditorSurfaceView` (`NSView`, layer-hosting)
        - `MetalEditorRenderer`

### Ownership split

- Rust owns:
  - editor state
  - viewport semantics
  - render-plan generation
  - pane metadata
  - diagnostics/cursor/selection content

- Swift owns:
  - surface lifecycle
  - input event plumbing
  - native scrolling behavior
  - render scheduling
  - line/glyph/texture caches
  - platform drawing backend

### Surface components

#### 1) `EditorSurfaceRepresentable`

SwiftUI wrapper that mounts the editor surface into the view tree.

Responsibilities:

- create and retain the native surface host
- pass geometry and environment changes into the surface controller
- keep overlays/chrome outside the render surface

This should be the direct replacement for the current editor `Canvas` subtree in `EditorView.swift`.

#### 2) `EditorSurfaceScrollView`

AppKit scroll container modeled on Ghostty's `SurfaceScrollView`.

Responsibilities:

- own `NSScrollView`
- own a document view representing the scrollable content height/width
- host `EditorSurfaceView`
- translate native scroll position into editor row/column scroll updates
- keep the rendered surface aligned to the visible rect

This is the correct place for trackpad momentum, deceleration, scroller behavior, and visible-rect coordination. It should replace the current SwiftUI/`EditorModel.handleScroll(...)` ownership of scrolling.

#### 3) `EditorSurfaceView`

Custom `NSView` that is first responder and directly owns the renderable surface.

Responsibilities:

- receive key/text/mouse/scroll events
- own focus state
- own layer-backed drawing host
- hand dirty-state/render requests to the renderer
- expose minimal platform hooks to the controller

This should be a layer-hosting AppKit view in the same spirit as Ghostty's `SurfaceView`.

#### 4) `EditorSurfaceController`

Per-surface coordinator object, analogous to Ghostty's surface object model.

Responsibilities:

- own the `EditorId`
- fetch FFI render snapshots/revisions
- maintain render caches
- coordinate scroll state, viewport size, and render scheduling
- manage renderer lifecycle
- arbitrate which updates require:
  - no action
  - partial redraw
  - full scene rebuild

This should become the narrow bridge between Swift app state and the native render surface.

#### 5) `EditorRenderer`

Swift-side rendering protocol with a Metal implementation.

Recommended shape:

```swift
protocol EditorRenderer {
    func attach(to view: EditorSurfaceView) throws
    func detach()
    func resize(to size: CGSize, scale: CGFloat)
    func updateScene(_ scene: EditorRenderScene, damage: EditorDamage?)
    func drawIfNeeded()
}
```

Concrete backends:

- `MetalEditorRenderer`: shipping path
- `CoreTextEditorRenderer`: optional bootstrap/debug backend only

The bootstrap renderer is useful to get the AppKit architecture in place without blocking on Metal, but it is not the end state.

## Rendering Model

### Keep the Rust display-list contract

This refactor does not replace the Rust render-plan model. It changes how Swift consumes it.

The editor should stay:

- Rust core -> render snapshot / display list
- Swift surface -> retained scene / caches
- Metal renderer -> glyphs/quads/highlights/cursors

That is the right compromise between:

- Ghostty's native-surface architecture
- this repo's multi-client display-list architecture

### Add explicit render generations

The current Swift path treats object replacement as change detection. That is too blunt.

Add explicit revision data to the FFI surface contract:

- `frame_generation`
- `text_generation`
- `decoration_generation`
- `scroll_generation`
- optional `damage` / changed row range

Swift should stop invalidating on object identity churn and instead invalidate only when the relevant generation changes.

### Scene translation

`EditorSurfaceController` should translate `RenderFramePlan` into a retained Swift-side scene:

- visible lines
- runs/spans
- line numbers/gutter content
- selections
- cursors
- underlines
- inline diagnostics
- EOL diagnostics
- pane backgrounds/borders

Do not draw directly from FFI wrapper objects every frame.

Build a scene snapshot once per generation change, then render from retained Swift-native buffers.

### Text and glyph caching

The shipping renderer should not rebuild attributed strings per visible line on every draw.

Instead:

- cache shaped line layouts by `(buffer font identity, line content, style runs, scale)`
- cache glyph atlases/textures at the renderer layer
- treat scrolling as a visible-window change, not a text-layout rebuild

If Metal implementation starts with CPU-side line shaping, that is acceptable. The key rule is that shaped text must be retained and reused across frames.

## Scrolling Model

### Native scroll ownership

The editor surface should scroll like Ghostty:

- AppKit owns the physical scroll interaction
- the surface host tracks visible rect changes
- the core receives logical row/column position updates
- the renderer redraws the visible region

This should replace the current pattern where scroll deltas are accumulated in `EditorModel`, coalesced, and then turned into full refreshes.

### Correct scroll responsibilities

- `NSScrollView` / clip view:
  - wheel events
  - trackpad momentum
  - scroller dragging
  - visible rect tracking

- Rust core:
  - clamped logical scroll position
  - viewport semantics
  - visible content selection

- Surface controller:
  - conversion between pixel position and row/column scroll
  - throttling duplicate scroll commands
  - deciding whether the next frame is:
    - pure translation
    - visible-range update
    - full rebuild

## Input and Focus

### Surface-owned input

The editor surface should become first responder and own input directly, rather than depending on a separate SwiftUI key-capture overlay for the main editing path.

Responsibilities of `EditorSurfaceView`:

- keyDown/text input
- IME / marked text
- mouse move/down/drag/up
- magnify / smart zoom if needed later
- focus transitions

The existing high-level command/palette chrome can remain SwiftUI-driven, but the main editor surface should receive events natively.

### Focus synchronization

Focus must stay synchronized across:

- native surface focus
- Rust active pane/editor state
- blink scheduler
- overlay visibility rules

This should remove the current split between SwiftUI focus, cursor blink state, and render invalidation.

## Overlays and Chrome

### Keep in SwiftUI

These should remain outside the editor render surface:

- command palette
- picker panels
- docs popups if they are window-level chrome
- notifications
- tab chrome
- sidebar / rail / toolbar / menus

### Move into renderer if part of the text plane

These should be rendered by the surface backend when practical:

- text
- gutter
- selections
- cursors
- underlines
- inline/EOL diagnostics
- active line highlights
- pane-local decorations

The rule is simple: if it scrolls with the buffer and is visually part of the editor plane, it belongs to the render surface.

## Rust / FFI Contract Changes

### Required near-term additions

1. Stable render generations instead of object-identity-based invalidation.
2. Visible-content revisions split by category:
   - text
   - gutter
   - decorations
   - cursor/blink
3. Scroll metrics suitable for `NSScrollView` integration:
   - total scrollable rows
   - viewport rows
   - row offset
   - horizontal scroll metrics
4. Stable pane/surface identifiers for cache ownership.

### Likely follow-up additions

1. Damage regions or changed row ranges.
2. Flattened scene snapshot APIs that reduce FFI per-span traversal overhead.
3. Optional direct bulk export for visible runs to make Metal scene upload cheaper.

### Explicit non-goal

Do not make Rust expose a platform renderer. Rust should keep exporting deterministic render content, not Metal commands or platform objects.

## Swift File/Module Plan

### New Swift components

Recommended new files:

- `the-swift/Sources/TheSwift/EditorSurfaceRepresentable.swift`
- `the-swift/Sources/TheSwift/EditorSurfaceScrollView.swift`
- `the-swift/Sources/TheSwift/EditorSurfaceView.swift`
- `the-swift/Sources/TheSwift/EditorSurfaceController.swift`
- `the-swift/Sources/TheSwift/EditorRenderScene.swift`
- `the-swift/Sources/TheSwift/EditorRenderer.swift`
- `the-swift/Sources/TheSwift/MetalEditorRenderer.swift`
- `the-swift/Sources/TheSwift/EditorGlyphAtlas.swift`
- `the-swift/Sources/TheSwift/EditorLineLayoutCache.swift`
- `the-swift/Sources/TheSwift/EditorRenderScheduler.swift`

### Existing code to shrink or remove

- the editor `Canvas` rendering path in `EditorView.swift`
- `drawFrame`, `drawPlan`, and `drawText` as the primary editor path
- scroll-coalescing ownership in `EditorModel`
- editor-specific render invalidation driven by SwiftUI object identity

## Migration Phases

### Phase 0: Architecture lock

Deliverables:

- this plan approved
- target surface boundaries agreed
- "no final `Canvas` renderer" decision locked

Exit criteria:

- no more investment in long-term `Canvas` optimization work

### Phase 1: Split shell from surface

Deliverables:

- isolate editor chrome from editor drawing in `EditorView`
- add `EditorSurfaceRepresentable`
- mount a placeholder `EditorSurfaceScrollView` + `EditorSurfaceView`

Exit criteria:

- editor surface can exist as a native view inside SwiftUI with overlays still working

### Phase 2: Native scroll/input host

Deliverables:

- `EditorSurfaceScrollView` owns scrolling
- `EditorSurfaceView` owns first-responder input
- pixel scroll position maps to Rust scroll updates

Exit criteria:

- trackpad scrolling works through the native scroll container
- key/mouse events reach Rust through the native surface

### Phase 3: Generation-based invalidation

Deliverables:

- Rust exports render generations
- Swift surface invalidates only when relevant generations change
- background timers stop forcing static-surface redraws

Exit criteria:

- idle editor does not redraw the text plane unless content actually changed

### Phase 4: Retained scene + line layout caches

Deliverables:

- visible `RenderFramePlan` translated into retained Swift scene objects
- line layout cache introduced
- no per-frame attributed-string rebuilds for unchanged lines

Exit criteria:

- scroll and blink do not rebuild visible line layouts unnecessarily

### Phase 5: Metal renderer bring-up

Deliverables:

- `MetalEditorRenderer` attached to `EditorSurfaceView`
- text plane rendered via renderer-owned layer/backend
- cursors, selections, gutter, and diagnostics moved into renderer path

Exit criteria:

- shipping editor path no longer uses SwiftUI `Canvas`

### Phase 6: Remove old path

Deliverables:

- delete legacy editor `Canvas` path
- delete scroll-refresh workaround code that only existed for SwiftUI rendering
- simplify `EditorModel` refresh ownership around surface revisions

Exit criteria:

- one editor surface path remains

## Performance Targets

These are the targets that justify the refactor.

### Interaction targets

- trackpad scroll should feel native and momentum-preserving
- typing should not visibly block on render work
- cursor blink should not redraw the full text plane
- idle editor should not repaint unless:
  - content changed
  - visible rect changed
  - a surface-local visual effect actually changed

### Measurement targets

On the existing `keymap.rs` workload used during recent profiling:

- steady-state text refresh work in Swift should usually stay near or below 1-2 ms
- steady-state draw cost for unchanged visible content should not require full line reconstruction
- scroll bursts should stay within display refresh budget on a normal 60 Hz display and degrade gracefully on higher refresh-rate panels

The exact numbers can be tightened once the Metal path exists, but the main requirement is architectural: no full immediate-mode rebuild on every interaction.

## Risks

### Main risks

- temporary complexity while both old and new paths coexist
- IME/text-input correctness in a custom AppKit view
- getting `NSScrollView` ownership correct without fighting Rust scroll state
- Metal text rendering complexity if glyph caching is underspecified

### Risk mitigation

- keep a bootstrap renderer available during bring-up
- introduce render generations before full Metal work
- move to native scroll/input early so the architecture is proven before renderer polish
- keep Rust render semantics stable while replacing the client path

## Acceptance Criteria

This refactor is done when:

1. The main editor surface is a custom AppKit host mounted via representable.
2. The shipping editor renderer is not SwiftUI `Canvas`.
3. Scrolling is owned by a native scroll container and feels native.
4. Blink/input/scroll no longer cause broad SwiftUI redraw churn.
5. Rust still owns editor semantics and deterministic render content.
6. Existing overlays/chrome continue to work above the surface.

## Final Recommendation

Copy Ghostty's client architecture, not Ghostty's core coupling.

That means:

- yes to custom AppKit surface hosting
- yes to renderer-owned layer/Metal drawing
- yes to native scroll/input ownership
- no to pushing renderer/platform state into Rust
- no to keeping SwiftUI `Canvas` as the final editor renderer

That is the most correct path if the goal is for this app to actually fly.
