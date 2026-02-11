# VCS Gutter Integration

This document describes the current VCS-diff pipeline used by `the-term` and `the-ffi` (`the-swift` through FFI).

## Current model

- Provider: `the-vcs::DiffProviderRegistry` (git CLI backend under `git` feature).
- Diff state: `the-vcs::DiffHandle` (host-driven update + `poll()` during `load()`).
- UI bridge: line signs are mapped into `BTreeMap<usize, RenderGutterDiffKind>`.

## Lifecycle

1. **File path set/opened**
   - Client resolves a diff base (`HEAD:<path>`).
   - A new `DiffHandle` is created from:
     - base text (from VCS)
     - current document text
   - Initial gutter signs are materialized.

2. **Document edit transaction**
   - Client calls `DiffHandle::update_document(doc_text, true)`.
   - Client snapshots signs from `handle.load().line_signs()`.
   - Signs are written into render state and consumed by `apply_diff_gutter_markers`.

3. **No VCS base available**
   - Handle is cleared.
   - Diff gutter signs are cleared.

## Why host-driven

- Keeps scheduling decisions at the client boundary (`the-term` / `the-ffi`).
- Avoids hidden background redraw coupling inside `the-vcs`.
- Fits the dispatch-first architecture in `docs/REWRITE.md`.

## Known constraints

- Diff base decoding currently uses lossy UTF-8 conversion from git bytes.
- No periodic background refresh of VCS base (it refreshes on path set/open + edits).
- Git backend currently shells out to `git` (not `gix`) for portability in this workspace.
