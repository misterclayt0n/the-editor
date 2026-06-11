/**
 * EditorVisuals: the ownership and lifecycle layer for everything visual
 * that anchors to text — text styles, range decorations, virtual text,
 * block widgets, gutter bands.
 *
 * The contract that keeps extensions structurally fast:
 *
 * - Owners contribute SETS (engine RangeSets), not per-call arrays. The
 *   renderer queries only the viewport window via the snapshot, so per-frame
 *   cost is O(window + log totalVisuals) no matter how much an extension
 *   contributes.
 * - Document edits slide every set through `mapChanges` (chunk-shared, O(log)
 *   away from the edit) without the owner running at all. Owners recompute
 *   only when THEY have new information, via their handle.
 * - Contributions are scoped: closing the contributing scope (e.g. package
 *   deactivation) retracts the visuals automatically.
 * - Every mutation publishes VisualsInvalidated with the affected offset
 *   ranges, so renderers extend their per-row dirty tracking to visuals.
 *
 * This service knows no product semantics: a "gutter band" is a width
 * reservation with an opaque per-line cell, a "widget" is reserved rows at
 * an anchored offset. What they MEAN belongs to the owners.
 */

import { Context, Effect, Ref, type Scope } from "effect";
import { RangeSet, type ChangeDesc, type TextBuffer } from "@the-editor/engine";
import {
  type DecorationLayer,
  type RangeDecoration,
  type TextStyleRun,
  type VirtualText,
  type VirtualTextAffinity,
} from "./layout";
import { invalidate, makeInvalidationTopic, type NotificationBus } from "./notifications";

export type TextStyleSpec = {
  readonly className?: string;
  readonly style?: Readonly<Record<string, string>>;
  readonly priority?: number;
};

export type DecorationSpec = {
  readonly layer: DecorationLayer;
  readonly className?: string;
  readonly style?: Readonly<Record<string, string>>;
  readonly priority?: number;
};

export type VirtualTextSpec = {
  readonly text: string;
  readonly affinity?: VirtualTextAffinity;
  readonly className?: string;
  readonly style?: Readonly<Record<string, string>>;
  readonly priority?: number;
};

/** Reserved rows after the line containing the anchor offset. */
export type BlockWidgetSpec = {
  readonly id: string;
  readonly heightRows: number;
};

export type GutterCell = {
  readonly text: string;
  readonly className?: string;
};

/**
 * A width reservation beside the text. The engine/renderer know only the
 * geometry; `cell` is the owner's fast text-mode content for a line (mount
 * mode for arbitrary components rides the widget mount contract).
 */
export type GutterBandSpec = {
  readonly id: string;
  readonly side: "left" | "right";
  readonly widthCh: number;
  readonly priority?: number;
  readonly cell?: (line: number) => GutterCell | null;
};

export type EditorVisualSet = {
  readonly textStyles?: RangeSet<TextStyleSpec>;
  readonly decorations?: RangeSet<DecorationSpec>;
  /** Point ranges (from === to). */
  readonly virtualText?: RangeSet<VirtualTextSpec>;
  /** Point ranges anchored anywhere in the widget's line. */
  readonly blockWidgets?: RangeSet<BlockWidgetSpec>;
  readonly gutterBands?: ReadonlyArray<GutterBandSpec>;
};

export type AnchoredBlockWidget = {
  readonly id: string;
  readonly atOffset: number;
  readonly afterLine: number;
  readonly heightRows: number;
  readonly owner: string;
};

export type ResolvedGutterBand = GutterBandSpec & { readonly owner: string };

/**
 * Immutable merged view over all contributions. Plain synchronous methods:
 * this object is what render hot paths consume.
 */
export type VisualsSnapshot = {
  /** Bumps on contribute/update/retract — NOT on document mapping. */
  readonly generation: number;
  readonly textStylesBetween: (from: number, to: number) => TextStyleRun[];
  readonly decorationsBetween: (from: number, to: number) => RangeDecoration[];
  readonly virtualTextBetween: (from: number, to: number) => VirtualText[];
  /** All widgets, resolved to lines against `text`, sorted by line. */
  readonly widgets: (text: TextBuffer) => AnchoredBlockWidget[];
  readonly totalWidgetRows: number;
  /** Bands sorted by (side, priority, owner). */
  readonly gutterBands: ReadonlyArray<ResolvedGutterBand>;
};

export type VisualsInvalidation = {
  readonly generation: number;
  /**
   * Offset ranges whose visuals changed, in current document coordinates.
   * Absent = treat everything as changed.
   */
  readonly changes?: ReadonlyArray<{ readonly from: number; readonly to: number }>;
};

export const VisualsInvalidated = makeInvalidationTopic<VisualsInvalidation>(
  "@the-editor/editor/visuals",
);

export interface VisualsHandle {
  /** Replace this contribution's set. `changes` localizes the invalidation. */
  readonly update: (
    set: EditorVisualSet,
    changes?: ReadonlyArray<{ readonly from: number; readonly to: number }>,
  ) => Effect.Effect<void>;
}

export class EditorVisuals extends Context.Tag("@the-editor/editor/EditorVisuals")<
  EditorVisuals,
  EditorVisuals.Service
>() {}

export namespace EditorVisuals {
  export interface Service {
    /**
     * Contribute a visual set. Retracted automatically when the surrounding
     * scope closes (package deactivation).
     */
    readonly contribute: (
      owner: string,
      set: EditorVisualSet,
    ) => Effect.Effect<VisualsHandle, never, Scope.Scope>;
    /** Current merged snapshot (cached per generation/mapping). */
    readonly snapshot: Effect.Effect<VisualsSnapshot>;
    /**
     * Slide all sets through a transaction. Called on the commit path,
     * before the document notification publishes. Does not bump generation:
     * relative alignment with the text is preserved by construction.
     */
    readonly mapChanges: (changes: ChangeDesc) => Effect.Effect<void>;
  }
}

type Entry = {
  readonly owner: string;
  readonly set: EditorVisualSet;
};

type State = {
  readonly entries: ReadonlyMap<number, Entry>;
  readonly generation: number;
  readonly snapshotCache: VisualsSnapshot | null;
};

function buildSnapshot(state: State): VisualsSnapshot {
  const entries = [...state.entries.values()];
  const styleSets = entries.filter((e) => e.set.textStyles !== undefined);
  const decorationSets = entries.filter((e) => e.set.decorations !== undefined);
  const virtualSets = entries.filter((e) => e.set.virtualText !== undefined);
  const widgetSets = entries.filter((e) => e.set.blockWidgets !== undefined);

  let totalWidgetRows = 0;
  for (const entry of widgetSets) {
    for (const range of entry.set.blockWidgets!.all()) {
      totalWidgetRows += Math.max(0, range.value.heightRows);
    }
  }

  const gutterBands = entries
    .flatMap((entry) =>
      (entry.set.gutterBands ?? []).map((band) => ({ ...band, owner: entry.owner })),
    )
    .sort(
      (a, b) =>
        (a.side === b.side ? 0 : a.side === "left" ? -1 : 1) ||
        (a.priority ?? 0) - (b.priority ?? 0) ||
        a.owner.localeCompare(b.owner),
    );

  return {
    generation: state.generation,
    textStylesBetween: (from, to) =>
      styleSets.flatMap((entry) =>
        entry.set
          .textStyles!.between(from, to)
          .map((range) => ({ from: range.from, to: range.to, owner: entry.owner, ...range.value })),
      ),
    decorationsBetween: (from, to) =>
      decorationSets.flatMap((entry) =>
        entry.set
          .decorations!.between(from, to)
          .map((range) => ({ from: range.from, to: range.to, owner: entry.owner, ...range.value })),
      ),
    virtualTextBetween: (from, to) =>
      virtualSets.flatMap((entry) =>
        entry.set
          .virtualText!.between(from, to)
          .map((range) => ({ at: range.from, owner: entry.owner, ...range.value })),
      ),
    widgets: (text) =>
      widgetSets
        .flatMap((entry) =>
          entry.set.blockWidgets!.all().map((range) => {
            const atOffset = Math.min(range.from, text.length);
            return {
              id: range.value.id,
              atOffset,
              afterLine: text.lineRangeAt(atOffset).index,
              heightRows: range.value.heightRows,
              owner: entry.owner,
            };
          }),
        )
        .sort((a, b) => a.afterLine - b.afterLine || a.id.localeCompare(b.id)),
    totalWidgetRows,
    gutterBands,
  };
}

export const makeEditorVisuals = (
  notifications: NotificationBus,
): Effect.Effect<EditorVisuals.Service> =>
  Effect.gen(function* () {
    const state = yield* Ref.make<State>({
      entries: new Map(),
      generation: 0,
      snapshotCache: null,
    });
    let nextHandle = 1;

    const publish = (generation: number, changes?: ReadonlyArray<{ from: number; to: number }>) =>
      notifications.publish([
        invalidate(VisualsInvalidated, { generation, ...(changes ? { changes } : {}) }),
      ]);

    const mutate = (
      f: (entries: Map<number, Entry>) => void,
      changes?: ReadonlyArray<{ from: number; to: number }>,
    ) =>
      Ref.modify(state, (current) => {
        const entries = new Map(current.entries);
        f(entries);
        const generation = current.generation + 1;
        return [generation, { entries, generation, snapshotCache: null }];
      }).pipe(Effect.flatMap((generation) => publish(generation, changes)));

    const contribute = (owner: string, set: EditorVisualSet) =>
      Effect.gen(function* () {
        const handleId = nextHandle++;
        yield* Effect.acquireRelease(
          mutate((entries) => entries.set(handleId, { owner, set })),
          () => mutate((entries) => entries.delete(handleId)),
        );
        const update = (
          next: EditorVisualSet,
          changes?: ReadonlyArray<{ from: number; to: number }>,
        ) => mutate((entries) => entries.set(handleId, { owner, set: next }), changes);
        return { update } satisfies VisualsHandle;
      });

    const snapshot = Ref.modify(state, (current) => {
      const cached = current.snapshotCache ?? buildSnapshot(current);
      return [cached, { ...current, snapshotCache: cached }];
    });

    const mapChanges = (changes: ChangeDesc) =>
      Ref.update(state, (current) => {
        const entries = new Map<number, Entry>();
        for (const [id, entry] of current.entries) {
          const set: EditorVisualSet = {
            ...(entry.set.textStyles ? { textStyles: entry.set.textStyles.map(changes) } : {}),
            ...(entry.set.decorations ? { decorations: entry.set.decorations.map(changes) } : {}),
            ...(entry.set.virtualText ? { virtualText: entry.set.virtualText.map(changes) } : {}),
            ...(entry.set.blockWidgets
              ? { blockWidgets: entry.set.blockWidgets.map(changes) }
              : {}),
            ...(entry.set.gutterBands ? { gutterBands: entry.set.gutterBands } : {}),
          };
          entries.set(id, { owner: entry.owner, set });
        }
        // Mapping preserves text-relative alignment; generation unchanged,
        // but the cache must rebuild against the new positions.
        return { ...current, entries, snapshotCache: null };
      });

    return { contribute, snapshot, mapChanges } satisfies EditorVisuals.Service;
  });
