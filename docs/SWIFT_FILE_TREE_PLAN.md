# Swift Native File Tree Plan

## Goal

Replace the `space e` / `space E` file-explorer workflow in `the-swift` with a native macOS sidebar file tree (Xcode-like), instead of opening the generic file picker overlay.

The file tree must follow project architecture:

- Rust owns semantic state and file-system data shaping.
- Swift renders native UI components.
- `the-term` behavior must not regress.

## Current State

- `space f` and `space F` are wired to file picker commands.
- `space e` and `space E` are currently commented in [`the-default/keymap.rs`](/Users/misterclayt0n/code/the-editor/the-default/keymap.rs).
- File browsing is currently picker-centric (`the-default/file_picker.rs`, native Swift replacement view in [`UiTreeView.swift`](/Users/misterclayt0n/code/the-editor/the-swift/Sources/TheSwift/UiTreeView.swift)).
- `EditorView` currently renders as a single editor content surface with overlays, not a persistent left sidebar.

## Target Semantics

1. `space e` => toggle/show workspace file tree sidebar.
2. `space E` => show file tree rooted to current buffer directory (or reveal current buffer inside workspace root, depending on config mode).
3. Selecting a file in the tree opens/focuses that file in the active Swift tab editor.
4. The file tree is persistent chrome, not a modal/popup picker.
5. Rust remains the source of truth for tree nodes, expansion state, and selection.

## Native Component Strategy (macOS)

To match Xcode-style behavior, use AppKit-native tree primitives embedded in SwiftUI.

Primary recommendation:

- `NSOutlineView` in `NSScrollView`, wrapped via `NSViewRepresentable`.

Why:

- Native disclosure behavior, keyboard navigation, selection visuals, and row interaction fidelity.
- Better control for large trees, context menus, drag/drop, and precise sidebar behavior.

SwiftUI-only fallback (acceptable for MVP if we need faster delivery):

- `List(_:children:)` or `OutlineGroup` with `.listStyle(.sidebar)`.

Shell/layout recommendation:

- Keep `EditorView` as editor surface, wrap top-level in a split container (`HSplitView` or `NavigationSplitView`) with a left sidebar pane.

References (Apple docs):

- https://developer.apple.com/documentation/appkit/nsoutlineview
- https://developer.apple.com/documentation/swiftui/nsviewrepresentable
- https://developer.apple.com/documentation/swiftui/list/init(_:children:rowcontent:)
- https://developer.apple.com/documentation/swiftui/outlinegroup
- https://developer.apple.com/documentation/swiftui/navigationsplitview

## Architecture Split

### Rust (`the-default` + `the-ffi`)

Add a file-tree subsystem with explicit semantic state.

Suggested state model:

- `FileTreeState`
  - `visible: bool`
  - `root: PathBuf`
  - `mode: WorkspaceRoot | CurrentBufferDirectory`
  - `selected_path: Option<PathBuf>`
  - `expanded_dirs: HashSet<PathBuf>`
  - `nodes_cache: ...` (lazy directory children cache)
  - `refresh_generation: u64`

Suggested node snapshot model:

- `FileTreeNodeSnapshot`
  - `id` (stable per path)
  - `path`
  - `name`
  - `depth`
  - `kind` (`file` or `directory`)
  - `expanded`
  - `selected`
  - `has_unloaded_children`

FFI surface (new methods on `App`):

- `file_tree_set_visible(id, visible)`
- `file_tree_toggle(id)`
- `file_tree_open_workspace_root(id)`
- `file_tree_open_current_buffer_directory(id)`
- `file_tree_set_expanded(id, path, expanded)`
- `file_tree_select_path(id, path)`
- `file_tree_open_selected(id)` (or open by path directly)
- `file_tree_snapshot(id, max_nodes)`

Command integration:

- Add `Command::FileExplorer` and `Command::FileExplorerInCurrentBufferDirectory`.
- Route them through `command.rs` to file-tree actions instead of picker actions.

### Swift (`the-swift`)

Add a native sidebar view driven by Rust snapshots.

Suggested new components:

- `FileTreeSidebarView` (SwiftUI container)
- `NativeOutlineFileTreeView` (`NSViewRepresentable` wrapper around `NSOutlineView`)
- `FileTreeViewModel` for diffing Rust snapshots to AppKit updates

Editor layout changes:

- Update `EditorView` root layout to include a left sidebar pane and editor content pane.
- Keep existing editor canvas, overlays, statusline, and split handling unchanged on the detail side.

Interaction wiring:

- Expand/collapse events call Rust (`file_tree_set_expanded`).
- Selection change calls Rust (`file_tree_select_path`).
- Double-click / Enter on file calls Rust open action.
- After open, refresh editor and tree selection.

## Scope Guardrails

1. This feature is Swift-first.
2. `the-term` must keep current behavior.
3. No migration of picker internals into Swift-only state.
4. No duplicate file-tree model in Swift.

## Rollout Plan

### Phase 0: Command + Capability Wiring

1. Add new command variants for file explorer intents.
2. Wire keymap `space e` / `space E` to these commands.
3. Add a context capability/fallback policy so non-Swift clients are unaffected.

Exit criteria:

- `space e` / `space E` dispatch to dedicated explorer actions.
- No regressions in existing picker commands (`f`/`F`).

### Phase 1: Rust File Tree State + Snapshot API

1. Implement `FileTreeState` in shared command/default layer.
2. Add lazy directory loading and expansion map.
3. Export FFI snapshot + action methods.

Exit criteria:

- Rust can produce deterministic tree snapshots and apply expansion/selection/open actions.

### Phase 2: Swift Native Sidebar Skeleton

1. Add sidebar split layout in `EditorView`.
2. Implement `NSOutlineView` wrapper.
3. Render static sample data first, then bind to Rust snapshot.

Exit criteria:

- Sidebar appears as native macOS file tree, matching platform selection/disclosure behavior.

### Phase 3: End-to-End Interaction

1. Bind command intents (`e`, `E`) to sidebar visibility/root mode.
2. Bind expand/select/open interactions between Swift and Rust.
3. Keep active-file reveal in sync after opens.

Exit criteria:

- `space e` and `space E` produce expected file-tree behavior.
- Opening files from tree updates active editor and buffer state correctly.

### Phase 4: Xcode-Like Polish

1. Sidebar spacing, row height, icon sizing, selection tint.
2. Header and optional filter field styling.
3. Divider width/behavior and collapse/expand transitions.

Exit criteria:

- Visual and interaction quality is close to Xcode sidebar ergonomics.

### Phase 5: Performance + Correctness Hardening

1. Snapshot diffing to avoid full reloads on every frame.
2. Debounced refresh for filesystem changes.
3. Large repo behavior tests.

Exit criteria:

- Smooth navigation in large workspaces.
- No stutter from full-tree rebuilds each refresh tick.

## Testing Plan

### Rust tests

1. Root selection behavior (`workspace` vs `current buffer directory`).
2. Expansion/selection invariants.
3. Snapshot stability (stable IDs/order).
4. Open action correctness (existing buffer focus vs new open semantics if applicable).

### Swift tests

1. Sidebar visibility toggle from `space e`.
2. Current-buffer-directory mode from `space E`.
3. Expand/collapse, selection, and double-click open behavior.
4. Layout regression checks with editor overlays and statusline.

### Manual QA

1. Open large workspace and traverse deep directories.
2. Open from tree repeatedly, ensure no duplicate/mismatched active file states.
3. Verify picker (`space f` / `space F`) still works independently.

## Acceptance Criteria

1. `space e` opens/toggles a native sidebar file tree in Swift.
2. `space E` opens tree in current buffer directory context.
3. Tree interactions are native-feeling and Xcode-like.
4. Rust remains source of truth for file-tree data/state.
5. No regressions in existing editor/picker behavior.
