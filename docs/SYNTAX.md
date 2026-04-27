# Syntax Module Architecture

This document defines the target architecture for the syntax subsystem in `the-lib`.

It answers four questions:

1. What belongs in the pure syntax core?
2. What belongs in runtime-backed syntax infrastructure?
3. What should the document-facing syntax state model look like?
4. How should the old `syntax.rs` and `syntax/` modules be split and rewritten?

This document is intentionally opinionated. Syntax is one of the biggest and most foundational subsystems in the editor. If its boundaries are vague, a large part of the engine will inherit those mistakes.

---

## Purpose

The syntax subsystem defines the native, document-owned model for:
- language assignment and detection
- tree-sitter-backed syntax state
- incremental parse lifecycle
- lazy compilation of language query helpers
- pure language metadata needed by text-bound editor features

It is responsible for:
- language registry/detection
- document-local syntax truth
- parser-backend adapter logic
- query compilation and caching
- pure syntax-derived query capabilities

It is **not** responsible for:
- runtime grammar discovery or filesystem asset loading
- LSP/formatter/debugger/tool configuration
- task orchestration or background worker management
- render caching/composition policy
- theme/style mapping
- feature algorithms like indentation, text object expansion, comment toggling, or auto-pair behavior

---

## Core position

Syntax is **document-owned optional derived state**.

That means:
- syntax belongs to the `Document`, not the `Surface`
- many surfaces may share one document and therefore one syntax truth
- syntax state must remain independent of selection, viewport, and render caches

This aligns with the architecture in `docs/ARCHITECTURE.md`:
- syntax is about the text itself
- grammar loading and async orchestration stay out of `the-lib`
- the pure engine owns deterministic syntax state and text-bound query capabilities

---

## Main decisions

### 1. Split syntax core from runtime-backed resource acquisition
**Decision:** yes.

### `the-lib` owns
- pure language metadata
- language detection rules
- syntax state
- lazy query compilation
- parser adapter code
- pure query helpers

### `the-runtime` owns
- runtime grammar acquisition
- runtime query file loading
- filesystem/resource/session oriented syntax infrastructure

**Reasoning:**
Syntax truth is pure and document-bound. Asset discovery and runtime loader policy are not.

---

### 2. Keep syntax-side language metadata broader than parse mechanics
**Decision:** yes.

`the-lib::syntax` should remain the home of **pure language metadata**, not just parser/query mechanics.

Keep fields like:
- `comment_tokens`
- `block_comment_tokens`
- `indent`
- `auto_pairs`
- `text_width`
- `soft_wrap`
- `rulers`
- `rainbow_brackets`

These are not parser state, but they are deterministic language-local editor defaults and belong in the pure engine.

Do **not** keep runtime/service/tool config here.

---

### 3. Use an editor-owned `LanguageId`
**Decision:** yes.

Use a small editor-owned language identity type publicly.

```rust
pub struct LanguageId(u32);
```

**Reasoning:**
The syntax registry’s language identity is an editor concept, not a third-party backend concept. Public syntax APIs should not be built around raw tree-house language handles.

---

### 4. Rename `Loader` to `LanguageRegistry`
**Decision:** yes.

The old `Loader` is conceptually a registry, not an asset loader.

`LanguageRegistry` should own:
- registered language definitions
- detection rules
- lazy compiled syntax/query helpers
- semantic highlight vocabulary

It should be conceptually immutable after construction, aside from lazy caches.

---

### 5. Rename `LanguageData` to `LanguageDefinition`
**Decision:** yes.

Each registered language should have one stored per-language record that owns:
- pure language metadata/config
- lazily compiled syntax config
- lazily compiled query helpers

This keeps per-language state local and easy to reason about.

---

### 6. Keep compiled query helpers under syntax
**Decision:** yes.

Syntax should centrally own compiled query helpers for:
- indent
- text objects
- tags
- rainbow queries
- highlights / injections / locals

But these should be understood as **query helpers**, not as syntax-state identity itself.

Feature algorithms consuming them stay in their own modules.

---

### 7. Keep syntax state document-scoped and selection-free
**Decision:** yes.

Syntax APIs must stay rooted in:
- text
- language assignment
- derived parse/query state

They must not own:
- selection state
- surface/view state
- render caches
- resource/file identity

Downstream feature modules may consume both syntax and selection, but syntax itself remains document-scoped.

---

### 8. Keep a rich `Syntax` capability object
**Decision:** yes.

`Syntax` may remain a fairly rich parse/query/tree capability object.

It can expose:
- tree access
- descendant lookup
- query iteration
- update/interpolate APIs
- typed helper methods like `tags(...)`

The goal is not to wrap every capability behind minimal façade methods. The goal is to keep top-level editor concepts editor-owned while allowing selective low-level power.

---

### 9. Selectively expose raw tree-house/tree-sitter types
**Decision:** yes.

It is acceptable for syntax APIs to expose some raw backend types where that is the most honest interface.

Examples that may remain visible:
- `Node`
- `Tree`
- `TreeCursor`
- `Query`
- `InputEdit`

But top-level engine concepts should stay editor-owned:
- `LanguageId`
- `LanguageRegistry`
- `Syntax`
- `SyntaxState`
- `DocumentSyntax`

---

### 10. Keep the full pure detection stack
**Decision:** yes.

Preserve language detection by:
- name
- scope
- filename/extension
- glob
- shebang
- injection/content regex

This detection logic is pure and belongs in `the-lib`.

---

### 11. Define one canonical detection precedence order
**Decision:** yes.

Preferred precedence:
1. explicit override / explicit assignment
2. name
3. scope
4. exact filename / extension
5. shebang
6. glob
7. injection/content regex

This should be the canonical order for higher-level document assignment helpers.

---

### 12. Use category-specific registry validation
**Decision:** yes.

Strictly reject conflicts for:
- duplicate language names / ids
- duplicate scopes
- duplicate exact extensions
- duplicate exact shebangs

Allow overlap with explicit precedence for:
- globs (longest match wins)
- injection/content regexes (longest match wins)

This is the right balance of safety and practicality.

---

### 13. Keep inherited query expansion in the registry layer
**Decision:** yes.

`SyntaxResources` should remain dumb and provide only raw query text.
The registry should own query inheritance/composition expansion before compilation.

This keeps resources simple and syntax semantics centralized.

---

### 14. Keep `QueryKind` as a central syntax enum
**Decision:** yes.

Use one central `QueryKind` enum for all supported language query assets:
- `Highlights`
- `Injections`
- `Locals`
- `Indents`
- `TextObjects`
- `Tags`
- `Rainbows`

This gives a canonical vocabulary to the registry/resource/query plumbing.

---

### 15. Keep injections first-class
**Decision:** yes.

Tree-sitter injection parsing remains part of syntax truth and should stay first-class.

Keep:
- layered parsing
- registry-based injection language resolution
- injection enable/disable capability

Do not reduce syntax to single-language trees just to simplify the port.

---

### 16. Keep injection enable/disable as a syntax capability, not a syntax policy
**Decision:** yes.

`Syntax` may expose:
- `injections_enabled()`
- `set_injections_enabled(bool)`

But policy for when to toggle injections stays outside syntax.

---

### 17. Keep syntax/query compilation errors explicit in lazy caches
**Decision:** yes.

Per-language lazy compiled state should remember:
- `Ok(None)` for absent query/config
- `Err(SyntaxError)` for broken query/config

Do **not** silently collapse broken queries into cached `None`.

This makes core syntax behavior much easier to debug and reason about.

---

### 18. Keep parser adapter code under syntax ownership
**Decision:** yes.

Conversion from `ChangeSet`/`Transaction` into tree-sitter `InputEdit`s belongs under syntax ownership as parser-backend adapter logic.

It should **not** live in `transaction.rs`.

This code should likely live in a dedicated syntax adapter module such as `syntax/edits.rs`.

---

### 19. Split highlight production from highlight presentation strategy
**Decision:** yes.

Syntax truth includes:
- parse tree
- query results capability
- semantic highlight production

Presentation strategy includes:
- highlight cache
- overlay composition
- render-specific refresh/caching policy
- semantic highlight -> style mapping

`HighlightCache`, `OverlayHighlights`, `OverlayHighlighter`, and similar presentation-facing utilities should move out of core syntax truth.

---

### 20. Use semantic highlight IDs, not final style IDs
**Decision:** yes.

Syntax should emit semantic highlight identities, not final presentation slot IDs.

Preferred direction:
- syntax registry owns a stable semantic highlight vocabulary
- syntax emits semantic `HighlightKindId`
- render/theme code later maps `HighlightKindId -> StyleId`

This preserves the semantic/presentation split cleanly.

---

### 21. Move mutable highlight scope/style mapping out of the registry
**Decision:** yes.

The old mutable recognized-scope mapping (`set_scopes(...)` / `reconfigure_highlights(...)`) should not remain central to the syntax registry.

Mutable presentation vocabulary is not syntax truth.

The registry may still own semantic highlight vocabulary, but render/theme code should own the later style mapping.

---

### 22. Keep `SyntaxState` in `the-lib`
**Decision:** yes.

Keep a versioned, thread-safe `SyntaxState` wrapper in `the-lib`, as long as it remains pure state.

It may own:
- versions
- parse-pending bit
- snapshot/apply-parsed protocol
- stale parse rejection

It must not own:
- task spawning
- worker orchestration
- runtime scheduler logic

---

### 23. Keep `SyntaxSnapshot` public
**Decision:** yes.

The snapshot/apply-parsed background parsing handshake is public behavior and should use a concrete public payload type.

`SyntaxSnapshot` should remain a small public type.

---

### 24. Keep both update API layers
**Decision:** yes.

Preserve both:
- convenience update/interpolate methods from `ChangeSet`
- lower-level update/interpolate methods from precomputed `InputEdit`s

This keeps the syntax API flexible for both engine and background-parse workflows.

---

### 25. Keep generic query iteration public
**Decision:** yes.

Preserve a generic query-iteration capability on `Syntax` in addition to specific convenience helpers.

This is a genuine capability and should not be hidden just to force feature-specific helper proliferation.

---

### 26. Keep thin typed helpers like `tags(...)`
**Decision:** yes.

Thin typed helper methods that sit naturally atop compiled query helpers are still worthwhile.

Examples:
- `tags(...)`

These should remain thin conveniences, not sprawling feature algorithms.

---

### 27. Move rainbow highlight generation out of core syntax
**Decision:** yes.

Keep:
- `RainbowQuery`

Move out:
- presentation-facing rainbow highlight overlay generation

That output is closer to feature/render adaptation than syntax truth.

---

### 28. Keep feature algorithm ownership outside syntax
**Decision:** yes.

Syntax owns:
- pure language facts
- compiled query helpers
- query/tree capabilities

Other modules own behavior:
- `indent.rs` computes indentation decisions
- `text_object.rs` computes text object behavior
- `comment.rs` computes commenting behavior
- `auto_pairs.rs` computes pair behavior
- movement/render features consume syntax as needed

This is a key anti-god-module boundary.

---

## Document-facing syntax model

A document’s syntax state should be explicit and honest.

### `DocumentSyntax`
Preferred shape:

```rust
pub enum DocumentSyntax {
  Unassigned,
  Assigned {
    assignment: LanguageAssignment,
    state:      DocumentSyntaxState,
  },
}
```

### `LanguageAssignment`
Preferred shape:

```rust
pub struct LanguageAssignment {
  pub language_id: LanguageId,
  pub source:      LanguageAssignmentSource,
}
```

### `LanguageAssignmentSource`
Preferred cases:

```rust
pub enum LanguageAssignmentSource {
  UserOverride,
  Name,
  Scope,
  Filename,
  Shebang,
  Glob,
  InjectionRegex,
}
```

### `DocumentSyntaxState`
Preferred shape:

```rust
pub enum DocumentSyntaxState {
  Deferred,
  Unavailable(SyntaxUnavailableReason),
  Active(SyntaxState),
}
```

### `SyntaxUnavailableReason`
Preferred shape:

```rust
pub enum SyntaxUnavailableReason {
  MissingGrammar,
  InvalidSyntaxConfig,
  InvalidQuery { kind: QueryKind },
  BackendInit,
}
```

### Important semantic rules
- assignment and syntax activation are separate steps
- `Active` means a real initialized syntax tree already exists
- `Deferred` means syntax is not active by choice/policy
- `Unavailable(...)` means activation failed for a structured reason
- when active syntax exists, its `LanguageId` must match the assigned language

---

## `Syntax` and `SyntaxState`

### `Syntax`
`Syntax` remains the core mutable parse/query object.

It may expose:
- `new`
- `update`
- `update_with_edits`
- `interpolate`
- `interpolate_with_edits`
- tree/layer access
- descendant/child lookups
- query iteration
- typed helpers like `tags(...)`
- injection enable/disable

It is allowed to remain rich.

### `SyntaxState`
`SyntaxState` is a higher-level wrapper around `Syntax` for versioned document-facing lifecycle semantics.

It should own:
- active `Syntax`
- `LanguageId`
- interpolated version
- parsed version
- parse-pending state
- snapshot/apply-parsed handshake
- stale parse rejection

It should publicly expose useful pure state like:
- `is_accurate()`
- version getters
- `is_parse_pending()`

---

## Error model

Use a split error model.

### Editor-owned `SyntaxError` is appropriate for:
- missing grammar
- invalid syntax config compilation
- invalid query compilation
- registry/config/resource interpretation failures

### Raw/near-raw backend errors are acceptable for:
- `Syntax::new`
- `Syntax::update`
- query/highlighter operational work
- backend timeouts

Also:
- keep rich errors at operation boundaries
- keep persistent `DocumentSyntax` unavailable reasons stable and normalized

---

## Config split

### Keep in syntax-side pure language metadata
Use narrower names like:
- `SyntaxConfiguration`
- `LanguageSyntaxConfig`

Keep fields like:
- `language_id`
- `scope`
- `file_types`
- `shebangs`
- `comment_tokens`
- `block_comment_tokens`
- `grammar`
- `injection_regex`
- `indent`
- `auto_pairs`
- `text_width`
- `soft_wrap`
- `rulers`
- `rainbow_brackets`

### Move out entirely
Do **not** keep in syntax core:
- language server config
- formatter config
- debugger config
- workspace LSP roots
- persistent diagnostic source config

Those belong in runtime/higher-layer language service configuration.

---

## `SyntaxResources`

Keep the acquisition trait very small and dumb.

Preferred shape:

```rust
pub trait SyntaxResources: Send + Sync {
  fn grammar(&self, grammar_name: &str) -> Option<Grammar>;
  fn query(&self, language_id: &str, kind: QueryKind) -> Option<Cow<'_, str>>;
}
```

Keep in `the-lib`:
- `SyntaxResources`
- `NullResources`

Move out of `the-lib` entirely:
- runtime-backed resource loaders/implementations

These belong in `the-runtime`.

---

## Query helper ownership

Keep compiled query helper types under syntax:
- `IndentQuery`
- `TextObjectQuery`
- `TagQuery`
- `RainbowQuery`

Keep explicit typed registry accessors:
- `indent_query(language_id)`
- `textobject_query(language_id)`
- `tag_query(language_id)`
- `rainbow_query(language_id)`

Do not replace these with a generic type-erased query helper retrieval abstraction.

---

## Module structure

Do **not** rebuild another giant `syntax.rs` god file.

Preferred structure:

```txt
the-lib/syntax/
  mod.rs
  config.rs
  resources.rs
  registry.rs
  state.rs
  edits.rs
  indent_query.rs
  textobject_query.rs
  tag_query.rs
  rainbow_query.rs
  highlighting.rs   # pure semantic highlight production only
```

### `mod.rs` façade discipline
Use a small façade that re-exports the intentional public surface while keeping internals private by default.

Good public re-exports include:
- `LanguageId`
- `LanguageRegistry`
- `LanguageDefinition`
- `Syntax`
- `SyntaxState`
- `SyntaxSnapshot`
- `DocumentSyntax`
- `LanguageAssignment`
- `LanguageAssignmentSource`
- `SyntaxConfiguration`
- `LanguageSyntaxConfig`
- `SyntaxResources`
- `QueryKind`
- `IndentQuery`
- `TextObjectQuery`
- `TagQuery`
- `RainbowQuery`
- semantic highlight-facing types

Avoid re-exporting helper internals by default.

---

## What should move out of the old syntax core

The old `syntax.rs` and `syntax/config.rs` carry several mixed concerns that should not survive intact.

### Move to runtime or higher layers
- runtime-backed syntax resource loader
- language server config
- formatter config
- debugger config
- workspace/root service config
- mutable highlight scope/style table logic

### Move to render/presentation-adjacent code
- `HighlightCache`
- `OverlayHighlights`
- `OverlayHighlighter`
- `requery_and_cache(...)`
- theme/style mapping from semantic highlights to render styles
- rainbow overlay/highlight generation

### Keep out of syntax feature ownership
Do not move these feature algorithms into syntax:
- indentation decisions
- text object behavior
- comment behavior
- auto-pair behavior

Syntax should provide the language facts and query helpers they consume.

---

## Testing split

### Keep in `the-lib`
Pure syntax tests should cover:
- registry validation
- detection semantics and precedence
- query inheritance expansion
- query helper compilation/parsing
- parser adapter edit generation
- `SyntaxState` snapshot/version/stale-result behavior
- deterministic simulation/differential syntax tests that do not depend on runtime asset loading

### Move with runtime-backed resource loading
Runtime-side integration tests should cover:
- actual grammar loading
- actual runtime query asset loading
- end-to-end parsing with shipped assets
- runtime integration concerns

---

## Recommended implementation order

1. Split and rename the syntax-side config types.
2. Introduce `LanguageId`, `LanguageRegistry`, and `LanguageDefinition`.
3. Add strict registry validation and canonical detection precedence.
4. Port/clean `SyntaxResources` and inherited query expansion.
5. Port compiled query helpers (`IndentQuery`, `TextObjectQuery`, `TagQuery`, `RainbowQuery`).
6. Port `Syntax` and `SyntaxState` with snapshot/apply-parsed protocol.
7. Add `DocumentSyntax` state machine and activation/unavailable modeling.
8. Port parser adapter code (`generate_edits(...)`) into dedicated syntax adapter module.
9. Split pure semantic highlight production from render/presentation caches and mapping.

---

## Summary

The syntax subsystem should become:
- a pure, document-owned syntax core in `the-lib`
- backed by a mostly immutable `LanguageRegistry`
- using editor-owned identities and explicit document-facing state
- rich in tree/query capability
- careful not to absorb runtime service config or presentation strategy

The key architectural split is:

```txt
Syntax truth = parse tree + query results capability + pure language metadata
Runtime syntax infra = grammar/query acquisition and runtime orchestration
Presentation strategy = caches, overlays, and semantic-highlight -> style mapping
```

That is the target shape for the syntax rewrite.
