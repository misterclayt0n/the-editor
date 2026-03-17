# Terminal File Tree Explorer Plan

## Purpose

This document captures the current assessment for implementing a real file tree
explorer in the terminal client using `the-default/file_tree.rs` as the shared
tree substrate.

The target is not "a file picker that looks like a tree".

The target is a proper split-hosted explorer surface that:

- participates in the terminal split system
- supports keyboard-driven navigation
- supports mouse-driven navigation
- can open files using the shared `OpenTarget` routing model
- is composable through the same core/default surfaces the rest of the editor
  uses
- leaves a path toward an oil.nvim-like editable file manager later

This document is informed by the explorer implementation in:

- `~/code/snacks.nvim/lua/snacks/explorer/tree.lua`
- `~/code/snacks.nvim/lua/snacks/explorer/actions.lua`
- `~/code/snacks.nvim/lua/snacks/explorer/git.lua`
- `~/code/snacks.nvim/lua/snacks/explorer/diagnostics.lua`
- `~/code/snacks.nvim/lua/snacks/explorer/watch.lua`
- `~/code/snacks.nvim/lua/snacks/picker/source/explorer.lua`
- `~/code/snacks.nvim/lua/snacks/picker/config/sources.lua`

---

## Summary

The current `the-default/file_tree.rs` is directionally correct and should be
used as the shared substrate.

The missing piece is not a new tree model. The missing piece is a real terminal
pane surface that consumes that model.

The correct implementation shape is:

- shared tree state, operations, decoration hooks, and traversal helpers in
  `the-default`
- pane routing/layout in `the-lib`
- rendering, pointer handling, prompts, watcher lifecycle, and explorer pane
  state in `the-term`

The biggest structural change required before terminal UI work is:

- panes must be able to host non-buffer client surfaces cleanly

That should be solved generically, not by faking the explorer as an editor
buffer.

---

## Key Takeaways From Snacks.nvim

### 1. The explorer is a tree/action system, not a renderer trick

The strongest part of Snacks is the separation between:

- tree state and traversal
- actions and file operations
- git/diagnostic propagation
- watcher lifecycle
- UI consumption

Even though Snacks renders the explorer through its picker framework, the
important design idea is not "explorer as picker". The important design idea is
"explorer as a structured model with reusable actions and decoration layers".

That aligns well with `the-default/file_tree.rs`.

### 2. Open state and expansion state are separate

Snacks distinguishes between:

- whether a directory should be open in the logical tree
- whether its children have been expanded/refreshed already

Our current tree model already has the same basic idea via selected/expanded
state and lazy loading, which is good.

### 3. Tree-native traversal helpers matter

Snacks relies on tree-native helpers for:

- visibility
- reveal
- open/close all
- next/prev matching node
- dirty checking

This is one of the main things our current `file_tree.rs` is still missing.

### 4. File operations are selection-driven

Snacks treats move/copy/delete as action workflows over one or more selected
nodes.

This is the biggest feature gap relative to our current file tree model. We
have typed single-node operations, but not a real multi-selection layer.

### 5. Git and diagnostics are decorations propagated through ancestors

Snacks does not bake these into the renderer. It computes status/severity over
the tree and then the UI consumes those values.

That is the right direction for us too. Our node decoration hooks are a good
start, but we still need shared helper logic or default providers for ancestor
aggregation.

### 6. Watchers should track open directories only

Snacks watches:

- git index
- directories that are currently open in visible explorer roots

and batches refresh work.

That is the right pattern for the terminal client. We should not watch the
entire filesystem tree eagerly.

### 7. Search is a mode layered onto the tree, not a separate model

Snacks uses its picker framework to transition into search/filter behavior, but
the core idea is still useful:

- browse mode and search mode are two ways of consuming the same explorer
  substrate

For us, this means the first implementation should focus on browse mode, while
leaving room for:

- filtering within the tree
- later search mode
- later editable "oil mode" using `FileTreeEditSession`

---

## Current State In `the-editor`

### What We Already Have

Shared/core:

- `the-default/file_tree.rs` now provides:
  - `FileTreeState`
  - typed `FileTreeOp`
  - node decoration types
  - row-layout helpers
  - edit-session support
  - `execute_file_tree_op`
  - `execute_file_tree_edit_patch`
- `the-default/extensions.rs` and `the-default/assembly.rs` provide:
  - file-tree node decorator hooks
  - file-tree context-menu postprocessing hooks
- `the-lib/editor.rs` provides:
  - stable pane ids
  - open routing through `OpenTarget`
  - pane neighbor queries
  - split focus/layout mutations

Terminal:

- `the-term/ctx.rs` already owns a `FileTreeState`
- active file changes already sync into the tree state
- file-tree decorators can already be supplied through the preset

### What We Do Not Have Yet

- no actual explorer pane surface in `the-term`
- no override of `supports_native_file_explorer` /
  `open_native_file_explorer`
- no tree rendering in pane content
- no explorer-specific pointer handling
- no explorer keyboard handling
- no file-tree context menu execution path
- no file-tree watcher manager

So the current system has the substrate, but not the UI surface.

---

## Architectural Recommendation

## Shared Principle

The explorer should be part of the split system, but explorer behavior itself
should remain mostly client-owned.

That means:

- `the-lib` should own pane routing/layout for non-buffer surfaces
- `the-default` should own shared file-tree state and operations
- `the-term` should own explorer rendering, pointer handling, prompts, watcher
  lifecycle, and local pane state

## Do Not Implement The Explorer As A Fake Buffer

This would be the wrong abstraction because it would:

- mix filesystem browser state with text document state
- force a fake parser/render path
- make pointer behavior and row hit-testing awkward
- blur the boundary between documents and non-document surfaces

## Add A Generic Client Surface Pane Type

The cleanest change is not `PaneContent::Explorer`.

The cleanest change is a generic pane content for client-owned surfaces, for
example:

- `PaneContentKind::ClientSurface`
- `PaneContent::ClientSurface { surface_id }`

with terminal-owned surface kinds such as:

- `Terminal`
- `Explorer`
- future diagnostics/references/outline surfaces

This avoids adding another ad hoc core enum variant for each UI surface while
still letting the split system host more than just editor buffers.

This is especially important because the repo already has one hardcoded
non-buffer pane surface (`Terminal`). The explorer is a good reason to
generalize that shape instead of adding another special case.

## Where Client Detours Make Sense

The following should remain terminal-owned:

- how explorer rows are painted
- how pointer hit zones work
- drag/hover behavior
- confirmation prompts
- inline status text formatting
- watcher scheduling
- whether to show the explorer as a left or right sidebar by default

The following should stay shared:

- tree state
- open/rename/delete/move/create semantics
- routing through `OpenTarget`
- tree decorations as data
- context menu structure
- traversal helpers

---

## Gaps In `the-default/file_tree.rs`

The current file tree substrate is good, but a proper explorer wants a few more
shared capabilities.

### 1. A Real Filter Model

Current state:

- hidden entry behavior is hardcoded
- there is no first-class hidden/ignored/include/exclude model

Needed:

- `FileTreeFilter`
- hidden toggle
- ignored toggle
- include/exclude globs
- snapshot behavior that respects that filter

This should be shared state, not terminal-only.

### 2. Visible Traversal Helpers

Needed helpers:

- `ensure_visible(path)`
- `is_visible(path)`
- `close_all(root)`
- `next_visible(path, predicate)`
- `prev_visible(path, predicate)`
- `expand_to(path)`

These are important for:

- follow-file behavior
- diagnostic/git navigation
- mouse reveal behavior
- jump-to-node and search mode

### 3. Multi-Selection / Marks

Current state:

- one selected path

Needed for real Snacks-like file operations:

- marked paths set
- toggle mark
- clear marks
- primary selection + marked set
- bulk helpers for move/copy/delete/yank

Without this, the explorer can still be useful, but it will not match the
selection-driven workflow that makes Snacks powerful.

### 4. Better Row Prefix / Tree Guide Helpers

Current row layout is enough for:

- indentation
- disclosure
- icon
- text
- badges

But a terminal explorer wants branch guides and ancestry-aware prefixes.

We should add a shared helper that can derive:

- whether a row has ancestor continuation lines
- whether it is the last visible child
- which connector glyphs to render

This keeps tree-structure rendering out of terminal ad hoc logic.

### 5. Decoration Aggregation Helpers

The decorator hooks are already in place, which is good.

What is still missing is a shared/default way to compute:

- aggregate git status on parent directories
- aggregate diagnostic severity on parent directories

This can be done by:

- new helper utilities in `the-default`, or
- default decorator providers installed by `the-default`

The important thing is to avoid recomputing that logic directly in the
renderer.

### 6. Optional Search/Reveal Helpers

Not required for the first browse-mode explorer, but useful soon:

- `find_visible_index(path)`
- `find_first(predicate)`
- `find_next(predicate)`
- `find_prev(predicate)`

These will make search mode and navigation commands cleaner.

---

## Terminal-Side Explorer Surface Requirements

## Explorer Surface State

The terminal needs a dedicated explorer pane surface state, separate from
`FileTreeState`.

That state should include at least:

- pane/surface id
- row scroll offset
- hovered row
- maybe marked drag/interaction state
- maybe local toggles if some filter state is per-surface
- maybe root behavior metadata

The shared tree state should remain the filesystem model.
The terminal surface state should remain the viewport/interaction model.

## Rendering

The terminal needs a renderer that consumes:

- `FileTreeSnapshot`
- node decorations
- row layouts

and paints:

- disclosure glyphs
- folder/file icons
- tree guides
- selected row styling
- active file styling
- badges
- severity/status lanes
- secondary text

This should be a pane render path, not an overlay.

## Keyboard Navigation

The terminal explorer should support at least:

- `j` / `k` or arrows for row navigation
- `l` / `Enter` to open file or expand directory
- `h` to close current directory or move to parent
- `<BS>` to go to parent/root shift
- `u` to refresh
- `.` to focus current directory
- `H` to toggle hidden files
- `I` to toggle ignored files
- `Z` to close all directories
- `]g` / `[g]`
- `]d` / `[d]`
- `]e` / `[e]`
- `]w` / `[w]`

File operations:

- `a` add file or directory
- `r` rename
- `d` delete
- `y` yank paths
- `p` paste/copy
- `m` move
- `c` copy

If multi-select is not ready on day one, we should explicitly ship these as
single-selection operations first and document that bulk operations arrive with
multi-select.

## Mouse Navigation

The terminal explorer should support:

- left click to focus pane and select row
- click disclosure glyph to toggle directory
- double click to open file or toggle directory
- wheel scroll
- right click to open the file-tree context menu

This implies row hit-testing by pane-local coordinates.

## Context Menus

We already have shared menu shapes in `the-default/context_menu.rs`.

The terminal still needs:

- right-click invocation from explorer rows
- action execution for file-tree actions
- prompt/confirm integration for new file/folder, rename, delete

## Watchers

The terminal explorer should watch:

- open explorer roots
- currently open directories within those roots
- git index for relevant repositories

and batch refresh requests.

This should be terminal-owned. It does not belong in `file_tree.rs`.

---

## Interaction With The Split System

The explorer must be navigable with the same pane-navigation model as other
split surfaces.

That means:

- focusing an explorer pane uses the same pane focus operations
- jumping across panes works unchanged
- opening a file from the explorer uses `OpenTarget`
- split-open actions from the explorer should use:
  - `OpenTarget::Active`
  - `OpenTarget::Split { ... }`
  - `OpenTarget::Neighbor { ... }`

This is one of the strongest reasons to implement the explorer as a pane
surface rather than a floating overlay.

Note:

- buffer cycling should continue to mean buffer cycling
- pane navigation should be what reaches the explorer pane

The explorer is not a buffer and should not be forced into buffer-MRU
semantics.

---

## How `Command::FileExplorer` Should Work

The existing seam is already correct:

- `Command::FileExplorer`
- `Command::FileExplorerInCurrentBufferDirectory`
- `supports_native_file_explorer`
- `open_native_file_explorer`

The terminal client should start overriding those hooks and open a real explorer
pane instead of falling back to file picker.

That gives us a clean migration path:

- current behavior: file picker fallback
- future behavior: split-hosted terminal explorer

without changing the command/keymap surface.

---

## Phased Implementation Plan

## Phase 1: Split-Hosted Client Surfaces

Goal:

- let panes host terminal-owned non-buffer surfaces cleanly

Changes:

- generalize pane content in `the-lib/editor.rs`
- add a generic client-surface pane content kind
- update `FrameRenderPlan` and terminal render to understand that kind
- keep routing/layout/focus in `the-lib`
- keep actual surface state in `the-term`

Why first:

- everything else depends on the explorer being a real pane

## Phase 2: Open `file_tree.rs` For Explorer-Grade Use

Goal:

- make the shared tree substrate good enough for a real explorer

Changes:

- add `FileTreeFilter`
- add visible traversal/reveal helpers
- add `close_all`
- add row-prefix/guide helpers
- optionally add multi-select now if we want bulk ops immediately

Why second:

- the terminal UI should consume a better tree substrate, not paper over
  missing shared helpers

## Phase 3: Terminal Explorer Surface State

Goal:

- introduce terminal-owned explorer pane state

Changes:

- add explorer surface registry in `the-term`
- attach explorer surfaces to pane content
- expose open/focus/close helpers
- wire `open_native_file_explorer`

Why third:

- once panes can host client surfaces, the terminal needs an actual explorer
  state object to drive rendering and input

## Phase 4: Explorer Rendering

Goal:

- render the shared file tree as a pane

Changes:

- build tree rows from `FileTreeSnapshot`
- apply shared decorators
- draw tree guides/icons/disclosure/badges/severity/status
- style selected and active rows
- keep pane rendering isolated from text-document rendering

Why fourth:

- this gives us the first visible explorer pane

## Phase 5: Keyboard And Mouse Interaction

Goal:

- make the explorer usable as a real terminal surface

Changes:

- keyboard navigation for browse mode
- open/toggle behavior
- refresh/focus/toggle-hidden/toggle-ignored
- pointer hit-testing
- click/double-click behavior
- wheel scrolling
- right-click context menu invocation

Why fifth:

- once the pane renders, interaction can be layered without changing the
  shared tree model

## Phase 6: File Operations And Context Menus

Goal:

- make the explorer operational, not just navigational

Changes:

- implement new file/folder prompts
- implement rename/delete confirmations
- execute `FileTreeOp`
- wire context menu actions to explorer state and prompts

Why sixth:

- typed file operations already exist; this phase is mostly terminal integration

## Phase 7: Decorations And Watchers

Goal:

- make the explorer feel alive

Changes:

- add watcher manager for open dirs and git index
- install default git/diagnostic decorators
- refresh tree incrementally
- follow active file when configured

Why seventh:

- this is where the explorer becomes comparable to Snacks in responsiveness and
  useful metadata

## Phase 8: Search Mode And Oil-Like Editing

Goal:

- unlock the more ambitious workflows

Changes:

- add tree filtering/search mode
- add editable explorer buffer mode using `FileTreeEditSession`
- add parser/commit hooks for tree-text edits

Why last:

- browse-mode explorer should land first
- `FileTreeEditSession` is already a strong foundation for this later phase

---

## Proposed API Shapes

The exact names can change, but the implementation should move in this shape.

### 1. Split-Hosted Client Surfaces

In `the-lib/editor.rs`:

- `PaneContentKind::ClientSurface`
- `PaneContent::ClientSurface { surface_id: ClientSurfaceId }`
- `ClientSurfaceId` as a stable pane-routable identity

Required editor helpers:

- `open_client_surface_in_active_pane(surface_id)`
- `open_client_surface(target, surface_id)`
- `focus_pane_with_surface(surface_id)`
- `pane_surface(pane_id) -> Option<ClientSurfaceId>`

The important constraint is that the split system should not need to know what
kind of client surface it is hosting.

### 2. File Tree Substrate

In `the-default/file_tree.rs`:

- `FileTreeFilter`
- `FileTreeFilterMode` or equivalent hidden/ignored/include/exclude settings
- `marked_paths: BTreeSet<PathBuf>` if/when multi-select lands

Traversal helpers should look roughly like:

- `expand_to(path)`
- `ensure_visible(path)`
- `is_visible(path) -> bool`
- `next_visible(path, predicate) -> Option<PathBuf>`
- `prev_visible(path, predicate) -> Option<PathBuf>`
- `close_all(root)`

Decoration helpers should be able to produce a composed node presentation, not
just raw tree data.

### 3. Terminal Explorer Surface State

In `the-term`:

- `ExplorerSurfaceId` or reuse `ClientSurfaceId`
- `ExplorerSurfaceState`

Expected state:

- explorer root mode/path
- scroll offset
- hovered row
- active row cache / selected row cache
- optional local view toggles
- optional watcher registration metadata

This should remain separate from `FileTreeState`, which is still the shared
filesystem model.

### 4. Row Rendering Contract

The terminal renderer should consume shared row layout data rather than
re-deriving tree structure ad hoc.

That means the shared contract should carry enough information to render:

- indentation depth
- tree guide ancestry
- disclosure state
- icon
- label text
- right-aligned secondary text
- badges
- severity/status
- selection/active hints

### 5. Operation / Prompt Boundary

Typed file operations should stay shared.
Prompting and confirmation should stay terminal-owned.

That means:

- `FileTreeOp::{Open, CreateFile, CreateDirectory, Rename, Delete, Move, Refresh}`
- terminal prompts gather paths/names/confirmation
- terminal then calls shared execution helpers

### 6. Oil-Like Editing Boundary

Editable mode should not turn the tree itself into a special case of the text
editor core.

The cleaner shape is:

- `FileTreeEditSession` produces editable text
- terminal opens that as a dedicated explorer-edit workflow
- commit path parses edits back into typed filesystem changes
- cancel path simply discards the session

This keeps tree editing as a feature built on top of shared file-tree parsing,
not as a distortion of the document model.

---

## Acceptance Criteria By Phase

### Phase 1

- explorer can occupy a pane without pretending to be a document buffer
- pane focus and split navigation work unchanged
- `OpenTarget` can route content opened from non-buffer panes

### Phase 2

- file tree supports hidden/ignored toggles through shared state
- follow/reveal/navigation helpers exist in shared code
- row layout exposes enough structure for tree-guide rendering

### Phase 3

- terminal can open, focus, and close explorer panes explicitly
- `Command::FileExplorer` and `Command::FileExplorerInCurrentBufferDirectory`
  stop falling back to file picker on the terminal client

### Phase 4

- explorer rows render in-pane with guides, disclosure, icons, badges, and
  active/selected styling
- active file can be highlighted if present in the tree

### Phase 5

- mouse click, double click, right click, and wheel scrolling all work
- keyboard browsing is fast enough to replace the picker fallback for normal
  file navigation

### Phase 6

- create, rename, delete, move, and refresh are usable from keyboard and
  context menu
- failures are surfaced through the message system without corrupting tree
  state

### Phase 7

- git/diagnostic decorations update without manual refresh spam
- refreshing large trees does not require rescanning the entire workspace on
  every small change

### Phase 8

- tree filtering/search mode feels like a natural extension of browse mode
- editable mode can round-trip a non-trivial rename/create/delete patch
- the shared tree editing substrate is usable for oil-like workflows

---

## Recommended File Touch Map

Shared/core:

- `the-lib/editor.rs`
- `the-lib/render/plan.rs`
- `the-default/file_tree.rs`
- `the-default/command.rs`
- `the-default/context_menu.rs`
- `the-default/assembly.rs`
- `the-default/extensions.rs`

Terminal:

- `the-term/ctx.rs`
- `the-term/input.rs`
- `the-term/render.rs`
- likely one new module dedicated to explorer surfaces

---

## Non-Goals For The First Pass

The first pass does not need to ship all Snacks behaviors at once.

It does not need:

- editable oil-like mode immediately
- full multi-select if that would slow down landing the pane surface
- preview pane integration on day one
- netrw-style directory auto-open replacement

The first pass should focus on:

- split-hosted explorer pane
- good row rendering
- good keyboard/mouse navigation
- correct file operations for the primary selection
- clean architecture that leaves room for the advanced features later

---

## Final Recommendation

The correct next move is:

1. generalize split panes so the terminal can host client-owned pane surfaces
2. strengthen `file_tree.rs` with filters and traversal helpers
3. build the terminal explorer as a pane surface on top of that

The current `file_tree.rs` should absolutely be used.

What should not be copied from Snacks is the picker-specific shell.
What should be copied is the architecture:

- tree model
- actions
- watch/decorate/refresh layers
- UI as a consumer

That gets us a split-hosted explorer that feels native to `the-editor`, while
still leaving a path toward oil.nvim-style editing later through
`FileTreeEditSession`.
