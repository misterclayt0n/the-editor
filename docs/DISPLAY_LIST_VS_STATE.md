# Display List vs. State‑Driven Rendering (Ghostty comparison)

## Context
We now have a working SwiftUI proof‑of‑life for `the-lib` via FFI. The goal is a
native macOS client (SwiftUI + Metal later) with a Ghostty‑style architecture
(core state machine + thin native UI), but also to keep the system multi‑client
friendly (terminal, wgpu, Swift, etc.).

This document compares our current **display‑list approach** to Ghostty’s
**state‑driven renderer** model, and records a proposed direction.

---

## Ghostty in brief (what it actually does)
- **Core state + Surface model:** `src/App.zig` owns surfaces; `src/Surface.zig`
  is the unit of UI state + input + rendering orchestration. “One surface = one
  view/pane.”
- **Renderer as interface:** Ghostty has a renderer abstraction with multiple
  backends. Main ones are **OpenGL** (Linux) and **Metal** (macOS). Backends
  live under `src/renderer/metal` and `src/renderer/opengl`.
- **State‑driven rendering:** Renderers read **core render state** directly
  (`renderer/State.zig`) rather than consuming a display list.
- **Dedicated render thread:** `renderer/Thread.zig` runs a render loop,
  manages timers (cursor blink, draw interval), and pulls state snapshots.
- **macOS app:** SwiftUI shell wraps a Metal view (SwiftUI is not the renderer).

Key takeaway: Ghostty tightly couples renderer ↔ core state for maximum
performance and GPU‑level caching.

---

## Our current approach (the‑lib)
- Core is a **pure state machine** producing a **RenderPlan** (display list):
  lines/spans/cursors/selections.
- The client is **dumb**: it draws what it’s told (SwiftUI `Canvas` right now).
- We do not embed GPU details in the core; rendering is a consumer concern.

---

## Comparison

### 1) Display List (current approach)
**What it is:**
- Core emits immutable render commands / plan. Client draws them.

**Pros**
- Great for **FFI** and **multi‑client** support.
- Deterministic + testable (plan can be unit tested).
- The core fully owns visibility and layout decisions.
- Fits the “clients are dumb” architecture we want.

**Cons**
- Rebuild cost every frame (unless cached/diffed).
- Harder to do renderer‑specific GPU optimizations inside core.
- Less opportunity for deep GPU caching from core.

### 2) State‑Driven Renderer (Ghostty model)
**What it is:**
- Core exposes internal render state. Renderer reads it directly.

**Pros**
- Great for **high‑FPS**, GPU‑heavy workloads.
- Renderer can keep glyph atlases, damage regions, partial redraws, etc.
- Fewer conversions / intermediate buffers.

**Cons**
- Tighter coupling between core and renderer.
- Harder to embed or expose via FFI.
- Harder to test deterministically.

---

## Proposed decision
**Stick to the display list (RenderPlan) for now.**

Why:
- It fits our architecture goal: **dumb clients + multi‑client support**.
- It matches what we’re already building and validating in Swift.
- It keeps the-lib decoupled from platform rendering.

If performance becomes an issue later:
- Add **RenderCache + diffed display list** in the core.
- Introduce **incremental plan updates** (damage regions) without abandoning
  the display list interface.
- Optionally allow a “state‑driven” backend later if we truly need it.

---

## Implementation‑direction summary
- Keep **core → RenderPlan → client** flow.
- Keep **client in charge of render loop** (like Ghostty Surface but without
  core GPU renderer).
- Use **multiple Editors** as “surfaces” in Swift UI (tabs/splits). Each Swift
  surface owns an `EditorId` and `ViewState`.
- Maintain **pure core layout**: `the-lib::render::plan` stays deterministic and
  doesn’t depend on platform APIs.

---

## Next steps (short list)
1. **Input pipeline**: Swift → FFI → the-lib (keys, text input, mouse).
2. **View metrics contract**: cell size + font metrics set from Swift, used
   by `render::plan` in Rust.
3. **Plan stabilization**: finish minimal plan output (no gutters/diagnostics
   yet) and ensure styles are injected via inputs.
4. **Multi‑surface support**: treat each Swift editor view as a “surface”
   that owns one `EditorId` + `ViewState`.

---

## Final position
Ghostty is a **state‑driven renderer** with multiple backends (Metal + OpenGL)
that reads core state directly. Our architecture is intentionally **display
list–driven** to keep clients dumb and keep FFI viable. This is the right
choice for now, with a possible future evolution toward diffed display lists
or renderer‑side caching if performance demands it.
