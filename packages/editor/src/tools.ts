import { contributionKind } from "./contributions.js";
import type { EditorHost } from "./editorHost.js";
import { toDisposable, type Disposable } from "./disposable.js";

export type ToolTypeId = string & { readonly __brand?: "ToolTypeId" };
export type ToolInstanceId = string & { readonly __brand?: "ToolInstanceId" };
export type ToolRootTarget = "leftSidebar" | "rightSidebar" | "bottomPanel" | "overlayModal" | "overlayFloating" | "editorAdjacent";
export type ToolInstancePolicy = "singleton" | "singletonPerWorkspace" | "multiInstance";

export interface ToolTypeRegistration {
  id: ToolTypeId | string;
  title: string;
  icon?: string;
  root: ToolRootTarget;
  instancePolicy?: ToolInstancePolicy;
  closeable?: boolean;
  persistenceKey?: string;
  packageId?: string;
  snapshot?(ctx: { editor: EditorHost; tool: ToolInstance }): unknown;
}

export interface ToolInstance {
  id: ToolInstanceId;
  typeId: ToolTypeId;
  title: string;
  root: ToolRootTarget;
  closeable: boolean;
  persistenceKey?: string;
  packageId?: string;
}

export interface ToolSnapshot extends ToolInstance {
  state?: unknown;
}

export class ToolRegistry {
  private nextInstance = 0;
  private readonly types = new Map<ToolTypeId, ToolTypeRegistration>();
  private readonly instances = new Map<ToolInstanceId, ToolInstance>();

  constructor(private readonly editor: EditorHost) {}

  registerType(registration: ToolTypeRegistration, moduleId = this.editor.currentModuleId()): Disposable {
    const id = toolTypeId(registration.id);
    this.types.set(id, { ...registration, id, packageId: registration.packageId ?? packageIdFromModuleId(moduleId) });

    const unregister = toDisposable(() => {
      const current = this.types.get(id);
      if (current?.id === id) this.types.delete(id);
      const currentFocus = this.editor.focus.current();
      const removedFocusedTool = currentFocus.kind === "tool"
        && [...this.instances.values()].some((instance) =>
          instance.typeId === id && currentFocus.toolId === instance.id,
        );
      for (const instance of [...this.instances.values()]) {
        if (instance.typeId !== id) continue;
        this.instances.delete(instance.id);
      }
      if (removedFocusedTool) this.editor.focus.restore();
    });

    return this.editor.contributions.add({
      moduleId,
      kind: contributionKind("toolType"),
      localId: id,
      label: registration.title,
      disposable: unregister,
    });
  }

  open(typeId: ToolTypeId | string, title?: string): ToolInstance {
    const type = this.require(typeId);
    if (type.instancePolicy !== "multiInstance") {
      const existing = [...this.instances.values()].find((instance) => instance.typeId === type.id);
      if (existing) {
        this.editor.focus.setTool(existing.id);
        return existing;
      }
    }

    const instance = this.createInstance(type, title);
    this.instances.set(instance.id, instance);
    this.editor.focus.setTool(instance.id);
    return instance;
  }

  close(id: ToolInstanceId | string): void {
    const normalized = toolInstanceId(id);
    const currentFocus = this.editor.focus.current();
    this.instances.delete(normalized);
    if (currentFocus.kind === "tool" && currentFocus.toolId === normalized) {
      this.editor.focus.restore();
    }
  }

  focus(id: ToolInstanceId | string): void {
    const instance = this.instances.get(toolInstanceId(id));
    if (!instance) return;
    this.editor.focus.setTool(instance.id);
  }

  allTypes(): ToolTypeRegistration[] {
    return [...this.types.values()].sort((a, b) => a.title.localeCompare(b.title));
  }

  allInstances(): ToolInstance[] {
    return [...this.instances.values()];
  }

  snapshotAll(): ToolSnapshot[] {
    return this.allInstances().map((tool) => ({
      ...tool,
      state: this.types.get(tool.typeId)?.snapshot?.({ editor: this.editor, tool }),
    }));
  }

  capturePackageInstances(packageId: string): ToolSnapshot[] {
    return this.snapshotAll().filter((tool) => tool.packageId === packageId);
  }

  restoreSnapshots(snapshots: readonly ToolSnapshot[]): void {
    for (const snapshot of snapshots) {
      const type = this.types.get(snapshot.typeId);
      if (!type) continue;
      if (type.instancePolicy !== "multiInstance") {
        const existing = [...this.instances.values()].find((instance) => instance.typeId === type.id);
        if (existing) continue;
      }
      const instance = this.createInstance(type, snapshot.title);
      this.instances.set(instance.id, instance);
    }
  }

  private createInstance(type: ToolTypeRegistration, title?: string): ToolInstance {
    return {
      id: toolInstanceId(`${type.id}:${this.nextInstance++}`),
      typeId: type.id as ToolTypeId,
      title: title ?? type.title,
      root: type.root,
      closeable: type.closeable ?? true,
      persistenceKey: type.persistenceKey,
      packageId: type.packageId,
    };
  }

  private require(typeId: ToolTypeId | string): ToolTypeRegistration {
    const type = this.types.get(toolTypeId(typeId));
    if (!type) throw new Error(`unknown tool type: ${typeId}`);
    return type;
  }
}

export function toolTypeId(id: ToolTypeId | string): ToolTypeId {
  return id as ToolTypeId;
}

export function toolInstanceId(id: ToolInstanceId | string): ToolInstanceId {
  return id as ToolInstanceId;
}

function packageIdFromModuleId(id: string): string | undefined {
  for (const suffix of ["/runtime", "/interaction", "/ui", "/theme"]) {
    if (id.endsWith(suffix)) return id.slice(0, -suffix.length);
  }
  return undefined;
}
