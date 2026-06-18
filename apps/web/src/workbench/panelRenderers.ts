/**
 * Panel renderers: the registry that makes the workbench's generic panel
 * model REAL for the React shell. The workbench core attaches no meaning to
 * a panel's kind; this registry maps kind → how to title and mount it.
 * Editor panes stay special-cased in WorkbenchView (they predate this);
 * everything else — file trees, shader toys, terminals — registers here
 * through the same scoped-contribution pattern as every other registry.
 */

import type { ReactNode } from "react";
import { Context, Effect, type Scope } from "effect";
import type { Panel, SurfaceKind } from "@the-editor/workbench";

/** A live mounted panel: how to tear it down, and (optionally) how to take
 * keyboard focus when the workbench focuses its slot. A panel that owns its
 * own focusable content (e.g. @pierre/trees in a shadow root) implements
 * `focus` so the workbench can hand it the keyboard, the same way an editor
 * pane's `.editor` element is focused — that's what makes every surface
 * keyboard-drivable. */
export type PanelMount = {
  readonly dispose: () => void;
  readonly focus?: () => void;
};

export type PanelRenderer = {
  /** Tab title for panels of this kind. */
  readonly title: (panel: Panel) => string;
  /** Optional icon rendered before the tab title for panels of this kind. */
  readonly icon?: (panel: Panel) => ReactNode;
  /**
   * Mount into the slot body element; returns a handle. The element belongs
   * to the renderer until dispose — portal React, mount a canvas, whatever.
   */
  readonly mount: (element: HTMLElement, panel: Panel) => PanelMount;
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
