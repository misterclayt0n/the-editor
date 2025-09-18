# Repository Guidelines

## Project Structure & Module Organization
This workspace collects crates under `the-editor-*` in the repository root. `the-editor/` is the entrypoint binary, bundling editors, handlers, and runtime glue. Editing primitives live in `the-editor/core/`, rendering engines in `the-editor-renderer/src/`, and shared utilities in `the-editor-stdx/` and `the-editor-event/`. LSP schema definitions sit in `the-editor-lsp-types/types/`. Persisted assets and themes live under `assets/`, `theme.toml`, and `languages.toml`. Keep new modules colocated with the crate that owns the functionality and expose them with the minimal `pub` surface.

## Build, Test, and Development Commands
- `cargo build` — compile the entire workspace; use `cargo build -p <crate>` for a single package.
- `cargo test` — execute all unit and doc tests; pass a substring (e.g. `cargo test movement`) to focus on one suite.
- `cargo run -p the-editor` — launch the editor binary with default assets.
- `cargo clippy --all-targets --all-features` — enforce lint coverage before submitting.
- `cargo fmt --all` — format according to `rustfmt.toml`; run after local edits.

## Coding Style & Naming Conventions
We target Rust 2024 with two-space indentation and a 100-character width. Group imports by standard library, external crates, then local modules, using vertical alignment. Public APIs require explicit types and `snake_case` names; constructors can use `new`. Handle fallible flows with `anyhow::Result` and prefer propagating errors via `?`. Comments should explain intent, not restate code, and remain brief.

## Testing Guidelines
Inline `#[cfg(test)]` modules live beside their implementations in each crate; mirror that pattern for new coverage. Favor descriptive `fn name_when_condition_then_expectation()` test names. Integration tests may reside under `tests/` when touching public APIs, but ensure fixtures stay small. Run `cargo test` before opening a PR and document any skipped scenarios.

## Commit & Pull Request Guidelines
Commits in history typically follow `<crate>: <present-tense summary>` (e.g. `core: tighten selection API`). Keep individual commits focused and run formatting/lints beforehand. Pull requests should describe the motivation, enumerate key changes, and reference related issues or discussion threads. Include repro steps, test commands, and screenshots when UX-facing. Mark TODOs explicitly and file follow-ups instead of leaving unfinished code paths.

## Configuration Tips
Default themes and language settings derive from `theme.toml`, `base16_theme.toml`, and `languages.toml`; update these alongside renderer or syntax changes. Use `taplo fmt languages.toml` if editing language metadata, and ensure new assets land under `assets/` with clear naming.
