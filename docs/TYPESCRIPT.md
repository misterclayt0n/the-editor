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
Rust engine (`the-lib`)        text/state/editing invariants + cell-grid frame projection
Rust runtime (`the-runtime`)   resources, sessions, services, file IO, search, LSP, watches
Node native runtime package     napi-rs wrapper around `the-runtime`
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
  the-runtime/             # impure Rust runtime/services

  packages/
    runtime-node/          # napi-rs Node native wrapper around the-runtime
    editor/                # TS programmable editor substrate
    default/               # first-party default distribution modules
    contracts/             # shared TS contracts for IPC/model messages, if needed

  apps/
    electron/              # Electron main + renderer + policy worker wiring
```

`packages/editor` must not import Electron, React DOM, browser globals, or Node filesystem APIs directly. It should depend on injected capabilities.

`packages/default` should be a composed distribution of first-party modules, not a monolithic app.

`apps/electron` is an adapter. It hosts the policy worker, renderer, native runtime package, and concrete painting/UI implementation.

---

## Process model

V1 uses three cooperating execution contexts.

```txt
┌──────────────────────────────────────────────────────────────┐
│ Electron main                                                │
│ - owns windows/app lifecycle                                 │
│ - owns windows/app lifecycle                                 │
│ - starts and supervises policy worker                        │
│ - bridges renderer requests to policy worker                 │
└───────────────────────────────────────────┬──────────────────┘
                                            │ Electron IPC / MessagePort
                                            │
                                     ┌──────▼────────────────────┐
                                     │ Policy worker              │
                                     │ - runs packages/editor      │
                                     │ - runs packages/default     │
                                     │ - imports runtime-node      │
                                     │ - owns RuntimeSession       │
                                     │ - loads user modules        │
                                     │ - owns logical UI model     │
                                     └──────┬────────────────────┘
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
- starting the policy worker
- restarting or shutting down the policy worker
- policy worker logs and crash diagnostics
- typed bridge endpoints for renderer ↔ policy worker requests

Electron main does not own editor product policy.

### Native runtime package

V1 uses `packages/runtime-node`, a napi-rs native Node package around `the-runtime`.

Preferred shape:

```txt
packages/runtime-node/
  package.json
  Cargo.toml
  build.rs
  index.d.ts
  index.js                 # generated by napi-rs
  src/lib.rs               # thin napi wrapper around RuntimeSession
```

This preserves the conceptual seam:

```txt
the-runtime           = runtime/session/services implementation
packages/runtime-node = Node native adapter over the runtime
```

The native package owns no editor personality. It exposes coarse runtime methods such as:

```ts
createScratch(text)
openFile(path)
createSurface(documentId, viewport)
setSurfaceViewport(surfaceId, viewport)
renderFrame(surfaceId, viewport)
saveSurfaceDocument(surfaceId)
```

It should stay thin. Runtime behavior belongs in `the-runtime`; Node/Electron binding behavior belongs in `packages/runtime-node`.

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

## Native runtime bridge

V1 uses napi-rs, but the native runtime package is owned by **Electron main**, not the policy worker and not the renderer.

```txt
Electron main
  owns runtime authority and IPC boundaries
  exposes a broad policy runtime bridge
  exposes a narrow renderer render bridge
  serializes native runtime operations

Runtime worker thread
  owns the single NativeRuntimeSession
  imports runtime-node directly
  executes Rust/NAPI calls off Electron main

Policy worker
  owns EditorHost/product logic
  uses an IPC-backed RuntimeApi client
  does not import runtime-node directly

Electron renderer
  paints UI and canvas surfaces
  uses only the narrow render bridge for visible text frames
  does not import runtime-node directly
```

This keeps one authoritative native runtime instance while separating behavior traffic from paint traffic and keeping heavy native work off Electron main.

The policy worker creates the editor host with an IPC-backed runtime client:

```ts
const runtime = createRuntimeIpcClient();
const editor = createEditorHost({ runtime });
```

The renderer uses a separate narrow bridge:

```ts
const frame = await renderBridge.renderFrame({ surfaceId, viewport, metrics });
```

Electron main routes runtime requests through an explicit serialized operation queue to the runtime worker thread. Public TypeScript methods remain async; callers do not observe whether a request came from policy or the renderer render bridge.

The important rule is unchanged:

- keep runtime APIs coarse-grained
- keep hot text/layout work in Rust
- do not expose chatty per-row/per-token APIs
- do not serialize hot-path render frames as JSON long-term
- do not copy full document text during normal typing/render refresh
- keep renderer access narrower than policy access

`packages/editor` sees a typed runtime client, not napi-rs details. Only `apps/electron/main` should know about `@the-editor/runtime-node`.

---

## Runtime API shape

The runtime client exposed to TypeScript should be domain-shaped, not a bag of low-level functions.

```ts
runtime.documents.*
runtime.resources.*
runtime.surfaces.*
runtime.registers.*
runtime.clipboard.*
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
runtime.surfaces.applyOperation({ surfaceId, operation })
runtime.surfaces.renderFrame(surfaceId, request)

runtime.registers.read(name)
runtime.registers.write(name, values)

runtime.clipboard.read(kind)
runtime.clipboard.write(kind, value)

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

The renderer uses a separate, explicit render bridge owned by Electron main. This bridge is not the full runtime API.

Allowed renderer bridge:

```ts
renderBridge.renderFrame({
  surfaceId,
  viewport,
  metrics,
});
```

Disallowed from the renderer bridge:

```txt
applyOperation
applyEdit
read full document text
openFile
save
search
LSP
commands
workbench mutation
```

`renderFrame({ surfaceId, viewport, metrics })` is stateful on the native `Surface`: Rust persists the viewport and metrics on the surface, reconciles surface/render state, and returns a `RenderFrame` for that request.

This avoids placing the policy worker in the paint hot path while keeping Rust authoritative for cell-grid layout truth.

Ownership remains clear:

```txt
Policy worker:
  decides which surfaces exist
  decides where surfaces are placed
  configures surface policy/options
  dispatches editing commands
  emits explicit surface invalidation events after surface mutations

Renderer:
  measures viewport size and cell metrics
  tracks concrete scroll/pixel state
  requests render frames for visible surfaces after invalidation/scroll/resize
  caches frames as presentation data only
  paints returned render frames

Rust:
  owns document truth
  owns surface truth, including persisted viewport/metrics
  owns cell-grid layout/projection truth
  returns RenderFrame paint data
```

The renderer must not use the render bridge to make semantic editor decisions.

### Surface invalidation model

Visible text frames are not part of the general workbench snapshot.

After policy code mutates a surface, it emits an explicit invalidation event:

```ts
{
  type: "surface.invalidated",
  surfaceIds: [surfaceId],
  reason: "edit" | "selection" | "viewport" | "syntax" | "configuration",
}
```

The renderer receives the event, checks which invalidated surfaces are visible, and requests fresh frames only for those surfaces.

There is no compatibility fallback where every policy snapshot causes visible surfaces to rerender. Workbench snapshots communicate product/layout state; surface invalidation events communicate render freshness.

### Policy render use

The policy worker should not request frames for normal visible editor painting.

Policy may request frames for utility and non-paint cases only, such as:

- picker previews
- file previews
- headless tests
- offscreen preview surfaces

Those utility render calls must not become the normal typing paint path.

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
```

React must not be in the text-paint hot path.

Bad:

```txt
RenderFrame -> React state -> row components -> span components
```

Good:

```txt
RenderFrame -> imperative SurfacePainter.paintFrame() -> canvas paint
```

The surface painter should be an adapter:

```ts
interface SurfacePainter {
  resize(size: PixelSize, metrics: CellGridMetrics): void;
  paintFrame(frame: RenderFrame): void;
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

The logical model must identify text surfaces by handle, but it must not embed normal visible `RenderFrame` payloads. A surface pane snapshot should carry identity and product metadata such as `surfaceId`, `documentId`, title, path, dirty/revision metadata, and tool/pane state. The renderer obtains the actual frame through the explicit render bridge after surface invalidation, scroll, or resize.

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
RenderFrame
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

```txt
editor split tree
  -> group leaves
    -> pane tabs
      -> workbench items (text surfaces or tool panes)
```

The split tree must be owned by a deep workbench layout model, not scattered across workbench command handlers.

Preferred module shape:

```ts
layout.apply(intent): LayoutChange
layout.snapshot(): WorkbenchEditorLayoutSnapshot
layout.assertInvariants(): void
```

The layout model owns:

- item, pane, group, and split identity allocation
- editor split tree mutation
- pane placement intents
- tab ordering
- active group / active pane / active item repair
- empty-group pruning
- split-tree collapse
- split size normalization
- duplicate surface-pane deduplication rules
- layout invariant checks

`WorkbenchController` should be the host-facing adapter over the layout model. It may translate product operations into layout intents and emit workbench invalidations, but it should not manually repair tree invariants in each command path.

Extensions and default commands use placement intents, not tree surgery:

```ts
type WorkbenchPanePlacement =
  | { kind: "tab"; group?: GroupId | "active"; index?: number }
  | { kind: "split"; direction: "left" | "right" | "up" | "down"; of?: GroupId | PaneId | "active" };
```

V1 may render only one active editor area plus optional panels/statusline, but the model should be multi-surface and split-capable from day one.

Splits should work soon after v1 without changing the Rust model.

---

### Workbench layout mutation model

Layout mutation should be transactional and intent-based.

Examples:

```ts
layout.apply({ kind: "openPane", pane, placement });
layout.apply({ kind: "movePane", paneId, placement });
layout.apply({ kind: "closePane", paneId });
layout.apply({ kind: "resizeSplit", splitId, sizes });
layout.apply({ kind: "focusPane", paneId });
```

Every `apply(...)` call must leave the layout valid or throw before mutating externally-visible state. After every successful mutation, these invariants should hold:

- if `root` is non-null, every group referenced by the tree exists
- every pane belongs to exactly one group
- every group referenced by the tree has a valid active pane or no panes
- active group, active pane, and active item agree
- split children and split sizes have the same length
- split sizes are finite, positive, and normalized
- no empty group remains in the split tree unless it is the single initial group
- item records without panes are removed unless explicitly retained by a future pinned-item policy
- duplicate same-item surface panes in one group are resolved by the layout model, not by callers

Snapshot projection is a separate responsibility:

```ts
layout.snapshot({ mapItem, mapPane })
```

Snapshots are read models for renderer/policy communication. They must not expose mutable internal maps or require the renderer to understand repair rules. Renderer drag/drop and split resizing should produce intents back to the host; renderer code should not perform semantic layout mutation locally.

Focus remains outside the layout model. The layout model decides active group/pane/item. After a successful focus-affecting layout change, `WorkbenchController` sends the corresponding focus intent to `InputRouter`.

## Input flow

Keyboard, pointer, and composition input flow through explicit ownership seams.

```txt
Browser event
  -> renderer normalizes browser-specific input
  -> policy worker InputRouter routes input from host focus/scope state
  -> active interaction profile reduces routed input to structured effects
  -> host executes effects through commands/runtime/workbench APIs
  -> Rust mutates Document/Surface or session support state
  -> renderer requests render frame
  -> Canvas painter applies delta
```

Renderer owns browser specifics:

- `KeyboardEvent`
- IME/composition events
- pointer events
- DOM focus capture and editable-element exclusion only
- clipboard/browser integration where necessary

Policy worker owns:

- `InputRouter`, the sole authority for host focus and key routing
- the host-owned interaction session for the editor window
- active interaction profile selection
- pending input state, including keymap prefixes and profile operators
- final scope-stack computation from focus, overlays, tools, and profile state
- command dispatch
- product behavior

Modes are not a generic workbench field. A mode is profile-owned state exposed through the interaction snapshot for UI/status/scope purposes.

Rust owns:

- deterministic native single-step semantic operations
- movement
- selection updates
- history
- render invalidation
- session-local support state such as registers

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
const result = await editor.input.routeKey(input);
```

```ts
// single-step semantic command
await editor.runtime.surfaces.applyOperation({
  surfaceId,
  operation: { name: "move_visual_line_down", count: 1 },
});
```

Committed text input should be sent to Rust as a coarse semantic operation:

```ts
await editor.runtime.surfaces.applyOperation({
  surfaceId,
  operation: { name: "insert_text", text },
});
```

Multi-step workflows remain in TypeScript, but they should be owned by the active interaction profile and lowered through structured interaction effects instead of ad hoc command chains:

```ts
return {
  state: { ...state, mode: "insert" },
  effects: [
    { kind: "surface.operation", surfaceId, operation: { name: "open_below" } },
    { kind: "interaction.mode", mode: "insert" },
  ],
};
```

Clipboard/register flavored variants are also TypeScript workflows:

```ts
const values = await editor.runtime.registers.read('"');
await pasteAfter(surfaceId, values);
```

The important rule is that TypeScript resolves the source/target policy while Rust owns the actual semantic paste planning.

### InputRouter model

`packages/editor` owns one `InputRouter` per editor host/window, exposed as `editor.input`.

`InputRouter` replaces any separate focus authority. There should not be a compatibility `editor.focus` facade. Focus is routing state, not a standalone registry.

Preferred interface:

```ts
editor.input.routeKey(input): Promise<InputRouteResult>
editor.input.focus(target, { reason, source? }): FocusTarget
editor.input.restore({ reason, fallback? }): FocusTarget
editor.input.scopes(): string[]
editor.input.snapshot(): InputSnapshot
```

`InputRouter` owns:

- current host focus target
- focus history/restoration
- active scope stack
- overlay precedence in routing
- pending-input cancellation on focus or active-surface changes
- escape/back/cancel routing policy
- validation/repair of stale role/tool targets

Workbench remains the owner of layout activation. For example, `workbench.focusPane(paneId)` updates active group/pane/item, then requests an input focus intent from `InputRouter`. The workbench must not mutate input focus state directly.

Roles, tools, overlays, and workbench modules request focus intents through `InputRouter`; they do not own focus state and do not compute final scopes.

### Interaction session model

There is one host-owned interaction session per editor window.

It owns all pending input state:

- keymap prefixes such as `g`, `space`, and `C-w`
- sticky/keymap node state
- pending profile operators such as `f<char>`, `r<char>`, surround operations, text objects, and register choice
- repeatable motion/operator memory when it is window-scoped

The keymap registry is a contribution registry and resolver. It must not own mutable pending input state. Pending input belongs to the interaction session.

The interaction session clears pending input on focus or active-surface changes unless a future profile explicitly introduces a narrower preserved state. V1 should not preserve pending input across focus/surface changes.

Surface-specific interaction memory is allowed only when the profile has a concrete surface-local concept, such as per-surface last motion or jump-list metadata. Document text, selection truth, visual goals, and history remain Rust-owned through `Document`/`Surface`.

### Interaction profile seam

An interaction profile owns its reducer/state/snapshot. The host owns routing and effect execution.

Preferred shape:

```ts
interface InteractionProfile<TState = unknown> {
  id: string;
  initialState(ctx: InteractionContext): TState;
  scopes(ctx: InteractionContext, state: TState): string[];
  reduceInput(ctx: InteractionContext, state: TState, input: KeyInput): Promise<InteractionTransition<TState>>;
  snapshot(ctx: InteractionContext, state: TState): InteractionProfileSnapshot;
}
```

The profile reducer may be imperative internally, but it must return structured state transitions and effects. It should not directly mutate workbench, focus, runtime, or command state.

Core editor behavior should use typed interaction effects, not stringly command dispatch:

```ts
type InteractionEffect =
  | { kind: "surface.operation"; surfaceId: SurfaceId; operation: SurfaceOperation }
  | { kind: "surface.insertText"; surfaceId: SurfaceId; text: string }
  | { kind: "interaction.mode"; mode: string }
  | { kind: "focus.set"; target: FocusTarget }
  | { kind: "command.dispatch"; dispatch: CommandDispatch };
```

Use typed effects for core editor semantics. Use command dispatch for product/extension actions such as palette, file picker, splits, save, reload, and extension commands.

The profile snapshot is the only public source for mode-like UI state:

```ts
interface InteractionProfileSnapshot {
  mode?: {
    id: string;
    title?: string;
    cursorShape?: "block" | "bar" | "underline";
    scopes: string[];
  };
  pending?: {
    kind: string;
    label?: string;
    keys?: string[];
    nextKeys?: string[];
  };
}
```

Renderer code must consume cursor shape/mode/pending information from the interaction snapshot, not from default-distribution workbench state.

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

## State stores

Raw string-keyed state slots are not part of the architecture. `EditorHost` must not expose `stateSlot`, `getStateSlot`, `setStateSlot`, or similar global mutable bags.

All TypeScript state should live in typed store tokens accessed through `editor.stores`.

Preferred store definition shape:

```ts
const workbenchLayoutStore = defineHostStore<WorkbenchLayoutState>({
  id: "workbench.layout",
  init: () => ({ ... }),
});

const hoverStore = defineModuleStore<HoverState>({
  id: "default.hover",
  init: () => ({ visible: false }),
});

const fileTreeStore = definePersistedStore<FileTreePersistedState>({
  id: "default.fileTree",
  version: 1,
  init: () => ({ expanded: [] }),
  serialize: (state) => state,
  deserialize: (raw) => raw,
});
```

Access is by token only:

```ts
const layout = editor.stores.host(workbenchLayoutStore);
const hover = editor.stores.module(hoverStore);
const fileTree = editor.stores.persisted(fileTreeStore);
```

No raw string access is allowed. Store IDs are declarative metadata, not lookup parameters passed by callers.

Store kinds:

1. **Host stores** — canonical editor/product shell state owned by `packages/editor`, such as input routing, workbench layout, role selections, tools, packages, and interaction state.
2. **Module stores** — volatile module-owned state. The owner is the current module/provenance. It is disposed when that module unloads.
3. **Persisted stores** — namespaced module-owned state with explicit versioning, serialization, deserialization, and migration. Persistence mechanics are host-owned; schema semantics are module-owned.
4. **Session stores** — state tied to an interactive session ID and disposed with that session.

Store tokens should carry enough metadata for diagnostics and reload behavior:

```ts
interface StoreToken<T> {
  id: string;
  kind: "host" | "module" | "persisted" | "session";
  init(): T;
  label?: string;
}
```

Persisted stores additionally need `version`, `serialize`, `deserialize`, and later `migrate`.

Lifecycle rules:

- host stores survive module reloads and are disposed only with the editor host
- module stores are scoped by module ID and reload generation
- persisted stores survive reloads and process restarts if backed by persistence
- session stores are scoped by session ID
- unloading a module disposes its volatile module stores and contributions together

The deletion test is strict: if a store does not need lifecycle/provenance/schema metadata, it probably belongs inside a deeper host module implementation, not in a generic public state bag.

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

- policy worker lifecycle
- native runtime package loading
- renderer/policy worker messaging
- surface painter delta application
- smoke boot of the workbench

The `packages/runtime-node` native package should have binding-level tests independent of Electron.

---

## V1 scope

V1 should prove the architecture, not complete the editor.

### V1 should include

- `packages/runtime-node` napi-rs native package
- Electron main owning the single native runtime session
- Electron main exposing separate policy-runtime and renderer-render IPC endpoints
- Electron main starting the policy worker
- policy worker using an IPC-backed runtime client instead of importing runtime-node
- renderer React shell
- Canvas2D surface painter using the explicit render bridge for visible frames
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
- Electron main owns policy worker lifecycle and the single native runtime session.
- Electron main exposes separate policy-runtime and renderer-render bridges.
- The renderer render bridge is explicit and narrow; it is not the full runtime API.
- Visible `RenderFrame` payloads do not ride the general workbench snapshot.
- Surface invalidation events, not generic snapshot changes, trigger renderer frame requests.
- The native runtime package exposes domain APIs, not product behavior.
- Keyboard/keymap resolution belongs in policy worker.
- Rust executes native editing/rendering intents.
- Splits and panes belong in TypeScript, not Rust.
- Runtime calls must be coarse-grained.
- Electron main owns the single native runtime session for the Electron distribution.
- Policy runtime access and renderer render access must use separate explicit bridges.
- Normal visible `RenderFrame` payloads must not ride the general workbench/app snapshot.
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
Electron main    = policy worker supervision + native runtime broker + IPC bridges
Rust runtime     = authoritative native state and services
runtime-node     = thin napi-rs native adapter owned by Electron main
```

The defining principle is:

> Build the default editor out of the same hook/dispatch/module substrate that users customize.

That is how the editor remains deeply extensible, reloadable, and clanker-editable without sacrificing the Rust authority model for text truth and hot paths.
