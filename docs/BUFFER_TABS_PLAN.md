# Buffer Tabs Plan

## Goal

Add a native-feeling buffer tab strip to both `the-term` and `the-swift` that:

- shows open buffers
- shows active buffer
- shows modified state
- supports mouse activation
- shares semantics across clients

The implementation must follow project architecture:

- shared semantic state in Rust
- native rendering/layout in each client
- no client-only buffer model divergence

## Design Principles

1. `the-lib` remains the source of truth for buffer state.
2. `the-default` (or app/dispatch layer) owns tab-strip semantic snapshots/policy.
3. Clients render native tab components (`the-term` row widget, SwiftUI/AppKit tab bar).
4. Buffer tabs are editor chrome, not text render-plan content.
5. Buffer tabs should not be faked as a generic list long-term.

## Existing Building Blocks (already in repo)

`the-lib/editor.rs` already exposes buffer metadata needed for tabs:

- `Editor::buffer_snapshot(index)`
- `Editor::buffer_snapshots_mru()`
- `BufferSnapshot { buffer_index, display_name, modified, is_active, file_path }`

This means MVP tabs do not require core buffer-model rewrites.

## Semantics (MVP)

- One tab strip per editor surface/window (not per pane)
- Tabs enumerate all open buffers in the `Editor`
- Active tab tracks `editor.active_buffer_index()`
- Clicking a tab activates that buffer in the active pane (`set_active_buffer`)
- No close button / reorder drag in MVP

## Architecture Shape

### Shared (Rust)

Add a semantic `BufferTabsSnapshot` API in the shared Rust layer (phase 1).

This snapshot is data-only and client-agnostic:

- visible flag / min-tabs policy
- tab ordering (natural or MRU)
- active tab index
- tab item metadata (title, modified, buffer index, path)

### Clients

- `the-term`: dedicated tab-row renderer (Flow-inspired), not `UiList`
- `the-swift`: native top bar component (`SwiftUI` first, optional AppKit wrapper if needed)

### Viewport / Layout

Clients must reserve top chrome space before editor content layout:

- terminal: reserve a top row
- swift: `VStack(spacing: 0)` tab bar + editor content

This avoids pointer/cursor/scroll coordinate mismatches.

## Inspirations (what to copy)

### Ghostty (macOS)

Files:

- `~/code/ghostty/macos/Sources/Features/Terminal/Window Styles/TitlebarTabsTahoeTerminalWindow.swift`
- `~/code/ghostty/macos/Sources/Features/Terminal/Window Styles/TitlebarTabsVenturaTerminalWindow.swift`

Copy:

- native component ownership
- idempotent layout/update strategy
- anti-flicker mindset

Do not copy directly:

- AppKit native tab groups as the semantic model (Ghostty tabs are window tabs; ours are editor buffers)

### Flow (terminal)

File:

- `~/code/flow/src/tui/status/tabs.zig`

Copy:

- dedicated tab widget (not generic list)
- active/inactive/focused styling
- dirty indicator
- full-bar rendering model

## Phases

### Phase 1: Shared Buffer Tab Snapshot API (Rust only)

Add a semantic snapshot type and builder in shared Rust (app/dispatch layer).

Outputs:

- `BufferTabsSnapshot`
- `BufferTabItemSnapshot`
- ordering / visibility options
- snapshot builder from `Editor` and/or `DefaultContext`

No client rendering changes yet.

### Phase 2: Viewport Insets / Chrome Reservation

Add top chrome reservation in both clients before editor content rendering.

- `the-term`: top row reserved
- `the-swift`: top component layout above canvas/content

### Phase 3: `the-term` Native Tab Bar (Flow-inspired)

- draw top tab row in terminal renderer
- active/inactive/dirty visuals
- click hit-testing to activate buffer

### Phase 4: `the-swift` Native Tab Bar (SwiftUI first)

- `BufferTabBarView` above editor canvas
- native hover/selection states
- click to activate buffer
- optional horizontal scrolling for many tabs

### Phase 5: Shared Commands + Mouse Parity

- `activate_buffer(index)` action path
- reuse existing next/previous buffer commands
- consistent click behavior between clients

### Phase 6: Theme Roles

Introduce semantic roles for tab styling, e.g.:

- `ui.buffer_tabs`
- `ui.buffer_tabs.tab.active`
- `ui.buffer_tabs.tab.inactive`
- `ui.buffer_tabs.tab.hovered`
- `ui.buffer_tabs.tab.modified`

### Phase 7 (Optional): First-Class `UiNode::BufferTabs`

Promote tabs into `UiTree` once top-chrome reservation and rendering adapters are generalized.

This should happen after MVP is stable.

## MVP Acceptance Criteria

1. Both clients render a tab strip from shared Rust snapshot data.
2. Active and modified state are visible.
3. Clicking a tab activates the corresponding buffer.
4. Editor viewport is correctly inset (no coordinate drift).
5. No client-specific buffer tab semantics diverge.

## Non-Goals (MVP)

- tab close buttons
- drag reorder
- pinning
- split-aware per-pane tab strips
- titlebar-native window tabs (Ghostty-style AppKit tab groups)

