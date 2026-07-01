/**
 * MountHub: the widget mount contract between the renderer and product code.
 *
 * The renderer owns geometry and lifecycle: when a block widget enters the
 * rendered window it creates a container element of exactly the reserved size
 * and announces it here; when the widget leaves the window the element is
 * withdrawn. Product/extension code subscribes and portals any framework into
 * the container (React createPortal, plain DOM, canvas).
 */

import { Context } from "effect";

export type MountEvent =
  | { readonly type: "mounted"; readonly id: string; readonly el: HTMLElement }
  | { readonly type: "unmounted"; readonly id: string };

export class ViewMountHub extends Context.Tag("@the-editor/editor/ViewMountHub")<
  ViewMountHub,
  MountHub
>() {}

export class MountHub {
  private readonly els = new Map<string, HTMLElement>();
  private readonly listeners = new Set<(event: MountEvent) => void>();

  /** Renderer-side: announce a widget container. */
  mounted(id: string, el: HTMLElement): void {
    this.els.set(id, el);
    for (const listener of this.listeners) listener({ type: "mounted", id, el });
  }

  /** Renderer-side: withdraw a widget container. */
  unmounted(id: string): void {
    if (!this.els.delete(id)) return;
    for (const listener of this.listeners) listener({ type: "unmounted", id });
  }

  get(id: string): HTMLElement | undefined {
    return this.els.get(id);
  }

  all(): ReadonlyMap<string, HTMLElement> {
    return this.els;
  }

  /** Subscribe; current mounts are replayed so late subscribers catch up. */
  subscribe(listener: (event: MountEvent) => void): () => void {
    this.listeners.add(listener);
    for (const [id, el] of this.els) listener({ type: "mounted", id, el });
    return () => void this.listeners.delete(listener);
  }
}
