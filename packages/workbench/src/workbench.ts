/**
 * The Workbench service: generic layout and surface state.
 *
 * Owns four truths — live panels, the arrangement (validated + normalized at
 * every commit), the focused slot, and focus history (so dismissing overlays
 * and closing panels restores focus). It renders nothing, knows no surface
 * semantics, and ships no gestures: products and user packages transform the
 * arrangement with pure functions and commit through `updateArrangement`.
 */

import { Context, Effect, Exit, Option, Ref, Scope, type Stream } from "effect";
import { makeStateCell } from "@the-editor/kernel";
import {
  emptyArrangement,
  findSlot,
  normalizeArrangement,
  referencedPanels,
  slotPanel,
  validateArrangement,
  type Arrangement,
} from "./arrangement.js";
import { PanelNotFoundError, SlotNotFoundError, type ArrangementError } from "./errors.js";
import { PanelId, type SlotId } from "./ids.js";
import type { CreatePanelInput, Panel } from "./panel.js";
import { removePanel } from "./surgery.js";

const FOCUS_HISTORY_LIMIT = 64;

let nextPanelId = 1;

export class Workbench extends Context.Tag("@the-editor/workbench/Workbench")<
  Workbench,
  Workbench.Service
>() {}

export namespace Workbench {
  export interface Service {
    /** Create a live, undisplayed panel. Display is arrangement surgery. */
    readonly createPanel: (input: CreatePanelInput) => Effect.Effect<Panel>;
    /** Close a panel: drop every reference, fix focus, close its scope. */
    readonly closePanel: (id: PanelId) => Effect.Effect<void>;
    readonly panel: (id: PanelId) => Effect.Effect<Option.Option<Panel>>;
    readonly panels: Effect.Effect<ReadonlyArray<Panel>>;

    readonly arrangement: Effect.Effect<Arrangement>;
    readonly arrangementChanges: Stream.Stream<Arrangement>;
    /** Transform → normalize → validate → commit. The only write path. */
    readonly updateArrangement: (
      f: (current: Arrangement) => Arrangement,
    ) => Effect.Effect<Arrangement, ArrangementError>;

    readonly focusedSlot: Effect.Effect<Option.Option<SlotId>>;
    readonly focusChanges: Stream.Stream<Option.Option<SlotId>>;
    readonly setFocus: (slot: SlotId) => Effect.Effect<void, SlotNotFoundError>;
    /** The panel shown by the focused slot (a stack's active tab, or an overlay's panel). */
    readonly focusedPanel: Effect.Effect<Option.Option<Panel>>;

    /** Workbench base context merged under the panel's own context. */
    readonly contextAt: (id: PanelId) => Effect.Effect<Context.Context<never>, PanelNotFoundError>;
    /** Context at the focused panel; base context when nothing is focused. */
    readonly focusedContext: Effect.Effect<Context.Context<never>>;
  }
}

export type MakeWorkbenchOptions = {
  /** Services available everywhere (merged under every panel context). */
  readonly baseContext?: Context.Context<never>;
};

export const makeWorkbench = (
  options?: MakeWorkbenchOptions,
): Effect.Effect<Workbench.Service, never, Scope.Scope> =>
  Effect.gen(function* () {
    const baseContext = options?.baseContext ?? Context.empty();
    const panelsRef = yield* Ref.make(new Map<PanelId, Panel>());
    const arrangementCell = yield* makeStateCell<Arrangement>(emptyArrangement);
    const focusCell = yield* makeStateCell<Option.Option<SlotId>>(Option.none());
    const focusHistory = yield* Ref.make<ReadonlyArray<SlotId>>([]);

    // Workbench teardown closes every panel scope that is still alive.
    yield* Effect.addFinalizer(() =>
      Ref.get(panelsRef).pipe(
        Effect.flatMap((panels) =>
          Effect.forEach(panels.values(), (panel) => Scope.close(panel.scope, Exit.void), {
            discard: true,
          }),
        ),
      ),
    );

    /**
     * After any commit the focused slot may have vanished (tab closed, overlay
     * dismissed). Pop history to the most recent slot that still exists.
     */
    const repairFocus = (arrangement: Arrangement): Effect.Effect<void> =>
      Effect.gen(function* () {
        const focused = yield* focusCell.get;
        if (Option.isSome(focused) && findSlot(arrangement, focused.value)) return;
        const history = yield* Ref.get(focusHistory);
        const liveIndex = history.findLastIndex((slot) => findSlot(arrangement, slot) !== undefined);
        const next = liveIndex === -1 ? Option.none<SlotId>() : Option.some(history[liveIndex]!);
        yield* Ref.set(focusHistory, liveIndex === -1 ? [] : history.slice(0, liveIndex));
        yield* focusCell.set(next);
      });

    const commit = (
      f: (current: Arrangement) => Arrangement,
    ): Effect.Effect<Arrangement, ArrangementError> =>
      Effect.gen(function* () {
        const current = yield* arrangementCell.get;
        const next = normalizeArrangement(f(current));
        const live = yield* Ref.get(panelsRef);
        const error = validateArrangement(next, new Set(live.keys()) as ReadonlySet<PanelId>);
        if (error) return yield* Effect.fail(error);
        yield* arrangementCell.set(next);
        yield* repairFocus(next);
        return next;
      });

    const panelById = (id: PanelId): Effect.Effect<Option.Option<Panel>> =>
      Ref.get(panelsRef).pipe(Effect.map((panels) => Option.fromNullable(panels.get(id))));

    const focusedPanel: Effect.Effect<Option.Option<Panel>> = Effect.gen(function* () {
      const focused = yield* focusCell.get;
      if (Option.isNone(focused)) return Option.none();
      const arrangement = yield* arrangementCell.get;
      const panelId = slotPanel(arrangement, focused.value);
      if (panelId === undefined) return Option.none();
      return yield* panelById(panelId);
    });

    return {
      createPanel: (input) =>
        Effect.gen(function* () {
          const id = PanelId(input.id ?? `panel:${nextPanelId++}`);
          const scope = yield* Scope.make();
          const panel: Panel = {
            id,
            kind: input.kind,
            scope,
            context: input.context ?? Context.empty(),
          };
          yield* Ref.update(panelsRef, (panels) => new Map(panels).set(id, panel));
          return panel;
        }),

      closePanel: (id) =>
        Effect.gen(function* () {
          const panel = yield* panelById(id);
          if (Option.isNone(panel)) return;
          yield* Ref.update(panelsRef, (panels) => {
            const next = new Map(panels);
            next.delete(id);
            return next;
          });
          // Strip every reference; cannot fail validation if the previous
          // arrangement was valid, so a failure here is a defect.
          yield* commit((current) => removePanel(current, id)).pipe(Effect.orDie);
          yield* Scope.close(panel.value.scope, Exit.void);
        }),

      panel: panelById,
      panels: Ref.get(panelsRef).pipe(Effect.map((panels) => [...panels.values()])),

      arrangement: arrangementCell.get,
      arrangementChanges: arrangementCell.changes,
      updateArrangement: commit,

      focusedSlot: focusCell.get,
      focusChanges: focusCell.changes,
      setFocus: (slot) =>
        Effect.gen(function* () {
          const arrangement = yield* arrangementCell.get;
          if (!findSlot(arrangement, slot)) {
            return yield* Effect.fail(new SlotNotFoundError({ slotId: slot }));
          }
          const previous = yield* focusCell.get;
          if (Option.isSome(previous) && previous.value !== slot) {
            yield* Ref.update(focusHistory, (history) =>
              [...history, previous.value].slice(-FOCUS_HISTORY_LIMIT),
            );
          }
          yield* focusCell.set(Option.some(slot));
        }),
      focusedPanel,

      contextAt: (id) =>
        panelById(id).pipe(
          Effect.flatMap(
            Option.match({
              onNone: () => Effect.fail(new PanelNotFoundError({ panelId: id })),
              onSome: (panel) => Effect.succeed(Context.merge(baseContext, panel.context)),
            }),
          ),
        ),
      focusedContext: focusedPanel.pipe(
        Effect.map(
          Option.match({
            onNone: () => baseContext,
            onSome: (panel) => Context.merge(baseContext, panel.context),
          }),
        ),
      ),
    } satisfies Workbench.Service;
  });
