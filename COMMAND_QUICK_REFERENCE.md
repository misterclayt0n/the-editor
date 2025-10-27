# Quick Reference: Adding Commands to The Editor

## Step 1: Define Command Function

```rust
pub fn cmd_your_feature(cx: &mut Context, args: Args, event: PromptEvent) -> Result<()> {
  // Only execute on Enter, not while typing
  if event != PromptEvent::Validate {
    return Ok(());
  }

  // Your implementation here
  let doc_id = cx.editor.new_file(Action::VerticalSplit);
  
  // Mark as special if needed
  cx.editor.mark_special_buffer(doc_id, SpecialBufferKind::Acp);
  
  // Async work (non-blocking)
  cx.jobs.callback_local(async move {
    // Do async stuff
    Ok(Some(LocalCallback::Editor(Box::new(|editor| {
      editor.set_status("Done!");
    }))))
  });
  
  Ok(())
}
```

## Step 2: Register in CommandRegistry

**File:** `/home/mister/code/the-editor/the-editor/core/command_registry.rs`

**Location:** Inside `CommandRegistry::register_builtin_commands()` method (~line 462-875)

```rust
self.register(TypableCommand::new(
  "your-command",              // Primary name
  &["yc", "shortcut"],         // Aliases
  "Brief description",          // Documentation
  cmd_your_feature,             // Function pointer
  CommandCompleter::none(),     // Argument completion
  Signature {
    positionals: (0, Some(0)),  // (min_args, max_args)
    flags: &[
      Flag {
        name: "layout",
        alias: Some('l'),
        doc: "Choose layout",
        completions: Some(&["vertical", "horizontal"]),
      },
    ],
    ..Signature::DEFAULT
  },
));
```

## Available Command Helpers

```rust
// Arguments access
args.is_empty()                    // Check if no args provided
args[0]                            // Get first positional arg
args.len()                         // Number of positional args
args.has_flag("name")              // Check if flag exists
args.get_flag("name")              // Get flag value (Option<String>)

// Context access
cx.editor                          // Full editor state
cx.editor.tree                     // View tree for splits
cx.editor.documents                // All documents
cx.editor.new_file(action)         // Create new document
cx.editor.switch(doc_id, action)   // Switch/split document
cx.editor.open(path, action)       // Open file
cx.editor.set_status(msg)          // Show status message
cx.editor.set_error(msg)           // Show error message

cx.jobs.callback(async_block)      // Queue async work
cx.jobs.callback_local(async_block) // Queue UI-thread async work

// Actions for splits
Action::Load                       // Load in current view
Action::Replace                    // Replace current document
Action::HorizontalSplit            // Split horizontally
Action::VerticalSplit              // Split vertically

// Special buffers
SpecialBufferKind::Acp             // ACP session
SpecialBufferKind::Terminal        // Terminal (if implemented)

// Mark document as special
cx.editor.mark_special_buffer(doc_id, kind)

// Macros
current!(editor)                   // (&mut View, &mut Document)
current_ref!(editor)               // (&View, &Document)
view!(editor)                      // &View
view_mut!(editor)                  // &mut View
doc!(editor)                       // &Document
doc_mut!(editor)                   // &mut Document
```

## Command Signature

```rust
pub type CommandFn = fn(&mut Context, Args, PromptEvent) -> Result<()>;

pub enum PromptEvent {
  Update,      // User typing - use for live previews
  Validate,    // User pressed Enter - execute command
  Abort,       // User pressed Escape - revert preview
}
```

## Accessing Tree and Views

```rust
// Focused view and document
let (view, doc) = current_ref!(cx.editor);

// All views with their documents
for (view, doc) in cx.editor.tree.views() {
  println!("View {:?} shows doc {:?}", view.id, view.doc);
}

// Access specific view by ID
let view = cx.editor.tree.get(view_id);
let view_mut = cx.editor.tree.get_mut(view_id);

// Focus a different view
cx.editor.tree.focus = another_view_id;

// Navigation
cx.editor.tree.left();   // Move focus left
cx.editor.tree.right();  // Move focus right
cx.editor.tree.up();     // Move focus up
cx.editor.tree.down();   // Move focus down
```

## Example: Split Command Pattern

```rust
fn cmd_split_feature(cx: &mut Context, args: Args, event: PromptEvent) -> Result<()> {
  if event != PromptEvent::Validate { return Ok(()); }
  
  // Get current document
  let (view, doc) = current_ref!(cx.editor);
  let doc_id = view.doc;
  
  // Get split direction from args or default
  let action = if args.len() > 0 && args[0] == "h" {
    Action::HorizontalSplit
  } else {
    Action::VerticalSplit
  };
  
  // Create split with the same document
  cx.editor.switch(doc_id, action);
  
  Ok(())
}
```

## Example: Terminal Command Pattern

```rust
fn cmd_terminal_new(cx: &mut Context, args: Args, event: PromptEvent) -> Result<()> {
  if event != PromptEvent::Validate { return Ok(()); }
  
  // Create new document in split
  let doc_id = cx.editor.new_file(Action::VerticalSplit);
  
  // Mark as terminal
  cx.editor.mark_special_buffer(doc_id, SpecialBufferKind::Terminal);
  
  // Spawn PTY in background
  cx.jobs.callback_local(async move {
    match spawn_pty().await {
      Ok(pty) => {
        Ok(Some(LocalCallback::Editor(Box::new(move |editor| {
          editor.set_status("Terminal ready");
        }))))
      },
      Err(e) => {
        Ok(Some(LocalCallback::Editor(Box::new(move |editor| {
          editor.set_error(format!("Failed to spawn terminal: {}", e));
        }))))
      },
    }
  });
  
  Ok(())
}
```

## Error Handling Pattern

```rust
fn cmd_with_errors(cx: &mut Context, args: Args, event: PromptEvent) -> Result<()> {
  if event != PromptEvent::Validate { return Ok(()); }
  
  // Return errors with context
  if args.is_empty() {
    return Err(anyhow!("command requires at least one argument"));
  }
  
  match cx.editor.open(&args[0].into(), Action::Replace) {
    Ok(doc_id) => {
      cx.editor.set_status(format!("Opened: {}", args[0]));
    },
    Err(err) => {
      cx.editor.set_error(format!("Failed to open: {}", err));
    },
  }
  
  Ok(())
}
```

## Flag Parsing Pattern

```rust
const FLAGS: &[Flag] = &[
  Flag {
    name: "force",
    alias: Some('f'),
    doc: "Force operation",
    completions: None,
  },
  Flag {
    name: "output",
    alias: Some('o'),
    doc: "Output file",
    completions: Some(&["file.txt"]),
  },
];

fn cmd_with_flags(cx: &mut Context, args: Args, event: PromptEvent) -> Result<()> {
  if event != PromptEvent::Validate { return Ok(()); }
  
  // Check for flag
  let force = args.has_flag("force");
  
  // Get flag value
  if let Some(output_file) = args.get_flag("output") {
    cx.editor.set_status(format!("Output: {}", output_file));
  }
  
  // Process positional args
  for arg in args.iter() {
    // ...
  }
  
  Ok(())
}
```

## Common Patterns

### Creating a Document
```rust
let doc_id = cx.editor.new_file(Action::VerticalSplit);
let doc = doc_mut!(cx.editor, &doc_id);
```

### Opening a File
```rust
let doc_id = cx.editor.open(&path, Action::Replace)?;
cx.editor.set_status(format!("Opened: {}", path.display()));
```

### Getting Current Document
```rust
let (view, doc) = current_ref!(cx.editor);
let doc_id = view.doc;
```

### Running Async Code
```rust
cx.jobs.callback_local(async move {
  let result = some_async_fn().await;
  Ok(Some(LocalCallback::Editor(Box::new(move |editor| {
    editor.set_status(result.to_string());
  }))))
});
```

### Error Message
```rust
cx.editor.set_error("Something went wrong".to_string());
```

### Status Message
```rust
cx.editor.set_status("Operation complete".to_string());
```

---

## File Locations

- **Register commands:** `/home/mister/code/the-editor/the-editor/core/command_registry.rs` (line 462)
- **Command implementations:** `/home/mister/code/the-editor/the-editor/core/commands.rs`
- **Alternative location:** `/home/mister/code/the-editor/the-editor/acp/commands.rs` (example of separate module)
- **Tree operations:** `/home/mister/code/the-editor/the-editor/core/tree.rs`
- **Documents:** `/home/mister/code/the-editor/the-editor/core/document.rs`
- **Views:** `/home/mister/code/the-editor/the-editor/core/view.rs`

---

## Testing Your Command

After registering, you can test by running:

```
:your-command              # Execute with no args
:your-command arg1 arg2    # With positional args
:your-command --flag arg   # With flags
:your-command --flag=value # Flag with value
```

Use `:help your-command` to see generated documentation.

