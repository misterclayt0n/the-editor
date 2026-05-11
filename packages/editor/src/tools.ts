import { contributionKind } from "./contributions.js";
import type { EditorHost } from "./editorHost.js";
import { toDisposable, type Disposable } from "./disposable.js";

export type ToolTypeId = string & { readonly __brand?: "ToolTypeId" };
export type ToolInstanceId = string & { readonly __brand?: "ToolInstanceId" };
export type ToolRootTarget = "editor" | "leftSidebar" | "rightSidebar" | "bottomPanel" | "overlayModal" | "overlayFloating";
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

export interface ToolOpenOptions {
  title?: string;
  input?: unknown;
  root?: ToolRootTarget;
  focus?: boolean;
}

export interface ToolInstance {
  id: ToolInstanceId;
  typeId: ToolTypeId;
  title: string;
  root: ToolRootTarget;
  closeable: boolean;
  persistenceKey?: string;
  packageId?: string;
  input?: unknown;
}

export interface ToolSnapshot extends ToolInstance {
  state?: unknown;
}

interface ToolTypeEntry extends ToolTypeRegistration {
  id: ToolTypeId;
}

export class ToolRegistry {
  private nextInstance = 0;
  private readonly types = new Map<ToolTypeId, ToolTypeEntry>();
  private readonly instances = new Map<ToolInstanceId, ToolInstance>();

  constructor(private readonly editor: EditorHost) {}

  registerType(registration: ToolTypeRegistration, moduleId = this.editor.currentModuleId()): Disposable {
    const id = toolTypeId(registration.id);
    this.types.set(id, {
      ...registration,
      id,
      packageId: registration.packageId ?? packageIdFromModuleId(moduleId),
    });

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

  getType(typeId: ToolTypeId | string): ToolTypeRegistration | undefined {
    return this.types.get(toolTypeId(typeId));
  }

  getInstance(id: ToolInstanceId | string): ToolInstance | undefined {
    return this.instances.get(toolInstanceId(id));
  }

  findByType(typeId: ToolTypeId | string): ToolInstance | undefined {
    return [...this.instances.values()].find((instance) => instance.typeId === toolTypeId(typeId));
  }

  open(typeId: ToolTypeId | string, titleOrOptions?: string | ToolOpenOptions): ToolInstance {
    const type = this.require(typeId);
    const options = normalizeToolOpenOptions(titleOrOptions);

    if (type.instancePolicy !== "multiInstance") {
      const existing = this.findByType(type.id);
      if (existing) {
        if (options.focus !== false) this.focus(existing.id);
        return existing;
      }
    }

    const instance = this.createInstance(type, options);
    this.instances.set(instance.id, instance);
    if (options.focus !== false && instance.root !== "editor") {
      this.editor.focus.setTool(instance.id);
    }
    return instance;
  }

  close(id: ToolInstanceId | string): void {
    const normalized = toolInstanceId(id);
    const instance = this.instances.get(normalized);
    if (!instance) return;
    const currentFocus = this.editor.focus.current();
    if (instance.root === "editor") {
      this.editor.workbench.closePanesForTool(normalized);
      this.instances.delete(normalized);
      return;
    }
    this.instances.delete(normalized);
    if (currentFocus.kind === "tool" && currentFocus.toolId === normalized) {
      this.editor.focus.restore();
    }
  }

  focus(id: ToolInstanceId | string): void {
    const instance = this.instances.get(toolInstanceId(id));
    if (!instance) return;
    const pane = instance.root === "editor" ? this.editor.workbench.findPaneForTool(instance.id) : undefined;
    if (pane) {
      this.editor.workbench.focusPane(pane.id);
      return;
    }
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
        const existing = this.findByType(type.id);
        if (existing) continue;
      }
      const instance = this.createInstance(type, {
        title: snapshot.title,
        input: snapshot.input,
        root: snapshot.root,
        focus: false,
      }, snapshot.id);
      this.instances.set(instance.id, instance);
    }
  }

  private createInstance(
    type: ToolTypeEntry,
    options: ToolOpenOptions,
    id = toolInstanceId(`${type.id}:${this.nextInstance++}`),
  ): ToolInstance {
    return {
      id,
      typeId: type.id,
      title: options.title ?? type.title,
      root: options.root ?? type.root,
      closeable: type.closeable ?? true,
      persistenceKey: type.persistenceKey,
      packageId: type.packageId,
      input: options.input,
    };
  }

  private require(typeId: ToolTypeId | string): ToolTypeEntry {
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

function normalizeToolOpenOptions(titleOrOptions: string | ToolOpenOptions | undefined): ToolOpenOptions {
  if (typeof titleOrOptions === "string") return { title: titleOrOptions };
  return titleOrOptions ?? {};
}

function packageIdFromModuleId(id: string): string | undefined {
  for (const suffix of ["/runtime", "/interaction", "/ui", "/theme"]) {
    if (id.endsWith(suffix)) return id.slice(0, -suffix.length);
  }
  return undefined;
}
