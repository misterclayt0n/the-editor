# The Editor — Architectural Refactor Proposal

## Purpose

This document defines a **ground-up refactor** of `the-editor`, focused on fixing the core architectural limitation:
**hard-coded behavior and static control flow**.

The goal is to transform the editor into a **fully overridable, composable dispatch system**, where *everything the editor does* is defined by replaceable handlers — without sacrificing performance or static guarantees.

This is **not** an ECS, **not** a pipeline, and **not** a plugin system bolted on top.
It is a **runtime-configurable Rust module system**.

---

## Core Insight

What we actually want is:

> *The Rust module system — but overridable.*

Most editors hard-code:

* key handling
* action execution
* cursor movement
* scrolling
* rendering triggers

This makes “modes” (vim/emacs/helix), plugins, and experimentation painful.

Instead:

* The editor is **just a bag of operations**
* Behavior is defined by **dispatchable handlers**
* Handlers can call *other handlers*
* All handlers are **compile-time generic**
* Optional **string-based dynamic access** exists on top

---

## Key Properties (Non-Negotiable)

### 1. No Central Pipeline

This is **not** a linear pipeline.

Handlers:

* are **not ordered stages**
* are **not forced to call “next”**
* can call *any other handler*
* can short-circuit, redirect, or replace behavior

This is **overridable dispatch**, not middleware.

---

### 2. Editor = Dispatch Graph

The editor core:

* does **zero interpretation**
* does **zero keymap logic**
* does **zero mode logic**

It only exposes **named dispatch points**.

Example conceptual dispatches:

* `pre_on_keypress`
* `on_keypress`
* `post_on_keypress`
* `pre_on_action`
* `on_action`
* `post_on_action`
* `move_cursor`
* `scroll_view`
* `render_request`
* `buffer_edit`
* `selection_update`

The editor never decides *what happens* — only *where calls go*.

---

### 3. Handlers Are Generic, Not Dynamic

All core handlers are:

* **generic type parameters**
* statically dispatched
* zero-cost by default

Dynamic dispatch is **optional**, layered on top.

This avoids:

* virtual calls everywhere
* trait object soup
* performance cliffs

---

### 4. Handlers Can Call Each Other

This is the critical design break from “pipelines”.

Example (conceptual):

```rust
.on_keypress(|editor, key| {
    let action = editor.dispatch.on_keymap(editor, key);
    editor.dispatch.on_action(editor, action);
})
```

There is:

* no “executor”
* no forced continuation
* no linear flow

Control flow is **entirely user-defined**. There is no implicit `ControlFlow` return type; handlers call other dispatch points explicitly.

---

### 5. Overridable at Compile Time

The **default editor** is just a configuration:

* default keymap
* default cursor logic
* default scrolling
* default rendering triggers

Modes (vim/emacs/helix):

* replace handlers
* wrap handlers
* ignore handlers

No flags. No branching logic. No `if mode ==`.

---

## The Dispatch System (`the-dispatch`)

This refactor requires extracting a **standalone library**.

### Responsibilities

* Define overridable dispatch points
* Hide generic boilerplate
* Allow static + dynamic access
* Be editor-agnostic

---

## Canonical Dispatch Structure (Key Insight)

The editor behavior is defined as **self-calling dispatch chains**, not a pipeline.

Each stage explicitly calls the next stage via `editor.dispatch.*`, and is free to:

* transform inputs
* suppress continuation
* redirect to another handler
* replace behavior entirely

### Keypress → Action chain

```rust
Dispatch::new()
  .with_pre_on_keypress(|editor, key| {
      // logging, remapping, filtering
      editor.dispatch.on_keypress(editor, key);
  })
  .with_on_keypress(|editor, key| {
      let action = key_to_action(key);
      editor.dispatch.post_on_keypress(editor, action);
  })
  .with_post_on_keypress(|editor, action| {
      editor.dispatch.pre_on_action(editor, action);
  });
```

### Action → Execution chain

```rust
Dispatch::new()
  .with_pre_on_action(|editor, action| {
      editor.dispatch.on_action(editor, action);
  })
  .with_on_action(|editor, action| {
      editor.apply(action);
      editor.dispatch.post_on_action(editor, ());
  })
  .with_post_on_action(|editor, _| {
      editor.dispatch.render(editor, ());
  });
```

---

## Rendering & Components (Data + Intent)

The editor core should provide **state + intent**, not pixel/row layout, for UI components.

**Why:**
- Clients are fundamentally different (terminal vs Swift vs other).
- Units vary (cells vs pixels), so a single layout computation in the core is brittle.
- Users should be able to override layout intent in config without binding to a specific renderer.

**Model:**
- **the-lib / the-default**: owns component *data* (query, items, selection) and *intent* (layout enum).
- **Clients**: decide layout + visuals.
- **Config**: overrides intent (e.g. `Floating`, `Bottom`, `Top`) without specifying coordinates.

Example intent override:

```rust
fn pre_render<Ctx: DefaultContext>(ctx: &mut Ctx, _unit: ()) {
  ctx.command_palette_style_mut().layout = CommandPaletteLayout::Floating;
}
```

### Optional Layout Helpers (Terminal)

We can keep **optional** layout helpers that build overlay nodes for terminal clients that want them,
but these are **not** the source of truth. Clients may ignore them entirely.

This keeps the system **composable** and **client-creative**, while preserving a usable default for
the-term.

---

# Client Architecture (macOS) + Renderer Plan

This section ties the current architecture to a Ghostty-style runtime and
documents the path from **the-term (proof of life)** to the **native macOS client**.

## Goals

- **Native-first macOS** (SwiftUI + Metal).
- **Core owns rendering** (clients are dumb).
- **Single shared data model** across clients (the-lib state machine).
- **FFI boundary is narrow and stable**.

## Ghostty Lessons (Applied)

Ghostty has:
- a **core runtime** that owns rendering and its render loop,
- **backend-specific renderers** (Metal/OpenGL/WebGL),
- a **platform host** that owns UI chrome (tabs, palette, etc),
- a **shared render state** guarded by a mutex,
- a **renderer thread** awakened by mailbox messages.

We adopt the same shape:

1) the-lib = **core state machine + render plan**
2) the-ffi = **surface/app API for SwiftUI**
3) the-client (SwiftUI) = **host UI + input events**
4) renderer = **Metal backend living inside core**

## Current Architecture (as of now)

### Core crates

- **the-core**: text, unicode, grapheme, rope utilities
- **the-lib**:
  - document/selection/transaction/history
  - syntax + highlighting
  - render::plan + render::doc_formatter
  - ViewState + RenderCache
- **the-ffi**: Rust -> Swift bridge (existing)
- **the-term**: minimal client proving the model works

### Current crates

- **the-dispatch**: dynamic overridable dispatch (behavior layer)

### Future crates

- **the-client-macos**: SwiftUI host + Metal view

## Responsibility Split (Core vs Client)

### Core (the-lib)
- Document/Editor state
- Transactions + undo/redo
- Selection + cursor mapping
- Syntax + highlight cache
- Render plan (display list)
- ViewState + RenderCache
- Renderer backend + render loop (Metal)

### Client (SwiftUI)
- Windowing, tabs, palette UI
- Input events + IME handling
- Passing surface/view handles to core
- Presenting overlays (search, palette, tabs)
- Setting theme + config values

## Recommended Approach (Ghostty-style)

### Option A (Recommended)
**Core owns render loop + Metal backend.**

This keeps clients dumb and aligns with the design goals. The SwiftUI app
hosts a view and forwards events. Core draws into a Metal layer.

### Option B (Not preferred)
**Client renders a display list in Swift.**

This moves renderer complexity into each client and conflicts with the
"render logic lives in the-lib" rule.

## Proposed Data Flow (macOS)

1) SwiftUI creates a SurfaceView (NSView or MTKView).
2) The view pointer is passed via the-ffi to the core.
3) Core owns:
   - App
   - Surface (Editor + Document + ViewState + RenderCache)
4) Client sends input + resize + scale updates to core.
5) Core updates state → render plan → render backend draws.
6) Render loop is internal (thread or CVDisplayLink-style tick).

```
SwiftUI (SurfaceView)
   ↓ input/resize/scale
the-ffi (C ABI)
   ↓
the-lib (App/Surface/Editor)
   → transactions/history
   → render::plan (display list)
   → renderer::metal (draw)
```

## UI Components (Tabs / Palette / Splits)

Follow Ghostty:

- Tabs and palette UI are **client-owned**.
- Core only exposes **commands** and **actions**.
- The client renders command palette UI and sends action events to core.

This keeps core deterministic and avoids platform-specific UI logic in Rust.

## What We Need to Add (Increment the-lib)

### 1) App + Surface Core Types

Introduce a stable "Surface" API:
- owns Editor + Document + ViewState + RenderCache
- can be created/destroyed via FFI
- has methods for input, resize, set scale, set theme

### 2) Renderer Thread (Ghostty model)

Add a render loop inside core:
- use mailbox/wakeup system
- coalesce redraws
- drive cursor blink and animations
- optionally sync to vsync

### 3) Metal Backend (Rust)

Implement a simple renderer that consumes render::plan:
- rects (backgrounds, selections)
- glyph runs (text)
- cursor (block/line/underline)

### 4) FFI Surface API

Expose minimal surface functions (mirroring Ghostty):
- create_app / destroy_app
- create_surface / destroy_surface
- set_platform_view (nsview)
- set_content_scale / set_size
- key/mouse events
- notify render needed

## Roadmap

### Phase 1 — Core Surface + Render Loop
- Define `App` + `Surface` in the-lib
- Add render loop thread + mailbox
- Add FFI entry points

### Phase 2 — Metal Backend MVP
- Implement rect + glyph rendering
- Connect render::plan output to Metal draw
- Verify with the-term

### Phase 3 — SwiftUI Host
- SwiftUI SurfaceView (like Ghostty)
- Pass NSView/scale/size to core
- Forward input events

### Phase 4 — UI Chrome
- Tabs, palette, split UI (SwiftUI)
- Connect to core action system (the-dispatch)

## Why This Fits Your Current Architecture

- the-lib already provides **pure, deterministic layout** (`render::plan`)
- the selection/cursor model is stable
- syntax highlighting cache can feed render::plan
- the-term proves end-to-end pipeline can work
- Ghostty proves Metal-in-core + SwiftUI host is production-viable

---

### Summary

The path forward is **Ghostty-style**:
Core owns rendering + render loop, client hosts the surface and handles UI.
This keeps the-lib authoritative and keeps clients dumb, while still allowing
native SwiftUI UI for tabs, palettes, and window management.

There is **no executor**.
The “flow” exists only because handlers explicitly invoke each other.

---

## Dispatch Definition Macro

A macro defines the editor’s *behavioral surface*.

```rust
use the_dispatch::define;

define! {
    Editor {
        pre_on_keypress: Key,
        on_keypress: Key,
        post_on_keypress: Action,

        pre_on_action: Action,
        on_action: Action,
        post_on_action: (),

        move_cursor: Direction,
        scroll: ScrollTarget,
        render: (),
    }
}
```

This generates:

* a dispatch struct with **generic handlers**
* typed accessors and builder overrides (`with_*`)
* no implicit control flow (handlers decide what to call)
* optional dynamic registry

---

### Generated Dispatch Structure (Conceptual)

```rust
struct EditorDispatch<
    Ctx,
    PreOnKeypressHandler, OnKeypressHandler, PostOnKeypressHandler,
    PreOnActionHandler, OnActionHandler, PostOnActionHandler,
    MoveCursorHandler, ScrollHandler, RenderHandler
> {
    pre_on_keypress: PreOnKeypressHandler,
    on_keypress: OnKeypressHandler,
    post_on_keypress: PostOnKeypressHandler,

    pre_on_action: PreOnActionHandler,
    on_action: OnActionHandler,
    post_on_action: PostOnActionHandler,

    move_cursor: MoveCursorHandler,
    scroll: ScrollHandler,
    render: RenderHandler,
    
    // Only present with feature = "dynamic-registry"
    #[cfg(feature = "dynamic-registry")]
    registry: DispatchRegistry<Ctx>,
}
```

Each field:

* is a callable (closure or fn) implementing `HandlerFn<Ctx, Input, Output>`
* receives `&mut Ctx` and the input type
* may call **any other dispatch** (via external coordination)
* owns control flow

---

### Builder-Style Composition

```rust
// EditorDispatch::new() returns a dispatch with no-op default handlers
// Each .with_* method replaces that specific handler
let dispatch = EditorDispatch::<EditorCtx, _, _, _, _, _, _, _, _>::new()
  .with_on_keypress(|ctx: &mut EditorCtx, key: Key| {
      // Handler logic here
      // Control flow is explicit - handlers call other dispatches as needed
  })
  .with_on_action(|ctx: &mut EditorCtx, action: Action| {
      ctx.apply(action);
  });
```

No pipeline. No stages. Just behavior.

---

### Optional Dynamic Access

For plugins, scripts, configuration, experimentation:

```rust
dispatch.registry_mut().set("pre_on_keypress", dyn_handler);
dispatch.registry().get("scroll");
```

This layer is:

* opt-in (feature-gated)
* isolated
* slower by design
* **never** on the hot path by default

Typed handlers may explicitly consult the dynamic registry.

---

## Keymaps Are Just Dispatch Configurations

Keymaps are **not special**.

A vim keymap is just:

```rust
.with_on_keypress(|editor, key| {
    match key {
        Key::Char('w') => editor.dispatch.move_cursor(editor, WordForward),
        _ => editor.dispatch.fallback_key(editor, key),
    }
})
```

Helix, Emacs, Kakoune:

* different handler graphs
* same editor core
* zero editor changes

---

## Editor Core After Refactor

### The Editor Does NOT

* interpret keys
* manage modes
* decide actions
* own control flow

### The Editor DOES

* hold state
* expose dispatch points
* provide operations
* guarantee consistency

---

## Transactions & Immutability (Future)

Transactions are **orthogonal**.

Best approach:

* immutable editor state
* transactional edits
* atomic swaps

This composes naturally with dispatch:

* handlers operate on snapshots
* commit happens at defined boundaries

---

## Daemon Architecture (Optional Layer)

### Core Rule

`libtheditor` **must not care** about daemons, IPC, or UI.

---

### Structure

```
libtheditor   → core logic + dispatch
thedaemon     → state sync + diffing + notifications
clients       → Qt / SwiftUI / TUI / CLI
```

* First-party clients go through the daemon
* Third-party clients may embed the lib directly
* Single-process mode is just “daemon omitted”

---

## Efficient State Diffing & Notifications (Daemon Concern)

For daemon-based clients, **full state replication is unacceptable**.

The daemon must:

* compute **structural diffs** of editor state
* send minimal patches to clients
* emit fine-grained notifications (buffer changed, cursor moved, viewport changed, etc.)

This problem is well-understood.

Modern UI frameworks already solve it.

### Key Insight

The editor state is effectively a **DOM-like tree**:

* buffers
* views
* cursors
* selections
* decorations

Frameworks like **React** and **SolidJS** already implement:

* virtual trees
* diffing algorithms
* efficient reconciliation

The daemon should **steal this idea wholesale**:

* maintain a virtual editor state
* diff snapshots after each transaction
* emit patches instead of full state
* let clients mirror state efficiently

React’s VDOM model is particularly relevant:

* immutable snapshots
* diff + patch
* minimal updates over a boundary (IPC instead of browser)

The editor core does **not** implement this.
The daemon does.

---

## Final Note

This architecture turns `the-editor` from:

> “an editor with plugins”

into:

> **a programmable text-editing runtime**

Everything else — modes, plugins, UIs, daemons — becomes configuration.
