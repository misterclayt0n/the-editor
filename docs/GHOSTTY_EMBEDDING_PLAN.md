# Ghostty Embedding Implementation Plan

## Summary
Embed Ghostty as an integrated terminal pane in `the-editor` while preserving current architecture:

1. `the-lib` stays deterministic and renderer/platform agnostic.
2. `the-default` owns shared command semantics, not platform UI/runtime details.
3. Clients render natively:
   - `the-swift`: Ghostty-backed terminal panes.
   - `the-term`: non-Ghostty fallback (placeholder first, optional richer backend later).

Recommended direction: do not add a new crate right now. Implement terminal pane semantics in existing Rust crates (`the-lib`, `the-default`, `the-ffi`) and keep Ghostty runtime/linking isolated to `the-swift`.

## Why This Direction

### Architectural fit
- Existing docs establish a display-list + dumb-client direction and keep platform concerns out of `the-lib`.
- Existing code paths in FFI/Swift/term assume pane rendering is client-driven from frame snapshots.
- Ghostty is platform/runtime-heavy and should not be linked into core editor state.

### Practical fit
- `cmux` already proves Ghostty can be embedded in SwiftUI/AppKit via:
  - app singleton (`ghostty_app_t` lifecycle),
  - per-surface objects (`ghostty_surface_t` lifecycle),
  - `NSViewRepresentable` host composition.
- Reusing that shape in `the-swift` minimizes risk.

## Scope

### Goals
- Add terminal pane support to the shared pane model.
- Run Ghostty inside Swift pane(s), embedded in the split layout.
- Keep command semantics shared across clients.
- Preserve existing text-pane behavior and rendering.

### Non-goals (initial rollout)
- No Ghostty integration in `the-term` (terminal client).
- No refactor of core renderer to Ghostty/state-driven rendering.
- No replacement of existing text editing/render pipeline.
- No new Rust crate dedicated to terminal integration in this phase.

## Crate and Module Strategy

### Naming and structure
Current direction is to avoid adding a new crate.

- Keep the existing crate layout.
- Add terminal-pane core semantics to existing crates only.
- Keep Ghostty runtime code entirely in `the-swift`.
- Keep `the-ffi` as a thin bridge with no Ghostty runtime ownership.

### Proposed responsibilities
- `the-lib`: pane content model + deterministic state transitions.
- `the-default`: shared terminal command semantics + context hooks.
- `the-ffi`: frame snapshot extensions for pane kind + terminal metadata; no Ghostty linking.
- `the-swift`: Ghostty runtime/surface embedding and pane composition (all Ghostty-specific code lives here).
- `the-term`: fallback rendering/behavior for terminal panes.

## Data Model Changes (Core)

### 1) Pane content model
Replace buffer-only pane mapping with explicit pane content.

### New core types (in `the-lib`)
```rust
pub enum PaneContentKind {
  EditorBuffer,
  Terminal,
}

pub enum PaneContent {
  EditorBuffer { buffer_index: usize },
  Terminal { terminal_id: TerminalId },
}

pub struct TerminalId(NonZeroUsize);
```

### Editor state changes
- Replace `pane_buffers: BTreeMap<PaneId, usize>` with `pane_content: BTreeMap<PaneId, PaneContent>`.
- Keep active pane mechanics unchanged.
- Add deterministic operations:
  - `open_terminal_in_active_pane() -> TerminalId`
  - `replace_active_pane_with_terminal() -> TerminalId`
  - `close_terminal_in_active_pane() -> bool`
  - `set_active_buffer_in_pane(...)` for text panes

### Invariants
- Every pane always has exactly one `PaneContent`.
- At least one pane exists and remains active.
- Split/close/rotate operations preserve content deterministically.

### 2) Frame snapshot contract
Extend pane snapshots to expose content kind and identity.

### `the-lib::render::FrameRenderPlan`
Add pane kind metadata:
- `pane_kind: PaneContentKind`
- `terminal_id: Option<TerminalId>` (for terminal panes)
- keep `plan` populated only for text panes (terminal panes may carry empty plan for compatibility)

This avoids forcing Ghostty output into text render plans.

## FFI Contract Changes

### 1) Extend `RenderFramePane` in `the-ffi`
Add:
- `pane_kind() -> u8` (0=text, 1=terminal)
- `terminal_id() -> u64` (0 if none)

Keep existing methods (`plan`, `rect`, `is_active`, `pane_id`) for compatibility.

### 2) Backward compatibility
- Existing Swift and term code can keep rendering text panes unchanged.
- Terminal panes can be feature-gated and initially skipped/placeholder-rendered.
- `the-ffi` remains platform-agnostic; it only exports metadata and actions for pane kinds.

## Shared Command Semantics (`the-default`)

### 1) New commands
Add command variants and mappings:
- `terminal_open`
- `terminal_open_split` (optional in phase 1)
- `terminal_close`
- `terminal_focus_next` / `terminal_focus_prev` (optional)
- `terminal_send_text` (optional, client-routed)

### 2) `DefaultContext` hooks
Add default no-op hooks so both clients can implement what they support:

```rust
fn supports_embedded_terminal(&self) -> bool { false }
fn open_terminal_in_active_pane(&mut self) -> Result<(), String> { ... }
fn close_terminal_in_active_pane(&mut self) -> Result<(), String> { ... }
fn is_active_pane_terminal(&self) -> bool { false }
```

Behavior policy:
- If unsupported client: emit consistent message via message center.
- Keep command dispatch shared; do not fork command semantics per client.
- Swift client opts in to terminal commands; `the-term` can keep explicit unsupported behavior.

## Swift Client Integration (`the-swift`)

### 1) Ghostty runtime layer (Swift-only)
Create Swift components patterned after `cmux`:
- `GhosttyRuntime` (singleton app/config/callback management)
- `GhosttySurfaceController` (one `ghostty_surface_t` per terminal pane)
- `GhosttyPaneView` (`NSViewRepresentable` host)

Responsibilities:
- Initialize `ghostty_app_t` once.
- Own callback wiring (`wakeup`, `action`, clipboard, close-surface).
- Manage per-surface lifecycle (create, attach, resize, focus, occlusion, free).
- Ensure main-thread lifecycle and safe reparenting across SwiftUI updates.

### 2) Pane composition in `EditorView`
Update frame drawing path to branch by pane kind:
- Text pane: existing Canvas rendering.
- Terminal pane: mount Ghostty host view clipped to pane rect.

Input routing:
- Pointer/key/scroll routes to Ghostty when focused terminal pane is active.
- Keep existing Rust pointer pipeline for text panes.
- Focus transitions must update both Rust active pane and Ghostty surface focus.

### 3) Editor model bridge
`EditorModel` should:
- read pane kind from FFI,
- maintain `paneId -> GhosttySurfaceController`,
- create/dispose surfaces as panes appear/disappear,
- keep stable surface identity across split tree updates.

### 4) Swift UX details
- Inactive pane dimming should not obscure terminal readability excessively.
- Keep overlay UI (palette/search/pickers) above both text and terminal panes.
- Respect current split resizing and tab/window behavior.

## Terminal Client Strategy (`the-term`)

### Phase-1 fallback
For terminal panes in `the-term`:
- render placeholder panel ("Terminal pane not supported in terminal client yet"),
- keep pane borders/splits/focus semantics intact,
- allow closing/focusing terminal panes through shared commands.

### Phase-2 optional improvement
Introduce non-Ghostty fallback backend for `the-term` (PTY + parser + ratatui rendering), but do not block initial Ghostty embed on this.

## Build and Packaging

### 1) Ghostty dependency path
Mirror `cmux` approach:
- add Ghostty source as submodule or pinned external build source,
- add setup/build script producing `GhosttyKit.xcframework`,
- cache by Ghostty commit hash to reduce rebuild time.

### Binding and build approach
- Build toolchain: Zig is used to build `GhosttyKit.xcframework`.
- Runtime API: Swift uses Ghostty's C API (`ghostty.h`) via bridging/interoperability.
- No Rust bindings to Ghostty in this phase.

### 2) Feature flags
- Rust/FFI: optional `embedded-terminal` for pane-kind model/commands only (no Ghostty linkage)
- Swift build setting: `GHOSTTY_ENABLED`
- Default CI path should compile without Ghostty artifacts.

### 3) Local dev scripts
Add scripts:
- `scripts/setup_ghostty.sh`
- `scripts/build_swift_with_ghostty.sh`

Document prerequisites (`zig`, submodule init, Xcode linkage).

## Implementation Phases

### Phase 0: Design lock
Status: completed.

Deliverables:
- lock "no new crate" direction for this phase,
- lock Ghostty location (`the-swift`, not `the-ffi`),
- finalize FFI pane-kind contract,
- finalize command names and UX behavior for unsupported clients.

Exit criteria:
- design notes approved and tracked in docs/TODO.

### Phase 1: Core pane model
Status: completed.

Work:
- `the-lib` pane content enum + editor operations + tests.
- ensure split/close/rotate behavior remains deterministic.

Exit criteria:
- all existing split tests pass,
- new tests for mixed pane content pass.

### Phase 2: Render and FFI contract
Status: completed.

Work:
- extend frame pane with `pane_kind` + `terminal_id`.
- keep compatibility for existing clients.

Exit criteria:
- FFI tests validate pane metadata and active pane behavior.

### Phase 3: Default command semantics
Status: completed.

Work:
- add terminal commands + mappings.
- add `DefaultContext` hooks and default fallbacks.

Exit criteria:
- both clients compile,
- unsupported paths produce clear status messages.

### Phase 4: Swift Ghostty runtime MVP
Work:
- add runtime/surface wrappers.
- add minimal embedded terminal pane view.
- map active pane focus to Ghostty focus.

Exit criteria:
- can open terminal pane and run shell in Swift client,
- split/resize/focus works without crashes.

### Phase 5: Swift composition polish
Work:
- robust reparenting, occlusion, display-id updates, and cleanup.
- overlay layering and focus correctness.

Exit criteria:
- no flicker/black frame on split operations,
- no leaked surfaces after close/reopen cycles.

### Phase 6: `the-term` fallback
Work:
- placeholder rendering for terminal panes.
- command and focus parity.

Exit criteria:
- mixed-pane states do not break terminal client rendering/input loop.

### Phase 7: Packaging and CI
Work:
- Ghostty setup script + cache.
- CI matrix with and without Ghostty feature.

Exit criteria:
- clean build from fresh checkout with documented setup path.

## Testing Plan

### Rust (`the-lib`, `the-default`, `the-ffi`)
- Unit tests:
  - pane content transitions (buffer <-> terminal),
  - split/close with mixed content,
  - terminal command semantics.
- Snapshot/contract tests:
  - frame panes expose correct kind/id,
  - active pane id/kind consistency.

### Swift
- Lifecycle tests:
  - surface create/attach/detach/free,
  - split reparenting and focus transitions.
- Interaction tests:
  - typing, paste, scroll, selection, close behavior.
- Regression tests:
  - overlays and tab/window operations with terminal pane present.

### Manual matrix
- single pane terminal,
- mixed text + terminal split layouts,
- rapid split close/open sequences,
- window/tab switching with active terminal,
- suspend/resume and monitor/screen changes.

## Risks and Mitigations

### Risk: surface lifecycle crashes
Mitigation:
- strict main-thread lifecycle,
- nil/guard checks around in-flight callbacks,
- generation tokens for stale host rebinds.

### Risk: architecture drift into core
Mitigation:
- no Ghostty symbols in `the-lib` or `the-default`,
- no Ghostty runtime/linking in `the-ffi`,
- keep terminal engine details Swift-client local.

### Risk: client divergence
Mitigation:
- keep semantics in `the-default`,
- define explicit unsupported behavior contract for `the-term`.

### Risk: build friction
Mitigation:
- optional feature flags,
- cached GhosttyKit build artifacts,
- clear setup scripts and docs.

## Acceptance Criteria
- Swift client can open and use embedded terminal panes powered by Ghostty.
- Mixed text/terminal panes can be split, resized, focused, and closed safely.
- No new terminal-dedicated Rust crate is required for initial delivery.
- `the-lib` remains deterministic and platform agnostic.
- `the-default` owns shared terminal command semantics.
- `the-ffi` stays a thin bridge (no Ghostty runtime ownership).
- `the-term` remains functional with deterministic fallback behavior.
- Build and test paths work with Ghostty enabled and disabled.
