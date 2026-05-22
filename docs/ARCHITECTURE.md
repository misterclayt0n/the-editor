# The Editor Architecture

This document is the architectural anchor for The Editor. When implementation details become confusing, this is the document we return to.

The Editor is a TypeScript-first editor. Its architecture is designed around local interaction, native browser scrolling, virtualized rendering, and package-based extensibility.

The goal is a simple, fast editor architecture:

- TypeScript owns editor state, input, rendering, scrolling, layout, extension composition, and UI.
- Effect is the default programming model for lifecycle, services, background work, package loading, and typed failures.
- The hot path for typing, cursor movement, selection, scrolling, and paint is local and synchronous.
- Workers and native code are optional background accelerators, never mandatory hops in the input-to-paint path.
- Packages and extensions remain first-class.
- Core editor modules are small in number but deep in behavior.

This document describes the shape of the system, not the exact implementation.

---

## 1. Core philosophy

The editor should feel fast because the important path is tiny.

A keystroke should look approximately like this:

```txt
keydown
  -> resolve command/keybinding
  -> apply transaction to editor state
  -> update selection/history
  -> schedule paint
  -> render visible rows
```

A keystroke should not require background work, worker round-trips, filesystem queries, syntax analysis, VCS refreshes, or extension-host responses before the user sees feedback.

Workers and native modules may be used later for isolated heavy computations. They must not own the interactive editor loop.

The architecture is optimized for:

1. **Locality** — the code responsible for a behavior should live near the state it changes.
2. **Low-latency input** — editor interaction should not wait behind background work.
3. **Simple ownership** — one module should own each kind of truth.
4. **Virtualized rendering** — only visible or near-visible UI should be rendered.
5. **Extensibility** — packages can contribute commands, keymaps, themes, decorations, gutters, language features, and UI.
6. **Deletion-friendly design** — if a concept does not hide meaningful complexity, remove it.

---

## 2. Non-goals

The architecture should stay focused. These are not goals for the foundation:

- making every subsystem generic before there are real use cases
- putting editor interaction behind asynchronous service calls
- requiring a worker or native module for ordinary typing, movement, selection, scrolling, or paint
- rendering entire large documents, trees, or diffs when only a viewport is visible
- making background metadata mandatory for initial paint
- letting package APIs expose arbitrary editor internals
- optimizing isolated algorithms before the end-to-end interaction path is understood
- introducing adapter layers before there are multiple real implementations

A concept should exist because it gives leverage. If it mainly passes state through, repeats another module's interface, or forces callers to understand its internals, it should be deleted or folded into a deeper module.

---

## 3. Ownership model

The most important architectural rule is ownership.

### TypeScript owns

- document model
- transactions
- selection
- history
- movement
- editor panes
- viewport
- scrolling
- layout
- rendering
- commands
- keymaps
- extension composition
- themes
- decorations
- gutters
- workbench UI
- file tree UI
- diff UI

### Background workers may own

- syntax tokenization work queues
- semantic analysis requests
- VCS/diff computation
- filesystem indexing
- search indexing
- extension-host isolation, if needed

Workers produce updates. They do not block typing.

### Native modules may eventually own

Only narrow, profiled compute kernels, such as:

- very large diff algorithms
- regex/search acceleration
- parser/tokenizer acceleration
- filesystem crawling, if JS proves insufficient

Native code is not a place to hide application architecture. It must sit behind a small TypeScript interface and be optional or replaceable where practical.

---

## 4. Technology choices

The default stack is:

```txt
Runtime/app:      Electron + Node
Renderer:         Chromium + React
Package manager: pnpm
Monorepo tasks:   Turbo
TypeScript:       strict TypeScript, tsgo when it behaves, tsc as fallback
Effect:           default model for services/lifecycle/background work
Build:            tsdown for packages, Vite for the web app
Tests:            Vitest + @effect/vitest
E2E:              Playwright
Benchmarks:       mitata + custom scenario CLIs
Lint/format:      oxlint + oxfmt
Native later:     N-API / napi-rs behind narrow package interfaces
```

Node/Electron is the runtime compatibility contract. Bun may be useful for experiments or scripts, but extension/runtime behavior should assume Node compatibility.

Effect should be used by default for:

- package activation, disposal, and reload
- service layers
- resource scopes
- typed errors
- filesystem/workspace services
- worker supervision
- background jobs
- persistence and stores
- extension lifecycle

Direct synchronous TypeScript is still allowed where profiling or simplicity demands it, especially inside tight editor kernels such as rope operations, selection movement, viewport math, and paint preparation.

---

## 5. Process model

The app is split into a generic web renderer and a desktop host:

```txt
apps/web
  - generic renderer/product app
  - React + Vite
  - workbench UI
  - editor panes
  - file tree
  - command dispatch
  - extension/package composition
  - local editor state
  - can run in a browser-like environment

apps/desktop
  - Electron main and preload
  - application lifecycle
  - native windows
  - menus
  - filesystem/dialog privileges
  - shell integration
  - process supervision
  - typed desktop bridge

Web Workers / Worker Threads
  - syntax work
  - search
  - VCS/diff work
  - filesystem indexing
  - other cancellable background jobs
```

The web app should not import Electron directly. Desktop-only capabilities flow through a typed bridge defined in `packages/contracts` and implemented by `apps/desktop/preload`.

The renderer is allowed to hold real editor state. This is intentional.

The desktop app should not be the editor brain. It is the host for privileged OS capabilities and app lifecycle.

---

## 6. Repository layout

The intended shape is:

```txt
apps/
  web/
    generic renderer/product app
    React + Vite
    no direct Electron imports

  desktop/
    Electron main process
    Electron preload bridge
    native menus/windows/dialogs/shell integration

packages/
  rope/
    standalone rope implementation
    publishable package
    tests and benchmarks
    optional native adapter later

  contracts/
    shared public types
    desktop bridge contracts
    protocol and manifest types

  kernel/
    package loading
    contribution ownership
    reload lifecycle
    stores
    command bus
    Effect services/layers

  extensions/
    ExtensionContext
    registries
    package manifests
    contribution types
    role/tool contracts

  core/
    document
    transaction
    selection
    movement
    history
    syntax-facing document metadata

  editor/
    editor pane
    editor view
    layout
    rendering
    decorations
    gutters
    editor commands

  workbench/
    layout tree
    dock roots
    focus model
    input router
    role/tool mounting coordination

  default/
    default package set
    default commands
    default keymaps/profile
    default theme
    default editor/file tree/palette/statusline implementations

  benchmarks/
    shared benchmark utilities
    scenario runners
    performance fixtures

docs/
  ARCHITECTURE.md
  EXTENSIONS.md
```

This layout separates stable editor concepts from application UI and default product behavior.

The default product is not hardwired into the kernel. It lives in `packages/default` as a package set.

`packages/rope` is first-class because text storage is core to the editor and may be useful as a standalone NPM package.

---

## 7. The core modules

The editor should have a small number of deep modules. A deep module exposes a small interface while hiding substantial implementation detail.

### 7.1 Document

The document module owns text truth.

It should answer questions like:

- How many lines are there?
- What is the text of line N?
- What text exists in this range?
- What offset corresponds to this position?
- What position corresponds to this offset?
- What happens if this transaction is applied?

Example conceptual interface:

```ts
interface TextDocument {
  readonly uri: string;
  readonly version: number;

  lineCount(): number;
  line(lineIndex: number): string;
  lines(startLine: number, endLineExclusive: number): readonly string[];

  offsetAt(position: Position): number;
  positionAt(offset: number): Position;

  apply(transaction: Transaction): DocumentChange;
}
```

The implementation may use a piece table, rope, gap buffer, or simpler line array initially. That choice is internal. Callers should not care.

The document module should not know about DOM, canvas, Electron, tabs, panes, scroll positions, or extensions.

### 7.2 Transactions

Transactions are the only way text changes.

A transaction describes edits and metadata:

- inserted/deleted text
- affected ranges
- selection before/after
- grouping information for undo/redo
- cause/source

Transactions should be precise and testable. They are the seam between commands and document mutation.

Conceptually:

```ts
interface Transaction {
  readonly edits: readonly TextEdit[];
  readonly selectionBefore?: SelectionSet;
  readonly selectionAfter?: SelectionSet;
  readonly annotations?: TransactionAnnotations;
}
```

The transaction module should make invalid states hard to express:

- ranges are normalized
- overlapping edits are rejected or canonicalized
- positions are validated against a document version
- application order is deterministic

### 7.3 Selection

The selection module owns cursor and selection semantics.

It should support:

- one or more selections
- anchors and heads
- range selections
- line selections
- cell/block-ish cursor rendering semantics if needed
- normalization
- mapping through document changes

Selection is not just an array of ranges. Editor behavior depends on selection invariants, anchor/head direction, and movement affinity.

### 7.4 Movement

The movement module owns text navigation.

It should answer:

- move left/right by grapheme
- move by word
- move to line start/end
- move vertically preserving goal column
- page up/down
- document start/end
- select while moving

Movement should be independent of rendering where possible, but it may need layout information for visual movement once wrapping, proportional fonts, or soft tabs exist.

The key design rule:

- model movement belongs in `packages/core/movement`
- visual movement that depends on layout belongs near `EditorPane` / layout

### 7.5 History

History owns undo/redo.

It groups transactions according to editing intent, not merely time.

Examples:

- typed characters may coalesce
- cursor movement may break a group
- paste is a group
- delete-word is a group
- auto-indent may be grouped with newline

History should not be a global app service. It belongs to an editable buffer/editor model.

### 7.6 Syntax

Syntax is background-enriched document metadata.

It may provide:

- lexical tokenization
- incremental parsing
- syntax highlighting spans
- bracket information
- fold ranges
- indentation hints

Syntax must not block input.

The editor must be able to paint plain text immediately and accept better syntax information later.

```txt
text change
  -> document updates synchronously
  -> editor paints immediately, possibly with stale tokens
  -> syntax worker receives new version
  -> syntax worker posts token update for version N
  -> editor applies token update only if still relevant
```

Syntax updates are versioned and cancellable/latest-wins.

### 7.7 Workspace

Workspace owns project-level state:

- open folders
- file metadata
- file watching
- resource status
- VCS status
- search roots

Workspace state is not editor state. It can update asynchronously without blocking panes.

---

## 8. Editor modules

### 8.1 EditorPane

`EditorPane` is the central interactive editor module.

It owns:

- current document reference
- selection set
- viewport state
- scroll state
- layout cache
- decorations relevant to this pane
- command dispatch for this pane
- history for this editable context

It coordinates core modules but does not replace them.

Conceptually:

```ts
interface EditorPane {
  readonly document: TextDocument;
  readonly selection: SelectionSet;

  dispatch(command: EditorCommand): void;
  applyTransaction(transaction: Transaction): void;

  setScrollTop(scrollTop: number): void;
  setViewportSize(size: Size): void;

  getVisibleRange(): LineRange;
  render(): void;
}
```

The pane is allowed to synchronously read the document for visible lines. This is the core simplification.

No asynchronous view-model request is required for ordinary rendering.

### 8.2 EditorView

`EditorView` owns the actual browser rendering strategy.

It owns:

- DOM structure
- canvas or DOM text layers
- native scroll container
- resize observation
- pointer/keyboard event attachment
- focus
- IME composition handling
- clipboard integration
- accessibility hooks

It should not own document truth. It renders an `EditorPane`.

The view computes viewport ranges using local state:

```ts
const firstLine = Math.floor(scrollTop / lineHeight);
const lastLine = Math.ceil((scrollTop + viewportHeight) / lineHeight);
const overscanned = expand(firstLine, lastLine, overscanLines);
```

Then it asks the pane/document for exactly that range.

### 8.3 Layout

Layout converts model positions into visual positions.

Initially, layout can be simple:

- monospace font
- fixed line height
- no soft wrap
- tab width known
- horizontal scroll local to the view

Over time, layout may support:

- soft wrap
- variable width fonts
- inline decorations
- code lenses
- folded ranges
- bidi text

The layout module must remain local and cacheable. It must not become a remote render service.

### 8.4 Rendering

Rendering paints visible editor content.

Possible implementations:

- DOM rows
- canvas text rendering
- hybrid DOM/canvas
- WebGL later, if justified

The rendering interface should be small:

```ts
interface TextRenderer {
  render(frame: TextRenderFrame): void;
}
```

`TextRenderFrame` is a local view artifact. It is not document truth.

It may contain:

- visible line texts
- token spans
- selections
- cursors
- decorations
- gutter data
- scroll offsets
- layout metrics

It is produced locally by `EditorPane`/`EditorView`.

---

## 9. Editor composition

The editor is composed from a few explicit owners:

- `TextDocument` owns text.
- `EditorPane` owns interactive editor state.
- `EditorView` owns DOM/canvas/native scrolling.
- `TextRenderer` paints a local frame.
- registries collect package and extension contributions.

Avoid creating a single catch-all editor abstraction that owns document state, selection, viewport, rendering, syntax, gutters, command dispatch, and background coordination all at once. That kind of module has a large interface and poor locality.

When a new module is proposed, ask:

- What truth does it own?
- Which callers become simpler because it exists?
- What complexity is hidden behind its interface?
- Could the same behavior live more locally in an existing module?

---

## 10. Rendering and scrolling

Scrolling is native browser scrolling.

The editor should use a normal scroll container with browser-managed scroll position. The renderer observes scroll and schedules paint.

The render loop should be frame-based:

```txt
scroll/input/model change
  -> mark editor dirty
  -> requestAnimationFrame
  -> compute visible range
  -> read visible document lines
  -> read cached metadata for visible range
  -> paint
```

Multiple invalidations in the same frame should coalesce.

The editor should never synchronously perform heavy background work during paint. Paint should use what is already available.

If syntax/decorations/gutters are stale, paint stale information and update later.

---

## 11. Virtualization

The editor, file tree, diff viewer, panels, and any large list must virtualize.

Virtualization rule:

> Rendering cost should be proportional to visible content, not total project or document size.

For a text editor this means:

- visible line range is derived from scroll position
- overscan is bounded
- only visible/overscanned lines are measured/rendered
- line metadata is cached and invalidated by document version/range

For a file tree this means:

- the tree model may contain hundreds of thousands of paths
- the DOM contains only a viewport slice
- expansion/collapse updates visible counts incrementally
- expensive projections are cached/lazy

For a diff viewer this means:

- large diffs render by window
- highlighting is cached
- workers can precompute expensive AST/token data
- DOM updates are coalesced

---

## 12. Scheduling

Scheduling is part of architecture, not an afterthought.

There are different classes of work:

### Hot work

Must finish immediately or within the current frame:

- key input
- cursor movement
- selection changes
- local transaction application
- scroll position update
- visible paint

Hot work must not await background work.

### Warm work

Should happen soon, but can be coalesced:

- syntax updates for visible range
- decoration recomputation for visible range
- gutter refresh for visible range
- diagnostics display update
- command palette filtering for small sets

### Cold/background work

Must be cancellable, chunked, or workerized:

- full-project search indexing
- VCS status refresh
- full file tree crawl
- large diff computation
- semantic analysis
- dependency/package discovery

Cold work must never monopolize the thread used for hot work.

Recommended scheduler primitives:

- `requestAnimationFrame` for painting
- `queueMicrotask` only for tiny follow-ups
- `setTimeout(0)` or `scheduler.postTask` for yielding between chunks
- Web Workers for CPU-heavy or latency-insensitive work
- `AbortSignal` for cancellation
- latest-wins version checks for stale results

---

## 13. Extensions and packages

The package/extension system is a core strength and should remain.

Extensions should contribute through explicit registries:

- commands
- keybindings
- themes
- languages
- tokenizers
- decorations
- gutters
- status bar items
- panels
- file tree contributions
- workspace providers

A package should not need privileged access to arbitrary internals to be useful.

Conceptual example:

```ts
interface ExtensionContext {
  commands: CommandRegistry;
  keymaps: KeymapRegistry;
  themes: ThemeRegistry;
  languages: LanguageRegistry;
  decorations: DecorationRegistry;
  gutters: GutterRegistry;
  workspace: WorkspaceRegistry;
}
```

Extension contribution points should be stable and narrow.

The editor should distinguish between:

- core packages required for editor operation
- default packages that provide product behavior
- optional extensions installed by users

### Extension execution

Initially, extensions may run in-process for simplicity.

If isolation is needed later, an extension host worker/process can be introduced. That must not require changing the editor core. The registries are the seam.

---

## 14. Commands and keymaps

Commands are named operations.

Keymaps bind input gestures to commands.

The command system should be context-aware but simple.

```ts
interface CommandRegistry {
  registerCommand(id: string, handler: CommandHandler): Disposable;
  executeCommand(id: string, args?: unknown): void | Promise<void>;
}
```

Editor commands that affect text should generally produce transactions.

Examples:

- `editor.insertText`
- `editor.deleteBackward`
- `editor.moveLeft`
- `editor.moveDown`
- `editor.selectLine`
- `editor.paste`
- `editor.undo`
- `editor.redo`

Workbench commands may affect UI:

- `workbench.openFile`
- `workbench.closeTab`
- `workbench.toggleSidebar`
- `workbench.openCommandPalette`

Keymap resolution should be synchronous for normal editor input.

---

## 15. Decorations and gutters

Decorations are visual annotations over editor content.

Examples:

- diagnostics squiggles
- search matches
- selections
- current line highlight
- bracket matches
- inline hints

Gutters are visual annotations beside lines.

Examples:

- line numbers
- breakpoints
- VCS hunks
- diagnostics markers

Decorations and gutters should be pane-local views over document/workspace state. They should not be part of the document truth.

Providers may be async, but rendering should use cached provider output.

```txt
provider computes update
  -> publishes decorations for document version/range
  -> editor invalidates affected visible rows
  -> next frame paints updated decorations
```

Line numbers are special: they are built-in, cheap, and should be rendered directly by the editor view rather than supplied by a cold provider.

---

## 16. Syntax highlighting

Syntax highlighting should be progressive.

Minimum behavior:

- plain text appears immediately
- syntax appears when available
- stale syntax is acceptable temporarily
- syntax results are versioned
- syntax work is cancellable/latest-wins

The editor should prefer viewport-prioritized syntax:

1. visible range
2. nearby overscan
3. rest of open document
4. background documents

Full-document tokenization must not block input.

---

## 17. File tree

The file tree should be TypeScript/data-structure driven.

It should support:

- large project trees
- lazy loading
- incremental updates
- native-feeling scrolling
- virtualization
- stable selection/focus
- search/filtering
- VCS decorations

The file tree should not rely on native code for speed by default. A well-designed TS model using arrays, numeric IDs, typed arrays, cached projections, and virtualization is sufficient for very large trees.

The file tree model should be separate from the file tree view.

---

## 18. Diff viewer

The diff viewer should be TS-first and worker-friendly.

It should support:

- virtualized rendering
- syntax-highlighted hunks
- cached tokenization/render output
- workerized expensive computation
- scroll synchronization
- inline and side-by-side modes eventually

Large diff rendering must be incremental and cancellable.

The visible UI should never wait for an entire diff to be highlighted.

---

## 19. Filesystem and persistence

Filesystem access belongs behind workspace/resource modules.

The editor should distinguish:

- an in-memory document
- a resource on disk
- a saved version
- dirty state
- external modifications

Saving is a workspace/resource operation triggered by editor state.

Documents should not directly call Electron APIs.

---

## 20. Testing strategy

The architecture should be testable through module interfaces.

Important tests:

### Core tests

- document edits
- line/offset conversion
- transaction application
- selection normalization
- movement behavior
- undo/redo grouping
- syntax version handling

These should run without Electron or DOM.

### Editor tests

- command dispatch
- keymap resolution
- transaction generation
- viewport range computation
- render-frame generation
- decoration/gutter invalidation

These may use DOM-like test utilities but should avoid full app startup.

### Workbench tests

- tab behavior
- file open/close
- command palette behavior
- extension contributions

### Performance tests

Performance is a feature. We should have targeted tests/benchmarks for:

- typing latency in large files
- cursor movement latency
- scrolling large files
- file tree with huge path sets
- large diff open time
- syntax worker responsiveness

---

## 21. Performance invariants

The following invariants should guide implementation reviews:

1. Typing must not await syntax, VCS, filesystem, search, or extension background work.
2. Cursor movement must not await background work.
3. Paint cost must be proportional to visible rows.
4. File tree DOM size must be proportional to visible rows.
5. Large async jobs must be cancellable or chunked.
6. Background results must be versioned before applying.
7. Cross-process calls must not be in the editor hot path unless there is no alternative.
8. Native code must not become an architectural owner.
9. The renderer may hold editor state.
10. Stale decorations are better than blocked input.

---

## 22. Implementation phases

A practical rebuild should happen in vertical slices.

### Phase 1: Minimal local editor

- pnpm workspace
- Turbo task graph
- strict TypeScript config
- Effect installed and used for kernel/service scaffolding
- `apps/web` Vite renderer
- `apps/desktop` Electron host/preload
- `packages/rope` initial text storage package
- document module
- transaction module
- selection module
- movement basics
- editor pane
- DOM or canvas editor view
- native scrolling
- basic commands/keymaps
- open an in-memory file
- type, delete, move, select

No syntax. No VCS. No extension host isolation. No native modules.

### Phase 2: Real files and workbench

- open/save files
- dirty state
- tabs
- file tree
- command palette
- default theme
- default keymap

### Phase 3: Syntax and decorations

- syntax worker
- token cache
- viewport-prioritized highlighting
- diagnostics/decorations registry
- gutters registry

### Phase 4: Extensions/packages

- extension contribution registries
- default package set
- package loading
- command/keymap/theme contributions

### Phase 5: Performance hardening

- large file behavior
- large tree behavior
- large diff behavior
- latency logging
- targeted benchmarks
- worker cancellation/coalescing

### Phase 6: Optional native acceleration

Only after profiling:

- identify narrow slow function
- define tiny TS interface
- implement native adapter
- keep JS fallback if practical
- benchmark end-to-end latency, not just raw compute

---

## 23. Architectural warning signs

If these appear, stop and revisit this document:

- Typing requires awaiting a Promise.
- Typing, movement, selection, or scrolling require cross-process calls.
- A background worker owns selection or viewport truth.
- A generic coordination module starts accumulating unrelated responsibilities.
- A render API needs to include every editor subsystem to be useful.
- Syntax or VCS work can delay key input.
- A provider recomputes whole-project state on each edit.
- Tests require launching the whole app for basic document behavior.
- Native code becomes necessary to understand editor behavior.
- A module exists mainly to preserve a design that the current architecture does not need.

---

## 24. Final principle

The editor is a local TypeScript application with deep modules, native browser scrolling, virtualized rendering, and optional background workers.

Keep the hot path small. Keep ownership obvious. Delete concepts that stop earning their complexity.
