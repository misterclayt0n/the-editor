# Document Core Plan (the-lib)

This plan defines how to rebuild the legacy `core/document.rs` into a **minimal, deterministic
Document** inside `the-lib`, keeping the library as a pure state machine and pushing IO/LSP/UI
concerns into the dispatch/client layer.

---

## Goals
- **Deterministic state**: `Document` is a pure data model + transformations.
- **No IO/services**: no file watching, no LSP clients, no async jobs.
- **Functional API**: explicit inputs/outputs, typed errors, no hidden side effects.
- **Performance**: avoid unnecessary allocations; stable iteration; minimal copying.

## Non‑Goals
- File open/save, encoding detection, editorconfig discovery.
- LSP diagnostics, inlay hints, code actions, formatting.
- View tree/layout, animation, rendering hooks.
- VCS/diff providers, filesystem metadata.

---

## Legacy document.rs: what it currently mixes (for reference)

**Core state** (keep)
- `text: Rope`
- `selections: HashMap<ViewId, Selection>`
- `history: History` (+ `ChangeSet` + `State`)
- `indent_style: IndentStyle`
- `line_ending: LineEnding`
- `version` / `revision`

**Render adjacency** (move to render or view layer)
- `view_data`, `ViewPosition`
- `inline annotations`, `overlays` for inlay hints
- `text_format`, soft wrap overrides, wrap indicator overrides

**External services** (move to dispatch/client)
- IO, encoding, savepoints, timestamps, paths
- LSP clients, diagnostics, inlay hints
- VCS/diff handles, file watcher
- tasks (`TaskController`), async parsing

---

## Proposed new shape

### 1) `DocumentCore` (the-lib)
Minimal, deterministic, no IO.

**Fields**
- `id: DocumentId`
- `text: Rope`
- `selections: SelectionSet` (map `ViewId -> Selection` or single selection, decision below)
- `history: History` (and internal `ChangeSet` + `State` for undo/redo)
- `line_ending: LineEnding`
- `indent_style: IndentStyle`
- `version: u64` (monotonic)
- `flags: DocumentFlags` (readonly, modified)
- `syntax: Option<Syntax>` (optional; pure data only)

**Methods (directional)**
- `apply(transaction) -> Result<()>`
- `commit(UndoKind)`
- `undo/redo -> Option<Transaction>`
- `set_selection(view_id, Selection)` / `selection(view_id)`
- `replace_text(range, text)` (convenience over `Transaction`)
- `set_syntax(Syntax)` / `clear_syntax()`

### 2) `DocumentMeta` (dispatch/client)
Everything that’s file/IO/UI specific.

**Examples**
- `path`, `encoding`, `has_bom`, `last_saved_time`
- file watchers, diff providers
- LSP diagnostics + inlay hints
- view decorations, scroll animations

---

## Ownership decisions to resolve

### Selection ownership
- **Option A (current)**: `HashMap<ViewId, Selection>` in `Document`.
  - Pros: central, matches legacy.
  - Cons: ties document to UI view identity.
- **Option B**: single selection in doc; per‑view selection lives in app.
  - Pros: pure core, smaller state.
  - Cons: multi‑view editing needs app logic.

### Syntax lifecycle
- **Option A**: `Document` owns `Syntax` and highlight cache (still pure).
- **Option B**: syntax lives in dispatch layer and is passed into render.

My current suggestion: **Option A for now** (keeps APIs close to helix), but
limit to pure data. The loader and parsing threads stay outside.

---

## API boundaries

### Allowed in the-lib
- Text transformations, selections, history
- Syntax tree data (no background tasks)
- Render‑adjacent data structures (display list, annotations)

### Disallowed in the-lib
- IO (open/save), encoding detection
- LSP, diagnostics, inlay hints collection
- Task controllers, threads
- View tree, layouts, terminal/window glue

---

## Migration plan (incremental)

### Step 1: Create `the-lib/document.rs`
- Implement a minimal struct with `text`, `selection`, `history`, `indent`, `line_ending`.
- Hook `Transaction`/`History` integration.
- Add tests for apply/undo/redo and selection mapping.

### Step 2: Add `DocumentId` + `ViewId` to the-lib
- If not already, define lightweight IDs inside the-lib (no UI dependencies).

### Step 3: Optional syntax storage
- Store `Syntax` and highlight cache only as pure data.
- Add `set_syntax` / `syntax()` accessors.

### Step 4: Split render data
- Move per‑view formatting (`soft_wrap_override`, `wrap_indicator_override`) out.
- Anything that needs view size or layout goes to `render/` or app.

### Step 5: Remove IO-specific fields completely
- Paths, encoding, savepoints, timestamps remain in app layer.

---

## Notes on legacy fields (where they go)

| Legacy field | New home |
| --- | --- |
| `text`, `history`, `changes`, `old_state` | `DocumentCore` |
| `selections`, `view_data` | `DocumentCore` (optional) or app |
| `syntax`, `highlight_cache` | `DocumentCore` (pure) |
| `path`, `encoding`, `has_bom`, `last_saved_time` | app metadata |
| `diagnostics`, `language_servers` | dispatch/app |
| `diff_handle`, `version_control_head` | dispatch/app |
| `inlay_hints`, `jump_labels`, `color_swatches` | render/app |
| `soft_wrap_override`, `wrap_indicator_override` | render/app |
| `special_buffer`, `font_size_override` | app |

---

## Deliverables for the-lib
- `the-lib/document.rs` (core)
- `the-lib/editor.rs` or `session.rs` (document registry + global state)
- tests for document apply/undo/redo/selection

