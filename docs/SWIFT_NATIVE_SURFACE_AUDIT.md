# Swift Native Surface Audit

## Current POC problems

### 1. It is not consuming `RenderPlan` faithfully
The first POC exported a lossy JSON projection:
- flattened line text
- lost per-span column data
- lost style information
- lost the distinction between content-space and gutter-space for some drawing

That caused visible correctness bugs such as:
- misaligned text
- cursor/selections entering the gutter
- inability to evolve into a retained native renderer cleanly

### 2. Input is not native enough
The current POC uses `keyDown` plus direct character extraction.
That is insufficient for:
- dead keys
- IME/preedit
- marked text
- some keyboard layout behaviors
- proper AppKit command routing

A shipping macOS editor surface must implement `NSTextInputClient` and use AppKit text input machinery.

### 3. Scrolling is not native enough
The current POC handles wheel deltas directly on the drawing view.
That is acceptable for a toy, but not for a real macOS editor.

The target architecture should use:
- `NSScrollView`
- a document/container view for content metrics
- a native editor surface view synchronized to the visible rect

### 4. Rendering is still immediate-mode AppKit text drawing
That is okay for a bootstrap POC only.
It is not the long-term path if the goal is Ghostty-grade responsiveness.

## Ghostty architecture to copy

From `~/code/ghostty` the important pieces are:

- SwiftUI shell hosts a native surface via representable.
- Native macOS surface is an `NSView` first responder.
- Surface is wrapped in a native `NSScrollView` for native scrolling behavior.
- Input is handled by the native view, including `NSTextInputClient`.
- Rendering attaches a renderer-owned layer directly to the host view.
- The renderer is driven by its own invalidation/render scheduling model.

For this project, we should copy that shape while keeping Rust display-list ownership.

## Recommended target shape for `the-swift`

- SwiftUI chrome / menus / panels
- `EditorSurfaceRepresentable`
  - `EditorSurfaceScrollView`
    - `EditorSurfaceView` (first responder, `NSTextInputClient`)
      - renderer-owned layer / Metal backend

## Recommended ownership split

### Rust
- editor state
- commands
- selection state
- layout
- visible content
- `RenderPlan` / frame generations / damage

### Swift
- focus
- IME / text services integration
- native scrolling
- native view lifecycle
- scene retention / caching
- Metal renderer implementation consuming `RenderPlan`

## Near-term next steps

1. Keep the POC only as a bootstrap path.
2. Replace lossy JSON snapshot export with structured span-oriented export.
3. Move the Swift surface to:
   - `NSScrollView`
   - custom `NSView`
   - `NSTextInputClient`
4. Stop treating SwiftUI as the renderer.
5. Introduce a retained scene layer on the Swift side keyed by Rust generations.
6. Bring up a Metal renderer after native input/scroll boundaries are correct.

## Immediate fixes already applied in this pass

- argument parsing bug fixed (`--` no longer treated as path)
- cursor drawing now respects gutter offset
- selection drawing now respects gutter offset
- line rendering now uses span positions rather than flattened line text

These improve correctness, but they do not change the fundamental conclusion:
this POC is still bootstrap code, not the target architecture.
