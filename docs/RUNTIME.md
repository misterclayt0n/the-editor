# Runtime Architecture

This document defines the target architecture for `the-runtime`.

It answers six questions:

1. What is `the-runtime` for?
2. How does it relate to `the-lib`, TypeScript, and the client?
3. What runtime-owned objects should exist?
4. What should v1 include, and what should it defer?
5. What should the crate layout look like initially?
6. How should old-editor runtime-ish code be reinterpreted under the new architecture?

This document is intentionally opinionated. `the-runtime` should be a small native ownership/session/services crate above `the-lib`, not a second editor engine and not a new app brain.

---

## Purpose

`the-runtime` exists to own the native things that are:
- impure
- external-world-facing
- lifecycle-oriented
- service/session-shaped
- awkward or dishonest to force into either `the-lib` or TypeScript

It is responsible for things like:
- runtime-owned registries and handles
- resource/document/surface attachment
- file open/save lifecycle
- syntax asset loading
- clipboard implementation
- later: LSP, file watching, workspace search, git/VCS, background work

It is **not** responsible for:
- pure editor semantics
- document text truth
- surface selection/history/render semantics
- command language
- keymaps
- modal behavior
- app/workspace chrome
- client painting

---

## Core position

The architecture should read like this:

```txt
Resource      = external-world identity
Document      = shared text truth
Surface       = one live per-view/editor session on that text
Runtime       = native ownership/session/services layer above those objects
TypeScript    = behavior/product composition layer
Client        = painting/platform host
```

`the-runtime` is where the architecture becomes operational.

`the-lib` defines the pure nouns.
`the-runtime` owns their native lifecycle, attachment, and impure integrations.

---

## Why `the-runtime` exists

Without `the-runtime`, the project gets forced into one of two bad shapes:

### 1. Push impure native concerns down into `the-lib`
That would pollute the pure engine with:
- file paths
- save policy
- grammar loading
- clipboard commands
- subprocesses
- watches
- LSP state

This would destroy the engine boundary.

### 2. Push all native orchestration upward into TypeScript/client code
That would force JS/client layers to own or emulate:
- native resource lifecycle
- document/surface registries
- syntax asset loading
- clipboard/platform integrations
- later async service orchestration

That would be artificial, slow, and architecturally muddy.

So `the-runtime` is the honest middle layer.

---

## Relationship to `the-lib`

`the-lib` owns:
- `Document`
- `Surface`
- `CellGridRenderSurface`
- editing semantics
- selection/history/cell-grid render invariants
- syntax core state model
- pure algorithms and data structures

`the-runtime` owns:
- runtime IDs/handles
- registries around `Document` and `Surface`
- `Resource`
- open/save lifecycle
- syntax resource loading and activation helpers
- clipboard implementation
- session-wide ownership above surfaces

The dependency direction remains:

```txt
the-stdx <- the-lib <- the-runtime
```

`the-runtime` may own `the-lib` objects.
`the-lib` must not know about `the-runtime`.

---

## Relationship to TypeScript

TypeScript should still own:
- commands
- hooks
- keymaps
- modes
- app/pane/workspace composition
- product behavior
- extension-driven policy

`the-runtime` is not a replacement for the TypeScript product layer.
It is the native substrate that TypeScript composes against.

That means:
- `the-runtime` should expose honest native capabilities
- it should not define the editor's personality
- it should not become a giant built-in command/UI system

---

## Runtime-owned object model

The runtime should operationalize the three key architecture nouns:

1. `Resource`
2. runtime-owned document entries
3. runtime-owned surface entries

plus one central owner object.

### `Resource`
A `Resource` is an external-world identity.

In v1, it should stay minimal and primarily mean:
- a canonical file path

Later it may also include:
- URIs
- other external sources

But v1 does not need a grand polymorphic resource system.

#### Why `Resource` is distinct from `Document`
A `Document` is text truth.
A `Resource` is where text comes from / saves to in the outside world.

This distinction matters because:
- a `Resource` may exist with no loaded `Document`
- a `Document` may exist with no `Resource`

Examples:
- unopened file in a workspace tree → `Resource`, no `Document`
- scratch buffer → `Document`, no `Resource`
- normal file editing → both, attached

### Runtime document entry
A runtime document entry should wrap a pure `the-lib::Document` and add only narrow runtime metadata.

It should own things like:
- `Document`
- optional attached `ResourceId`
- narrow persistence/lifecycle metadata

Examples of justified metadata:
- save baseline / dirty tracking
- readonly flag
- maybe display name derived from resource

It should **not** absorb:
- selection
- history
- viewport
- render state
- per-view interaction state

Those stay on `Surface`.

### Runtime surface entry
A runtime surface entry should wrap a pure `the-lib::Surface` and attach it to a runtime document handle.

It should own things like:
- `Surface`
- attached `DocumentHandle`

This is where the architectural rule becomes operational:
- one surface is attached to exactly one primary document

### `RuntimeSession`
`RuntimeSession` should be the central v1 runtime object.

It should own:
- resource registry
- document registry
- surface registry
- syntax runtime context
- clipboard provider/service
- session-wide `Registers`

This should be the main runtime API surface.

---

## Runtime handles and identity

Unlike `the-lib`, the runtime should introduce explicit handles.

Examples:
- `ResourceId`
- `DocumentHandle`
- `SurfaceHandle`

This is the right place for IDs because:
- the runtime owns registries
- the runtime owns lifecycle
- the runtime owns attachment relationships

`the-lib` stays ID-light and pure.
`the-runtime` owns operational identity.

---

## V1 responsibilities

`the-runtime` v1 should include:

### 1. Runtime ownership/session layer
This is the backbone.

It should provide:
- registries for resources/documents/surfaces
- creation and destruction operations
- attachment relationships
- a central `RuntimeSession`

### 2. File open/save lifecycle
This is the first real payoff of the `Resource` split.

V1 should support:
- open file into a runtime document entry
- create scratch document with no resource
- save an attached document back to its resource
- save-as to a new resource path

### 3. Syntax runtime loading and activation
The pure syntax side already exists in `the-lib`.

`the-runtime` should add:
- a concrete `SyntaxResources` implementation
- grammar/query asset loading
- a runtime-owned `LanguageRegistry`
- automatic language detection / syntax activation when opening files

Scratch docs may remain unassigned unless the caller opts into syntax explicitly.

### 4. Clipboard implementation
`the-lib` already defines:
- `ClipboardProvider`
- `ClipboardType`
- `NoClipboard`

`the-runtime` should provide the real platform/process-backed provider(s).

### 5. Session-wide registers ownership
We already decided registers belong above `Surface`.

So v1 runtime should own:
- one `Registers` object per `RuntimeSession`

This gives a natural place for session-wide yank/put state and later clipboard integration.

---

## Explicitly deferred from v1

### LSP
LSP absolutely belongs in `the-runtime`, but not in v1.

Reason:
- it is a large domain
- it wants a stable runtime backbone first
- it would distort the crate shape if introduced too early

LSP should come after:
- resource model
- runtime session
- document/surface registries
- open/save lifecycle
- syntax runtime loading

### File watching
File watching belongs in `the-runtime` architecturally, but should be deferred to phase 2.

Reason:
- not needed to validate the crate shape
- reload/conflict policy gets messy quickly
- better introduced once the base runtime model is solid

### Async/event bus architecture
V1 runtime should be allowed to stay mostly synchronous.

Reason:
- open/save/syntax loading/clipboard do not require a full async runtime story yet
- a synchronous first version is smaller and easier to reason about
- later async services can layer on once there is a real need

### Workspace/global-search/git/VCS
These should come later.

They are runtime-shaped, but not needed for v1.

---

## Post-v1: complete syntax runtime direction

V1 proved the ownership split, but it is not the final performance architecture for syntax.

The complete model should make `the-runtime` the owner of syntax **scheduling**, not syntax **truth**.

### What remains on `Document`
The document continues to own:
- persistent syntax layer graph truth
- invalidated regions
- parse/injection/enrichment generations
- snapshot/apply protocols for advancement

### What moves firmly into runtime policy
The runtime should own a real per-document syntax scheduler responsible for:
- coalescing rapid edit bursts
- tiny bounded foreground advancement on the typing path
- background completion for the rest
- stale-result rejection via source generations/snapshots
- prioritizing `stale ∩ demanded` work first
- degraded large-file/pathological-file policy

### Surface demand as runtime input
The scheduler should consume demand reported by attached surfaces, such as:
- visible viewport byte ranges
- viewport margins
- cursor-near priority
- active vs background surface priority

Runtime should not guess visibility or importance in the abstract if surfaces can report it directly.

### Runtime should stop pushing whole-document syntax payloads
The long-term runtime shape is **not**:
- collect full-document highlights
- clone them into every surface

The long-term shape is:
- advance document-owned syntax truth
- notify/fan out that syntax generations advanced
- let surfaces pull visible extraction as needed

### Tiered syntax work
Runtime should treat syntax-derived lanes differently.

Examples:
- baseline structural syntax freshness
- injection maintenance
- locals-sensitive enrichment
- semantic token overlay updates

These lanes share one document syntax truth, but they do not all deserve the same freshness budget.

### Why this belongs in runtime
This policy is impure and session-shaped because it depends on:
- task execution
- budgeting
- cancellation/supersession
- surface demand
- large-file policy

That is exactly why it belongs here rather than in `the-lib` or TypeScript.

---

## Ideal v1 public API shape

The exact interface can vary, but it should feel roughly like this:

```rust
pub struct RuntimeSession { ... }

impl RuntimeSession {
  pub fn new(...) -> Self;

  pub fn open_file(&mut self, path: &Path) -> Result<DocumentHandle>;
  pub fn create_scratch(&mut self, text: Rope) -> Result<DocumentHandle>;

  pub fn create_surface(&mut self, document: DocumentHandle) -> Result<SurfaceHandle>;

  pub fn save(&mut self, document: DocumentHandle) -> Result<()>;
  pub fn save_as(&mut self, document: DocumentHandle, path: PathBuf) -> Result<ResourceId>;

  pub fn document(&self, handle: DocumentHandle) -> Option<&Document>;
  pub fn document_mut(&mut self, handle: DocumentHandle) -> Option<&mut Document>;

  pub fn surface(&self, handle: SurfaceHandle) -> Option<&Surface>;
  pub fn surface_mut(&mut self, handle: SurfaceHandle) -> Option<&mut Surface>;

  pub fn registers(&self) -> &Registers;
  pub fn registers_mut(&mut self) -> &mut Registers;
}
```

This is illustrative, not frozen.

---

## Initial crate layout

Keep the initial crate layout flat unless a domain clearly justifies multiple files.

Preferred v1 shape:

```txt
the-runtime/
  Cargo.toml
  lib.rs

  session.rs
  resource.rs
  io.rs
  syntax.rs
  clipboard.rs
```

### Why this flatter shape
- v1 should stay small
- do not create folders when each domain only has one real file
- add subdirectories later only when a domain genuinely grows into multiple coherent files

### What about `documents.rs` and `surfaces.rs`?
Those would have existed only to hold runtime entry/registry helpers for runtime-owned documents and surfaces.

That is, they would have contained things like:
- runtime document entry structs
- runtime surface entry structs
- maybe small handle/lookup helpers

But for v1 they do **not** need to exist as separate files.
Those concepts can live inside `session.rs` initially.

So in the flatter layout:
- `session.rs` owns runtime registries and runtime entry structs
- `resource.rs` owns resource types
- `io.rs` owns open/save helpers
- `syntax.rs` owns runtime syntax loading/activation
- `clipboard.rs` owns runtime clipboard provider implementation

If later the session module grows too large, it can be split then.

---

## Proposed responsibility split by file

### `session.rs`
Owns:
- `RuntimeSession`
- runtime handles (`DocumentHandle`, `SurfaceHandle`, maybe `ResourceId` if not placed in `resource.rs`)
- runtime document entry struct
- runtime surface entry struct
- registries/maps
- document/surface/resource attachment logic
- high-level lifecycle operations

### `resource.rs`
Owns:
- `ResourceId` if you prefer it here
- `Resource`
- minimal file-backed resource model
- path normalization/canonicalization helpers if needed

### `io.rs`
Owns:
- file reads/writes
- open/save/save-as helpers
- runtime-facing persistence helpers
- maybe document dirty-baseline updates tied to save lifecycle

### `syntax.rs`
Owns:
- runtime-backed `SyntaxResources` implementation
- language registry construction helpers
- grammar/query asset loading
- syntax detection/activation helpers used by runtime session

### `clipboard.rs`
Owns:
- concrete clipboard provider implementation(s)
- platform/process-backed clipboard operations
- detection/selection of available clipboard backend

---

## How old-editor code should be reinterpreted

### Old `syntax/runtime_loader.rs`
This was directionally correct in responsibility, but in the wrong crate.

In the new architecture:
- it belongs in `the-runtime`
- it should integrate with the new pure syntax registry/resources design
- it should not leak back into `the-lib`

### Old `clipboard.rs`
The abstraction may exist near the core, but real implementation belongs in `the-runtime`.

### Old `editor.rs` / giant FFI runtime state
This is the main cautionary tale.

The old code mixed:
- runtime session ownership
- file watching
- LSP lifecycle
- clipboard wiring
- open/save
- UI-facing orchestration
- product behavior

The new `the-runtime` should extract the honest runtime/service/session core from that blob, but must not become another giant everything object.

---

## Non-goals for v1

V1 `the-runtime` should **not** become:
- a new monolithic editor object
- a pane/tab/workspace UI layer
- a command system
- a keymap/mode layer
- an async event framework before actual async services exist
- a giant service zoo on day one

Keep it focused on one job:

> make the pure architecture real in native runtime ownership and services

---

## Recommended implementation order

1. create crate skeleton
2. define `Resource`, runtime handles, and `RuntimeSession`
3. add document/surface registries and attachment model
4. add open/create/save/save-as lifecycle
5. add runtime syntax loading + activation
6. add clipboard provider implementation
7. later: file watching
8. later: LSP
9. later: workspace/global-search/git services

---

## Summary

`the-runtime` v1 should be:
- small
- explicit
- synchronous-first
- lifecycle-oriented
- service-oriented
- clearly above `the-lib` and below TypeScript

It should make these architecture facts operational:
- `Resource` is external-world identity
- `Document` is shared text truth
- `Surface` is one live per-view/editor session
- runtime owns the native registries, attachments, and impure integrations around them
