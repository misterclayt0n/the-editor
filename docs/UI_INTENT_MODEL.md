# UI Intent Model (Plan)

## Goal
Build a **client-agnostic UI model** that supports complex, user-authored “plugins” through
dispatch/config without locking any client to pixel layout.  

**Core idea:**  
- the-lib emits **structure + intent**, not coordinates  
- clients render **native UI** for their platform (terminal, SwiftUI, etc.)  
- config/dispatch can **mutate the UI model** to add panels, lists, inputs, etc.  

This enables "opencode inside the editor" style plugins while keeping clients free.

---

## Architecture Summary

### the-lib = data + intent
UI is represented as a tree of semantic nodes (panel, list, input, text, etc.).
No pixel math, no row/col layout.

### clients = layout + visuals
Each client maps UI intent to its own coordinate system and aesthetic.

### config/dispatch = UI overrides
User config modifies UI intent and structure via dispatch hooks.

---

## Core Data Model (Sketch)

### UiTree
- `root`: primary UI tree (non-floating, anchored to the app surface)
- `overlays`: floating overlays / modals / palettes
- `focus`: optional focus target + cursor position

### UiNode (semantic building blocks)
- `Container { layout, children }`
- `Panel { title, intent, style, child }`
- `Text { content, style }`
- `List { items, selected, scroll }`
- `Input { value, cursor, placeholder }`
- `Divider`
- `Spacer`
- `Tooltip { target, content, placement }`
- `StatusBar { left, center, right }`

### Layout intent (not coordinates)
```rust
enum LayoutIntent {
  Floating,
  Bottom,
  Top,
  SidebarLeft,
  SidebarRight,
  Fullscreen,
  Custom(String), // client-specific intent token
}
```

### Style tokens (not pixels)
```rust
enum UiColor { Token(UiColorToken), Value(Color) }

struct UiStyle {
  fg: Option<UiColor>,
  bg: Option<UiColor>,
  border: Option<UiColor>,
  accent: Option<UiColor>,
  emphasis: Emphasis, // normal, muted, strong
  radius: RadiusToken, // small/medium/large
}
```

### Constraints + alignment
```rust
struct UiConstraints {
  min_width: Option<u16>,
  max_width: Option<u16>,
  min_height: Option<u16>,
  max_height: Option<u16>,
  padding: UiInsets,
  align: UiAlignPair,
}
```

### Layering + focus
```rust
enum UiLayer { Background, Overlay, Tooltip }

struct UiFocus {
  id: String,
  cursor: Option<usize>,
}
```

---

## Dispatch Integration (Hooks)

Add a UI pipeline that mirrors the existing render pipeline:

```rust
fn pre_ui(ctx: &mut Ctx);
fn on_ui(ctx: &mut Ctx) -> UiTree;          // default UI (command palette, etc.)
fn post_ui(ctx: &mut Ctx, tree: UiTree) -> UiTree; // user/plugin overrides
```

Then in clients:
```rust
render_plan = render_plan(ctx)
ui_tree = UiTree::new()
dispatch.pre_ui(ctx, ())
let tree = dispatch.on_ui(ctx, ())
let tree = dispatch.post_ui(ctx, tree)
client.render(render_plan, tree)
```

### UI event pipeline (optional)
```rust
fn pre_ui_event(ctx: &mut Ctx, event: UiEvent) -> UiEventOutcome;
fn on_ui_event(ctx: &mut Ctx, event: UiEvent) -> UiEventOutcome;
fn post_ui_event(ctx: &mut Ctx, outcome: UiEventOutcome) -> UiEventOutcome;
```

This lets configs/plugins handle UI-specific keys or actions without coupling
them to any one client.

---

## Plugin Example (Conceptual)

```rust
fn post_ui(ctx: &mut Ctx, tree: UiTree) -> UiTree {
  let mut tree = tree;
  tree.overlays.push(
    Panel::floating("agent")
      .with_title("Agent")
      .with_body(List::from(ctx.agent_messages()))
  );
  tree
}
```

This is the core enabler for “full plugins” via config.

---

## Why This Matches the Vision
- Clients stay native (SwiftUI looks like SwiftUI; terminal looks like terminal)
- Users can compose full UI panels without modifying clients
- Avoids pixel/row/col mismatch across platforms
- Keeps the existing dispatch model as the single extensibility surface

---

## Next Steps
1) Define minimal `UiTree` + `UiNode` in the-lib.
2) Add UI dispatch hooks to DefaultDispatch + DefaultContext.
3) Port command palette to UiTree data.
4) Implement rendering adapters in the-term + the-swift.
