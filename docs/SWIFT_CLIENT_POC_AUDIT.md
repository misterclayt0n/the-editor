# Swift Client POC Audit

## What exists today

### Strong parts
- `the-lib` is already a deterministic editor core.
- `the-default` is already the shared composition / command / input layer.
- `the-term` is already proof that a client can compose shared logic into its own renderer.
- `the-lib::render::RenderPlan` / `FrameRenderPlan` already carries generations and damage reasons.

### Gaps
- There is **no current `the-swift/` client in this checkout**.
- There is **no current `the-ffi/` crate in this checkout**, even though multiple docs reference one.
- `the-term::Ctx` is monolithic and owns a lot of runtime concerns directly, which makes it a poor long-term cross-client host type.
- Some older docs still describe a Rust-owned Metal renderer in core; that conflicts with the newer display-list + dumb-client direction and with the current code reality.

## Architectural conclusion

For this project's current direction, the right Swift-client architecture is:

- Rust owns editor state, commands, layout, and render snapshots.
- Swift owns the native surface and platform rendering/input.
- A **thin FFI layer** bridges the two.
- The FFI layer should **not** become a renderer.

That keeps the architecture aligned with:
- `the-default` as the shared composition layer
- `the-term` as one renderer/input host
- `the-swift` as another renderer/input host

## Why FFI is the right POC move

For a same-process native macOS client, FFI is simpler and lower-latency than spinning up a sidecar process.

For the POC, the bridge can stay intentionally simple:
- opaque editor handle
- viewport updates
- key input
- scroll input
- render snapshot export

## What this POC implements

### Rust
- New `the-ffi` crate.
- Minimal `DefaultContext` host around:
  - `the-lib::Editor`
  - `the-default` key handling / command dispatch
- Thin C ABI:
  - create/free editor
  - open file
  - set viewport
  - handle key
  - scroll lines
  - export snapshot JSON

### Swift
- New `the-swift/` Swift package.
- Local binary target packaging pattern via `TheEditorFFI.xcframework`.
- AppKit-backed `NSView` hosted from SwiftUI.
- Basic renderer:
  - gutter text
  - buffer text
  - selections
  - cursors
- Basic input:
  - keyboard
  - native focus
  - mouse-wheel scrolling

## Why this is only a POC

This POC intentionally does **not** solve yet:
- IME / marked text
- rich mouse editing semantics
- LSP/runtime/file watching parity with `the-term`
- syntax highlighting over FFI
- retained scene caching on the Swift side
- Metal rendering
- multi-pane / multi-surface composition

## Recommended next steps after POC validation

1. Extract a reusable runtime host from `the-term::Ctx` so both `the-term` and `the-ffi` share more logic.
2. Replace JSON snapshot export with bulk, typed snapshot APIs.
3. Add pointer-event FFI aligned with `docs/MOUSE_NAVIGATION_PLAN.md`.
4. Add explicit surface/frame generations to the exported FFI contract.
5. Move the Swift renderer from AppKit text drawing to a native surface controller architecture.
6. Bring in AppKit first-responder + IME correctness before any serious editor shipping work.
7. Only then consider Metal.

## Build shape inspiration taken from Ghostty

The packaging choice here intentionally follows the same broad shape as Ghostty's macOS app:
- native library packaged as an `.xcframework`
- Swift imports that framework as a module
- SwiftUI hosts a native view

The important lesson copied from Ghostty is the **boundary**, not the terminal-specific internals.
