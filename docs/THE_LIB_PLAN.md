# the-lib completion plan

## summary
Finish the-lib by introducing a **slim document core**, a **minimal editor/session core**, and a **pure render pipeline** under `the_lib::render`. Keep the-lib a deterministic state machine: no IO, no LSP, no async tasks, no UI. Clients own event loops and rendering; the-lib provides the data and transformations.

---

## architectural references (what we’re copying, what we’re avoiding)

### ghostty (~/code/ghostty)
- **Core state is renderer-agnostic**: `Termio` owns terminal state and updates it; renderer is a separate subsystem. (`src/termio/Termio.zig`)
- **Render wakeups are explicit**: mailbox + wakeup handle, not implicit UI logic.
- **Event loop lives in the app**: the core doesn’t run a loop; the app owns it.

**Takeaway for the-lib:**
- Keep editor state independent from any renderer or UI.
- The lib should emit *data* (display list, layout info, dirty ranges) and let clients decide when/how to render.
- Any wakeup or scheduling is a client concern.

### helix (~/code/helix)
- **helix-core** holds pure algorithms; **helix-view** holds the big `Document` and `Editor` state with LSP, diagnostics, view state, etc.
- Their `Document` is feature-rich (LSP, diagnostics, diffs, selections, view data, etc.).

**Takeaway for the-lib:**
- We want helix-core-style purity **without** helix-view’s service coupling.
- Document/editor should be split into *core* + *external attachments* (handled by dispatch/app).

### the-editor legacy (~/the-editor/the-editor)
- `core/document.rs` mixes IO, LSP, tasks, diff providers, diagnostics, view state, and rendering hooks.
- `editor.rs` orchestrates everything in a single stateful type.

**Takeaway for the-lib:**
- Extract only the pieces required for deterministic state evolution.
- Everything else becomes **ports** or **attachments** managed by the-dispatch or clients.

---

## principles (already established)
- **Functional style**: transform state via explicit inputs; no hidden side effects.
- **Minimal state**: avoid mixing runtime services into core types.
- **Typed errors** (`thiserror`) and predictable APIs.
- **Performance first**: avoid unnecessary allocations; prefer `SmallVec`, `RopeSlice`, `Cow` when possible.

---

## target module structure (the-lib)

### core (pure state + algorithms)
- `selection`, `transaction`, `history`, `movement`, `search`, `fuzzy`, `diff`, `auto_pairs`, `comment`, `object`, `text_object`, `surround`, `match_brackets`, `indent`, `position` (logical)

### syntax
- `syntax` and `syntax/config` (already split and cleaned)
- `syntax/indent_query`, `syntax/highlight_cache`

### render (pure layout/data)
- `render/graphics.rs`
- `render/text_format.rs`
- `render/text_annotations.rs`
- `render/doc_formatter.rs`
- `render/visual_position.rs`

No UI, no windowing, no IO in render.

---

## what’s missing (critical path)

### 1) Document core (new)
A minimal, deterministic `Document` that only models text + editing state.

**Must include**
- `Rope` text
- `Selection` (per view or per cursor, see decision below)
- `History` + `ChangeSet`
- `LineEnding`, `IndentStyle`, and related config overrides
- `Syntax` state *as data* (optional, controlled by caller)
- `version`/`revision` counters

**Must exclude (move to dispatch/client)**
- IO (load/save, file watching)
- LSP clients and requests
- Diagnostics sources and async tasks
- Diff provider registry
- View tree / UI layout
- Theme, color, or actual rendering

**API sketch (directional)**
- `apply(Transaction) -> Result<()>`
- `commit(UndoKind)`
- `set_selection(view_id, Selection)`
- `selection(view_id)`
- `with_text(...)` / `replace_text(...)`
- `set_syntax(Syntax)` / `clear_syntax()`

**Decision:** If multiple views per doc are required for the-lib, keep
`HashMap<ViewId, Selection>` but make `ViewId` a lightweight newtype
owned by the-lib (not UI).

---

### 2) Editor/Session core (new)
A minimal `EditorState` (or `Session`) that groups documents and global data.

**Must include**
- `DocumentStore` (map `DocumentId -> Document`)
- `Registers` (yank/put) if needed for core ops
- `Global config` snapshot (immutable or copy-on-write)
- `SearchState` (optional)
- `ViewState` **only if** required for core logic (scroll offsets, cursor position per view)

**Must exclude**
- UI tree / panes / widgets
- file system, LSP, clipboard, diagnostics
- rendering loops

**API sketch (directional)**
- `new_document(text) -> DocumentId`
- `close_document(id)`
- `document(id)` / `document_mut(id)`
- `apply_to_document(id, Transaction)`
- `set_active_view(view_id, doc_id)`

---

### 3) Render pipeline (finish `the_lib::render`)
This is required to unblock `indent`, `visual_position`, and final UI.

**Port order**
1. `render/graphics.rs` (rects/colors/cursor types) – pure data
2. `render/text_format.rs` (wrap, tab width, line metrics) – pure config
3. `render/text_annotations.rs` (overlays, inline annotations) – pure data
4. `render/doc_formatter.rs` (layout + display-list builder)
5. `render/visual_position.rs` (visual coords <-> char coords)

**Outputs**
- `DisplayList` or `RenderPlan`: a flat list of draw ops with positions,
  styles, and text spans. This keeps clients dumb and stateless.

---

## porting order (next steps)

### Phase 1: finish render primitives
- Port `graphics.rs`, `text_format.rs`, `text_annotations.rs` into `the_lib::render`.
- Keep all logic pure and deterministic.

### Phase 2: port `doc_formatter` and expose layout APIs
- Build a minimal layout pipeline that converts `(Rope + annotations + format)`
  into a display list.
- Add narrow, testable helpers (e.g. line wrap logic, soft wrap, line numbers).

### Phase 3: implement core `Document`
- Introduce new `the-lib/document.rs` with minimal state.
- Move `History` integration into this doc (commit/apply/undo hooks).
- Keep syntax optional: document can run without it.

### Phase 4: implement core `EditorState`
- Add `the-lib/editor.rs` or `session.rs` as document registry + global state.
- Keep view tracking only as far as core logic requires it.

### Phase 5: integrate render + doc
- `doc_formatter` takes `Document` and `ViewState` (if any) and returns display list.
- All UI clients consume display list only.

---

## open design decisions (resolve before heavy work)

1) **Selection ownership**
   - Per view (document owns `HashMap<ViewId, Selection>`), or
   - Per document (single selection, client handles multi-view)?

2) **View state location**
   - If view scroll/viewport affects layout, view state probably lives in the-lib.
   - If clients can handle it, keep view state outside and pass it to render.

3) **Syntax lifecycle**
   - Should `Document` own Syntax + highlight cache, or should it be an external attachment?

4) **Persistent data structures**
   - Decide whether Rope-only persistence is enough for now or if full-state persistence
     is worth it. (Probably Rope-only for minimal path.)

5) **Display list API shape**
   - Finalize data structures before porting `doc_formatter` so it doesn’t churn.

---

## recommended immediate move
Start with **Phase 1 + Phase 2** (render pipeline), because it unblocks
`visual_position` and `indent` and gives a stable target for the Document/Editor API.

After that, build `Document` and then `EditorState` in that order.

