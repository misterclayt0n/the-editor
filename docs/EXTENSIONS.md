# Extension Architecture

This document defines the target extension architecture for the editor.

It answers these questions:

1. What does “100% extensible” actually mean here?
2. What stays in the stable host shell, and what becomes swappable?
3. What is a role, a tool, and an interaction profile?
4. How should `:reload` work?
5. How do we support arbitrary UI, weird apps, and native-backed extensions without turning the workbench into mush?

This document extends:

- `docs/ARCHITECTURE.md`
- `docs/TYPESCRIPT.md`

---

## North Star

The editor should feel like a programmable product shell, not a fixed app with a tiny plugin API.

Users should be able to say things like:

- “I don't like this command palette. Make it look like X.”
- “Replace Helix bindings with Vim.”
- “Open Doom inside the editor.”
- “Swap out the statusline.”
- “Add a weird custom panel app.”

And the answer should usually be:

```txt
install or edit a module
run :reload
continue working
```

The stable host shell should remain small and boring.
Most of the editor's personality should live in swappable TypeScript extension packages.

---

## Core decisions

These are the current architectural decisions.

### 1. Stable host shell, swappable modules

The shell is not fully replaceable.
The host still owns:

- app/window lifecycle
- layout tree
- focus graph
- key routing
- persistence
- extension runtime
- reload/disposal
- editor text surface mounting

But most product behavior above that should be swappable.

### 2. Typed roles + tools

The extension model has two important UI categories:

- **roles** = replaceable built-in responsibilities
- **tools** = pluggable mounted UI apps/panes

### 3. One extension package, split entrypoints

One package can contribute multiple targets:

- runtime behavior
- renderer UI
- interaction profile
- theme/data

A package is the install unit.
Targets are the activation/reload unit.

### 4. Interaction profiles are first-class

Keymaps are not enough.
A package must be able to replace the active interaction model:

- Helix profile
- Vim profile
- Emacs-ish profile
- custom overlays

The active model is:

```txt
base profile + overlays
```

### 5. Host owns canonical state; modules own local state

Host-owned:

- documents/text surfaces/session truth
- workbench layout truth
- focus truth
- command/keymap/profile activation
- palette history and other shared product state

Module-owned:

- local UI state
- local view state
- arbitrary app state

Modules may persist namespaced state through host APIs.

### 6. Hot swapping means explicit reload, not file-watch HMR

The core product model is:

```txt
edit code
run :reload
old contributions dispose
new contributions register
state reattaches where possible
```

No architecture should depend on magical watcher-driven HMR.

### 7. Reload works at package and target granularity

Support both:

```txt
:reload my-extension
:reload my-extension/ui
:reload my-extension/runtime
:reload my-extension/interaction
```

### 8. Host owns global layout

Extensions do not imperatively rewrite the workbench tree.
The host owns:

- dock roots like `leftSidebar`, `rightSidebar`, `bottomPanel`, and overlays
- the main **editor split tree**
- split sizing
- group/tab truth
- pane focus truth

The editor area is a persistent tree of:

```txt
split nodes
  -> group leaves
    -> pane tabs
      -> text surfaces or tool panes
```

Extensions request **placement intents** like “open in a right split” or “open as a tab in the active group”.
They do not perform raw tree surgery.

The host should implement those intents through a deep workbench layout model:

```ts
layout.apply(intent): LayoutChange
layout.snapshot(): WorkbenchEditorLayoutSnapshot
layout.assertInvariants(): void
```

The layout model, not extension code and not renderer code, owns split-tree mutation, active pane repair, empty-group pruning, split collapse, size normalization, duplicate surface-pane handling, and layout invariant checks.

`WorkbenchController` is the host-facing adapter. It may expose commands and APIs such as `openSurface`, `openTool`, `movePane`, `closePane`, and `resizeSplit`, but those operations should lower into layout intents.

After layout mutation, `WorkbenchController` requests input focus through `InputRouter` when the active pane/group changes. Layout activation and input focus are coordinated but separately owned.

### 9. Extensions own arbitrary subtrees inside those roots

Once mounted, an extension may render any internal React subtree it wants.
That includes:

- custom component trees
- local split layouts inside a tool pane
- canvases
- games like Doom
- internal mini-apps
- animations
- routers

### 10. Roles are scarce and special

Roles should stay small in number.
Everything else should be a tool.

### 11. Host owns focus and key routing

This is non-negotiable.
The host controls:

- which root is focused
- which input scope is active
- overlay precedence
- cancel/back/escape behavior
- focus restoration across reloads

### 12. Input scopes are structured

Think in terms of scopes such as:

```txt
global
editor.normal
editor.insert
editor.select
palette
dialog
tool.<id>
tool-kind.<kind>
```

### 13. Interaction profiles compose workflows from stable primitives

For now, profiles should not replace the primitive editor semantic substrate.
They should compose different behaviors from shared lower-level primitives.

---

## Roles vs tools

This distinction is important.

> **Naming note:** a workbench **tool** is not the Rust/native text `Surface` from `the-lib/surface.rs`.
> In this document, **tool** means a mounted workbench app or pane.

## Roles

A **role** is a named built-in job that the host understands.
Only one implementation is active for a given role at a time.

Examples:

- `commandPalette`
- `completionMenu`
- `statusline`
- `tabstrip`
- `notifications`
- `dialogs`

A role has:

- a host-owned mount point
- a small typed capability contract
- a single active implementation
- integration with shared host behavior

Example:

```txt
commandPalette role
  host knows how to open/close/focus it
  module decides how it looks and behaves
```

## Tools

A **tool** is a mounted UI app or pane.
The host provides either:

- a dock root
- or an editor pane inside the split tree

The module owns the subtree inside that mounted pane/root.

Examples:

- Doom
- markdown preview
- terminal UI
- file tree
- search pane
- git pane
- extension manager
- inspector
- arbitrary experimental panel

A tool has:

- a default root target
- light metadata
- arbitrary internal UI
- zero implication that it is a built-in product responsibility

A tool type's `root` is its **default placement**, not a prison.
Extensions may also open tools into editor splits through the host workbench API.

## Rule of thumb

```txt
roles    = replaceable built-ins
tools = pluggable apps/panes
```

---

## What the host shell owns

The host shell must stay stable and predictable.

It owns:

- the editor split tree
- dock root allocation for roles and tools
- focus graph
- key routing and scope activation
- workbench/session persistence
- extension discovery/loading/reloading
- command registry
- keymap registry
- interaction profile stack
- palette provider registry
- shared history and recents
- notifications/dialog launching semantics
- renderer mounting lifecycle

The host should be the boring traffic cop, not the product personality.

---

## What extension packages own

An extension package can own any mix of:

- commands
- keymaps
- interaction profile overlays
- role implementations
- tool implementations
- palette providers
- themes/tokens
- runtime services/workflows
- native-backed subsystems

The package may contribute different things from different entrypoints.

---

## Extension package shape

Preferred shape:

```txt
my-extension/
  package.json
  runtime.ts
  ui.tsx
  interaction.ts
  theme.ts
```

Current first-pass manifest shape:

```json
{
  "name": "@acme/the-editor-extension",
  "editor": {
    "title": "Acme Extension",
    "runtime": "./runtime.ts",
    "ui": "./ui.tsx",
    "interaction": "./interaction.ts",
    "theme": "./theme.ts",
    "defaultTargets": ["runtime", "interaction", "ui"]
  }
}
```

### Why split entrypoints

Because these targets have different jobs:

- `runtime` can use Node APIs, native modules, long-lived services
- `ui` renders React trees into host roots
- `interaction` contributes profiles, keymaps, scopes, overlays
- `theme` contributes tokens, icons, theme data

This keeps the package model unified without pretending every target lives in the same environment.

---

## Execution contexts

The existing high-level direction from `docs/TYPESCRIPT.md` still applies.

```txt
Electron main        app lifecycle + worker supervision
Policy worker        runtime/editor behavior/modules
Renderer             React UI + surface painting + UI module mounting
Rust runtime         canonical native state and services
```

## Recommended ownership by target

### Runtime target

Lives in the policy worker.

Owns things like:

- command registration
- runtime workflows
- service adapters
- palette providers
- interaction profile registration
- native module usage
- Node/process/filesystem access

### UI target

Lives in the renderer.

Owns things like:

- role components
- tool components
- arbitrary React trees
- canvas/WebGL apps
- local UI state

### Interaction target

Likely loaded into the policy worker/editor substrate side.

Owns things like:

- base profile definition
- keymaps
- mode transitions
- pending-key logic
- overlays

### Theme target

Can be data-only, or partly renderer-facing.

Owns things like:

- tokens
- icons
- semantic colors
- spacing hints
- maybe component recipes later

---

## Native module policy

Extensions are trusted local code.
This system is not trying to sandbox them.

That means:

- runtime targets may load Node/native-backed modules
- runtime targets may talk to local processes or OS APIs
- tool apps may talk to their runtime half through host channels

The recommended rule is:

```txt
native/Node-heavy code belongs in runtime entrypoints
renderer UI entrypoints stay renderer-safe where possible
```

This keeps the renderer simpler while still allowing powerful extensions.

If someone wants a wild extension with native code behind it, the architecture should allow it.

---

## Role contracts

Roles are not just “some component in some div.”
They need small typed contracts.

The contract should be as small as possible while still supporting host integration.

## Example: command palette role

See [COMMAND_PALETTE.md](COMMAND_PALETTE.md) for the command id, alias, argument provider, keybinding, and palette session model.

The host should know how to:

- open it
- close it
- focus it
- restore focus after close
- feed it provider-backed items/query state
- receive execute/cancel/selection events

The palette implementation should own:

- rendering
- grouping
- keyboard feel
- visual hierarchy
- ranking tweaks
- overall UX personality

The host should not hardcode the palette's visual design.

## Example: completionMenu role

The host should know how to:

- open it
- close it
- keep focus in the editor when it is an editor-attached overlay
- provide the active completion session snapshot
- route selection/accept/cancel commands through stable runtime commands

The completion implementation should own:

- rendering
- layout
- docs pane behavior
- row styling
- source badges/icons
- all visual personality

The completion data model itself should stay outside the role, so users can swap the UI without rewriting autocomplete control flow.

Default autocomplete is intentionally not a generic source-fanout feature. It is an explicit, snapshot-driven controller with provider-specific LSP policy. See [COMPLETION.md](COMPLETION.md). Extension completion providers should not be added back until they can satisfy the same snapshot, cancellation, and no-runtime-read-on-typing invariants.

## Diagnostics are data first, not a role

Diagnostics should not be modeled as one giant special UI role.
They are canonical editor state that many projections consume.

The host/runtime side should own a diagnostics store with collection-style writes such as:

- `editor.diagnostics.createCollection(...)`
- `collection.set(uri, diagnostics)`
- `collection.replace(...)`
- `collection.clear()`

Extensions should be able to read diagnostics at will through selectors such as:

- `editor.diagnostics.all()`
- `editor.diagnostics.forPath(path)`
- `editor.diagnostics.forLine(...)`
- `editor.diagnostics.atPosition(...)`
- `editor.diagnostics.summary()`

This keeps the seam deep:

- **sources write diagnostics**
- **presentations read diagnostics**

That means the same diagnostics can be projected through existing extension seams instead of inventing a monolithic diagnostics UI contract:

- `decorations` for underlines / inline projections
- `gutters` for line markers
- `hover` for current-position fallback details
- `statusline` for counts / summaries
- `tools` for diagnostic panels
- `pickers` / palette providers for navigation
- file tree or tab UI contributions for badges / rollups

LSP diagnostics are just one adapter.
A first-party LSP module may mirror runtime LSP diagnostics into `editor.diagnostics`, but user extensions should be equally able to publish diagnostics from any source.

## Gutter provider model

Gutter providers are presentation policy, not text layout owners.

The low-level runtime/render frame provides visible gutter facts:

```ts
interface GutterRow {
  surfaceRow: number;
  docLine: number | null;
  lineNumber: number | null;
  active: boolean;
  selected: boolean;
  startsLogicalLine: boolean;
  wrappedContinuation: boolean;
}
```

Extension gutter providers should consume visible rows and metadata, then return compact visible gutter lines:

```ts
editor.gutters.registerProvider({
  id: "example.markers",
  lane: "markers",
  provide(ctx) {
    return ctx.gutterRows.map((row) => ({
      id: `marker:${row.surfaceRow}`,
      surfaceRow: row.surfaceRow,
      kind: "marker",
    }));
  },
});
```

Provider code should not require a full `RenderFrame` during normal editor refresh. If a feature needs expensive metadata, such as VCS hunks, it should maintain/cache that metadata separately and only project it onto `ctx.gutterRows` when asked to draw visible gutters. Provider results are fetched by an explicit visible-surface gutter request, not by the general workbench snapshot.

## Example: statusline role

The host should know how to:

- mount the active statusline implementation
- provide shared status segments/model
- handle visibility and persistence

The implementation should own:

- visual layout
- styling
- interaction affordances
- micro-UX

## Expected role properties

A role implementation should generally expose:

- `roleId`
- mount component/factory
- capability hooks required by the role
- optional configuration schema
- disposable cleanup

Roles should remain few because every role creates a real product contract.

---

## Tool contracts

Tools should have a lighter contract.
They are closer to mounted apps.

Expected tool metadata:

- `toolTypeId`
- title
- icon
- default root target
- closeability
- persistence key/policy
- focus behavior
- maybe instance policy (`singleton` vs `multi-instance`)

Inside the mounted pane/root, the tool owns its subtree.

The host should be able to do things like:

- open tool type X in its default root
- open tool type X in an editor split
- open tool type X as a tab in the active group
- reopen prior persisted tools
- focus tool Y
- close tool Y
- reload tool package Z

But the host should not care how the tool internally renders itself.

### Editor split placement API

The host should expose placement intents, not raw tree mutation.

Example shape:

```ts
editor.workbench.openTool("webview.video", {
  title: "Subway Surfers",
  input: { url: "https://..." },
  placement: { kind: "split", direction: "right", of: "active" },
});
```

And for text surfaces:

```ts
editor.workbench.openSurface(surfaceDescriptor, {
  placement: { kind: "tab", group: "active" },
});
```

Good placement intents for v1 are:

- `{ kind: "tab", group?: "active" | groupId, index?: number }`
- `{ kind: "split", direction: "left" | "right" | "up" | "down", of?: "active" | paneId | groupId }`
- `{ kind: "dock", root: "leftSidebar" | "rightSidebar" | "bottomPanel" | "overlayModal" | "overlayFloating" }`

---

## Role arbitration

Roles are single-active.

That means the host must decide which implementation wins.
This should not be implicit chaos.

Recommended rule:

```txt
host/user/workspace config chooses the active implementation per role
```

Not:

- last one wins magically
- highest random priority wins
- modules negotiate privately

If multiple packages provide `commandPalette`, the host should allow an explicit selection.

Example:

```txt
role.commandPalette = "@acme/raycast-palette"
role.statusline     = "@acme/minimal-statusline"
```

---

## Tool multiplicity

Unlike roles, tools may have many instances.

Examples:

- one terminal tool type, many terminal instances
- one preview tool type, many preview instances
- one Doom tool type, maybe one or many instances depending on declaration

Recommended tool instance policies:

- `singleton`
- `singleton-per-workspace`
- `multi-instance`

---

## Interaction profiles

Interaction must be first-class, not an afterthought.

## Model

```txt
base profile + overlays
```

Examples:

- base profile: Helix
- base profile: Vim
- overlays: leader pack, workspace overrides, extension-specific bindings

## Host-owned interaction stack

The host owns precedence and activation.

Example stack:

```txt
base profile
+ user overlay
+ workspace overlay
+ temporary modal overlay
```

This should be deterministic and inspectable.

## Profiles should own

- keymaps as data/resolution inputs
- profile reducer/state/snapshot
- mode definitions and mode transitions
- all pending input behavior, including keymap prefixes and profile operators
- workflow composition from stable primitives
- profile-specific editor UX rules

There is one host-owned interaction session per editor window. The active profile owns the meaning of its state, but the host owns the session runner, routing, and effect execution.

Pending input is not split across registries. A keymap prefix like `g`, a sticky node like `space`, and an operator like `f<char>` are all interaction-session state. The keymap registry is a contribution registry/resolver and must not own mutable pending keys.

Pending input clears on focus or active-surface changes in v1. Profiles may keep explicit surface-specific substate only for real surface-local concepts such as last motion or jump-list metadata.

Modes are profile-owned. The host exposes mode-like data only through the interaction snapshot, for example cursor shape, mode title, and mode scopes. The workbench should not keep a separate canonical mode field.

## Profiles should not own, for now

- replacement of the primitive editor engine semantics
- a separate document model
- direct mutation of focus/layout shell truth
- direct runtime/workbench mutation outside structured host-executed effects

---

## Focus and key routing

The host owns global focus and key routing through one `InputRouter` per editor window.

There is no separate compatibility focus registry. Focus is routing state.

`InputRouter` must include:

- active root tracking
- active focus target
- focus history/restoration
- active scope stack
- active interaction session for the editor window
- active profile selection
- pending-input cancellation on focus/surface changes
- overlay precedence
- escape/back/cancel handling
- module disposal safety
- stale role/tool focus repair

Recommended scope families:

```txt
global
editor.normal
editor.insert
editor.select
palette
dialog
tool.<instanceId>
tool-kind.<toolTypeId>
```

This gives enough structure for:

- editor modal behavior
- palette-specific controls
- dialog confirm/cancel
- tool-local controls
- global escape hatches

Profiles may contribute scopes through their snapshot/reducer state, but `InputRouter` decides the final scope stack from focus, overlays, tools, and the active profile. Profiles do not directly control global focus.

Workbench owns layout activation, not input focus. A pane/group focus command updates workbench active pane/group/item first, then sends a focus intent to `InputRouter`. Roles, tools, overlays, and workbench modules request focus intents; they do not mutate focus state themselves.

---

## Command palette architecture

The command palette is the most important role to get right.

## Source of truth

The host owns:

- command registry
- tool registry hooks
- file/symbol/search/picker providers
- history/recents
- palette provider registry

The active palette role owns:

- visual design
- interaction feel
- grouping
- acceptance flow
- filtering/ranking customization

## Filtering/ranking

This should be hybrid.

The host provides:

- default fuzzy matching utilities
- history/recentness information
- provider metadata

The palette role may:

- use defaults
- tune scores
- group differently
- bias categories differently
- feel more like VS Code, Spotlight, Raycast, etc.

This is how “make it look and feel like X” becomes real without rewriting every backend registry.

---

## State model

Extensions and first-party modules must not use a raw string-keyed host state bag. The host exposes typed state stores through `editor.stores`.

Store access is token-based, not string-based:

```ts
const local = editor.stores.module(myLocalStore);
const persisted = editor.stores.persisted(myPersistedStore);
```

The store token declares the ID, kind, initializer, and lifecycle metadata. Raw calls like `stateSlot("some.key")` are intentionally not part of the architecture.

Store kinds:

- **host stores** — canonical host shell state owned by `packages/editor`
- **module stores** — volatile module-local state disposed on module unload/reload
- **persisted stores** — module-owned state with explicit versioning and serialization
- **session stores** — state tied to an interactive session ID

## Host-owned canonical state

Examples:

- document/text-surface/session state
- layout truth
- split tree / groups / pane descriptors
- active role selections
- active interaction profile selection
- active group / active pane truth
- palette history
- recent commands
- persisted open tools
- focus restoration info

## Module-owned local state

Examples:

- current palette animation state
- local filter view state
- panel widget internals
- Doom game session state
- local canvas state

Module-owned local state should use module stores. The module's current provenance is part of the storage key, so reload can dispose stale volatile state predictably.

## Persisted module state

Persisted module state should use persisted store tokens.

Example:

```ts
const doomSaveStore = definePersistedStore<DoomSaveState>({
  id: "@acme/doom.save",
  version: 1,
  init: () => ({ level: 1 }),
  serialize: (state) => state,
  deserialize: (raw) => parseDoomSave(raw),
});
```

The host owns persistence mechanics. Modules own their schema, versioning, serialization, and semantics.

---

## Reload model

Reload is explicit and deterministic.

## Core command model

Support commands like:

```txt
:reload
:reload <package>
:reload <package>/<target>
```

Examples:

```txt
:reload @acme/raycast-palette
:reload @acme/raycast-palette/ui
:reload @acme/vim-profile/interaction
```

## Reload lifecycle

```txt
1. identify package/target scope
2. dispose that contribution scope
3. keep host-owned canonical state intact
4. reload target source code from disk
5. re-register contributions
6. remount roles/tools
7. reattach state where applicable
8. restore focus if possible
```

This should be the official hot-swap mechanism.

---

## Trust and failure model

Extensions are trusted code, but reload/disposal must still be robust.

The host should:

- track contribution ownership by package and target
- dispose in reverse registration order where needed
- isolate renderer mount failures to the package/target when possible
- fall back gracefully if a role implementation fails to load
- tool diagnostics for failed reloads

If a palette package fails to reload, the host should be able to fall back to a known-good palette role.

---

## Styling and themes

If users want to say “make this component look like X,” we need both freedom and structure.

Recommended model:

- host provides theme tokens and semantic slots
- UI modules may render arbitrary React trees
- UI modules may ship their own styling
- global shell styling should remain host-controlled enough to avoid total CSS warfare

That means:

```txt
host gives semantic tokens
module owns component styling
```

Not:

```txt
everyone sprays global CSS everywhere
```

The exact styling API can be decided later, but the ownership should look like this.

---

## Mapping this onto the current codebase

The current TS substrate already has the beginnings of the right shape.

Relevant existing files:

- `packages/editor/src/modules.ts`
- `packages/editor/src/contributions.ts`
- `packages/editor/src/commands.ts`
- `packages/editor/src/keymaps.ts`

Today we already have:

- module loading/unloading
- contribution ownership by module id
- command registration
- keymap registration

That is the seed of the bigger extension system.

## Immediate architectural implication

Instead of inventing a separate weird extension world later, we should evolve the existing substrate into:

- role registry
- tool registry
- interaction profile registry
- palette provider registry
- reload command support
- package/target contribution scopes

In other words:

```txt
current module/contribution system
  -> becomes the real extension substrate
```

Not a disposable prototype.

---

## Example packages

## 1. Vim profile package

```txt
@acme/vim-profile
  interaction.ts
```

Contributes:

- base interaction profile `vim`
- keymaps
- mode rules
- workflow composition

No UI required.

## 2. Raycast-style palette package

```txt
@acme/raycast-palette
  ui.tsx
  runtime.ts
```

Contributes:

- `commandPalette` role implementation
- maybe extra palette providers from runtime side

## 3. Doom package

```txt
@acme/doom
  ui.tsx
  runtime.ts
```

Contributes:

- tool type `doom`
- optional runtime save/state/native helpers

The host treats it as a tool, not as a role.

---

## Current v1 status

The first version now proves the core architecture instead of only sketching it.

Current host-owned seams include:

- package/target loading and explicit `:reload`
- role registry
- tool registry
- interaction profile registry
- completion presentation registry; the old completion source registry is deprecated for default autocomplete
- dock roots (`leftSidebar`, `rightSidebar`, `bottomPanel`, overlays)
- editor split tree with groups, tabs, and pane placement intents
- editor-attached overlays that keep focus/key routing in the editor while rendering swappable UI like hover and completion menus

Current public workbench direction is:

- use `editor.workbench.openTool(...)` for tool placement
- use `editor.workbench.openSurface(...)` for surface placement
- use placement intents like tab/split/dock
- let the host keep layout truth and focus truth

That is the important v1 proof:

- built-in product UI is already using the extension seams
- arbitrary tools can live in dock roots or editor splits
- reload can preserve the host shell while swapping module code
- the editor area is no longer a one-off special case

---

## Anti-goals

To keep this architecture sane, avoid these traps:

- making the host shell a giant product-brain blob
- treating extensions as only UI skinning
- letting modules imperatively mutate the global workbench tree
- coupling command semantics to one palette implementation
- coupling interaction profiles to one specific UI module
- requiring file-watch HMR for reload to work
- forcing every extension to live entirely in the renderer
- creating too many formal roles too early

---

## Final summary

The target architecture is:

```txt
stable host shell
  + typed replaceable roles
  + pluggable tools
  + a host-owned editor split tree
  + first-class interaction profiles
  + explicit reload lifecycle
  + host-owned focus/layout/state truth
  + extension-owned arbitrary subtrees inside mounted panes/roots
```

That gives us:

- hot swapping via `:reload`
- replaceable command palette/statusline/tabstrip
- swappable Helix/Vim-style interaction layers
- support for arbitrary panel apps like Doom or a video tool in a right split
- clanker-friendly module boundaries
- room for native-backed extensions without collapsing the core shell
```
