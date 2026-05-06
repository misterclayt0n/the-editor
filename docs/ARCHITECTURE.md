# Editor Architecture

This document defines the target architecture for the new editor.

It is the canonical answer to three questions:

1. What belongs in Rust, and what belongs in TypeScript?
2. What belongs in the pure editor engine versus the impure native runtime?
3. What old-editor code should be ported, rewritten, or left behind?

This document is intentionally opinionated. The old editor mixed too many concerns. The new editor should not.

---

## North Star

The editor is a native Rust editor platform with a TypeScript product layer.

```txt
Rust engine (`the-lib`):      text/state/editing invariants + built-in cell-grid layout/rendering
Rust runtime (`the-runtime`): services, resources, async/impure native orchestration
TypeScript packages:          commands, hooks, keymaps, modes, extensions, defaults, UI composition
Client:                       painting, platform chrome, event loop, native UI resources
```

The editor is not a monolithic app. It is a capability-oriented platform.

A prompt, text input, code editor, minimap, diff experience, or coding-agent surface should all be built from the same native primitives and composed differently by TypeScript.

The built-in renderer is explicitly a **monospace cell-grid renderer**. It is client-agnostic in the sense that a TUI, REPL, Electron canvas, native GUI, or test harness can paint its output, but it is not a universal proportional-font layout engine.

A client may still choose to do extra paint-time shaping policy on top of that projection. For example, the current Electron canvas renderer paints row/span text as browser-shaped runs so ligatures can appear, while still using Rust's cell-grid row/col projection for viewport layout, selections, and cursor placement.

The default editor experience is only one distribution. In the future, an alternate distribution should be able to replace most of the product layer in TypeScript, including commands, hooks, keymaps, layout, and UI composition.

---

## Core goals

- Keep the native engine minimal, sound, performant, and correct.
- Make TypeScript powerful enough to define the editor's personality.
- Keep authoritative semantic editor state in Rust.
- Avoid split-brain document models between Rust and JS.
- Support multiple clients without duplicating editor semantics.
- Support deep extensibility without pushing hot loops into JS.
- Preserve a clean porting rule for old-editor code.

---

## Non-goals

These are explicitly **not** goals for `the-lib`:

- defining the editor's command system
- defining keymaps or modal behavior
- defining app chrome like tabs, panes, file tree, command palette, statusline
- owning LSP protocol sessions
- owning FFF/workspace search integration
- owning persistence, file watching, git, clipboard, or other impure services
- owning client painting or platform UI
- owning universal GUI text shaping, proportional-font measurement, or framework-specific layout semantics
- becoming a dump for every feature that happens to be written in Rust

---

## Final crate/package shape

Preferred shape:

```txt
the-editor/
  the-stdx/      # reusable Rust/platform utilities
  the-lib/       # pure editor engine
  the-runtime/   # impure native runtime and services
  packages/
    editor/      # TS command/hook/extension/product framework
    default/     # first-party default distribution
```

Dependency direction:

```txt
the-stdx <- the-lib <- the-runtime <- TypeScript packages
```

The client sits beside this stack and talks to the runtime for native state and render data.

### Why this shape

#### `the-stdx`
Purpose:
- reusable utilities
- no editor personality
- no runtime services

Reasoning:
- low-level path/env/range/rope helpers are useful across crates
- these utilities should stay boring and portable

#### `the-lib`
Purpose:
- pure editor engine
- semantic text state
- deterministic editing operations
- built-in cell-grid layout and visible-frame projection

Reasoning:
- this is where correctness, invariants, and hot-path performance matter most
- this crate must stay small enough to reason about clearly
- this crate should be reusable regardless of LSP/search/git/product choices

#### `the-runtime`
Purpose:
- impure native runtime
- resources, services, background work, async orchestration, platform/process/filesystem integration
- authoritative native session state above the pure engine

Reasoning:
- a real editor needs impure Rust
- forcing all impure behavior into TS would be artificial and slow
- forcing all impure behavior into `the-lib` would destroy the engine boundary
- one runtime crate is simpler than creating feature crates like `the-fff/`, `the-lsp/`, etc. too early

#### TypeScript packages
Purpose:
- commands
- hooks
- keymaps
- modes
- extension loading
- product composition
- app/workspace chrome
- alternate distributions

Reasoning:
- this is the layer users and extensions should be able to replace
- this is where editor personality belongs
- this is how the editor can support radically different product layers without rewriting the native substrate

---

## Top-level ownership model

The architecture has three important native object families:

1. `Resource`
2. `Document`
3. `Surface`

Plus native runtime service/session objects.

### `Resource`
A `Resource` is an external-world identity.

Examples:
- a file path
- a URI
- a watched filesystem object
- a workspace-known but not currently loaded file

A `Resource` is **not** text state.

Reasoning:
- text editing and file identity are not the same thing
- prompts, scratch buffers, previews, generated buffers, snapshots, and AI-generated content should not be forced to pretend to be files
- runtime services like watch/save/LSP/workspace search need an external-world anchor that is not the same as a text document

### `Document`
A `Document` is semantic text state.

It owns things like:
- text storage
- transactions
- monotonic revisions/generations
- syntax state
- narrow document-local text metadata like line ending and indent style
- semantic attachments that belong to the text itself

A `Document` is explicitly resource-agnostic.

Reasoning:
- document truth should stay about text and derived text state
- history, selection, and other session facts do not belong to shared text truth
- resource identity belongs to runtime, not the pure engine
- this makes scratch buffers, prompts, previews, and generated docs natural

### `Surface`
A `Surface` is a per-view native object attached to one primary document.

It owns things like:
- selections and stable cursor IDs
- active cursor / primary-cursor state
- visual goals
- surface-local undo/redo history
- viewport/scroll state
- reveal behavior
- cell-grid view state via `CellGridRenderSurface`
- render capabilities/options
- per-view interaction state

A `Surface` is not a pane tree, tab, or app widget. It is the native editor/view object.

Reasoning:
- selection does not belong on `Document` if many views can show the same text
- history restores exact committed selection snapshots and is therefore surface/session-local
- a surface is the right unit for cell-grid view/layout state
- pane/tab/split layout is product composition and should stay above the native engine/runtime

### Native service/session handles
The runtime also owns long-lived service/session objects.

Examples:
- `LspSessionId`
- `SearchSessionId`
- `WatchSubscriptionId`
- `WorkspaceId`

Reasoning:
- LSP, search, watch, and similar domains are stateful and async
- a handle/event model scales better than pretending all services are stateless request/response calls

---

## Object relationships

### Documents and resources are independent
A `Document` may exist with no `Resource`.
A `Resource` may exist with no loaded `Document`.

Reasoning:
- supports scratch docs, prompts, generated docs, previews, snapshots
- supports runtime knowledge of workspace resources before loading text

### A surface has one primary document
A `Surface` is attached to exactly one primary document.

Reasoning:
- this keeps the native render and selection model simple and sound
- diff and other multi-document products can be composed from multiple surfaces
- it avoids baking product-shaped view kinds into the core

### Many surfaces may share one document
A single document may be viewed by many surfaces.

Reasoning:
- this is the clean payoff of moving selection to surfaces
- it supports splits, secondary views, minimaps, readonly mirrors, etc.

---

## Native authority model

Rust is authoritative for semantic editor state.
TypeScript is authoritative for behavior and product composition.

### Rust authoritative state
Rust owns the canonical truth for:
- documents
- surfaces
- selections
- syntax
- annotations/decorations
- revisions/generations
- runtime service state
- render state

### TypeScript authoritative state
TypeScript owns the canonical truth for:
- commands
- hooks
- keymaps
- modes
- editor personality
- pane/tab/split/workspace layout
- UI composition
- extension behavior
- alternate distributions

### Why this split
This preserves both goals:

1. Rust owns correctness, invariants, and hot-path state.
2. TypeScript remains deeply extensible and able to replace the product layer.

This is the critical distinction:

> Rust being authoritative does **not** mean TypeScript is weak.
>
> It means TypeScript programs the editor platform instead of reimplementing the text engine.

---

## Capability model

The editor is capability-oriented, not product-kind-oriented.

A prompt, text input, code editor, or diff experience should be composed from capabilities rather than implemented as separate unrelated native products.

### Capabilities are split by ownership
Some capabilities belong to `Document`. Others belong to `Surface`.

#### Document-owned capabilities
These are semantic/text features.

Examples:
- text
- syntax state
- semantic diagnostics data
- document-local revisions
- line ending / indent-style metadata

Reasoning:
- these are intrinsic to the text, not to a particular view
- many surfaces should be able to reuse them

#### Surface-owned capabilities
These are per-view/per-interaction features.

Examples:
- selection and stable cursor IDs
- active cursor and visual goals
- surface-local history
- viewport/scroll state
- reveal behavior
- wrapping and line-display options
- single-line behavior
- render options
- visible decoration layers
- cell-grid view state
- readonly-as-view-policy

Reasoning:
- these vary by view
- they should not pollute document truth

### Avoid fixed native product kinds
A "code editor" should mostly be a preset, not a fundamental native species.

Reasoning:
- fixed product kinds tend to accrete special cases
- capability composition is more flexible and matches the extensibility goal
- TypeScript should be free to define new combinations without waiting for Rust to invent new built-in products

---

## What belongs in `the-lib`

Put something in `the-lib` when it is any of the following:

- semantic text state
- deterministic state transition logic
- hot-path editor math
- syntax/tree-derived state tied to text
- selection/cursor primitives
- viewport-first cell-grid layout/projection semantics
- reusable editing primitives expressed as text/selection/syntax -> result or transaction

### Examples
- rope/text storage
- transactions
- undo/redo engine
- selections/cursors primitives
- movement
- text objects/object expansion
- surround/comment/match-brackets/indent/auto-pairs
- search within a document
- diff primitives
- syntax state and queries
- diagnostic/annotation data structures if they are generic and text-bound
- viewport/scroll/wrap/fold/visual-row mapping
- cell-grid surfaces and visible-frame projection

### Reasoning
This is the engine substrate that should stay:
- fast
- deterministic
- sharable across clients
- independent of product personality and impure services

### Editing operation layering
The editing operation stack in `the-lib` should be explicit and file-shaped:

```txt
movement.rs   = movement-related pure logic and movement semantics
editing.rs    = edit-related pure logic, fragment extraction, paste planning, delete span planning
operation.rs  = default semantic operations assembled from movement/editing, plus thin named dispatch
```

Rules:
- plain functions in `movement.rs` and `editing.rs` are the real `the-lib` API
- `operation.rs` may add small default semantic policy, but it must stay pure
- `operation.rs` should return composable pure operation outputs/atoms such as edit application and register writes
- `execute_operation(...)` is only a convenience bridge for single-step semantic operations
- `operation.rs` should not contain substantial algorithmic logic; push that down into `movement.rs` or `editing.rs`
- multi-step workflows such as `open_below(); insert_mode()` or `change_selection(); insert_mode()` belong in TypeScript
- clipboard/register source-or-target flavored commands belong in TypeScript workflows over runtime APIs, not in `the-lib`

---

## What belongs in `the-runtime`

Put something in `the-runtime` when it is any of the following:

- impure
- async
- process/filesystem/network based
- service-oriented
- resource- or workspace-scoped
- runtime/session orchestration
- native integration above the pure engine

### Examples
- LSP sessions and server lifecycle
- FFF/workspace search integration
- file watching
- open/save/reload/persistence
- git/VCS integration
- clipboard integration
- grammar loading/runtime parser management
- background task orchestration
- document/resource binding
- workspace model
- service/session handles and event routing

### Reasoning
A real editor needs these things in Rust, but they are not engine purity.
Putting them in `the-runtime` keeps the architecture honest:
- impure services remain native and efficient
- `the-lib` stays minimal and comprehensible
- TypeScript still gets a powerful platform to compose against

---

## What belongs in TypeScript

Put something in TypeScript when it is any of the following:

- behavior or policy
- user-overridable workflow
- command or hook composition
- keymap or mode logic
- app/workspace layout and chrome
- default setup or distribution behavior
- extension-facing composition glue
- structured providers into native domains

### Examples
- command registry
- hook system
- keybindings
- Vim/Emacs/Helix/Doom-style behavior layers
- command palette behavior
- picker behavior
- pane/tab/split/workspace layout
- default LSP/search/git UI flows
- extension loading
- custom providers for decorations/widgets/completions/code actions/etc.
- multi-step workflow composition over `the-lib` semantic ops
- mode transitions, pending-key logic, register choice, and clipboard policy

### Reasoning
This is the replaceable product layer.
If a future distribution wants to redefine the editor's personality, it should mostly live here.
The important split is:
- `the-lib` may provide useful single-step semantic defaults
- TypeScript owns sequencing and workflow composition
- `the-runtime` supports those workflows but does not define them

---

## What belongs in the client

Put something in the client when it is any of the following:

- platform-specific painting
- native UI resources
- frame loop / presentation
- AppKit/SwiftUI/Metal/web/terminal details
- platform event loop and host chrome

### Examples
- terminal painting
- Metal or wgpu drawing
- SwiftUI/AppKit windowing and UI chrome
- DOM/canvas rendering
- glyph atlases and native text objects
- IME handling and platform event routing

### Reasoning
The client paints. It should not own semantic editor truth.

---

## Boundary rules

### Rule 1: JS decides intent; Rust performs native work

Good:

```ts
runtime.documents.applyTransaction(documentId, tx)
runtime.surfaces.renderFrame({ surfaceId, viewport })
```

Bad:

```ts
for (const row of visibleRows) {
  const line = runtime.documents.getLine(documentId, row)
  const wrapped = wrapInJs(line)
  draw(wrapped)
}
```

Reasoning:
- avoid chatty per-row/per-token/per-grapheme APIs
- keep hot loops in Rust

### Rule 2: TypeScript must not maintain a second semantic document model

Reasoning:
- avoids split-brain state
- avoids correctness drift between Rust and JS
- keeps the runtime authoritative

### Rule 3: The client must not maintain a semantic document model

Reasoning:
- the client should retain presentation, not semantics
- semantic queries should go back to Rust

### Rule 4: Rendering should stay pure and viewport-first

Reasoning:
- Rust should own cell-grid layout truth
- clients should own painting and richer presentation policy

### Rule 5: App/chrome state is separate from surface render state

Reasoning:
- scrolling a text surface must not rebuild file tree, command palette, tabs, statusline, etc.

---

## Rendering position

The built-in cell-grid renderer is split into two stages.

```txt
Rust (`the-lib`):   viewport-first cell-grid layout + visible-frame projection
Client:             native painting and presentation scene
TypeScript:         orchestration + providers + surface composition
```

Rust owns the built-in monospace path:
- viewport-first cell-grid text layout
- visible-row mapping
- column-based wrap
- visible-window resume/indexing helpers
- visible-frame projection

Client owns:
- painting
- GPU/native text resources
- platform presentation
- frame pacing/presentation loop
- non-cell-grid layout/rendering paths

TypeScript owns:
- decoration/provider composition
- deciding which surfaces exist and how they are arranged

The important rendering decision is this:

> `CellGridRenderSurface` is a viewport-first **cell-grid projection helper**, not a whole-document layout owner.

There is no document-wide layout cache at the center of the architecture.
Rows, row metadata, selections, cursors, and syntax spans should all derive from a single shared visible-layout pass for the viewport plus a small margin.

For more detail, see `docs/RENDERING.md`.

---

## Next-generation syntax model

The long-term syntax architecture should no longer be centered on eager full-document highlight payloads.

The intended split is:

```txt
Document      = persistent syntax truth
Surface       = visible syntax extraction/cache + render adapter
CellGridRenderSurface = retained syntax-agnostic renderer
Runtime       = syntax scheduler/orchestrator
```

### Document-owned syntax truth
`Document` owns:
- persistent root + injected syntax layer graph
- invalidated regions
- syntax generations/freshness state
- document-scoped baseline/enrichment truth

`Document::apply_transaction(...)` should stay cheap. It should:
- apply text edits
- interpolate/map syntax state forward
- mark the necessary regions/layers stale
- never become an unbounded full reparse/highlight pass on the typing hot path

### Surface-owned visible syntax projection
`Surface` owns:
- current syntax demand for its viewport
- visible baseline syntax cache
- visible enrichment cache
- byte/char/visual conversion near render

It should stop being fed document-global syntax span vectors as its primary syntax input.

### Runtime-owned syntax scheduling
`the-runtime` owns:
- grammar/query acquisition
- per-document syntax work queues
- small bounded foreground advancement
- background completion
- stale-result rejection
- demand-aware prioritization from surfaces
- degraded large-file policy

### Tiered syntax model
The complete model should be tiered:

1. **Tier 1 baseline**
   - fast structural syntax paint
   - viewport/range extracted
   - good enough for immediate feedback
2. **Tier 2 enrichment**
   - locals-sensitive refinements
   - semantic tokens
   - other richer semantic overlays
   - async and stale-tolerant

### Rendering consequence
The render path should consume visible syntax projections, not whole-document highlight truth.

That means:
- `CellGridRenderSurface` stays syntax-agnostic
- `Surface` adapts syntax extraction into render-visible payloads using the visible layout
- runtime pushes freshness notifications, not whole-document syntax payloads

For more detail, see `docs/SYNTAX.md`, `docs/SURFACE.md`, `docs/DOCUMENT.md`, `docs/RUNTIME.md`, and `docs/RENDERING.md`.

---

## Extensibility model

The editor should be highly extensible through TypeScript without making TypeScript the semantic source of truth.

### TypeScript should be able to:
- replace the default product layer
- define commands/hooks/keymaps/modes
- create and compose surfaces dynamically
- define pane/tab/split/workspace layout
- build alternate distributions
- register structured providers into native domains
- contribute decorations, widgets, completions, code actions, etc.

### TypeScript should not need to:
- own text storage
- own selection truth
- own syntax truth
- own render/layout truth
- reimplement native services like LSP/search/git internally

### Handle-and-event model
TypeScript should work with stable native handles and subscriptions.

Examples:
- `DocumentId`
- `SurfaceId`
- `ResourceId`
- `SearchSessionId`
- `LspSessionId`

Reasoning:
- this is a better platform model than snapshot/mirror sync
- it allows TypeScript distributions to program the editor deeply without owning the engine state

### Structured providers, not chatty callbacks
TypeScript may contribute structured providers into runtime domains and surfaces, but the boundaries must remain coarse-grained and revisioned.

Reasoning:
- this preserves extensibility without sacrificing performance
- providers should submit structured data, not run per-row native callbacks

---

## Native runtime API shape

The runtime should expose coarse-grained domain APIs, not a bag of unrelated low-level functions.

Examples:

```txt
runtime.documents.*
runtime.surfaces.*
runtime.resources.*
runtime.workspace.*
runtime.search.*
runtime.lsp.*
runtime.git.*
runtime.providers.*
```

Reasoning:
- easier to document
- easier to reason about
- easier for alternate distributions to build against
- discourages accidental chatty APIs

---

## Revision model

Use simple monotonic counters/generations for now.

Examples:
- document text revision
- syntax generation
- annotation generation
- surface render generation

Reasoning:
- enough for invalidation, staleness checks, and event ordering
- avoids overdesigning revision identity too early
- richer history identity can still exist internally where needed

---

## History and selection decisions

### History is surface-owned
Reasoning:
- history restores exact committed selection snapshots
- that makes it one editing session's state, not shared document truth
- many surfaces over one document may legitimately have different local undo timelines

### Selection is surface-owned
Reasoning:
- many surfaces may show one document
- different views need independent selections/cursors/scroll state

### Cursor identities should be stable within a surface
Reasoning:
- multi-cursor programming becomes easier
- TypeScript/providers can refer to the active cursor or preserve visual goals cleanly

---

## Search, LSP, and runtime services

### Document-local search
Belongs in `the-lib`.

Reasoning:
- this is text-engine work
- it is document-scoped and deterministic

### Workspace/global search
Belongs in `the-runtime`.
FFF can be the primary or only workspace search backend, but it should live in the runtime, not the pure engine.

Reasoning:
- workspace search is resource/filesystem/index/service oriented
- it involves caches, watchers, frecency, query tracking, and project-wide behavior
- it is native and important, but not pure editor core

### LSP
Belongs in `the-runtime`, not `the-lib`.

Reasoning:
- LSP is protocol/session/service orchestration
- it should remain native for performance and ownership reasons
- it should not pollute the pure engine with server lifecycle and async protocol state

### Git/VCS
Belongs in `the-runtime`.

Reasoning:
- impure service integration
- workspace/resource scoped, not pure text core

---

## Porting rule for old-editor code

Default rule:

> Do **not** port something unless it clearly passes the engine or runtime test.

This avoids recreating the old architecture by inertia.

### Port into `the-lib` when
The old code is clearly one of:
- pure text/state primitive
- deterministic editing primitive
- syntax/annotation/render infrastructure
- document/surface support for the pure engine

### Port into `the-runtime` when
The old code is clearly one of:
- impure service/runtime/resource/workspace integration
- async orchestration
- filesystem/process/platform integration

### Rewrite in TypeScript when
The old code is clearly one of:
- command/policy/product behavior
- keymaps/modes/hooks
- app layout and chrome
- default UI workflows
- multi-step workflow composition over `the-lib` operations
- mode transitions, pending-key logic, register choice, or clipboard policy

---

## Old-editor module classification

The old editor is a reference, not a blueprint. Port selectively.

### Strong `the-lib` candidates
- `transaction.rs`
- `history.rs`
- `selection.rs`
- `movement.rs`
- `editing.rs` for edit primitives, fragment extraction, paste planning, and delete-span planning
- `operation.rs` for semantic default ops and thin single-step named dispatch
- `object.rs`
- `text_object.rs`
- `match_brackets.rs`
- `surround.rs`
- `comment.rs`
- `indent.rs`
- `auto_pairs.rs`
- `search.rs` (document-local scope)
- `diff.rs`
- `syntax.rs`
- render modules:
  - `render/text_format.rs`
  - `render/doc_formatter.rs`
  - `render/layout_index.rs`
  - `render/visible_layout.rs`
    - `render/layer_projection.rs`
- reworked `document.rs`
- reworked `surface.rs` (the old `view.rs` concept folded into the new surface model)
- narrowed `diagnostics.rs` if treated as generic semantic/render data
- narrowed `registers.rs` if reduced to pure register storage only

### Strong `the-runtime` candidates
These should stay in Rust, but not in `the-lib`:
- LSP ownership and orchestration
- FFF/workspace search integration
- clipboard provider integration
- persistence/open/save/reload/watch
- resource/document binding
- git/VCS integration
- grammar loader/runtime parser management
- background task orchestration

### Strong non-port candidates for `the-lib`
- `command_line.rs`
- `clipboard.rs`
- `app.rs`
- `split_tree.rs`
- `messages.rs`
- `docs_markdown.rs`
- old `editor.rs` as-is

### Strong TypeScript rewrite candidates
- command registry
- hooks system
- keymaps
- modes
- command palette behavior
- file picker behavior
- search UI behavior
- tabs/splits/panes/workspace layout
- default app behavior and product composition

---

## TS-ready native platform definition

The first goal is not a user-facing milestone. It is a boundary milestone:

> the minimum native platform that lets TypeScript become the real product layer.

### `the-lib` must provide
- `Document`
  - text
  - transactions
  - revisions
  - optional syntax state
- `Surface`
  - selection
  - stable cursor IDs
  - history
  - viewport/scroll
  - render cache and dirty state
- deterministic editing primitives
- viewport-first cell-grid frame projection

### `the-runtime` must provide
- resource model
- document/resource registry and binding
- surface lifecycle
- coarse domain APIs
- handles/events/subscriptions
- enough persistence lifecycle for TS to work with real documents and resources

### It is acceptable to defer initially
- LSP
- FFF
- git
- richer product features

Reasoning:
- TypeScript can start validating the architecture as soon as the native substrate is real
- runtime services should not block the core boundary if they are not necessary for initial TS integration

---

## Non-negotiable invariants

- `the-lib` must remain the pure editor engine.
- plain functions in `movement.rs` and `editing.rs` are the real core API; `operation.rs` is the semantic-default and thin-dispatch layer above them.
- `the-runtime` may be impure, but it must not define the editor's personality.
- `the-runtime` must not special-case editor command names or hide multi-step workflows.
- TypeScript may define the editor's personality, but must not own semantic document truth.
- Clients must not maintain a second semantic document model.
- Rendering must stay pure and viewport-first.
- Features like soft wrap and syntax must not disable the incremental path.
- Pane/tab/split/workspace layout belongs above Rust native document/surface truth.
- Documents are resource-agnostic.
- Resources are first-class runtime objects.
- A surface has one primary document.
- Many surfaces may share one document.

---

## Decision tree, resolved

This section records the architecture decisions explicitly.

### 1. Is the native core capability-oriented or app-oriented?
**Decision:** capability-oriented engine.

**Reasoning:**
A minimal engine should provide reusable primitives for prompt/input/editor/diff/minimap-style experiences without hard-coding one app personality.

### 2. Should Rust have a native session/registry layer?
**Decision:** yes, but minimal.

**Reasoning:**
Stable native identities for documents, surfaces, resources, and services are necessary. The registry should exist, but it should not become an app brain.

### 3. How many native editor/view layers should exist initially?
**Decision:** two layers: `Document` and `Surface`.

**Reasoning:**
A middle "instance" layer can be added later if necessary, but it is not required to establish the fundamental text/view separation.

### 4. Where does selection live?
**Decision:** on `Surface`, not `Document`.

**Reasoning:**
Many surfaces may share one document. Selection is per-view state.

### 5. Where does syntax live?
**Decision:** on `Document`, as optional pure derived state.

**Reasoning:**
Syntax is about the text itself and should be shared across surfaces. Grammar loading and async orchestration stay out of `the-lib`.

### 6. Does `Document` know about files/resources?
**Decision:** no.

**Reasoning:**
Documents are resource-agnostic semantic text objects.

### 7. Should deterministic editing primitives live in Rust?
**Decision:** yes.

**Reasoning:**
Movement, text objects, indent, surround, comment, match-brackets, auto-pairs, and similar operations are native editor semantics and often hot-path.

### 8. Should Rust own actual client painting?
**Decision:** no. Rust produces layout and visible frames; the client paints.

**Reasoning:**
Painting is platform-specific. Layout and cell-grid projection are editor-semantic.

### 9. Should `the-lib` include built-in UI products like command line or command palette?
**Decision:** no.

**Reasoning:**
Those are product-layer features, not engine substrate.

### 10. Should generic annotations/decorations/diagnostic data be in `the-lib`?
**Decision:** yes.

**Reasoning:**
They participate in layout, invalidation, and rendering, so they belong in the engine if modeled generically.

### 11. Should LSP live in `the-lib`?
**Decision:** no. Put it in `the-runtime`.

**Reasoning:**
LSP is an impure protocol/session service, not pure editor core.

### 12. What is the default porting rule?
**Decision:** do not port unless it clearly passes the engine/runtime test.

**Reasoning:**
This is the best defense against recreating the old architecture accidentally.

### 13. Should there be separate per-feature crates like `the-fff/`?
**Decision:** no, not by default.

**Reasoning:**
One `the-runtime` crate is a better early home for impure native services than many feature crates.

### 14. Is `the-native` the right crate concept/name?
**Decision:** conceptually yes, naming-wise no; use `the-runtime`.

**Reasoning:**
The important distinction is runtime role, not mere nativeness.

### 15. Where do pane/tab/split/workspace layout state and app chrome live?
**Decision:** in TypeScript/client composition, not Rust core/runtime semantic state.

**Reasoning:**
These are product-layer concerns and should stay replaceable.

### 16. Should Rust be authoritative for semantic editor state?
**Decision:** yes.

**Reasoning:**
This preserves correctness and performance while still allowing TypeScript to define behavior and composition.

### 17. Can TypeScript still be deeply extensible under that model?
**Decision:** yes.

**Reasoning:**
TypeScript should program the platform, not mirror the semantic text engine.

### 18. Should TypeScript create and compose surfaces dynamically?
**Decision:** yes.

**Reasoning:**
This is necessary for a replaceable product layer and alternate distributions.

### 19. Should TypeScript contribute structured providers into native surfaces/runtime domains?
**Decision:** yes.

**Reasoning:**
This enables deep extensibility without giving up native authority or performance.

### 20. Should TypeScript be able to replace the default product layer entirely?
**Decision:** yes.

**Reasoning:**
The default editor should be just one distribution. Alternate distributions should be possible.

### 21. Where does the command/hook/keymap system live?
**Decision:** in TypeScript.

**Reasoning:**
This is exactly the personality layer that should stay replaceable.

### 22. Should the runtime expose domain APIs or low-level function soup?
**Decision:** domain APIs.

**Reasoning:**
A coherent platform is easier to program against and avoids accidental chatty boundaries.

### 23. Should TypeScript interact through handles/events or snapshots/mirrors?
**Decision:** handles and events.

**Reasoning:**
This matches Rust authority and scales better for extensibility.

### 24. Should native objects be fixed product kinds or capability-composed?
**Decision:** capability-composed.

**Reasoning:**
This best supports extensibility and avoids product-shaped engine creep.

### 25. What is the first native target?
**Decision:** the TS-ready platform, not a polished product milestone.

**Reasoning:**
The goal is to reach a sound native substrate that lets TypeScript become the real product layer.

### 26. Does a surface attach to one or many documents?
**Decision:** one primary document.

**Reasoning:**
This keeps surface semantics simple; multi-document products can be composed above it.

### 27. Can many surfaces share one document?
**Decision:** yes.

**Reasoning:**
This is required for the document/surface split to pay off.

### 28. How are capabilities split?
**Decision:** by ownership: document-owned semantic capabilities, surface-owned view capabilities.

**Reasoning:**
This keeps text truth separate from per-view behavior and rendering.

### 29. Where does undo/redo history live?
**Decision:** on `Surface`.

**Reasoning:**
History restores exact committed selection snapshots and is therefore one live editing session's state, not shared document truth.

### 30. Should cursors have stable IDs within a surface?
**Decision:** yes.

**Reasoning:**
This supports multi-cursor programming and TypeScript/provider references.

### 31. How rich should revision identity be initially?
**Decision:** simple monotonic generations/counters.

**Reasoning:**
Enough for invalidation and ordering; avoid premature complexity.

### 32. Are resources first-class runtime objects?
**Decision:** yes.

**Reasoning:**
This cleanly separates text state from external-world identity.

### 33. Can documents and resources exist independently?
**Decision:** yes.

**Reasoning:**
This supports scratch docs, previews, generated docs, and unloaded workspace resources.

### 34. Should services expose long-lived session handles?
**Decision:** yes.

**Reasoning:**
LSP/search/watch domains are stateful and async; explicit handles fit the platform model.

### 35. Can TypeScript register structured providers into runtime domains?
**Decision:** yes.

**Reasoning:**
This is necessary for a truly extensible product layer.

---

## Summary

The architecture is:

```txt
the-stdx   = utilities
the-lib    = pure editor engine
the-runtime= impure native runtime/services
TS         = product/distribution/extensibility layer
client     = painting/platform host
```

And the central design rule is:

> Rust owns truth, invariants, and hot paths.
> TypeScript owns behavior, composition, and personality.
> Clients paint.

That is the line this rewrite should protect.
