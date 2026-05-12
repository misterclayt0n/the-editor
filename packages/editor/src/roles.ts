import { contributionKind, type ModuleId } from "./contributions.js";
import type { EditorHost } from "./editorHost.js";
import { toDisposable, type Disposable } from "./disposable.js";

export type RoleId = string & { readonly __brand?: "RoleId" };
export type RoleImplementationId = string & { readonly __brand?: "RoleImplementationId" };

export interface RoleContext {
  editor: EditorHost;
}

export interface RoleImplementation<TState = unknown> {
  role: RoleId | string;
  implementationId: RoleImplementationId | string;
  title?: string;
  priority?: number;
  open?(ctx: RoleContext, args?: unknown): void | Promise<void>;
  close?(ctx: RoleContext): void | Promise<void>;
  focus?(ctx: RoleContext): void | Promise<void>;
  snapshot?(ctx: RoleContext): TState | undefined;
}

interface RoleEntry<TState = unknown> extends RoleImplementation<TState> {
  moduleId: ModuleId;
  order: number;
  packageId?: string;
}

export interface RoleImplementationSnapshot {
  implementationId: RoleImplementationId;
  title?: string;
  moduleId: ModuleId;
  packageId?: string;
}

export interface RoleSnapshot {
  implementationId?: RoleImplementationId;
  selectedImplementationId?: RoleImplementationId;
  selectionSource: "explicit" | "implicit" | "fallback" | "none";
  title?: string;
  moduleId?: ModuleId;
  packageId?: string;
  fallbackReason?: string;
  state?: unknown;
  available: RoleImplementationSnapshot[];
}

const roleSelectionsSlot = "roles.selections";
const roleFallbackImplementationsSlot = "roles.fallbackImplementations";
const roleFallbackReasonsSlot = "roles.fallbackReasons";

export class RoleRegistry {
  private nextOrder = 0;
  private readonly entries = new Map<RoleId, RoleEntry[]>();
  private readonly active = new Map<RoleId, RoleImplementationId>();

  constructor(private readonly editor: EditorHost) {}

  register<TState>(implementation: RoleImplementation<TState>, moduleId = this.editor.currentModuleId()): Disposable {
    const role = roleId(implementation.role);
    const implementationId = roleImplementationId(implementation.implementationId);
    const entry: RoleEntry<TState> = {
      ...implementation,
      role,
      implementationId,
      moduleId,
      order: this.nextOrder++,
      packageId: packageIdFromModuleId(moduleId),
    };

    const list = this.entries.get(role) ?? [];
    list.push(entry as RoleEntry);
    list.sort((a, b) => a.order - b.order);
    this.entries.set(role, list);
    this.reconcile(role);

    const unregister = toDisposable(() => {
      const current = this.entries.get(role);
      if (!current) return;
      const next = current.filter((candidate) => candidate !== entry);
      if (next.length) this.entries.set(role, next);
      else this.entries.delete(role);
      this.reconcile(role);
    });

    return this.editor.contributions.add({
      moduleId,
      kind: contributionKind("role"),
      localId: `${role}:${implementationId}`,
      label: implementation.title,
      disposable: unregister,
    });
  }

  all(role?: RoleId | string): RoleImplementation[] {
    if (!role) return [...this.entries.values()].flat();
    return [...(this.entries.get(roleId(role)) ?? [])];
  }

  selectedImplementationId(role: RoleId | string): RoleImplementationId | undefined {
    return this.selections().get(roleId(role));
  }

  activeImplementation(role: RoleId | string): RoleImplementation | undefined {
    const normalizedRole = roleId(role);
    const activeId = this.active.get(normalizedRole);
    const list = this.entries.get(normalizedRole) ?? [];
    if (!list.length) return undefined;
    if (!activeId) return list[0];
    return list.find((entry) => entry.implementationId === activeId) ?? list[0];
  }

  activate(role: RoleId | string, implementationId: RoleImplementationId | string): void {
    const normalizedRole = roleId(role);
    const normalizedId = roleImplementationId(implementationId);
    this.selections().set(normalizedRole, normalizedId);
    this.reconcile(normalizedRole, `selected ${normalizedId}`);
  }

  setFallback(role: RoleId | string, implementationId: RoleImplementationId | string): void {
    this.fallbackImplementations().set(roleId(role), roleImplementationId(implementationId));
    this.reconcile(roleId(role));
  }

  clearFallback(role: RoleId | string): void {
    this.fallbackImplementations().delete(roleId(role));
    this.reconcile(roleId(role));
  }

  clearSelection(role: RoleId | string): void {
    const normalizedRole = roleId(role);
    this.selections().delete(normalizedRole);
    this.reconcile(normalizedRole);
  }

  async open(role: RoleId | string, args?: unknown): Promise<void> {
    const implementation = this.activeImplementation(role);
    if (!implementation?.open) return;
    await implementation.open({ editor: this.editor }, args);
  }

  async close(role: RoleId | string): Promise<void> {
    const implementation = this.activeImplementation(role);
    if (!implementation?.close) return;
    await implementation.close({ editor: this.editor });
  }

  async focus(role: RoleId | string): Promise<void> {
    const implementation = this.activeImplementation(role);
    if (!implementation?.focus) return;
    await implementation.focus({ editor: this.editor });
  }

  snapshot(role: RoleId | string): RoleSnapshot | undefined {
    const normalizedRole = roleId(role);
    const implementations = this.entries.get(normalizedRole) ?? [];
    const active = this.activeImplementation(normalizedRole) as RoleEntry | undefined;
    const selected = this.selectedImplementationId(normalizedRole);
    const fallbackReason = this.fallbackReasons().get(normalizedRole);

    if (!implementations.length && !selected) return undefined;

    return {
      implementationId: active?.implementationId,
      selectedImplementationId: selected,
      selectionSource: selected
        ? (active?.implementationId === selected ? "explicit" : "fallback")
        : (active ? "implicit" : "none"),
      title: active?.title,
      moduleId: active?.moduleId,
      packageId: active?.packageId,
      fallbackReason,
      state: active?.snapshot?.({ editor: this.editor }),
      available: implementations.map((entry) => ({
        implementationId: roleImplementationId(entry.implementationId),
        title: entry.title,
        moduleId: entry.moduleId,
        packageId: entry.packageId,
      })),
    };
  }

  snapshotAll(): Record<string, RoleSnapshot> {
    const snapshots: Record<string, RoleSnapshot> = {};
    const roles = new Set<RoleId>([...this.entries.keys(), ...this.selections().keys()]);
    for (const role of roles) {
      const snapshot = this.snapshot(role);
      if (snapshot) snapshots[role] = snapshot;
    }
    return snapshots;
  }

  private reconcile(role: RoleId, fallbackReason?: string): void {
    const list = this.entries.get(role) ?? [];
    const previousActive = this.active.get(role);
    const selected = this.selections().get(role);
    const fallbackReasons = this.fallbackReasons();

    this.editor.health.clear(`role:missing:${role}`);
    this.editor.health.clear(`role:selected-missing:${role}`);

    if (!list.length) {
      this.active.delete(role);
      if (selected) {
        fallbackReasons.set(role, `selected implementation ${selected} is unavailable`);
        this.editor.health.report({
          id: `role:selected-missing:${role}`,
          severity: "warn",
          source: "roles",
          role,
          implementationId: selected,
          message: `selected role implementation ${selected} for ${role} is unavailable`,
        });
      } else {
        fallbackReasons.delete(role);
        this.editor.health.report({
          id: `role:missing:${role}`,
          severity: "warn",
          source: "roles",
          role,
          message: `no implementations registered for role ${role}`,
        });
      }
      return;
    }

    const selectedEntry = selected ? list.find((entry) => entry.implementationId === selected) : undefined;
    if (selectedEntry) {
      this.active.set(role, roleImplementationId(selectedEntry.implementationId));
      fallbackReasons.delete(role);
      return;
    }

    const fallbackImplementation = this.fallbackImplementations().get(role);
    const fallbackEntry = fallbackImplementation
      ? list.find((entry) => entry.implementationId === fallbackImplementation)
      : undefined;
    if (fallbackEntry) {
      this.active.set(role, roleImplementationId(fallbackEntry.implementationId));
      if (selected) {
        const reason = fallbackReason ?? `selected implementation ${selected} is unavailable; using fallback ${fallbackEntry.implementationId}`;
        fallbackReasons.set(role, reason);
        this.editor.health.report({
          id: `role:selected-missing:${role}`,
          severity: "warn",
          source: "roles",
          role,
          implementationId: selected,
          packageId: fallbackEntry.packageId,
          details: reason,
          message: `falling back to ${fallbackEntry.implementationId} for role ${role}`,
        });
      } else {
        fallbackReasons.delete(role);
      }
      return;
    }

    const previousEntry = previousActive ? list.find((entry) => entry.implementationId === previousActive) : undefined;
    if (previousEntry && !selected) {
      this.active.set(role, roleImplementationId(previousEntry.implementationId));
      fallbackReasons.delete(role);
      return;
    }

    const next = list[0];
    this.active.set(role, roleImplementationId(next.implementationId));
    if (selected) {
      const reason = fallbackReason ?? `selected implementation ${selected} is unavailable; using ${next.implementationId}`;
      fallbackReasons.set(role, reason);
      this.editor.health.report({
        id: `role:selected-missing:${role}`,
        severity: "warn",
        source: "roles",
        role,
        implementationId: selected,
        packageId: next.packageId,
        details: reason,
        message: `falling back to ${next.implementationId} for role ${role}`,
      });
      return;
    }

    fallbackReasons.delete(role);
  }

  private selections(): Map<RoleId, RoleImplementationId> {
    return this.editor.stateSlot(roleSelectionsSlot, () => new Map<RoleId, RoleImplementationId>());
  }

  private fallbackImplementations(): Map<RoleId, RoleImplementationId> {
    return this.editor.stateSlot(roleFallbackImplementationsSlot, () => new Map<RoleId, RoleImplementationId>());
  }

  private fallbackReasons(): Map<RoleId, string> {
    return this.editor.stateSlot(roleFallbackReasonsSlot, () => new Map<RoleId, string>());
  }
}

function packageIdFromModuleId(id: ModuleId | string): string | undefined {
  const value = String(id);
  for (const suffix of ["/runtime", "/interaction", "/ui", "/theme"]) {
    if (value.endsWith(suffix)) return value.slice(0, -suffix.length);
  }
  return undefined;
}

export function roleId(id: RoleId | string): RoleId {
  return id as RoleId;
}

export function roleImplementationId(id: RoleImplementationId | string): RoleImplementationId {
  return id as RoleImplementationId;
}
