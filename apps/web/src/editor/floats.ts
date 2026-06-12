/**
 * Floats: the popover primitive. A float is a DOM element anchored to a
 * TEXT OFFSET — the renderer owns its geometry, repositioning it every
 * frame (so it stays glued through scrolling, edits, and reflows, exactly
 * like the caret) and flipping it above the line when the viewport runs
 * out of room below. Products portal arbitrary UI into the element:
 * completion lists, hover cards, signature help — anything anchored to
 * text is a float.
 */

export type FloatPlacement = "below" | "above";

export type FloatAnchor = {
  readonly offset: number;
  /** Preferred side of the anchored line; flips when space runs out. */
  readonly placement: FloatPlacement;
};

export interface FloatHandle {
  readonly element: HTMLElement;
  setAnchor(anchor: FloatAnchor): void;
  hide(): void;
  dispose(): void;
}

export type FloatEntry = {
  readonly element: HTMLElement;
  anchor: FloatAnchor | null;
};

export class FloatHub {
  private readonly entries = new Set<FloatEntry>();
  private readonly listeners = new Set<() => void>();

  /** Renderer side: current floats; re-read on change notifications. */
  all(): ReadonlySet<FloatEntry> {
    return this.entries;
  }

  onChanged(listener: () => void): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  /** Product side: create an anchored element to portal UI into. */
  create(): FloatHandle {
    const element = document.createElement("div");
    element.className = "editor-float";
    element.style.display = "none";
    const entry: FloatEntry = { element, anchor: null };
    this.entries.add(entry);
    this.notify();
    return {
      element,
      setAnchor: (anchor) => {
        entry.anchor = anchor;
        element.style.display = "";
        this.notify();
      },
      hide: () => {
        entry.anchor = null;
        element.style.display = "none";
        this.notify();
      },
      dispose: () => {
        this.entries.delete(entry);
        element.remove();
        this.notify();
      },
    };
  }

  private notify(): void {
    for (const listener of this.listeners) listener();
  }
}
