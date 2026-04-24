# Editor Architecture

This document is the short decision tree for the new editor architecture. It is meant to guide implementation and old-editor code porting.

## North Star

The editor is a native Rust editor engine with a TypeScript behavior/composition layer.

```txt
Rust:     fast editor capabilities, state, layout, IO-heavy services where needed
TypeScript: behavior, commands, hooks, keymaps, extensions, defaults
Client:   native presentation and painting
```

The editor should be batteries-included as composable capabilities, not as a monolithic app. A text input, full code editor, diff viewer, minimap, prompt, or coding-agent surface should all be built from the same primitives with different capabilities enabled.

## Crate/package shape

Preferred starting shape:

```txt
the-editor/
  the-stdx/      # reusable Rust/platform utilities
  the-lib/       # editor engine and native capabilities
  the-native/    # JS/TS bridge, FFI/N-API/etc.
  packages/
    editor/      # TS command/hook/extension framework
    default/     # default editor setup in TS
```

Dependency direction:

```txt
the-stdx <- the-lib <- the-native <- TypeScript packages
```

Rust core must not depend on TypeScript behavior concepts.

## Decision tree: Rust vs TypeScript vs client

### Put it in Rust when...

- it touches large editor state
- it runs per keystroke, per frame, per parse, or across many files
- it needs stable data ownership/invariants
- it is core editor math
- it should be shared by all clients

Examples:

- rope/text storage
- transactions
- selections/cursors primitives
- undo/redo engine
- search/diff primitives
- tree-sitter parsing and queries
- diagnostics storage/range mapping
- LSP protocol/session/document sync if native performance matters
- render surfaces, layout, dirty tracking, display-list deltas
- viewport/scroll/wrap/fold/visual-row mapping

### Put it in TypeScript when...

- it decides behavior or policy
- users/extensions should redefine it
- it composes Rust capabilities
- it is command/hook/keymap/default setup
- it is integration glue

Examples:

- command registry
- keybindings
- Vim/modal behavior
- hook system
- extension loading
- default editor setup
- LSP server configuration/policy/UI
- git feature policy/UI
- coding-agent/AI harness orchestration
- custom decoration/widget providers

### Put it in the client when...

- it is platform-specific presentation
- it draws pixels/cells/views
- it owns native UI resources
- it depends on AppKit/SwiftUI/Metal/web/terminal APIs

Examples:

- terminal ANSI/cell painting
- Metal/wgpu drawing
- SwiftUI/AppKit chrome
- web canvas/DOM rendering
- glyph atlas/native text objects
- platform event loop/frame loop

## Boundary rule

JS/TS decides intent. Rust performs batched native work. Clients paint retained presentation state.

Good:

```ts
native.applyTransaction(instanceId, tx)
native.renderFrame({ surfaceId, viewport, sinceGeneration })
```

Bad:

```ts
for (const line of visibleLines) {
  const text = native.getLine(buffer, line)
  const wrapped = wrapInJs(text)
  draw(wrapped)
}
```

Avoid chatty per-row/per-grapheme/per-token APIs across the Rust/JS/client boundary.

## Editor instances and capabilities

The core unit should be an editor/text instance, not a global monolithic app editor.

```ts
const input = editor.instances.create({
  kind: "text-input",
  capabilities: ["text", "history", "render"],
})

const code = editor.instances.create({
  kind: "code-editor",
  capabilities: ["text", "history", "render", "syntax", "diagnostics", "lsp"],
})
```

A small text input and a full code editor use the same underlying architecture. They differ by attached capabilities.

## Rendering decision

Rendering is split into two stages.

```txt
Rust:   retained render surfaces, layout, dirty tracking, render deltas
Client: measurement backend, retained presentation scene, native painting
TS:     decoration/widget providers and surface orchestration
```

Rust should not rebuild and export a whole world snapshot every frame. It should expose per-surface deltas:

```rust
pub struct RenderFrameDelta {
  pub surface_id: SurfaceId,
  pub generation: u64,
  pub full: bool,
  pub retained_rows: Vec<RowMove>,
  pub changed_rows: Vec<RenderRow>,
  pub removed_rows: Vec<RowId>,
  pub layers: LayerDeltaSet,
  pub damage: Damage,
}
```

Scrolling a surface must not serialize command palette, file tree, tabs, statusline, or unrelated app state.

## Porting old editor code

When porting old code, classify it first.

### Likely portable into `the-lib`

- pure text/rope utilities
- transaction machinery
- selection/cursor primitives
- syntax/tree-sitter infrastructure
- diagnostics data structures
- render layout concepts, but not full per-frame snapshot assumptions
- search/diff primitives
- reusable path/platform utilities into `the-stdx`

### Likely rewrite in TypeScript

- dispatch layer
- default command behavior
- keybindings
- Vim-like modes
- app feature policy
- default LSP/git/search UI behavior
- extension/hook system

### Likely client-specific

- terminal renderer
- SwiftUI/AppKit/Metal renderer
- UI chrome
- native window/event-loop code

## Non-negotiable invariants

- Rust does not define the editor's personality.
- TypeScript does not perform hot loops over large editor state.
- Clients do not maintain a second semantic document model.
- Rendering is retained/incremental, not full-snapshot-per-frame.
- Features like soft wrap, diagnostics, virtual lines, and syntax must not disable the incremental path.
- App/chrome snapshots are separate from render-surface snapshots.
