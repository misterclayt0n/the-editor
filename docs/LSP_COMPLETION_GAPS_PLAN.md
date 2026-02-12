# LSP Completion Gap Closure Plan

## Problem Summary

Current completion behavior works for manual trigger (`C-x`) but still has major gaps:

- no auto-trigger while typing
- no robust request context (all requests look "manual" to LSP)
- no stale-response guard against cursor drift
- insert-mode UX is incomplete (`ret`/arrow navigation depend on UI focus path)
- popup placement/rendering quality still needs a dedicated pass

## Constraints

- Keep command/input behavior declarative through keymap + dispatch where possible.
- Keep core editor deterministic; avoid UI-only state leaking into document model.
- Avoid command hardcoding when command-layer hooks can express behavior cleanly.

## Milestones

### 1. Completion Auto-Trigger Lifecycle + Request Context

Goal: completion opens while typing, with correct trigger metadata and stale-response safety.

- Add completion trigger model:
  - `invoked` (manual)
  - `triggerCharacter`
  - `triggerForIncompleteCompletions`
- Add auto-trigger scheduling with debounce in term client context.
- Detect server trigger chars from LSP `completionProvider.triggerCharacters`.
- Dispatch completion with proper LSP `context` fields.
- Add stale guards on response handling:
  - request generation id
  - cursor position at request time
- Silence noisy "no completion candidates" status spam for auto-triggered requests.

### 2. Insert-Mode Completion Key Routing

Goal: when completion is active, insert-mode keys behave like a completion menu first.

- Route insert-mode `up/down`, `tab/shift-tab`, `ret`, and `esc` to completion actions.
- Keep existing keymap behavior when completion menu is inactive.
- Preserve completion session across typing/deletion actions where appropriate.

### 3. Popup Geometry + Visual Quality

Goal: stop center-floating behavior and anchor popup to cursor with better readability.

- Replace generic centered custom-layout placement with cursor-anchored geometry.
- Add collision/flip logic (below/above cursor, viewport clamping).
- Improve list row layout (kind/detail columns, truncation, spacing, selected style).
- Improve docs pane width/height policy and wrapping.

### 4. Apply Semantics + Highlight Stability

Goal: completion apply should not destabilize syntax/highlight state.

- Track replacement range from completion request context (not only cursor insert).
- Ensure apply path funnels through transaction+syntax update path identical to normal edit.
- Add regression tests for:
  - accept completion preserves highlighting consistency
  - undo/redo after completion keeps highlighting stable
  - cursor-scoped popup positioning correctness

## Testing Plan

- Unit tests for completion request context serialization.
- Term integration tests for:
  - auto-trigger from word typing
  - trigger-character auto-trigger
  - stale response ignored after cursor move/new request
  - insert-mode key routing (`ret`, arrows, cancel)
- Manual validation in Rust buffers with `rust-analyzer`.

## Execution Order

1. Milestone 1
2. Milestone 2
3. Milestone 3
4. Milestone 4

Milestones 1 and 2 are the immediate implementation target.
