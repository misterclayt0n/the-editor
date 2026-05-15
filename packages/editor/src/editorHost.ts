import type { RuntimeApi } from "@the-editor/contracts/runtime";
import { CommandRegistry } from "./commands.js";
import { CompletionRegistry } from "./completion.js";
import { ContributionRegistry, moduleId, type ModuleId } from "./contributions.js";
import { DecorationRegistry } from "./decorations.js";
import { DiagnosticStore } from "./diagnostics.js";
import { DisposableStore } from "./disposable.js";
import { HealthDiagnosticRegistry } from "./health.js";
import { InputRouter } from "./input.js";
import { GutterRegistry } from "./gutters.js";
import { Hooks } from "./hooks.js";
import { InteractionRegistry } from "./interaction.js";
import { KeymapRegistry } from "./keymaps.js";
import { ModuleRegistry, type EditorModule, type EditorModuleRegistration } from "./modules.js";
import { NotificationRegistry } from "./notifications.js";
import { OverlayRegistry } from "./overlays.js";
import { PackageRegistry, type PackageTargetAdapterMap } from "./packages.js";
import { CommandPaletteRegistry } from "./palette.js";
import { PickerRegistry } from "./pickers.js";
import { RoleRegistry } from "./roles.js";
import { InteractiveSessionRegistry } from "./sessions.js";
import { StoreRegistry } from "./stores.js";
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
  readonly diagnostics = new DiagnosticStore(this);
  readonly health = new HealthDiagnosticRegistry();
  readonly notifications = new NotificationRegistry(this);
  readonly overlays = new OverlayRegistry(this);
  readonly commands = new CommandRegistry(this);
  readonly completion = new CompletionRegistry(this);
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
  readonly input = new InputRouter(this);
  readonly themes = new ThemeRegistry(this);
  readonly sessions = new InteractiveSessionRegistry(this);
  readonly stores = new StoreRegistry(() => this.currentModuleId());
  readonly cwd: WorkingDirectoryRegistry;
  readonly workbench = new WorkbenchController(this);

  private readonly disposables = new DisposableStore();
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

  dispose(): void {
    for (const id of this.modules.all().reverse()) {
      this.modules.unload(id);
    }
    this.notifications.dispose();
    this.stores.clear();
    this.disposables.dispose();
  }
}

export function createEditorHost(options: EditorHostOptions): EditorHost {
  return new EditorHost(options);
}

export type { EditorModule, EditorModuleRegistration };
