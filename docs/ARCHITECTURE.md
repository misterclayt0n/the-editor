# The Editor Architecture

The Editor is a programmable editor kernel plus packages.

The goal is not to build a fixed editor with a plugin API. The goal is to build a small local operating system for editing: a kernel that loads packages, a set of deep host modules, and an editor surface whose core behavior can be replaced, extended, and reloaded.

The default product is just one package set. User configuration is also a package. Workspace configuration is also a package.

This document is the broad entrypoint for understanding the repo.

---

## North star

The Editor should be:

- **Extensible to the core** — commands, keymaps, editor behavior, panes, statusline, trees, tools, themes, language features, and workflows are package contributions.
- **Fast in the hot path** — typing, movement, selection, scrolling, and paint are local and synchronous.
- **Simple and direct** — modules should own real state and hide meaningful complexity. No pass-through abstractions.
- **Cross-platform** — Electron is the intended host, but the renderer/product architecture should not depend on Electron directly.
- **Tree-sitter native** — syntax should eventually be parser-driven, not text-grammar-driven.
- **Composable for modal editing** — keymaps are global, context-based, and capable of supporting Vim/Helix/Emacs/custom interaction models.
- **Reloadable** — arbitrary runtime packages can be loaded, unloaded, and reloaded without restarting the app.

If the implementation starts requiring callers to understand hidden lifecycle details, background services, renderer hacks, or stale compatibility layers, it is drifting away from the goal.

---

## Current foundation

After the purge, the repo intentionally keeps only the spine:

```txt
packages/kernel      package runtime, ownership, registries, stores
packages/extensions  package authoring helpers
packages/commands    first package-backed product capability
packages/engine      editor state primitives, starting with text storage/transactions
docs/                architecture and extension docs
```

Everything else should be rebuilt deliberately.

---

## Core model

The Editor is composed from these concepts:

### Kernel

The stable runtime that survives package reloads.

The kernel owns:

- package activation
- package disposal
- package reload mechanics
- contribution ownership
- registries
- stores
- activation scopes
- failure tracking

The kernel does **not** own product behavior. It does not decide what the editor looks like, how tabs work, how input behaves, or what tools exist.

### Package

A reloadable unit of product behavior.

Packages can contribute commands, keybindings, editor modes, tools, panes, statusline items, themes, language features, diagnostics, gutters, file trees, and configuration.

### Contribution

A typed thing a package adds to the product.

Examples:

- command
- keybinding
- theme
- language definition
- syntax parser
- diagnostic provider
- gutter provider
- editor mode
- workbench view
- tool

### Registry

A host-owned typed collection of contributions.

Registries are where package contributions live. They track ownership so reload can remove old contributions deterministically.

### Store

A package-scoped state container with lifecycle.

Stores exist so reload does not become random global-state soup.

---

## Ownership rules

Each kind of truth should have one owner.

The intended ownership model:

```txt
Shell (window host)
  one per window: the shared theme + OS appearance, platform capability
  bridges, the active workspace, and routing input/dialogs/status to it

Kernel
  package lifecycle, registries, stores, ownership

Commands
  command contributions and execution

Keymaps
  global/contextual input routing and modal composition

Engine
  text storage, transactions, document state, future selections/history/layout primitives

Working copy
  URI-backed document lifecycle: open, dirty, save, revert, conflict

Renderer/product layer
  DOM/TUI/canvas drawing, CSS/components, input capture, viewport presentation, gutters/decorations UI

Workbench session
  panes, tabs/items, splits, focus routing, active item, layout

Workspace/files/resources
  roots, filesystem operations, watching, snapshots

LSP/language workers
  background language intelligence; never blocks typing

VCS/diff workers
  background metadata and diff computation; never blocks typing
```

Each open workspace is an independent session: its own kernel package graph,
workbench, working copy, LSP, VCS, and terminals. The shell keeps inactive
sessions mounted but hidden so their state (terminals, scroll, editor buffers)
survives a switch, and owns the one concern that is genuinely process-global —
the theme — so sessions never fight over the window's appearance.

React, when present, renders and dispatches. It should not own editor truth.

Electron, when present, provides privileged OS capabilities. It should not be the editor brain.

---

## Hot path

The interactive path must stay tiny.

A keystroke should look like:

```txt
keydown
  -> keymaps resolve command from context
  -> command applies synchronous text/editor transaction
  -> selection/history update
  -> product renderer schedules paint
  -> visible rows render
```

This path must not wait for:

- filesystem
- LSP
- tree-sitter
- VCS
- package reload
- workers
- native code
- async services

Background systems produce updates. They do not gate input.

---

## Engine direction

The editor should own its engine.

The current engine starts with a deep text subsystem:

- immutable text snapshots
- fast visible line/chunk extraction
- offset/position mapping
- transactions
- change descriptions
- document state through an Effect-facing service

The next engine work should move upward deliberately:

- grapheme/character boundary primitives
- visual position and line-column policy
- selection values and selection state
- history over transactions plus selection snapshots
- editing primitives that product commands can compose

Do not port old monolithic concepts blindly. In particular, avoid starting from a single `Surface` object that combines document state, selections, viewport, rendering, input, and product behavior. The engine should expose state primitives and operation libraries. Product packages decide how to assemble them.

## Rendering philosophy

Rendering should not live in the engine as DOM, CSS, React, terminal drawing, canvas drawing, widgets, or a fixed editor shell.

The engine should eventually support rendering by producing snapshot-based layout information from editor state. The renderer/product layer owns the actual presentation.

The intended split is:

```txt
engine
  owns document snapshots, positions, line/chunk extraction, selection/history state,
  and eventually generic layout/positioning calculations

product renderer
  owns DOM/TUI/canvas/React/Solid/etc, CSS, components, colors, widgets,
  gutter visuals, diagnostic UI, completion UI, and event capture
```

A future layout API should be functional and snapshot-based: given the same document snapshot, viewport request, and anchored layout inputs, it returns the same positioned output. It should not mutate renderer state or call into UI code.

The key abstraction should be generic geometry, not product semantics. The engine should not know what a diagnostic underline, breakpoint marker, clanker annotation, blame hint, or autocomplete widget is. It may know generic anchor kinds such as:

- range anchor: an opaque payload attached to a text range
- line anchor: an opaque payload attached to a document line
- block anchor: an opaque payload that reserves rows before/after a line
- inline/overlay anchor: an opaque payload attached to an offset, possibly with declared size

A rough future shape could look like this:

```ts
type ViewportLayoutRequest<Payload = unknown> = {
  readonly document: TextDocumentSnapshot;
  readonly startLine: number;
  readonly lineCount: number;
  readonly tabSize: number;
  readonly wrap: "none"; // later: soft wrap policy
  readonly anchors: readonly LayoutAnchor<Payload>[];
};

type LayoutAnchor<Payload = unknown> =
  | {
      readonly kind: "range";
      readonly id: string;
      readonly from: number;
      readonly to: number;
      readonly payload: Payload;
    }
  | {
      readonly kind: "line";
      readonly id: string;
      readonly line: number;
      readonly payload: Payload;
    }
  | {
      readonly kind: "block";
      readonly id: string;
      readonly afterLine: number;
      readonly heightRows: number;
      readonly payload: Payload;
    }
  | {
      readonly kind: "overlay";
      readonly id: string;
      readonly at: number;
      readonly payload: Payload;
    };
```

The corresponding output should be positioning data, not UI elements:

```ts
type ViewportLayoutSnapshot<Payload = unknown> = {
  readonly documentVersion: TextDocumentVersion;
  readonly rows: readonly LayoutRow[];
  readonly placements: readonly LayoutPlacement<Payload>[];
};

type LayoutRow =
  | {
      readonly kind: "text";
      readonly row: number;
      readonly line: number;
      readonly from: number;
      readonly to: number;
      readonly chunks: readonly VisibleChunk[];
    }
  | {
      readonly kind: "block";
      readonly row: number;
      readonly afterLine: number;
      readonly heightRows: number;
    };

type LayoutPlacement<Payload = unknown> =
  | {
      readonly kind: "range";
      readonly id: string;
      readonly row: number;
      readonly line: number;
      readonly from: number;
      readonly to: number;
      readonly columnStart: number;
      readonly columnEnd: number;
      readonly payload: Payload;
    }
  | {
      readonly kind: "line" | "block" | "overlay";
      readonly id: string;
      readonly row: number;
      readonly payload: Payload;
    };
```

A diagnostics package might supply anchors like this:

```ts
const anchors: LayoutAnchor[] = [
  {
    kind: "range",
    id: "diag:underline:1",
    from: diagnostic.from,
    to: diagnostic.to,
    payload: { type: "diagnosticUnderline", severity: "error" },
  },
  {
    kind: "line",
    id: "diag:gutter:1",
    line: diagnostic.line,
    payload: { type: "diagnosticMarker", severity: "error" },
  },
  {
    kind: "block",
    id: "diag:message:1",
    afterLine: diagnostic.line,
    heightRows: 1,
    payload: { type: "diagnosticMessage", message: diagnostic.message },
  },
];
```

The engine maps these anchors to visible rows, document ranges, and columns. The payload remains product-owned. Renderers decide whether a payload becomes CSS, a TSX component, terminal cells, canvas marks, gutters, popovers, or nothing:

```tsx
function DiagnosticPlacement({ placement }: { placement: LayoutPlacement }) {
  if (placement.payload?.type === "diagnosticMessage") {
    return <DiagnosticBlock message={placement.payload.message} />;
  }

  return null;
}
```

This keeps Monaco-style editor behavior from becoming hardcoded into the engine. The default product can be opinionated, but it should be opinionated through packages and renderers, not because the engine has a fixed shell.

Do not start by building a full IDE. Start by making typing and movement correct, local, and fast.

---

## Extension direction

The Editor should feel more like an operating system than an application.

Packages should be able to replace or extend most product behavior. The default editor is not special; it is just the default package set.

Examples of things packages should eventually control:

- command palette
- file tree
- statusline
- tab strip
- editor modes
- keymaps
- diagnostics UI
- gutters
- LSP wiring
- syntax parsers
- tools and panels
- agents
- terminals
- browser panes
- workspace behavior

The architecture should prefer public package-facing seams over hidden app internals. But seams should only exist when they provide leverage. Do not invent generic extension points before the host module knows what it owns.

---

## Module design rules

A good module is deep: small interface, meaningful behavior, strong ownership.

Use the deletion test:

- If deleting a module makes complexity disappear, it was probably shallow.
- If deleting a module spreads complexity across many callers, it was earning its keep.

Avoid:

- compatibility shims
- pass-through services
- UI render code owning lifecycle truth
- invented entities that exist only to force UI state
- global mutable bags
- generic abstractions with one adapter
- background work in the input path

Prefer:

- explicit ownership
- synchronous local hot paths
- package-owned contributions
- deterministic reload/disposal
- small public interfaces
- tests at module seams

---

## Current repo expectation

At this stage, the repo should stay boring.

If a package does not support the kernel/package/command spine or a deliberately chosen new module, it probably should not exist yet.
