/**
 * Statusline as a segment registry. The shell renders the bar FROM this
 * registry instead of hardcoding it: first-party segments (file name, cursor
 * position, VCS branch, diagnostics, pending chords) register here reading host
 * state, and an extension adds its own segment the same way.
 *
 * Segments are pull-based DATA, not React: the shell recomputes `content()`
 * every render (the bar already re-renders on focus/edit/selection ticks), so a
 * segment stays live without its own subscription. Content is a plain
 * descriptor any shell can draw.
 */

import { Context, Effect, type Scope } from "effect";

export type StatuslineContent = {
  readonly text: string;
  readonly tooltip?: string;
  readonly className?: string;
} | null;

export type StatuslineSegment = {
  readonly id: string;
  /** Which end of the bar. Default "left". */
  readonly side?: "left" | "right";
  /** Lower sorts closer to the outer edge (left: leftmost; right: rightmost). */
  readonly priority?: number;
  /** Pulled every render; return null to render nothing this frame. */
  readonly content: () => StatuslineContent;
};

export class StatuslineSegments extends Context.Tag("@the-editor/workbench/StatuslineSegments")<
  StatuslineSegments,
  StatuslineSegments.Service
>() {}

export const makeStatuslineSegments = (): StatuslineSegments.Service => {
  const segments = new Set<StatuslineSegment>();
  const listeners = new Set<() => void>();
  const notify = () => {
    for (const listener of listeners) listener();
  };
  return {
    register: (segment) =>
      Effect.acquireRelease(
        Effect.sync(() => {
          segments.add(segment);
          notify();
        }),
        () =>
          Effect.sync(() => {
            segments.delete(segment);
            notify();
          }),
      ).pipe(Effect.asVoid),
    /** Segments for a side, sorted by priority then registration order. */
    forSide: (side) =>
      [...segments]
        .filter((segment) => (segment.side ?? "left") === side)
        .sort((a, b) => (a.priority ?? 0) - (b.priority ?? 0)),
    onChanged: (listener) => {
      listeners.add(listener);
      return () => void listeners.delete(listener);
    },
  };
};

export namespace StatuslineSegments {
  export interface Service {
    readonly register: (segment: StatuslineSegment) => Effect.Effect<void, never, Scope.Scope>;
    readonly forSide: (side: "left" | "right") => readonly StatuslineSegment[];
    readonly onChanged: (listener: () => void) => () => void;
  }
}
