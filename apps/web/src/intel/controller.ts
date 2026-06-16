/**
 * The intel controller: completion sessions and hover, Monaco's model.
 *
 * Completion: a session anchors at the word start. Providers are queried
 * ONCE per session (plus on trigger characters and for providers that
 * reported an incomplete list); every other keystroke refilters CLIENT-side
 * with fuzzy scoring against the growing/shrinking prefix. While a session
 * is open the controller intercepts navigation keys ahead of the keymap
 * pipeline; accepting replaces the item's range through the ordinary
 * commit path.
 *
 * Hover: pointer dwell (300ms stationary) queries hover providers and
 * anchors a card above the position; any movement to a different word,
 * edit, or keystroke dismisses it.
 *
 * The controller renders nothing — it owns state and floats; IntelView
 * portals the React UI into the float elements.
 */

import { Context, Effect, Exit, Scope } from "effect";
import {
  commitDocumentTransaction,
  commitTransaction,
  DocumentInvalidated,
  EditorVisuals,
  payloadsOf,
  type EditorNotification,
  type VisualsHandle,
} from "@the-editor/editor";
import { RangeSet, Transaction, Selection, SelectionRange } from "@the-editor/engine";
import type { KeyboardEventLike } from "@the-editor/keymaps";
import type { AppRuntime, EditorPane } from "../runtime";
import type { FloatHandle } from "../editor/floats";
import { fuzzyScore } from "./fuzzy";
import { looksLikeSnippet, parseSnippet, type SnippetStop } from "./snippet";
import type {
  CompletionEdit,
  CompletionItem,
  CompletionProvider,
  CompletionProviders,
  CompletionResolution,
  CompletionTrigger,
  HoverProviders,
  HoverResult,
  SignatureProviders,
  SignatureResult,
  SignatureTrigger,
} from "./types";

const HOVER_DELAY_MS = 300;
/** Grace before a shown hover hides — long enough to mouse INTO the card.
 * 200ms matches Pierre's hover-intent dismiss: snappy but not twitchy. */
const HOVER_HIDE_GRACE_MS = 200;
const PROVIDER_TIMEOUT_MS = 1500;
/**
 * Accept-time resolve budget. Generous: it runs in the background after the
 * insert, and big-project servers (tsserver) take well over the provider
 * query budget on their first auto-import resolution.
 */
const RESOLVE_TIMEOUT_MS = 8000;
const MAX_VISIBLE_SCORED = 200;
const WORD_CHAR = /[\p{L}\p{N}_$]/u;

export type ScoredItem = {
  readonly item: CompletionItem;
  readonly score: number;
  readonly providerOrder: number;
};

export type IntelState = {
  readonly completion: {
    readonly open: boolean;
    readonly items: readonly ScoredItem[];
    readonly selected: number;
    readonly anchor: number;
  };
  /** Docs for the SELECTED completion item (eager + resolve-updated). The
   * language is the document's file extension, for highlighting the detail. */
  readonly completionDocs: {
    readonly detail?: string;
    readonly documentation?: string;
    readonly language?: string;
  } | null;
  readonly hover: { readonly open: boolean; readonly parts: readonly HoverResult[] };
  readonly signature: {
    readonly open: boolean;
    readonly result: SignatureResult | null;
    /** Selected signature index (ArrowUp/Down when several overloads). */
    readonly selected: number;
  };
};

type CompletionSession = {
  pane: EditorPane;
  docId: string;
  anchor: number; // word start; the float anchors here
  generation: number;
  /** Providers answered; before this, empty results mean "still loading". */
  loaded: boolean;
  /** Raw provider results, in provider registration order. */
  sources: Array<{ provider: CompletionProvider; order: number; items: readonly CompletionItem[]; isIncomplete: boolean }>;
  scored: readonly ScoredItem[];
  selected: number;
};

type HoverSession = {
  parts: readonly HoverResult[];
  anchor: number;
  range: { from: number; to: number } | null;
};

/**
 * A live tab-stop session after accepting a snippet completion: Tab and
 * Shift+Tab move between stops (selecting their placeholder text, all
 * occurrences as a multi-cursor). Stop ranges are mapped through every
 * subsequent edit via the notification change ranges, CHAIN-GATED by
 * version — the bus is sliding, and a dropped batch means the offsets can
 * no longer be trusted, so the session ends.
 */
type SnippetSessionState = {
  pane: EditorPane;
  stops: Array<{ index: number; ranges: Array<{ from: number; to: number }> }>;
  current: number;
  version: number;
};

/**
 * A live signature-help widget: opened by a trigger character ("(", ",")
 * or explicitly; every subsequent edit or caret move RE-QUERIES (the
 * server tracks which call/parameter the caret is in and answers null once
 * it leaves — that closes the widget). ArrowUp/Down cycle overloads.
 */
type SignatureSessionState = {
  pane: EditorPane;
  anchor: number;
  result: SignatureResult;
  selected: number;
};

type DocumentChangeRange = {
  readonly fromA: number;
  readonly toA: number;
  readonly fromB: number;
  readonly toB: number;
};

const mapOffsetThrough = (
  offset: number,
  changes: ReadonlyArray<DocumentChangeRange>,
  bias: "left" | "right",
): number => {
  let delta = 0;
  for (const change of changes) {
    if (change.toA < offset) {
      delta += change.toB - change.fromB - (change.toA - change.fromA);
      continue;
    }
    if (change.fromA > offset) break;
    // Overlap (including an insertion exactly at the offset): the stop's
    // start hugs the new text's start, its end hugs the new text's end —
    // typing over a selected placeholder keeps the stop covering the typed
    // text, and appending at a stop's edge grows it.
    return bias === "left" ? change.fromB : change.toB;
  }
  return offset + delta;
};

const wordStart = (text: { sliceString(from: number, to: number): string }, offset: number): number => {
  let start = offset;
  while (start > 0 && WORD_CHAR.test(text.sliceString(start - 1, start))) start--;
  return start;
};

const withTimeout = async <T>(
  value: Promise<T> | T | null,
  fallback: T,
  timeoutMs = PROVIDER_TIMEOUT_MS,
): Promise<T> => {
  if (value === null) return fallback;
  if (!(value instanceof Promise)) return value;
  return Promise.race([
    value.catch(() => fallback),
    new Promise<T>((resolve) => setTimeout(() => resolve(fallback), timeoutMs)),
  ]);
};

export class IntelController {
  private pane: EditorPane | null = null;
  private unsubscribePane: (() => void) | null = null;
  private completionFloat: FloatHandle | null = null;
  private hoverFloat: FloatHandle | null = null;
  /** Highlights the text range a hover card describes. */
  private hoverRangeScope: Scope.CloseableScope | null = null;
  private hoverRangeHandle: VisualsHandle | null = null;
  private session: CompletionSession | null = null;
  private snippet: SnippetSessionState | null = null;
  private signatureSession: SignatureSessionState | null = null;
  private signatureGeneration = 0;
  /** Coalesces doc-change + selection-change into one signature re-query. */
  private signatureQueryTimer: number | null = null;
  private signaturePendingTrigger: SignatureTrigger | null = null;
  private signatureFloat: FloatHandle | null = null;
  private hover: HoverSession | null = null;
  private hoverTimer: number | null = null;
  /** Pending graceful hide (pointer left the range, may be heading to the card). */
  private hoverHideTimer: number | null = null;
  private hoverOffset: number | null = null;
  private hoverGeneration = 0;
  /** True while the pointer is inside the hover card (stickiness). */
  private hoverCardHovered = false;
  /**
   * Memoized item resolutions (LSP completionItem/resolve). Kicked off when
   * an item becomes SELECTED in the popup so accept has the result in hand —
   * resolving only after accept is both slow on big projects and fragile
   * (tsserver evicts its completion cache on the next request).
   */
  private readonly resolutions = new WeakMap<
    CompletionItem,
    Promise<CompletionResolution | null>
  >();
  /** Settled resolutions, for the docs panel (documentation arrives lazily). */
  private readonly resolvedDocs = new WeakMap<CompletionItem, CompletionResolution>();

  private readonly listeners = new Set<() => void>();
  private state: IntelState = {
    completion: { open: false, items: [], selected: 0, anchor: 0 },
    completionDocs: null,
    hover: { open: false, parts: [] },
    signature: { open: false, result: null, selected: 0 },
  };

  constructor(
    private readonly runtime: AppRuntime,
    private readonly completionProviders: CompletionProviders.Service,
    private readonly hoverProviders: HoverProviders.Service,
    private readonly signatureProviders: SignatureProviders.Service,
  ) {}

  // -------------------------------------------------------------- host API

  getState(): IntelState {
    return this.state;
  }

  /** Dev diagnostics. */
  debugInfo(): Record<string, unknown> {
    return {
      paneAttached: this.pane !== null,
      paneDoc: this.pane !== null ? String(this.pane.document.id) : null,
      providers: Effect.runSync(this.completionProviders.list).map((p) => p.id),
      hoverProviders: Effect.runSync(this.hoverProviders.list).map((p) => p.id),
      ptrCalls: this.ptrCalls,
      session: this.session !== null ? { loaded: this.session.loaded, sources: this.session.sources.length, scored: this.session.scored.length } : null,
      snippet:
        this.snippet !== null
          ? { current: this.snippet.current, stops: this.snippet.stops }
          : null,
      signature:
        this.signatureSession !== null
          ? {
              selected: this.signatureSession.selected,
              signatures: this.signatureSession.result.signatures.map((s) => s.label),
            }
          : null,
      signatureTriggers: Effect.runSync(this.signatureProviders.list).map((provider) => ({
        id: provider.id,
        triggers:
          this.pane !== null
            ? provider.triggerCharacters?.(String(this.pane.document.id)) ?? []
            : [],
      })),
    };
  }

  subscribe(listener: () => void): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  completionElement(): HTMLElement | null {
    return this.completionFloat?.element ?? null;
  }

  hoverElement(): HTMLElement | null {
    return this.hoverFloat?.element ?? null;
  }

  signatureElement(): HTMLElement | null {
    return this.signatureFloat?.element ?? null;
  }

  /** Follow workbench focus: one active pane at a time. */
  setPane(pane: EditorPane | null): void {
    if (pane === this.pane) return;
    this.dismissCompletion();
    this.dismissHover();
    this.endSnippetSession();
    this.dismissSignature();
    if (this.pane !== null) {
      this.pane.keyInterceptor = undefined;
      this.pane.pointerHover = undefined;
      this.pane.pointerScrolled = undefined;
      this.completionFloat?.dispose();
      this.hoverFloat?.dispose();
      this.signatureFloat?.dispose();
      this.completionFloat = null;
      this.hoverFloat = null;
      this.signatureFloat = null;
      if (this.hoverRangeScope !== null) {
        Effect.runFork(Scope.close(this.hoverRangeScope, Exit.void));
        this.hoverRangeScope = null;
        this.hoverRangeHandle = null;
      }
    }
    this.unsubscribePane?.();
    this.unsubscribePane = null;
    this.pane = pane;
    if (pane === null) return;

    this.completionFloat = pane.floats.create();
    this.hoverFloat = pane.floats.create();
    this.signatureFloat = pane.floats.create();
    const visuals = Context.get(
      pane.document.context as Context.Context<EditorVisuals>,
      EditorVisuals,
    );
    this.hoverRangeScope = Effect.runSync(Scope.make());
    this.hoverRangeHandle = Effect.runSync(
      visuals.contribute("intel-hover-range", {}).pipe(Scope.extend(this.hoverRangeScope)),
    );
    pane.keyInterceptor = (event) => this.interceptKey(event);
    pane.pointerHover = (offset) => this.onPointerHover(offset);
    pane.pointerScrolled = () => this.onScrolled();
    this.unsubscribePane = pane.subscribe((notification, source) => {
      if (source === "document") this.onDocumentChanged(notification);
      if (source === "view") this.onSelectionChanged();
    });
  }

  /** ctrl+space and the palette command land here. */
  triggerCompletion(): void {
    if (this.pane === null) return;
    void this.startSession({ kind: "invoked" });
  }

  /** ctrl+shift+space and the palette command land here. */
  triggerSignatureHelp(): void {
    if (this.pane === null) return;
    void this.querySignature({ kind: "invoked" });
  }

  // -------------------------------------------------------- signature help

  private signatureTriggerKind(character: string): "trigger" | "retrigger" | null {
    if (character.length === 0 || this.pane === null) return null;
    const docId = String(this.pane.document.id);
    const providers = Effect.runSync(this.signatureProviders.list);
    if (providers.some((p) => p.triggerCharacters?.(docId).includes(character))) return "trigger";
    if (providers.some((p) => p.retriggerCharacters?.(docId).includes(character)))
      return "retrigger";
    return null;
  }

  /**
   * Doc edits and caret moves both land here; one query per frame-ish.
   * Trigger characters outrank plain content changes when they coalesce.
   */
  private scheduleSignatureQuery(trigger: SignatureTrigger): void {
    if (trigger.kind === "triggerCharacter" || this.signaturePendingTrigger === null) {
      this.signaturePendingTrigger = trigger;
    }
    if (this.signatureQueryTimer !== null) return;
    this.signatureQueryTimer = window.setTimeout(() => {
      this.signatureQueryTimer = null;
      const pending = this.signaturePendingTrigger;
      this.signaturePendingTrigger = null;
      if (pending !== null) void this.querySignature(pending);
    }, 30);
  }

  private async querySignature(trigger: SignatureTrigger): Promise<void> {
    const pane = this.pane;
    if (pane === null) return;
    const generation = ++this.signatureGeneration;
    const snapshot = pane.snapshot();
    const caret = snapshot.selection.primary.head;
    const isRetrigger = this.signatureSession !== null;
    const context = {
      docId: String(pane.document.id),
      path: pane.fileInfo()?.path ?? null,
      text: snapshot.text,
      offset: caret,
      trigger,
      isRetrigger,
      ...(this.signatureSession !== null ? { active: this.signatureSession.result } : {}),
    };
    const providers = Effect.runSync(this.signatureProviders.list);
    // First provider with an answer wins (overloads aren't merged across
    // providers the way completion items are).
    let result: SignatureResult | null = null;
    for (const provider of providers) {
      result = await withTimeout(provider.provide(context), null);
      if (result !== null && result.signatures.length > 0) break;
      result = null;
    }
    if (generation !== this.signatureGeneration || this.pane !== pane) return;
    if (result === null) {
      this.dismissSignature();
      return;
    }
    const anchor = this.signatureSession?.anchor ?? caret;
    const selected = Math.min(result.activeSignature ?? 0, result.signatures.length - 1);
    this.signatureSession = { pane, anchor, result, selected };
    this.signatureFloat?.setAnchor({ offset: anchor, placement: "above" });
    this.publish();
  }

  private moveSignatureSelection(delta: number): void {
    const session = this.signatureSession;
    if (session === null) return;
    const count = session.result.signatures.length;
    session.selected = (session.selected + delta + count) % count;
    this.publish();
  }

  private dismissSignature(): void {
    this.signatureGeneration++;
    this.signaturePendingTrigger = null;
    if (this.signatureQueryTimer !== null) {
      window.clearTimeout(this.signatureQueryTimer);
      this.signatureQueryTimer = null;
    }
    if (this.signatureSession === null) return;
    this.signatureSession = null;
    this.signatureFloat?.hide();
    this.publish();
  }

  acceptSelected(): void {
    this.acceptIndex(this.session?.selected ?? 0);
  }

  acceptIndex(index: number): void {
    const session = this.session;
    if (session === null || session.scored.length === 0) return;
    const scored = session.scored[Math.min(index, session.scored.length - 1)]!;
    const pane = session.pane;
    const anchor = session.anchor;
    this.dismissCompletion();
    this.endSnippetSession();
    const item = scored.item;
    const snapshot = pane.snapshot();
    const caret = snapshot.selection.primary.head;
    const range = item.range ?? { from: anchor, to: caret };
    const from = Math.min(range.from, caret);
    const to = Math.max(range.to, caret);

    // Snippet items expand placeholders into a tab-stop session.
    let insert = item.insertText;
    let stops: ReadonlyArray<SnippetStop> = [];
    if (item.isSnippet === true && looksLikeSnippet(insert)) {
      const parsed = parseSnippet(insert);
      insert = parsed.text;
      stops = parsed.stops;
    }

    // Additional edits the provider already knows (eager auto-imports) ride
    // the same transaction; ones overlapping the insert range are dropped.
    const additional = (item.additionalEdits ?? [])
      .filter((edit) => edit.to <= from || edit.from >= to)
      .filter((edit) => edit.from >= 0 && edit.to <= snapshot.text.length);
    const replacements = [...additional, { from, to, text: insert }].sort(
      (a, b) => a.from - b.from || a.to - b.to,
    );
    // The inserted text's final position: edits before it shift it.
    const insertStart =
      from +
      additional
        .filter((edit) => edit.to <= from)
        .reduce((n, edit) => n + edit.text.length - (edit.to - edit.from), 0);

    let selection = Selection.point(insertStart + insert.length);
    let sessionStops: SnippetSessionState["stops"] | null = null;
    if (stops.length > 0) {
      const absolute = stops.map((stop) => ({
        index: stop.index,
        ranges: stop.ranges.map((r) => ({
          from: insertStart + r.from,
          to: insertStart + r.to,
        })),
      }));
      const first = absolute[0]!;
      selection = Selection.create(
        first.ranges.map((r) => SelectionRange.create(r.from, r.to)),
        { primaryIndex: 0 },
      );
      // A lone final-caret stop is just a caret position, not a session.
      if (absolute.length > 1 || absolute[0]!.index !== 0) sessionStops = absolute;
    }

    const transaction = Effect.runSync(
      Transaction.fromReplacements(snapshot.text.length, replacements),
    );
    Effect.runSync(
      commitTransaction(transaction, selection).pipe(
        Effect.provide(pane.view.context as Context.Context<any>),
        Effect.orDie,
      ),
    );
    if (sessionStops !== null) {
      this.snippet = {
        pane,
        stops: sessionStops,
        current: 0,
        version: pane.snapshot().version,
      };
    }

    // Lazy details (LSP completionItem/resolve): auto-import edits arrive
    // after the insert and apply as a document-level transaction — the
    // caret and any snippet stops map through its change ranges. Skipped
    // when the item carried eager edits (resolve repeats them). The item's
    // post-accept command (if any) runs last, per the LSP spec.
    void this.finalizeAccept(pane, item, additional.length > 0);
  }

  /** Memoized resolve; proactively started when the item gets selected. */
  private resolveItem(item: CompletionItem): Promise<CompletionResolution | null> {
    if (item.resolve === undefined) return Promise.resolve(null);
    let promise = this.resolutions.get(item);
    if (promise === undefined) {
      promise = item.resolve().catch(() => null);
      this.resolutions.set(item, promise);
    }
    return promise;
  }

  private preresolveSelected(): void {
    const session = this.session;
    if (session === null || session.scored.length === 0) return;
    const item = session.scored[session.selected]?.item;
    if (item === undefined) return;
    void this.resolveItem(item).then((resolution) => {
      if (resolution === null) return;
      this.resolvedDocs.set(item, resolution);
      // Repaint the docs panel if this item is still the selected one.
      const current = this.session;
      if (current !== null && current.scored[current.selected]?.item === item) this.publish();
    });
  }

  private async finalizeAccept(
    pane: EditorPane,
    item: CompletionItem,
    hadEagerEdits: boolean,
  ): Promise<void> {
    if (!hadEagerEdits && item.resolve !== undefined) {
      const resolution = await withTimeout(this.resolveItem(item), null, RESOLVE_TIMEOUT_MS);
      const edits = resolution?.additionalEdits;
      if (edits !== undefined && edits.length > 0 && this.pane === pane) {
        const snapshot = pane.snapshot();
        const sorted: CompletionEdit[] = [...edits]
          .filter(
            (edit) => edit.from >= 0 && edit.to <= snapshot.text.length && edit.from <= edit.to,
          )
          .sort((a, b) => a.from - b.from || a.to - b.to);
        if (sorted.length > 0) {
          const transaction = Effect.runSync(
            Transaction.fromReplacements(snapshot.text.length, sorted),
          );
          Effect.runSync(
            commitDocumentTransaction(transaction).pipe(
              Effect.provide(pane.document.context as Context.Context<any>),
              Effect.orDie,
            ),
          );
        }
      }
    }
    if (item.postAccept !== undefined) {
      await withTimeout(
        item.postAccept().then(() => true),
        false,
        RESOLVE_TIMEOUT_MS,
      );
    }
  }

  dispose(): void {
    this.setPane(null);
  }

  // ------------------------------------------------------------ completion

  private interceptKey(event: KeyboardEventLike): boolean {
    // Any keypress dismisses hover.
    this.dismissHover();
    if (this.session !== null) {
      switch (event.key) {
        case "ArrowDown":
          this.moveSelection(1);
          return true;
        case "ArrowUp":
          this.moveSelection(-1);
          return true;
        case "Tab":
        case "Enter":
          this.acceptSelected();
          return true;
        case "Escape":
          this.dismissCompletion();
          return true;
        default:
          return false;
      }
    }
    if (this.signatureSession !== null) {
      if (event.key === "Escape") {
        this.dismissSignature();
        return true;
      }
      // Up/Down cycle overloads — only intercepted when there ARE several.
      if (this.signatureSession.result.signatures.length > 1) {
        if (event.key === "ArrowUp") {
          this.moveSignatureSelection(-1);
          return true;
        }
        if (event.key === "ArrowDown") {
          this.moveSignatureSelection(1);
          return true;
        }
      }
    }
    if (this.snippet !== null) {
      if (event.key === "Tab") {
        this.advanceSnippet(event.shiftKey ? -1 : 1);
        return true;
      }
      if (event.key === "Escape") {
        this.endSnippetSession();
        return true;
      }
    }
    return false;
  }

  // --------------------------------------------------------- snippet stops

  private advanceSnippet(delta: number): void {
    const snippet = this.snippet;
    if (snippet === null) return;
    const next = snippet.current + delta;
    if (next < 0) return;
    if (next >= snippet.stops.length) {
      // Past the last stop: the session is over; the caret stays put.
      this.endSnippetSession();
      return;
    }
    snippet.current = next;
    const stop = snippet.stops[next]!;
    snippet.pane.setSelection(
      Selection.create(
        stop.ranges.map((r) => SelectionRange.create(r.from, r.to)),
        { primaryIndex: 0 },
      ),
    );
    if (stop.index === 0) this.endSnippetSession();
  }

  private endSnippetSession(): void {
    this.snippet = null;
  }

  /** Map stop ranges through a committed change set; end on any gap. */
  private mapSnippetThrough(payload: {
    readonly version: number;
    readonly changes?: ReadonlyArray<DocumentChangeRange>;
  }): void {
    const snippet = this.snippet;
    if (snippet === null) return;
    if (payload.version <= snippet.version) return; // our own insert's batch
    if (payload.version !== snippet.version + 1 || payload.changes === undefined) {
      this.endSnippetSession(); // dropped batch: offsets can't be trusted
      return;
    }
    for (const stop of snippet.stops) {
      stop.ranges = stop.ranges.map((range) => {
        const from = mapOffsetThrough(range.from, payload.changes!, "left");
        const to = mapOffsetThrough(range.to, payload.changes!, "right");
        return { from, to: Math.max(from, to) };
      });
    }
    snippet.version = payload.version;
  }

  private moveSelection(delta: number): void {
    const session = this.session;
    if (session === null || session.scored.length === 0) return;
    session.selected =
      (session.selected + delta + session.scored.length) % session.scored.length;
    this.publish();
    this.preresolveSelected();
  }

  private async startSession(trigger: CompletionTrigger): Promise<void> {
    const pane = this.pane;
    if (pane === null) return;
    const snapshot = pane.snapshot();
    const caret = snapshot.selection.primary.head;
    const anchor =
      trigger.kind === "triggerCharacter" ? caret : wordStart(snapshot.text, caret);
    const docId = String(pane.document.id);
    const session: CompletionSession = {
      pane,
      docId,
      anchor,
      generation: (this.session?.generation ?? 0) + 1,
      loaded: false,
      sources: [],
      scored: [],
      selected: 0,
    };
    this.session = session;

    const providers = Effect.runSync(this.completionProviders.list);
    const prefix = snapshot.text.sliceString(anchor, caret);
    const context = {
      docId,
      path: pane.fileInfo()?.path ?? null,
      text: snapshot.text,
      offset: caret,
      prefix,
      trigger,
    };
    const results = await Promise.all(
      providers.map(async (provider, order) => ({
        provider,
        order,
        result: await withTimeout(provider.provide(context), null),
      })),
    );
    if (this.session !== session) return; // superseded or dismissed
    session.loaded = true;
    const sources = results
      .filter((entry) => entry.result !== null && entry.result.items.length > 0)
      .map((entry) => ({
        provider: entry.provider,
        order: entry.order,
        items: entry.result!.items,
        isIncomplete: entry.result!.isIncomplete === true,
      }));
    // Fallback providers (buffer words) only count when nothing richer
    // answered — otherwise a bare word shadows the real LSP item.
    const hasPrimary = sources.some((source) => source.provider.fallback !== true);
    session.sources = hasPrimary
      ? sources.filter((source) => source.provider.fallback !== true)
      : sources;
    if (session.sources.length === 0) {
      this.dismissCompletion();
      return;
    }
    this.refilter();
  }

  /** Client-side fuzzy refilter against the current prefix. */
  private refilter(): void {
    const session = this.session;
    if (session === null || !session.loaded) return; // results still in flight
    const snapshot = session.pane.snapshot();
    const caret = snapshot.selection.primary.head;
    if (caret < session.anchor) {
      this.dismissCompletion();
      return;
    }
    const prefix = snapshot.text.sliceString(session.anchor, caret);
    const scored: ScoredItem[] = [];
    for (const source of session.sources) {
      for (const item of source.items) {
        const score = fuzzyScore(prefix, item.filterText ?? item.label);
        if (score === null) continue;
        scored.push({ item, score, providerOrder: source.order });
      }
    }
    scored.sort(
      (a, b) =>
        b.score - a.score ||
        a.providerOrder - b.providerOrder ||
        (a.item.sortText ?? a.item.label).localeCompare(b.item.sortText ?? b.item.label),
    );
    // Keep the user's arrow selection on the SAME item when it survives
    // the refilter; reset to top only when it filtered out.
    const previousItem = session.scored[session.selected]?.item;
    session.scored = scored.slice(0, MAX_VISIBLE_SCORED);
    const kept =
      previousItem !== undefined
        ? session.scored.findIndex((entry) => entry.item === previousItem)
        : -1;
    session.selected = kept >= 0 ? kept : 0;
    if (session.scored.length === 0) {
      this.dismissCompletion();
      return;
    }
    this.completionFloat?.setAnchor({ offset: session.anchor, placement: "below" });
    this.publish();
    this.preresolveSelected();
  }

  private onDocumentChanged(notification: EditorNotification): void {
    const pane = this.pane;
    if (pane === null) return;
    // Only a real text change dismisses hover. The document bus also carries
    // VisualsInvalidated (diagnostics republish, inlay hints, our OWN hover
    // range highlight) — dismissing on those makes the hover kill itself the
    // frame after it shows.
    const documentPayloads = payloadsOf(notification, DocumentInvalidated);
    if (documentPayloads.length > 0) this.dismissHover();
    for (const payload of documentPayloads) this.mapSnippetThrough(payload);
    const snapshot = pane.snapshot();
    const caret = snapshot.selection.primary.head;

    // "Simple forward typing" is read off the CHANGE RANGES, never caret
    // deltas: document and view notifications ride separate fibers, so
    // caret bookkeeping races. A single one-char insertion ending at the
    // caret is a keystroke; everything else (paste, undo, hot reload) is
    // not.
    // (No caret comparison: in a typing burst this event may process after
    // later keystrokes already moved the caret; the session reads fresh
    // state anyway.)
    let typedChar = "";
    for (const payload of documentPayloads) {
      const changes = payload.changes;
      if (changes === undefined || changes.length !== 1) continue;
      const change = changes[0]!;
      if (change.fromA === change.toA && change.toB - change.fromB === 1) {
        typedChar = snapshot.text.sliceString(
          Math.min(change.fromB, snapshot.text.length - 1),
          Math.min(change.toB, snapshot.text.length),
        );
      }
    }

    this.onDocumentChangedSignature(typedChar, documentPayloads.length > 0);

    // Below here: reactions to TEXT changes only. The document bus also
    // carries VisualsInvalidated (diagnostics/hints republish) — refiltering
    // on those would reset the user's arrow selection in the open popup.
    if (documentPayloads.length === 0) return;

    if (this.session !== null) {
      // Inside a session: trigger chars re-query; otherwise refilter, and
      // re-query if any provider's last list was incomplete.
      if (typedChar !== "" && this.isTriggerCharacter(typedChar)) {
        void this.startSession({ kind: "triggerCharacter", character: typedChar });
        return;
      }
      const hasIncomplete = this.session.sources.some((source) => source.isIncomplete);
      if (hasIncomplete && this.session.loaded) {
        void this.startSession({ kind: "incomplete" });
        return;
      }
      this.refilter();
      return;
    }

    if (typedChar === "") return;
    if (this.isTriggerCharacter(typedChar)) {
      void this.startSession({ kind: "triggerCharacter", character: typedChar });
    } else if (WORD_CHAR.test(typedChar)) {
      void this.startSession({ kind: "typing" });
    }
  }

  /** Signature reactions to a document change (runs for ANY session state). */
  private onDocumentChangedSignature(typedChar: string, hadTextChange: boolean): void {
    if (typedChar !== "") {
      const kind = this.signatureTriggerKind(typedChar);
      if (kind === "trigger" || (kind === "retrigger" && this.signatureSession !== null)) {
        this.scheduleSignatureQuery({ kind: "triggerCharacter", character: typedChar });
        return;
      }
    }
    if (hadTextChange && this.signatureSession !== null) {
      this.scheduleSignatureQuery({ kind: "contentChange" });
    }
  }

  private onSelectionChanged(): void {
    const pane = this.pane;
    if (pane === null) return;
    const caret = pane.snapshot().selection.primary.head;
    if (this.session !== null && (caret < this.session.anchor || caret > this.session.anchor + 200)) {
      this.dismissCompletion();
    }
    if (this.snippet !== null) {
      // Clicking/moving outside the snippet's whole region abandons it.
      const offsets = this.snippet.stops.flatMap((stop) =>
        stop.ranges.flatMap((r) => [r.from, r.to]),
      );
      const lo = Math.min(...offsets);
      const hi = Math.max(...offsets);
      if (caret < lo || caret > hi) this.endSnippetSession();
    }
    // Caret moves re-query signature help: the server answers null once
    // the caret leaves the call, which closes the widget.
    if (this.signatureSession !== null) this.scheduleSignatureQuery({ kind: "contentChange" });
    this.dismissHover();
  }

  private isTriggerCharacter(character: string): boolean {
    if (character.length === 0 || this.pane === null) return false;
    const docId = String(this.pane.document.id);
    const providers = Effect.runSync(this.completionProviders.list);
    return providers.some((provider) =>
      provider.triggerCharacters?.(docId).includes(character),
    );
  }

  private dismissCompletion(): void {
    if (this.session === null) return;
    this.session = null;
    this.completionFloat?.hide();
    this.publish();
  }

  /**
   * Scrolling hides hover IMMEDIATELY and disarms any pending dwell —
   * Zed's model: a hover may only re-arm from a real mouse move after the
   * scroll, never from text sliding under a stationary cursor.
   */
  private onScrolled(): void {
    if (this.hoverTimer !== null) {
      window.clearTimeout(this.hoverTimer);
      this.hoverTimer = null;
    }
    this.hoverOffset = null;
    this.dismissHover();
  }

  /** IntelView reports pointer enter/leave on the card itself. */
  setHoverCardHovered(hovered: boolean): void {
    this.hoverCardHovered = hovered;
    if (hovered) {
      this.cancelHoverHide();
      // A dwell timer from the last text position would query a stale
      // offset (and an empty answer would blank the card) — drop it.
      if (this.hoverTimer !== null) {
        window.clearTimeout(this.hoverTimer);
        this.hoverTimer = null;
      }
      this.hoverOffset = null;
    } else {
      this.scheduleHoverHide();
    }
  }

  /**
   * A shown hover never hides the instant the pointer leaves its range —
   * the user may be moving INTO the card to select text (Monaco/Zed). The
   * hide runs after a grace period and is cancelled by reaching the card
   * or returning to the range.
   */
  private scheduleHoverHide(): void {
    if (this.hover === null || this.hoverHideTimer !== null) return;
    this.hoverHideTimer = window.setTimeout(() => {
      this.hoverHideTimer = null;
      if (!this.hoverCardHovered) this.dismissHover();
    }, HOVER_HIDE_GRACE_MS);
  }

  private cancelHoverHide(): void {
    if (this.hoverHideTimer !== null) {
      window.clearTimeout(this.hoverHideTimer);
      this.hoverHideTimer = null;
    }
  }

  /** Dev counter: pointer-hover callbacks (e2e pipeline probes). */
  ptrCalls = 0;
  private onPointerHover(offset: number | null): void {
    this.ptrCalls++;
    if (offset === null) {
      if (this.hoverTimer !== null) {
        window.clearTimeout(this.hoverTimer);
        this.hoverTimer = null;
      }
      this.scheduleHoverHide();
      this.hoverOffset = null;
      return;
    }
    // Same position: let any pending dwell timer keep running.
    if (offset === this.hoverOffset) return;
    if (this.hoverTimer !== null) {
      window.clearTimeout(this.hoverTimer);
      this.hoverTimer = null;
    }
    if (this.hover !== null && this.hover.range !== null) {
      if (offset < this.hover.range.from || offset > this.hover.range.to) {
        this.scheduleHoverHide();
      } else {
        this.cancelHoverHide(); // back on the hovered range: keep the card
      }
    }
    this.hoverOffset = offset;
    this.hoverTimer = window.setTimeout(() => {
      this.hoverTimer = null;
      void this.queryHover(offset);
    }, HOVER_DELAY_MS);
  }

  private async queryHover(offset: number): Promise<void> {
    const pane = this.pane;
    if (pane === null || this.session !== null) return;
    const generation = ++this.hoverGeneration;
    const snapshot = pane.snapshot();
    const context = {
      docId: String(pane.document.id),
      path: pane.fileInfo()?.path ?? null,
      text: snapshot.text,
      offset,
    };
    const providers = Effect.runSync(this.hoverProviders.list);
    const parts: HoverResult[] = [];
    for (const provider of providers) {
      const result = await withTimeout(provider.provide(context), null);
      if (result !== null && result.contents.trim().length > 0) parts.push(result);
    }
    if (generation !== this.hoverGeneration || this.pane !== pane) return;
    if (parts.length === 0) {
      // Nothing at the queried offset. If the pointer made it onto the
      // card meanwhile, this is a stale query — the card stays.
      if (!this.hoverCardHovered) this.dismissHover();
      return;
    }
    // Diagnostics card on top, info beneath (stacked separate dialogs).
    const sorted = [...parts].sort(
      (a, b) => (a.kind === "diagnostic" ? 0 : 1) - (b.kind === "diagnostic" ? 0 : 1),
    );
    const range = sorted.find((part) => part.range !== undefined)?.range ?? null;
    this.cancelHoverHide(); // the new card supersedes any pending hide
    this.hover = { parts: sorted, anchor: range?.from ?? offset, range };
    this.hoverFloat?.setAnchor({ offset: this.hover.anchor, placement: "above" });
    if (range !== null && this.hoverRangeHandle !== null) {
      Effect.runSync(
        this.hoverRangeHandle.update({
          decorations: RangeSet.of([
            {
              from: range.from,
              to: range.to,
              value: { layer: "background" as const, className: "intel-hover-range" },
            },
          ]),
        }),
      );
    }
    this.publish();
  }

  private dismissHover(): void {
    this.hoverGeneration++;
    this.cancelHoverHide();
    if (this.hoverTimer !== null) {
      window.clearTimeout(this.hoverTimer);
      this.hoverTimer = null;
    }
    // Forget the position too: otherwise a stationary cursor can never
    // re-trigger after a dismissal (e.g. a diagnostics republish under it).
    this.hoverOffset = null;
    if (this.hover === null) return;
    this.hover = null;
    this.hoverFloat?.hide();
    if (this.hoverRangeHandle !== null) {
      Effect.runSync(this.hoverRangeHandle.update({}));
    }
    this.publish();
  }

  // ------------------------------------------------------------- internals

  private publish(): void {
    const selectedItem =
      this.session !== null ? this.session.scored[this.session.selected]?.item : undefined;
    const resolved = selectedItem !== undefined ? this.resolvedDocs.get(selectedItem) : undefined;
    const documentation = resolved?.documentation ?? selectedItem?.documentation;
    const detail = resolved?.detail ?? selectedItem?.detail;
    const basename = this.pane?.fileInfo()?.path?.split("/").pop() ?? "";
    const dot = basename.lastIndexOf(".");
    const language = dot > 0 ? basename.slice(dot + 1) : undefined;
    this.state = {
      completion: {
        open: this.session !== null && this.session.scored.length > 0,
        items: this.session?.scored ?? [],
        selected: this.session?.selected ?? 0,
        anchor: this.session?.anchor ?? 0,
      },
      completionDocs:
        selectedItem !== undefined && (documentation !== undefined || detail !== undefined)
          ? {
              ...(detail !== undefined ? { detail } : {}),
              ...(documentation !== undefined ? { documentation } : {}),
              ...(language !== undefined ? { language } : {}),
            }
          : null,
      hover: { open: this.hover !== null, parts: this.hover?.parts ?? [] },
      signature: {
        open: this.signatureSession !== null,
        result: this.signatureSession?.result ?? null,
        selected: this.signatureSession?.selected ?? 0,
      },
    };
    for (const listener of this.listeners) listener();
  }
}
