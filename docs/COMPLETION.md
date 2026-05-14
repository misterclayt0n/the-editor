# Completion Architecture

This document records the target autocomplete architecture for `the-editor`.

The completion system previously caused typing freezes because autocomplete was allowed to call into runtime and LSP from the keypress path. This document intentionally favors deletion and stronger invariants over compatibility shims.

---

## Problem statement

Completion must feel like Zed: suggestions stay open and narrow while typing, without blocking editing.

The current implementation failed that goal because it mixed these responsibilities in one shallow module:

- command-dispatch hooks
- debounce policy
- runtime context acquisition
- provider fanout
- LSP lifecycle and requests
- incomplete-list policy
- selected-item resolve
- overlay/session publication
- code-action picker behavior
- buffer-word completion behavior

The observed bad path was:

```txt
type key
  -> commands.afterDispatch
  -> schedule completion
  -> showOrRefreshCompletion
  -> readCompletionContext
  -> runtime.surfaces.readState
  -> maybe LSP request/resolve
```

`surfaces.readState()` is not a cheap local snapshot read. In the native client it performs synchronous NAPI calls. The native runtime currently protects `RuntimeSession` with one mutex, and blocking LSP request code can hold that mutex while waiting for a language server response. Therefore completion context reads can block behind unrelated LSP/runtime work and freeze typing.

Representative evidence:

```txt
[completion] context { ms: 831.6, textLength: 4365, query: 'rejectPend', trigger: 'input' }
[completion] provider { sourceId: 'default.lsp', ms: 1.6, items: 0 }
[completion] show end { ms: 912, reason: 'empty' }
```

and after making active sessions sticky:

```txt
[completion] schedule { trigger: 'delete', delayMs: 160 }
[completion] show start { trigger: 'delete' }
[completion] context { ms: 211.5, query: '' }
[completion] show end { ms: 211.6, reason: 'no-context-or-closed' }
```

That delete path touched runtime just to learn that the query was empty. That must not happen.

---

## Reference architecture lessons

### Zed

Zed's completion path is snapshot/task based:

- completion query is derived from an in-memory buffer snapshot and anchors
- existing completion menus filter in memory
- provider calls are task based and identified by completion ids
- stale results are ignored
- provider relevance is checked before provider work
- LSP trigger characters come from cached buffer/server capability state
- the menu is created only after non-empty results
- word completion uses bounded snapshot work, not full synchronous runtime reads

The important lesson is not to copy every Zed feature. The important lesson is the **seam**: typing and filtering cross a cheap snapshot interface, not a synchronous runtime/LSP interface.

### Helix

Helix has a completion handler with explicit events, debouncing, task handles, and cancellation. It checks LSP completion capabilities and trigger characters before requests, filters active completions in memory, and does not show an empty completion UI.

The important lesson is that completion lifecycle is explicit state, not generic source fanout plus full context reads.

---

## Non-negotiable invariants

1. **Automatic and active completion must never call `runtime.surfaces.readState()`.**
2. **Automatic completion must not touch runtime/LSP if no relevant cached provider exists.**
3. **Typing must stay responsive above all else.**
4. **Completion UI must not open until there are actual completions, except for deliberate manual requests.**
5. **Existing completions must filter in memory while typing.**
6. **LSP trigger characters must come from server capabilities.**
7. **LSP `isIncomplete` is advisory, not permission to re-request on every character.**
8. **Selected-item resolve/documentation must not run eagerly on every publish.**
9. **Code actions are not autocomplete.**
10. **Buffer-word completion stays deleted until it can satisfy the same snapshot/background invariants.**
11. **Completion diagnostics logging is off by default.**

If preserving an old interface violates these invariants, delete or quarantine the old interface.

---

## Target modules

### Surface snapshot module

A deep host-side module owns cached surface state needed by product features.

It exposes cheap reads such as:

- active surface id
- path
- revision
- text
- selection
- active cursor id
- active cursor offset
- completion query at cursor
- line prefix/suffix

Completion consumes this module and does not call runtime to discover context.

The snapshot is updated by runtime mutation results and normal workbench refreshes. If the snapshot is missing, stale, or ambiguous, automatic completion does nothing. Manual completion may use an explicit slower path.

### Runtime mutation updates

Runtime mutation calls should return the updated surface state or a surface update.

Target shape:

```ts
runtime.surfaces.applyOperation(...) -> SurfaceState | SurfaceUpdate
runtime.surfaces.applyEdit(...)      -> SurfaceState | SurfaceUpdate
```

Start with full `SurfaceState` if that is the simplest correct implementation. Optimize to deltas later.

The command pipeline should become:

```txt
edit command
  -> runtime mutation
  -> returned surface update
  -> host surface snapshot update
  -> completion/filtering reads host snapshot only
```

Completion should not reconstruct authoritative state from dispatch args plus runtime reads.

### Runtime/LSP lock split

LSP request/resolve must not hold the same native lock as document/surface state while waiting for a language server.

Invalid implementation:

```txt
lock RuntimeSession
  -> send LSP request
  -> wait for LSP response
  -> unlock RuntimeSession
```

Target implementation:

- surface/document operations are protected independently from LSP request waits
- LSP process/session state has its own lock or owner channel
- LSP requests are async/cancellable from the TypeScript perspective where feasible
- stale completion request results are ignored by completion request id/session id

This is required in the same redesign pass as completion. Completion-side caching alone is not enough if LSP can still block rendering/surface reads elsewhere.

### Completion controller

The default completion controller is a single explicit module, not a generic source fanout loop.

It owns:

- active session
- cached context snapshot
- request id
- all provider items
- filtered visible items
- selected index
- incomplete flag
- cached trigger characters/provider capabilities
- delayed requery timer
- delayed resolve timer

Autocomplete lanes:

1. **LSP lane**
   - automatic + manual
   - automatic only when a cached running/initialized LSP completion provider exists
   - manual may start/ensure LSP
   - trigger characters only from cached server capabilities

2. **Existing-session filter lane**
   - in-memory only
   - no runtime reads
   - no provider calls
   - used for normal typing/deleting while a menu is active

3. **Delayed incomplete requery lane**
   - only when previous LSP response is incomplete
   - scheduled after a pause, not every typed character
   - stale-safe by request/session id
   - does not close or block the current menu while pending

4. **Manual slow lane**
   - explicit user request only
   - may refresh snapshot and start/ensure LSP
   - may show loading/empty UI because the user requested it

Resolve/documentation behavior:

- do not resolve after every publish
- resolve on accept
- optionally resolve after stable selection delay
- resolve must use cached context and stale guards

### Code actions

Code actions must be separate from autocomplete.

They should use their own picker/session path. They must not register as or be invoked through a completion source in the default autocomplete controller.

### Buffer words

Buffer-word completion remains deleted.

It can return only as a separate lane that uses snapshots, bounded search ranges, background work, and deduplication against LSP items. It must not require runtime text reads on keypress.

---

## Source registry decision

The existing `CompletionSource` registry is not the default autocomplete architecture.

The registry made the wrong seam deep: it hid provider fanout behind a generic interface but forced callers to know too much about scheduling, context construction, trigger semantics, empty UI, stale results, and provider-specific behavior.

Default autocomplete must ignore `editor.completion.sources()`.

Allowed future options:

- delete the source registry entirely
- keep only presentation/customization APIs
- reintroduce extension completion through a stricter snapshot-based provider interface later

Do not preserve the current source registry with compatibility shims if it conflicts with the invariants above.

---

## Empty UI policy

Automatic completion:

- no loading popup before results
- no empty popup after zero results
- close existing menu when filtered items become empty

Manual completion:

- may show loading
- may show empty state
- may start providers

---

## Logging policy

Completion logs are disabled by default.

A debug flag may expose coarse events:

- request id
- trigger kind
- context source (`cached`, `manual-refresh`)
- provider latency
- stale/cancel reason

No per-key log spam by default.

---

## Implementation checklist

- [ ] Add a host-side surface snapshot module.
- [ ] Change runtime mutation calls to return surface updates.
- [ ] Update command/editing paths to write returned updates into the host snapshot.
- [ ] Split native LSP request waiting away from the document/surface runtime lock.
- [ ] Replace default completion controller with an explicit LSP-first controller.
- [ ] Remove automatic/active `readState()` from completion.
- [ ] Remove generic completion source fanout from default autocomplete.
- [ ] Move code actions to their own picker/session.
- [ ] Keep buffer-word completion deleted.
- [ ] Disable completion logging by default.
- [ ] Add tests for no-runtime-read autocomplete filtering, no-provider quiet typing, trigger-character gating, stale LSP results, delayed incomplete requery, and manual slow path.
