import type { RuntimeApi } from "@the-editor/contracts/runtime";
import { CommandRegistry } from "./commands.js";
import { ContributionRegistry, moduleId, type ModuleId } from "./contributions.js";
import { DisposableStore } from "./disposable.js";
import { Hooks } from "./hooks.js";
import { KeymapRegistry } from "./keymaps.js";
import { ModuleRegistry, type EditorModule, type EditorModuleRegistration } from "./modules.js";

export interface EditorHostOptions {
  runtime: RuntimeApi;
}

const anonymousModuleId = moduleId("<anonymous>");

export class EditorHost {
  readonly runtime: RuntimeApi;
  readonly hooks = new Hooks();
  readonly contributions = new ContributionRegistry();
  readonly commands = new CommandRegistry(this);
  readonly keymaps = new KeymapRegistry(this);
  readonly modules = new ModuleRegistry(this);

  private readonly disposables = new DisposableStore();
  private moduleStack: ModuleId[] = [];

  constructor(options: EditorHostOptions) {
    this.runtime = options.runtime;
  }

  async use(module: EditorModule | EditorModuleRegistration, id?: string): Promise<void> {
    if (typeof module === "function") {
      const moduleRegistration = {
        id: id ?? (module.name || "<anonymous>"),
        activate: module,
      };
      await this.modules.load(moduleRegistration);
      return;
    }

    await this.modules.load(module);
  }

  currentModuleId(): ModuleId {
    return this.moduleStack.at(-1) ?? anonymousModuleId;
  }

  async withModule<T>(id: ModuleId, fn: () => T | Promise<T>): Promise<T> {
    this.moduleStack.push(id);
    try {
      return await fn();
    } finally {
      this.moduleStack.pop();
    }
  }

  dispose(): void {
    for (const id of this.modules.all().reverse()) {
      this.modules.unload(id);
    }
    this.disposables.dispose();
  }
}

export function createEditorHost(options: EditorHostOptions): EditorHost {
  return new EditorHost(options);
}

export type { EditorModule, EditorModuleRegistration };
