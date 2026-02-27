# Swift Shared LSP Broker Plan

## Objective

Keep the new Swift tab semantics (one editor instance per native tab) while running a single LSP server process per workspace/server configuration across all Swift tabs in the app process.

## Current State

Today each `App` owns its own `LspRuntime` and full LSP state:

- `App` has `lsp_runtime` and related state fields in `the-ffi/lib.rs`.
- `set_file_path` / buffer activation paths call `refresh_lsp_runtime_for_active_file`.
- `refresh_lsp_runtime_for_active_file` does `shutdown -> LspRuntime::new -> start`.

With one `App` per Swift tab, this creates one LSP process per tab and frequent restarts.

## Requirements

1. One LSP process per `(workspace_root, server_config)` in the Swift process.
2. Multiple editor instances can concurrently use the shared process.
3. Keep per-tab editor state isolated (buffers/splits/history unchanged).
4. Do not regress `the-term` behavior.
5. Maintain deterministic routing for responses, diagnostics, progress, and statusline.

## Non-Goals

1. Reworking all LSP features in one pass.
2. Cross-process LSP daemonization.
3. Changing command/UI semantics in `the-term`.

## Core Design

### 1) Process-global Broker Registry

Add a global registry in `the-ffi`:

- `LspBrokerRegistry`: map of `SessionKey -> Arc<Mutex<LspBrokerSession>>`
- `SessionKey` includes:
  - canonical workspace root
  - server executable/args/env identity
  - optional language/server selector

Each unique key owns one `LspRuntime` process.

### 2) Broker Session

`LspBrokerSession` responsibilities:

1. Own exactly one `LspRuntime`.
2. Start/shutdown lifecycle with ref-counted clients.
3. Poll runtime events once and fan them out to subscribed clients.
4. Route request responses back to originating client.
5. Track open/synced docs at broker level.

### 3) Per-Editor LSP Client Handle

Each `App` gets an LSP client handle:

- `lsp_client_id` (unique)
- `lsp_session_key` (optional)
- local UI state remains in `App`:
  - pending requests map
  - completion menu state
  - statusline presentation
  - hover/signature overlays

Only transport/process ownership moves to broker.

### 4) Document Ownership Model

Need explicit behavior when same file is open in multiple tabs.

Proposed rule:

1. Broker allows multiple subscribers to one URI.
2. One client is the sync owner for a URI at a time (focused/most-recently-active).
3. Owner sends `didOpen/didChange/didSave/didClose`.
4. Non-owner tabs can request hover/completion/etc, but sync ownership may transfer on focus.

This avoids duplicate `didOpen` and undefined server behavior.

## Migration Plan

## Phase 0: Guardrails and Feature Flag

1. Add `THE_EDITOR_SWIFT_SHARED_LSP=1` feature flag (default off initially).
2. Keep current path as fallback.
3. Add logging counters:
   - broker sessions created
   - active clients per session
   - lsp process starts/restarts

Exit criteria:

- No behavior change when flag is off.

## Phase 1: Extract Transport Layer

1. Introduce `the-ffi/lsp_broker.rs` with:
   - `SessionKey`
   - `LspBrokerRegistry`
   - `LspBrokerSession`
   - request id remap tables
2. Move raw runtime polling/start/stop into broker session.
3. Keep existing `App` LSP UI logic, but call broker for transport.

Exit criteria:

- Single `App` behavior unchanged.
- Existing LSP tests still pass.

## Phase 2: Multi-Client Registration

1. On `App::new`, allocate `lsp_client_id`.
2. On file activation/path changes, compute session key and register client.
3. If key changes, detach from old session and attach to new one.
4. Ref-count broker sessions and shutdown only when no clients remain.

Exit criteria:

- Two `App` instances in same workspace attach to one broker session.
- Process count remains one.

## Phase 3: Request/Response Multiplexing

1. Broker assigns runtime request ids and stores `(runtime_id -> client_id, client_req_id)`.
2. Responses/timeouts/cancel acknowledgements routed to correct client queue.
3. `App::poll_background` drains client-local event queue.

Exit criteria:

- Completion/hover/goto/code-actions work correctly in multiple tabs.
- No cross-tab response leakage.

## Phase 4: Shared Diagnostics and Progress Fanout

1. Diagnostics from broker are distributed to subscribed clients by URI.
2. WorkDone progress and server state fanout to all session clients.
3. Client filters and renders its own statusline/UI.

Exit criteria:

- Consistent diagnostics in all tabs for same file/workspace.
- No duplicate progress spam.

## Phase 5: URI Ownership and Sync Transfer

1. Implement URI sync ownership table in broker.
2. On focus change:
   - transfer ownership if needed
   - new owner pushes full text sync
3. Ensure `didClose` only when final owner/subscriber leaves.

Exit criteria:

- Same file open in two tabs does not corrupt broker state.
- Ownership transfer is deterministic and debounced.

## Phase 6: Swift-only Rollout, then Generalize

1. Enable flag from Swift bridge only (`the-swift` startup path).
2. Keep `the-term` on legacy mode initially.
3. After stabilization, optionally make broker default for all clients.

Exit criteria:

- Swift multi-tab: one LSP process, no correctness regressions.

## API and Code Changes

## `the-ffi/lib.rs`

1. Add broker module and `App` fields:
   - `lsp_client_id`
   - `lsp_session_key`
   - client-local inbound event queue
2. Replace direct runtime calls:
   - `dispatch_lsp_request`
   - `poll_lsp_events`
   - `refresh_lsp_runtime_for_active_file`
   with broker-mediated versions.
3. Keep `App` UI-facing state and rendering logic intact.

## Swift side

No architectural change required for per-tab editors. Optional:

1. Add bridge call to enable shared broker mode explicitly for Swift-created apps.
2. Turn on once at `EditorModel` init.

## Testing Plan

## Unit

1. Broker key normalization and lifecycle.
2. Request id remap and timeout routing.
3. URI ownership transitions.

## Integration (mock LSP)

1. Two Swift-tab-style `App` instances, same workspace:
   - assert one initialize sequence
   - assert one process start
2. Request routing isolation:
   - completion in tab A, hover in tab B, responses routed correctly.
3. Ownership transfer:
   - same file in both tabs, alternate focus, verify sync owner switches cleanly.

## Regression

1. Existing LSP command tests (`goto`, `hover`, `code_actions`, `symbols`).
2. File-watch behavior and self-save suppression windows.

## Risks and Mitigations

1. Same-URI multi-tab unsaved divergence.
   - Mitigation: explicit single-owner sync rule and full-sync on ownership transfer.
2. Deadlocks around shared broker mutex.
   - Mitigation: short critical sections, queue events outside lock.
3. Subtle response misrouting bugs.
   - Mitigation: strict request-id mapping invariants + tracing assertions in debug builds.

## Acceptance Criteria

1. In Swift, 2+ tabs in same workspace use one LSP process.
2. Opening/switching files no longer restarts process per tab.
3. Completion/hover/goto/code-actions remain correct per active tab.
4. Split/layout isolation between tabs remains unchanged.
