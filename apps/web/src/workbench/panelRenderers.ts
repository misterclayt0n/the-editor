/**
 * Panel renderers: the registry that makes the workbench's generic panel
 * model REAL for the React shell. The workbench core attaches no meaning to
 * a panel's kind; this registry maps kind → how to title and mount it.
 * Editor panes stay special-cased in WorkbenchView (they predate this);
 * everything else — file trees, shader toys, terminals — registers here
 * through the same scoped-contribution pattern as every other registry.
 */

import { Context, Effect, type Scope } from "effect";
import type { Panel, SurfaceKind } from "@the-editor/workbench";

export type PanelRenderer = {
  /** Tab title for panels of this kind. */
  readonly title: (panel: Panel) => string;
  /**
   * Mount into the slot body element; returns dispose. The element belongs
   * to the renderer until dispose — portal React, mount a canvas, whatever.
   */
  readonly mount: (element: HTMLElement, panel: Panel) => () => void;
};

export class PanelRenderers extends Context.Tag("the-editor/web/PanelRenderers")<
  PanelRenderers,
  {
    readonly register: (
      kind: SurfaceKind,
      renderer: PanelRenderer,
    ) => Effect.Effect<void, never, Scope.Scope>;
    /** Synchronous lookup — the React shell reads this during render. */
    readonly get: (kind: SurfaceKind) => PanelRenderer | undefined;
    readonly onChanged: (listener: () => void) => () => void;
  }
>() {}

export const makePanelRenderers = (): PanelRenderers.Service => {
  const renderers = new Map<SurfaceKind, PanelRenderer>();
  const listeners = new Set<() => void>();
  const notify = () => {
    for (const listener of listeners) listener();
  };
  return {
    register: (kind, renderer) =>
      Effect.acquireRelease(
        Effect.sync(() => {
          renderers.set(kind, renderer);
          notify();
        }),
        () =>
          Effect.sync(() => {
            if (renderers.get(kind) === renderer) renderers.delete(kind);
            notify();
          }),
      ).pipe(Effect.asVoid),
    get: (kind) => renderers.get(kind),
    onChanged: (listener) => {
      listeners.add(listener);
      return () => void listeners.delete(listener);
    },
  };
};

export namespace PanelRenderers {
  export type Service = Context.Tag.Service<PanelRenderers>;
}
