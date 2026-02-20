# the-lib finish checklist (short-term)

Goal: validate the data model with a **basic render path** (regular buffer only), then
finish the remaining core correctness tasks.

## 1) Selection / view model (decision + plan)

### Decision
Drop the idea of a “single distinguished cursor drives viewport.” Keep **all cursors equal** in core.
The *view* (client) decides which cursor to follow, if any. This matches your preference
and keeps the-lib purely stateful.

### Implementation plan
- [ ] **Move “active cursor” out of `Selection`**:
  - `Selection` becomes just `SmallVec<Range>`.
  - Remove `primary_index` and any logic that depends on it.
- [ ] **Add optional active index in view state** (client or a small `ViewState` in the-lib):
  - `active_cursor: Option<usize>` (or a stable cursor id).
  - Used by the client for “collapse” and viewport follow.
- [ ] **Update selection APIs**:
  - Methods that used `primary_index` should take `idx: Option<usize>` or a strategy
    (e.g. `Pick::Last`, `Pick::Nearest(Position)`, `Pick::First`).
  - `Selection::collapse(idx)` should return a new `Selection` with only the chosen range.
- [ ] **Update call sites**:
  - Replace `selection.primary()` usage in render/plan and commands with explicit index or strategy.

Notes:
- If we want stable identity across edits, add `CursorId` and keep a side map
  `Vec<CursorId>`, but this can wait until multi-view actually needs it.

## 2) Syntax highlight adapter for render::plan

Minimal adapter to get syntax color spans in the basic plan (no gutters yet).

- [ ] Add `render::SyntaxHighlightAdapter` implementing `HighlightProvider`.
- [ ] Back it with `syntax::HighlightCache` and a `Highlighter` or precomputed ranges.
- [ ] Provide a fast path: cache highlight ranges per visible line and reuse when
      `Document.version()` and syntax generations match.
- [ ] Update `render::plan` tests to include at least one highlighted span.

## 3) Render plan (minimal, regular buffer only)

Keep this intentionally small for now; no gutters or diagnostics yet.

- [ ] Ensure wrap indicators are emitted when soft-wrap is enabled (already in formatter;
      verify in plan output).
- [ ] Allow caller to pass `Style` for selections/cursors (no defaults in plan).
- [ ] Keep `RenderCache` persisted by the client for smooth scrolling.

## 4) Correctness TODOs (must clear before client)

From `rg -n "TODO" the-lib`:
- [x] `comment.rs`: uncommenting with uneven indent.
- [x] `selection.rs`: TODOs in range creation / search helpers.
- [x] `diff.rs`: heuristics + tokenizer perf TODOs.
- [x] `transaction.rs`: delete tracking + missing test coverage.
- [x] `search.rs`: pattern API TODO.
- [ ] `syntax.rs`: predicate handling + query conflict TODOs - ignored for now
- [ ] `text_format.rs`: TODO on type or defaults - same thing

## Suggested order (practical)

1) Selection model change (removes primary; updates render + commands)
2) Syntax highlight adapter wired into render::plan
3) Clear correctness TODOs that touch transactions/selection/search
4) Minimal client proof-of-life (single buffer render loop)
