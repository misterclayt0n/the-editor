# AGENTS.md

Guidance for AI coding agents working on this repository.

## Project Overview

Modern text editor written in Rust (inspired by Helix). Workspace structure:

- **the-editor**: Main binary - core logic, UI, LSP, input handling
- **the-renderer**: GPU renderer using wgpu/winit
- **the-event**: Event system with hooks, cancellation, debouncing
- **the-loader**: Tree-sitter grammar/config loading
- **the-lsp-types**: LSP protocol types
- **the-parsec**: Parsing utilities
- **the-stdx**: Standard library extensions
- **the-vcs**: Version control (git, diff)
- **the-terminal**: Terminal emulator

## Build & Development Commands

```bash
# Build
cargo build                          # entire workspace
cargo build -p the-editor            # specific crate

# Run
cargo run -p the-editor

# Test
cargo nextest r --features unicode-lines     # all tests
cargo nextest r <pattern>                    # match test name
cargo nextest r -p the-editor                # specific crate
cargo nextest r -p the-editor <test_name>    # single test in crate

# Lint & Format (required before commits)
cargo fmt --all
cargo clippy --all-targets --all-features
cargo check                          # verify compilation

# Format TOML config files
taplo fmt languages.toml
```

## Code Style

### Formatting (.rustfmt.toml)
- **Edition**: 2024
- **Indentation**: 2 spaces (`tab_spaces = 2`)
- **Line width**: 100 characters
- **Imports**: Vertical layout, grouped (std, external, local)

### Import Organization
```rust
use std::{
  collections::HashMap,
  path::Path,
};

use anyhow::Result;
use tokio::sync::mpsc;

use crate::{
  core::document::Document,
  editor::Editor,
};
```

### Naming Conventions
- Functions/variables: `snake_case`
- Types/traits: `PascalCase`
- Constants: `SCREAMING_SNAKE_CASE`

### Error Handling
- Use `anyhow::Result` for fallible operations
- Propagate errors with `?`
- Use `thiserror` for custom error types in libraries

### Tests
- Inline `#[cfg(test)]` modules alongside implementation
- Naming: `fn test_<what>_<condition>()` or descriptive names
```rust
#[cfg(test)]
mod test {
  use super::*;

  #[test]
  fn test_create_normalizes_and_merges() { ... }
}
```

## Version Control (Jujutsu)

This project uses Jujutsu (jj), not git. **Never push to upstream.**

### Commit Format
```
<context>: <message>
```
Examples: `renderer: optimize text cache`, `editor: fix theme transition`

Keep messages lowercase, no periods.

### Workflow
```bash
jj status                    # check changes
jj describe -m "ctx: msg"    # set commit message
jj new                       # create new commit
jj log                       # view history
```

### Rules
- **Every completed TODO = one commit**
- **Never commit code that doesn't compile** - always run `cargo check` first
- **Never push to upstream**
- **Ask before rebasing**

## Key Architecture

### Entry Point
`the-editor/main.rs` -> registers events -> spawns Tokio runtime -> initializes loaders -> creates Editor -> launches renderer

### Core Components
- `editor.rs`: Top-level Editor struct
- `core/document.rs`: Document model (Rope + syntax + history)
- `core/view.rs`: View with cursors and scroll
- `core/selection.rs`: Multi-cursor selection
- `core/commands.rs`: All editing operations
- `core/layout.rs`: Flexible UI layout system

### Event System (the-event)
```rust
// Dispatch events
dispatch!(EventType { field: value });

// Register hooks
hook!(move |event: EventType| { ... });
```

### LSP
- `lsp.rs`: Client management
- `lsp/client.rs`: Per-server client
- `lsp/transport.rs`: stdio transport

## NO GAMBIARRA POLICY

Keep code high quality, clean, simple, modular, and fast. Reference: dtolnay crates, tracing.

**If a task seems too hard:**
1. Do NOT implement a half-baked solution
2. Leave code unchanged
3. Explain the situation honestly
4. Ask for feedback and clarification

The user is a domain expert who can help find proper solutions.

## Quick Reference

| Task | Command |
|------|---------|
| Build | `cargo build` |
| Run | `cargo run -p the-editor` |
| Test all | `cargo nextest r --features unicode-lines` |
| Test one | `cargo nextest r <test_name>` |
| Format | `cargo fmt --all` |
| Lint | `cargo clippy --all-targets --all-features` |
| Check | `cargo check` |
| Commit | `jj describe -m "ctx: msg" && jj new` |
