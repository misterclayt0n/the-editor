# Compile-Time Assembly Plan

## Purpose

This document replaces the "DLL plugin" line of thinking with the model that
actually matches `the-editor`:

- user config is not a plugin
- user config lives at the same abstraction level as the shipped editor
- composition remains compile-time Rust composition
- clients remain free to detour however they want

The goal is to make the current Rust-based configuration model feel like a real
authoring surface for the editor rather than a thin hook layer over
`the-default`.

This document has two jobs:

1. describe a compile-time assembly API for `the-config`
2. inventory the existing composition surfaces in `the-default` and `the-lib`
   so they can be reviewed and opened up over time

---

## Important Clarification

`the-dispatch` itself is generic.

It generates dispatch structs whose handler slots are generic over the handler
types. That means the underlying dispatch system can support function pointers,
closures, and other handler storage strategies.

However, `the-default` currently exports a concrete specialization:

- `DefaultDispatchStatic<Ctx>` is an alias that pins every handler slot to a
  plain `fn(...)` pointer
- `CommandFn<Ctx>` in `the-default/command_registry.rs` is also a plain function
  pointer type

So the situation today is:

- `the-dispatch` = generic infrastructure
- `the-default` = current static `fn`-pointer authoring surface

That distinction matters because the project vision is broader than the current
surface exposed by `the-default`.

---

## Problem Statement

The current Rust config model proves the idea, but it does not yet expose the
editor as a satisfying composition surface.

Today, users can override a few things, but the system does not yet feel like a
place where you can comfortably author:

- custom keymaps
- custom typable commands
- custom functions and reusable modules
- custom picker-driven tools
- custom render-plan augmentation
- custom line annotations
- custom gutters or gutter extensions
- custom UI overlays and popups
- custom dispatch subgraphs
- shareable "features as crates" that other users can import

The main issue is not rebuild cost.

The main issue is that the interesting contact surfaces are still too
incidental, too closed, or too tied to low-level internal shapes.

---

## Non-Negotiable Constraints

Any plan in this area should preserve the following:

### 1. No runtime plugin ABI

No DLL model, no special plugin boundary, no user-facing C ABI, no separate
"plugin API" tier.

### 2. Compile-time Rust composition

The user config crate should compose the editor exactly the same way the editor
composes itself.

### 3. Clients stay free

Clients are allowed to detour however they want.

This plan must not force the terminal client and FFI/Swift client to share all
policy or all presentation behavior. Shared authoring surfaces should exist,
but clients keep the right to opt into them, wrap them, ignore them, or extend
them.

### 4. Shared features should be importable as normal Rust

If someone builds a useful feature, sharing it should look like:

- copy-paste code
- import a crate
- call an install function

### 5. Static dispatch remains a first-class option

Performance-sensitive paths should not be forced into dynamic dispatch just to
gain flexibility.

---

## What Success Looks Like

The following should become natural:

- a user crate installs custom keymaps without re-implementing the entire
  keymap system
- a user crate registers typable commands in the command palette
- a user crate defines its own actions and binds keys to them
- a user crate opens a picker, fills it with custom items, and handles submit in
  custom ways
- a user crate injects inline annotations, overlays, line annotations, and
  render-plan edits
- a user crate contributes gutter content or extends gutter rendering
- a user crate adds UI overlays or popups through shared UI hooks
- a user crate ships a feature as a normal Rust crate with an `install(...)`
  entrypoint
- clients can choose which parts of that shared assembly to consume

---

## Core Design Direction

The next step is not "more hooks".

The next step is a first-class compile-time assembly layer.

Call it `Assembly`, `Preset`, `DetourSet`, or similar. The exact name is less
important than the role:

- `the-default` should define the common authoring surface
- `the-config` should extend that surface
- clients should be able to consume that assembled surface selectively

This is not a plugin model. It is editor assembly.

---

## Proposed Model

## 1. Introduce an Assembly API

The shared authoring surface should become a value that can be built,
detoured, and finalized at compile time.

Conceptually:

```rust
pub struct EditorAssembly<Ctx> {
    pub dispatch: ...,
    pub keymaps: ...,
    pub commands: ...,
    pub actions: ...,
    pub picker_hooks: ...,
    pub render_hooks: ...,
    pub ui_hooks: ...,
    pub startup_hooks: ...,
}
```

The exact field layout can change, but the important part is that config and
reusable crates are installing into a shared assembly object rather than
reaching into random internals directly.

### Why this matters

It gives the project one intentional authoring surface for:

- shared defaults
- config crate detours
- reusable feature crates
- client-specific assembly steps

## 2. Introduce an Install Trait or Convention

Reusable features should install into assembly via a small convention:

```rust
pub trait InstallIntoAssembly<Ctx> {
    fn install(self, assembly: &mut EditorAssembly<Ctx>);
}
```

or simply:

```rust
pub fn install<Ctx>(assembly: &mut EditorAssembly<Ctx>) { ... }
```

This is the key enabler for shareable Rust features.

Examples:

- `copilot_like::install(&mut assembly)`
- `git_diff_picker::install(&mut assembly)`
- `funny_popup::install(&mut assembly)`

## 3. Let Clients Choose How Much to Consume

Clients should not be forced into one unified policy model.

Instead, they should be able to choose:

- consume the full shared assembly
- consume only shared keymaps + commands
- consume only shared dispatch hooks
- wrap shared assembly with client-specific detours
- bypass parts of shared assembly where the client needs a different path

Conceptually:

```rust
let mut assembly = the_config::build_assembly::<Ctx>();
term_client::install_term_detours(&mut assembly);
let built = assembly.finish();
```

and another client could do:

```rust
let mut assembly = the_config::build_assembly::<App>();
ffi_client::install_ffi_detours(&mut assembly);
let built = assembly.finish();
```

The important thing is that this preserves client freedom while still allowing
`the-config` to be a serious authoring layer.

---

## What the Assembly API Must Cover

The compile-time assembly API should explicitly support at least these areas.

## 1. Dispatch

Config should be able to:

- replace existing dispatch points
- wrap existing dispatch points
- chain shared detours cleanly
- install local dispatch graphs owned by a feature crate

This should be the main shared behavioral surface.

### Recommendation

Keep two paths:

- a static `fn`-pointer fast path for the simplest cases
- a generic authoring path for richer composition

That keeps existing performance characteristics available while allowing the
shared authoring layer to grow beyond plain function pointers.

## 2. Keymaps

Config should be able to:

- define entire keymaps from Rust
- merge keymaps into defaults
- remove or override bindings
- bind keys to custom actions, not only built-in commands
- expose reusable keymap bundles from external crates

Current keymap support is close, but the binding target model is too narrow.

`KeyAction::Named` currently resolves against built-in command names and mode
names. That should expand into a real named-action system owned by assembly.

## 3. Typable Commands

Config should be able to:

- register typable commands
- provide docs and completions
- preview and validate
- reuse picker and UI APIs inside commands
- expose commands from shareable crates

The current registry already has useful concepts:

- command docs
- aliases
- completers
- signatures
- preview/validate lifecycle

This is a strong foundation and should become part of the intended config
surface.

## 4. Custom Functions and Named Actions

Config should be able to define reusable named behaviors that can be targeted
by:

- keymaps
- command palette entries
- picker submit flows
- UI events
- custom dispatch chains

This is broader than `Command`.

There should be a first-class notion of user-defined action or named behavior
owned by assembly.

## 5. Picker Authoring

Config should be able to:

- open custom static pickers
- open custom dynamic pickers
- fill them with arbitrary items
- define custom submit behavior
- provide preview behavior
- reuse the file picker UI shell without being limited to file-opening semantics

This is one of the highest-value surfaces in the whole system.

Today the picker UI exists and is powerful, but item actions are too closed for
the intended use cases.

## 6. Render and Annotation Authoring

Config should be able to:

- add inline annotations
- add overlays
- add line annotations
- mutate the render plan after it is built
- add decorations that feel like ghost text, suggestion previews, inline VCS
  metadata, etc.

This is the main path for things like:

- Copilot-like ghost text
- Supermaven-like inline suggestion UI
- diff decorations
- inline diagnostics variants
- experimentation with novel visual features

## 7. Gutters

Config should be able to:

- extend gutter content
- add custom gutter markers
- reorder or suppress gutter elements
- possibly add new gutter concepts over time

Current gutter types are useful but too closed for the long-term goal.

## 8. UI Overlays and Popups

Config should be able to:

- add panels to `UiTree`
- add floating overlays
- respond to custom UI events
- create intentionally silly or experimental UI behavior

The current UI hooks already move in the right direction. They should become an
explicit part of the config story.

## 9. Stateful Features

Config-authored features will need state.

Examples:

- AI suggestion state
- cached picker state
- popup visibility state
- request/response tracking
- ephemeral render state

That means the shared authoring model must include an extension state story.

---

## State Model Requirements

If config is going to author serious features, it needs somewhere to store
state without patching client internals ad hoc.

### Recommended direction

Introduce an extension state bag on the shared context surface.

Conceptually:

```rust
ctx.extensions_mut().get_or_insert_with::<MyState>(MyState::default)
```

This can be implemented in multiple ways:

- typed map keyed by `TypeId`
- explicitly namespaced slot registry
- client-owned extension store surfaced through trait methods

What matters is that shared features have a sanctioned place to persist state.

### Why this matters

Without this, the system remains limited to:

- stateless hooks
- direct mutation of existing editor state
- ad hoc client-specific fields

That is not enough for the level of composition this project wants.

---

## Startup / Install Lifecycle

The shared authoring surface also needs lifecycle hooks.

At minimum:

- install/build time: register commands, actions, keymaps, hooks
- startup/init time: seed state, kick off services, show initial UI if needed
- optional teardown or reset hooks later

Examples:

- register a command that opens a custom picker
- initialize feature state on first use
- register a popup module
- attach a line annotation provider

This lifecycle should be explicit.

---

## Recommended Architectural Shape

## Shared crates

### `the-dispatch`

Keep it generic and low-level.

Its job is still:

- generate dispatch structs
- provide handler slot mechanics
- allow static and optional richer storage modes

### `the-default`

This becomes the main shared authoring layer.

Its job should be:

- define the common assembly surface
- provide default assembly pieces
- expose reusable editor-facing APIs for config authors

### `the-config`

This becomes an assembly crate, not just a hook override crate.

Its job should be:

- build or detour the shared assembly
- install reusable feature crates
- define user-specific commands, keymaps, and features

### clients

Clients remain clients.

Their job should be:

- choose how to consume shared assembly
- add client-specific detours
- keep client-specific rendering/input/runtime details

---

## Implementation Phases

## Phase 0 - Clarify the boundary

Document the distinction between:

- generic dispatch infrastructure
- current `fn`-pointer-specialized default surface

This is required so future work does not accidentally optimize around the wrong
abstraction.

## Phase 1 - Define assembly types

Introduce the first version of:

- `EditorAssembly<Ctx>`
- install convention for reusable modules
- finalization/build path for clients

The first version can stay small. It just needs to establish the pattern.

## Phase 2 - Open keymaps and commands

Make it easy to:

- register typable commands from config
- register named actions from config
- bind keymaps to those actions
- merge/override keymaps intentionally

This will immediately improve authoring UX.

## Phase 3 - Open picker authoring

Add a picker authoring layer that supports:

- custom item kinds
- custom submit actions
- custom preview strategies
- dynamic query-driven pickers

This unlocks many serious features.

## Phase 4 - Open render and annotation authoring

Add a clearer path for:

- inline annotations
- overlays
- line annotations
- render-plan post processing
- UI overlays

This unlocks AI features and experimental visual tools.

## Phase 5 - Introduce extension state

Provide sanctioned state storage for config-authored features.

This is what turns the system from "hook scripting" into a real composition
platform.

## Phase 6 - Audit and expand surfaces

Review the existing exported API surface in `the-default` and `the-lib` and
decide:

- which surfaces are ready to become intentional authoring APIs
- which are too closed
- which are too low-level
- which need helper wrappers
- which should stay client-owned

---

## Surface Audit Inventory

This section is intentionally broad. It is not a list of required changes; it
is a map of contact surfaces that should be reviewed by future agents.

## A. `the-dispatch`

### Areas to review

- `the-dispatch/lib.rs`
- `the-dispatch/define.rs`
- `the-dispatch/registry.rs`
- `the-dispatch/README.md`

### Why review it

- confirm which handler storage modes should remain available
- confirm whether a shared assembly API should use plain generics, `cow-handlers`,
  a wrapper type, or multiple entrypoints
- confirm whether custom config-defined sub-dispatches should be encouraged as a
  normal pattern

### Questions

- Should `the-default` continue exporting a fully static alias?
- Should there be a richer generic alias alongside it?
- Should the config authoring path default to closures or keep function pointers
  by default?

## B. `the-default`

### 1. `command.rs`

This is the largest and most important shared authoring surface.

It currently owns:

- dispatch point definitions
- default dispatch construction
- render pipeline entrypoints
- UI pipeline entrypoints
- command execution routing
- `DefaultContext`

#### Review goals

- identify which dispatch points should be considered public authoring surfaces
- identify where helper wrappers are needed for config authors
- review whether `DefaultDispatchStatic` should remain the only exported path
- review `DefaultContext` for missing extension-state and lifecycle methods

### 2. `command_registry.rs`

This is already close to a useful config surface.

It currently owns:

- typable commands
- command docs
- signatures
- preview/validate lifecycle
- completers

#### Review goals

- open command registration as an intentional config feature
- review whether `CommandFn<Ctx>` should remain a plain `fn` pointer
- review whether command palette registration needs helper APIs
- review how user-defined actions and typable commands should relate

### 3. `keymap.rs`

This is another core authoring surface.

It currently owns:

- key binding parsing
- key trie structure
- mode switching
- action palette generation
- key action application

#### Review goals

- make keymap definition and merging feel intentional
- open binding targets beyond built-in named commands
- decide how custom user actions should be represented
- decide whether keymap helper builders or macros are needed beyond the current
  low-level types

### 4. `command_types.rs`

This currently defines the built-in command universe.

#### Review goals

- decide what remains built-in
- decide what should move into named-action or custom-action territory
- avoid forcing every useful feature through the closed `Command` enum

### 5. `file_picker.rs`

This is a major feature surface.

It currently exposes:

- custom/static picker opening
- dynamic picker opening
- item replacement
- query management
- preview support
- picker UI building

#### Review goals

- open custom submit behavior
- review whether `FilePickerItemAction` should become extensible
- review preview extensibility
- review whether the picker should become a more general "selection tool"
  surface rather than a mostly file-centric shell

### 6. `global_search.rs`

#### Review goals

- decide whether global-search infrastructure should become reusable for
  user-defined dynamic pickers
- review where shared search worker patterns should live

### 7. `search_prompt.rs`

#### Review goals

- review whether prompt-based workflows should be authorable by config
- review whether reusable prompt primitives should be exposed

### 8. `completion_menu.rs`

#### Review goals

- review whether config-authored features can reuse the completion menu shell
- review how much of this is specific to LSP/completion versus generic list UI

### 9. `signature_help.rs`

#### Review goals

- decide whether this remains specialized or becomes part of a more general
  overlay/help system

### 10. `context_menu.rs`

#### Review goals

- determine whether user-defined context menu entries should become possible
- determine whether context menu actions should target custom actions

### 11. `buffer_tabs.rs`

#### Review goals

- review whether buffer-tab snapshots and operations should become more
  consumable by config-authored UI

### 12. `message_bar.rs`

#### Review goals

- ensure config-authored features can publish messages and optionally customize
  message presentation behavior where appropriate

### 13. `overlay_layout.rs`

#### Review goals

- review whether layout helpers should remain internal conveniences or be
  formalized for config-authored overlays

### 14. `theme_catalog.rs`

#### Review goals

- decide whether theme discovery and theme-related UI should expose better
  shared authoring helpers

### 15. `input.rs`

#### Review goals

- confirm that shared input types are sufficient for client-agnostic authoring
- review whether any extra semantic input events are needed

## C. `the-lib`

`the-lib` is not the config layer, but it provides many of the data and render
primitives that config-authored features will need.

### 1. `render/plan.rs`

This is the core render-plan surface.

#### Review goals

- review how safe and ergonomic post-plan mutation is
- review whether helper APIs are needed for common plan mutations
- review where custom row insertions, overlays, and gutter edits should happen

### 2. `render/gutter.rs`

#### Review goals

- review whether the gutter type model is too closed
- review whether custom gutter contributions need a more open representation

### 3. `render/text_annotations.rs`

This is one of the most important long-term extension surfaces.

#### Review goals

- make line annotations easier to consume from config-authored features
- review whether annotation layering helpers are sufficient
- review where authoring ergonomics are missing

### 4. `render/overlay.rs`

#### Review goals

- review whether overlay node shapes are enough for user-authored visual
  features
- review whether helper constructors are needed for common overlay patterns

### 5. `render/ui.rs`

This is the shared UI intent model.

#### Review goals

- confirm `UiTree` and `UiNode` are sufficient for config-authored overlays
- review whether more semantic node helpers are needed
- review focus and event ergonomics

### 6. `render/theme.rs` and `render/ui_theme.rs`

#### Review goals

- review whether custom UI authored in config can participate cleanly in
  theming
- review whether theme-role helpers are needed

### 7. `document.rs`, `editor.rs`, `view.rs`

#### Review goals

- identify what data/config-authored features need access to
- ensure access patterns are stable enough for advanced features

### 8. `selection.rs`, `transaction.rs`, `history.rs`

#### Review goals

- ensure custom commands and actions can build robust editing workflows
- identify helper gaps for author-authored editing features

### 9. `messages.rs`

#### Review goals

- confirm it is sufficient as a common communication channel for shared
  features

### 10. `registers.rs`

#### Review goals

- review whether reusable feature crates can rely on registers sanely

### 11. `search.rs`, `fuzzy.rs`, `diff.rs`

#### Review goals

- identify algorithmic utilities worth exposing more intentionally to config
  authors, especially for pickers and VCS-like tools

### 12. `syntax.rs`, `syntax_async.rs`, render highlight adapters

#### Review goals

- decide what syntax/highlight services should be reusable by config-authored
  render features

### 13. `split_tree.rs`

#### Review goals

- review whether picker submit flows and custom UI need clearer split/pane
  control surfaces

## D. Client review targets

Clients remain free, but their shared consumption points should still be mapped.

### `the-term`

Review:

- how dispatch is stored
- how commands are registered
- how picker/render/UI helpers are consumed
- where client-specific detours should continue to live

### `the-ffi`

Review:

- same questions as `the-term`
- especially where client-specific UI/runtime behavior diverges from terminal
  behavior in healthy ways

---

## Design Questions That Must Stay Open

The plan should not prematurely lock in answers to these.

### 1. Should the main shared assembly surface support closures directly?

Possible answers:

- yes, by exposing a generic authoring path
- yes, but only with optional `cow-handlers`
- no, keep shared authoring on `fn` pointers and rely on extension state

This should be decided intentionally, not accidentally.

### 2. Should `DefaultDispatchStatic` remain as a stable fast path?

Likely yes.

The real question is whether it should remain the only first-class exported
authoring path.

### 3. How open should the command/action model become?

Possible directions:

- closed `Command` enum plus custom named actions
- broader open action registry
- command enum for built-ins, named actions for extensions

### 4. How open should picker item actions become?

Possible directions:

- generic action callback
- named-action dispatch on submit
- typed custom picker session object

### 5. How open should gutters become?

Possible directions:

- allow contributions to existing gutter lanes first
- later open custom gutter kinds

### 6. What belongs in the shared authoring layer versus client-specific layers?

This should stay conservative.

If a surface is inherently presentation-specific, it may be better left
client-owned. The assembly API should not erase that distinction.

---

## Suggested Deliverables For Future Agents

This section exists so this document can be delegated in pieces later.

### Agent track 1: dispatch and assembly shape

- audit `the-dispatch`
- audit `the-default/command.rs`
- propose concrete `Assembly<Ctx>` types
- propose how static and generic handler paths coexist

### Agent track 2: keymaps and actions

- audit `the-default/keymap.rs`
- audit `command_types.rs`
- propose user-defined action model
- propose keymap merging/override APIs

### Agent track 3: command palette and commands

- audit `command_registry.rs`
- propose custom command registration APIs
- propose reusable command modules

### Agent track 4: picker authoring

- audit `file_picker.rs`
- propose custom picker item action model
- propose picker authoring helpers

### Agent track 5: render and annotations

- audit `render/plan.rs`
- audit `render/text_annotations.rs`
- audit `render/overlay.rs`
- propose authoring helpers for AI-like features

### Agent track 6: gutters and layout lanes

- audit `render/gutter.rs`
- audit gutter usage in shared/default code
- propose extensibility strategy

### Agent track 7: UI intent and overlays

- audit `render/ui.rs`
- audit `post_ui` and UI event hooks
- propose better config-facing helpers

### Agent track 8: context/state/lifecycle

- audit `DefaultContext`
- propose extension-state API
- propose startup/install lifecycle hooks

---

## Final Position

The next step is not to turn config into plugins.

The next step is to make compile-time Rust detours feel like a first-class
editor assembly model.

That means:

- a real assembly surface
- a broad review of existing composition APIs
- opening the current closed or accidental contact surfaces
- preserving client freedom
- keeping the authoring surface at the same abstraction level as the editor
  itself

If this plan is followed, `the-config` stops being "a crate that overrides a few
hooks" and becomes "the place where the editor is authored".
