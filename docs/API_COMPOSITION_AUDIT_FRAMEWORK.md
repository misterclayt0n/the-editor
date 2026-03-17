# API Composition Audit Framework

## Purpose

This document captures the current direction of `the-editor` after the recent
composition work and turns it into a repeatable framework for auditing the rest
of the system.

This is not just a to-do list.

It is a way of thinking about API design in this project:

- what kind of composition model we are building
- what a clean surface looks like
- what smells indicate a surface is still wrong
- which APIs should be audited next
- what each audit should produce

This should be used together with
[docs/COMPILE_TIME_ASSEMBLY_PLAN.md](/Users/misterclayt0n/code/the-editor/docs/COMPILE_TIME_ASSEMBLY_PLAN.md),
but it is broader. That document explains the assembly direction. This one is
the framework for reviewing every other public surface against that direction.

---

## Current Direction

The system is moving toward a single idea:

`the-editor` should be authorable as a composition platform in Rust, at the
same abstraction level the shipped editor uses to compose itself.

That means:

- user config is not a plugin
- user config is not a scripting tier
- user config is not a "settings file with hooks"
- user config is a compile-time detour and assembly layer

At the same time:

- clients remain free to detour however they want
- low-level hooks remain available
- common authoring tasks should not require users to rebuild internal machinery

The project should feel like this:

- raw dispatch for low-level control
- shared presets for assembly
- higher-level builders/helpers where the raw surface is too repetitive,
  stringly, error-prone, or performance-hostile

That is the model.

---

## Architectural Decisions Already Made

The following direction is already established and should be treated as the
baseline for future audits.

### 1. Dispatch points should prefer generics, not `fn` pointers

`the-dispatch` is a generic dispatch system. Dispatch construction should stay
generic and should not collapse into `fn`-pointer-only authoring surfaces.

Function items are fine. Closures are fine. Zero-sized generic handler types
are fine. Explicit `fn(...)` pinning is not the direction.

### 2. Shared composition should use one real preset surface

The shared composition API should be the same API used by `the-default`
internally and by config externally.

Config should not get a second-tier wrapper API that the shipped editor itself
does not use.

### 3. Common authoring paths should be high-level

If a config author has to manually reconstruct scanning, matching, preview
wiring, item shaping, handler naming, or state plumbing just to use a feature,
the API is too low-level.

We want to preserve low-level escape hatches, but the common path must be much
cleaner than that.

### 4. Stateful features are first-class

If the system wants to support features like:

- AI overlays
- inline suggestions
- VCS pickers
- custom gutters
- render overlays
- popups and status surfaces

then sanctioned extension state is required. The system cannot be treated as
pure hook scripting.

### 5. Client freedom is preserved

The terminal client and FFI/Swift client are not being collapsed into one
policy model.

Clients can still detour or ignore shared behavior. The audit target is the
shared authoring surface, not client homogenization.

---

## What We Are Optimizing For

When an API is in a good state, a feature author should be able to:

- import a crate or copy a module
- install it into a preset
- capture state where needed
- bind keys to it
- expose commands for it
- use standard editor primitives without manually rebuilding internals
- keep an escape hatch for weird or experimental cases

The ideal user story is:

1. A person has an idea for a feature.
2. They write normal Rust using the same surface `the-default` uses.
3. The feature feels native to the editor.
4. Sharing it is just code reuse, not a special deployment model.

---

## Composition Standards

Every API audit should judge surfaces against these standards.

### 1. Same-level composition

Ask:

- Is this the same abstraction level the shipped editor uses?
- Or is config being pushed into a lower-quality side channel?

If the internal implementation uses one clean path and the external author has
to use a worse path, the surface is wrong.

### 2. Low-level power with high-level ergonomics

Ask:

- Does the system expose the primitive operations?
- Does it also expose a clean common path for the main use case?

Do not confuse "possible" with "ergonomic".

### 3. Typed over stringly

Ask:

- Are authors naming things with freeform strings when the system could own
  stable typed IDs or builder-managed handles?
- Are we asking users to manually coordinate names that the runtime could
  coordinate for them?

Strings are acceptable as escape hatches. They should not define the common
path.

### 4. Stateful composition where needed

Ask:

- Can an authored feature own durable state without abusing globals, client
  internals, or unrelated context fields?
- Can it evolve over time without becoming a hack?

### 5. Performance by default

Ask:

- Does the common API naturally use the performant path?
- Or does the author have to accidentally reimplement a slow version of logic
  the editor already has internally?

The common API should guide users toward the fast path automatically.

### 6. Escape hatches remain available

Ask:

- If the high-level API cannot express something unusual, is there still a raw
  path available?

High-level surfaces should not trap the system. But the escape hatch should be
clearly lower-level, not the default expectation.

### 7. Importable feature model

Ask:

- Can a feature be packaged as a normal Rust crate with a small install
  function?
- Or does it depend on private client internals and one-off wiring?

### 8. Client neutrality where appropriate

Ask:

- Is this a shared authoring surface that should live above any specific
  client?
- Or is it genuinely client-specific policy or presentation?

Do not force every surface into the common layer. But shared authoring
primitives should not be trapped inside a single client.

### 9. Discoverability

Ask:

- Can a user learn how to use this surface by reading the type names and a
  small example?
- Or do they need to reverse-engineer internal editor code to discover the
  correct wiring?

### 10. Layering clarity

Ask:

- What is the primitive layer?
- What is the assembly layer?
- What is the high-level authoring layer?

If those layers are blurred, the API will feel muddy even when it is powerful.

---

## Audit Smells

These are recurring signs that a surface probably needs work.

### 1. "The user has to rebuild the internal pipeline manually"

Examples:

- manual filesystem walking for a picker
- manual fuzzy matching or query splitting
- manual preview wiring
- manual render-plan plumbing for common visual behavior
- manual action registration where a builder should do it

### 2. Raw struct literal burden

If authors are expected to construct large internal structs field-by-field just
to express a common concept, the API is too low-level.

Common things should have constructors/builders/helpers.

### 3. String coordination burden

If authors have to manually coordinate handler names, event names, or lookup
keys across unrelated calls, the system is leaking internal runtime wiring.

### 4. `fn`-pointer-only callback surfaces

If a callback surface only accepts raw `fn` pointers, it blocks captured state
and pushes authors toward awkward workarounds.

This is especially important for dispatch and authoring APIs.

### 5. State hiding in the wrong place

If authors need custom state, but the only place to put it is a random client
field or an unrelated editor object, the surface is not ready.

### 6. Shared primitives trapped in a client

If a generally useful authoring primitive only exists as terminal-specific or
FFI-specific glue, it should be reviewed for extraction.

### 7. Internal types presented as the first thing users see

If the "public" path is just "here is the internal state struct, good luck",
the system is exposing internals rather than designing an API.

### 8. Configuration that is technically possible but operationally confusing

If users build the configured binary but do not know where it ended up, or if a
config path is implicit and hard to inspect, the API might be technically sound
but the workflow is not.

The CLI is part of the authoring surface too.

---

## Desired Layering Model

Every major subsystem should ideally expose three levels.

### 1. Primitive layer

The raw operations and data structures.

Examples:

- dispatch points
- render plan structures
- annotation data
- document/view/editor operations
- low-level picker state and rows

### 2. Assembly layer

The place where features are registered and composed.

Examples:

- `EditorPreset`
- command installation
- named action installation
- extension-state installation
- render and UI post-processors

### 3. Authoring layer

The clean path for common feature construction.

Examples:

- `PickerBuilder`
- helper constructors for common rows or overlays
- typed registration helpers
- key-binding builders or merge helpers

The primitive layer must exist. The authoring layer must also exist wherever
the primitive path is too repetitive for normal use.

---

## Audit Output Template

Every audit of a surface should try to answer these questions.

### 1. Current shape

- What is the surface today?
- What is public?
- What is actually used by `the-default` internally?

### 2. Author burden

- What does a feature author have to do manually today?
- What code is repetitive, stringly, or easy to get wrong?

### 3. Missing abstraction

- What builder/helper/typed handle/install step is missing?
- What part of the workflow should the system own instead of the user?

### 4. Escape hatch

- If we add a higher-level surface, what raw path should remain available?

### 5. Performance and correctness

- Does the new common path naturally use existing performant internals?
- Are there correctness risks if the high-level surface hides too much?

### 6. Shareability

- Can the result be packaged as a reusable crate or module?

### 7. Example and test requirements

- What minimal example should exist?
- What test proves the surface is actually usable?

---

## Audit Priorities

Not every API needs the same urgency.

### Priority 0: Shared authoring surfaces

These are the places that directly determine whether config feels like a real
composition platform.

- `the-dispatch`
- `the-default/assembly.rs`
- `the-default/command.rs`
- `the-default/extensions.rs`
- `the-default/command_registry.rs`
- `the-default/keymap.rs`
- `the-default/file_picker.rs`
- `the-default/extension_state.rs`
- render and annotation surfaces in `the-lib/render`

### Priority 1: Editor-facing feature surfaces

These shape common editor capabilities that users will want to detour or
extend.

- command palette
- completion UI
- context menus
- file tree
- global search
- message/status surfaces
- overlays
- signature help
- tabs and related snapshots

### Priority 2: Core editing and render model

These determine whether advanced features can compose cleanly over time.

- document/editor/view
- selection/history/transaction
- search/movement/object/text-object
- diagnostics and diff
- split tree
- syntax and async syntax attachment points

### Priority 3: Client seams and build workflow

These are not the conceptual center of composition, but they can still damage
the authoring experience if they stay confusing or too closed.

- `the-term/config_cli.rs`
- `the-term/main.rs`
- `the-term/ctx.rs`
- `the-ffi/lib.rs`
- `the-config` template/fallback bridge

---

## Concrete Audit Inventory

The following is the concrete queue of modules and surfaces that should be
reviewed.

## A. `the-dispatch`

### Files

- `the-dispatch/lib.rs`
- `the-dispatch/define.rs`
- `the-dispatch/registry.rs`

### Audit focus

- Generic dispatch ergonomics
- handler storage strategies
- builder naming and generated APIs
- trait-bound ergonomics for callers
- docs/examples quality
- whether optional dynamic registry concepts are bleeding into shared authoring
  too much

### Questions

- Is the generic path the obvious path?
- Are generated trait names and builder methods discoverable?
- Are examples showing the intended composition style?

---

## B. Shared composition kernel in `the-default`

### Files

- `the-default/assembly.rs`
- `the-default/command.rs`
- `the-default/extensions.rs`
- `the-default/extension_state.rs`
- `the-default/command_registry.rs`
- `the-default/keymap.rs`
- `the-default/input.rs`
- `the-default/pending.rs`

### Audit focus

- whether `EditorPreset` is the real shared assembly surface everywhere it
  should be
- whether dispatch detours are easy to author and chain
- whether command/action installation is clean and capture-friendly
- whether keymaps are expressive enough and merge cleanly
- whether extension state covers real feature authoring needs
- whether low-level event hooks remain available without being the only path

### Questions

- Does this feel like the surface `the-default` itself should use?
- Are there still internal-only detour paths that should be promoted?
- Are command/action/keymap APIs too stringly anywhere?
- Can a feature author bind keys, expose commands, and own state without
  friction?

---

## C. Picker and search-adjacent authoring in `the-default`

### Files

- `the-default/file_picker.rs`
- `the-default/global_search.rs`
- `the-default/search_prompt.rs`
- `the-default/buffer_tabs.rs`

### Audit focus

- whether the high-level picker API now covers the real common cases
- whether file-scan, dynamic, and static picker paths are clean
- whether preview behavior is ergonomic
- whether search and navigation surfaces compose cleanly with pickers
- whether tab snapshots and other navigation surfaces expose enough structure
  for reusable tools

### Questions

- What still forces authors down to `FilePickerItem` internals?
- Are dynamic query-driven surfaces debounced, incremental, and state-friendly
  by default?
- Do search-related surfaces share enough primitives with picker authoring?
- Can VCS and diagnostics-style pickers be built naturally?

---

## D. Commands, palettes, completion, and menus in `the-default`

### Files

- `the-default/command_palette.rs`
- `the-default/completion_menu.rs`
- `the-default/context_menu.rs`
- `the-default/signature_help.rs`
- `the-default/message_bar.rs`
- `the-default/statusline.rs`
- `the-default/theme_catalog.rs`

### Audit focus

- command palette authoring
- completion/menu extensibility
- menu item construction burden
- theme and presentation surfaces that affect authored tools
- whether UI-oriented helper builders are needed

### Questions

- Can config authors add rich command/action palette content naturally?
- Are completion menus and signature/help panels authorable or only consumable?
- Are menu and palette items too raw to construct comfortably?

### Current Findings

The direction here is now clearer:

- command authoring is in a much better state after `CommandBuilder`
- action and command palette authoring was the main remaining shared-API gap
- completion and signature-help state are reusable, but they are still closer
  to "show this client UI state" than to a first-class authored surface
- context menus are typed, but still mostly shipped as hardcoded snapshots
- message, status, and theme surfaces are consumable and useful, but still not
  very install-oriented for authored tools

The most important correction in this section is:

- command and action palette content should be an extension surface owned by
  `EditorPreset`, not hardcoded population logic trapped in
  `the-default/keymap.rs` and `the-default/command_registry.rs`

That means the shared model should support:

- installing palette item providers from config
- contributing items to the action palette without patching keymap internals
- contributing richer command-palette items when the palette is in
  command-name browsing mode
- using small item builders instead of filling raw structs by hand

This does not mean the palette should become a scripting host.

It means authored features should be able to install content into the same
shared UI surfaces the shipped editor uses.

### What To Look For In This Audit

When reviewing these modules, prefer the following direction:

- `CommandPaletteItem` should be easy to construct fluently and should not
  force raw field assignment for common cases
- palette content should come from registered providers where appropriate, not
  only from hardcoded walks over builtin commands and keymaps
- completion and signature-help items should be cheap to author directly from
  config hooks
- context-menu construction should use typed IDs plus light builders, not raw
  section and item struct assembly everywhere
- presentation surfaces like message, status, and theme should expose enough
  shared helpers that authored tools can feel native without reaching into
  client-only UI policy

### Likely Follow-Up Work

This section is not finished.

The next probable upgrades after opening palette authoring are:

- typed handles for named actions so key bindings and palette actions stop
  coordinating through repeated string literals
- authored completion providers and signature-help providers, if we want those
  panels to become first-class composition surfaces instead of LSP and client
  consumers only
- context-menu provider and install hooks for editor and tree surfaces
- higher-level status and message builders for feature-owned transient UI

---

## E. Render and annotation surfaces in `the-lib/render`

### Files

- `the-lib/render/mod.rs`
- `the-lib/render/plan.rs`
- `the-lib/render/doc_formatter.rs`
- `the-lib/render/text_annotations.rs`
- `the-lib/render/overlay.rs`
- `the-lib/render/gutter.rs`
- `the-lib/render/ui.rs`
- `the-lib/render/ui_theme.rs`
- `the-lib/render/theme.rs`
- `the-lib/render/inline_diagnostics.rs`
- `the-lib/render/highlight_adapter.rs`
- `the-lib/render/text_format.rs`
- `the-lib/render/graphics.rs`
- `the-lib/render/visual_position.rs`
- `the-lib/render/grapheme.rs`

### Audit focus

- inline annotation authoring
- overlay authoring
- line annotation authoring
- render-plan post processing
- gutter extensibility
- UI overlay and popup primitives
- theme/style layering
- whether visual augmentation requires too much internal knowledge

### Questions

- Can users build AI overlays and experimental visuals without fighting the
  render internals?
- Are gutters open enough for extension, or still too closed?
- Are annotation and overlay types sufficiently typed and layered?
- Is there a builder/helper layer missing for common visual tools?

---

## F. Editing core in `the-lib`

### Files

- `the-lib/document.rs`
- `the-lib/editor.rs`
- `the-lib/view.rs`
- `the-lib/selection.rs`
- `the-lib/transaction.rs`
- `the-lib/history.rs`
- `the-lib/position.rs`
- `the-lib/movement.rs`
- `the-lib/object.rs`
- `the-lib/text_object.rs`
- `the-lib/indent.rs`
- `the-lib/comment.rs`
- `the-lib/auto_pairs.rs`
- `the-lib/surround.rs`
- `the-lib/match_brackets.rs`
- `the-lib/registers.rs`
- `the-lib/clipboard.rs`
- `the-lib/messages.rs`
- `the-lib/app.rs`

### Audit focus

- whether the core state model is cleanly composable
- whether view/editor/document boundaries are correct
- whether core operations expose good reusable primitives
- whether high-level editor features can be assembled without reaching through
  unstable internals

### Questions

- What should be pure state versus attachment versus client policy?
- Are editor/document/view APIs shaped for reuse by higher layers?
- Are there places where a feature author would need client-only knowledge just
  to use core editing capabilities?

---

## G. Search, diagnostics, diff, and syntax in `the-lib`

### Files

- `the-lib/search.rs`
- `the-lib/fuzzy.rs`
- `the-lib/diff.rs`
- `the-lib/diagnostics.rs`
- `the-lib/syntax.rs`
- `the-lib/syntax_async.rs`
- `the-lib/docs_markdown.rs`

### Audit focus

- whether search and fuzzy primitives are reusable by authoring surfaces
- whether diagnostics and diff data models are open enough for custom tools
- whether syntax attachment and highlighting are layered correctly
- whether async syntax integration exposes stable extension points

### Questions

- Can reusable authoring surfaces depend on these modules without leaking
  implementation detail?
- Are syntax/highlight APIs too renderer-aware or too opaque?
- Can diagnostics, diff, and search feed picker/render features cleanly?

---

## H. Layout and workspace structure in `the-lib`

### Files

- `the-lib/split_tree.rs`
- `the-lib/view.rs`
- `the-lib/editor.rs`

### Audit focus

- whether layout/tree operations expose enough reusable structure
- whether authored features can target splits/views cleanly
- whether view identity and routing are stable enough for composition

### Questions

- Can a feature choose where to open or focus content in a composable way?
- Are view/split operations too tied to one client’s assumptions?

---

## I. Client-facing composition seams

### Files

- `the-term/config_cli.rs`
- `the-term/main.rs`
- `the-term/ctx.rs`
- `the-term/dispatch.rs`
- `the-ffi/lib.rs`
- `the-config/src/lib.rs`
- `the-config/template/src/lib.rs`
- `the-config/fallback/src/lib.rs`

### Audit focus

- whether shared authored features can actually flow into real clients
- whether client-specific detours remain clean
- whether the config/build workflow is explicit and understandable
- whether there are still hidden client-owned seams that should become shared
  primitives

### Questions

- Can a composed feature reach the client without undocumented glue?
- Is client freedom preserved without degrading shared authoring?
- Is the build/run/install workflow obvious to a user?

---

## What Auditors Should Specifically Look For

When reviewing any of the surfaces above, look for the following concrete
upgrade opportunities.

### 1. Builders for common authored concepts

Add builders/helpers when users currently have to:

- walk data structures manually
- fill large internal structs by hand
- coordinate multiple registrations by name
- recreate editor-internal matching or rendering behavior

### 2. Typed handles instead of manual naming

Replace user-managed strings with typed IDs or builder-managed opaque handles
where the system can safely own that coordination.

### 3. Captured closures where authors need state

If a surface obviously wants per-feature state or configuration, it should
accept captured closures or equivalent callable storage rather than forcing raw
function items.

### 4. Extension-state integration

If a feature naturally wants durable state, the API should say where that state
lives and how it is accessed.

### 5. A clean install story

A reusable feature should ideally reduce to:

```rust
pub fn install<Ctx>(preset: &mut EditorPreset<Ctx, ...>) { ... }
```

If a surface makes this awkward, it should be reviewed.

### 6. Better examples

A common sign of a weak API is that the example code looks like internal
plumbing instead of feature authoring.

If the example feels like "rebuild the subsystem by hand", the surface still
needs work.

---

## Practical Review Sequence

If this audit work is split across multiple agents, the best order is:

1. `the-default` composition kernel
2. `the-lib/render`
3. picker/search/tree/navigation surfaces
4. command palette/completion/menu/UI surfaces
5. editing core and document/view/editor boundaries
6. diagnostics/diff/syntax/search primitives
7. client seams and workflow polish

This order keeps the authoring surface coherent while deeper core reviews
happen in parallel.

---

## Final Standard

A surface in this project is in a good state when all of the following are
true:

- the raw primitive path exists
- the common path is dramatically cleaner than the primitive path
- the editor itself uses the same composition surface it exposes
- authors can capture state where that makes sense
- performance-sensitive paths remain efficient by default
- client freedom is preserved
- examples look like authored features, not subsystem reconstruction

That is the bar for this audit work.
