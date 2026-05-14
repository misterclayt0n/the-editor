# LSP Redesign Plan

This document defines the concrete redesign plan for LSP in `the-editor`.

It exists because the current shape still allows three failure classes:

1. typing stalls while an LSP request is in flight
2. completions observe stale document state
3. completion correctness depends on ad hoc transport timing instead of explicit module rules

This plan intentionally follows the seams that Zed gets right:

- document edits are pushed to LSP continuously
- completion requests are built from snapshots
- request waiting does not own editor state
- stale and content-modified responses are tolerated, not promoted to editor-wide failure

`COMPLETION.md` defines completion policy. This document defines the runtime and transport shape that completion depends on.

---

## Non-negotiable invariants

1. Typing must never wait on an LSP request.
2. Document edits must never be dropped because an LSP request is in flight.
3. Completion requests must observe a well-defined document version per server.
4. `content modified` / canceled completion responses are request-local noise, not server-health failures.
5. TypeScript must not know about lock timing, flush timing, or transport ordering.
6. Runtime editing code must not own LSP sync policy.

---

## Current structural problems

### Shared mutable LSP store as transport seam
The current `LspStore` mixes:

- process ownership
- open-document version tracking
- diagnostics/logs
- request waiting
- notification delivery
- completion transport

That interface is shallow. Callers still pay for transport details.

### Editing owns sync recovery
`the-runtime/session.rs` currently knows too much about LSP transport behavior:

- incremental sync attempt
- `try_lock()` failure handling
- pending full-sync queueing
- flush-before-request recovery

That is the wrong seam. Editing should say only: **document changed**.

### Completion still depends on transport quirks
`packages/default/src/completion.ts` currently compensates for missing runtime seams:

- request timing
- stale result handling
- request-local error interpretation
- incomplete-list policy

That makes the completion module larger than it should be.

---

## Target modules

## 1. LSP server actor module

One actor per language server process owns:

- child process handles
- stdin writer
- stdout/stderr readers
- request id allocation
- request/response correlation
- notification handling
- server-local open-document versions
- diagnostics/log accumulation
- lifecycle state

The actor interface should be message-based:

- `StartServer`
- `OpenDocument`
- `ChangeDocument`
- `CloseDocument`
- `Request`
- `Notify`
- `Shutdown`

Properties:

- transport ordering is serialized inside the actor
- waiting for a response does not lock editor state
- request-local failures stay request-local
- actor state is the only owner of server-local document version truth

This replaces the current "shared store + blocking request" seam.

## 2. LSP document mirror module

A separate module owns the relationship between editor documents and each server.

It owns:

- per-document, per-server version
- previous text snapshot for diffing
- incremental/full-sync decision
- coalesced fallback when incremental sync cannot be sent immediately
- document-open/close state per server

Its interface should look like:

- `document_opened(document, path, text, language)`
- `document_edited(document, revision, transaction)`
- `document_closed(document)`
- `pending_change_for(server, document)`
- `snapshot_for_request(server, document)`

Properties:

- editing does not build LSP payload policy inline
- completion/hover/formatting consume one consistent server-aware document view
- fallback full sync is a mirror concern, not a session concern

## 3. LSP health/cache module

A separate module owns:

- server lifecycle state
- cached completion capabilities
- cached trigger characters
- request-local error classification
- diagnostics freshness

It must distinguish:

- server startup failure
- server transport failure
- request canceled
- content modified
- timeout

Completion should depend on cached capability state and lifecycle state, not on raw request exceptions.

## 4. Completion adapter module

Completion should talk to LSP through a smaller seam than `editor.lsp.request(...)` plus ad hoc interpretation.

It should own:

- completion request construction
- stale request id handling
- request-local `content modified` tolerance
- item normalization
- resolve behavior
- incomplete-list requery policy

`completion.ts` should not own transport semantics.

---

## End-to-end flows

## Open document

```txt
resource/document opens
  -> document mirror marks document open for matching servers
  -> server actor receives didOpen
  -> health cache records capability state
```

## Edit document

```txt
runtime edit applied
  -> document mirror records new revision + diff basis
  -> server actor receives didChange when ready
  -> if actor is busy, mirror coalesces pending change for that server/document
  -> typing continues immediately
```

## Request completion

```txt
completion snapshot derived on host
  -> completion adapter asks mirror for server-aware request snapshot
  -> actor flushes pending change for that document/server before request
  -> actor sends textDocument/completion
  -> response returns to adapter
  -> stale/content-modified responses are ignored or retried per policy
```

## Response handling

```txt
response/error arrives
  -> actor correlates request id
  -> adapter classifies request-local result
  -> completion session ignores stale ids
  -> health cache updates only for real server failures
```

---

## Implementation phases

## Phase 1 — Replace blocking LSP store seam

### Goal
Move from `LspStore` as a blocking shared mutable transport owner to per-server actors.

### Files likely touched
- `the-runtime/lsp.rs`
- `the-runtime/lib.rs`
- `packages/runtime-node/src/lib.rs`

### Work
- Introduce actor-owned server state.
- Replace blocking `request()` loop ownership with actor message routing.
- Keep request API shape stable if possible while changing implementation underneath.
- Keep diagnostics/log/status reading actor-backed.

### Delete
- request paths that require a caller-visible `Mutex<LspStore>` critical section
- transport logic that treats a shared lock as ordering

### Acceptance criteria
- no LSP request wait holds document/surface state hostage
- status/log/diagnostic reads do not need to know request timing
- completion requests can run while typing continues

## Phase 2 — Introduce LSP document mirror

### Goal
Move document-to-server sync policy out of `RuntimeSession` editing code.

### Files likely touched
- `the-runtime/session.rs`
- `the-runtime/lsp.rs`
- maybe a new `the-runtime/lsp_document.rs`

### Work
- add mirror state per document/server
- track previous snapshot + server-local version
- move incremental/full fallback there
- move pending-change coalescing there

### Delete
- `try_lock()`-driven sync policy in `RuntimeSession`
- ad hoc pending full-sync queueing in session code

### Acceptance criteria
- editing code emits document-change events only
- mirror owns full/incremental/coalesced sync policy
- no edit path stringifies full text unless the mirror explicitly chooses a full-sync fallback

## Phase 3 — Split completion by seam

### Goal
Make completion consume snapshots and the LSP completion adapter, not transport details.

### Files likely touched
- `packages/default/src/completion.ts`
- `packages/default/src/surfaceSnapshot.ts`
- new completion submodules in `packages/default/src/`

### Work
Split into:
- completion session module
- completion query module
- LSP completion adapter module
- completion ranker module

Port ranking semantics and tests from Zed where applicable.

### Delete
- giant one-file completion controller ownership of every concern
- ad hoc request-local LSP error interpretation inside UI/session code

### Acceptance criteria
- automatic completion never reads runtime state directly
- active-session filtering is in-memory only
- ranking behavior is covered by pure tests
- screenshot regressions like `piped`, `unwrap_or_else`, and stale Rust completions get explicit tests

## Phase 4 — Add end-to-end stall timeline

### Goal
Make the next freeze diagnosable in one pass.

### Files likely touched
- `packages/default/src/perfDiagnostics.ts`
- `packages/editor/src/runtimeClient.ts`
- `apps/electron/src/main/main.ts`
- input/interaction/command modules

### Work
Trace one interaction across:
- input receipt
- keymap/interaction resolution
- command dispatch
- runtime mutation
- completion request
- workbench invalidate
- frame publish

### Acceptance criteria
- a freeze produces one interaction timeline instead of scattered category logs

---

## Temporary stopgaps to remove

These are acceptable only as migration scaffolding:

- pending full-document sync queue in `RuntimeSession`
- flush-before-request logic in runtime-node
- request-local error handling smeared across runtime and completion layers

The redesign is not done while these remain load-bearing.

---

## Test plan

### Runtime/LSP tests
- edits are not dropped while a completion request is in flight
- per-server versions advance monotonically
- incremental change payloads match the previous snapshot basis
- full-sync fallback replaces pending incremental noise cleanly
- content-modified response does not poison server health

### Completion tests
- active session filters in memory only
- no automatic runtime read when snapshot exists
- `filterText` beats label when appropriate
- exact-match and sortText ordering match expected Rust/LSP behavior
- stale completion responses do not publish
- incomplete requery happens only after pause

### Diagnostics tests
- server startup failure updates health
- request cancellation/content-modified does not surface as server failure

---

## Sequence constraints

1. Do **not** spend more time tuning ranking inside the current transport seam than needed for triage.
2. Do **not** reintroduce synchronous runtime reads on typing.
3. Do **not** keep the current `RuntimeSession`/LSP recovery path as the long-term architecture.
4. Do **not** keep completion as the place where LSP transport semantics are interpreted.

---

## Relationship to other docs

- `ARCHITECTURE.md` defines why LSP belongs in `the-runtime`.
- `RUNTIME.md` defines runtime ownership and async-service rules.
- `COMPLETION.md` defines completion policy above this transport shape.

This document is the concrete redesign plan for getting from the current stopgaps to the target seams.
