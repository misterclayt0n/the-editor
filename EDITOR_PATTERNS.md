# The Editor - Command, Layout, and Component Systems

## 1. COMMAND SYSTEM

### 1.1 Command Registration and Dispatch

The command system is built on a **command registry** pattern with type-safe execution.

#### Key Files:
- `/home/mister/code/the-editor/the-editor/core/command_registry.rs` - Registry and command definitions
- `/home/mister/code/the-editor/the-editor/core/commands.rs` - Command implementations  
- `/home/mister/code/the-editor/the-editor/acp/commands.rs` - Example of how to add commands

#### Command Execution Flow:

```
User types `:command_name arg1 arg2`
  ↓
Input handler captures command
  ↓
CommandRegistry::execute(name, args_line)
  ↓
Get TypableCommand by name (from registry)
  ↓
Parse arguments with command's Signature
  ↓
Call command function with (Context, Args, PromptEvent)
  ↓
Command function modifies editor/documents/tree
```

### 1.2 How to Define a New Command

**Step 1: Create the command function**

```rust
// In a relevant module (e.g., core/commands.rs or acp/commands.rs)

pub fn cmd_terminal_new(cx: &mut Context, args: Args, event: PromptEvent) -> Result<()> {
  if event != PromptEvent::Validate {
    return Ok(());
  }
  
  // Get the action to use for split behavior
  let action = match args.get_flag("layout") {
    Some("vsplit") | Some("v") => Action::VerticalSplit,
    Some("hsplit") | Some("h") => Action::HorizontalSplit,
    _ => Action::VerticalSplit, // Default
  };
  
  // Create a new terminal view
  let terminal_doc_id = cx.editor.new_file(action);
  
  // Mark it as special buffer so editor knows it's not a regular file
  cx.editor.mark_special_buffer(terminal_doc_id, SpecialBufferKind::Terminal);
  
  // Spawn the terminal process in a background job
  cx.jobs.callback(async move {
    // Your async terminal setup code here
    Ok(...)
  });
  
  Ok(())
}
```

**Step 2: Register the command in CommandRegistry::register_builtin_commands()**

```rust
// In CommandRegistry::register_builtin_commands() method
// Around line 760-800 in command_registry.rs

self.register(TypableCommand::new(
  "terminal-new",                    // Primary command name
  &["term", "tn"],                   // Aliases
  "Open a new terminal in a split",  // Documentation
  cmd_terminal_new,                  // Function pointer
  CommandCompleter::none(),          // Completion config
  Signature {
    positionals: (0, Some(0)),       // 0 required, 0 max positional args
    flags: &[
      Flag {
        name: "layout",
        alias: Some('l'),
        doc: "Split layout: vsplit/v or hsplit/h",
        completions: Some(&["vsplit", "hsplit"]),
      },
    ],
    ..Signature::DEFAULT
  },
));
```

**Step 3: Add command to mod.rs or pub use it appropriately**

If defining in a separate module, ensure it's publicly accessible from the command_registry context.

### 1.3 Command Function Signature

```rust
pub type CommandFn = fn(&mut Context, Args, PromptEvent) -> Result<()>;

pub struct Context {
  pub editor: &mut Editor,      // Full mutable access to editor state
  pub scroll: Option<usize>,    // Optional scroll offset
  pub jobs: &mut Jobs,          // Job queue for async tasks
  pub dt: f32,                  // Delta time since last frame
}

pub enum PromptEvent {
  Update,    // User is still typing - preview behavior
  Validate,  // User pressed Enter - execute the command
  Abort,     // User pressed Escape - cancel
}
```

**Important:** Commands should return early if `event != PromptEvent::Validate` unless you want to support live previews.

### 1.4 Accessing Command Arguments

```rust
pub struct Args {
  positionals: Vec<String>,    // Positional arguments
  flags: HashMap<String, Option<String>>,  // Named flags
}

// Usage in command:
fn my_command(cx: &mut Context, args: Args, event: PromptEvent) -> Result<()> {
  if event != PromptEvent::Validate { return Ok(()); }
  
  // Access positional arguments
  if !args.is_empty() {
    let first_arg = &args[0];
    let all_args = &args[..];
  }
  
  // Access flags
  if args.has_flag("force") {
    // --force was passed
  }
  
  if let Some(value) = args.get_flag("output") {
    // --output <value> was passed
  }
  
  Ok(())
}
```

### 1.5 Existing Command Examples

The codebase has excellent examples of command patterns:

1. **Split commands** (vsplit, hsplit, split) - Lines 1964-2004 in command_registry.rs
   - Shows how to use Action::VerticalSplit and Action::HorizontalSplit
   - Opens files or duplicates current buffer in splits

2. **ACP commands** (acp_new_session, acp_send_prompt) - In acp/commands.rs
   - Shows async pattern with cx.jobs.callback_local()
   - Creates special buffers and manages sessions
   - Returns LocalCallback for deferred UI updates

3. **File operations** (open, write, buffer-close) - Various lines in command_registry.rs
   - Shows error handling and status messages
   - Uses cx.editor methods for document management

---

## 2. LAYOUT AND TREE SYSTEM

The editor uses a **tree-based view system** for managing splits and layout.

### 2.1 View Tree Architecture

```
                    Root (Container, Vertical)
                           |
              +------------+------------+
              |                         |
          Container               View (Buffer1)
         (Horizontal)                 
              |                        
        +-----+-----+                  
        |           |                  
      View       View              
    (Buffer2)  (Buffer3)           
```

Each view is a pane displaying a document. Containers hold views in splits.

#### Key Files:
- `/home/mister/code/the-editor/the-editor/core/tree.rs` - Tree structure and layout logic
- `/home/mister/code/the-editor/the-editor/core/layout.rs` - Layout engine for UI positioning
- `/home/mister/code/the-editor/the-editor/core/view.rs` - View data structure

### 2.2 How Splits Work (Tree Operations)

When you call `cx.editor.switch(doc_id, Action::VerticalSplit)`:

```rust
// In editor.rs, switch() method, lines 2537-2557

Action::VerticalSplit => {
  // 1. Get the current focused view and document
  let view = self.tree.try_get(self.tree.focus)
    .cloned()
    .unwrap_or_else(|| View::new(doc_id, gutters));
  
  // 2. Ask tree to split: creates a new view for this doc
  let view_id = self.tree.split(view, Layout::Vertical);
  
  // 3. Initialize view state for the document
  let doc = doc_mut!(self, &doc_id);
  doc.ensure_view_init(view_id);
  doc.mark_as_focused();
}
```

The tree.split() method (lines 208-337 in tree.rs):
1. Creates a new Node for the view
2. Inserts it into the tree's parent container
3. **Creates a new container if needed** (if split direction differs from parent)
4. Recalculates all layouts
5. Sets up animations for smooth transitions

### 2.3 Tree Structure: Node and Container

```rust
// From tree.rs, lines 79-140

#[derive(Debug)]
pub enum Content {
  View(Box<View>),           // Leaf node - displays a document
  Container(Box<Container>), // Internal node - holds children
}

#[derive(Debug)]
pub struct Container {
  layout: Layout,           // Horizontal or Vertical
  children: Vec<ViewId>,    // IDs of child nodes
  area: Rect,               // Size and position
  child_sizes: Vec<Option<u16>>, // Custom sizes (None = fill)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layout {
  Horizontal,  // Left-right split
  Vertical,    // Top-bottom split
}
```

### 2.4 Layout Engine (Constraint-Based)

For UI components that aren't views, use the layout engine:

```rust
use crate::core::layout::{Layout, Constraint, Alignment, align};

// Split screen: header (1 line), body (fill), footer (1 line)
let chunks = Layout::vertical()
  .constraints(vec![
    Constraint::Length(1),      // Fixed 1 cell
    Constraint::Fill(1),        // Remaining space (weight: 1)
    Constraint::Length(1),      // Fixed 1 cell
  ])
  .split(screen);

// Position a button in top-right corner
let button_rect = align(screen, 8, 2, Alignment::End);

// Create horizontal split: 50% | 50%
let halves = Layout::horizontal()
  .constraints(vec![
    Constraint::Percentage(50),
    Constraint::Percentage(50),
  ])
  .split(screen);
```

#### Constraint Types:
- `Length(u16)` - Fixed size in cells
- `Percentage(u16)` - Percentage of available space (0-100)
- `Ratio(u32, u32)` - Proportional ratio (numerator/denominator)
- `Min(u16)` - Minimum size, grows if space available
- `Max(u16)` - Maximum size, shrinks if needed
- `Fill(u16)` - Fills remaining space with weight for distribution

### 2.5 Inserting Views into the Tree

When you create a document and open it in a split:

```rust
// This is what happens internally:

// 1. Create or get a document
let doc_id = cx.editor.new_file(Action::VerticalSplit);

// 2. switch() uses the tree to create a view node
cx.editor.switch(doc_id, Action::VerticalSplit);

// Inside switch(), this happens:
// - tree.split() creates a new ViewId for the document
// - The view is inserted as a child in the container
// - Animations are set up for smooth resize

// 3. Access the view later
let view = cx.editor.tree.get(view_id);
let view_mut = cx.editor.tree.get_mut(view_id);

// 4. Iterate all views
for (view, doc) in cx.editor.tree.views_mut() {
  // Do something with each view
}
```

### 2.6 Focus Management

The tree tracks which view has focus:

```rust
// Current focused view
let focus_id = cx.editor.tree.focus;
let focused_view = cx.editor.tree.get(focus_id);

// Change focus
cx.editor.tree.focus = some_other_view_id;

// Navigate to adjacent views
cx.editor.tree.left();   // Move focus left
cx.editor.tree.right();  // Move focus right
cx.editor.tree.up();     // Move focus up
cx.editor.tree.down();   // Move focus down
```

---

## 3. COMPONENTS AND COMPOSITOR

### 3.1 Component Trait

All UI elements implement the `Component` trait:

```rust
// From ui/compositor.rs, lines 73-117

pub trait Component: Any + AnyComponent {
  /// Process input events, return true if handled
  fn handle_event(&mut self, event: &Event, ctx: &mut Context) -> EventResult {
    EventResult::Ignored(None)
  }
  
  /// Should redraw?
  fn should_update(&self) -> bool {
    true
  }
  
  /// Render onto the provided surface
  fn render(&mut self, area: Rect, surface: &mut Surface, ctx: &mut Context);
  
  /// Get cursor position and kind
  fn cursor(&self, area: Rect, ctx: &Editor) -> (Option<Position>, CursorKind) {
    (None, CursorKind::Hidden)
  }
  
  /// Required size (for scrollable components)
  fn required_size(&mut self, viewport: (u16, u16)) -> Option<(u16, u16)> {
    None
  }
  
  /// Component identifier (for searching/replacing)
  fn id(&self) -> Option<&'static str> {
    None
  }
  
  /// Whether component is animating
  fn is_animating(&self) -> bool {
    false
  }
}
```

### 3.2 Adding Components to the Application

Components are added to a compositor in layers. The application adds them during initialization:

```rust
// From application.rs, lines 100-135

pub fn new(editor: Editor, ...) -> Self {
  let area = Rect::new(0, 0, 120, 40);
  let mut compositor = Compositor::new(area);
  
  // Layer 1: Editor view (main editor - bottom layer)
  let editor_view = Box::new(EditorView::new(keymaps));
  compositor.push(editor_view);
  
  // Layer 2: Statusline (on top)
  let statusline = Box::new(StatusLine::new());
  compositor.push(statusline);
  
  // Layer 3: Button (optional component example)
  let button = Box::new(Button::new("Run")...);
  compositor.push(button);
  
  Self {
    compositor,
    editor,
    // ... other fields
  }
}
```

**Layers are rendered in order**, with later layers on top (front). Event handling goes **reverse** (top to bottom).

### 3.3 Compositor Operations

```rust
pub struct Compositor {
  pub layers: Vec<Box<dyn Component>>,
  area: Rect,
}

impl Compositor {
  /// Add a new layer on top
  pub fn push(&mut self, layer: Box<dyn Component>) { ... }
  
  /// Remove the topmost layer
  pub fn pop(&mut self) -> Option<Box<dyn Component>> { ... }
  
  /// Remove a component by ID
  pub fn remove(&mut self, id: &'static str) -> Option<Box<dyn Component>> { ... }
  
  /// Replace or push a component
  pub fn replace_or_push<T: Component>(&mut self, id: &'static str, layer: T) { ... }
  
  /// Handle input (bubbles from top to bottom)
  pub fn handle_event(&mut self, event: &Event, cx: &mut Context) -> bool { ... }
  
  /// Render all layers
  pub fn render(&mut self, surface: &mut Surface, cx: &mut Context) { ... }
}
```

### 3.4 Example: Terminal Component (Hypothetical)

To add a terminal as a component (not just as a view):

```rust
// new_file: the-editor/ui/components/terminal.rs

pub struct Terminal {
  id: ViewId,           // Reference to the underlying view
  area: Rect,
  pty_handle: Option<PtyHandle>,
  // ... other fields
}

impl Component for Terminal {
  fn handle_event(&mut self, event: &Event, ctx: &mut Context) -> EventResult {
    match event {
      Event::Key(key) => {
        // Send key to PTY
        if let Some(pty) = &self.pty_handle {
          pty.write_key(key);
        }
        EventResult::Consumed(None)
      },
      _ => EventResult::Ignored(None),
    }
  }
  
  fn render(&mut self, area: Rect, surface: &mut Surface, ctx: &mut Context) {
    self.area = area;
    // Render terminal content from PTY buffer
    // ...
  }
  
  fn id(&self) -> Option<&'static str> {
    Some("terminal")
  }
}
```

However, **the current architecture strongly prefers View-based components** because:
1. They integrate with the split/layout system
2. They have automatic selection/cursor management
3. They work with the document/buffer infrastructure

### 3.5 EventResult and Callbacks

Components can consume events and trigger deferred operations:

```rust
pub enum EventResult {
  Ignored(Option<Callback>),  // Event propagates, optional deferred work
  Consumed(Option<Callback>), // Event handled, optional deferred work
}

pub type Callback = Box<dyn FnOnce(&mut Compositor, &mut Context)>;
```

Example:

```rust
fn handle_event(&mut self, event: &Event, ctx: &mut Context) -> EventResult {
  match event {
    Event::Key(key) => {
      if key.code == KeyCode::Enter {
        // Handle immediately
        self.process_input();
        
        // Defer UI update until after event is fully processed
        let callback = Box::new(|compositor: &mut Compositor, ctx: &mut Context| {
          ctx.editor.set_status("Done!");
        });
        
        return EventResult::Consumed(Some(callback));
      }
    },
    _ => {},
  }
  EventResult::Ignored(None)
}
```

---

## 4. CREATING A TERMINAL FEATURE

Based on the patterns above, here's the recommended approach:

### 4.1 Architecture Choice

**Best approach: Terminal as a View, not a Component**

Why:
- Views are already designed for split management
- They integrate with document/buffer system
- They support selection, scrolling, undo/redo via special buffers
- No need to reinvent split logic

### 4.2 Implementation Outline

**Step 1: Create a terminal command**

```rust
// In core/commands.rs or new file core/terminal.rs

pub fn cmd_terminal_new(cx: &mut Context, args: Args, event: PromptEvent) -> Result<()> {
  if event != PromptEvent::Validate { return Ok(()); }
  
  // Create a new document for terminal
  let doc_id = cx.editor.new_file(Action::VerticalSplit);
  
  // Mark it as a terminal buffer
  cx.editor.mark_special_buffer(doc_id, SpecialBufferKind::Terminal);
  
  // Spawn terminal in background
  cx.jobs.callback(async move {
    // Start PTY process
    let pty = spawn_pty()?;
    
    Ok(LocalCallback::EditorCompositor(Box::new(move |editor, _compositor| {
      // Store PTY handle somewhere accessible from the view
      // This could be in a ThreadLocal or Arc<Mutex<>>
      TERMINAL_PTYS.insert(doc_id, pty);
      editor.set_status("Terminal spawned");
    })))
  });
  
  Ok(())
}
```

**Step 2: Register the command**

```rust
// In CommandRegistry::register_builtin_commands()

self.register(TypableCommand::new(
  "terminal-new",
  &["term", "tn"],
  "Open a new terminal",
  cmd_terminal_new,
  CommandCompleter::none(),
  Signature {
    positionals: (0, Some(0)),
    flags: &[
      Flag {
        name: "layout",
        alias: Some('l'),
        doc: "Split layout: v (vertical) or h (horizontal)",
        completions: Some(&["v", "h"]),
      },
    ],
    ..Signature::DEFAULT
  },
));
```

**Step 3: Handle terminal rendering in EditorView**

The EditorView already renders all views. For terminal views, you'd modify the rendering to:
1. Check if view is a terminal buffer
2. Instead of rendering document text, render PTY output
3. Handle mouse clicks and input differently

**Step 4: Input handling**

Forward key input to the PTY instead of applying editor commands:

```rust
// In EditorView::handle_event() or similar

if is_terminal_view {
  // Send keys to PTY instead of editor command system
  send_to_pty(view_id, key);
} else {
  // Normal editor command handling
  handle_editor_command(key);
}
```

---

## 5. KEY TYPES AND MACROS

### 5.1 Helpful Macros

```rust
// From crate macros

current!(editor)        // Get mutable (&mut View, &mut Document)
current_ref!(editor)    // Get immutable (&View, &Document)
view!(editor)           // Get &View only
view_mut!(editor)       // Get &mut View only
doc!(editor)            // Get &Document only
doc_mut!(editor)        // Get &mut Document only
```

### 5.2 Key Type Hierarchies

```rust
pub type DocumentId = typed_index::TypedIndex<Document>;
pub type ViewId = slotmap::DefaultKey;

pub struct View {
  pub id: ViewId,
  pub doc: DocumentId,      // Which document this view displays
  pub area: Rect,            // Position and size on screen
  pub offset: Position,      // Scroll offset
  pub // ... many other fields
}

pub struct Document {
  pub id: DocumentId,
  pub text: Rope,           // Content
  pub selections: HashMap<ViewId, Selection>, // Per-view selections
  pub history: History,     // Undo/redo
  pub diagnostics: Vec<Diagnostic>,
  pub // ... many other fields
}
```

### 5.3 Special Buffers

Special buffers are marked documents that behave differently:

```rust
pub enum SpecialBufferKind {
  Acp,           // ACP session buffer
  Terminal,      // Terminal buffer
  // Add more as needed
}

// Mark a buffer as special:
editor.mark_special_buffer(doc_id, SpecialBufferKind::Terminal);

// Check if special:
if let Some(kind) = editor.special_buffers.get(&doc_id) {
  // Handle specially
}
```

---

## 6. ASYNC PATTERNS WITH JOBS

The editor uses a `Jobs` queue for async operations:

```rust
// cx.jobs is of type &mut Jobs

// For !Send futures (UI thread required):
cx.jobs.callback_local(async move {
  // Do async work
  let result = some_async_operation().await;
  
  // Return a callback to run on the main thread
  Ok(Some(LocalCallback::EditorCompositor(Box::new(move |editor, compositor| {
    editor.set_status(result);
  }))))
});

// For Send futures (can run on any thread):
cx.jobs.callback(async move {
  // Do async work that doesn't need UI access
  let result = blocking_operation().await;
  
  Ok(Some(Callback::EditorCompositor(Box::new(move |editor, compositor| {
    editor.set_status(result);
  }))))
});
```

---

## 7. COMMON PATTERNS AND ANTIPATTERNS

### Good Patterns:
1. **Use Actions for splits** - `Action::VerticalSplit`, `Action::HorizontalSplit`
2. **Mark special buffers** - So editor knows how to treat them
3. **Use jobs for async work** - Don't block the render thread
4. **Return LocalCallback** - For deferred UI updates from async work
5. **Use tree operations** - Don't manage views directly

### Antipatterns to Avoid:
1. **Don't modify tree directly** - Use tree.split(), tree.insert(), tree.remove()
2. **Don't spawn heavy work on render thread** - Use cx.jobs
3. **Don't create components outside compositor** - They won't be rendered/updated
4. **Don't hardcode UI positions** - Use the layout engine
5. **Don't duplicate split logic** - Tree handles all split complexity

---

## 8. QUICK REFERENCE: ADDING TERMINAL_NEW COMMAND

```rust
// 1. Add function in core/commands.rs or acp/commands.rs
pub fn cmd_terminal_new(cx: &mut Context, args: Args, event: PromptEvent) -> Result<()> {
  if event != PromptEvent::Validate { return Ok(()); }
  
  let doc_id = cx.editor.new_file(Action::VerticalSplit);
  cx.editor.mark_special_buffer(doc_id, SpecialBufferKind::Terminal);
  
  cx.jobs.callback_local(async move {
    // Start terminal
    Ok(Some(LocalCallback::Editor(Box::new(move |editor| {
      editor.set_status("Terminal spawned");
    }))))
  });
  
  Ok(())
}

// 2. Register in CommandRegistry::register_builtin_commands()
self.register(TypableCommand::new(
  "terminal-new",
  &["term"],
  "Open a new terminal",
  cmd_terminal_new,
  CommandCompleter::none(),
  Signature {
    positionals: (0, Some(0)),
    ..Signature::DEFAULT
  },
));

// 3. User can now run: :terminal-new or :term
```

---

## 9. FILE LOCATIONS SUMMARY

- **Commands**: `/home/mister/code/the-editor/the-editor/core/command_registry.rs` (registration)
- **Command impls**: `/home/mister/code/the-editor/the-editor/core/commands.rs` (main), `/home/mister/code/the-editor/the-editor/acp/commands.rs` (examples)
- **Tree/splits**: `/home/mister/code/the-editor/the-editor/core/tree.rs`
- **Layout engine**: `/home/mister/code/the-editor/the-editor/core/layout.rs`
- **Compositor**: `/home/mister/code/the-editor/the-editor/ui/compositor.rs`
- **Application setup**: `/home/mister/code/the-editor/the-editor/application.rs`
- **Views**: `/home/mister/code/the-editor/the-editor/core/view.rs`
- **Documents**: `/home/mister/code/the-editor/the-editor/core/document.rs`
- **Special buffers**: `/home/mister/code/the-editor/the-editor/core/special_buffer.rs`

