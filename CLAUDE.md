# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is a modern text editor written in Rust, inspired by Helix. The project uses a workspace structure with multiple crates, each responsible for different aspects of the editor:

- **the-editor**: Main binary crate containing the editor core logic, UI components, LSP integration, and input handling
- **the-renderer**: GPU-backed renderer using wgpu and winit for graphics and windowing
- **the-event**: Event system with cancellation, debouncing, hooks, and async runtime integration
- **the-loader**: Language grammar and configuration loading (tree-sitter integration)
- **the-lsp-types**: LSP protocol type definitions
- **the-parsec**: Parsing utilities
- **the-stdx**: Standard library extensions (path utilities, rope helpers, file access)
- **the-vcs**: Version control system integration (git, diff providers)

## Build and Development Commands

```bash
# Build entire workspace
cargo build

# Build specific crate
cargo build -p the-editor

# Run the editor
cargo run -p the-editor

# Run tests (all crates)
cargo test

# Run tests matching a pattern
cargo test <pattern>
# Example: cargo test movement

# Run tests for specific crate
cargo test -p the-editor

# Format code (required before commits)
cargo fmt --all

# Run lints (required before commits)
cargo clippy --all-targets --all-features

# Build with Nix (if using Nix)
nix build
```

## Code Architecture

### Main Application Flow

The entry point is `the-editor/main.rs`, which:
1. Registers event types and hooks via `event::register_all_events()`
2. Spawns a Tokio multi-threaded runtime for async operations (LSP, word indexing)
3. Initializes theme and language loaders from `runtime/` directories
4. Constructs the `Editor` with handlers (completion, signatures, word index, diagnostics)
5. Creates the `App` wrapper which integrates the compositor and input handling
6. Launches the renderer event loop via `the_editor_renderer::run()`

### Core Editor Components

- **`editor.rs`**: Top-level `Editor` struct managing documents, views, language server clients, theme, clipboard, registers, and job queue
- **`core/document.rs`**: Document model wrapping a `Rope` with syntax highlighting, diagnostics, history, and LSP integration
- **`core/view.rs`**: View into a document with its own cursor(s), scroll offset, and gutter state
- **`core/selection.rs`**: Multi-cursor selection and range logic
- **`core/commands.rs`**: Command implementations for all editing operations
- **`core/syntax.rs`**: Tree-sitter integration for syntax highlighting and queries

### UI and Rendering

- **`ui/compositor.rs`**: Component trait and compositor for layering UI elements (editor views, prompts, pickers)
- **`ui/editor_view.rs`**: Main editor view rendering (text, cursors, gutters, diagnostics)
- **`ui/components/`**: Reusable UI components (statusline, picker, prompt, debug panel, buttons)
- **`application.rs`**: `App` struct implementing the `Application` trait, handling input events and frame rendering
- **Renderer architecture**: `the-renderer` provides a `Renderer` that handles text layout with `wgpu_text`, rectangle drawing, and blur effects. It exposes an `Application` trait with `init()`, `render()`, `handle_event()`, `resize()`, and `wants_redraw()`.

### LSP Integration

- **`lsp.rs`**: High-level LSP client management (spawning, capability detection, document sync)
- **`lsp/client.rs`**: Per-server LSP client with request/response handling
- **`lsp/jsonrpc.rs`**: JSON-RPC protocol implementation
- **`lsp/transport.rs`**: stdio transport for language servers
- **`handlers/lsp.rs`**: Hooks for LSP events (diagnostics, completion, etc.)

### Input and Keymaps

- **`input.rs`**: Input event handling and keymap resolution
- **`keymap/default.rs`**: Default Vim-like keybindings
- **`keymap/macros.rs`**: Macro system for recording and replaying key sequences

### Event System

The `the-event` crate provides:
- **Event registration and dispatch**: Type-safe event emission with `dispatch!(EventType { ... })`
- **Hooks**: Register listeners for events with `hook!(move |event: EventType| { ... })`
- **Cancellation**: Events can be cancelled by returning `Status::Cancelled` from hooks
- **Debouncing**: Automatic debouncing for high-frequency events

### Configuration and Assets

- **`runtime/themes/`**: Theme definitions (TOML)
- **`runtime/queries/`**: Tree-sitter query files for syntax highlighting
- **`languages.toml`**: Language definitions (file types, language servers, formatters, tree-sitter grammars)
- **`theme.toml`, `base16_theme.toml`**: User themes
- **`~/.config/the-editor/config.toml`**: User configuration (loaded in `core/config.rs`)

## Coding Conventions

- **Rust edition**: 2024
- **Formatting**: Use `.rustfmt.toml` settings (2-space indentation, 100-char width, vertical imports)
- **Import organization**: Group by std, external crates, then local modules; use `imports_layout = "Vertical"`
- **Error handling**: Use `anyhow::Result` for fallible operations; propagate with `?`
- **Naming**: `snake_case` for functions/variables, `PascalCase` for types
- **Tests**: Inline `#[cfg(test)]` modules; use `fn test_name_when_condition_then_expectation()` pattern
- **Comments**: Explain intent, not mechanics; keep concise

## Testing

- Tests live in `#[cfg(test)]` modules alongside implementation
- Run `cargo test` before submitting PRs
- Use `cargo test <pattern>` to run specific tests

## Commit Guidelines

- Format: `<crate>: <present-tense summary>`
  - Example: `core: tighten selection API`
  - Example: `renderer: optimize text layout cache`
- Run `cargo fmt --all` and `cargo clippy --all-targets --all-features` before committing

## Configuration Editing

- Edit `languages.toml` for language definitions; run `taplo fmt languages.toml` after changes
- Theme files are TOML; add new themes to `runtime/themes/`
- Assets go in `assets/` with clear naming

### NO GAMBIARRA POLICY - ASK FOR FEEDBACK INSTEAD
Due to the difficulty of implementing this codebase, we must strive to keep
the code high quality, clean (not clean code), simple, modular, functional
and super fast - More like a professional rust codebase (see dtolnay crates,
or lib.rs/tracing for reference as to what this looks like).
Gambiarras, hacks and duct taping must be COMPLETELY AVOIDED, in favor of
robust, simple and general solutions.

In some cases, you will be asked to perform a seemingly impossible task, either
because it is (and the user is unaware), or because you don't grasp how to do it
correclty. In these cases, DO NOT ATTEMPT TO IMPLEMENT A HALF-BAKED SOLUTION JUST
TO SATISFY THE USER'S REQUEST. If the task seems too hard, be honest that you could
not solve it in the proper way, leave the code unchanged, explain the situation to
the user and ask for further feedback and clarifications.

The user is a domain expert and will be able to not only assist, but to also suggest
and think about the proper solutions in these cases.


