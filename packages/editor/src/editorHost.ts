import type { RuntimeApi } from "@the-editor/contracts/runtime";
import { CommandRegistry } from "./commands.js";
import { ContributionRegistry, moduleId, type ModuleId } from "./contributions.js";
import { DecorationRegistry } from "./decorations.js";
import { DiagnosticRegistry } from "./diagnostics.js";
import { DisposableStore } from "./disposable.js";
import { FocusRegistry } from "./focus.js";
import { GutterRegistry } from "./gutters.js";
import { Hooks } from "./hooks.js";
import { InteractionRegistry } from "./interaction.js";
import { KeymapRegistry } from "./keymaps.js";
import { ModuleRegistry, type EditorModule, type EditorModuleRegistration } from "./modules.js";
import { NotificationRegistry } from "./notifications.js";
import { PackageRegistry, type PackageTargetAdapterMap } from "./packages.js";
import { CommandPaletteRegistry } from "./palette.js";
import { PickerRegistry } from "./pickers.js";
import { RoleRegistry } from "./roles.js";
import { InteractiveSessionRegistry } from "./sessions.js";
import { ThemeRegistry } from "./themes.js";
import { ToolRegistry } from "./tools.js";
import { WorkingDirectoryRegistry } from "./workingDirectory.js";
import { WorkbenchController } from "./workbench.js";

export interface EditorHostOptions {
  runtime: RuntimeApi;
  cwd?: string;
  packageTargetAdapters?: PackageTargetAdapterMap;
}

const anonymousModuleId = moduleId("<anonymous>");

export class EditorHost {
  readonly runtime: RuntimeApi;
  readonly hooks = new Hooks();
  readonly contributions = new ContributionRegistry();
  readonly diagnostics = new DiagnosticRegistry();
  readonly notifications = new NotificationRegistry(this);
  readonly commands = new CommandRegistry(this);
  readonly keymaps = new KeymapRegistry(this);
  readonly modules = new ModuleRegistry(this);
  readonly packages: PackageRegistry;
  readonly roles = new RoleRegistry(this);
  readonly palette = new CommandPaletteRegistry(this);
  readonly pickers = new PickerRegistry(this);
  readonly tools = new ToolRegistry(this);
  readonly interaction = new InteractionRegistry(this);
  readonly decorations = new DecorationRegistry(this);
  readonly gutters = new GutterRegistry(this);
  readonly focus = new FocusRegistry(this);
  readonly themes = new ThemeRegistry(this);
  readonly sessions = new InteractiveSessionRegistry(this);
  readonly cwd: WorkingDirectoryRegistry;
  readonly workbench = new WorkbenchController(this);

  private readonly disposables = new DisposableStore();
  private readonly stateSlots = new Map<string, unknown>();
  private moduleStack: ModuleId[] = [];

  constructor(options: EditorHostOptions) {
    this.runtime = options.runtime;
    this.packages = new PackageRegistry(this, options.packageTargetAdapters);
    this.cwd = new WorkingDirectoryRegistry(this, options.cwd);
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

  stateSlot<T>(key: string, init: () => T): T {
    if (this.stateSlots.has(key)) return this.stateSlots.get(key) as T;
    const value = init();
    this.stateSlots.set(key, value);
    return value;
  }

  getStateSlot<T>(key: string): T | undefined {
    return this.stateSlots.get(key) as T | undefined;
  }

  setStateSlot<T>(key: string, value: T): T {
    this.stateSlots.set(key, value);
    return value;
  }

  deleteStateSlot(key: string): void {
    this.stateSlots.delete(key);
  }

  dispose(): void {
    for (const id of this.modules.all().reverse()) {
      this.modules.unload(id);
    }
    this.notifications.dispose();
    this.stateSlots.clear();
    this.disposables.dispose();
  }
}

export function createEditorHost(options: EditorHostOptions): EditorHost {
  return new EditorHost(options);
}

export type { EditorModule, EditorModuleRegistration };
