# Secondary Click Menus Plan

## Purpose

Add native-feeling secondary-click menus to:

- the Swift file tree sidebar
- the Swift editor buffer surface

without breaking the current architecture:

- Rust owns semantic editor/file state and shared action meaning
- clients own native menu presentation and OS integration
- behavior should stay portable enough that `the-term` can eventually expose the same actions through a different UI

This document treats the feature as **secondary-click / context menu** support, not text double-click selection behavior.

## Why This Fits the Existing Vision

This plan follows the direction already laid out in:

- [docs/MOUSE_NAVIGATION_PLAN.md](/Users/misterclayt0n/code/the-editor/docs/MOUSE_NAVIGATION_PLAN.md)
- [docs/SWIFT_FILE_TREE_PLAN.md](/Users/misterclayt0n/code/the-editor/docs/SWIFT_FILE_TREE_PLAN.md)
- [docs/UI_INTENT_MODEL.md](/Users/misterclayt0n/code/the-editor/docs/UI_INTENT_MODEL.md)
- [docs/SWIFT_RENDER_SURFACE_REFACTOR_PLAN.md](/Users/misterclayt0n/code/the-editor/docs/SWIFT_RENDER_SURFACE_REFACTOR_PLAN.md)

The important constraints are:

- `the-lib` and `the-default` should own semantic actions, not AppKit menu wiring.
- Swift should use real native menus (`NSMenu`), not fake popup panels.
- We should not hard-code all business logic in Swift just because the menu is native.
- We should not try to force context menus into the text render plan.

## Design Principles

1. Keep menus smaller than Xcode's, but more editor-aware than Finder's.
2. Prefer context-sensitive menus over one giant static list.
3. Use shared action ids and enablement rules for app-defined actions.
4. Allow Swift-local OS integration actions where that is the right boundary.
5. Ship Swift-first, but do not design ourselves into a Swift-only corner.

## Naming

Use `secondary click` in code/docs, not `2 click`.

That matches AppKit terminology and avoids confusion with double-click selection semantics.

## UX Recommendation

We should copy the **shape** of Xcode/Zed, not the full menu contents.

Recommended direction:

- file tree menu: file-management oriented, split-aware, repo-aware
- editor buffer menu: symbol-aware, LSP-aware, edit-aware

We should **not** clone Xcode's generic macOS items like `Services`, help, or broad project-system actions into our shared model.

Those stay client-local if the platform wants them.

## File Tree Menu

### Behavior Rules

- Secondary-click on a file tree row should select that row first, then open the menu.
- Secondary-click on an already selected row should preserve selection.
- Secondary-click should never trigger open.
- Directory rows should get directory-oriented actions.
- File rows should get file-oriented actions.

### Recommended Menu Sections

#### Open

For files:

- `Open`
- `Open in Split Right`
- `Open in Split Down`
- `Open in Default App`
- `Reveal in Finder`

For directories:

- `Expand` or `Collapse`
- `Reveal in Finder`
- `Open in Terminal Here`

#### Create

For directories, and optionally the parent directory of a file:

- `New File...`
- `New Folder...`

#### Paths

- `Copy Path`
- `Copy Relative Path`

#### File Operations

- `Rename...`
- `Move to Trash`

#### Repo / History

Only when the node is file-backed and inside a repo:

- `View File History`
- `Copy Permalink`

### What To Exclude Initially

- `Open As`
- `Open in New Native Window Tab`
- `Add Files to Project`
- generic `Source Control` submenu
- `Bookmark`
- permanent delete as a first-class primary action

Those either do not fit this editor, are too Xcode-specific, or require more shared infrastructure than the first pass should take on.

### Recommended MVP File Tree Set

This is the smallest good first menu:

- `Open`
- `Open in Split Right`
- `Open in Split Down`
- `Reveal in Finder`
- `Open in Default App`
- `Open in Terminal Here`
- `New File...`
- `New Folder...`
- `Rename...`
- `Copy Path`
- `Copy Relative Path`
- `Move to Trash`

## Editor Buffer Menu

### Behavior Rules

- Secondary-click inside an existing selection should keep that selection.
- Secondary-click outside the current selection should use the clicked buffer position as the context anchor.
- The menu should be built from the clicked pane/buffer, not from stale active-buffer state.
- Symbol/LSP actions should only appear when the clicked position supports them.

### Recommended Menu Sections

#### Navigation

When a symbol/actionable position is available:

- `Go to Definition`
- `Go to Type Definition`
- `Go to Implementation`
- `Find References`

#### Symbol / Code Actions

When the active server supports them:

- `Rename Symbol...`
- `Show Code Actions`
- `Format Buffer`

#### Edit

- `Cut`
- `Copy`
- `Paste`

Optional later:

- `Select All`
- `Duplicate Selection`

#### File / Workspace

For file-backed buffers:

- `Reveal in Finder`
- `Open Buffer Directory in Terminal`
- `Copy Path`
- `Copy Relative Path`

#### Repo / History

When repo/VCS context exists:

- `View File History`
- `Copy Permalink`
- `Show Last Change for Line`

### What To Exclude Initially

- bookmark actions
- autofill / services / spelling
- broad app-global actions that do not depend on click context
- line-history / blame items before we have a shared VCS query path for them

### Recommended MVP Buffer Set

This is the smallest strong first menu:

- `Go to Definition`
- `Go to Type Definition`
- `Go to Implementation`
- `Find References`
- `Rename Symbol...`
- `Show Code Actions`
- `Format Buffer`
- `Cut`
- `Copy`
- `Paste`
- `Reveal in Finder`
- `Open Buffer Directory in Terminal`
- `Copy Path`
- `Copy Relative Path`

## Shared Semantic Shape

We should not model this as "Swift builds a random menu and calls random methods".

The better fit is a small shared action snapshot.

### Suggested Model

```rust
enum ContextMenuTarget {
  PathItem { path: PathBuf, is_directory: bool },
  EditorBuffer {
    pane_id: u64,
    logical_row: u16,
    logical_col: u16,
    char_idx: usize,
  },
}

struct ContextMenuSnapshot {
  sections: Vec<ContextMenuSection>,
}

struct ContextMenuSection {
  title: Option<String>,
  items: Vec<ContextMenuItem>,
}

struct ContextMenuItem {
  id: ContextMenuActionId,
  title: String,
  enabled: bool,
  destructive: bool,
}
```

### Why This Shape

- Rust can decide what actions make sense in the current semantic context.
- Swift can render a native `NSMenu` with separators, destructive styling, and OS polish.
- `the-term` can later expose the same snapshot through a popup, command palette, or inline chooser.

## Action Ownership Split

Not every action belongs on the same side of the boundary.

### Shared semantic actions (Rust-owned)

- open in active pane
- open in split right/down
- rename symbol
- go to definition/type/implementation
- references
- code actions
- format buffer
- new file / new folder
- rename path
- trash path

### Client-local integration actions (Swift-owned, using shared state)

- reveal in Finder
- open in default app
- open directory in Terminal.app / configured external terminal
- copy to macOS pasteboard

These actions still need shared context data like active file path, selected tree node path, workspace root, or current line.

## Implementation Plan

### Phase 0: Design Lock

Decide the first menu contents and naming.

Exit criteria:

- this doc is accepted as the action inventory
- `secondary click` terminology is used consistently

### Phase 1: Shared Action IDs + Snapshot Builder

Add shared menu action ids and snapshot builders in Rust for:

- file tree node context
- editor buffer context

Add action execution entry points that dispatch by action id.

Exit criteria:

- Swift can ask Rust for a context menu snapshot
- Swift can invoke a selected app-defined action by id

### Phase 2: Swift File Tree Secondary Click

Implement file tree row context menus in the AppKit outline view.

Suggested path:

- select row on secondary-click
- ask Rust for a file-tree menu snapshot
- append Swift-local OS actions where appropriate
- execute selected action

Exit criteria:

- file tree rows show the new native menu
- open/split/path actions work end-to-end

### Phase 3: Swift Editor Buffer Secondary Click

Implement editor-surface context menus from the host view, not from SwiftUI text overlays.

Suggested path:

- right-click hit-tests to pane-local logical position
- if click is outside selection, create a transient context anchor
- ask Rust for an editor menu snapshot
- render native `NSMenu`
- execute selected action and refresh

Exit criteria:

- buffer secondary-click works in split panes
- symbol actions reflect the clicked position

### Phase 4: File Mutation + Repo Actions

Add missing shared operations needed for the better menus:

- create file/folder
- rename path
- trash path
- file history query
- permalink generation
- line blame / last change

Exit criteria:

- menu items do not exist as dead placeholders
- file tree and buffer menus feel coherent instead of half-finished

### Phase 5: Terminal / Multi-Client Convergence

Do not block Swift on this, but keep the shared action model portable.

Possible terminal parity paths:

- command palette-style context action picker
- mouse popup in supporting terminals
- keyboard-only "context actions" command on current file/tree selection

Exit criteria:

- shared snapshot/action model is not Swift-only

## Recommended Shipping Order

If we want the highest value with the least architecture risk:

1. File tree secondary-click menu
2. Editor buffer secondary-click menu
3. Shared file mutation actions
4. Repo/history actions

That order matches the current codebase:

- file tree already has native AppKit ownership
- editor buffer already has pane-aware hit testing
- file mutation and VCS actions are the missing semantic layer, not the menu shell

## Acceptance Criteria

1. Swift file tree rows expose a native context menu.
2. Swift editor buffers expose a native context menu at the clicked position.
3. Menu contents are context-sensitive, not giant static lists.
4. App-defined actions come from shared semantic snapshots, not ad hoc Swift conditionals.
5. OS integration actions remain native and platform-appropriate.
6. The design remains compatible with future `the-term` parity.
