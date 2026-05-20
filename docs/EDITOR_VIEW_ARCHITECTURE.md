# Editor View Architecture

Status: proposed definitive architecture after render/scroll/input experiments.

## Goals

- Native-feeling scrolling and input latency.
- Rust/Core remains authoritative for document state, selection, history, syntax state, and commands.
- Renderer remains authoritative for viewport, scroll, reveal, projection, canvas rendering, and fixed gutters.
- No Rust-owned render-frame/layout path.
- No full workbench snapshot in the normal keypress-to-pixels path.
- Keep a clean architecture instead of special-case hot path duct tape.

## Core Principle

There are two truths:

1. **EditorCore model truth**
   - document text
   - revisions
   - selections/cursors
   - active cursor id
   - history/undo/redo
   - syntax parse/highlight state
   - command/keymap/interaction state

2. **Renderer view truth**
   - scrollTop/scrollLeft
   - viewport dimensions
   - cached presentation window
   - reveal policy
   - canvas projection and paint
   - visual columns / grapheme / tab expansion
   - fixed gutter layout and clipping

Presentation snapshots are a versioned handshake between these two truths.

```txt
Core owns model truth.
Renderer owns view truth.
Presentation is model-space data for a renderer-owned window.
```

## Process Topology

Target topology:

```txt
Renderer main thread
  ├─ native scroll container
  ├─ SurfaceViewportController
  └─ canvas painter

        MessagePort / IPC

EditorCore worker
  ├─ policy/packages/keymaps/commands/interactions
  ├─ workbench model
  ├─ NativeRuntimeSession
  ├─ documents/selections/history
  └─ syntax scheduler

Electron main
  ├─ window lifecycle
  ├─ native dialogs/prompts
  ├─ app menus
  └─ OS/native services only
```

Electron main must not broker every runtime call in the hot path. The runtime session should live with EditorCore, not in a second worker behind main.

## Communication Model

Presentation updates are **subscription/event based**, not request/response per input.

Renderer does not ask:

```txt
apply this key and return presentation
```

Instead:

```txt
renderer subscribes surface viewport window
core remembers subscription
any core mutation publishes presentation update for subscribed windows
renderer accepts fresh update and paints
```

Input is just one mutation source. File reloads, undo, commands, syntax freshening, and external changes use the same presentation update path.

## Surface Viewport Subscription

Renderer owns actual viewport truth. Core stores a mirrored subscription descriptor only so it can push updates.

```ts
interface SurfaceViewportSubscription {
  surfaceId: SurfaceId;
  subscriptionSeq: number;
  startLine: number;
  lineCount: number;
}
```

The subscription unit is the **overscanned cached presentation window**, not the visible viewport.

Renderer computes:

```ts
visibleWindow = from scrollTop/clientHeight
presentationWindow = visibleWindow ± overscan
```

Core stores `presentationWindow`.

Invariant:

```txt
Scroll inside cached presentation window = zero core roundtrip.
Cross cache threshold = renderer updates subscription.
```

## Rows, Not Columns

Subscriptions are row-windowed and full-line.

Do not include horizontal columns in the core subscription.

```ts
{ startLine, lineCount }
```

not:

```ts
{ startLine, lineCount, firstCol, colCount }
```

Renderer handles horizontal scroll locally by clipping and offsetting full line text.

Long-line optimization may be added later as a separate exceptional strategy. It must not shape the normal path.

## Overscan / Cache Policy

Start with simple large adaptive overscan:

```ts
overscanRows = clamp(visibleRows * 4, 120, 500)
```

Renderer refreshes subscription when visible rows leave the middle safe region of the cached window, not only when visible exits the cache.

Recommended policy:

- cached window = visible window ± overscan
- inner safe region = middle ~50% of cached window
- update subscription when visible window crosses safe region boundary

Future enhancement: increase overscan on high scroll velocity.

## Presentation Snapshot Contract

Core sends model-space data only.

```ts
interface SurfacePresentationSnapshot {
  surfaceId: SurfaceId;
  presentationSeq: number;      // monotonic per surface
  documentRevision: number;
  subscriptionSeq: number;

  startLine: number;
  requestedLineCount: number;
  actualLineCount: number;
  totalLineCount: number;

  lines: Array<{
    lineIndex: number;
    startOffset: number;      // document char offset
    endOffset: number;        // before newline
    fullEndOffset: number;    // includes newline if present
    text: string;             // no trailing newline
  }>;

  syntax: {
    revision: number;
    fresh: boolean;
    syntaxLegendVersion: number;
    spans: Array<{
      lineIndex: number;
      startChar: number;      // line-local character index
      endChar: number;        // line-local character index
      kind: number;
    }>;
  };

  selection: Array<{
    cursorId: number;
    anchor: number;           // document char offset
    head: number;             // document char offset
  }>;

  activeCursorId: number | null;
  activeCursorPosition: {
    line: number;
    charColumn: number;
  } | null;

  cursorSemantics: "InsertionPoint" | "Cell";

  gutterRows: Array<{
    lineIndex: number;
    active: boolean;
    selected: boolean;
  }>;
}
```

Not included:

- pixels
- visual columns
- scroll offsets
- row y positions
- cursor rectangles
- theme colors
- canvas-ready syntax spans
- Rust render rows

## Visual Projection

Renderer computes all visual/canvas positions:

- character column → visual cell column
- tabs → configured tab cells
- grapheme segmentation
- wide characters
- selection spans in visual cells
- cursor x/y
- syntax span visual projection

Core sends model-space positions only.

Naming must be explicit: use `startChar`, `endChar`, `charColumn`; avoid ambiguous `startCol` in model contracts.

## Syntax Legend

Syntax legend is out-of-band and versioned.

Core maintains:

```ts
syntaxLegendVersion: number
syntaxLegend: Record<number, string>
```

Presentation includes only `syntaxLegendVersion` and syntax kind ids.

Renderer caches the legend. If it sees a newer or unknown legend version, it requests or receives a `SyntaxLegendUpdated` event. Presentation paint must not block on legend refresh.

Fallback for unknown kind: paint foreground, then repaint when legend arrives.

## Gutters

Split gutters into hot built-in gutters and cold provider gutters.

Hot presentation includes minimal built-in gutter facts:

```ts
lineIndex
active
selected
```

Renderer derives line numbers and paints fixed gutter lanes immediately.

Provider gutters are cold/sideband:

- updated asynchronously
- may use visible rows/presentation as input
- must never block input-to-paint

Invariant:

```txt
No plugin gutter provider may block hot presentation updates.
```

## Input and Hot UI Patches

Input dispatch is one-way:

```txt
renderer keydown → EditorCore dispatchInput
```

EditorCore applies keymaps/commands/operations, then emits relevant patches:

```ts
SurfacePresentationUpdated
InteractionStatePatch
```

Hot interaction patches include UI-critical state:

```ts
interface InteractionStatePatch {
  interactionSeq: number;
  mode?: {
    id: string;
    title: string;
    cursorShape: "bar" | "block" | "underline";
  };
}
```

Full workbench snapshots are cold/debounced and must not gate cursor/text paint.

## Ordering and Race Correctness

Every hot event has monotonic sequencing.

Per surface:

```ts
presentationSeq: number
subscriptionSeq: number
documentRevision: number
```

Renderer accepts a presentation only if:

```ts
event.subscriptionSeq === currentSubscriptionSeq
&& event.presentationSeq > lastAcceptedPresentationSeq
```

`documentRevision` is useful for consistency/debugging but is not sufficient for ordering because selections can change without text revision.

Global interaction state uses:

```ts
interactionSeq: number
```

Renderer accepts only newer interaction patches.

## Workbench State

Workbench state is cold and patch-based where possible.

Cold/debounced concerns:

- tabs/layout
- package load state
- tools
- diagnostics
- notifications
- resource status
- command palette metadata
- file tree state

Normal cursor movement or typing should not rebuild/send:

```txt
roles + packages + tools + diagnostics + resources + layout + syntax legend
```

## Reveal Policy

Renderer owns reveal.

Core may emit active cursor model position. Renderer decides whether to adjust `scrollTop`/`scrollLeft`.

Input/event flow:

```txt
presentation update includes activeCursorPosition
renderer projects charColumn to visual column
renderer reveals if command/input requires reveal
renderer updates subscription if reveal crosses cache threshold
```

Core must not set scroll position.

## Failure / Freshness Policy

Syntax may be stale independently of text/selection presentation.

If `syntax.fresh === false`:

- renderer paints text without syntax or with previous compatible syntax if safe
- core schedules syntax work
- when syntax becomes fresh for subscribed window, core emits another presentation update or syntax-only presentation update

Presentation must always be revision-consistent: lines, selection, active cursor, and syntax metadata must not mix document revisions.

## What Not To Do

Do not reintroduce Rust `RenderFrame`/Rust viewport layout.

Do not make renderer request `renderFrame` through Electron main.

Do not update full workbench snapshot after every input.

Do not make plugin gutters, syntax legend refresh, resource status, package state, or diagnostics block hot presentation.

Do not include horizontal column windows in the normal presentation subscription.

Do not patch input with special `input.keyDownPresentation` request/response APIs. Use subscription events instead.

## Implementation Phases

1. Define contracts for:
   - viewport subscription
   - presentation update event
   - interaction patch
   - syntax legend update

2. Build renderer `SurfaceViewportController` around:
   - native scroll
   - cached row window
   - adaptive overscan
   - fixed gutters
   - model-space projection
   - presentation seq acceptance

3. Replace Rust `RenderFrame` path with presentation snapshots.

4. Add EditorCore viewport subscription registry.

5. Make core mutations publish presentation updates for subscribed surfaces.

6. Move normal input off full workbench snapshot path.

7. Collapse policy/runtime into one EditorCore worker, keeping Electron main for native services only.

8. Convert workbench/session updates to cold/debounced patches/snapshots.

## Final Target Invariant

For normal typing or cursor movement:

```txt
keydown
→ EditorCore applies input
→ EditorCore emits SurfacePresentationUpdated (+ optional InteractionStatePatch)
→ Renderer accepts by seq
→ Canvas paints
```

No full workbench snapshot. No render frame. No Electron-main runtime bridge. No scroll/reveal decision outside renderer.
