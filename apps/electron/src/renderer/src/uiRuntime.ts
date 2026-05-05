import * as React from "react";
import * as ReactJsxRuntime from "react/jsx-runtime";
import ts from "typescript";
import type { PackageSnapshot, ToolRootTarget } from "@the-editor/editor";
import type { DiagnosticEntry } from "@the-editor/editor";
import * as editorUiModule from "@the-editor/editor/ui";
import { createUiHost, type EditorUiModule, type WorkbenchSnapshot } from "@the-editor/editor/ui";

type LoadedTarget = {
  moduleKey: string;
  contributionModuleId: string;
  dispose(): void;
  path: string;
  version: number;
  target: "ui" | "theme";
};

export class RendererPackageRuntime {
  readonly host = createUiHost({
    dispatch(dispatch) {
      return window.theEditor.policyRequest("command.dispatch", dispatch);
    },
  });

  private readonly listeners = new Set<() => void>();
  private readonly loaded = new Map<string, LoadedTarget>();
  private readonly localDiagnostics = new Map<string, DiagnosticEntry>();

  async update(snapshot: WorkbenchSnapshot): Promise<void> {
    const desired = new Map<string, { moduleKey: string; path: string; version: number; target: "ui" | "theme" }>();
    for (const pkg of snapshot.packages) {
      for (const target of ["ui", "theme"] as const) {
        if (!pkg.loadedTargets.includes(target)) continue;
        const path = pkg.entrypointPaths[target];
        const version = pkg.loadVersions[target];
        if (!path || !version) continue;
        const moduleKey = `${pkg.id}/${target}`;
        desired.set(moduleKey, { moduleKey, path, version, target });
      }
    }

    let changed = false;

    for (const [moduleKey, loaded] of [...this.loaded.entries()]) {
      const next = desired.get(moduleKey);
      if (!next) {
        this.unload(moduleKey);
        this.clearUiDiagnostic(moduleKey);
        changed = true;
        continue;
      }
      if (next.path === loaded.path && next.version === loaded.version) continue;

      try {
        const replacement = await this.instantiate(next.moduleKey, next.path, next.version, next.target);
        this.loaded.set(moduleKey, replacement);
        loaded.dispose();
        this.clearUiDiagnostic(moduleKey);
        changed = true;
      } catch (error) {
        changed = this.reportUiDiagnostic(moduleKey, next, error) || changed;
      }
    }

    for (const [moduleKey, next] of desired.entries()) {
      if (this.loaded.has(moduleKey)) continue;
      try {
        const loaded = await this.instantiate(next.moduleKey, next.path, next.version, next.target);
        this.loaded.set(moduleKey, loaded);
        this.clearUiDiagnostic(moduleKey);
        changed = true;
      } catch (error) {
        changed = this.reportUiDiagnostic(moduleKey, next, error) || changed;
      }
    }

    this.host.update({
      ...snapshot,
      diagnostics: [...(snapshot.diagnostics ?? []), ...this.localDiagnostics.values()],
    });

    if (changed) this.emitChange();
  }

  renderRole(role: string): React.ReactNode {
    return this.host.roles.render(role) as React.ReactNode;
  }

  renderToolRoot(root: ToolRootTarget): React.ReactNode {
    const nodes = this.host.tools.renderRoot(root) as React.ReactNode[];
    if (!nodes.length) return null;
    return nodes;
  }

  hasToolRoot(root: ToolRootTarget): boolean {
    return this.host.tools.toolsForRoot(root).length > 0;
  }

  subscribe(listener: () => void): () => void {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  }

  private async instantiate(moduleKey: string, entryPath: string, version: number, target: "ui" | "theme"): Promise<LoadedTarget> {
    const contributionModuleId = `${moduleKey}@${version}`;
    const loader = new RendererSourceModuleLoader(contributionModuleId, version);
    const activate = loader.loadDefault<EditorUiModule>(entryPath);
    let moduleDisposable: { dispose(): void } | undefined;
    await this.host.withModule(contributionModuleId, async () => {
      const result = await activate(this.host);
      if (result) moduleDisposable = result;
    });

    return {
      moduleKey,
      contributionModuleId,
      path: entryPath,
      version,
      target,
      dispose: () => {
        moduleDisposable?.dispose();
        this.host.unloadModule(contributionModuleId);
        loader.dispose();
      },
    };
  }

  private unload(moduleKey: string): void {
    const loaded = this.loaded.get(moduleKey);
    if (!loaded) return;
    this.loaded.delete(moduleKey);
    loaded.dispose();
  }

  private reportUiDiagnostic(
    moduleKey: string,
    next: { moduleKey: string; path: string; version: number; target: "ui" | "theme" },
    error: unknown,
  ): boolean {
    const id = `ui:${moduleKey}`;
    const previous = this.localDiagnostics.get(id);
    const entry: DiagnosticEntry = {
      id,
      severity: "error",
      source: "ui-runtime",
      packageId: moduleKey.split("/")[0],
      target: next.target,
      message: `failed to load ${moduleKey}`,
      details: error instanceof Error ? error.stack ?? error.message : String(error),
    };
    this.localDiagnostics.set(id, entry);
    console.error(`failed to load ${moduleKey}`, error);
    return JSON.stringify(previous) !== JSON.stringify(entry);
  }

  private clearUiDiagnostic(moduleKey: string): void {
    this.localDiagnostics.delete(`ui:${moduleKey}`);
  }

  private emitChange(): void {
    for (const listener of this.listeners) listener();
  }
}

class RendererSourceModuleLoader {
  private readonly cache = new Map<string, { exports: unknown }>();
  private readonly styleElements = new Map<string, HTMLStyleElement>();

  constructor(
    private readonly moduleId: string,
    private readonly version: number,
  ) {}

  loadDefault<T>(entryPath: string): T {
    const module = this.evaluate(entryPath).exports as { default?: T } | T;
    if (typeof module === "function") return module as T;
    if (module && typeof module === "object" && "default" in module) return module.default as T;
    throw new Error(`ui module does not export a default value: ${entryPath}`);
  }

  dispose(): void {
    for (const style of this.styleElements.values()) style.remove();
    this.styleElements.clear();
    this.cache.clear();
  }

  private evaluate(filePath: string): { exports: unknown } {
    const cached = this.cache.get(filePath);
    if (cached) return cached;

    if (filePath.endsWith(".css")) {
      const cssModule = { exports: this.loadCss(filePath) };
      this.cache.set(filePath, cssModule);
      return cssModule;
    }

    if (filePath.endsWith(".json")) {
      const jsonModule = { exports: JSON.parse(window.theEditor.readExtensionFile(filePath)) as unknown };
      this.cache.set(filePath, jsonModule);
      return jsonModule;
    }

    const source = window.theEditor.readExtensionFile(filePath);
    const transpiled = ts.transpileModule(source, {
      fileName: filePath,
      compilerOptions: {
        module: ts.ModuleKind.CommonJS,
        target: ts.ScriptTarget.ES2024,
        jsx: ts.JsxEmit.ReactJSX,
        esModuleInterop: true,
        allowJs: true,
      },
    }).outputText;

    const module = { exports: {} as unknown };
    this.cache.set(filePath, module);

    const wrapper = new Function("require", "module", "exports", "__filename", "__dirname", transpiled);
    wrapper(
      (specifier: string) => this.requireFrom(specifier, filePath),
      module,
      module.exports,
      filePath,
      dirname(filePath),
    );
    return module;
  }

  private requireFrom(specifier: string, importerPath: string): unknown {
    if (specifier === "react") return React;
    if (specifier === "react/jsx-runtime") return ReactJsxRuntime;
    if (specifier === "@the-editor/editor/ui") return editorUiModule;
    if (!specifier.startsWith(".") && !specifier.startsWith("/")) {
      throw new Error(`unsupported ui module import ${specifier} from ${importerPath}`);
    }
    const resolved = window.theEditor.resolveExtensionPath(specifier, importerPath);
    if (!resolved) throw new Error(`unable to resolve ${specifier} from ${importerPath}`);
    return this.evaluate(resolved).exports;
  }

  private loadCss(filePath: string): string {
    const existing = this.styleElements.get(filePath);
    if (existing) return existing.textContent ?? "";

    const cssText = window.theEditor.readExtensionFile(filePath);
    const style = document.createElement("style");
    style.dataset.theEditorModule = `${this.moduleId}@${this.version}`;
    style.dataset.theEditorPath = filePath;
    style.textContent = cssText;
    document.head.appendChild(style);
    this.styleElements.set(filePath, style);
    return cssText;
  }
}

function dirname(filePath: string): string {
  const normalized = filePath.replace(/\\/g, "/");
  const index = normalized.lastIndexOf("/");
  return index >= 0 ? normalized.slice(0, index) : normalized;
}

export type { WorkbenchSnapshot, PackageSnapshot };
