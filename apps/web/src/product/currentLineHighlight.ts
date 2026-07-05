import { Effect, Stream } from "effect";
import { RangeSet, type Selection } from "@the-editor/engine";
import { definePackage } from "@the-editor/extensions";
import {
  DecorationProviders,
  SelectionInvalidated,
  hasInvalidation,
  type DecorationViewTarget,
  type EditorVisualSet,
  type LineDecorationSpec,
} from "@the-editor/editor";

const OWNER = "the-editor/current-line-highlight";

const currentLineSpec: LineDecorationSpec = {
  target: "content",
  scope: "visual-row",
  paint: "horizontal-border",
  role: "current-line",
  className: "editor-current-line",
  priority: 0,
};

const emptySet: EditorVisualSet = { lineDecorations: RangeSet.empty() };

export function currentLineOffsets(selection: Selection): readonly number[] {
  if (!selection.ranges.every((range) => range.isCollapsed)) return [];
  return [...new Set(selection.ranges.map((range) => range.head))].sort((a, b) => a - b);
}

const visualSetFor = (offsets: readonly number[]): EditorVisualSet => ({
  lineDecorations: RangeSet.of(
    offsets.map((offset) => ({ from: offset, to: offset, value: currentLineSpec })),
  ),
});

const changesFor = (
  before: readonly number[],
  after: readonly number[],
): readonly { readonly from: number; readonly to: number }[] | undefined => {
  if (before.length === 0 && after.length === 0) return undefined;
  return [...before, ...after].map((offset) => ({ from: offset, to: offset }));
};

const attachCurrentLineHighlight = (target: DecorationViewTarget) =>
  Effect.gen(function* () {
    const handle = yield* target.viewVisuals.contribute(OWNER, emptySet);
    let current: readonly number[] = [];

    const refresh = Effect.gen(function* () {
      const selection = yield* target.selection.get;
      const next = currentLineOffsets(selection);
      const set = next.length === 0 ? emptySet : visualSetFor(next);
      yield* handle.update(set, changesFor(current, next));
      current = next;
    });

    yield* refresh;
    yield* Stream.runForEach(target.viewNotifications.notifications, (notification) =>
      hasInvalidation(notification, SelectionInvalidated) ? refresh : Effect.void,
    ).pipe(Effect.forkScoped);
  });

export const currentLineHighlight = definePackage(OWNER, () =>
  Effect.gen(function* () {
    const decorations = yield* DecorationProviders;
    yield* decorations.register({ id: OWNER, attachView: attachCurrentLineHighlight });
  }),
);
