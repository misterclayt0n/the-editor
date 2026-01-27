# the-dispatch as DLL-config (plan + roadmap)

## Goal
Make `~/.config/the-editor` a **Rust plugin crate** (DLL) that composes with
`the-dispatch` and overrides defaults **without mutating** `the-default`.

We want:
- Default behavior to stay immutable (shipped by `the-default`).
- User crate overlays that behavior at runtime.
- Client stays dumb; all policy lives in dispatch.

---

## Architecture proposal

### 1) Dispatch pipeline (composition)
Treat `the-dispatch` as a **middleware chain**:

```
UserPlugin  →  DefaultPlugin
```

The user plugin gets first shot. If it doesn’t handle, it forwards to default.
This makes overrides trivial (e.g., `j` moves down 5) without replacing the
whole system.

### 2) Plugin crate (DLL)
`~/.config/the-editor` is a Rust crate compiled as a **cdylib**.

- `the-dispatch` loads it via `libloading`.
- The plugin exposes a single C‑ABI symbol (v1) that returns a vtable.

Example (conceptual):

```rust
#[repr(C)]
pub struct DispatchPluginV1 {
    pub on_key: extern "C" fn(*mut Editor, KeyEvent) -> DispatchResult,
    pub on_mouse: extern "C" fn(*mut Editor, MouseEvent) -> DispatchResult,
    pub drop: extern "C" fn(),
}

#[no_mangle]
pub extern "C" fn the_editor_plugin_v1() -> DispatchPluginV1 { ... }
```

### 3) Defaults remain separate
`the-default` provides **DefaultPlugin** with built‑in commands + keymaps.

Dispatch composition:

```
Dispatch = UserPlugin → DefaultPlugin
```

So changes at `~/.config/the-editor` only **override** behavior, not defaults.

---

## “Do we need KeyResult?”
Maybe. There are two viable shapes for the dispatch return type:

### Option A: `DispatchResult` (explicit)
A small enum like:
- `Continue` (fallthrough)
- `Handled`
- `Emit(Action)` or `EmitMany(Vec<Action>)`

**Pros:** explicit, testable, predictable.
**Cons:** adds a new type to `the-dispatch` surface.

### Option B: “Action buffer + bool” (minimal)
Dispatch writes to a buffer and returns `bool`:
- `true` = handled, stop
- `false` = call next

**Pros:** thinner API surface.
**Cons:** less explicit, harder to compose safely.

**Recommendation:** start with a small `DispatchResult` enum. Keep it tiny so
`the-dispatch` stays reusable. We can always wrap it in helpers so users don’t
see it much (e.g., `ctx.emit(action)` and return `Handled`).

---

## Roadmap

### Phase 0 — Stabilize dispatch API
- Define the minimal `DispatchPlugin` trait or vtable.
- Define minimal event types (`KeyEvent`, `MouseEvent`, `CommandEvent`).
- Decide on result type (Option A or B above).

### Phase 1 — Default plugin
- Move current “default commands + keymaps” into `the-default`.
- Expose `DefaultPlugin` to be placed at the tail of the pipeline.

### Phase 2 — DLL loader in the-dispatch
- Add `libloading` based loader.
- Define `the_editor_plugin_v1` symbol + ABI stable vtable.
- Load plugin from `~/.config/the-editor/target/release/libthe_editor_config.*`.

### Phase 3 — Build tool
- CLI helper: `the-editor config build`.
- Builds the user crate into a DLL and places it in a known path.
- Optional: `the-editor config reload` for hot reload (later).

### Phase 4 — Runtime integration
- `the-dispatch` creates pipeline: `UserPlugin → DefaultPlugin`.
- All clients use dispatch, not raw lib calls.

### Phase 5 — Tests + safety
- ABI smoke tests (load plugin, call dispatch).
- Regression tests for event routing.
- Failure handling: if plugin load fails, fall back to defaults.

---

## End state
- `the-lib` = core state machine + display list.
- `the-dispatch` = reusable policy router with a stable plugin ABI.
- `the-default` = shipped preset that can be overridden.
- `~/.config/the-editor` = user Rust crate (DLL) that composes with defaults.

This gives:
- Customization without touching shipped defaults.
- A clean separation of policy vs state.
- A scalable path to hot reload and more advanced user config in the future.
