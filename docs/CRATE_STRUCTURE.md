# Crate Structure Mapping (core)

This file maps every source file under the-editor `legacy` branch into one of:

- the-core: low-level primitives and Unicode/text utilities; no editor state.
- libtheditor: editor state, commands, data model, and core behavior.
- libtheditor-render: layout, formatting, decorations, and render-prep logic.

Notes:
- Some files mix concerns; those are placed by dominant responsibility and
  tagged "split later" where appropriate.
- This file is expected to evolve as the rewrite progresses.

## the-core

- core/chars.rs - basic character helpers and Unicode predicates.
- core/grapheme.rs - grapheme handling and width helpers.
- core/line_ending.rs - line ending parsing and normalization.
- core/uri.rs - URI type and helpers.

## libtheditor

- core/auto_pairs.rs - editing behavior (auto pairs).
- core/case_conversion.rs - text editing helpers.
- core/clipboard.rs - clipboard abstraction; likely move to runtime/ports later.
- core/command_line.rs - command-line mode state and behavior.
- core/command_registry.rs - command registration and lookup.
- core/commands.rs - editing commands and operations.
- core/comment.rs - comment toggling logic.
- core/config.rs - editor config model (non-render).
- core/diff.rs - document diffing.
- core/document.rs - document model and storage.
- core/editor_config.rs - editor configuration types.
- core/expansion.rs - snippet/expansion behavior.
- core/file_watcher.rs - filesystem watching; likely move to runtime/ports later.
- core/fuzzy.rs - fuzzy matching/search.
- core/global_search.rs - workspace/global search logic.
- core/global_search/tests.rs - tests for global_search.
- core/history.rs - undo/redo history.
- core/indent.rs - indentation logic.
- core/info.rs - info box data; currently includes formatting, may split later.
- core/lsp_commands.rs - LSP request helpers; likely move to service layer later.
- core/macros.rs - editor access macros.
- core/match_brackets.rs - bracket matching.
- core/mod.rs - core module root; will become libtheditor crate root.
- core/movement.rs - cursor movement logic.
- core/object.rs - text object logic.
- core/quick_slots.rs - view slotting model.
- core/registers.rs - registers/yank history.
- core/search.rs - search logic.
- core/selection.rs - selection model.
- core/special_buffer.rs - special buffer types.
- core/surround.rs - surround editing behavior.
- core/syntax.rs - syntax parsing/highlight data model (render uses types).
- core/syntax/config.rs - syntax configuration.
- core/textobject.rs - text object selection logic.
- core/transaction.rs - edit transactions.
- core/tree.rs - view tree/state and split management (render-adjacent).
- core/view.rs - view state and behavior (render-adjacent).

## libtheditor-render

- core/animation.rs - UI/selection animations.
- core/context_fade.rs - fade decoration logic.
- core/diagnostics.rs - inline diagnostics rendering/annotation logic; split later.
- core/doc_formatter.rs - document formatting and layout into visual positions.
- core/graphics.rs - render primitives (rects, colors, cursor kinds).
- core/layout.rs - layout engine for view areas.
- core/position.rs - visual position mapping and soft-wrap math.
- core/text_annotations.rs - virtual text/overlays/line annotations.
- core/text_format.rs - text layout formatting parameters.
- core/theme.rs - theme and style definitions.
- core/tool_display.rs - tool call display formatting.

## considerations
- core/history.rs → build undo/redo on top of your new Transaction API.
- core/registers.rs → yank/put storage (pure data, minimal deps).
- core/search.rs + core/fuzzy.rs + core/diff.rs → pure query/algorithms; good to lock in before doc/view state.
- core/match_brackets.rs, core/object.rs, core/textobject.rs, core/surround.rs, core/comment.rs, core/indent.rs →
  all can be expressed as “selection in → transaction out”.
- Then a slim Document/Buffer core (rope + selections + history + config), explicitly excluding IO, LSP, syntax,
  diagnostics, view data, etc. The old document.rs mixes too many services.

On commands.rs + command_registry.rs:
You’re right—those shouldn’t live in the-lib if the goal is to keep clients “dumb” and let the-dispatch own
command wiring + UI + editor context. The old command_registry.rs depends on editor state, prompt UI,
completions, LSP, etc. That belongs in the dispatch/app layer. In the-lib, keep only the command‑line parser
(command_line.rs) and maybe a tiny CommandSpec/Signature type if you want shared metadata.
