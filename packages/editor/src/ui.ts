import { contributionKind, moduleId, type ModuleId } from "./contributions.js";
import type { BufferDecoration } from "./decorations.js";
import type { DiagnosticRecord } from "./diagnostics.js";
import { toDisposable, type Disposable } from "./disposable.js";
import type { HealthDiagnosticEntry } from "./health.js";
import type { FocusTarget } from "./focus.js";
import type { GutterSnapshot } from "./gutters.js";
import type { CommandDispatch } from "./commands.js";
import type { InteractionSnapshot } from "./interaction.js";
import type { NotificationEntry } from "./notifications.js";
import type { PackageSnapshot } from "./packages.js";
import { roleId, roleImplementationId, type RoleId, type RoleImplementationId, type RoleSnapshot } from "./roles.js";
import type { InteractiveSessionSnapshot } from "./sessions.js";
import type { ThemeSnapshot } from "./themes.js";
import type { ToolRootTarget, ToolSnapshot, ToolTypeId } from "./tools.js";
import type { WorkingDirectorySnapshot } from "./workingDirectory.js";
import type { WorkbenchEditorLayoutSnapshot, WorkbenchGroupSnapshot, WorkbenchItemDescriptor, WorkbenchPaneDescriptor } from "./workbench.js";

export interface UiCommandDispatcher {
  dispatch(dispatch: CommandDispatch): Promise<unknown>;
}

export interface WorkbenchSnapshot {
  mode?: string;
  activePath?: string;
  roles: Record<string, RoleSnapshot>;
  packages: PackageSnapshot[];
  diagnostics?: DiagnosticRecord[];
  healthDiagnostics?: HealthDiagnosticEntry[];
  interaction?: InteractionSnapshot;
  tools?: ToolSnapshot[];
  decorations?: BufferDecoration[];
  gutters?: GutterSnapshot[];
  focus?: FocusTarget;
  theme?: ThemeSnapshot;
  syntaxLegend?: Record<number, string>;
  sessions?: InteractiveSessionSnapshot[];
  notifications?: NotificationEntry[];
  workingDirectory?: WorkingDirectorySnapshot;
  editorLayout?: WorkbenchEditorLayoutSnapshot;
}

export interface UiSessionClient {
  all(): InteractiveSessionSnapshot[];
  get<TState = unknown>(id: string | undefined): InteractiveSessionSnapshot<TState> | undefined;
  subscribe(id: string, listener: () => void): () => void;
  dispatch(id: string, method: string, args?: unknown): Promise<unknown>;
}

export interface RoleViewContext<TState = unknown> {
  commands: UiCommandDispatcher;
  sessions: UiSessionClient;
  role: RoleId;
  snapshot: RoleSnapshot | undefined;
  state: TState | undefined;
  workbench: WorkbenchSnapshot;
}

export interface RoleViewRegistration<TState = unknown> {
  role: RoleId | string;
  implementationId: RoleImplementationId | string;
  render(ctx: RoleViewContext<TState>): unknown;
}

export interface ToolViewContext<TState = unknown> {
  commands: UiCommandDispatcher;
  sessions: UiSessionClient;
  tool: ToolSnapshot;
  state: TState | undefined;
  workbench: WorkbenchSnapshot;
}

export interface ToolViewRegistration<TState = unknown> {
  typeId: ToolTypeId | string;
  render(ctx: ToolViewContext<TState>): unknown;
}

export interface WorkbenchTabstripViewContext {
  commands: UiCommandDispatcher;
  sessions: UiSessionClient;
  workbench: WorkbenchSnapshot;
  group: WorkbenchGroupSnapshot;
  items: WorkbenchItemDescriptor[];
  panes: WorkbenchPaneDescriptor[];
  activePaneId?: string;
  showWindowControls?: boolean;
}

export interface WorkbenchTabstripViewRegistration {
  implementationId: RoleImplementationId | string;
  render(ctx: WorkbenchTabstripViewContext): unknown;
}

interface RegisteredRoleView extends RoleViewRegistration {
  moduleId: ModuleId;
}

interface RegisteredToolView extends ToolViewRegistration {
  moduleId: ModuleId;
}

interface RegisteredWorkbenchTabstripView extends WorkbenchTabstripViewRegistration {
  implementationId: RoleImplementationId;
  moduleId: ModuleId;
}

export interface FileTreeUiContribution {
  id: string;
  options?: Record<string, unknown>;
  renderContextMenu?: (item: unknown, context: unknown) => unknown;
  renderRowDecoration?: (context: unknown) => unknown;
}

interface RegisteredFileTreeUiContribution extends FileTreeUiContribution {
  moduleId: ModuleId;
}

export type EditorUiModule = (host: UiHost) => void | Disposable | Promise<void | Disposable>;

export class RoleViewRegistry {
  private readonly entries = new Map<RoleId, Map<RoleImplementationId, RegisteredRoleView>>();

  constructor(private readonly host: UiHost) {}

  register<TState>(view: RoleViewRegistration<TState>, moduleId = this.host.currentModuleId()): Disposable {
    const role = roleId(view.role);
    const implementationId = roleImplementationId(view.implementationId);
    const entry: RegisteredRoleView = {
      ...view,
      role,
      implementationId,
      moduleId,
    };

    const implementations = this.entries.get(role) ?? new Map<RoleImplementationId, RegisteredRoleView>();
    implementations.set(implementationId, entry);
    this.entries.set(role, implementations);

    const unregister = toDisposable(() => {
      const current = this.entries.get(role);
      if (!current) return;
      const candidate = current.get(implementationId);
      if (candidate !== entry) return;
      current.delete(implementationId);
      if (!current.size) this.entries.delete(role);
    });

    return this.host.addContribution({
      moduleId,
      kind: contributionKind("roleView"),
      localId: `${role}:${implementationId}`,
      disposable: unregister,
    });
  }

  render(role: RoleId | string, workbench = this.host.snapshot()): unknown {
    const normalizedRole = roleId(role);
    const snapshot = workbench.roles[normalizedRole];
    const implementations = this.entries.get(normalizedRole);
    if (!implementations?.size) return null;

    const implementation = snapshot?.implementationId
      ? implementations.get(roleImplementationId(snapshot.implementationId))
      : implementations.values().next().value;
    if (!implementation) return null;

    return implementation.render({
      commands: this.host.commands,
      sessions: this.host.sessions,
      role: normalizedRole,
      snapshot,
      state: snapshot?.state,
      workbench,
    });
  }
}

export class ToolViewRegistry {
  private readonly entries = new Map<ToolTypeId, RegisteredToolView>();

  constructor(private readonly host: UiHost) {}

  register<TState>(view: ToolViewRegistration<TState>, moduleId = this.host.currentModuleId()): Disposable {
    const typeId = view.typeId as ToolTypeId;
    const entry: RegisteredToolView = {
      ...view,
      typeId,
      moduleId,
    };
    this.entries.set(typeId, entry);

    const unregister = toDisposable(() => {
      const current = this.entries.get(typeId);
      if (current === entry) this.entries.delete(typeId);
    });

    return this.host.addContribution({
      moduleId,
      kind: contributionKind("toolView"),
      localId: typeId,
      disposable: unregister,
    });
  }

  renderTool(tool: ToolSnapshot, workbench = this.host.snapshot()): unknown {
    const view = this.entries.get(tool.typeId);
    if (!view) return null;
    return view.render({
      commands: this.host.commands,
      sessions: this.host.sessions,
      tool,
      state: tool.state,
      workbench,
    });
  }

  renderRoot(root: ToolRootTarget, workbench = this.host.snapshot()): unknown[] {
    return this.toolsForRoot(root, workbench)
      .map((tool) => this.renderTool(tool, workbench))
      .filter((node) => node !== null && node !== undefined);
  }

  toolsForRoot(root: ToolRootTarget, workbench = this.host.snapshot()): ToolSnapshot[] {
    return (workbench.tools ?? []).filter((tool) => tool.root === root);
  }
}

export class WorkbenchTabstripRegistry {
  private readonly entries = new Map<RoleImplementationId, RegisteredWorkbenchTabstripView>();

  constructor(private readonly host: UiHost) {}

  register(view: WorkbenchTabstripViewRegistration, moduleId = this.host.currentModuleId()): Disposable {
    const implementationId = roleImplementationId(view.implementationId);
    const entry: RegisteredWorkbenchTabstripView = {
      ...view,
      implementationId,
      moduleId,
    };
    this.entries.set(implementationId, entry);

    const unregister = toDisposable(() => {
      const current = this.entries.get(implementationId);
      if (current === entry) this.entries.delete(implementationId);
    });

    return this.host.addContribution({
      moduleId,
      kind: contributionKind("tabstripView"),
      localId: implementationId,
      disposable: unregister,
    });
  }

  render(input: Omit<WorkbenchTabstripViewContext, "commands" | "sessions" | "workbench">, workbench = this.host.snapshot()): unknown {
    const snapshot = workbench.roles[roleId("tabstrip")];
    const implementation = snapshot?.implementationId
      ? this.entries.get(roleImplementationId(snapshot.implementationId))
      : this.entries.values().next().value;
    if (!implementation) return null;
    return implementation.render({
      commands: this.host.commands,
      sessions: this.host.sessions,
      workbench,
      ...input,
    });
  }
}

export class FileTreeUiRegistry {
  private readonly entries = new Map<string, RegisteredFileTreeUiContribution>();

  constructor(private readonly host: UiHost) {}

  register(contribution: FileTreeUiContribution, moduleId = this.host.currentModuleId()): Disposable {
    const entry: RegisteredFileTreeUiContribution = { ...contribution, moduleId };
    this.entries.set(contribution.id, entry);
    const unregister = toDisposable(() => {
      if (this.entries.get(contribution.id) === entry) this.entries.delete(contribution.id);
    });
    return this.host.addContribution({ moduleId, kind: contributionKind("fileTreeUi"), localId: contribution.id, disposable: unregister });
  }

  all(): readonly FileTreeUiContribution[] {
    return [...this.entries.values()];
  }
}

const emptyUiSessionClient: UiSessionClient = {
  all: () => [],
  get: () => undefined,
  subscribe: () => () => {},
  dispatch: async () => undefined,
};

export class UiHost {
  readonly roles = new RoleViewRegistry(this);
  readonly tools = new ToolViewRegistry(this);
  readonly tabstrips = new WorkbenchTabstripRegistry(this);
  readonly fileTrees = new FileTreeUiRegistry(this);

  private readonly moduleStack: ModuleId[] = [];
  private readonly contributions = new Map<string, Disposable>();
  private workbench: WorkbenchSnapshot = {
    roles: {},
    packages: [],
    diagnostics: [],
    tools: [],
    decorations: [],
    focus: { kind: "editor" },
    theme: { available: [] },
    syntaxLegend: {},
    sessions: [],
    notifications: [],
    workingDirectory: undefined,
  };

  constructor(readonly commands: UiCommandDispatcher, readonly sessions: UiSessionClient = emptyUiSessionClient) {}

  update(snapshot: WorkbenchSnapshot): void {
    this.workbench = snapshot;
  }

  snapshot(): WorkbenchSnapshot {
    return this.workbench;
  }

  currentModuleId(): ModuleId {
    return this.moduleStack.at(-1) ?? moduleId("<ui>");
  }

  async withModule<T>(id: ModuleId | string, fn: () => T | Promise<T>): Promise<T> {
    this.moduleStack.push(moduleId(typeof id === "string" ? id : id));
    try {
      return await fn();
    } finally {
      this.moduleStack.pop();
    }
  }

  unloadModule(id: ModuleId | string): void {
    const normalized = moduleId(typeof id === "string" ? id : id);
    for (const [key, disposable] of [...this.contributions.entries()]) {
      if (!key.startsWith(`${normalized}:`)) continue;
      this.contributions.delete(key);
      disposable.dispose();
    }
  }

  dispose(): void {
    for (const disposable of [...this.contributions.values()].reverse()) {
      disposable.dispose();
    }
    this.contributions.clear();
  }

  addContribution(input: { moduleId: ModuleId; kind: string; localId: string; disposable: Disposable }): Disposable {
    const key = `${input.moduleId}:${input.kind}:${input.localId}`;
    const previous = this.contributions.get(key);
    previous?.dispose();
    this.contributions.set(key, input.disposable);
    return toDisposable(() => {
      const current = this.contributions.get(key);
      if (current !== input.disposable) return;
      this.contributions.delete(key);
      input.disposable.dispose();
    });
  }
}

export function createUiHost(commands: UiCommandDispatcher, sessions?: UiSessionClient): UiHost {
  return new UiHost(commands, sessions);
}
