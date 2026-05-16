import type { UiCommandDispatcher } from "@the-editor/editor/ui";

export type WorkbenchDragTarget =
  | { kind: "tabstrip"; groupId: string; index: number }
  | { kind: "split"; groupId: string; direction: "left" | "right" | "up" | "down" };

export interface WorkbenchDragSnapshot {
  readonly draggedPaneId?: string;
  readonly sourceGroupId?: string;
  readonly currentTarget?: WorkbenchDragTarget;
}

export class WorkbenchDragController {
  private state: WorkbenchDragSnapshot = {};
  private readonly listeners = new Set<() => void>();

  constructor(private readonly commands: UiCommandDispatcher) {}

  subscribe = (listener: () => void): (() => void) => {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  };

  snapshot = (): WorkbenchDragSnapshot => this.state;

  beginPaneDrag(paneId: string, sourceGroupId: string): void {
    this.set({ draggedPaneId: paneId, sourceGroupId, currentTarget: undefined });
  }

  updateTarget(target: WorkbenchDragTarget | undefined): void {
    if (!this.state.draggedPaneId) return;
    if (sameTarget(this.state.currentTarget, target)) return;
    this.set({ ...this.state, currentTarget: target });
  }

  clear(): void {
    if (!this.state.draggedPaneId && !this.state.currentTarget) return;
    this.set({});
  }

  async drop(): Promise<void> {
    const { draggedPaneId, currentTarget } = this.state;
    this.clear();
    if (!draggedPaneId || !currentTarget) return;
    if (currentTarget.kind === "tabstrip") {
      await this.commands.dispatch({
        id: "workbench.movePane",
        args: { paneId: draggedPaneId, placement: { kind: "tab", group: currentTarget.groupId, index: currentTarget.index } },
      });
      return;
    }
    await this.commands.dispatch({
      id: "workbench.movePane",
      args: { paneId: draggedPaneId, placement: { kind: "split", direction: currentTarget.direction, of: currentTarget.groupId } },
    });
  }

  private set(next: WorkbenchDragSnapshot): void {
    this.state = next;
    for (const listener of this.listeners) listener();
  }
}

function sameTarget(a: WorkbenchDragTarget | undefined, b: WorkbenchDragTarget | undefined): boolean {
  if (!a || !b) return a === b;
  if (a.kind !== b.kind || a.groupId !== b.groupId) return false;
  return a.kind === "tabstrip" ? b.kind === "tabstrip" && a.index === b.index : b.kind === "split" && a.direction === b.direction;
}
