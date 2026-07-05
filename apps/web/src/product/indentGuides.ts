import { Effect, Fiber, Stream } from "effect";
import { RangeSet, type RenderableText, type Selection } from "@the-editor/engine";
import { definePackage } from "@the-editor/extensions";
import {
  DecorationProviders,
  DocumentInvalidated,
  SelectionInvalidated,
  hasInvalidation,
  type DecorationViewTarget,
  type EditorVisualSet,
  type VerticalGuideSpec,
} from "@the-editor/editor";

const OWNER = "the-editor/indent-guides";
const DEBOUNCE_MS = 25;
const TAB_WIDTH = 4;

export type IndentGuideRange = {
  readonly startLine: number;
  readonly endLine: number;
  /** Zero-based indent-guide depth. Depth 0 is the first visible guide. */
  readonly depth: number;
  readonly active: boolean;
};

const emptySet: EditorVisualSet = { verticalGuides: RangeSet.empty() };

const isBlank = (text: string): boolean => !/[^\t ]/.test(text);

export function indentationColumns(line: string, tabWidth = TAB_WIDTH): number | null {
  if (isBlank(line)) return null;
  let column = 0;
  for (const char of line) {
    if (char === " ") column++;
    else if (char === "\t") column += tabWidth - (column % tabWidth);
    else break;
  }
  return column;
}

const previousNonBlankIndent = (
  indents: readonly (number | null)[],
  before: number,
): number | null => {
  for (let line = before - 1; line >= 0; line--) if (indents[line] !== null) return indents[line]!;
  return null;
};

const nextNonBlankIndent = (indents: readonly (number | null)[], after: number): number | null => {
  for (let line = after + 1; line < indents.length; line++) {
    if (indents[line] !== null) return indents[line]!;
  }
  return null;
};

/** Monaco-style blank-line handling: derive blank-line guides from surrounding content. */
export function indentDepthsForLines(
  lines: readonly string[],
  tabWidth = TAB_WIDTH,
): readonly number[] {
  const indents = lines.map((line) => indentationColumns(line, tabWidth));
  return indents.map((indent, line) => {
    if (indent !== null) return Math.floor(indent / tabWidth);
    const above = previousNonBlankIndent(indents, line);
    const below = nextNonBlankIndent(indents, line);
    if (above === null || below === null) return 0;
    if (above < below) return 1 + Math.floor(above / tabWidth);
    if (above === below) return Math.floor(below / tabWidth);
    return 1 + Math.floor(below / tabWidth);
  });
}

function guideRangesFromDepths(
  depths: readonly number[],
): readonly Omit<IndentGuideRange, "active">[] {
  const guides: Omit<IndentGuideRange, "active">[] = [];
  const stack: { readonly startLine: number; readonly depth: number; endLine: number }[] = [];

  for (let line = 0; line < depths.length; line++) {
    const depth = Math.max(0, depths[line] ?? 0);
    while (stack.length > depth) guides.push(stack.pop()!);
    while (stack.length < depth) stack.push({ startLine: line, endLine: line, depth: stack.length });
    for (const guide of stack) guide.endLine = line;
  }
  guides.push(...stack);
  return guides;
}

export function activeIndentGuide(
  depths: readonly number[],
  line: number,
): { readonly line: number; readonly depth: number } | null {
  if (depths.length === 0) return null;
  line = Math.max(0, Math.min(depths.length - 1, line));
  const currentDepth = depths[line] ?? 0;
  const nextDepth = depths[line + 1] ?? 0;
  const depth = nextDepth > currentDepth ? nextDepth - 1 : currentDepth - 1;
  return depth >= 0 ? { line, depth } : null;
}

export function indentGuideRanges(
  text: RenderableText,
  selection: Selection,
  tabWidth = TAB_WIDTH,
): readonly IndentGuideRange[] {
  const lines: string[] = [];
  for (let line = 0; line < text.lineCount; line++) {
    const range = text.lineRange(line);
    lines.push(text.sliceString(range.from, range.to));
  }
  const depths = indentDepthsForLines(lines, tabWidth);
  const selectionLine = selection.rangeCount === 1 && selection.primary.isCollapsed
    ? text.lineRangeAt(Math.min(selection.primary.head, text.length)).index
    : -1;
  const active = selectionLine >= 0 ? activeIndentGuide(depths, selectionLine) : null;
  return guideRangesFromDepths(depths).map((guide) => ({
    ...guide,
    active:
      active !== null &&
      guide.depth === active.depth &&
      guide.startLine <= Math.max(active.line, active.line + 1) &&
      active.line <= guide.endLine + 1,
  }));
}

const offsetRangeForLines = (
  text: RenderableText,
  startLine: number,
  endLine: number,
): { readonly from: number; readonly to: number } => {
  const start = text.lineRange(Math.max(0, Math.min(text.lineCount - 1, startLine)));
  const end = text.lineRange(Math.max(0, Math.min(text.lineCount - 1, endLine)));
  return { from: start.from, to: Math.max(end.breakTo, end.to, start.from) };
};

const specFor = (guide: IndentGuideRange): VerticalGuideSpec => ({
  column: guide.depth * TAB_WIDTH,
  widthPx: 1,
  role: guide.active ? "indent-guide-active" : "indent-guide",
  className: guide.active ? "editor-indent-guide editor-indent-guide-active" : "editor-indent-guide",
  priority: guide.active ? 1 : 0,
});

const visualSetFor = (text: RenderableText, guides: readonly IndentGuideRange[]): EditorVisualSet => ({
  verticalGuides: RangeSet.of(
    guides.map((guide) => ({
      ...offsetRangeForLines(text, guide.startLine, guide.endLine),
      value: specFor(guide),
    })),
  ),
});

const rangesOf = (text: RenderableText, guides: readonly IndentGuideRange[]) =>
  guides.map((guide) => offsetRangeForLines(text, guide.startLine, guide.endLine));

const attachIndentGuides = (target: DecorationViewTarget) =>
  Effect.gen(function* () {
    const handle = yield* target.viewVisuals.contribute(OWNER, emptySet);
    let currentRanges: readonly { readonly from: number; readonly to: number }[] = [];
    let latestToken = 0;
    let pending: Fiber.RuntimeFiber<void, never> | null = null;

    const publish = (text: RenderableText, guides: readonly IndentGuideRange[]) => {
      const nextRanges = rangesOf(text, guides);
      const changes = currentRanges.length === 0 && nextRanges.length === 0 ? undefined : [...currentRanges, ...nextRanges];
      return handle.update(visualSetFor(text, guides), changes).pipe(
        Effect.tap(() =>
          Effect.sync(() => {
            currentRanges = nextRanges;
          }),
        ),
      );
    };

    const cancelPending = Effect.gen(function* () {
      if (pending !== null) {
        yield* Fiber.interruptFork(pending);
        pending = null;
      }
    });

    const schedule = Effect.gen(function* () {
      const token = ++latestToken;
      yield* cancelPending;
      pending = Effect.runFork(
        Effect.gen(function* () {
          yield* Effect.sleep(DEBOUNCE_MS);
          const text = yield* target.text.get;
          const selection = yield* target.selection.get;
          const guides = indentGuideRanges(text, selection, TAB_WIDTH);
          if (token !== latestToken) return;
          yield* publish(text, guides);
        }),
      );
    });

    yield* Effect.addFinalizer(() => cancelPending);
    yield* schedule;
    yield* Stream.runForEach(
      Stream.merge(target.viewNotifications.notifications, target.notifications.notifications),
      (notification) =>
        hasInvalidation(notification, SelectionInvalidated) ||
        hasInvalidation(notification, DocumentInvalidated)
          ? schedule
          : Effect.void,
    ).pipe(Effect.forkScoped);
  });

export const indentGuides = definePackage(OWNER, () =>
  Effect.gen(function* () {
    const decorations = yield* DecorationProviders;
    yield* decorations.register({ id: OWNER, attachView: attachIndentGuides });
  }),
);
