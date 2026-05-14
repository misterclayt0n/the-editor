import { moduleId, type ModuleId } from "./contributions.js";
import type { EditorHost } from "./editorHost.js";
import { DisposableStore, type Disposable } from "./disposable.js";

export type EditorModule = (editor: EditorHost) => void | Disposable | Promise<void | Disposable>;

export interface EditorModuleRegistration {
  id: string;
  activate: EditorModule;
}

interface LoadedModule {
  id: ModuleId;
  disposable: DisposableStore;
}

export class ModuleRegistry {
  private readonly loaded = new Map<ModuleId, LoadedModule>();

  constructor(private readonly editor: EditorHost) {}

  async load(registration: EditorModuleRegistration): Promise<void> {
    const id = moduleId(registration.id);
    this.unload(id);

    const disposable = new DisposableStore();
    this.loaded.set(id, { id, disposable });

    await this.editor.withModule(id, async () => {
      const result = await registration.activate(this.editor);
      if (result) disposable.add(result);
    });
  }

  unload(id: ModuleId | string): void {
    const normalized = typeof id === "string" ? moduleId(id) : id;
    const loaded = this.loaded.get(normalized);
    if (!loaded) return;
    this.loaded.delete(normalized);
    loaded.disposable.dispose();
    this.editor.contributions.disposeModule(normalized);
    this.editor.stores.disposeModule(normalized);
  }

  all(): ModuleId[] {
    return [...this.loaded.keys()];
  }
}
