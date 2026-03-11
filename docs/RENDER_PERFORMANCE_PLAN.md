# Render Performance Plan

## Goal

Fix rendering and scrolling performance across both clients without abandoning the current
architecture vision.

The core direction is:

- keep `the-lib` as the source of truth for editor semantics and visible content
- keep the display-list / `RenderPlan` contract
- stop rebuilding and republishing unrelated state on every scroll/update
- make render output revisioned, diffable, and cache-friendly
- improve `the-term` first at the shared render layer
- move `the-swift` to a native AppKit/Metal surface after the shared path is fixed

This is not a Swift-only problem. The terminal client also feels slower than desired while
scrolling, which means the shared render path is part of the bottleneck.

---

## Current Diagnosis

### 1. Full render plans are rebuilt on every render

Today both clients request a fresh `RenderPlan` / `FrameRenderPlan` for each render pass.

That means:

- visible text rows are reformatted again
- gutter data is rebuilt again
- selection/cursor/decorations are rebuilt again
- pane render plans are rebuilt again

Some caches exist today, but the final render product is still reconstructed every time.

### 2. Shared render work is doing too much on scroll

The most serious shared issue is that line annotations can disable the fast-start path in
`build_plan`, causing scroll to fall back toward document-start traversal behavior.

This makes features such as inline diagnostics much more expensive than they should be during
viewport movement.

### 3. Swift scroll is attached to a giant refresh path

The Swift client currently routes scroll through a broad `refresh()` path that can touch:

- background polling
- UI tree JSON
- buffer tab JSON
- frame render plan generation
- key hint state
- file picker state
- file tree state
- native window metadata

Most of that work is unrelated to scrolling a buffer.

### 4. Swift redraw also rebuilds presentation objects

After paying for a full core render plan, the Swift client then pays again by:

- bridging visible span text into Swift strings
- creating fresh SwiftUI `Text` values per span
- doing theme/style lookups in hot draw paths
- redrawing via immediate-mode `Canvas`

This is why the Swift client currently suffers more than the terminal client.

### 5. Multi-pane cost scales badly

Pane rendering currently rebuilds inactive pane plans too, and some inactive-pane paths do not
reuse highlighting caches well enough. Splits increase cost faster than they should.

---

## Architectural Decision

We should **keep the display-list architecture** and make it incremental.

We should **not** move `the-lib` to a Ghostty-style renderer-owned state machine at this stage.

This remains aligned with:

- dumb clients
- deterministic render output
- clean FFI boundaries
- multi-client support
- testable render semantics

The part that must change is not the existence of `RenderPlan`, but the fact that it is currently
treated as a fully rebuilt snapshot with no explicit revisions, damage, or retained scene model.

---

## End State

The target system should behave like this:

1. Rust still computes editor-visible content and layout semantics.
2. Rust emits revisioned pane/frame render snapshots with explicit damage.
3. Decorations are separated from base text layout so scroll stays cheap.
4. Clients retain scene state and only rebuild what changed.
5. The terminal client benefits immediately from lower shared render cost.
6. The Swift client later mounts a native AppKit/Metal surface that consumes the improved
   incremental render contract.

---

## Non-Negotiable Constraints

- `the-lib` remains platform-agnostic.
- Rust remains the source of truth for editor semantics, selection state, and visible content.
- FFI remains a bridge, not a renderer.
- `the-term` must continue working without platform-specific rendering assumptions.
- SwiftUI remains app chrome, not the final editor pixel renderer.

### Must not do

- Do not push Metal/AppKit details into Rust.
- Do not create a second Swift-owned semantic document model.
- Do not optimize only the Swift client while leaving the shared render path structurally expensive.
- Do not replace the display-list model with a renderer-owned state model unless the incremental
  display-list approach proves insufficient after this plan is executed.

---

## Workstreams

## 1. Instrumentation and Baselines

Before changing the architecture, establish stable profiling for scroll-heavy workloads.

### Required outputs

- core render time while scrolling
- client draw time while scrolling
- per-pane cost
- with/without inline diagnostics
- single-pane vs split-pane
- Swift vs terminal comparisons on the same file

### Existing hooks to use

- Swift editor perf logging
- FFI UI/profile logging
- terminal render perf logging

### Success criteria

- we can attribute time to shared render generation vs client presentation
- we can confirm whether line annotations, panes, syntax highlighting, or client redraw dominate

---

## 2. Narrow the Update Paths

The system needs separate update pipelines instead of a single giant refresh.

### Introduce distinct refresh classes

- runtime/background polling
- render-surface snapshot refresh
- chrome/UI refresh
- file picker refresh
- file tree refresh
- window metadata refresh

### Rules

- scrolling a text pane must only request render-surface updates
- typing should usually touch render-surface updates and only relevant UI overlays
- chrome/UI updates must not be coupled to every viewport move

### Swift implications

`EditorModel.refresh()` should be split into narrower refresh methods so scroll does not decode UI
tree JSON, buffer-tab JSON, picker state, tree state, and window presentation on every tick.

### FFI implications

Background polling should not be redundantly triggered by multiple unrelated APIs during one user
gesture. The FFI surface/render API should not implicitly repoll background state if the caller
already did so.

---

## 3. Redesign the Render Contract Around Generations and Damage

The current contract is too blunt. Object replacement is being used as change detection.

### Add explicit generations

Per pane:

- `layout_generation`
- `text_generation`
- `decoration_generation`
- `cursor_generation`
- `scroll_generation`
- `theme_generation`

Per frame:

- `frame_generation`
- `pane_structure_generation`

### Add explicit damage

At minimum:

- changed row range
- changed pane ids
- full-vs-partial invalidation reason

### Why

Clients should be able to answer:

- did text content change?
- did only scroll change?
- did only cursor blink/state change?
- did only pane layout change?

without diffing whole object graphs.

---

## 4. Separate Base Text Layout from Decorations

This is the biggest shared render correction.

### Base text layer

The base text layer should own:

- visible rows
- spans/runs
- gutter text
- content offsets
- wrap results

This path must preserve fast-start behavior during scroll.

### Decoration layer

Separate layers should own:

- selections
- cursors
- hover highlights
- underlines
- inline diagnostics
- end-of-line diagnostics
- transient overlays

### Key rule

Inline diagnostics and other line annotations must no longer force the base text render traversal
onto a slow path during ordinary scrolling.

If necessary, inline diagnostics should be computed from base visible rows plus targeted visual
position helpers, not by making the main text pass annotation-heavy.

---

## 5. Add Persistent Per-Pane Render Caches

We need real retained render caches, not just partial helper caches.

### Cache scope

Per pane:

- visible row metadata
- shaped or merged line runs
- gutter payload
- syntax highlight spans
- decoration payload where feasible

### Cache keys

- document version
- text-format signature
- annotation generation
- viewport
- pane id
- theme generation

### Important rule

Inactive panes must reuse caches too. Split-heavy layouts should not rebuild from scratch when
nothing in those panes changed except active focus elsewhere.

---

## 6. Improve `the-term` First

The terminal client is the best first consumer of the improved shared render path because it makes
shared costs obvious.

### Why this phase comes first

- it validates the cross-client architecture
- it proves whether the render contract changes actually fix scroll pain
- it avoids hiding shared render problems behind Swift-specific work

### Expected outcome

After generations, damage, decoration separation, and pane caches are in place:

- scroll should spend materially less time in shared plan generation
- terminal redraw should mostly pay for visible content changes and terminal buffer diffing
- split panes should scale more reasonably

If this phase does not produce meaningful improvement, we should revisit whether the display-list
architecture is still sufficient.

---

## 7. Swift Stopgap on the Existing Surface

Before the AppKit/Metal migration, the Swift client still needs a less stupid presentation path.

### Required improvements

- no broad `refresh()` on scroll
- no hot-path theme FFI lookups during draw
- no per-span fresh `String` + fresh `Text` construction each frame
- retained Swift-side scene derived from render generations
- line layout caching keyed by content/style/font/scale

### What this gives us

- immediate speedup before the native surface lands
- a better scene/cache model that can be reused by the later native renderer

This phase is not the final destination, but it prevents wasted time on the current path.

---

## 8. Swift Native Surface

Only after the shared render path is fixed should we do the full Swift surface refactor.

### Final Swift architecture

- SwiftUI shell for chrome and overlays
- native AppKit surface host
- native scroll ownership
- retained scene translation from render generations
- Metal renderer for shipping

### Why this phase is last

If the terminal client still feels slow, a Swift renderer rewrite alone cannot solve the shared
problem. The native Swift surface should consume a better render contract, not compensate for a
bad one.

---

## Proposed Execution Order

### Phase 0: Lock design and baseline measurements

- record representative scroll benchmarks
- identify current top offenders with diagnostics enabled

### Phase 1: Split update paths

- narrow refresh APIs in Swift and FFI
- eliminate redundant background polling

### Phase 2: Revisioned render contract

- add generations and damage to pane/frame snapshots

### Phase 3: Base text vs decoration separation

- keep scroll-fast base layout independent from heavy overlays

### Phase 4: Persistent pane render caches

- active and inactive pane cache reuse

### Phase 5: Terminal client adoption

- consume incremental render output in `the-term`
- verify scroll improvement

### Phase 6: Swift retained-scene stopgap

- eliminate per-frame text/style rebuilding on the existing surface

### Phase 7: AppKit/Metal surface

- replace SwiftUI `Canvas`
- preserve the improved render contract

---

## Success Criteria

The plan is successful when:

- scrolling in `the-term` feels materially better than it does today
- the shared render path no longer shows document-start-like slowdowns when diagnostics/annotations
  are active
- split panes do not multiply render cost as sharply as today
- Swift scroll no longer triggers broad non-render refresh work
- Swift can scroll and blink from retained scene data rather than rebuilding draw primitives every
  frame
- the final native Swift surface is an acceleration step, not the first real fix

---

## Decision Checkpoint

After Phases 3 through 5, reassess.

If the incremental display-list path produces strong gains, continue.

If the system is still too slow even after:

- narrow refresh paths
- generation/damage support
- decoration separation
- retained pane caches
- terminal validation

then revisit the architecture question and evaluate whether a more state-driven rendering model is
necessary for some clients.

Until then, the correct move is to make the existing display-list architecture behave like a modern
incremental rendering pipeline rather than discarding it prematurely.
