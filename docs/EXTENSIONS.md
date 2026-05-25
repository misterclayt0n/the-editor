# Extension Architecture

The Editor is a kernel plus packages.

The kernel is intentionally small and boring. Packages create the product.

A default distribution is just a curated package set. User configuration is also a package. Reload replaces package contributions without restarting the app.

The goal is not to build an editor with plugins. The goal is to build a programmable editor shell where nearly every product decision can be replaced.

Users should be able to:

- replace the command palette
- replace the file tree
- replace the statusline
- replace the tab strip
- replace Helix-style interaction with Vim, Emacs, or their own model
- add a browser pane
- add an agent panel
- add a diagnostics monster that screams when LSP errors appear
- add arbitrary local tools, games, canvases, terminals, dashboards, and workflows
- edit their configuration as code
- run `:reload`
- keep working

This document defines the extension architecture that makes that possible.

---

## 1. North star

The Editor should feel like a programmable product shell, not a fixed app with a small plugin API.

The important distinction is:

```txt
not: editor + plugin API
yes: kernel + packages
```

The kernel provides the stable mechanics required to load, unload, compose, focus, route, persist, and recover packages.

Packages provide the editor's personality.

That means the default editor experience is not special. It is the default package set.

---

## 2. Five primitives

The extension system is built from five primitives:

1. **Kernel**
2. **Package**
3. **Contribution**
4. **Registry**
5. **Store**

Everything else should be explainable in terms of these primitives.

### Kernel

The stable host that survives reload.

### Package

The install, activation, reload, and disposal unit.

### Contribution

A typed thing a package adds to the editor.

Examples:

- command
- keymap
- interaction profile
- role implementation
- tool type
- theme
- language support
- decoration provider
- gutter provider
- palette provider

### Registry

A host-owned typed collection of contributions.

Examples:

- command registry
- role registry
- tool registry
- theme registry
- interaction profile registry

### Store

A declared state container with a lifecycle.

Stores make reload deterministic. State is not a random global bag.

---

## 3. Kernel responsibilities

The kernel owns only the mechanics required to load, unload, compose, persist, and recover packages safely.

The kernel owns:

- package discovery
- package activation
- package disposal
- explicit reload
- activation scopes
- contribution ownership tracking
- registry lifetimes
- persistence mechanics
- typed store lifetimes
- package failure tracking
- package recovery mechanics

The kernel does not own product behavior.

Command buses, workbench layout trees, focus models, input routers, active interaction profile selection, theme selection, diagnostics models, and similar application systems are not kernel concepts. They are host-owned services, stores, registries, or packages built on top of the kernel primitives.

The kernel should not decide what the command palette looks like, how commands execute, how the statusline is arranged, what interaction model is active, what the file tree feels like, what theme is selected, or what tools users can mount.

The kernel is the package runtime, not the product brain.

---

## 4. Package responsibilities

A package can contribute any part of the product experience.

Packages may contribute:

- commands
- keybindings
- interaction profiles
- role implementations
- tools
- themes
- icons
- language support
- syntax providers
- completion providers
- diagnostics sources
- decorations
- gutters
- status segments
- palette providers
- workbench panels
- editor behaviors
- native-backed services
- arbitrary UI

Packages are trusted local code.

A package may be first-party, third-party, workspace-local, or user-local. The kernel should treat them through the same contribution model.

---

## 5. Default distribution

The default distribution is a package set.

For example:

```txt
@the-editor/kernel
@the-editor/editor
@the-editor/editor-engine-monaco
@the-editor/workbench
@the-editor/default-workbench
@the-editor/default-file-tree
@the-editor/default-command-palette
@the-editor/default-statusline
@the-editor/default-interaction-helix
@the-editor/default-theme
@the-editor/default-language-basics
```

These packages may ship together, but architecturally they are still packages.

This keeps the product honest: if a feature cannot be replaced, it should be because the kernel truly needs it, not because the default implementation was hardwired by accident.

The default editor engine is Monaco. It is provided by `@the-editor/editor-engine-monaco` behind the engine-neutral APIs in `@the-editor/editor`. Extension packages should target The Editor APIs, not Monaco APIs. Monaco can be exposed later as an unstable escape hatch if a real extension needs it, but it is not the core extension contract.

---

## 6. User configuration is a package

User configuration is not a separate magical settings layer.

User configuration is an extension package loaded late in the package graph.

Conceptual shape:

```txt
~/.the-editor/config/
  package.json
  activate.ts
  theme.ts
  keymap.ts
```

Example:

```ts
export function activate(editor: ExtensionContext) {
  editor.roles.select("commandPalette", "@acme/raycast-palette");
  editor.interaction.selectBaseProfile("@acme/vim");

  editor.commands.register("user.sayHello", () => {
    console.log("hello from my editor");
  });

  editor.keymaps.register({
    scope: "global",
    bindings: [{ key: "space h", command: "user.sayHello" }],
  });
}
```

Because config is a package, it is:

- typed
- reloadable
- shareable
- versionable
- inspectable
- composable

Running `:reload user-config` should reload configuration the same way any other package reloads.

---

## 7. Package shape

The simplest package has one activation entrypoint:

```txt
my-extension/
  package.json
  activate.ts
```

Manifest sketch:

```json
{
  "name": "@acme/my-extension",
  "editor": {
    "title": "My Extension",
    "main": "./activate.ts"
  }
}
```

Activation sketch:

```ts
export function activate(editor: ExtensionContext): Disposable | void {
  editor.commands.register("acme.doThing", () => {
    // ...
  });
}
```

The one-entrypoint model is the v1 default because it is obvious.

As packages become more sophisticated, a package may split by environment or target:

```txt
my-extension/
  package.json
  activate.ts
  ui.tsx
  worker.ts
  theme.ts
```

But split entrypoints are an optimization and organization tool, not the foundation of the mental model.

The foundation remains:

```txt
package activates -> package registers contributions -> kernel tracks ownership -> package disposes/reloads
```

---

## 8. Contribution ownership

Every contribution is owned by a package activation scope.

When a package reloads, the kernel disposes all contributions owned by that package scope.

This must be deterministic.

```txt
activate package
  -> register contribution A
  -> register contribution B
  -> register contribution C

reload package
  -> dispose C
  -> dispose B
  -> dispose A
  -> reload source
  -> activate package again
```

The kernel should track:

- package id
- activation generation
- contribution id
- contribution type
- disposable cleanup
- mount ownership, if UI was mounted
- store ownership, if volatile state was created

Package code should not need to manually remember everything it registered. Registration should attach ownership automatically through the active activation context.

---

## 9. Reload model

Reload is explicit and deterministic.

The product model is:

```txt
edit extension/config code
run :reload
old contributions dispose
new contributions register
state reattaches where possible
continue working
```

Support commands like:

```txt
:reload
:reload <package>
:reload <package>/<target>
```

Examples:

```txt
:reload
:reload user-config
:reload @acme/raycast-palette
:reload @acme/vim-profile
```

The kernel reload lifecycle:

```txt
1. identify reload scope
2. snapshot reload-relevant host state
3. dispose owned contributions
4. unload or invalidate source module cache
5. load source again
6. activate package
7. register new contributions
8. remount affected roles/tools
9. reattach stores where valid
10. restore focus if possible
11. report failures without killing the shell
```

Reload should not depend on file-watch hot module replacement.

File watching may offer convenience, but the official mechanism is explicit reload.

---

## 10. Roles and tools

Packages can contribute many things, but UI replacement has two especially important contribution kinds:

```txt
roles = replaceable product responsibilities
tools = arbitrary mounted apps/panes
```

Roles are scarce and special.

Tools are common and flexible.

---

## 11. Roles

A role is a named product responsibility that the kernel or workbench understands.

Only one implementation is active for a role at a time.

Examples:

- command palette
- completion menu
- statusline
- tab strip
- notifications
- dialogs
- file tree, if treated as a shell responsibility

A role has:

- a role id
- a typed contract
- a host-owned mount point or lifecycle
- one active implementation
- explicit selection/arbitration

Example:

```txt
commandPalette role
  kernel knows how to open, close, focus, and restore from it
  package decides how it looks and feels
```

The role contract should be as small as possible. Every role becomes a stable product seam, so roles should not be created casually.

### Role arbitration

If multiple packages provide the same role, the active implementation is selected by host/user/workspace configuration.

Not:

- last package wins
- random priority wins
- implementations negotiate privately

Example:

```txt
role.commandPalette = "@acme/raycast-palette"
role.statusline = "@acme/minimal-statusline"
```

If the selected implementation fails, the kernel should fall back to a known-good implementation or a minimal recovery implementation.

---

## 12. Tools

A tool is an arbitrary mounted app or pane.

Examples:

- file tree
- terminal
- browser
- git panel
- search panel
- diagnostics panel
- markdown preview
- agent panel
- extension manager
- Doom
- a video player
- a project dashboard

A tool has light metadata:

- tool type id
- title
- icon
- default placement
- instance policy
- persistence policy
- focus behavior

Inside the mounted pane/root, the tool owns its subtree.

A tool may render:

- React
- canvas
- WebGL
- plain DOM
- an iframe/webview if the host allows it
- any internal layout it wants

The kernel/workbench should not care how the tool works internally.

### Tool multiplicity

Tools may have many instances.

Common policies:

- `singleton`
- `singleton-per-workspace`
- `multi-instance`

Examples:

- command palette: role, single active implementation
- terminal: tool, many instances
- browser: tool, many instances
- file tree: can be a role or a singleton tool depending on how integrated it needs to be

---

## 13. Workbench layout

The kernel/workbench owns global layout truth.

Packages do not imperatively mutate the layout tree.

Instead, packages request placement intents.

The workbench owns:

- dock roots
- editor split tree
- split sizing
- group/tab truth
- active pane truth
- empty group cleanup
- split collapse
- focus repair after close/reload
- persistence of layout state

Conceptual layout tree:

```txt
workbench
  dock roots
    leftSidebar
    rightSidebar
    bottomPanel
    overlays
  editor area
    split nodes
      group leaves
        pane tabs
          editor panes or tool panes
```

Placement intent examples:

```ts
editor.workbench.openTool("browser", {
  title: "Docs",
  input: { url: "https://example.com" },
  placement: { kind: "split", direction: "right", of: "active" },
});
```

```ts
editor.workbench.openTool("agent", {
  placement: { kind: "dock", root: "rightSidebar" },
});
```

Useful v1 placement kinds:

```ts
type PlacementIntent =
  | { kind: "tab"; group?: "active" | GroupId; index?: number }
  | {
      kind: "split";
      direction: "left" | "right" | "up" | "down";
      of?: "active" | PaneId | GroupId;
    }
  | {
      kind: "dock";
      root: "leftSidebar" | "rightSidebar" | "bottomPanel" | "overlayModal" | "overlayFloating";
    };
```

Packages get expressive placement without raw tree surgery.

---

## 14. Focus and input routing

The kernel owns focus and input routing.

This is non-negotiable because arbitrary packages must compose safely.

The input router owns:

- active root
- active focus target
- focus history
- focus restoration
- scope stack
- active interaction profile
- overlay precedence
- escape/cancel behavior
- stale focus repair after reload/disposal

Packages request focus. They do not mutate global focus truth directly.

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

Scopes let command/key handling compose across:

- editor modes
- palettes
- dialogs
- overlays
- tools
- global commands

---

## 15. Interaction profiles

Interaction profiles are first-class.

Keymaps are not enough to model Vim, Helix, Emacs, Kakoune, or custom modal workflows.

An interaction profile owns input interpretation.

Examples:

- default profile
- Helix profile
- Vim profile
- Emacs-ish profile
- custom user profile

The active model is:

```txt
base profile + overlays
```

The kernel owns:

- active profile selection
- overlay precedence
- input routing into the profile
- execution of profile effects

A profile owns:

- profile state
- modes
- pending-key behavior
- key sequence interpretation
- operator/motion composition
- mode-specific scopes
- profile-specific cursor/mode metadata
- translation from input to effects

Conceptual shape:

```ts
interface InteractionProfile {
  id: string;
  initialState(): unknown;
  reduce(input: InteractionInput, state: unknown): InteractionResult;
}

interface InteractionResult {
  state: unknown;
  effects: readonly InteractionEffect[];
  snapshot: InteractionSnapshot;
}
```

Effects are host-executed:

```ts
type InteractionEffect =
  | { type: "command.execute"; command: string; args?: unknown }
  | { type: "focus.request"; target: FocusTarget }
  | { type: "overlay.open"; role: string }
  | { type: "overlay.close"; role: string };
```

Profiles should not directly mutate layout, focus, or document internals. They compose workflows from stable commands/effects.

This keeps interaction powerful but inspectable.

---

## 16. Commands and keymaps

Commands are named effects.

Keymaps are data contributions used by interaction profiles or simpler command resolution paths.

A package may register commands:

```ts
editor.commands.register("acme.openBrowser", async (ctx, args) => {
  await ctx.workbench.openTool("browser", {
    input: args,
    placement: { kind: "split", direction: "right", of: "active" },
  });
});
```

A package may register keymaps:

```ts
editor.keymaps.register({
  scope: "global",
  bindings: [
    { key: "cmd+p", command: "commandPalette.open" },
    { key: "space b", command: "acme.openBrowser" },
  ],
});
```

For simple profiles, keymaps may map directly to commands.

For modal profiles, keymaps are one input into the profile state machine. Pending input belongs to the interaction profile/session, not to the keymap registry itself.

---

## 17. Stores

Extensions and first-party modules must not use a raw global state bag.

State is declared through typed stores.

Store kinds:

- **host stores** — canonical kernel/workbench/editor state
- **module stores** — volatile package-local state disposed on reload
- **persisted stores** — package-owned persisted state with versioning
- **session stores** — state tied to an interactive session or mounted instance

Conceptual API:

```ts
const local = editor.stores.module(myLocalStore);
const persisted = editor.stores.persisted(myPersistedStore);
```

Persisted store declaration:

```ts
const doomSaveStore = definePersistedStore<DoomSaveState>({
  id: "@acme/doom.save",
  version: 1,
  init: () => ({ level: 1 }),
  serialize: (state) => state,
  deserialize: (raw) => parseDoomSave(raw),
});
```

The kernel owns persistence mechanics.

Packages own schema, versioning, serialization, and semantics.

---

## 18. Diagnostics as data

Diagnostics are data first, not one special UI.

A diagnostics source writes diagnostics to a diagnostics store.

Many presentations can read diagnostics:

- editor decorations
- gutter markers
- hover details
- statusline summaries
- diagnostics panel tool
- command palette navigation
- file tree badges
- tab badges
- notification rules
- screaming monster package

Conceptual API:

```ts
const collection = editor.diagnostics.createCollection("acme.linter");
collection.set(uri, diagnostics);
collection.clear(uri);
```

Readers use selectors:

```ts
editor.diagnostics.forUri(uri);
editor.diagnostics.forLine(uri, line);
editor.diagnostics.summary();
```

This creates a deep seam:

```txt
sources write diagnostics
presentations read diagnostics
```

No monolithic diagnostics UI role is required.

---

## 19. Decorations and gutters

Decorations and gutters are presentation contributions over editor state.

They should be visible-range oriented.

Providers should not need to recompute whole-document or whole-project state during paint.

A provider should consume a visible context and return compact visible results.

Conceptual gutter provider:

```ts
editor.gutters.registerProvider({
  id: "acme.markers",
  lane: "markers",
  provide(ctx) {
    return ctx.visibleLines.map((line) => ({
      line: line.index,
      kind: "marker",
    }));
  },
});
```

If a feature needs expensive metadata, it should maintain/cache that metadata separately and project it onto visible rows when asked.

---

## 20. Native and Node-backed extensions

Extensions are trusted local code.

This system is not primarily a sandbox.

Packages may use:

- Node APIs
- local processes
- native modules
- workers
- external CLIs
- local servers

The architecture should allow powerful extensions without making native code part of the kernel's mental model.

Recommended rule:

```txt
heavy or privileged work belongs behind package-owned services/workers
UI contributions should stay renderer-safe where practical
```

If a package wants to mount a browser, terminal, emulator, LSP adapter, agent runtime, or native-backed subsystem, the architecture should allow it.

---

## 21. Failure model

Packages are trusted, but failures must not destroy the shell.

The kernel should:

- associate errors with package id and contribution type
- dispose failed contribution scopes
- keep other packages alive where possible
- show reload diagnostics
- provide fallback role implementations for critical roles
- preserve host-owned state across failures
- avoid leaving focus/layout in invalid states

Examples:

- if a statusline package fails, hide or replace the statusline
- if a command palette package fails, fall back to a minimal palette
- if a tool fails, show an error pane for that tool instance
- if user config fails, show recovery UI and allow editing/reloading it

Recovery UI is one of the few things the kernel may own directly.

---

## 22. Styling and themes

The styling model should balance freedom and composability.

Recommended ownership:

```txt
kernel/default shell provides semantic theme tokens
packages own component styling inside their mounted subtrees
```

Packages should not need permission to look different.

But unrestricted global CSS can make packages impossible to compose. Prefer:

- semantic tokens
- scoped styles
- package-owned class namespaces
- explicit theme contributions
- controlled global reset/base styles

A user should be able to install a radically different command palette or statusline without rewriting the workbench.

---

## 23. Minimal v1

The v1 extension system should be real but small.

Build only the primitives needed to prove the model:

- package activation
- contribution ownership
- disposal
- explicit reload
- command registry
- keymap registry
- role registry
- tool registry
- basic stores
- input router
- layout placement intents
- user config package

Avoid implementing every possible role or provider upfront.

A good v1 proof:

1. default command palette is a role package
2. default file tree is a tool or role package
3. default interaction profile is a package
4. user config can replace a role selection
5. user config can register a command/keybinding
6. `:reload user-config` works
7. a custom tool can open in a split or dock
8. a failed package can be recovered without restarting the app

---

## 24. Anti-goals

Avoid these traps:

- making the kernel a giant product-brain blob
- treating extensions as only UI skins
- hardcoding the default package set as special
- letting packages imperatively mutate global layout
- coupling command semantics to one palette implementation
- coupling interaction semantics to one editor engine implementation
- exposing Monaco as the primary extension API
- requiring file-watch HMR for reload to work
- forcing every package to live entirely in one execution context
- creating too many formal roles too early
- creating registries before there is a real contribution type
- making stores a raw string-keyed global state bag
- making native code necessary to understand normal editor behavior

---

## 25. Final summary

The target architecture is:

```txt
small stable kernel
  + packages as the product
  + typed contributions
  + host-owned registries and stores built on kernel primitives
  + replaceable roles
  + arbitrary tools
  + first-class interaction profiles
  + explicit reload lifecycle
  + typed stores
  + focus/layout/input systems derived outside the kernel
```

This gives The Editor its identity:

```txt
It is whatever editor you want it to be.
```
