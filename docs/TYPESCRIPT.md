# TypeScript Product Layer Architecture

This document defines the target architecture for the TypeScript product layer and the first Electron client.

It answers seven questions:

1. What belongs in TypeScript policy versus the Electron app?
2. How does TypeScript talk to the Rust runtime?
3. Where does extensibility live?
4. How should hot reload work?
5. How should custom UI work across worker and renderer seams?
6. How should input, commands, keymaps, tabs, panes, pickers, and statusline be modeled?
7. What should v1 build first without painting the architecture into a corner?

This document extends `docs/ARCHITECTURE.md`. The core rule remains:

> Rust owns truth, invariants, and hot paths.  
> TypeScript owns behavior, composition, and personality.  
> Clients paint.

---

## North Star

The editor is a native Rust editor platform with a programmable TypeScript product layer and an Electron client adapter.

```txt
Rust engine (`the-lib`)        text/state/editing invariants + cell-grid render deltas
Rust runtime (`the-runtime`)   resources, sessions, services, file IO, search, LSP, watches + JSON-RPC host binary
TypeScript editor substrate    hooks, dispatch, commands, keymaps, layout, modules
Default distribution           first-party modules built on the same public substrate
Policy worker                  runs the editor product layer and user modules
Electron renderer              paints app chrome + cell-grid surfaces
Electron main                  app lifecycle + runtime process supervision
```

The TypeScript layer is not a thin settings file around a fixed app. It is the editor's programmable personality.

The default editor should be built from the same public API that user code uses. User customization should not be a second-class plugin system bolted onto a privileged default app.

The model is closer to Emacs in spirit:

```txt
the editor = public programmable substrate + loaded modules
```

A first-party feature and a user customization differ by provenance, not by power category.

---

## Target package shape

Preferred shape:

```txt
the-editor/
  the-lib/                 # pure Rust editor engine
  the-runtime/             # impure Rust runtime/services + JSON-RPC host binary

  packages/
    editor/                # TS programmable editor substrate
    default/               # first-party default distribution modules
    contracts/             # shared TS contracts for IPC/model messages, if needed

  apps/
    electron/              # Electron main + renderer + policy worker wiring
```

`packages/editor` must not import Electron, React DOM, browser globals, or Node filesystem APIs directly. It should depend on injected capabilities.

`packages/default` should be a composed distribution of first-party modules, not a monolithic app.

`apps/electron` is an adapter. It hosts the policy worker, renderer, `the-runtime` host process, and concrete painting/UI implementation.

---

## Process model

V1 uses four cooperating execution contexts.

```txt
┌──────────────────────────────────────────────────────────────┐
│ Electron main                                                │
│ - owns windows/app lifecycle                                 │
│ - spawns and supervises Rust runtime host                    │
│ - owns JSON-RPC stdio transport                              │
│ - bridges runtime API to policy worker and renderer          │
└───────────────┬───────────────────────────┬──────────────────┘
                │                           │
                │ JSON-RPC stdio            │ Electron IPC / MessagePort
                │                           │
┌───────────────▼────────────────┐   ┌──────▼────────────────────┐
│ `the-runtime` host binary       │   │ Policy worker              │
│ - thin RPC adapter              │   │ - runs packages/editor      │
│ - owns RuntimeSession           │   │ - runs packages/default     │
│ - exposes domain RPC methods    │   │ - loads user modules        │
│ - emits runtime events          │   │ - owns logical UI model     │
└────────────────────────────────┘   └──────┬────────────────────┘
                                             │ logical app model + commands
                                             │
                                      ┌──────▼────────────────────┐
                                      │ Electron renderer          │
                                      │ - React app chrome adapter │
                                      │ - Canvas2D surface painter │
                                      │ - captures browser input   │
                                      │ - requests render frames   │
                                      └───────────────────────────┘
```

### Electron main

Electron main owns:

- application lifecycle
- window lifecycle
- spawning the Rust runtime host
- restarting or shutting down the runtime host
- runtime process logs and crash diagnostics
- JSON-RPC client transport over stdio
- typed bridge endpoints for policy worker and renderer

Electron main does not own editor product policy.

### `the-runtime` host binary

`the-runtime` exposes both a library and a thin host binary in the same crate.

Preferred flat crate shape:

```txt
the-runtime/
  Cargo.toml
  lib.rs       # runtime library API
  main.rs      # JSON-RPC host binary
  session.rs
  resource.rs
  io.rs
  syntax.rs
  clipboard.rs
  rpc.rs       # JSON-RPC protocol/server glue
```

Cargo shape:

```toml
[lib]
path = "lib.rs"

[[bin]]
name = "the-runtime"
path = "main.rs"
```

This avoids a separate `the-runtime-host/` crate while preserving the conceptual seam:

```txt
the-runtime library = runtime/session/services implementation
the-runtime binary  = JSON-RPC process adapter
```

The host binary owns:

- one or more `RuntimeSession`s
- JSON-RPC request/response handling
- runtime event emission
- serialization of handles, render deltas, and runtime errors

It should stay thin. Runtime behavior belongs in the library side of `the-runtime`; transport behavior belongs in `main.rs`/`rpc.rs`.

### Policy worker

The policy worker owns TypeScript editor personality and logical app composition.

It owns:

- loaded editor modules
- command registry
- keymap resolution
- hook/dispatch graph
- buffer tabs policy
- pane/split layout state
- active surface/leaf policy
- picker state
- statusline model
- panel registrations
- extension/module provenance
- hot reload lifecycle

The policy worker does not paint text rows. It does not own document text truth. It does not own selection truth. It does not perform cell-grid layout.

### Electron renderer

The renderer is the concrete view adapter.

It owns:

- React workbench chrome
- Canvas2D surface painter
- DOM layout and browser input capture
- mounting custom renderer-side view modules
- translating pointer/keyboard/composition events into normalized input messages
- requesting surface render frames for visible surfaces

The renderer does not own semantic editor truth. It should not maintain a second document model.

---

## Runtime IPC

V1 uses JSON-RPC over stdio between Electron main and the `the-runtime` host binary.

Request:

```json
{ "id": 1, "method": "documents.openFile", "params": { "path": "/tmp/a.txt" } }
```

Response:

```json
{ "id": 1, "result": { "documentId": 1 } }
```

Error:

```json
{
  "id": 1,
  "error": {
    "code": "document.notFound",
    "message": "document not found",
    "data": { "documentId": 1 }
  }
}
```

Push event:

```json
{
  "channel": "runtime.event",
  "sequence": 42,
  "data": { "type": "document.changed", "documentId": 1, "revision": 12 }
}
```

### Why JSON-RPC first

JSON-RPC is chosen for v1 because it is simple, inspectable, and easy to supervise from Electron main.

The known risk is serialization overhead, especially for render deltas. The mitigation is architectural rather than premature optimization:

- keep runtime APIs coarse-grained
- keep render deltas retained and incremental
- isolate the runtime transport behind a typed client seam
- benchmark render-frame serialization once the Electron loop exists
- replace the codec later only if measurements justify it

The product API should not expose JSON-RPC details. `packages/editor` should see a typed runtime client.

---

## Runtime API shape

The runtime client exposed to TypeScript should be domain-shaped, not a bag of low-level functions.

```ts
runtime.documents.*
runtime.resources.*
runtime.surfaces.*
runtime.workspace.*
runtime.search.*
runtime.lsp.*
runtime.watch.*
runtime.providers.*
```

V1 minimum:

```ts
runtime.documents.openFile(path)
runtime.documents.createScratch(options)
runtime.documents.close(documentId)

runtime.surfaces.create(documentId, options)
runtime.surfaces.configure(surfaceId, options)
runtime.surfaces.applyIntent(surfaceId, intent)
runtime.surfaces.renderFrame(surfaceId, request)

runtime.resources.save(documentId)
runtime.resources.saveAs(documentId, path)
```

Later runtime domains:

```ts
runtime.search.startWorkspaceSearch(...)
runtime.lsp.startSession(...)
runtime.watch.subscribe(...)
runtime.git.*
```

The runtime boundary should use stable handles:

```ts
type DocumentId = Brand<number, "DocumentId">;
type SurfaceId = Brand<number, "SurfaceId">;
type ResourceId = Brand<number, "ResourceId">;
type SearchSessionId = Brand<number, "SearchSessionId">;
type LspSessionId = Brand<number, "LspSessionId">;
```

---

## Render bridge

Most semantic runtime calls flow through the policy worker. Rendering is the intentional exception.

The renderer may use a narrow runtime render bridge:

```ts
runtime.surfaces.renderFrame({
  surfaceId,
  viewport,
  sinceGeneration,
});
```

This avoids placing the policy worker in the paint hot path.

Ownership remains clear:

```txt
Policy worker:
  decides which surfaces exist
  decides where surfaces are placed
  configures surface policy/options
  dispatches editing commands

Renderer:
  measures viewport size
  tracks concrete scroll/pixel state
  requests render frames for visible surfaces
  paints returned render deltas

Rust:
  owns layout, retained render state, selection truth, document truth
```

The renderer must not use the render bridge to make semantic editor decisions.

---

## Renderer technology

V1 uses React for app chrome and Canvas2D for the cell-grid surface painter.

```txt
React:
  workbench shell
  tabs
  panels
  pickers
  command palette
  statusline
  custom view hosts

Canvas2D:
  text rows
  cursors
  selections
  syntax spans
  widgets participating in cell-grid rendering
```

React must not be in the text-paint hot path.

Bad:

```txt
RenderFrameDelta -> React state -> row components -> span components
```

Good:

```txt
RenderFrameDelta -> imperative SurfacePainter.applyDelta() -> canvas paint
```

The surface painter should be an adapter:

```ts
interface SurfacePainter {
  resize(size: PixelSize, metrics: CellGridMetrics): void;
  applyDelta(delta: RenderFrameDelta): void;
  paint(damage?: Damage): void;
  dispose(): void;
}
```

V1 implementation:

```txt
Canvas2dSurfacePainter
```

Possible future implementations:

```txt
WebGpuSurfacePainter
WebGlSurfacePainter
TerminalSurfacePainter
```

React is an adapter choice, not the product architecture. `packages/editor` must not depend on React.

---

## Policy worker owns logical UI composition

The policy worker owns the logical app model.

Examples:

```ts
type WorkbenchModel = {
  buffers: BufferEntry[];
  tabs: TabEntry[];
  activeTabId: TabId | null;
  layout: LayoutNode;
  panels: PanelEntry[];
  pickers: PickerModel[];
  statusline: StatuslineModel;
};
```

The renderer receives this model and paints it.

The policy worker decides:

- which buffers exist in the product model
- which tabs exist
- which surface is active
- which panels are visible
- whether the statusline is visible
- which picker is open
- how splits and panes are arranged

The renderer decides:

- DOM placement
- CSS/layout implementation
- Canvas size and DPI handling
- concrete painting
- mounting renderer-side view modules

---

## Splits and panes

Splits and panes are pure TypeScript policy/layout state.

Rust must not know about splits.

Rust owns:

```txt
Document
Surface
RenderFrameDelta
Selection/history/viewport per Surface
```

TypeScript owns:

```txt
tabs
splits
panes
active leaf
which SurfaceId is shown in which leaf
surface lifecycle policy
```

Suggested model:

```ts
type LayoutNode =
  | { kind: "leaf"; leafId: LeafId; surfaceId: SurfaceId }
  | { kind: "split"; axis: "horizontal" | "vertical"; children: LayoutNode[]; weights: number[] }
  | { kind: "panel"; panelId: PanelId };
```

V1 may render only one active editor area plus optional panels/statusline, but the model should be multi-surface and split-capable from day one.

Splits should work soon after v1 without changing the Rust model.

---

## Input flow

Keyboard, pointer, and composition input flow through explicit ownership seams.

```txt
Browser event
  -> renderer normalizes input
  -> policy worker resolves keymaps/commands
  -> command dispatch invokes runtime intent
  -> Rust mutates Document/Surface
  -> renderer requests render frame
  -> Canvas painter applies delta
```

Renderer owns browser specifics:

- `KeyboardEvent`
- IME/composition events
- pointer events
- focus
- clipboard/browser integration where necessary

Policy worker owns:

- keymap resolution
- modes
- command dispatch
- product behavior

Rust owns:

- deterministic native editing intents
- movement
- selection updates
- history
- render invalidation

Example:

```ts
// renderer -> policy worker
{
  kind: "key",
  key: "j",
  code: "KeyJ",
  modifiers: [],
  text: "j"
}
```

```ts
// policy worker
const command = await editor.keymaps.resolve(input, context);
await editor.commands.dispatch(command);
```

```ts
// command implementation
await editor.runtime.surfaces.applyIntent(surfaceId, {
  kind: "move",
  direction: "down",
  count: 1,
});
```

Committed text input should be sent to Rust as a coarse intent:

```ts
await editor.runtime.surfaces.applyIntent(surfaceId, {
  kind: "insertText",
  text,
});
```

---

## Programmable substrate

`packages/editor` is the public programmable editor substrate.

It should expose:

```ts
createEditorHost(...)
editor.use(module)
editor.dispatch(action)
editor.hooks.*
editor.commands.*
editor.keymaps.*
editor.buffers.*
editor.tabs.*
editor.layout.*
editor.panels.*
editor.pickers.*
editor.statusline.*
editor.diagnostics.*
```

The first-class concept is an editor module, not an extension.

```ts
type EditorModule = (editor: Editor) => void | Disposable | Promise<void | Disposable>;
```

A first-party module and a user module use the same API:

```ts
export function activate(editor: Editor) {
  editor.commands.register(...);
  editor.statusline.registerSlot(...);
  editor.keymaps.bind(...);
}
```

"Extension" means a dynamically loaded module with provenance:

```txt
source path
module id
reload generation
registered contributions
diagnostics
```

It is not a separate second-class product API.

Core rule:

> If the default editor can do it, user code can do it through the same public substrate.

---

## Hook and dispatch system

The editor should be hook/dispatch driven.

This follows the same philosophy as the old `the-dispatch` experiment: define named behavior points, let modules contribute behavior, and keep orchestration explicit.

Unlike the old Rust dispatch system, the TypeScript version should be dynamic and reloadable.

It must support:

- dynamic module loading
- hot reload
- contribution provenance
- disposal
- ordering/priorities
- diagnostics
- async handlers
- user modules loaded from disk
- worker/renderer split

### Hook kinds

The hook system should encode combination semantics explicitly.

```ts
hooks.event<T>(name)       // broadcast; no result
hooks.waterfall<T>(name)   // transform value through handlers
hooks.first<T, R>(name)    // first handled result wins
hooks.collect<T, R>(name)  // collect all results
hooks.reduce<T>(name)      // reduce to model/state
hooks.around<T>(name)      // advice/wrapping around operation
```

Examples:

```ts
await editor.hooks.event("workspace.opened", workspace);

const input = await editor.hooks.waterfall("input.preprocess", rawInput);

const resolved = await editor.hooks.first("keymap.resolve", {
  input,
  context,
});

const slots = await editor.hooks.collect("statusline.slots", context);

const model = await editor.hooks.reduce("workbench.resolve", baseModel);

await editor.hooks.around("commands.dispatch", command, async () => {
  return dispatchCommand(command);
});
```

### Domain facades

Most code should use domain-specific facades rather than raw hooks.

```ts
editor.commands.register(...)
editor.keymaps.bind(...)
editor.statusline.registerSlot(...)
editor.layout.setRoot(...)
editor.panels.register(...)
editor.pickers.open(...)
```

Internally, these facades create tracked contributions and/or install hook handlers.

This gives both:

- ergonomic APIs for common tasks
- low-level programmability for deep customization

---

## Contributions and provenance

Every registered thing should be a tracked contribution.

Examples:

- command
- keybinding
- statusline slot
- panel
- picker provider
- hook handler
- theme
- renderer view module
- runtime provider contribution

Contribution identity should include provenance:

```ts
type ContributionId = {
  moduleId: ModuleId;
  kind: string;
  localId: string;
};
```

The registry should know:

```txt
who registered this
when it was registered
which reload generation owns it
how to dispose it
whether it conflicts with another contribution
```

This enables:

- hot reload
- diagnostics
- conflict reporting
- inspection UI
- deterministic tests
- clanker-friendly modification

---

## Hot reload

Hot reload should be dispose-and-reactivate per module.

Do not patch individual closures in place.

Module lifecycle:

```ts
export function activate(editor: Editor) {
  editor.commands.register(...);
  editor.panels.register(...);
  editor.hooks.waterfall(...);
}
```

On reload:

```txt
1. find module by source path/module id
2. dispose all contributions owned by that module
3. clear module cache
4. import fresh module
5. run activate(editor)
6. reconcile logical app model
7. notify renderer
```

Optional cleanup:

```ts
export async function deactivate(editor: Editor) {
  // close external resources if needed
}
```

The default distribution should hot reload the same way as user modules.

---

## App model mutation

Raw app model mutation should be private to `packages/editor` implementation modules.

First-party and user code should change behavior through public modules, registries, hooks, and actions.

Bad:

```ts
editor.__appModel.statusline.visible = false;
editor.__appModel.tabs.push(tab);
```

Good:

```ts
editor.statusline.disable();
editor.tabs.open(documentId);
editor.layout.splitActiveLeaf({ axis: "vertical" });
editor.commands.register(...);
editor.hooks.around(...);
```

This is not about restricting users. It is about making changes tracked, reloadable, inspectable, and testable.

---

## Default distribution

`packages/default` is a first-party distribution built from modules.

Preferred shape:

```txt
packages/default/src/index.ts
packages/default/src/buffers.ts
packages/default/src/tabs.ts
packages/default/src/workbench.ts
packages/default/src/statusline.ts
packages/default/src/keymaps.ts
packages/default/src/pickers.ts
packages/default/src/panels.ts
packages/default/src/commands/file.ts
packages/default/src/commands/edit.ts
packages/default/src/commands/view.ts
```

Activation should compose modules:

```ts
export function activate(editor: Editor) {
  editor.use(defaultBuffers());
  editor.use(defaultTabs());
  editor.use(defaultWorkbench());
  editor.use(defaultStatusline());
  editor.use(defaultPickers());
  editor.use(defaultPanels());
  editor.use(defaultKeymaps());
  editor.use(defaultFileCommands());
  editor.use(defaultEditCommands());
  editor.use(defaultViewCommands());
}
```

This makes first-party behavior clanker-navigable. A request like "disable the statusline" should point to an obvious module, not a hidden app singleton.

---

## Custom UI modules

Custom UI must support the split between policy worker and renderer.

Policy modules run in the policy worker. Renderer view modules run in the Electron renderer.

A module may register a custom panel by reference:

```ts
editor.panels.register({
  id: "doom",
  title: "Doom",
  view: {
    runtime: "react",
    module: "./doom.view.tsx",
    export: "DoomPanel",
  },
});
```

The renderer dynamically imports the view module and mounts it in a scoped host.

Policy-side code can:

- register the panel
- decide when it is visible
- expose commands for it
- update logical state

Renderer-side view code can:

- render custom UI
- use renderer-provided view capabilities
- send scoped messages back to policy

Renderer-side view code should not call Rust runtime directly. Runtime access should go through scoped capabilities and policy-owned actions unless explicitly exposed by the host.

---

## Testing strategy

The TS layer should be testable without Electron.

`packages/editor` tests should cover:

- hook ordering
- hook combination semantics
- module activation/disposal
- hot reload contribution cleanup
- command registration/dispatch
- keymap resolution
- layout model updates
- statusline resolution
- panel registration
- diagnostics/conflicts

`packages/default` tests should cover:

- default module composition
- basic command availability
- default keymap behavior
- statusline model
- picker model
- tab/buffer lifecycle policy

`apps/electron` tests should cover:

- runtime process supervision
- JSON-RPC bridge
- renderer/policy worker messaging
- surface painter delta application
- smoke boot of the workbench

The `the-runtime` host binary/RPC layer should have protocol-level tests independent of Electron.

---

## V1 scope

V1 should prove the architecture, not complete the editor.

### V1 should include

- `the-runtime` JSON-RPC host binary
- Electron main spawning the `the-runtime` host process
- policy worker
- renderer React shell
- Canvas2D surface painter
- `packages/editor` substrate skeleton
- `packages/default` composed module skeleton
- open file
- create surface
- render active surface
- keyboard input through policy worker
- native edit/movement intents through Rust
- basic buffer tabs model
- basic statusline model
- basic command registry
- basic keymap registry
- module activation/disposal
- user module loading from a configured path
- hot reload of a module

### V1 may defer

- full LSP UI
- FFF workspace search UI
- hot file reloading
- multi-window runtime sharing
- WebGPU painter
- full split-pane UX
- extension marketplace/packaging
- security sandboxing beyond local trusted code assumptions

Splits should be planned in the model from day one and implemented soon after v1.

---

## Future runtime services

Once the Electron app exists, the next runtime paths become valuable because they can be tested against a real product layer.

Strong `the-runtime` candidates:

- FFF/global workspace text search
- LSP sessions and server lifecycle
- file watching and hot reload
- workspace/resource model enrichment
- git/VCS integration

These should stay in Rust runtime, exposed to TypeScript as coarse-grained domain APIs and events.

TypeScript should compose their UI/policy:

```txt
workspace search picker
LSP diagnostics panel
code action picker
reload conflict prompt
git statusline slot
```

Rust should own the impure/session/service work.

---

## Non-negotiable invariants

- `packages/editor` must not depend on Electron or React.
- `packages/default` must use the same public API as user modules.
- The policy worker owns logical UI composition.
- The renderer paints and hosts views; it does not own semantic editor truth.
- Electron main owns runtime process lifecycle.
- The Rust runtime host exposes domain APIs, not product behavior.
- Keyboard/keymap resolution belongs in policy worker.
- Rust executes native editing/rendering intents.
- Splits and panes belong in TypeScript, not Rust.
- Runtime calls must be coarse-grained.
- React must not be in the text-paint hot path.
- Canvas2D painter must sit behind a replaceable `SurfacePainter` adapter.
- Hot reload is dispose-and-reactivate by module.
- Contributions must be tracked with provenance.
- User modules are not second-class extensions; they are editor modules loaded dynamically.

---

## Summary

The TypeScript architecture is:

```txt
packages/editor  = programmable editor substrate
packages/default = first-party module distribution
policy worker    = product behavior + logical UI composition
Electron renderer= React chrome + Canvas surface painter
Electron main    = runtime process supervision + IPC bridge
Rust runtime     = authoritative native state, services, and thin host binary
```

The defining principle is:

> Build the default editor out of the same hook/dispatch/module substrate that users customize.

That is how the editor remains deeply extensible, reloadable, and clanker-editable without sacrificing the Rust authority model for text truth and hot paths.
