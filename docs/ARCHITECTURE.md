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
Kernel
  package lifecycle, registries, stores, ownership

Commands
  command contributions and execution

Keymaps
  global/contextual input routing and modal composition

Text document
  text storage, edits, selections, undo/redo, line mapping

Working copy
  URI-backed document lifecycle: open, dirty, save, revert, conflict

Editor surface
  viewport, scrolling, cursor/selection rendering, gutters, decorations, input capture

Workbench session
  panes, tabs/items, splits, focus routing, active item, layout

Workspace/files/resources
  roots, filesystem operations, watching, snapshots

LSP/language workers
  background language intelligence; never blocks typing

VCS/diff workers
  background metadata and diff computation; never blocks typing
```

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
  -> editor surface schedules paint
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

## Text engine direction

The editor should own its text engine.

The first real rebuild target should be a deep `text` module with a small interface and serious behavior behind it:

- document creation from text
- line access
- offset/position mapping
- transactions
- multi-selection support
- undo/redo
- change ranges
- decoration anchoring
- diagnostic range anchoring

Then build an editor surface on top:

- virtualized visible lines
- native scrolling
- cursor rendering
- selection rendering
- gutters
- decorations
- basic input capture
- command hooks

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
