# The Editor - Agent Guidelines

## Build Commands
- `cargo build` - Build the project
- `cargo test` - Run all tests
- `cargo test [TESTNAME]` - Run specific test containing TESTNAME
- `cargo clippy` - Run linter for common mistakes
- `cargo fmt` - Format code according to rustfmt.toml

## Code Style
- **Rust Edition**: 2024
- **Formatting**: 2 spaces, 100 char max width, Unix line endings
- **Imports**: Group by std/external/crate, use vertical layout
- **Error Handling**: Use `anyhow::Result` for fallible operations
- **Function Style**: `pub fn name()` with snake_case naming
- **Comments**: Avoid unless necessary, use `//` for inline
- **Types**: Prefer explicit types for public APIs
- **Dependencies**: Check Cargo.toml before adding new crates

## Project Structure
- Workspace with multiple crates (the-editor-*)
- Core functionality in `the-editor/core/`
- LSP types in `the-editor-lsp-types/`
- Rendering in `the-editor-renderer/`