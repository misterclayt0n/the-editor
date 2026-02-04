# Porting `core/commands.rs` → `the-default/command.rs`

This doc summarizes the current structure differences and gives a concrete playbook for porting **all** commands from the old editor (`~/the-editor/the-editor/core/commands.rs`) into the new layout (`the-default/command.rs`). Use this when delegating work to other agents.

## High‑level differences (old vs new)

### Old (the-editor)
- **Context** is a large UI/runtime object:
  - `Context` contains `editor`, view/job references, register state, macro state, etc.
  - Many commands rely on view-specific features: view ids, inner area, text annotations, gutters, jumps, UI jobs, etc.
- **Commands are functions** on `Context` (e.g. `fn move_char_left(cx: &mut Context)`).
- **Keymaps map to functions** by name.
- **Undo/redo/commit** are handled by doc history, often with explicit calls.

### New (the-default + the-lib)
- **the-lib** is core, minimal, *no command enum* and *no input types*.
- **the-default** is the policy layer:
  - Owns `Command`, `Motion`, `Direction`, etc in `the-default/command_types.rs`.
  - Owns input types `KeyEvent`, `KeyOutcome`, `Modifiers` in `the-default/input.rs`.
  - Owns keymaps and the command registry.
- **Dispatch chain** lives in `the-default/command.rs` and is configurable.
- **Editor model** is simplified:
  - `Editor` → one `Document`, one `ViewState`.
  - No view IDs or multiple documents/views in the core.
- **No UI side effects** (animations, compositor jobs, etc). Ignore old “noop effects” and UI flush logic.
- **UiTree is now the shared UI source of truth** for overlays (command palette, etc.).

## Where to add new commands

1) **Command types**
   - Add variants to `the-default/command_types.rs`:
     - `enum Command { ... }`
     - `enum Motion { ... }` and helpers (counts already present).

2) **Dispatch plumbing**
   - **Always add a dispatch hook for every `Command` variant** in
     `define! { Default { ... } }` inside `the-default/command.rs`.
   - Update `build_dispatch()` to wire all hooks.
   - Update `on_action` to call the dispatch hook for every command.
   - **Rule:** if a command exists in `Command`, it must have a hook. If it does
     not need a hook, it should not be a `Command`.

3) **Implementation functions**
   - Add actual behavior in `the-default/command.rs` (e.g. `fn delete_word_forward<Ctx: DefaultContext>(ctx: &mut Ctx, ... )`).

4) **Command lookup**
   - Extend `command_from_name` in `the-default/command.rs` to map string names to `Command`.

5) **Keymaps**
   - Add bindings in `the-default/keymap.rs` (or in config keymaps).

## Structural mapping: old → new

### Old command functions
Old commands are simple functions like:
```rust
pub fn move_char_left(cx: &mut Context) {
  move_impl(cx, move_horizontally, Direction::Backward, Movement::Move)
}
```

### New command style
In the new structure, these become `Command` variants and are dispatched through `Motion`:
```rust
// command_types.rs
pub enum Command {
  Motion(Motion),
  // ...
}

// command.rs
fn on_action<Ctx: DefaultContext>(ctx: &mut Ctx, command: Command) {
  match command {
    Command::Motion(motion) => ctx.dispatch().motion(ctx, motion),
    // ...
  }
}

fn motion<Ctx: DefaultContext>(ctx: &mut Ctx, motion: Motion) { /* ... */ }
```

### The common helper patterns to port
- Old `move_impl` and `move_word_impl` are already expressed in `motion()`.
- Old `extend_*` commands map to `Motion` with `extend: true`.
- Old “counts” are supported via `Motion`’s `count` field.

**Therefore:** most movement/selection commands do *not* need new custom functions. They should map to `Command::Motion(...)` and use the existing `motion()`.

## What already exists in new `the-default/command.rs`
- Insert/delete char
- Basic motion + word motions (via `Motion`)
- Save/Quit
- Default dispatch chain (`pre_on_keypress`, `on_keypress`, `post_on_keypress`, `pre_on_action`, `on_action`, `post_on_action`)
- Keymap entry point (`handle_key` + keymap logic)
- UI tree pipeline: `pre_ui → on_ui → post_ui` with UiTree overlays
- UiEvent routing for overlays (command palette uses UiEvents)

## What is still missing (categories to port)

Below are the **major categories** from old `core/commands.rs` that still need to be ported. Use these as phases for implementation.

### 1) Selection / cursor ops
- `collapse_selection`, `flip_selections`, `merge_selections`, `split_selection`, `split_selection_on_newline`, etc.
- `add_cursor_above/below` (already partially implemented as `add_cursor`)
- `keep_primary_selection`, `remove_primary_selection` → must be rethought because new selection model removed “primary cursor”. (We now use cursor IDs + active cursor in view state.)

### 2) Text object / selection expansion
- `select_textobject_{inner,around}`
- `expand_selection`, `shrink_selection`, `select_all_children`, `select_prev_sibling`, etc.

### 3) Editing / deletion variants
- `delete_word_*`, `kill_to_line_*`, `replace`, `replace_with_yanked`, etc.
- `delete_selection` vs `delete_selection_noyank`
- `change_selection` / `change_selection_noyank`

### 4) Yank / paste / registers
- Yank / paste operations, clipboard integration, registers.
- `the-lib/registers.rs` already exists; it should be used from `the-default` layer.

### 5) Search + find
- `find_next_char`, `find_prev_char`, `extend_next_char`, etc.
- `search`, `rsearch`, `search_next`, `extend_search_next`, etc.

### 6) View / navigation / scrolling
- `page_up`, `page_down`, half-page variants
- view jumps (`goto_last_modification`, `jump_forward`, etc.)
- window movement / split commands (may not exist yet in new core)

### 7) Commands that depend on UI subsystems
Some old commands depend on prompt, file picker, tree view, terminal, etc.
Those should be stubbed or moved to the client later:
- `file_picker`, `buffer_picker`, `terminal_picker`, `tree_explorer`, etc.
- In the new architecture, these belong in **client UI** or in a **runtime layer**, not core.

## How to port: recommended workflow for an agent

### Step 1 — Inventory
- List all old commands and group them by category (movement, edit, selection, search, etc.).
- Identify which ones map directly to existing movement helpers.

### Step 2 — Extend Command enum
- For each new command, add a `Command` variant in `command_types.rs`.
- If it is a parameterized movement, use `Command::Motion(Motion)` with the right `Motion` helper instead of adding a new variant.

### Step 3 — Add handlers
- Implement logic in `the-default/command.rs`.
- Keep behavior **pure**, no UI side effects (ignore old effects).
- Use the-lib building blocks:
  - `movement::*`
  - `selection::Selection` + `transform()`
  - `transaction::*` for edits
  - `text_object`, `object`, `surround`, `comment`, `indent`, `match_brackets`, `search`, `textobject`

### Step 4 — Integrate with dispatch
- Wire into `on_action` match.
- If you add a new command that requires extra arguments, you might want a dedicated dispatch hook in `define! { Default { ... } }`.

### Step 5 — Expose in `command_from_name`
- Add string mapping for any command used by keymaps or command registry.

### Step 6 — Update keymaps (optional)
- Add bindings in `the-default/keymap.rs` or leave for user config.

## Example: mapping a legacy command

Old code:
```rust
pub fn move_next_long_word_start(cx: &mut Context) {
  move_word_impl(cx, movement::move_next_long_word_start)
}
```

New mapping:
```rust
// command_from_name
"move_next_long_word_start" => Some(Command::move_next_long_word_start(1))

// or in keymap: "W" => move_next_long_word_start
```
No new handler needed—`motion()` already implements `WordMotion::NextLongWordStart`.

## Core design constraints to respect

- **No UI-side effects** in the-default (no compositor callbacks, no job system, no view effects). These belong in clients.
- **No view IDs** in the-lib. Use the single document / selection API.
- **Counts** are part of `Motion`; if a command needs a count, encode it there.
- **Primary cursor** is gone. Use `CursorId` + view’s `active_cursor` for styling/selection, but core selection operations should be cursor‑agnostic.
- **UiTree is the only UI contract**: the-default builds UiTree overlays; clients render UiTree and send UiEvents.

## Quick map: old `Context` → new `DefaultContext`

| Old Context | New DefaultContext |
|-----------|--------------------|
| `cx.editor` | `ctx.editor()` |
| `cx.count()` | encode in `Motion.count` (or add to Command) |
| `cx.jobs` | **client only** (ignore in port) |
| `cx.on_next_key` | use dispatch hook in the-default or client key pipeline |
| `cx.register` | use `Registers` in the-default layer |

## Pending input (new in default layer)

The new architecture does **not** have `cx.on_next_key()` in core. Instead, use a
**pending-input state** stored in the default layer and consumed at the top of
`pre_on_keypress`.

## Render pipeline (dispatch-backed)

Rendering is now driven through dispatch as well. Clients should **not** build
render plans directly; instead they call the default helpers:

- `the_default::render_plan(ctx)` → runs:
  `pre_render → on_render → post_render`
- `the_default::render_plan_with_styles(ctx, styles)` → runs:
  `pre_render → pre_render_with_styles → on_render_with_styles → post_render`

`DefaultContext` provides:
- `build_render_plan(&mut self) -> RenderPlan`
- `build_render_plan_with_styles(&mut self, RenderStyles) -> RenderPlan`

The term and Swift/FFI clients call these helpers so render layout/config can be
customized through dispatch hooks.

### Where it lives
- `the-default/pending.rs` → `enum PendingInput`
- `DefaultContext` now exposes:
  - `fn pending_input(&self) -> Option<&PendingInput>`
  - `fn set_pending_input(&mut self, pending: Option<PendingInput>)`

### Dispatch behavior
`pre_on_keypress` now checks `pending_input` and consumes the next key before
normal keymap handling:

```rust
fn pre_on_keypress<Ctx: DefaultContext>(ctx: &mut Ctx, key: KeyEvent) {
  if let Some(pending) = ctx.pending_input().cloned() {
    ctx.set_pending_input(None);
    if handle_pending_input(ctx, pending, key) {
      return;
    }
  }

  ctx.dispatch().on_keypress(ctx, key);
}
```

## UI: UiTree + Themes (current model)

### UiTree pipeline (clients render this)
- the-default builds a `UiTree` in `on_ui`, then `post_ui` can mutate it.
- `the_default::ui_tree(ctx)` runs `pre_ui → on_ui → post_ui` and returns the tree.
- Clients **must render UiTree** (Swift + term do this now).
- Overlays are regular UiNodes under `tree.overlays`; no client-specific palette UI.

### UiEvents
- UiEvents are routed in `ui_event` via `pre_ui_event → on_ui_event → post_ui_event`.
- Command palette is driven by UiEvents:
  - `UiEventKind::Key` for key handling
  - `UiEventKind::Activate` to submit
  - `UiEventKind::Dismiss` to close

### Theme resolution (shared, in the-lib)
- UiTree styles are resolved **in the-default** before clients render:
  - `the_default::ui_tree(ctx)` calls `resolve_ui_tree(&mut tree, ctx.ui_theme())`.
- Clients now **only consume resolved UiColor values**; no token-to-color logic in clients.
- Theme selection is owned by the client (`DefaultContext::ui_theme()`), so CLI/FFI can pick a theme.

### UI roles (semantic styling)
- `UiStyle.role` is used for semantic scopes like `command_palette` and `command_palette.help`.
- Role → scope mapping lives in `the-lib/render/ui_theme.rs` and follows:
  `ui.{role}.{component}.{state}.{prop}` → fallbacks → `ui.{prop}`.

### Command palette status
- The command palette UI is built from UiTree (`the-default/command_palette.rs`).
- The help panel overlay has been removed (description is rendered inline).

### How to use (pattern)
For any command that needs a follow‑up character (f/t/F/T, insert‑register,
replace, etc.), set pending state and return:

```rust
fn find_char_forward<Ctx: DefaultContext>(ctx: &mut Ctx, inclusive: bool, extend: bool) {
  ctx.set_pending_input(Some(PendingInput::FindChar {
    direction: Direction::Forward,
    inclusive,
    extend,
    count: 1,
  }));
}
```

Then implement actual behavior in `handle_pending_input`.

### Current status
`handle_pending_input` is currently a placeholder that consumes the key and
returns `true` (handled). This unblocks the architecture so other agents can
wire real behaviors without changing core/dispatch again.

## Suggested port order (minimize regressions)

1) **Movement + selection** (already mostly in `Motion`) → fill gaps.
2) **Insert / delete** primitives (backspace, delete word, kill line).
3) **Yank / paste / registers**.
4) **Search / find**.
5) **Text objects / structure**.
6) **View / navigation** (may require additional core APIs).
7) **UI-only commands** (leave stubs or skip).

---

If you need to create new helpers, prefer adding **pure functions** to `the-lib` and calling them from `the-default` commands. Keep the-default thin: it should orchestrate, not reinvent text/selection logic.
