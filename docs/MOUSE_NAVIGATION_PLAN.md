# Mouse Navigation Implementation Plan

## Purpose

Add mouse navigation that feels native in both `the-term` and `the-swift` while staying aligned with the project architecture:

- `the-lib` / `the-default` own deterministic state transitions and UI semantics
- clients own native input capture, hit testing, layout, and visuals
- behavior remains consistent across clients

This plan is intentionally implementation-focused (what to build, in what order).

---

## Architectural Constraints (from project direction)

This plan follows the existing docs and current architecture:

- `the-lib` remains a deterministic state machine (no platform UI/event-loop logic).
- Clients render native UI (`the-term`, Swift/AppKit) and own coordinate systems.
- UI behavior should be expressed as semantic actions / dispatch, not hard-coded per-client control flow.
- Display-list / UI-intent model remains the source of truth; clients are native adapters.

This means:

- Do not push pixel hit-testing into `the-lib`.
- Do not implement Swift-only mouse semantics that diverge from `the-term`.
- Do route mouse input into shared semantic behavior as early as practical.

---

## UX Goals

Mouse support should feel:

- Native: scrolling, dragging, and hover behavior match platform expectations
- Predictable: same intent maps to same result in both clients
- Stable: no jitter, no scroll fighting, no selection flicker
- Mode-aware: normal/insert/select behavior is coherent
- Composable: overlays/pickers/popups can capture mouse without breaking editor input

---

## Scope (V1)

### In scope

- Left click to focus pane/editor and move cursor
- Mouse wheel / trackpad scroll for editor content
- Left drag selection in editor content
- Shift-click selection extension
- Pane split resize drag
- Completion/code-action popup mouse hover + click select/submit
- Picker/picker-preview mouse parity (selection, scrollbar drag, wheel) where applicable

### Out of scope (V1)

- Right-click context menu
- Cmd-click goto definition
- Option-click multi-cursor
- Drag-and-drop text/files
- Gesture-specific interactions (pinch, swipe history)
- Advanced word/line selection (`double/triple click`) unless it is trivial after V1

---

## Core Design: Two-Layer Mouse Model

### Layer 1: Client-native capture + hit testing

Each client captures native mouse input and performs local hit testing in its own coordinate space:

- `the-term`: crossterm mouse events (`the-term/input.rs`)
- `the-swift`: AppKit events from the editor host (`KeyCaptureView`, popup views, native scroll views)

Clients decide:

- raw coordinates (cells vs pixels)
- native scroll deltas
- which local surface/widget was hit
- drag capture boundaries for native UI components

### Layer 2: Shared semantic actions / dispatch

Clients convert native events into semantic editor actions:

- focus pane
- move cursor
- extend selection
- scroll view
- resize split
- select popup item
- submit popup item

These semantics should be handled in shared Rust paths (`the-default` / client ctx hooks), not reimplemented independently in Swift and terminal code.

---

## Event Routing Order (Required)

Mouse routing must be explicit and consistent. Recommended order:

1. Floating overlays/popups (completion, code actions, docs panel)
2. File picker / picker preview / picker scrollbars
3. Pane split separators (resize hit targets)
4. Editor-local overlays/gutter interactions
5. Editor text content
6. Passive UI (statusline, decorations) unless explicitly interactive

This prevents accidental editor selection changes when a popup/picker should own the event.

---

## Drag Model (State Machine)

Use drag capture state, not repeated free hit testing during drag.

### Rules

- `mouseDown` establishes capture target
- `mouseDragged` updates only the captured target
- `mouseUp` releases capture
- `mouseMoved` updates hover only (no stateful edits)

### Example capture targets

- `EditorSelection`
- `PaneResize(split_id)`
- `PickerListScrollbar`
- `PickerPreviewScrollbar`
- `CompletionList`
- `CompletionDocsScroll`

This mirrors the good parts of the existing `the-term/input.rs` approach and avoids jitter.

---

## Shared Semantic API (Recommended)

Add a small pointer event interface at the dispatch/context layer (names illustrative):

```rust
pointer_down(event)
pointer_drag(event)
pointer_up(event)
pointer_move(event)
pointer_scroll(event)
```

### Pointer event payload (conceptual)

```rust
struct PointerEvent {
  surface_id: Option<SurfaceId>,   // client/editor surface identity
  x: i32,                          // client-local coordinates (pixels or cells)
  y: i32,
  logical_col: Option<u16>,        // optional normalized coordinates for editor text
  logical_row: Option<u16>,
  button: Option<PointerButton>,   // left/right/middle
  phase: PointerPhase,             // down/drag/up/move/scroll
  click_count: u8,                 // 1/2/3...
  modifiers: PointerModifiers,     // shift/cmd/alt/ctrl
  scroll_x: f32,                   // for wheel/trackpad
  scroll_y: f32,
}
```

Notes:

- Clients can provide both raw and normalized coordinates.
- `the-lib` should consume logical/editor-space coordinates where possible.
- Popup/picker native widgets may short-circuit without going through this path when they are fully native and self-contained.

---

## Client-Specific Strategy

## `the-term`

`the-term` already has a solid mouse routing baseline in `the-term/input.rs`.

Plan:

- Keep current behavior as the semantic reference implementation.
- Gradually refactor terminal mouse handling to call shared pointer semantics (instead of duplicating logic).
- Preserve current drag-state approach for picker scrollbars and pane resizing.

Short-term goal:

- No behavior regressions while extracting shared semantics.

## `the-swift`

Swift must be native-first:

- AppKit handles scroll views (`NSScrollView`) for pickers/preview/native lists
- Swift/AppKit views own pixel hit testing and hover
- Rust receives semantic actions / normalized editor interactions

Plan:

- Use `KeyCaptureView` / editor host view for editor content mouse events
- Keep native scroll ownership for `NSScrollView`-backed components
- Only send semantic scroll updates to Rust for editor text/canvas scroll (not native picker scroll surfaces)
- Reuse popup-local mouse handling where it already exists (completion/code-actions)

Important:

- Coalesce `mouseMoved` / `mouseDragged` before crossing FFI when possible (send only on cell/target change).

---

## Phased Implementation Plan

## Phase 0: Semantics Audit + Test Matrix

Document current mouse semantics from `the-term` and define expected behavior for Swift parity.

Deliverables:

- Behavior matrix for:
  - click cursor move
  - drag selection
  - wheel scroll
  - split resize drag
  - picker interactions
  - completion/code action popup click behavior
- Manual repro checklist for both clients

Success criteria:

- Clear reference behavior exists before refactors begin

## Phase 1: Shared Pointer Semantics Surface (Rust)

Introduce shared pointer event hooks in `the-default` / `DefaultContext` (or equivalent dispatch entry points) with no-op defaults.

Deliverables:

- New semantic pointer event types
- Default dispatch entry points
- Minimal routing hooks in shared command/input layer

Success criteria:

- Clients can call shared pointer handlers without platform-specific dependencies in `the-lib`

## Phase 2: Terminal Adapter Refactor (Behavior-Preserving)

Refactor `the-term/input.rs` to map crossterm events into the shared pointer semantics where applicable.

Deliverables:

- Terminal mouse adapter -> shared pointer events
- Existing tests preserved / expanded
- No regressions in picker/split resize/editor interactions

Success criteria:

- `the-term` behavior matches pre-refactor behavior

## Phase 3: FFI Mouse Bridge for Swift Editor Surfaces

Expose mouse event entrypoints in `the-ffi` for editor-surface semantics.

Deliverables:

- FFI functions for pointer down/drag/up/move/scroll (or a consolidated event function)
- Modifier/button/click-count encoding
- Coordinate conversion contract (pixels + optional logical cell position)

Success criteria:

- Swift can send editor mouse events into shared Rust semantics

## Phase 4: Swift Editor Content Mouse (V1 UX)

Implement editor-content mouse navigation in `the-swift` using the FFI bridge.

Deliverables:

- Click-to-focus + click-to-move-cursor
- Drag selection
- Shift-click extend
- Wheel scroll editor content

Performance requirements:

- Coalesce high-frequency move/drag events
- No visible flicker from over-publishing state

Success criteria:

- Swift editor content mouse feels stable and matches terminal semantics

## Phase 5: Overlay/Popup/Picker Parity Pass

Unify mouse semantics for popups and picker interactions across clients where they are not already correct.

Deliverables:

- Completion/code-action popup click/hover/submit parity
- Picker list click/hover behavior parity
- Picker scrollbar drag/wheel parity (where component is not fully native)

Success criteria:

- Overlay/picker mouse interactions behave consistently and do not leak events to editor content

## Phase 6: Polish + Instrumentation

Add targeted instrumentation and UX tuning after the baseline works.

Deliverables:

- Debug counters for pointer event rate, coalescing, and state applies
- Configurable thresholds (if needed) for move/drag throttling in Swift
- UX tuning for drag hysteresis / hover updates

Success criteria:

- Hot paths are measurable and tunable

---

## Behavior Rules (to lock down early)

### Click semantics

- Left click in editor content:
  - focus pane/editor
  - move primary cursor
  - clear transient hover-only state

- Shift + left click:
  - extend primary selection from anchor

### Drag semantics

- Left drag from editor content:
  - create/extend selection continuously
  - maintain drag capture until mouse up

### Scroll semantics

- Editor content scroll:
  - scroll viewport without fighting selection
  - preserve cursor/selection unless explicitly configured otherwise

- Native scroll surfaces (`NSScrollView`):
  - native scroll authority owns motion
  - Rust receives window/selection updates, not synthetic scroll corrections

### Popup semantics

- Hover updates selection
- Single click selects
- Click selected row submits
- `Enter` submits selected row
- Popup consumes mouse events while active

---

## Testing Strategy

## Automated (Rust / terminal)

- Extend `the-term/input.rs` tests for mouse scenarios already covered there
- Add tests for routing precedence (popup > picker > editor)
- Add tests for drag capture state transitions

## Manual (Swift + terminal)

Required scenarios:

- Click/drag selection in editor content
- Wheel scroll editor content
- Resize split drag
- Completion popup click + enter
- Code action popup click + enter
- Picker list click/scroll/scrollbar drag

Test on:

- large file
- wrapped lines (if enabled)
- active LSP diagnostics + overlays

---

## Open Design Decisions

1. Where to normalize coordinates

- Option A: Swift computes logical cell coordinates and sends both raw+logical
- Option B: Rust computes logical coordinates from raw pixels + metrics

Recommendation:

- Send both when possible. Keep client-side hit testing native, but give Rust enough normalized data to avoid duplicate mapping work.

2. Normal-mode drag semantics

- Drag should likely create a selection without implicit mode switch churn
- Exact mode transitions should be explicit and shared across clients

3. Double/triple click timing ownership

- Prefer client-native click count (`NSEvent.clickCount`, terminal approximation if available)

4. Future multi-cursor mouse modifiers

- Reserve modifier mapping now (`alt`/`option`) even if V1 does not implement it

---

## Future Enhancements (Post-V1)

- Cmd-click goto definition
- Option-click add cursor
- Double click word select / triple click line select
- Right-click context menu (including code actions)
- Drag-and-drop text/files
- Hover tooltips (symbol info) with debounce

---

## Why This Fits the Project Vision

This plan keeps the architecture clean:

- native clients stay native
- shared Rust semantics stay deterministic
- UI remains intent/data-driven
- dispatch remains the extensibility point

That is the path to a mouse experience that is both good now and maintainable as the editor grows.
