import { moduleId, type ModuleId } from "./contributions.js";
import type { EditorHost } from "./editorHost.js";
import type { EditorModule, EditorModuleRegistration } from "./modules.js";

export type PackageId = string & { readonly __brand?: "PackageId" };
export type PackageTargetKind = "runtime" | "interaction" | "ui" | "theme";
export type EditorPackageEntrypoint = string | EditorModule | EditorModuleRegistration;

export interface EditorPackageEntrypoints {
  runtime?: EditorPackageEntrypoint;
  interaction?: EditorPackageEntrypoint;
  ui?: EditorPackageEntrypoint;
  theme?: EditorPackageEntrypoint;
}

export interface EditorPackageManifest {
  id: PackageId | string;
  title?: string;
  entrypoints: EditorPackageEntrypoints;
  defaultTargets?: readonly PackageTargetKind[];
}

export interface PackageTargetActivationContext {
  editor: EditorHost;
  manifest: EditorPackageManifest;
  target: PackageTargetKind;
  entrypoint: EditorPackageEntrypoint;
  moduleId: ModuleId;
  loadVersion: number;
}

export interface PackageTargetAdapter {
  activate(ctx: PackageTargetActivationContext): void | Promise<void>;
  deactivate?(ctx: PackageTargetActivationContext): void | Promise<void>;
}

export type PackageTargetAdapterMap = Partial<Record<PackageTargetKind, PackageTargetAdapter>>;

interface RegisteredPackage {
  manifest: EditorPackageManifest;
  loadedTargets: Set<PackageTargetKind>;
  loadVersions: Map<PackageTargetKind, number>;
}

const packageTargetOrder: readonly PackageTargetKind[] = ["runtime", "interaction", "ui", "theme"] as const;
const targetSet = new Set<PackageTargetKind>(packageTargetOrder);

export interface PackageSnapshot {
  id: PackageId;
  title?: string;
  loadedTargets: PackageTargetKind[];
  availableTargets: PackageTargetKind[];
  entrypointPaths: Partial<Record<PackageTargetKind, string>>;
  loadVersions: Partial<Record<PackageTargetKind, number>>;
}

export class PackageRegistry {
  private readonly registered = new Map<PackageId, RegisteredPackage>();

  constructor(
    private readonly editor: EditorHost,
    private readonly adapters: PackageTargetAdapterMap = {},
  ) {}

  register(manifest: EditorPackageManifest): void {
    const id = packageId(manifest.id);
    const existing = this.registered.get(id);
    if (existing) {
      existing.manifest = { ...manifest, id };
      return;
    }
    this.registered.set(id, {
      manifest: { ...manifest, id },
      loadedTargets: new Set(),
      loadVersions: new Map(),
    });
  }

  async unregister(id: PackageId | string): Promise<void> {
    const normalized = packageId(id);
    const pkg = this.registered.get(normalized);
    if (!pkg) return;
    await this.unload(normalized);
    this.registered.delete(normalized);
  }

  async discover(): Promise<void> {
    await this.editor.hooks.event("packages.discover", undefined);
  }

  async load(id: PackageId | string, target?: PackageTargetKind): Promise<void> {
    const pkg = this.require(id);
    for (const nextTarget of this.targetsToLoad(pkg.manifest, target)) {
      if (pkg.loadedTargets.has(nextTarget)) continue;
      await this.loadTarget(pkg, nextTarget);
    }
  }

  async loadAll(): Promise<void> {
    for (const id of this.all()) {
      await this.load(id);
    }
  }

  async unload(id: PackageId | string, target?: PackageTargetKind): Promise<void> {
    const pkg = this.require(id);
    const targets = target ? [target] : [...pkg.loadedTargets];
    for (const nextTarget of targets.reverse()) {
      await this.unloadTarget(pkg, nextTarget);
    }
  }

  async reload(specifier?: string): Promise<void> {
    await this.discover();
    if (!specifier) {
      for (const id of this.all()) await this.reloadPackage(id);
      return;
    }

    const parsed = parsePackageSpecifier(specifier);
    await this.reloadPackage(parsed.id, parsed.target);
  }

  async reloadPackage(id: PackageId | string, target?: PackageTargetKind): Promise<void> {
    const pkg = this.require(id);
    const targets = this.targetsToLoad(pkg.manifest, target);
    if (!targets.length) {
      if (!target) return;
      throw new Error(`package ${pkg.manifest.id} has no ${target} target`);
    }

    const capturedFocus = this.editor.focus.current();
    const capturedTools = targets.includes("runtime")
      ? this.editor.tools.capturePackageInstances(String(pkg.manifest.id))
      : [];
    const capturedFocusedTool = capturedFocus.kind === "tool"
      ? capturedTools.find((tool) => tool.id === capturedFocus.toolId)
      : undefined;

    try {
      for (const nextTarget of targets) {
        const wasLoaded = pkg.loadedTargets.has(nextTarget);
        if (wasLoaded) await this.unloadTarget(pkg, nextTarget);
        try {
          await this.loadTarget(pkg, nextTarget);
        } catch (error) {
          this.editor.health.report({
            id: `packages:reload:${pkg.manifest.id}/${nextTarget}`,
            severity: "error",
            source: "packages",
            packageId: pkg.manifest.id,
            target: nextTarget,
            message: `failed to reload ${pkg.manifest.id}/${nextTarget}`,
            details: error instanceof Error ? error.stack ?? error.message : String(error),
          });
          throw error;
        }
      }

      if (capturedTools.length) this.editor.tools.restoreSnapshots(capturedTools);
      if (capturedFocusedTool) {
        const restoredTool = this.editor.tools.snapshotAll().find((tool) =>
          tool.typeId === capturedFocusedTool.typeId
          && tool.packageId === capturedFocusedTool.packageId
          && tool.persistenceKey === capturedFocusedTool.persistenceKey
          && tool.title === capturedFocusedTool.title,
        );
        if (restoredTool) this.editor.focus.setTool(restoredTool.id);
        else this.editor.focus.restoreTarget(capturedFocus);
      } else {
        this.editor.focus.restoreTarget(capturedFocus);
      }
      this.editor.workbench.invalidate("roles", "tools", "focus", "decorations", "gutters", "theme");
    } catch (error) {
      this.editor.focus.restoreTarget(capturedFocus);
      this.editor.workbench.invalidate("roles", "tools", "focus", "decorations", "gutters", "theme");
      throw error;
    }
  }

  all(): PackageId[] {
    return [...this.registered.keys()].sort((a, b) => a.localeCompare(b));
  }

  snapshot(): PackageSnapshot[] {
    return this.all().map((id) => {
      const pkg = this.require(id);
      return {
        id,
        title: pkg.manifest.title,
        loadedTargets: [...pkg.loadedTargets],
        availableTargets: availableTargets(pkg.manifest),
        entrypointPaths: snapshotEntrypointPaths(pkg.manifest),
        loadVersions: Object.fromEntries(pkg.loadVersions.entries()) as Partial<Record<PackageTargetKind, number>>,
      };
    });
  }

  private async loadTarget(pkg: RegisteredPackage, target: PackageTargetKind): Promise<void> {
    const entrypoint = pkg.manifest.entrypoints[target];
    if (!entrypoint) return;

    const nextVersion = (pkg.loadVersions.get(target) ?? 0) + 1;
    const ctx: PackageTargetActivationContext = {
      editor: this.editor,
      manifest: pkg.manifest,
      target,
      entrypoint,
      moduleId: packageTargetModuleId(pkg.manifest.id as PackageId, target),
      loadVersion: nextVersion,
    };

    this.editor.health.clear(`packages:load:${pkg.manifest.id}/${target}`);
    this.editor.health.clear(`packages:reload:${pkg.manifest.id}/${target}`);

    try {
      if (isEditorModuleEntrypoint(entrypoint)) {
        await this.editor.modules.load(normalizeEntrypoint(pkg.manifest.id as PackageId, target, entrypoint));
      } else {
        const adapter = this.adapters[target];
        if (adapter) await adapter.activate(ctx);
      }
    } catch (error) {
      this.editor.health.report({
        id: `packages:load:${pkg.manifest.id}/${target}`,
        severity: "error",
        source: "packages",
        packageId: pkg.manifest.id,
        target,
        message: `failed to load ${pkg.manifest.id}/${target}`,
        details: error instanceof Error ? error.stack ?? error.message : String(error),
      });
      throw error;
    }

    pkg.loadVersions.set(target, nextVersion);
    pkg.loadedTargets.add(target);
    this.editor.workbench.invalidate("roles", "tools", "decorations", "gutters", "focus", "interaction", "theme");
  }

  private async unloadTarget(pkg: RegisteredPackage, target: PackageTargetKind): Promise<void> {
    if (!pkg.loadedTargets.has(target)) return;
    const entrypoint = pkg.manifest.entrypoints[target];
    if (!entrypoint) return;

    const ctx: PackageTargetActivationContext = {
      editor: this.editor,
      manifest: pkg.manifest,
      target,
      entrypoint,
      moduleId: packageTargetModuleId(pkg.manifest.id as PackageId, target),
      loadVersion: pkg.loadVersions.get(target) ?? 0,
    };

    try {
      if (isEditorModuleEntrypoint(entrypoint)) {
        this.editor.modules.unload(ctx.moduleId);
      } else {
        await this.adapters[target]?.deactivate?.(ctx);
      }
    } catch (error) {
      this.editor.health.report({
        id: `packages:unload:${pkg.manifest.id}/${target}`,
        severity: "warn",
        source: "packages",
        packageId: pkg.manifest.id,
        target,
        message: `failed to unload ${pkg.manifest.id}/${target}`,
        details: error instanceof Error ? error.stack ?? error.message : String(error),
      });
      throw error;
    }

    pkg.loadedTargets.delete(target);
    this.editor.workbench.invalidate("roles", "tools", "decorations", "gutters", "focus", "interaction", "theme");
  }

  private targetsToLoad(manifest: EditorPackageManifest, target?: PackageTargetKind): PackageTargetKind[] {
    if (target) return manifest.entrypoints[target] ? [target] : [];
    const defaults = manifest.defaultTargets?.filter((nextTarget) => manifest.entrypoints[nextTarget]) ?? [];
    return defaults.length ? [...defaults] : availableTargets(manifest);
  }

  private require(id: PackageId | string): RegisteredPackage {
    const normalized = packageId(id);
    const pkg = this.registered.get(normalized);
    if (!pkg) throw new Error(`unknown package: ${normalized}`);
    return pkg;
  }
}

function normalizeEntrypoint(pkgId: PackageId, target: PackageTargetKind, entrypoint: EditorModule | EditorModuleRegistration): EditorModuleRegistration {
  const id = packageTargetModuleId(pkgId, target);
  if (typeof entrypoint === "function") {
    return { id, activate: entrypoint };
  }
  return {
    id,
    activate: entrypoint.activate,
  };
}

function isEditorModuleEntrypoint(entrypoint: EditorPackageEntrypoint): entrypoint is EditorModule | EditorModuleRegistration {
  return typeof entrypoint === "function" || (typeof entrypoint === "object" && entrypoint !== null && "activate" in entrypoint);
}

function availableTargets(manifest: EditorPackageManifest): PackageTargetKind[] {
  return packageTargetOrder.filter((target) => Boolean(manifest.entrypoints[target]));
}

function snapshotEntrypointPaths(manifest: EditorPackageManifest): Partial<Record<PackageTargetKind, string>> {
  const paths: Partial<Record<PackageTargetKind, string>> = {};
  for (const target of packageTargetOrder) {
    const entrypoint = manifest.entrypoints[target];
    if (typeof entrypoint === "string") paths[target] = entrypoint;
  }
  return paths;
}

export function packageTargetModuleId(id: PackageId | string, target: PackageTargetKind): ModuleId {
  return moduleId(`${packageId(id)}/${target}`);
}

export function packageId(id: PackageId | string): PackageId {
  return id as PackageId;
}

export function defineEditorPackage(manifest: EditorPackageManifest): EditorPackageManifest {
  return manifest;
}

export function parsePackageSpecifier(specifier: string): { id: PackageId; target?: PackageTargetKind } {
  const trimmed = specifier.trim();
  if (!trimmed) throw new Error("empty package specifier");
  for (const target of packageTargetOrder) {
    const suffix = `/${target}`;
    if (trimmed.endsWith(suffix)) {
      return { id: packageId(trimmed.slice(0, -suffix.length)), target };
    }
  }
  return { id: packageId(trimmed) };
}

export function isPackageTargetKind(value: string): value is PackageTargetKind {
  return targetSet.has(value as PackageTargetKind);
}
