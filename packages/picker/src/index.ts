/**
 * The picker: one completing-read seam, N sources (telescope's model).
 *
 * Three layers, only the first two live here:
 *
 * 1. **Source protocol** — a `PickerSource` answers a query with data rows.
 *    The controller owns everything every bespoke picker used to hand-roll:
 *    debounce, staleness (a generation counter drops answers from superseded
 *    queries), ranking, loading/error state, and select-first-on-fresh-results.
 * 2. **The operation** — the `Picker` service tag. `picker.open(request)` from
 *    any package or user extension; the request is pure data plus callbacks.
 * 3. **The renderer** — host-owned (a dialog + windowed list in the default
 *    shell). This package never imports React or touches the DOM, so a source
 *    written by an extension works in any shell that mounts a picker host.
 *
 * Items are data, not elements: label/detail/hint/icon descriptors the host
 * renders with its own chrome. `PickerRequest.renderItem` exists as the same
 * shell-shaped escape hatch `PanelRenderer.icon` uses (`unknown`, interpreted
 * by the host), but the data fields are the contract sources should target —
 * they are what a non-React shell or an agent-facing bridge can consume.
 */

import { Context } from "effect";

// ---------------------------------------------------------------------------
// Items and sources
// ---------------------------------------------------------------------------

export interface PickerItem<T = unknown> {
  /** Stable identity: selection value and the unit of navigation. */
  readonly id: string;
  readonly label: string;
  /** Secondary text after the label (a file's directory). */
  readonly detail?: string;
  /** Right-aligned annotation (a theme's appearance, "active"). */
  readonly hint?: string;
  /** Host-rendered leading icon descriptor. */
  readonly icon?: { readonly kind: "file"; readonly fileName: string };
  /** Trailing status badge (a git status). */
  readonly badge?: string;
  /** Extra haystacks for the built-in ranking (command ids, aliases). */
  readonly searchTerms?: readonly string[];
  readonly data: T;
}

export interface PickerSourceResult<T = unknown> {
  readonly items: ReadonlyArray<PickerItem<T>>;
  /** Status line under the input ("312 matches · 4,120 files"). */
  readonly note?: string;
}

export interface PickerSource<T = unknown> {
  readonly id: string;
  readonly placeholder?: string;
  /**
   * Coalesce keystrokes before querying. Default 0 — right for in-memory
   * sources; backend sources (an indexed file search, a grep) should set the
   * debounce that matches their cost. The empty query always runs immediately
   * so opening a picker never waits on the debounce.
   */
  readonly debounceMs?: number;
  /**
   * The source's answer is already ranked for the query (native fuzzy +
   * frecency); the controller must not re-rank it. Default false: the
   * controller filters + ranks with the tiered matcher over
   * `label`/`searchTerms`.
   */
  readonly preRanked?: boolean;
  readonly query: (query: string) => PickerSourceResult<T> | Promise<PickerSourceResult<T>>;
}

// ---------------------------------------------------------------------------
// Requests
// ---------------------------------------------------------------------------

/** A bounded window of a file, rendered beside the list by hosts that can. */
export interface PickerFilePreview {
  readonly kind: "file";
  readonly path: string;
  readonly size: number;
  readonly focusLine?: number;
}

export interface PickerRequest<T = unknown> {
  readonly source: PickerSource<T>;
  /** Dialog title (accessibility + chrome). */
  readonly title?: string;
  /** Row height in px for the windowed list (host default applies). */
  readonly rowHeight?: number;
  /** Highlight this item initially instead of the first (the active theme). */
  readonly initialSelectedId?: string;
  readonly onChoose: (item: PickerItem<T>) => void;
  /**
   * Fires as the highlight moves (and with `null` when the list empties).
   * This is the live-preview seam: preview on move, commit in `onChoose`,
   * revert in `onDismiss` — the theme selector's contract, generalized.
   */
  readonly onSelectionChange?: (item: PickerItem<T> | null) => void;
  /** Fires when the picker closes without a choice (Escape, outside click). */
  readonly onDismiss?: () => void;
  /** A file preview for the highlighted item, when the host can render one. */
  readonly preview?: (item: PickerItem<T>) => PickerFilePreview | null;
  /**
   * Shell-specific row rendering (the host defines the shape, e.g. a React
   * node in the default shell). Prefer the data fields; this is the escape
   * hatch for rows that need bespoke interactivity.
   */
  readonly renderItem?: (
    item: PickerItem<T>,
    ctx: { readonly selected: boolean; readonly query: string },
  ) => unknown;
}

// ---------------------------------------------------------------------------
// Ranking — the palette's battle-tested tiered matcher, shared by default.
// ---------------------------------------------------------------------------

const normalize = (value: string): string => value.trim().toLowerCase();

/** 0 exact · 1 prefix · 2 word-prefix · 3 substring · 4 no match. */
export const rankItem = (item: PickerItem, query: string): number => {
  if (query.length === 0) return 0;
  const terms = [item.label, ...(item.searchTerms ?? [])].map(normalize);
  if (terms.some((term) => term === query)) return 0;
  if (terms.some((term) => term.startsWith(query))) return 1;
  if (terms.some((term) => term.split(/\s+/).some((word) => word.startsWith(query)))) return 2;
  if (terms.some((term) => term.includes(query))) return 3;
  return 4;
};

/** Filter + stable-rank. Ties keep source order, so a source that clusters
 *  its items (groups) keeps its clusters at the empty query. */
export const rankItems = <T>(
  items: ReadonlyArray<PickerItem<T>>,
  query: string,
): ReadonlyArray<PickerItem<T>> => {
  const q = normalize(query);
  if (q.length === 0) return items;
  const matched = items.filter((item) => rankItem(item, q) < 4);
  return matched.sort((left, right) => rankItem(left, q) - rankItem(right, q));
};

// ---------------------------------------------------------------------------
// Controller — the shared open/query/select lifecycle, framework-agnostic.
// ---------------------------------------------------------------------------

export interface PickerState {
  readonly open: boolean;
  readonly request: PickerRequest | null;
  readonly query: string;
  readonly items: ReadonlyArray<PickerItem>;
  readonly selectedId: string | null;
  readonly note: string | undefined;
  readonly loading: boolean;
  readonly error: string | null;
}

const CLOSED: PickerState = {
  open: false,
  request: null,
  query: "",
  items: [],
  selectedId: null,
  note: undefined,
  loading: false,
  error: null,
};

export interface PickerController {
  readonly state: () => PickerState;
  readonly subscribe: (listener: () => void) => () => void;
  readonly open: <T>(request: PickerRequest<T>) => void;
  /** Close without a choice; fires the request's `onDismiss`. */
  readonly dismiss: () => void;
  readonly setQuery: (query: string) => void;
  readonly setSelected: (id: string) => void;
  /** Choose the highlighted item (or an explicit one, e.g. a row click). */
  readonly choose: (id?: string) => void;
  /** Re-run the current query (the source's data changed under an open
   *  picker — a row-level action mutated it). Keeps the selection when the
   *  selected item survives. No-op when closed. */
  readonly refresh: () => void;
}

export function makePickerController(): PickerController {
  let state: PickerState = CLOSED;
  /** Bumped on every open/query/close; async answers from a stale generation
   *  are dropped. The single mechanism replacing the per-surface `searchSeq`
   *  refs and `cancelled` flags. */
  let generation = 0;
  let debounceTimer: ReturnType<typeof setTimeout> | undefined;
  const listeners = new Set<() => void>();

  const notify = () => {
    for (const listener of listeners) listener();
  };
  const set = (next: Partial<PickerState>) => {
    state = { ...state, ...next };
    notify();
  };

  const clearDebounce = () => {
    if (debounceTimer !== undefined) {
      clearTimeout(debounceTimer);
      debounceTimer = undefined;
    }
  };

  const selectionOf = (
    items: ReadonlyArray<PickerItem>,
    preferred: string | null,
  ): string | null => {
    if (preferred !== null && items.some((item) => item.id === preferred)) return preferred;
    return items[0]?.id ?? null;
  };

  const fireSelectionChange = (request: PickerRequest, id: string | null) => {
    if (request.onSelectionChange === undefined) return;
    const item = id === null ? null : (state.items.find((entry) => entry.id === id) ?? null);
    request.onSelectionChange(item);
  };

  const applyResult = (
    request: PickerRequest,
    query: string,
    result: PickerSourceResult,
    preferredSelection: string | null,
  ) => {
    const items = request.source.preRanked === true ? result.items : rankItems(result.items, query);
    const selectedId = selectionOf(items, preferredSelection);
    set({ items, selectedId, note: result.note, loading: false, error: null });
    fireSelectionChange(request, selectedId);
  };

  const runQuery = (request: PickerRequest, query: string, preferredSelection: string | null) => {
    const gen = ++generation;
    let answer: PickerSourceResult | Promise<PickerSourceResult>;
    try {
      answer = request.source.query(query);
    } catch (caught) {
      set({ items: [], selectedId: null, loading: false, error: describe(caught) });
      return;
    }
    if (isPromise(answer)) {
      set({ loading: true });
      answer.then(
        (result) => {
          if (generation !== gen || state.request !== request) return;
          applyResult(request, query, result, preferredSelection);
        },
        (caught) => {
          if (generation !== gen || state.request !== request) return;
          set({ items: [], selectedId: null, loading: false, error: describe(caught) });
        },
      );
    } else {
      applyResult(request, query, answer, preferredSelection);
    }
  };

  const dismiss = () => {
    if (!state.open) return;
    const request = state.request;
    clearDebounce();
    generation++;
    state = CLOSED;
    notify();
    request?.onDismiss?.();
  };

  return {
    state: () => state,
    subscribe: (listener) => {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },

    open: (request) => {
      // Opening over an open picker dismisses it first, so the previous
      // request's revert semantics (a theme preview) always run.
      if (state.open) dismiss();
      clearDebounce();
      state = {
        ...CLOSED,
        open: true,
        request: request as PickerRequest,
        query: "",
      };
      notify();
      runQuery(request as PickerRequest, "", request.initialSelectedId ?? null);
    },

    dismiss,

    setQuery: (query) => {
      const request = state.request;
      if (!state.open || request === null) return;
      set({ query });
      clearDebounce();
      const debounceMs = request.source.debounceMs ?? 0;
      // The empty query always runs immediately: clearing the input (or
      // opening) should never sit behind a backend debounce.
      if (debounceMs <= 0 || query.length === 0) {
        runQuery(request, query, null);
        return;
      }
      set({ loading: true });
      debounceTimer = setTimeout(() => {
        debounceTimer = undefined;
        if (state.request === request && state.query === query) {
          runQuery(request, query, null);
        }
      }, debounceMs);
    },

    setSelected: (id) => {
      const request = state.request;
      if (!state.open || request === null || state.selectedId === id) return;
      if (!state.items.some((item) => item.id === id)) return;
      set({ selectedId: id });
      fireSelectionChange(request, id);
    },

    choose: (id) => {
      const request = state.request;
      if (!state.open || request === null) return;
      const targetId = id ?? state.selectedId;
      const item = state.items.find((entry) => entry.id === targetId);
      if (item === undefined) return;
      clearDebounce();
      generation++;
      state = CLOSED;
      notify();
      request.onChoose(item);
    },

    refresh: () => {
      const request = state.request;
      if (!state.open || request === null) return;
      clearDebounce();
      runQuery(request, state.query, state.selectedId);
    },
  };
}

const isPromise = (value: unknown): value is Promise<PickerSourceResult> =>
  typeof value === "object" &&
  value !== null &&
  typeof (value as { then?: unknown }).then === "function";

const describe = (caught: unknown): string =>
  caught instanceof Error ? caught.message : String(caught);

// ---------------------------------------------------------------------------
// The operation
// ---------------------------------------------------------------------------

export class Picker extends Context.Tag("@the-editor/picker/Picker")<
  Picker,
  Picker.Service
>() {}

export namespace Picker {
  export interface Service {
    readonly open: <T>(request: PickerRequest<T>) => void;
    /** Close the open picker, if any (fires its `onDismiss`). */
    readonly close: () => void;
    /** Re-run the open picker's query (its source's data changed). */
    readonly refresh: () => void;
  }
}

/** The service over a controller — what the host puts in the services layer. */
export const makePicker = (controller: PickerController): Picker.Service => ({
  open: (request) => controller.open(request),
  close: () => controller.dismiss(),
  refresh: () => controller.refresh(),
});
