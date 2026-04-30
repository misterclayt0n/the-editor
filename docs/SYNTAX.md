# Syntax Module Architecture

This document defines the target long-term architecture for the syntax subsystem in `the-lib` and its orchestration in `the-runtime`.

It answers these questions:

1. What is the real syntax truth of a document?
2. What belongs in `the-lib`, `the-runtime`, `Surface`, and render?
3. How should parsing, injections, highlighting, and enrichment be split?
4. How do we support large files and pathological injection-heavy cases without destroying typing latency?
5. What should replace the current eager full-document highlight pipeline?

This document is intentionally opinionated. Syntax is one of the largest and most performance-sensitive parts of the editor. If its boundaries are vague, large-file performance and feature correctness both rot.

---

## Purpose

The syntax subsystem defines the native, document-owned model for:
- language assignment and detection
- persistent tree-sitter-backed syntax truth
- incremental parse and injection maintenance
- lazy compilation of language query helpers
- byte-oriented syntax extraction for higher-level consumers
- pure language metadata needed by text-bound editor features

It is responsible for:
- language registry/detection
- document-local syntax truth
- persistent syntax layer graph maintenance
- explicit invalidation tracking
- generation/freshness tracking
- query helper compilation and caching
- baseline structural extraction APIs
- pure query capabilities used by editor features

It is **not** responsible for:
- filesystem grammar/query acquisition
- async scheduling or background worker management
- viewport ownership
- render row caches
- theme/style mapping
- client painting
- whole-editor command/product policy

---

## Core position

Syntax is **document-owned derived text truth**.

That means:
- syntax belongs to `Document`, not `Surface`
- many surfaces may share one document and therefore one syntax truth
- syntax state must remain independent of selection, viewport, render caches, and product/UI concerns

The architecture should read like this:

```txt
Document      = text truth + syntax truth
Surface       = per-view/editor-session state + visible syntax caches
CellGridRenderSurface = retained render subsystem, syntax-agnostic
Runtime       = syntax scheduling/orchestration and asset loading
```

This is the crucial split:
- **`Document` owns persistent syntax truth**
- **`Surface` owns visible extracted projections**
- **`the-runtime` owns scheduling, demand prioritization, and async advancement**

---

## The problem with the current/v1 shape

The current vertical slice proved correctness and integration, but it has the wrong performance shape for a complete implementation.

The bad pattern is:

```txt
edit
-> reparse document
-> recollect full-document highlights
-> convert all ranges to render-facing spans
-> push payloads into surfaces
```

That shape is too eager.

It couples typing cost to:
- total file size
- total highlight span count
- total injection complexity
- total locals/enrichment complexity
- number of attached surfaces

This is exactly how large files and injection-heavy files end up lagging even when incremental parsing itself is cheap.

The root parse is not the real problem. The expensive parts are the post-parse query passes and the choice to materialize too much too early.

---

## Final model

The complete model has four layers:

```txt
1. Persistent syntax truth
2. Incremental syntax advancement
3. Visible-range extraction
4. Render-facing retained visible caches
```

### 1. Persistent syntax truth
Document-owned. Pure. Byte-oriented.

### 2. Incremental syntax advancement
Document defines the pure state machine; runtime drives it with bounded foreground work and background completion.

### 3. Visible-range extraction
Surface/runtime pull byte-oriented visible runs from syntax truth on demand.

### 4. Render-facing retained visible caches
Surface converts extracted runs into row-local render payloads by projecting them through the current visible layout; `CellGridRenderSurface` remains syntax-agnostic.

---

## Ownership split

### `the-lib::syntax` owns
- pure language metadata
- `LanguageId`, `LanguageRegistry`, `LanguageDefinition`
- language detection rules
- persistent syntax layer graph
- parse/injection invalidation truth
- syntax/enrichment generations
- snapshot/apply protocols for background advancement
- byte-oriented extraction APIs
- feature query helpers (indent/textobjects/tags/etc.)

### `Document` owns
- `DocumentSyntax`
- all syntax truth attached to one text buffer
- syntax invalidated regions
- syntax freshness state
- no viewport-specific highlight caches

### `Surface` owns
- viewport and demand reporting
- visible baseline syntax cache
- visible enrichment cache
- byte->char/visual conversion near render
- adaptation of syntax extraction into visible-layout-backed render payloads

### `CellGridRenderSurface` owns
- retained visible render state only
- projection/diffing of row-local syntax spans from the current visible layout
- no direct knowledge of `Document` syntax internals
- no document-global syntax payload truth

### `the-runtime` owns
- grammar/query asset loading
- syntax scheduler/work queue per document
- tiny bounded foreground advancement
- background advancement/completion
- stale-result rejection
- demand-aware prioritization from surfaces
- large-file/pathological degradation policy

---

## Main decisions

### 1. Split syntax truth from syntax scheduling
**Decision:** yes.

`the-lib` owns the syntax state machine.
`the-runtime` owns when work happens.

`Document::apply_transaction(...)` must stay cheap and must not secretly become a parse scheduler.

---

### 2. Keep syntax-side language metadata broader than parse mechanics
**Decision:** yes.

Keep pure language-local editor metadata in syntax-side config, including things like:
- comment tokens
- block comment tokens
- indent config
- auto-pairs config
- text width / soft wrap / rulers
- rainbow bracket config

Do **not** keep runtime/service/tool config here.

---

### 3. Keep editor-owned syntax identity types
**Decision:** yes.

Keep editor-owned public concepts like:
- `LanguageId`
- `LanguageRegistry`
- `LanguageDefinition`
- `DocumentSyntax`
- `SyntaxState`

Do not let backend-specific handles become the public syntax vocabulary.

---

### 4. Keep syntax document-scoped and selection-free
**Decision:** yes.

Syntax truth stays rooted in:
- text
- language assignment
- parse/injection/enrichment state
- extraction/query capability

Not in:
- selection
- viewport
- render payloads
- surface session state

---

### 5. Make injections first-class persistent syntax truth
**Decision:** yes.

Injections are not optional decoration. They affect:
- nested language structure
- text objects
- indent behavior
- bracket behavior
- highlighting
- later semantic feature routing

So they belong in syntax truth.

But they must be maintained:
- incrementally
- range-bounded
- layer-reusing
- not by full-document rescans after every edit

---

### 6. Support combined injections as real incremental citizens
**Decision:** yes.

No brute-force fallback architecture for combined injections.

The complete model should support:
- included subranges
- incremental range splicing
- reuse of existing combined layers
- precise invalidation and reparse behavior

---

### 7. Replace eager full-document highlight collection
**Decision:** yes.

The old center of gravity:

```txt
collect full-document highlights -> push into surfaces
```

must be replaced.

The new center of gravity is:

```txt
maintain persistent syntax truth -> extract visible runs on demand
```

This means whole-document `Vec<SyntaxHighlight>` is no longer the primary truth shape.

---

### 8. Keep baseline highlighting cheap and structural
**Decision:** yes.

Baseline syntax highlighting should be:
- fast
- syntax-tree/capture based
- visible-range extracted
- byte-oriented at the core
- good enough for immediate typing feedback

It should not require slow locals-sensitive global analysis on the typing hot path.

---

### 9. Keep locals-sensitive highlighting, but demote it hard
**Decision:** yes.

Locals-aware coloration remains useful, but it is **tier-2 enrichment**, not baseline syntax paint.

That means it is:
- optional
- stale-tolerant
- async/deferred
- not required for immediate feedback
- replaceable/overridable by semantic tokens later

---

### 10. Make semantic tokens a first-class sibling overlay
**Decision:** yes.

The complete model should have:
- fast baseline tree-sitter structural highlighting
- semantic/LSP enrichment as a first-class overlay lane

Do not treat semantic tokens as an awkward afterthought bolted on top of final tree-sitter colors.

---

### 11. Keep syntax consumer lanes separate
**Decision:** yes.

There is one shared syntax truth, but different consumers have different performance/freshness needs.

Examples:
- visible baseline highlighting: near-immediate, viewport-priority
- locals/semantic enrichment: async, stale-tolerant
- indent: cheap and fallback-tolerant
- bracket matching: very local and bounded
- text objects: interactive and local
- outline/tags: can lag more

Do not collapse all syntax-derived work into one giant binary “syntax fresh” bit.

---

### 12. Keep byte orientation until near render
**Decision:** yes.

The syntax core should expose byte-oriented truth and extraction first.

Late conversion near surface/render should handle:
- byte -> char
- char -> row/column
- row-local clipping
- measured placement

Do not pay whole-document byte->char conversion costs in runtime just to create surface payloads.

---

### 13. Prefer streaming extraction APIs over eager collectors
**Decision:** yes.

The natural extraction API shape should be:
- iterator / visitor / streaming callbacks over a requested byte range

Bulk `Vec` collection may exist as convenience, but it must not be the architectural default.

---

### 14. Keep `Surface` as the syntax-to-render adapter
**Decision:** yes.

`CellGridRenderSurface` should not talk directly to `Document` syntax.

Instead:
- `Document` exposes syntax truth/extraction
- runtime advances freshness
- `Surface` requests visible extraction for its viewport
- `Surface` projects that through the current visible layout into retained render-facing payloads
- `CellGridRenderSurface` diffs and emits those payloads

---

### 15. Keep `CellGridRenderSurface` syntax-agnostic
**Decision:** yes.

`CellGridRenderSurface` should retain rows/layers and diff row-local syntax payloads, but not own syntax query policy or syntax truth.

This preserves the clean object split:
- document = truth
- surface = editor/view adapter
- render surface = retained renderer

---

### 16. Keep `Document` lean; do not let it become a presentation cache bucket
**Decision:** yes.

`Document` should own:
- syntax truth
- invalidation truth
- generations
- reusable document-scoped semantic/enrichment truth when justified

It should not default to owning:
- viewport caches
- row-local highlight slices
- render-facing syntax spans
- per-surface extracted visible projections

---

### 17. Keep extraction caches outside `Document` by default
**Decision:** yes.

Visible extraction caches belong on `Surface` unless a cache is clearly:
- document-global
- broadly reusable across surfaces
- expensive enough to justify
- not tied to viewport presentation

---

### 18. Tree-house should stop being the architecture center
**Decision:** yes.

The editor should own:
- its syntax layer graph
- invalidation model
- extraction APIs
- scheduling contract

A third-party highlighter/query worldview must not remain the center of the design.

The implementation now directly owns the syntax backend around local tree-sitter bindings and no longer depends on `tree-house` as either architecture center or dependency-graph backend.

---

## Language registry and pure metadata

The language registry remains a central pure object.

It should continue to own:
- registered language definitions
- detection rules by name/scope/file/shebang/glob/content regex
- query text lookup and inheritance expansion
- lazy compiled query helpers
- semantic highlight vocabulary / capture vocabulary

Detection precedence remains:
1. explicit override / explicit assignment
2. name
3. scope
4. exact filename / extension
5. shebang
6. glob
7. injection/content regex

Registry validation remains strict for:
- duplicate language ids/names
- duplicate scopes
- duplicate exact extensions
- duplicate exact shebangs

and pragmatic for:
- globs (longest match wins)
- content/injection regexes (longest match wins)

---

## Syntax truth model

The syntax subsystem should move from “a parse tree plus some query helpers” to an explicit layered model.

### Persistent syntax layer graph
A document’s active syntax truth should explicitly track:
- root parsed layer
- injected child layers
- combined injection layers
- per-layer language
- per-layer covered ranges
- included subranges for combined layers
- unresolved/pending injections where applicable
- per-layer invalidation state

This is the core truth that everything else consumes.

### Why explicit layers matter
If injections are the performance problem, injections cannot remain a hidden implementation detail.

Explicit layers are what make it possible to:
- invalidate only affected subtrees/subranges
- reuse injected layers across edits
- update combined injections incrementally
- answer feature queries against the real layered syntax truth
- extract only visible syntax runs later

---

## Freshness and generations

The complete model needs multiple freshness axes.

At minimum, conceptually distinguish:
- text revision
- structural parse generation
- injection generation
- baseline extraction generation
- enrichment generation

Do **not** collapse these into one fuzzy `accurate: bool` if that loses necessary information.

That does **not** force one giant public mega-enum. Prefer orthogonal state facts.

Examples of real situations:
- text changed, parse is stale
- parse is current, injections are stale
- syntax truth is current, visible surface cache is stale
- baseline is current, semantic enrichment is stale

The architecture must represent those distinctions honestly.

---

## Invalidation model

A boolean `parse_pending` is not enough.

Syntax truth should explicitly track invalidated regions, at least as coalesced byte ranges, and possibly also deeper per-layer invalidation facts later.

At minimum we need to know:
- what text ranges are stale
- whether the staleness affects parse structure, injection structure, enrichment, or multiple lanes

This is how the scheduler avoids paying globally for local edits.

---

## Document-facing syntax model

The high-level document-facing syntax model should stay explicit.

### `DocumentSyntax`
Conceptually:
- unassigned
- assigned but unavailable
- assigned and active

But the active branch now carries a much richer internal state than v1.

### `DocumentSyntaxState`
An active state should carry:
- persistent syntax layer graph
- invalidated regions
- parse freshness generations
- injection freshness generations
- enrichment freshness generations
- snapshot/apply protocol support

The public type names may stay close to:
- `DocumentSyntax`
- `DocumentSyntaxState`
- `SyntaxUnavailableReason`
- `SyntaxState`
- `SyntaxSnapshot`

But their semantics should evolve beyond the v1 “tree plus parse_pending bit” worldview.

---

## `Document::apply_transaction(...)`

The typing hot path must change.

When a transaction is applied:
1. apply the text change
2. bump the text revision if text changed
3. map/interpolate existing syntax state through the edit
4. update invalidated regions
5. mark the necessary syntax lanes stale
6. **do not perform an unbounded full parse/highlight recomputation**

This is the non-negotiable typing-path rule.

The document edit contract should be:
- cheap
- deterministic
- non-blocking
- syntax-coherent but not necessarily syntax-fresh

---

## Snapshot / compute / conditional-apply protocol

The pure syntax core should expose an explicit protocol for async advancement.

### The pure side owns
- taking immutable syntax work snapshots
- defining what constitutes a valid source generation
- applying parse/injection/enrichment advances
- rejecting stale results cleanly

### Runtime owns
- when snapshots are taken
- what work is scheduled
- where work runs
- when results are attempted/applied

This means `the-lib` owns the syntax state machine and `the-runtime` only orchestrates it.

---

## Runtime syntax scheduler model

`the-runtime` should own a real syntax scheduler per document.

It should be responsible for:
- pending syntax work queues
- coalescing rapid edits
- at most one in-flight advancement job per document/lane where appropriate
- tiny bounded foreground work
- background completion for the rest
- stale-result rejection by generations/snapshots
- demand-aware prioritization from surfaces
- degradation policy for pathological cases

The runtime should not keep the current ad hoc shape of “after edit, refresh syntax/highlights”.

---

## Foreground vs background policy

The complete model should be budgeted.

### Foreground
Allowed:
- small bounded advancement work
- enough to keep the currently visible editing experience feeling responsive

### Background
Responsible for:
- completing structural parse freshness
- injection maintenance beyond tiny foreground budgets
- tier-2 locals/semantic enrichment
- cleanup of broader stale regions

The typing path must not wait for whole-file syntax richness.

---

## Demand-driven scheduling

Surface view demand should be explicit.

Surfaces should report syntax interest such as:
- current viewport byte range
- margin around viewport
- maybe cursor-near priority
- active/inactive/hidden priority hints

Scheduler policy should treat these as separate from invalidation.

The fundamental prioritization rule is:

```txt
first:  stale ∩ demanded
then:   stale near demanded
then:   broader stale background work
```

This gives the best user-facing behavior under load.

---

## Visible-range extraction model

The old API shape:

```rust
collect_document_highlights(&document) -> Vec<...>
```

should be replaced by byte-range extraction.

### Distinct levels
There are three different layers of output and they should not be conflated.

#### 1. Syntax truth
- layered parse trees
- query capabilities
- byte-oriented categories/captures

#### 2. Extraction result
- “for byte range X..Y, here are baseline/enrichment runs”
- still not row-local render payloads
- still surface-independent by default

#### 3. Render payload
- clipped to viewport
- row-local columns
- measured placement if needed

A dedicated extraction-layer concept should exist between syntax truth and render payload.

---

## Baseline highlighting lane

Baseline highlighting should be:
- structural
- viewport/range extracted
- synchronous if strictly bounded
- streaming
- cache-friendly

It is acceptable for a stale visible baseline cache to be rebuilt synchronously on demand during rendering **only if** it is:
- viewport-bounded
- range-streaming
- not secretly a whole-document pass

This is the correct analogue of Zed’s visible-range highlighting model for our setup.

---

## Enrichment lane

Tier-2 enrichment includes things like:
- locals-sensitive highlight refinements
- semantic token overlays
- other document-scoped semantic classifications

Enrichment truth should be document-owned when it is document-local and reusable across surfaces.

Surfaces should only own visible extracted projections of that enrichment.

This keeps multi-surface behavior coherent and avoids duplicated surface-local enrichment work.

---

## Large-file and pathological-file policy

Even a good architecture needs overload policy.

The system should support explicit degraded modes such as:
- baseline only
- defer injection refresh beyond budget
- pause tier-2 enrichment
- viewport-first structural freshness only
- recover fuller richness when idle

These are runtime policies, not excuses to corrupt syntax truth or text editing correctness.

Correctness of editing must remain intact.
Syntax richness/freshness may degrade temporarily.

---

## Feature consumers

The persistent layered syntax truth is not only for highlighting.

Over time, syntax-derived features should query the same truth for:
- indent
- bracket matching
- text objects
- structural object movement
- tags/outline
- later semantic helpers

Do not let each feature reconstruct a private syntax worldview.

One syntax truth, many consumers.

---

## Error model

Editor-owned `SyntaxError` remains appropriate for:
- invalid syntax config
- invalid query compilation
- document-level syntax state transitions
- explicit syntax advancement/apply failures

Raw backend query/tree/node types remain acceptable where they are the most honest low-level interface, but they should no longer define the top-level architecture.

---

## `SyntaxResources`

`SyntaxResources` should remain small and dumb.

It should provide only raw asset access such as:
- grammar lookup
- query text lookup

It should not own:
- scheduling
- cache policy
- feature policy
- render policy

Runtime-backed implementations belong in `the-runtime`.

---

## Module structure direction

The exact file split can evolve, but the architecture wants something roughly like:

```txt
the-lib/syntax/
  mod.rs
  config.rs
  resources.rs
  registry.rs
  state.rs            # document-facing syntax state machine + generations
  graph.rs            # persistent syntax layer graph
  invalidation.rs     # stale ranges / lane invalidation
  snapshot.rs         # syntax work snapshots / apply protocol
  extract.rs          # streaming byte-range baseline extraction
  enrichment.rs       # tier-2 enrichment truth contracts
  edits.rs            # parser edit generation helpers
  indent_query.rs
  textobject_query.rs
  tag_query.rs
  rainbow_query.rs
```

This does **not** mean all files must appear immediately.
It means the architecture has these separable concepts now.

---

## What should move out of the old/current center

The following should stop being central architecture:
- opaque highlighter-driven whole-document highlight collection
- surface-owned document-global syntax span payloads
- synchronous “refresh syntax/highlights after every edit” runtime flow
- locals-sensitive full-pass coloration as baseline paint
- tree-house as the defining syntax worldview

---

## Testing split

### Keep in `the-lib`
- language registry validation/detection tests
- syntax graph correctness tests
- invalidation and interpolation tests
- injection reuse and combined-injection tests
- snapshot/apply stale-rejection tests
- streaming extraction correctness tests
- deterministic syntax simulation tests

### Keep in `the-runtime`
- filesystem/runtime grammar/query acquisition
- scheduling/coalescing policy tests
- demand-prioritization tests
- degradation-policy tests
- document->surface syntax freshness fanout behavior

### Keep at surface/render boundaries
- visible extraction cache invalidation tests
- viewport-clipped syntax projection tests
- retained syntax-layer cache coherence tests

---

## Recommended implementation order

1. Redefine the architecture around document-owned syntax truth, surface-owned visible caches, and runtime scheduling.
2. Replace full-document pushed syntax payload assumptions in docs and APIs.
3. Introduce explicit syntax graph / invalidation / generation concepts in `the-lib`.
4. Make `Document::apply_transaction(...)` strictly cheap and advancement-oriented, not reparsing-oriented.
5. Add snapshot/apply protocol for syntax advancement.
6. Add visible-range streaming extraction APIs for fast baseline highlighting.
7. Replace surface/render syntax input from whole-document span vectors to visible caches.
8. Add runtime scheduler/work queue and demand reporting from surfaces.
9. Add tier-2 enrichment lanes and semantic-token overlay support.
10. Migrate feature consumers toward the shared layered syntax truth.
11. Keep deepening the direct tree-sitter-owned implementation until injected layer maintenance and enrichment lanes are fully first-party as well.

---

## Summary

Syntax truth should no longer be modeled as:

```txt
parse tree + eager full-document highlight vector
```

The complete model is:

```txt
Document-owned persistent layered syntax truth
+ explicit invalidation and generations
+ runtime-owned budgeted advancement and scheduling
+ surface-owned visible extraction caches
+ syntax-agnostic retained rendering
+ tier-1 structural baseline
+ tier-2 semantic/locals enrichment
```

That is the architecture that gives us:
- good large-file behavior
- correct injection support
- multi-surface coherence
- clean runtime scheduling
- a much better foundation than the current eager syntax payload path
