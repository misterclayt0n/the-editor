/**
 * The session store: a tiny subscribable mirror of the open sessions, so the
 * session-sidebar panel can SELF-MOUNT (read it via useSyncExternalStore) like
 * any other workbench panel — no App-portal bridge. App still owns session
 * lifecycle; it pushes the derived tab list here and wires the row actions.
 */

export type SessionTab = { readonly id: string; readonly label: string };
export type SessionSnapshot = {
  readonly tabs: ReadonlyArray<SessionTab>;
  readonly activeId: string | null;
};

export class SessionStore {
  private snapshot: SessionSnapshot = { tabs: [], activeId: null };
  private readonly listeners = new Set<() => void>();

  /** Row actions, wired by App (session lifecycle lives there). */
  onSelect: (id: string) => void = () => {};
  onAdd: () => void = () => {};
  onClose: (id: string) => void = () => {};

  /** Stable identity for useSyncExternalStore. */
  readonly getSnapshot = (): SessionSnapshot => this.snapshot;
  readonly subscribe = (listener: () => void): (() => void) => {
    this.listeners.add(listener);
    return () => void this.listeners.delete(listener);
  };

  /** App pushes the current tabs + active id; no-ops (keeping a stable snapshot
   *  reference) when nothing changed, so subscribers don't churn. */
  set(tabs: ReadonlyArray<SessionTab>, activeId: string | null): void {
    const prev = this.snapshot;
    const same =
      prev.activeId === activeId &&
      prev.tabs.length === tabs.length &&
      prev.tabs.every((t, i) => t.id === tabs[i]!.id && t.label === tabs[i]!.label);
    if (same) return;
    this.snapshot = { tabs, activeId };
    for (const listener of this.listeners) listener();
  }
}
